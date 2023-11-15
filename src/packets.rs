use std::fmt::{Debug, Formatter};

use async_std::io;
use async_std::io::{Read, ReadExt, Write};
use async_std::net::TcpStream;
use async_trait::async_trait;

use crate::Assets;
use crate::connection::{Connection, ConnectionState};
use crate::protocol_types::{ReadProt, SizedProt, VarInt, WriteProt};

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
    PingReq(i64),
    PingRes(i64),
}

impl SizedProt for PacketData {
    fn size(&self) -> usize {
        match self {
            PacketData::Handshake(h) => h.next_state.size() + 2 + (VarInt::from(h.server_address.len())).size() + h.server_address.len() + h.prot_version.size(),
            PacketData::StatusReq => 0,
            PacketData::StatusRes(s) => {
                VarInt::from(s.len()).size() + s.len() },
            PacketData::PingReq(i) => i.size(),
            PacketData::PingRes(i) => i.size(),
        }
    }
}

#[async_trait]
impl WriteProt for PacketData {
    async fn write(&self, stream: &mut (impl Write + Unpin + Send)) -> Result<(), String> {
        match self {
            PacketData::StatusRes(s) => {
                s.write(stream).await?;
            },
            PacketData::PingRes(i) => {
                i.write(stream).await?;
            }
            _ => panic!("Can't write {:?}", self)
        }
        Ok(())
    }
}

pub(crate) struct Packet {
    size: VarInt,
    id: VarInt,
    data: PacketData
}

impl Packet {
    pub(crate) async fn parse(stream: &mut TcpStream, connection: &Connection) -> Result<Packet, String> {
        let length = VarInt::read(stream).await?;
        let id = VarInt::read(stream).await?;
        let data = match (id.value, &connection.state) {
            (0x00, ConnectionState::Handshake) => PacketData::Handshake(Handshake::read(stream).await?),
            (0x00, ConnectionState::Status) => PacketData::StatusReq,
            (0x01, _) => PacketData::PingReq(i64::read(stream).await?),
            _ => {
                // eat remainder of packet
                io::copy(&mut stream.take((length.value - id.size() as i32) as u64), &mut io::sink()).await.or_else(|err| Err(format!("{err:?}")))?;
                return Err(format!("Unrecognized packet with id {:x} (current connection state: {:?}", id.value, connection.state));
            }
        };

        Ok(Packet {
            size: length,
            id,
            data,
        })
    }

    pub(crate) async fn handle(&self, stream: &mut TcpStream, connection: &mut Connection, assets: &Assets) -> Result<(), String> {
        match &self.data {
            PacketData::Handshake(handshake) => handshake.handle(stream, connection).await,
            PacketData::StatusReq => {
                let res = Packet::new(0x00.into(), PacketData::StatusRes(String::from(REPLY).replacen("§§§", assets.icon.as_ref(), 1)));
                res.write(stream).await.or_else(|err| return Err(format!("{err}")))
            }
            PacketData::PingReq(i) => {
                let res = Packet::new(0x01.into(), PacketData::PingRes(*i));
                println!("Replying with {res:?}");
                res.write(stream).await.or_else(|err| Err(format!("{err}")))
            }
            PacketData::StatusRes(_) => Err(String::from("Can't handle serverbound packet")),
            PacketData::PingRes(_) => Err(String::from("Can't handle serverbound packet")),
        }
    }

    pub(crate) fn new(id: VarInt, data: PacketData) -> Packet {
        Packet {
            size: (data.size() as i32 + id.size() as i32).into(),
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
    prot_version: VarInt,
    server_address: String,
    server_port: u16,
    next_state: VarInt,
}

impl Handshake {
    pub(crate) async fn handle(&self, _stream: &mut TcpStream, connection: &mut Connection) -> Result<(), String> {
        connection.state = ConnectionState::Status;
        Ok(())
    }
}

#[async_trait]
impl ReadProt for Handshake {
    async fn read(stream: &mut (impl Read + Unpin + Send)) -> Result<Self, String> where Self: Sized {
        Ok(Handshake {
            prot_version: VarInt::read(stream).await?,
            server_address: String::read(stream).await?,
            server_port: u16::read(stream).await?,
            next_state: VarInt::read(stream).await?,
        })
    }
}