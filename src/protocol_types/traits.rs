use crate::connection::Connection;
use crate::err::ProtError;
use crate::packets::client::ClientPackets;
use crate::Assets;
use async_trait::async_trait;
use std::fmt::{Debug, Display};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;

#[async_trait]
pub(crate) trait ServerPacket:
    SizedProt + ReadProtPacket + Debug + Display + Sync + Send
{
    fn id() -> u8
    where
        Self: Sized;

    async fn handle(
        &self,
        stream: &mut TcpStream,
        connection: &mut Connection,
        assets: Arc<Assets>,
    ) -> Result<Vec<ClientPackets>, ProtError>;
}

pub(crate) trait ClientPacket: SizedProt + WriteProtPacket + Debug + Display {
    fn id() -> u8
    where
        Self: Sized;
}

#[async_trait]
pub(crate) trait ReadProt {
    async fn read(stream: &mut (impl AsyncRead + Unpin + Send)) -> Result<Self, String>
    where
        Self: Sized;
}

#[async_trait]
pub(crate) trait ReadProtPacket {
    async fn read(
        stream: &mut (impl AsyncRead + Unpin + Send),
        connection: &mut Connection,
    ) -> Result<Self, String>
    where
        Self: Sized;
}

#[async_trait]
pub(crate) trait WriteProt {
    async fn write(&self, stream: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), String>;
}

#[async_trait]
pub(crate) trait WriteProtPacket {
    async fn write(
        &self,
        stream: &mut (impl AsyncWrite + Unpin + Send),
        connection: &mut Connection,
    ) -> Result<(), String>;
}

pub(crate) trait SizedProt {
    fn prot_size(&self) -> usize;
}
