use std::io::Cursor;
use std::sync::{Arc, RwLock};

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::net::tcp::OwnedReadHalf;

use crate::connection::{ConnectionInfo, ConnectionState};
use crate::packets::server;
use crate::protocol_types::primitives::VarInt;
use crate::protocol_types::traits::{ReadProt, ReadProtPacket, ServerPacket, SizedProt};
use crate::protocol_util::skip;

pub(crate) async fn parse_packet(
    stream: &mut OwnedReadHalf,
    connection: Arc<RwLock<ConnectionInfo>>,
) -> Result<Box<dyn ServerPacket>, String> {
    let (mut read_from, length): (Box<(dyn AsyncRead + Unpin + Send)>, Option<VarInt>) =
        if connection.read().unwrap().decrypter.is_some() {
            let length = VarInt::read_decrypt(stream, connection.clone()).await?;
            let mut packet = vec![0u8; length.value as usize];
            stream.read_exact(&mut packet).await.or_else(|err| {
                Err(format!(
                    "Trying to read encrypted packet with size {length}: {err}"
                ))
            })?;
            let mut decrypted = vec![0u8; length.value as usize];
            connection
                .write()
                .unwrap()
                .decrypter
                .as_mut()
                .unwrap()
                .update(&packet, &mut decrypted)
                .unwrap();
            (Box::new(Cursor::new(decrypted)), Some(length))
        } else {
            (Box::new(stream), None)
        };
    let length = if let Some(length) = length {
        length
    } else {
        VarInt::read(&mut read_from).await?
    };

    let id = VarInt::read(&mut read_from).await?;
    let state = connection.read().unwrap().state().clone();
    let packet: Box<dyn ServerPacket> = match (id.value, state) {
        (0x00, ConnectionState::Handshake) => {
            Box::new(server::Handshake::read(&mut read_from).await?)
        }

        (0x00, ConnectionState::Status) => Box::new(server::StatusReq::read(&mut read_from).await?),
        (0x01, ConnectionState::Status) => Box::new(server::PingReq::read(&mut read_from).await?),

        (0x00, ConnectionState::Login) => {
            let p = Box::new(server::LoginStart::read(&mut read_from).await?);
            skip(&mut read_from, 16).await?;
            p
        }
        (0x01, ConnectionState::Login) => {
            Box::new(server::EncryptionResponse::read(&mut read_from).await?)
        }
        (0x03, ConnectionState::Login) => Box::new(server::LoginAck::read(&mut read_from).await?),

        (0x00, ConnectionState::Configuration) => {
            Box::new(server::ClientInfo::read(&mut read_from).await?)
        }
        (0x02, ConnectionState::Configuration) => {
            Box::new(server::ConfigurationFinish::read(&mut read_from).await?)
        }
        (0x03, ConnectionState::Configuration) => {
            Box::new(server::ConfigurationKeepAlive::read(&mut read_from).await?)
        }

        (0x14, ConnectionState::Play) => {
            Box::new(server::PlayKeepAlive::read(&mut read_from).await?)
        }
        (0x06, ConnectionState::Play) => {
            Box::new(server::PlayerSession::read(&mut read_from).await?)
        }
        (0x16, ConnectionState::Play) => {
            Box::new(server::SetPlayerPosition::read(&mut read_from).await?)
        }
        (0x17, ConnectionState::Play) => {
            Box::new(server::SetPlayerPositionAndRotation::read(&mut read_from).await?)
        }
        (0x18, ConnectionState::Play) => {
            Box::new(server::SetPlayerRotation::read(&mut read_from).await?)
        }
        (0x21, ConnectionState::Play) => {
            Box::new(server::PlayerCommand::read(&mut read_from).await?)
        }
        (0x00, ConnectionState::Play) => {
            Box::new(server::ConfirmTeleportation::read(&mut read_from).await?)
        }
        _ => {
            // eat remainder of packet
            skip(
                &mut read_from,
                (length.value - id.prot_size() as i32) as u64,
            )
            .await?;
            return Err(format!(
                "Unrecognized packet with id 0x{:x} (current connection state: {:?})",
                id.value,
                connection.read().unwrap().state()
            ));
        }
    };

    Ok(packet)
}
