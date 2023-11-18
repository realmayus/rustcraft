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
