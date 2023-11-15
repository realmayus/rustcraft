use std::fmt::{Debug, Formatter};
use async_std::io;
use async_std::io::{Read, ReadExt, Write};
use async_std::net::TcpStream;
use async_trait::async_trait;
use futures::AsyncRead;
use crate::Assets;
use crate::connection::{Connection, ConnectionState};
use crate::protocol_types::{ReadProt, SizedProt, WriteProt};

const REPLY: &str = r#"
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


#[derive(Debug)]
pub(crate) enum PacketData {
    Handshake(Handshake),
    StatusReq,
    StatusRes(String),
    PingReq,
    PingRes,
}

impl SizedProt for PacketData {
    fn size(&self) -> usize {
        match self {
            PacketData::Handshake(h) => h.next_state.size() + 2 + (h.server_address.len() as i32).size() + h.server_address.len() + h.prot_version.size(),
            PacketData::StatusReq => 0,
            PacketData::StatusRes(s) => {
                println!("s.len {}, s.len.size {}", s.len(), (s.len() as i32).size());
                (s.len() as i32).size() + s.len() },
            PacketData::PingReq => 0,
            PacketData::PingRes => 0,
        }
    }
}

#[async_trait]
impl WriteProt for PacketData {
    async fn write(&self, stream: &mut (impl Write + Unpin + Send)) -> Result<(), String> {
        match self {
            PacketData::StatusRes(s) => {
                s.write(stream).await?;
            }
            _ => panic!("Can't write {:?}", self)
        }
        Ok(())
    }
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
            (0, ConnectionState::Handshake) => PacketData::Handshake(Handshake::read(stream).await?),
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

    pub(crate) async fn handle(&self, stream: &mut TcpStream, connection: &mut Connection, assets: &Assets) {
        match &self.data {
            PacketData::Handshake(handshake) => handshake.handle(stream, connection).await,
            PacketData::StatusReq => {
                println!("Sending JSON reply to StatusReq");
                println!("{}", String::from(REPLY).replacen("§§§", assets.icon.as_ref(), 1));
                let res = Packet::new(0x00, PacketData::StatusRes(String::from(REPLY).replacen("§§§", assets.icon.as_ref(), 1)));
                res.write(stream).await.unwrap();
            }
            PacketData::StatusRes(_) => {}
            PacketData::PingReq => {}
            PacketData::PingRes => {}
        }
    }

    pub(crate) fn new(id: i32, data: PacketData) -> Packet {
        println!("Packet has size {}", data.size());
        Packet {
            size: data.size() as i32 + id.size() as i32,
            id,
            data,
        }
    }
}

#[async_trait]
impl WriteProt for Packet {
    async fn write(&self, stream: &mut (impl Write + Unpin + Send)) -> Result<(), String> {
        self.size.write(stream).await?;
        self.id.write(stream).await?;
        self.data.write(stream).await?;
        Ok(())
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
    pub(crate) async fn handle(&self, stream: &mut TcpStream, connection: &mut Connection) {
        connection.state = ConnectionState::Status;
    }
}

#[async_trait]
impl ReadProt for Handshake {
    async fn read(stream: &mut (impl Read + Unpin + Send)) -> Result<Self, String> where Self: Sized {
        Ok(Handshake {
            prot_version: i32::read(stream).await?,
            server_address: String::read(stream).await?,
            server_port: u16::read(stream).await?,
            next_state: i32::read(stream).await?,
        })
    }
}