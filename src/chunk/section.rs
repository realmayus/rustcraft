use crate::chunk::palette::{PaletteValue, PalettedContainer};
use crate::chunk::{Biome, BlockState, AIR, SECTION_BLOCKS};
use crate::err::ProtError;
use crate::protocol_types::compound::Position;
use crate::protocol_types::traits::WriteProt;
use async_trait::async_trait;
use tokio::io::AsyncWrite;

pub(crate) struct ChunkSection {
    air_count: u16,
    blocks: PalettedContainer,
    biomes: PalettedContainer,
}
impl ChunkSection {
    pub(crate) fn new() -> Self {
        Self {
            air_count: SECTION_BLOCKS as u16,
            blocks: PalettedContainer::new_blocks(),
            biomes: PalettedContainer::new_biomes(),
        }
    }
    fn update_air_at(&mut self, position: Position, new: BlockState) {
        let old = self.block(position);
        if old == AIR && new != AIR {
            self.air_count -= 1;
        } else if old != AIR && new == AIR {
            self.air_count += 1;
        }
    }

    pub(crate) fn set_block(
        &mut self,
        position: Position,
        state: BlockState,
    ) -> Result<(), ProtError> {
        self.update_air_at(position, state);
        self.blocks.set_at(position, PaletteValue::Block(state))?;

        Ok(())
    }

    pub(crate) fn block(&self, position: Position) -> BlockState {
        match self.blocks.get_at(position) {
            PaletteValue::Block(state) => state,
            _ => AIR,
        }
    }

    // Biomes are 4x4 regions within a chunk
    pub(crate) fn set_biome(&mut self, position: Position, biome: u32) -> Result<(), ProtError> {
        self.biomes.set_at(position, PaletteValue::Biome(biome))?;
        Ok(())
    }

    pub(crate) fn biome(&self, position: Position) -> Biome {
        match self.biomes.get_at(position) {
            PaletteValue::Biome(biome) => biome,
            _ => 39,
        }
    }

    pub(crate) fn fill(&mut self, state: BlockState) {
        for i in 0..16 {
            for j in 0..16 {
                for k in 0..16 {
                    self.set_block(Position::new(i, j, k), state).unwrap();
                }
            }
        }
    }

    pub(crate) fn air_count(&self) -> u16 {
        self.air_count
    }
}
#[async_trait]
impl WriteProt for ChunkSection {
    async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), String> {
        (SECTION_BLOCKS as u16 - self.air_count)
            .write(stream)
            .await?;
        // println!("air count: {}", self.air_count);
        self.blocks.write(stream).await?;
        // println!(
        //     "block: palette: {:?}\ndata: {:?}",
        //     self.blocks.palette, self.blocks.data
        // );
        self.biomes.write(stream).await?;
        // println!(
        //     "biomes: palette: {:?}\ndata: {:?}",
        //     self.biomes.palette, self.biomes.data
        // );
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_air_count() {
        let mut chunk = ChunkSection::new();
        assert_eq!(chunk.air_count(), 16 * 16 * 16);
        chunk.set_block(Position::new(0, 0, 0), 1).unwrap();
        assert_eq!(chunk.air_count(), 16 * 16 * 16 - 1);
        chunk.set_block(Position::new(0, 0, 0), 0).unwrap();
        assert_eq!(chunk.air_count(), 16 * 16 * 16);
    }

    #[test]
    fn test_resize() {
        let mut chunk = ChunkSection::new();
        for i in 0..16 * 16 * 16 {
            chunk.set_block(Position::new(0, 0, 0), i as u32).unwrap();
        }
        assert_eq!(chunk.air_count(), 16 * 16 * 16 - 1);
    }

    #[test]
    fn set_and_get_biome() {
        let mut chunk = ChunkSection::new();
        chunk.set_biome(Position::new(0, 0, 0), 1).unwrap();
        assert_eq!(chunk.biome(Position::new(0, 0, 0)), 1);
    }

    #[test]
    fn set_and_get_biomes() {
        let mut chunk = ChunkSection::new();
        for i in 0..4 {
            for j in 0..4 {
                chunk
                    .set_biome(Position::new(i, 0, j), (i * 16 + j) as u32)
                    .unwrap();
            }
        }
        for i in 0..4 {
            for j in 0..4 {
                assert_eq!(chunk.biome(Position::new(i, 0, j)), (i * 16 + j) as u32);
            }
        }
    }
    #[test]
    fn set_and_get_blocks() {
        let mut chunk = ChunkSection::new();
        for i in 0..16 {
            for j in 0..16 {
                chunk
                    .set_block(Position::new(i, 0, j), (i * 16 + j) as u32)
                    .unwrap();
            }
        }
        for i in 0..16 {
            for j in 0..16 {
                assert_eq!(chunk.block(Position::new(i, 0, j)), (i * 16 + j) as u32);
            }
        }
    }
}
