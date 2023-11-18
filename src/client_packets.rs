use crate::protocol_types::Position;
use core::fmt::Debug;
use core::fmt::Display;

use async_trait::async_trait;
use tokio::io::AsyncWrite;
use log::debug;
use crate::{packet, packet_base};
use crate::protocol_types::{ClientPacket, SizedProt, WriteProt, VarInt};

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
        public_key: Vec<u8>,
        verify_token: Vec<u8>,
    }
);

packet!(
    LoginSuccess 0x02 {
        uuid1: u64,
        uuid2: u64,
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
    PlayLogin 0x29 {
        entity_id: i32,
        is_hardcore: bool,
        dimension_names: Vec<String>,
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