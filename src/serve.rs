use std::env;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use base64::Engine;
use base64::engine::general_purpose;
use dotenv::dotenv;
use log::{debug, error, info};
use openssl::rsa::Rsa;
use tokio::fs;
use tokio::net::{TcpListener, TcpStream};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::{mpsc, oneshot};
use tokio::sync::mpsc::{Receiver, Sender};
use rustcraft_lib::web::dto;

use crate::{Assets, MSG, ONLINE, PORT, web};
use crate::chunk::world::World;
use crate::connection::{ConnectionInfo, ConnectionState};
use crate::data::registry::load_registry;
use crate::err::ProtError;
use crate::packets::{client, parse};
use crate::packets::client::ClientPackets;
use crate::protocol_types::traits::WriteProtPacket;
use crate::serve::ConnectionActorMessage::{PlayerInfo, SendPacket};


async fn accept_packet(
    read: &mut OwnedReadHalf,
    connection: Arc<RwLock<ConnectionInfo>>,
    assets: Arc<Assets>,
    sender: Sender<ConnectionActorMessage>,
) -> Result<(), ProtError> {
    let result = {
        let packet = parse::parse_packet(read, connection.clone()).await;
        match packet {
            Ok(p) => {
                if env::var("LOG_PACKETS").is_ok_and(|s| s == "true") {
                    debug!("{} Inbound packet: {p:?}", read.peer_addr().unwrap());
                }
                let res = p.handle(connection.clone(), assets.clone()).await;
                res
            }
            Err(err) => {
                error!("Couldn't parse packet: {err}");
                return Err(ProtError::Any(err));
            }
        }
    };

    if let Ok(ps) = result {
        for packet in ps {
            sender.send(SendPacket(packet)).await.unwrap();
        }
    } else if let Err(e) = result {
        error!("Couldn't handle packet {e}");
        return Err(e);
    }
    Ok(())
}

/**
 * The connection actor is responsible for handling all packets for a single connection.
 * It is spawned for every new connection and runs in its own task.
 * The connection actor has three tasks:
 * - The message handler, which handles messages sent over the internal channel. It exclusively manages the WriteHalf of the TcpStream.
 * - The packet handler, which reads packets from the TCP stream, parses them, and handles them.
 * - The heartbeat handler, which sends keepalive packets over the internal channel.
*/
struct ConnectionActor {
    receiver: Receiver<ConnectionActorMessage>,
    connection: Arc<RwLock<ConnectionInfo>>,
}

impl ConnectionActor {
    fn new(receiver: Receiver<ConnectionActorMessage>) -> Self {
        Self {
            receiver,
            connection: Arc::new(RwLock::new(ConnectionInfo::new())),
        }
    }

    async fn run(
        &mut self,
        read: OwnedReadHalf,
        write: OwnedWriteHalf,
        sender: Sender<ConnectionActorMessage>,
        assets: Arc<Assets>,
    ) {
        let connection = self.connection.clone();
        let assets = assets.clone();
        let sender_clone = sender.clone();
        tokio::spawn(
            async move { run_packet_handler(connection, read, sender_clone, assets).await },
        );
        let connection = self.connection.clone();
        tokio::spawn(async move { run_heartbeat(connection, sender).await });
        self.run_msg_handler(write).await;
    }

    /**
     * Runs the internal channel message handler.
     * Primarily, messages sent over this channel tell the handler task to send packets over the TCP connection.
     * Another use case is the web interface requesting player information from the connection object.
     */
    async fn run_msg_handler(&mut self, mut write: OwnedWriteHalf) {
        while let Some(msg) = self.receiver.recv().await {
            let result = self.handle(msg, &mut write).await;
            if let Err(e) = result {
                if e.is_fatal() {
                    self.connection.write().unwrap().close();
                    break;
                }
            }
        }
    }

    async fn handle(
        &mut self,
        msg: ConnectionActorMessage,
        write: &mut OwnedWriteHalf,
    ) -> Result<(), ProtError> {
        match msg {
            SendPacket(packet) => {
                packet
                    .write(write, self.connection.clone())
                    .await
                    .or_else(|err| Err(ProtError::Any(err)))?;
            },
            PlayerInfo(sender) => {
                let player = {
                    let connection = self.connection.read().unwrap();
                    dto::Player {
                        username: connection.username.clone(),
                        uuid: connection.uuid.to_string(),
                        position: dto::Position {
                            x: connection.position.x,
                            y: connection.position.y,
                            z: connection.position.z,
                            pitch: connection.position.pitch,
                            yaw: connection.position.yaw,
                            on_ground: connection.position.on_ground,
                        },
                    }
                };
                sender.send(player).unwrap();
            }
        }
        Ok(())
    }
}

