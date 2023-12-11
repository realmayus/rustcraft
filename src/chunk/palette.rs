use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;

use async_trait::async_trait;
use tokio::io::AsyncWrite;

use crate::chunk::{
    Biome, BlockState, GLOBAL_PALETTE_BITS_BIOMES, GLOBAL_PALETTE_BITS_BLOCKS,
    MAX_PALETTE_BITS_BIOMES, MAX_PALETTE_BITS_BLOCKS, MIN_PALETTE_BITS_BIOMES,
    MIN_PALETTE_BITS_BLOCKS, SECTION_BLOCKS, SECTION_EDGE,
};
use crate::chunk::packed_array::PackedArray;
use crate::err::ProtError;
use crate::protocol_types::compound::Position;
use crate::protocol_types::primitives::VarInt;
use crate::protocol_types::traits::WriteProt;

#[derive(Debug)]
pub(crate) enum PaletteKind {
    Blocks,
    Biomes,
}

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
pub(crate) enum PaletteValue {
    Block(BlockState),
    Biome(Biome),
}

impl From<PaletteValue> for u64 {
    fn from(value: PaletteValue) -> Self {
        match value {
            PaletteValue::Block(value) => value as u64,
            PaletteValue::Biome(value) => value as u64,
        }
    }
}

impl From<PaletteValue> for VarInt {
    fn from(value: PaletteValue) -> Self {
        match value {
            PaletteValue::Block(value) => value.into(),
            PaletteValue::Biome(value) => value.into(),
        }
    }
}

impl PaletteKind {
    fn int_val(&self, value: u64) -> PaletteValue {
        match &self {
            PaletteKind::Blocks => PaletteValue::Block(value as BlockState),
            PaletteKind::Biomes => PaletteValue::Biome(value as Biome),
        }
    }
    pub(crate) fn neutral(&self) -> PaletteValue {
        match &self {
            PaletteKind::Blocks => PaletteValue::Block(0 as BlockState),
            PaletteKind::Biomes => PaletteValue::Biome(39 as Biome),
        }
    }
    fn container_length(&self) -> usize {
        match &self {
            PaletteKind::Blocks => SECTION_BLOCKS,
            PaletteKind::Biomes => SECTION_BLOCKS / 4,
        }
    }

    fn min_palette_bits(&self) -> usize {
        match &self {
            PaletteKind::Blocks => MIN_PALETTE_BITS_BLOCKS,
            PaletteKind::Biomes => MIN_PALETTE_BITS_BIOMES,
        }
    }

    fn max_palette_bits(&self) -> usize {
        match &self {
            PaletteKind::Blocks => MAX_PALETTE_BITS_BLOCKS,
            PaletteKind::Biomes => MAX_PALETTE_BITS_BIOMES,
        }
    }

    fn global_palette_bits(&self) -> usize {
        match &self {
            PaletteKind::Blocks => GLOBAL_PALETTE_BITS_BLOCKS,
            PaletteKind::Biomes => GLOBAL_PALETTE_BITS_BIOMES,
        }
    }

    fn section_edge(&self) -> usize {
        match &self {
            PaletteKind::Blocks => SECTION_EDGE,
            PaletteKind::Biomes => SECTION_EDGE / 4,
        }
    }

    fn section_blocks(&self) -> usize {
        match &self {
            PaletteKind::Blocks => SECTION_BLOCKS,
            PaletteKind::Biomes => SECTION_BLOCKS / 4,
        }
    }
}

pub(crate) struct PalettedContainer {
    pub(crate) palette: Option<Palette>,
    pub(crate) data: PackedArray,
    kind: PaletteKind,
}

impl PalettedContainer {
    pub(crate) fn new_blocks() -> Self {
        let kind = PaletteKind::Blocks;
        Self {
            palette: Some(Palette::new(PaletteKind::Blocks)),
            data: PackedArray::new(kind.container_length(), kind.min_palette_bits()),
            kind,
        }
    }
    pub(crate) fn new_biomes() -> Self {
        let kind = PaletteKind::Biomes;
        Self {
            palette: Some(Palette::new(PaletteKind::Biomes)),
            data: PackedArray::new(kind.container_length(), kind.min_palette_bits()),
            kind,
        }
    }

    pub(crate) fn block_index(&self, position: Position) -> Option<usize> {
        if position.x >= self.kind.section_edge() as i32
            || position.y >= self.kind.section_edge() as i32
            || position.z >= self.kind.section_edge() as i32
        {
            None
        } else {
            match &self.kind {
                PaletteKind::Blocks => {
                    Some(((position.y << 8) | (position.z << 4) | position.x) as usize)
                }
                PaletteKind::Biomes => {
                    Some(((position.y << 4) | (position.z << 2) | position.x) as usize)
                }
            }
        }
    }

    pub(crate) fn get_at(&self, position: Position) -> PaletteValue {
        let index = self.block_index(position);
        if let Some(index) = index {
            let block = match &self.palette {
                Some(palette) => palette.get(self.data.get(index).unwrap() as u32),
                None => self.kind.int_val(self.data.get(index).unwrap()),
            };
            block
        } else {
            self.kind.neutral()
        }
    }

    pub(crate) fn set_at(
        &mut self,
        position: Position,
        block_state: PaletteValue,
    ) -> Result<(), ProtError> {
        let index = self.block_index(position);
        if index.is_none() {
            return Err(ProtError::PositionOutOfBounds(position));
        }
        let block = match &mut self.palette {
            Some(palette) => {
                let palette_index = palette.index_or_insert(block_state);
                if self.resize_palette() {
                    block_state.into()
                } else {
                    palette_index as u64
                }
            }
            None => block_state.into(),
        };
        self.data.set(index.unwrap(), block);
        Ok(())
    }

