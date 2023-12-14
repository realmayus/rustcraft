use std::sync::RwLock;
use crate::serve::start_server;
use async_nbt::NbtCompound;
use dotenv::dotenv;
use log::info;
use openssl::pkey::Private;
use openssl::rsa::Rsa;
use crate::chunk::world::World;

mod chunk;
mod connection;
mod data;
mod encryption;
mod err;
mod packets;
mod protocol_types;
mod protocol_util;
mod serve;
pub mod web;

const MSG: &str = r#"
{
    "version": {
        "name": "1.20.2",
        "protocol": 764
    },
    "players": {
        "max": 100,
        "online": 5,
        "sample": [
            {
                "name": "thinkofdeath",
                "id": "4566e69f-c907-48ee-8d71-d7ba5aa00d20"
            }
        ]
    },
    "description": {
        "text": "made in §mogaml§rRUST §c§l(/) (°,,,,°) (/)"
    },
    "favicon": "data:image/png;base64,§§§",
    "enforcesSecureChat": true,
    "previewsChat": true
}
"#;
const PORT: u16 = 25565;
const ONLINE: bool = true;

#[tokio::main]
async fn main() {
    dotenv().ok();
    env_logger::init();
    info!("Starting up server on port {PORT}...");
    start_server().await;
}

struct Assets {
    key: Rsa<Private>,
    pub_key: Vec<u8>,
    online: bool,
    motd: String,
    registry: NbtCompound,
    world: RwLock<World>,
}
