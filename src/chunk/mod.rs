use crate::chunk::section::ChunkSection;

mod palette;
pub(crate) mod section;

mod packed_array;
pub(crate) mod world;

type ChunkCol = Vec<ChunkSection>;
type ChunkColRef<'a> = Vec<&'a ChunkSection>;

type BlockState = u32;
type Biome = u32;

const GLOBAL_PALETTE_BITS_BLOCKS: usize = 15;
const GLOBAL_PALETTE_BITS_BIOMES: usize = 6;
const MIN_PALETTE_BITS_BLOCKS: usize = 4;
const MIN_PALETTE_BITS_BIOMES: usize = 1;
const MAX_PALETTE_BITS_BLOCKS: usize = 8;
const MAX_PALETTE_BITS_BIOMES: usize = 3;
const SECTION_EDGE: usize = 16;
const SECTION_BLOCKS: usize = SECTION_EDGE * SECTION_EDGE * SECTION_EDGE;
const AIR: u32 = 0;

pub(crate) const COLUMN_HEIGHT: usize = 24; // 24 chunk sections