    fn resize_palette(&mut self) -> bool {
        let palette = self.palette.as_ref().unwrap();
        if palette.len() - 1 > self.data.max_value() as usize {
            let new_size = self.data.bits_per_value() + 1;
            if new_size > self.kind.max_palette_bits() {
                self.data = self.data.resized(self.kind.global_palette_bits());
                for i in 0..self.kind.section_blocks() {
                    let block = palette.get(self.data.get(i).unwrap() as u32);
                    self.data.set(i, block.into());
                }
                self.palette = None;
                return true;
            } else {
                self.data = self.data.resized(new_size);
                return false;
            }
        }
        false
    }
}

#[async_trait]
impl WriteProt for PalettedContainer {
    async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), String> {
        if self.palette.as_ref().is_some_and(|pal| pal.len() == 1) {
            // TODO support non-air single palettes
            0u8.write(stream).await?; // bits per value is 0 in this case
            VarInt::from(*self.palette.as_ref().unwrap().id_to_state.get(&0).unwrap())
                .write(stream)
                .await?;
        } else {
            (self.data.bits_per_value() as u8).write(stream).await?;
            if let Some(palette) = &self.palette {
                palette.write(stream).await?;
            }
        }
        self.data.write(stream).await?; // long array
        Ok(())
    }
}

pub(crate) struct Palette {
    pub(crate) id_to_state: HashMap<u32, PaletteValue>,
    state_to_id: HashMap<PaletteValue, u32>,
    kind: PaletteKind,
}

impl Palette {
    fn new(kind: PaletteKind) -> Self {
        Self {
            id_to_state: HashMap::from([(0, kind.neutral())]),
            state_to_id: HashMap::from([(kind.neutral(), 0)]),
            kind,
        }
    }

    fn index_or_insert(&mut self, state: PaletteValue) -> u32 {
        if let Some(id) = self.state_to_id.get(&state) {
            *id
        } else {
            let id = self.id_to_state.len() as u32;
            self.id_to_state.insert(id, state);
            self.state_to_id.insert(state, id);
            id
        }
    }

    fn get(&self, id: u32) -> PaletteValue {
        self.id_to_state
            .get(&id)
            .copied()
            .unwrap_or_else(|| self.kind.neutral())
    }

    fn len(&self) -> usize {
        self.id_to_state.len()
    }
}

#[async_trait]
impl WriteProt for Palette {
    async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), String> {
        VarInt::from(self.len()).write(stream).await?;
        let mut i = 0u32;  // todo check this
        for (id, state) in &self.id_to_state {
            // assert_eq!(id, &i);
            i += 1;
            VarInt::from(*state).write(stream).await?;
        }

        Ok(())
    }
}

impl Debug for Palette {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "kind: {:?}, map: {:?}", self.kind, self.id_to_state).unwrap();
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_palette() {
        let mut palette = Palette::new(PaletteKind::Blocks);
        let id = palette.index_or_insert(palette.kind.int_val(1));
        assert_eq!(id, 1);
        let id = palette.index_or_insert(palette.kind.int_val(2));
        assert_eq!(id, 2);
        let id = palette.index_or_insert(palette.kind.int_val(1));
        assert_eq!(id, 1);
        let id = palette.index_or_insert(palette.kind.int_val(3));
        assert_eq!(id, 3);
        let id = palette.index_or_insert(palette.kind.int_val(2));
        assert_eq!(id, 2);
        let id = palette.index_or_insert(palette.kind.int_val(3));
        assert_eq!(id, 3);
    }

    #[test]
    fn set_block_get_block() {
        let mut container = PalettedContainer::new_blocks();
        container
            .set_at(Position::new(0, 0, 0), container.kind.int_val(1))
            .unwrap();
        assert_eq!(
            container.get_at(Position::new(0, 0, 0)),
            container.kind.int_val(1)
        );
        container
            .set_at(Position::new(0, 0, 0), PaletteValue::Block(2))
            .unwrap();
        assert_eq!(
            container.get_at(Position::new(0, 0, 0)),
            PaletteValue::Block(2)
        );
        container
            .set_at(Position::new(0, 0, 0), PaletteValue::Block(0))
            .unwrap();
        assert_eq!(
            container.get_at(Position::new(0, 0, 0)),
            PaletteValue::Block(0)
        );
    }

    #[test]
    fn test_multiple_positions() {
        let mut container = PalettedContainer::new_blocks();
        for i in 0..16 {
            for j in 0..16 {
                for k in 0..16 {
                    container
                        .set_at(
                            Position::new(i, j, k),
                            PaletteValue::Block((i + j - k).abs() as BlockState),
                        )
                        .unwrap();
                }
            }
        }
        for i in 0..16 {
            for j in 0..16 {
                for k in 0..16 {
                    assert_eq!(
                        container.get_at(Position::new(i, j, k)),
                        PaletteValue::Block((i + j - k).abs() as BlockState)
                    );
                }
            }
        }
    }

    #[test]
    fn test_multiple_positions_same_block() {
        let mut container = PalettedContainer::new_blocks();
        for i in 0..16 {
            for j in 0..16 {
                for k in 0..16 {
                    container
                        .set_at(Position::new(i, j, k), PaletteValue::Block(3))
                        .unwrap();
                }
            }
        }
        for i in 0..16 {
            for j in 0..16 {
                for k in 0..16 {
                    assert_eq!(
                        container.get_at(Position::new(i, j, k)),
                        PaletteValue::Block(3)
                    );
                }
            }
        }
    }
}
