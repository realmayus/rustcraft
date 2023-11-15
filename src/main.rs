use std::net::SocketAddr;

use async_std::io::{ReadExt, WriteExt};
use async_std::net::{TcpListener, TcpStream};
use async_std::task;
use base64::Engine;
use base64::engine::general_purpose;
use futures::StreamExt;

use crate::connection::Connection;
use crate::connection::ConnectionState::Handshake;
use crate::packets::Packet;
use crate::protocol_types::{ReadProt, SizedProt, WriteProt};

pub(crate) mod protocol_util;
mod connection;
mod protocol_types;
mod packets;

const PORT: u16 = 25565;


async fn handle_connection(mut stream: TcpStream, assets: &Assets) {
    println!("New connection: {}", stream.peer_addr().unwrap().ip());
    let mut connection = Connection { state: Handshake };
    loop {

        let alive = stream.peek(&mut [0]).await;
        match alive {
            Ok(0) => {
                println!("Connection {} closed.", stream.peer_addr().map(|some| some.ip().to_string()).unwrap_or("{unknown}".into()));
                break;
            }
            Err(e) => {
                println!("Error: {:?}", e);
                break;
            }
            _ => {}
        }

        let packet = Packet::parse(&mut stream, &connection).await;
        match packet {
            Ok(p) => {
                println!("{p:?}");
                let res = p.handle(&mut stream, &mut connection, assets).await;
                match res {
                    Ok(_) => {}
                    Err(e) => eprintln!("Couldn't handle packet {e}")
                }
            }
            Err(err) => println!("Couldn't parse packet: {err}")
        }
    }
}

struct Assets {
    icon: String
}

async fn start_server() {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], PORT))).await.unwrap();
    let icon = async_std::fs::read("icon.png").await.unwrap();
    let assets = Assets {
        icon: general_purpose::STANDARD.encode(icon.as_slice()),
    };
    listener
        .incoming()
        .for_each_concurrent(/* limit */ None, |tcpstream| async {
            let tcpstream = tcpstream.unwrap();
            handle_connection(tcpstream, &assets).await;
        })
        .await;
}

fn main() {
    task::block_on(start_server());
}