/**
 * Runs the minecraft packet handler for the connection actor.
 * Packets are read from the TCP stream, parsed, and handled. In case handling involves response packets,
 * these are sent over the message channel for the message handler to send them over the TCP stream.
 */
async fn run_packet_handler(
    connection: Arc<RwLock<ConnectionInfo>>,
    mut read: OwnedReadHalf,
    sender: Sender<ConnectionActorMessage>,
    assets: Arc<Assets>,
) {
    let address = read.peer_addr().unwrap();
    loop {
        let connection = connection.clone();
        {
            if connection.read().unwrap().closed() {
                break;
            }
            let alive = read.peek(&mut [0]).await;
            match alive {
                Ok(0) => {
                    info!("Connection {:?} closed.", address);
                    connection.write().unwrap().close();
                    break;
                }
                Err(e) => {
                    error!("Error: {:?}", e);
                    connection.write().unwrap().close();
                    break;
                }
                _ => {}
            }
        }
        let result = accept_packet(
            &mut read,
            connection.clone(),
            assets.clone(),
            sender.clone(),
        )
        .await;

        if let Err(e) = result {
            if e.is_fatal() {
                connection.write().unwrap().close();
                break;
            }
        }
    }
}

/**
 * Runs the heartbeat handler for the connection actor.
 * This sends keepalive packets over the message channel to the message handler, which then sends them over the TCP stream.
 */
async fn run_heartbeat(
    connection: Arc<RwLock<ConnectionInfo>>,
    sender: Sender<ConnectionActorMessage>,
) {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        if connection.read().unwrap().closed() {
            break;
        }
        let keep_alive_id = rand::random::<i64>();
        let state = {
            let mut connection = connection.write();
            let connection = connection.as_mut().unwrap();
            connection.keep_alive_id = keep_alive_id;
            connection.state().clone()
        };
        match state {
            ConnectionState::Configuration => {
                let packet = client::ConfigurationKeepAlive::new(keep_alive_id);
                sender
                    .send(SendPacket(ClientPackets::ConfigurationKeepAlive(packet)))
                    .await
                    .unwrap();
            }
            ConnectionState::Play => {
                let packet = client::PlayKeepAlive::new(keep_alive_id);
                sender
                    .send(SendPacket(ClientPackets::PlayKeepAlive(packet)))
                    .await
                    .unwrap();
            }
            _ => break,
        }
    }
}

#[derive(Clone)]
pub(crate) struct ConnectionActorHandle {
    sender: Sender<ConnectionActorMessage>,
}

impl ConnectionActorHandle {
    pub fn new(stream: TcpStream, assets: Arc<Assets>) -> Self {
        let (sender, receiver) = mpsc::channel(8);
        let (read, write) = stream.into_split();
        let mut actor = ConnectionActor::new(receiver);
        let sender_clone = sender.clone();
        tokio::spawn(async move {
            actor.run(read, write, sender_clone, assets).await;
        });

        Self { sender }
    }

    pub async fn send(&self, msg: ConnectionActorMessage) {
        self.sender.send(msg).await.unwrap();
    }
}

pub(crate) enum ConnectionActorMessage {
    SendPacket(ClientPackets),
    PlayerInfo(oneshot::Sender<dto::Player>)
}

pub(crate) async fn start_server() {
    let icon = fs::read("icon.png").await.unwrap();
    let rsa = Rsa::generate(1024).unwrap();
    let motd =
        String::from(MSG).replacen("§§§", &general_purpose::STANDARD.encode(icon.as_slice()), 1);
    let registry = load_registry().await.unwrap();

    let assets = Assets {
        pub_key: rsa.public_key_to_der().unwrap(),
        key: rsa,
        online: ONLINE,
        motd,
        registry,
        world: RwLock::new(World::new_grass()),
    };
    let assets = Arc::new(assets);


    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], PORT)))
        .await
        .unwrap();

    // We need an async RwLock here due to axum's state management
    let connection_handles: Arc<tokio::sync::RwLock<Vec<ConnectionActorHandle>>> = Arc::new(tokio::sync::RwLock::new(vec![]));
    let connection_handles_clone = connection_handles.clone();
    tokio::spawn(async move {
        web::serve::init(connection_handles_clone).await;
    });

    // For every incoming connection on the listener, we spawn a new task with a reference to the assets (possibly an arc or sth else), and the stream
    loop {
        let (stream, addr) = listener.accept().await.unwrap();
        let assets = assets.clone();
        let handle = ConnectionActorHandle::new(stream, assets);
        connection_handles.write().await.push(handle);
    }
}
