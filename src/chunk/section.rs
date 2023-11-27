use crate::chunk::BlockState;
use crate::chunk::palette::PalettedContainer;
use crate::protocol_types::compound::Position;

fn block_index(pos: Position) -> usize {
    (pos.y as usize) << 8 | (pos.z as usize) << 4 | (pos.x as usize)
}

pub(crate) struct ChunkSection {
    block_count: i16,
    block_states: PalettedContainer,
    biomes: PalettedContainer,
}

// impl ChunkSection {
//     pub(crate) fn new() -> ChunkSection {
//         ChunkSection {
//             block_count: 0,
//             block_states: PalettedContainer::new(),
//             biomes: PalettedContainer::new(),
//         }
//     }
//
//     pub(crate) fn set_block(&mut self, pos: Position, block: BlockState) {
//         let index = block_index(pos);
//         let paletted_index;
//         let index_in_palette = self.block_states.palette.id_for_state(block);
//         if index_in_palette.is_some() {
//             paletted_index = index_in_palette.unwrap();
//         } else {
//             self.block_states.palette.push(block);
//             paletted_index = self.block_states.palette.len() - 1;
//
//         }
//     }
// }
//
//
