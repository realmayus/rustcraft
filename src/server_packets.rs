use core::fmt::Debug;
use core::fmt::Display;
use std::sync::Arc;

use async_trait::async_trait;
use log::debug;
use openssl::rsa::Padding;
use tokio::io::AsyncRead;
use tokio::net::TcpStream;
use log::info;
use crate::Assets;
use crate::client_packets::StatusRes;
use crate::connection::Connection;
use crate::connection::ConnectionState;
use crate::encryption::encrypt;
use crate::packet;
use crate::packet_base;
use crate::protocol_types::{ReadProt, ServerPacket, SizedProt};
use crate::protocol_types::VarInt;
use crate::protocol_types::WriteProt;
use crate::client_packets::*;
use uuid::uuid;
use tokio::net::tcp::OwnedWriteHalf;

packet!(
    Handshake 0x00 {
        prot_version: VarInt,
        server_address: String,
        server_port: u16,
        next_state: VarInt,
    },
    handler |this, stream, connection, _assets| {
        connection.state = if this.next_state.value == 1 {
            ConnectionState::Status
        } else if this.next_state.value == 2 {
            ConnectionState::Login
        } else {
            return Err(format!("Invalid next_state {}", this.next_state))
        };
        debug!("Connection state is now {:?}", connection.state);
        Ok(())
    }
);

packet!(
    StatusReq 0x00 {},
    handler |_this, stream, _connection, assets| {
        let res = StatusRes::new(assets.motd.clone());
        return res.write(stream).await.or_else(|err| Err(format!("{err}")))
    }
);

packet!(
    PingReq 0x01 {
        payload: i64,
    },
    handler |this, stream, connection, assets| {
        let res = PingRes::new(this.payload);
        res.write(stream).await.or_else(|err| Err(format!("{err}")))
    }
);

packet!(
    LoginStart 0x00 {
        name: String,
    },
    handler |this, stream, connection, assets| {
        info!("Player {} wants to login...", this.name);
        connection.player = this.name.clone();
        connection.verify_token = vec!(rand::random::<u8>(), rand::random::<u8>(), rand::random::<u8>(), rand::random::<u8>());
        if assets.online {
            let res = EncryptionReq::new("".into(), assets.pub_key.clone(), connection.verify_token.clone());
            res.write(stream).await.or_else(|err| Err(format!("{err}")))
        } else {
            let res = LoginSuccess::new((uuid!("900bf5ae-3f2f-4594-8250-1871d6aec064").as_u128() >> 4*8) as u64, uuid!("900bf5ae-3f2f-4594-8250-1871d6aec064").as_u128() as u64, this.name.clone(), VarInt::from(0));
            res.write(stream).await.or_else(|err| Err(format!("{err}")))
        }
    }
);

packet!(
    EncryptionResponse 0x01 {
        shared_secret: Vec<u8>,
        verify_token: Vec<u8>,
    },
    handler |this, stream, connection, assets| {
        let mut verify_token_plain = vec![0; assets.key.size() as usize];
        let num = assets.key.private_decrypt(&this.verify_token, &mut verify_token_plain, Padding::PKCS1).or_else(|err| Err(format!("{err}")))?;
        let verify_token_plain = &verify_token_plain[0..num];

        let mut shared_secret_plain = vec![0; assets.key.size() as usize];
        let num = assets.key.private_decrypt(&this.shared_secret, &mut shared_secret_plain, Padding::PKCS1).or_else(|err| Err(format!("{err}")))?;
        let shared_secret_plain = &shared_secret_plain[0..num];

        encrypt(verify_token_plain, shared_secret_plain, assets, connection).await?;
        Ok(())
    }
);

packet!(
    LoginAck 0x03 {},
    handler |this, stream, connection, assets| {
        connection.state = ConnectionState::Configuration;
        Ok(())
    }
);

packet!(
    ClientInfo 0x00 {
        locale: String,
        view_distance: u8,
        chat_mode: VarInt,
        chat_colors: bool,
        displayed_skin_parts: u8,
        main_hand: VarInt,
        enable_text_filtering: bool,
        allow_server_listings: bool,
    },
    handler |_this, stream, connection, assets| {
        Ok(())
    }
);

packet!(
    ConfigurationFinish 0x02 {},
    handler |_this, stream, connection, assets| {
        connection.state = ConnectionState::Play;
        Ok(())
    }
);

packet!(
    ConfigurationKeepAlive 0x03 {
        id: i64,
    },
    handler |this, stream, connection, assets| {
        if (this.id != connection.keep_alive_id) {
            Err(format!("Keep alive id mismatch: {} (received) != {} (expected)", this.id, connection.keep_alive_id))
        } else {
            Ok(())
        }
    }
);