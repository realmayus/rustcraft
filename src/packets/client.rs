use crate::protocol_types::compound::{Position, TagGroup, Recipe, BitSet, BlockEntity};
use crate::protocol_types::primitives::SizedVec;
use core::fmt::Debug;
use core::fmt::Display;
use uuid::Uuid;

use crate::connection::Connection;
use crate::protocol_types::primitives::VarInt;
use crate::protocol_types::traits::{ClientPacket, SizedProt, WriteProt, WriteProtPacket};
use crate::{packet, packet_base};
use async_nbt::NbtCompound;
use async_trait::async_trait;
use log::debug;
use rustcraft_derive::WriteProtPacket;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;

packet!(
    StatusRes 0x00 {
        json: String,
    }
);

packet!(
    PingRes 0x01 {
        payload: i64,
    }
);

packet!(
    EncryptionReq 0x01 {
        server_id: String,
        public_key: SizedVec<u8>,
        verify_token: SizedVec<u8>,
    }
);

packet!(
    LoginSuccess 0x02 {
        uuid: Uuid,
        username: String,
        num_properties: VarInt,
    }
);

packet!(
    ConfigurationKeepAlive 0x03 {
        id: i64,
    }
);

packet!(
    ConfigurationFinish 0x02 {}
);

packet!(
    RegistryData 0x05 {
        data: NbtCompound
    }
);

packet!(
    PlayLogin 0x29 {
        entity_id: i32,
        is_hardcore: bool,
        dimension_names: SizedVec<String>,
        max_players: VarInt,
        view_distance: VarInt,
        simulation_distance: VarInt,
        reduced_debug_info: bool,
        enable_respawn_screen: bool,
        do_limited_crafting: bool,
        dimension_type: String,
        dimension_name: String,
        hashed_seed: i64,
        game_mode: u8,
        previous_game_mode: u8,
        is_debug: bool,
        is_flat: bool,
        has_death_location: bool,
        death_dimension_name: {has_death_location == true} && String,
        death_location: {has_death_location == true} && Position,
        portal_cooldown: VarInt,
    }
);

packet!(
    PlayKeepAlive 0x24 {
        id: i64,
    }
);

packet!(
    SetHeldItem 0x4f {
        slot: u8,
    }
);

packet!(
    UpdateRecipes 0x6f {
        recipes: SizedVec<Recipe>,
    }
);

packet!(
    UpdateTags 0x70 {
        tag_groups: SizedVec<TagGroup>,
    }
);

packet!(
    SynchronizePlayerPosition 0x3e {
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
        flags: u8,
        teleport_id: VarInt,
    }
);

packet!(
    ChunkDataAndUpdateLight 0x25 {
        chunk_x: i32,
        chunk_z: i32,
        heightmaps: NbtCompound,
        data: SizedVec<u8>,
        block_entities: SizedVec<BlockEntity>,
        sky_light_mask: BitSet,
        block_light_mask: BitSet,
        empty_sky_light_mask: BitSet,
        empty_block_light_mask: BitSet,
        sky_lights: SizedVec<SizedVec<u8>>,
        block_lights: SizedVec<SizedVec<u8>>,
    }
);

packet!(
    SetDefaultSpawnPosition 0x52 {
        location: Position,
        angle: f32,
    }
);


#[derive(WriteProtPacket)]
pub(crate) enum ClientPackets {
    StatusRes(StatusRes),
    PingRes(PingRes),
    EncryptionReq(EncryptionReq),
    LoginSuccess(LoginSuccess),
    ConfigurationKeepAlive(ConfigurationKeepAlive),
    ConfigurationFinish(ConfigurationFinish),
    RegistryData(RegistryData),
    PlayLogin(PlayLogin),
    PlayKeepAlive(PlayKeepAlive),
    SetHeldItem(SetHeldItem),
    UpdateRecipes(UpdateRecipes),
    UpdateTags(UpdateTags),
    SynchronizePlayerPosition(SynchronizePlayerPosition),
    ChunkDataAndUpdateLight(ChunkDataAndUpdateLight),
    SetDefaultSpawnPosition(SetDefaultSpawnPosition),
}
