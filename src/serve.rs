use crate::connection::{Connection, ConnectionState};
use crate::nbt::load_registry;
use crate::packets::client::ClientPackets;
use crate::packets::{client, parse};
use crate::protocol_types::traits::WriteProtPacket;
use crate::{Assets, MSG, ONLINE, PORT};
use base64::engine::general_purpose;
use base64::Engine;
use log::{debug, error, info};
use openssl::rsa::Rsa;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};

async fn handle_connection(mut stream: TcpStream, assets: Arc<Assets>) {
    info!("New connection: {}", stream.peer_addr().unwrap().ip());
    let (tx, mut rx) = mpsc::channel::<ClientPackets>(32); // write queue for packets
    let connection = Connection::new();
    let connection = Arc::new(Mutex::new(connection));

    // scheduler for keepalive packets
    let heartbeat_connection = connection.clone();
    let heartbeat_tx = tx.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            let mut connection = heartbeat_connection.lock().await;
            if connection.closed {
                break;
            }
            // set connection.keep_alive_id to a random number
            connection.keep_alive_id = rand::random::<i64>();

            match connection.state() {
                ConnectionState::Configuration => {
                    let packet = client::ConfigurationKeepAlive::new(*&connection.keep_alive_id);
                    heartbeat_tx
                        .send(ClientPackets::ConfigurationKeepAlive(packet))
                        .await
                        .unwrap();
                }
                ConnectionState::Play => {
                    let packet = client::PlayKeepAlive::new(*&connection.keep_alive_id);
                    heartbeat_tx
                        .send(ClientPackets::PlayKeepAlive(packet))
                        .await
                        .unwrap();
                }
                _ => break,
            }
        }
    });
    let main_connection = connection.clone();
    loop {
        let alive = stream.peek(&mut [0]).await;
        let mut connection = main_connection.lock().await;
        match alive {
            Ok(0) => {
                info!(
                    "Connection {} closed.",
                    stream
                        .peer_addr()
                        .map(|some| some.ip().to_string())
                        .unwrap_or("{unknown}".into())
                );
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
        let packet = parse::parse_packet(&mut stream, &mut connection).await;
        match packet {
            Ok(p) => {
                debug!("Inbound packet: {p:?}");
                let res = p.handle(&mut stream, &mut connection, assets.clone()).await;
                if let Ok(ps) = res {
                    for packet in ps {
                        tx.send(packet).await.unwrap();
                    }
                } else if let Err(e) = res {
                    error!("Couldn't handle packet {:?} {e}", p);
                    if e.is_fatal() {
                        connection.closed = true;
                        break;
                    }
                }
            }
            Err(err) => error!("Couldn't parse packet: {err}"),
        }
        while let Ok(to_send) = rx.try_recv() {
            to_send.write(&mut stream, &mut connection).await.unwrap();
        }
    }
    stream.shutdown().await.unwrap();
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
    };
    let assets = Arc::new(assets);

    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], PORT)))
        .await
        .unwrap();
    // For every incoming connection on the listener, we spawn a new task with a reference to the assets (possibly an arc or sth else), and the stream
    loop {
        let (stream, _addr) = listener.accept().await.unwrap();
        let assets = assets.clone();
        tokio::spawn(async move {
            handle_connection(stream, assets).await;
        });
    }
}
