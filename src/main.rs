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
use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::Mutex;

use crate::connection::{Connection, ConnectionState};
use crate::connection::ConnectionState::Handshake;
use crate::protocol_types::{ReadProt, ServerPacket, SizedProt, VarInt, WriteProt};

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

async fn skip(stream: &mut OwnedReadHalf, n: u64) -> Result<(), String> {
    // skip n bytes in the given stream
    let mut took = stream.take(n);
    let mut buf = Vec::with_capacity(n as usize);
    took.read_to_end(&mut buf).await.or_else(|err| Err(format!("{err}")))?;
    Ok(())
}

async fn parse_packet(stream: &mut OwnedReadHalf, connection: &Connection) -> Result<Box<dyn ServerPacket>, String> {
    let length = VarInt::read(stream).await?;
    let id = VarInt::read(stream).await?;

    let packet: Box<dyn ServerPacket> = match (id.value, &connection.state()) {
        (0x00, Handshake) => Box::new(server_packets::Handshake::read(stream).await?),

        (0x00, ConnectionState::Status) => Box::new(server_packets::StatusReq::read(stream).await?),
        (0x01, ConnectionState::Status) => Box::new(server_packets::PingReq::read(stream).await?),

        (0x00, ConnectionState::Login) => {
            let p = Box::new(server_packets::LoginStart::read(stream).await?);
            skip(stream, 16).await?;
            p
        },
        (0x01, ConnectionState::Login) => Box::new(server_packets::EncryptionResponse::read(stream).await?),
        (0x03, ConnectionState::Login) => Box::new(server_packets::LoginAck::read(stream).await?),

        (0x00, ConnectionState::Configuration) => Box::new(server_packets::ClientInfo::read(stream).await?),
        (0x02, ConnectionState::Configuration) => Box::new(server_packets::ConfigurationFinish::read(stream).await?),
        (0x03, ConnectionState::Configuration) => Box::new(server_packets::ConfigurationKeepAlive::read(stream).await?),
        _ => {
            // eat remainder of packet
            skip(stream, (length.value - id.prot_size() as i32) as u64).await?;
            return Err(format!("Unrecognized packet with id {:x} (current connection state: {:?}", id.value, connection.state()));
        }    };

    Ok(packet)
}

async fn handle_connection(stream: TcpStream, assets: Arc<Assets>) {
    info!("New connection: {}", stream.peer_addr().unwrap().ip());
    let (read_stream, write_stream) = stream.into_split();
    let read_stream = Arc::new(Mutex::new(read_stream));
    let write_stream = Arc::new(Mutex::new(write_stream));
    let connection = Connection::new();
    let connection = Arc::new(Mutex::new(connection));

    // scheduler for keepalive packets
    let heartbeat_stream = write_stream.clone();
    let heartbeat_connection = connection.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            let mut connection = heartbeat_connection.lock().await;
            if connection.closed {
                break;
            }
            match connection.state() {
                ConnectionState::Configuration => {
                    // set connection.keep_alive_id to a random number
                    connection.keep_alive_id = rand::random::<i64>();

                    let packet = client_packets::ConfigurationKeepAlive::new(*&connection.keep_alive_id);
                    let mut stream = heartbeat_stream.lock().await;
                    packet.write(&mut *stream).await.unwrap();

                }
                _ => break
            }
        }
    });
    let main_stream_read = read_stream.clone();
    let main_stream_write = write_stream.clone();
    let main_connection = connection.clone();
    loop {
        let read_stream = &mut *main_stream_read.lock().await;
        let alive = read_stream.peek(&mut [0]).await;
        let mut connection = main_connection.lock().await;
        match alive {
            Ok(0) => {
                info!("Connection {} closed.", read_stream.peer_addr().map(|some| some.ip().to_string()).unwrap_or("{unknown}".into()));
                connection.closed = true;
                break;
            }
            Err(e) => {
                error!("Error: {:?}", e);
                connection.closed = true;
                break;
            }
            _ => {}
        }
        let packet = parse_packet(read_stream, &connection).await;
        match packet {
            Ok(p) => {
                debug!("Inbound packet: {p:?}");
                let res = {
                    let write_stream = &mut *main_stream_write.lock().await;
                    p.handle(write_stream, &mut connection, assets.clone()).await
                };
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
        let (stream, _addr) = listener.accept().await.unwrap();
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