use std::net::SocketAddr;
use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose;
use dotenv::dotenv;
use log::{debug, error, info};
use openssl::pkey::Private;
use openssl::rsa::Rsa;
use tokio::fs;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};

use crate::connection::{Connection, ConnectionState};
use crate::connection::ConnectionState::Handshake;
use crate::protocol_types::{ReadProt, ServerPacket, SizedProt, VarInt};

pub(crate) mod protocol_util;
mod connection;
mod protocol_types;
mod encryption;
mod server_packets;
mod client_packets;

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
const ONLINE: bool = false;

async fn skip(stream: &mut TcpStream, n: u64) -> Result<(), String> {
    // skip n bytes in the given stream
    let mut took = stream.take(n);
    let mut buf = Vec::with_capacity(n as usize);
    took.read_to_end(&mut buf).await.or_else(|err| Err(format!("{err}")))?;
    Ok(())
}

async fn parse_packet(stream: &mut TcpStream, connection: &Connection) -> Result<Box<dyn ServerPacket>, String> {
    let length = VarInt::read(stream).await?;
    let id = VarInt::read(stream).await?;

    let packet: Box<dyn ServerPacket> = match (id.value, &connection.state) {
        (0x00, Handshake) => Box::new(server_packets::Handshake::read(stream).await?),
        (0x00, ConnectionState::Status) => Box::new(server_packets::StatusReq::read(stream).await?),
        (0x00, ConnectionState::Login) => {
            let p = Box::new(server_packets::LoginStart::read(stream).await?);
            skip(stream, 16).await?;
            p
        },
        (0x01, ConnectionState::Status) => Box::new(server_packets::PingReq::read(stream).await?),
        (0x01, ConnectionState::Login) => Box::new(server_packets::EncryptionResponse::read(stream).await?),
        (0x03, ConnectionState::Login) => Box::new(server_packets::LoginAck::read(stream).await?),
        (0x00, ConnectionState::Configuration) => Box::new(server_packets::ClientInfo::read(stream).await?),
        _ => {
            // eat remainder of packet
            skip(stream, (length.value - id.prot_size() as i32) as u64).await?;
            return Err(format!("Unrecognized packet with id {:x} (current connection state: {:?}", id.value, connection.state));
        }    };

    Ok(packet)
}

async fn handle_connection(mut stream: TcpStream, assets: Arc<Assets>) {
    info!("New connection: {}", stream.peer_addr().unwrap().ip());
    let mut connection = Connection { state: Handshake, verify_token: vec!(0, 0, 0, 0), shared_secret: vec![], player: "".to_string() };
    loop {
        let alive = stream.peek(&mut [0]).await;
        match alive {
            Ok(0) => {
                info!("Connection {} closed.", stream.peer_addr().map(|some| some.ip().to_string()).unwrap_or("{unknown}".into()));
                break;
            }
            Err(e) => {
                error!("Error: {:?}", e);
                break;
            }
            _ => {}
        }

        let packet = parse_packet(&mut stream, &connection).await;
        match packet {
            Ok(p) => {
                debug!("Inbound packet: {p:?}");
                let res = p.handle(&mut stream, &mut connection, assets.clone()).await;
                match res {
                    Ok(_) => {}
                    Err(e) => error!("Couldn't handle packet {:?} {e}", p)
                }
            }
            Err(err) => error!("Couldn't parse packet: {err}")
        }
    }
}

struct Assets {
    icon: String,
    key: Rsa<Private>,
    pub_key: Vec<u8>,
    online: bool,
    motd: String,
}

async fn start_server() {
    let icon = fs::read("icon.png").await.unwrap();
    let rsa = Rsa::generate(1024).unwrap();
    let motd = String::from(MSG).replacen("§§§", &general_purpose::STANDARD.encode(icon.as_slice()), 1);

    let assets = Assets {
        icon: general_purpose::STANDARD.encode(icon.as_slice()),
        pub_key: rsa.public_key_to_der().unwrap(),
        key: rsa,
        online: ONLINE,
        motd,
    };
    let assets = Arc::new(assets);

    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], PORT))).await.unwrap();
    // For every incoming connection on the listener, we spawn a new task with a reference to the assets (possibly an arc or sth else), and the stream
    loop {
        let (stream, addr) = listener.accept().await.unwrap();
        let assets = assets.clone();
        tokio::spawn(async move {
            handle_connection(stream, assets).await;
        });
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    env_logger::init();
    info!("Starting up server on port {PORT}...");
    start_server().await;
}