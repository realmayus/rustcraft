use std::io::Cursor;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::net::TcpStream;
use crate::connection::{Connection, ConnectionState};
use crate::protocol_types::primitives::VarInt;
use crate::protocol_types::traits::{ServerPacket, ReadProt, ReadProtPacket, SizedProt};
use crate::protocol_util::skip;
use crate::packets::server;

pub(crate) async fn parse_packet(
    stream: &mut TcpStream,
    connection: &mut Connection,
) -> Result<Box<dyn ServerPacket>, String> {
    let (mut read_from, length): (Box<(dyn AsyncRead + Unpin + Send)>, Option<VarInt>) =
        if let Some(crypter) = connection.decrypter.as_mut() {
            let length = VarInt::read_decrypt(stream, crypter).await?;
            let mut packet = vec![0u8; length.value as usize];
            stream.read_exact(&mut packet).await.or_else(|err| {
                Err(format!(
                    "Trying to read encrypted packet with size {length}: {err}"
                ))
            })?;
            let mut decrypted = vec![0u8; length.value as usize];
            crypter.update(&packet, &mut decrypted).unwrap();
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

    let packet: Box<dyn ServerPacket> = match (id.value, &connection.state()) {
        (0x00, ConnectionState::Handshake) => {
            Box::new(server::Handshake::read(&mut read_from, connection).await?)
        }

        (0x00, ConnectionState::Status) => {
            Box::new(server::StatusReq::read(&mut read_from, connection).await?)
        }
        (0x01, ConnectionState::Status) => {
            Box::new(server::PingReq::read(&mut read_from, connection).await?)
        }

        (0x00, ConnectionState::Login) => {
            let p = Box::new(server::LoginStart::read(&mut read_from, connection).await?);
            skip(&mut read_from, 16).await?;
            p
        }
        (0x01, ConnectionState::Login) => {
            Box::new(server::EncryptionResponse::read(&mut read_from, connection).await?)
        }
        (0x03, ConnectionState::Login) => {
            Box::new(server::LoginAck::read(&mut read_from, connection).await?)
        }

        (0x00, ConnectionState::Configuration) => {
            Box::new(server::ClientInfo::read(&mut read_from, connection).await?)
        }
        (0x02, ConnectionState::Configuration) => {
            Box::new(server::ConfigurationFinish::read(&mut read_from, connection).await?)
        }
        (0x03, ConnectionState::Configuration) => Box::new(
            server::ConfigurationKeepAlive::read(&mut read_from, connection).await?,
        ),

        (0x14, ConnectionState::Play) => {
            Box::new(server::PlayKeepAlive::read(&mut read_from, connection).await?)
        }
        (0x06, ConnectionState::Play) => {
            Box::new(server::PlayerSession::read(&mut read_from, connection).await?)
        }
        (0x17, ConnectionState::Play) => Box::new(
            server::SetPlayerPositionAndRotation::read(&mut read_from, connection).await?,
        ),
        (0x16, ConnectionState::Play) => {
            Box::new(server::SetPlayerPosition::read(&mut read_from, connection).await?)
        }
        (0x00, ConnectionState::Play) => {
            Box::new(server::ConfirmTeleportation::read(&mut read_from, connection).await?)
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
                connection.state()
            ));
        }
    };

    Ok(packet)
}
