use std::collections::HashMap;

use tokio::io::{AsyncRead, AsyncWrite};

use crate::chunk::{AIR, BlockState, GLOBAL_PALETTE_BITS};
use crate::chunk::packed_array::PackedArray;
use crate::err::ProtError;
use crate::protocol_types::compound::Position;
use crate::protocol_types::traits::{ReadProt, WriteProt};

pub(crate) struct PalettedContainer {
    pub(crate) palette: Option<Palette>,
    pub(crate) data: PackedArray,
    pub(crate) air_count: usize,
}


impl PalettedContainer {
    pub(crate) fn new() -> Self {
        Self {
            palette: Some(Palette::new()),
            data: PackedArray::new(16 * 16 * 16, 4),
            air_count: 16 * 16 * 16,
        }
    }

    pub(crate) fn new_global() -> Self {
        Self {
            palette: None,
            data: PackedArray::new(16 * 16 * 16, 4),
            air_count: 16 * 16 * 16,
        }
    }

    fn count_air(&self) -> u32 {
        let mut count = 0;
        for i in 0..self.data.len() {
            let block = match &self.palette {
                Some(palette) => palette.get(self.data.get(i).unwrap() as u32),
                None => self.data.get(i).unwrap() as BlockState,
            };
            if block == AIR {
                count += 1;
            }
        }
        count
    }

    fn block_index(position: Position) -> Option<usize> {
        if position.x >= 16 || position.y >= 16 || position.z >= 16 {
            None
        } else {
            Some(((position.y << 8) | (position.z << 4) | position.x) as usize)
        }
    }

    pub(crate) fn block_at(&self, position: Position) -> BlockState {
        let index = Self::block_index(position);
        if let Some(index) = index {
            let block = match &self.palette {
                Some(palette) => palette.get(self.data.get(index).unwrap() as u32),
                None => self.data.get(index).unwrap() as BlockState,
            };
            block as BlockState
        } else {
            AIR
        }
    }

    pub(crate) fn set_block_at(&mut self, position: Position, block_state: BlockState) -> Result<(), ProtError> {
        let index = Self::block_index(position);
        if index.is_none() {
            return Err(ProtError::PositionOutOfBounds(position));
        }
        self.update_air_at(position, block_state);
        let block = match &mut self.palette {
            Some(palette) => {
                let palette_index = palette.index_or_insert(block_state);
                self.resize_palette();
                palette_index
            }
            None => block_state as u32,
        };
        self.data.set(index.unwrap(), block as u64);
        Ok(())
    }

    pub(crate) fn air_count(&self) -> usize {
        self.air_count
    }

    fn update_air_at(&mut self, position: Position, new: BlockState) {
        let old = self.block_at(position);
        if old == AIR && new != AIR {
            self.air_count -= 1;
        } else if old != AIR && new == AIR {
            self.air_count += 1;
        }
    }

    fn resize_palette(&mut self) {
        let palette = self.palette.as_ref().unwrap();
        if palette.len() - 1 > self.data.max_value() as usize {
            let new_size = self.data.bits_per_value() + 1;
            if new_size > 8usize {
                self.data = self.data.resized(GLOBAL_PALETTE_BITS);
                for i in 0..16 * 16 * 16 {
                    let block = palette.get(self.data.get(i).unwrap() as u32);
                    self.data.set(i, block as u64);
                }
                self.palette = None;
            } else {
                self.data = self.data.resized(new_size);
            }
        }
    }
}

pub(crate) struct Palette {
    id_to_state: HashMap<u32, BlockState>,
    state_to_id: HashMap<BlockState, u32>,
}

impl Palette {
    fn new() -> Self {
        Self {
            id_to_state: HashMap::from([(0, AIR as BlockState)]),
            state_to_id: HashMap::from([(0, AIR as BlockState)]),
        }
    }

    fn index_or_insert(&mut self, state: BlockState) -> u32 {
        if let Some(id) = self.state_to_id.get(&state) {
            *id
        } else {
            let id = self.id_to_state.len() as u32;
            self.id_to_state.insert(id, state);
            self.state_to_id.insert(state, id);
            id
        }
    }

    fn get(&self, id: u32) -> BlockState {
        self.id_to_state.get(&id).copied().unwrap_or_else(|| AIR as BlockState)
    }

    fn len(&self) -> usize {
        self.id_to_state.len()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_palette() {
        let mut palette = Palette::new();
        let id = palette.index_or_insert(1);
        assert_eq!(id, 1);
        let id = palette.index_or_insert(2);
        assert_eq!(id, 2);
        let id = palette.index_or_insert(1);
        assert_eq!(id, 1);
        let id = palette.index_or_insert(3);
        assert_eq!(id, 3);
        let id = palette.index_or_insert(2);
        assert_eq!(id, 2);
        let id = palette.index_or_insert(3);
        assert_eq!(id, 3);
    }

    #[test]
    fn test_air_count() {
        let mut container = PalettedContainer::new();
        assert_eq!(container.air_count(), 16 * 16 * 16);
        container.set_block_at(Position::new(0, 0, 0), 1).unwrap();
        assert_eq!(container.air_count(), 16 * 16 * 16 - 1);
        container.set_block_at(Position::new(0, 0, 0), 0).unwrap();
        assert_eq!(container.air_count(), 16 * 16 * 16);
    }

    #[test]
    fn set_block_get_block() {
        let mut container = PalettedContainer::new();
        container.set_block_at(Position::new(0, 0, 0), 1).unwrap();
        assert_eq!(container.block_at(Position::new(0, 0, 0)), 1);
        container.set_block_at(Position::new(0, 0, 0), 2).unwrap();
        assert_eq!(container.block_at(Position::new(0, 0, 0)), 2);
        container.set_block_at(Position::new(0, 0, 0), 0).unwrap();
        assert_eq!(container.block_at(Position::new(0, 0, 0)), 0);
    }

    #[test]
    fn test_resize() {
        let mut container = PalettedContainer::new();
        for i in 0..16 * 16 * 16 {
            container.set_block_at(Position::new(0, 0, 0), i as u32).unwrap();
        }
        assert_eq!(container.air_count(), 16 * 16 * 16 - 1);
    }

    #[test]
    fn test_multiple_positions() {
        let mut container = PalettedContainer::new();
        let mut zero_count = 0;
        for i in 0..16 {
            for j in 0..16 {
                for k in 0..16 {
                    container.set_block_at(Position::new(i, j, k), (i + j - k).abs() as BlockState).unwrap();
                    if i + j - k == 0 {
                        zero_count += 1;
                    }
                }
            }
        }
        for i in 0..16 {
            for j in 0..16 {
                for k in 0..16 {
                    assert_eq!(container.block_at(Position::new(i, j, k)), (i + j - k).abs() as BlockState);
                }
            }
        }
        assert_eq!(container.air_count(), zero_count);
    }

    #[test]
    fn test_multiple_positions_same_block() {
        let mut container = PalettedContainer::new();
        for i in 0..16 {
            for j in 0..16 {
                for k in 0..16 {
                    container.set_block_at(Position::new(i, j, k), 3).unwrap();
                }
            }
        }
        for i in 0..16 {
            for j in 0..16 {
                for k in 0..16 {
                    assert_eq!(container.block_at(Position::new(i, j, k)), 3 as BlockState);
                }
            }
        }
        assert_eq!(container.air_count(), 0);
    }
}
