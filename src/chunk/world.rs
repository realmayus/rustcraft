use std::collections::HashMap;

use log::{debug, error};
use uuid::Uuid;

use crate::chunk::{BlockState, ChunkCol, COLUMN_HEIGHT, SECTION_EDGE};
use crate::chunk::section::ChunkSection;
use crate::packets::client::{BlockUpdate, ClientPackets};
use crate::protocol_types::compound::Position;

pub(crate) struct WorldPlayer {
    pub(crate) uuid: Uuid,
    pub(crate) username: String,
    pub(crate) position: Position,
}

pub(crate) struct World {
    chunks: HashMap<Position, ChunkSection>,
    players: HashMap<Uuid, WorldPlayer>,
}

impl World {
    fn new() -> Self {
        Self {
            chunks: HashMap::new(),
            players: HashMap::new(),
        }
    }

    pub(crate) fn new_grass() -> Self {
        let mut chunks: HashMap<Position, ChunkSection> = HashMap::new();
        for x in -3..=3 {
            for z in -3..=3 {
                for y in 0..COLUMN_HEIGHT {
                    let mut chunk = ChunkSection::new();
                    if y < 3 {
                        chunk.fill(9);
                        chunk.set_block(Position::new(5, 5, 5), 1).unwrap();
                    }
                    chunks.insert(Position::new(x, y as i32 - 4, z), chunk);
                }
            }
        }
        Self {
            chunks,
            players: HashMap::new(),
        }
    }

    /**
     * Returns the chunk at the given chunk position.
     */
    fn chunk(&self, chunk_pos: &Position) -> Option<&ChunkSection> {
        self.chunks.get(chunk_pos)
    }

    fn chunk_mut(&mut self, chunk_pos: &Position) -> Option<&mut ChunkSection> {
        self.chunks.get_mut(chunk_pos)
    }

    /**
        * Returns the chunk pos at the given global position.
    */
    fn chunk_pos_for_global_pos(&self, global_pos: Position) -> Position {
        let offset_neg_x = if global_pos.x < 0 { -1 } else { 0 };
        let offset_neg_y = if global_pos.y < 0 { -1 } else { 0 };
        let offset_neg_z = if global_pos.z < 0 { -1 } else { 0 };
        Position::new(global_pos.x / SECTION_EDGE as i32 + offset_neg_x, global_pos.y / SECTION_EDGE as i32 + offset_neg_y, global_pos.z / SECTION_EDGE as i32 + offset_neg_z)
    }


    /**
        * Returns the relative position of a block within its chunk given its global position.
    */
    fn rel_chunk_pos_for_global_pos(&self, global_pos: Position) -> Position {
        Position::new(global_pos.x.rem_euclid(SECTION_EDGE as i32), (global_pos.y + 64).rem_euclid(SECTION_EDGE as i32), global_pos.z.rem_euclid(SECTION_EDGE as i32))
    }

    /**
     * Returns a vector of pos + chunk columns that are within the radius/render distance of the given position.
     */
    pub(crate) fn get_chunk_radius(&self, position: Position, radius: i32) -> Vec<(i32, i32, ChunkCol)> {
        let mut chunks: Vec<(i32, i32, ChunkCol)> = Vec::new();
        for x in -radius..=radius {
            for z in -radius..=radius {
                let mut column: ChunkCol = Vec::with_capacity(COLUMN_HEIGHT);
                for y in 0..COLUMN_HEIGHT {
                    if let Some(chunk) = self.chunk(&Position::new(position.x / SECTION_EDGE as i32 + x, y as i32 - 4, position.z / SECTION_EDGE as i32 + z)) {
                        column.push(chunk.clone());
                    } else {
                        error!("Chunk not found at {}, {}, {}", position.x / SECTION_EDGE as i32 + x, y as i32 - 4, position.z / SECTION_EDGE as i32 + z);
                    }
                }
                chunks.push((x, z, column));
            }
        }
        chunks
    }

    pub(crate) fn set_block(&mut self, position: Position, block: BlockState) -> Option<Vec<ClientPackets>> {
        let chunk_pos = self.chunk_pos_for_global_pos(position);
        let rel_pos = self.rel_chunk_pos_for_global_pos(position);
        if let Some(chunk) = self.chunk_mut(&chunk_pos) {
            chunk.set_block(rel_pos, block).unwrap();
            // todo generate update packets for all players who have this chunk loaded
            Some(vec![ClientPackets::BlockUpdate(BlockUpdate::new(position, block.into()))])
        } else {
            None
        }
    }
    
    pub(crate) fn set_player(&mut self, player: WorldPlayer) {
        self.players.insert(player.uuid, player);
    }
    
    pub(crate) fn player(&self, uuid: Uuid) -> Option<&WorldPlayer> {
        self.players.get(&uuid)
    }
}