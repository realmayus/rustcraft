use std::fmt::{Debug, Formatter};
use async_std::io;
use async_std::io::ReadExt;
use async_std::net::TcpStream;
use crate::connection::{Connection, ConnectionState};
use crate::protocol_types::{ReadProt, SizedProt, WriteProt};

const REPLY: &str = r#"
{
    "version": {
        "name": "1.19.4",
        "protocol": 762
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
        "text": "Hello world"
    },
    "favicon": "data:image/png;base64,<data>",
    "enforcesSecureChat": true,
    "previewsChat": true
}
"#;


#[derive(Debug)]
pub(crate) enum PacketData {
    Handshake(Handshake),
    StatusReq,
    StatusRes(String),
    PingReq,
    PingRes,
}

pub(crate) struct Packet {
    size: i32,
    id: i32,
    data: PacketData
}

impl Packet {
    pub(crate) async fn parse(stream: &mut TcpStream, connection: &Connection) -> Result<Packet, String> {
        let length = i32::read(stream).await.unwrap();
        let id = i32::read(stream).await.unwrap();
        let data = match (id, &connection.state) {
            (0, ConnectionState::Handshake) => PacketData::Handshake(Handshake::parse(stream).await?),
            (0, ConnectionState::Status) => PacketData::StatusReq,
            _ => {
                // eat remainder of packet
                io::copy(&mut stream.take((length - id.size() as i32) as u64), &mut io::sink()).await.unwrap();
                return Err(format!("Unrecognized packet with id {:x} (current connection state: {:?}", id, connection.state));
            }
        };

        Ok(Packet {
            size: length,
            id,
            data,
        })
    }

    pub(crate) async fn handle(&self, stream: &mut TcpStream, connection: &mut Connection) {
        match &self.data {
            PacketData::Handshake(handshake) => handshake.handle(stream, connection).await,
            PacketData::StatusReq => {
                String::from(REPLY).write(stream).await.unwrap()
            }
            PacketData::StatusRes(_) => {}
            PacketData::PingReq => {}
            PacketData::PingRes => {}
        }
    }
}

impl Debug for Packet {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Packet: size={} id={} data={:?}", self.size, self.id, self.data)
    }
}

#[derive(Debug)]
pub(crate) struct Handshake {
    prot_version: i32,
    server_address: String,
    server_port: u16,
    next_state: i32,
}

impl Handshake {
    async fn parse(stream: &mut TcpStream) -> Result<Handshake, String> {
        Ok(Handshake {
            prot_version: i32::read(stream).await?,
            server_address: String::read(stream).await?,
            server_port: u16::read(stream).await?,
            next_state: i32::read(stream).await?,
        })
    }

    pub(crate) async fn handle(&self, stream: &mut TcpStream, connection: &mut Connection) {
        connection.state = ConnectionState::Status;
    }
}