use std::collections::HashMap;
use std::net::SocketAddr;

use async_std::io::{ReadExt, WriteExt};
use async_std::net::{TcpListener, TcpStream};
use async_std::task;
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


async fn handle_connection(mut stream: TcpStream) {
    println!("New connection: {}", stream.peer_addr().unwrap().ip());
    let mut connection = Connection { state: Handshake };
    loop {

        let alive = stream.peek(&mut [0]).await;
        match alive {
            Ok(0) => {
                println!("Connection {} closed.", stream.peer_addr().unwrap().ip());
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
                println!("Packet: {p:?}");
                p.handle(&mut stream, &mut connection).await
            }
            Err(err) => println!("Couldn't parse packet: {err}")
        }
    }
}

async fn start_server() {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], PORT))).await.unwrap();

    listener
        .incoming()
        .for_each_concurrent(/* limit */ None, |tcpstream| async move {
            let tcpstream = tcpstream.unwrap();
            handle_connection(tcpstream).await;
        })
        .await;
}

fn main() {
    task::block_on(start_server());
}