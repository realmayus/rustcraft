use core::fmt::Debug;
use core::fmt::Display;
use std::sync::Arc;
use uuid::Uuid;

use async_trait::async_trait;
use log::{debug, info};
use openssl::rsa::Padding;
use tokio::io::AsyncRead;
use tokio::net::TcpStream;

use crate::connection::Connection;
use crate::connection::ConnectionState;
use crate::encryption::encrypt;
use crate::err::ProtError;
use crate::err::ProtError::{KeepAliveIdMismatch, TeleportIdMismatch};
use crate::packet;
use crate::packet_base;
use crate::packets::client;
use crate::packets::client::ClientPackets;
use crate::protocol_types::primitives::SizedVec;
use crate::protocol_types::primitives::VarInt;
use crate::protocol_types::traits::{ReadProt, ReadProtPacket, ServerPacket, SizedProt};
use crate::protocol_util::name_uuid;
use crate::Assets;

packet!(
    Handshake 0x00 {
        prot_version: VarInt,
        server_address: String,
        server_port: u16,
        next_state: VarInt,
    },
    handler |this, stream, connection, _assets| {
        connection.set_state(if this.next_state.value == 1 {
            ConnectionState::Status
        } else if this.next_state.value == 2 {
            ConnectionState::Login
        } else {
            return Err(ProtError::InvalidNextState(this.next_state.clone()));
        });
        Ok(vec![])
    }
);

packet!(
    StatusReq 0x00 {},
    handler |_this, stream, connection, assets| {
        let res = client::StatusRes::new(assets.motd.clone());
        Ok(vec![ClientPackets::StatusRes(res)])
    }
);

packet!(
    PingReq 0x01 {
        payload: i64,
    },
    handler |this, stream, connection, assets| {
        let res = client::PingRes::new(this.payload);
        Ok(vec![ClientPackets::PingRes(res)])
    }
);

packet!(
    LoginStart 0x00 {
        name: String,
    },
    handler |this, stream, connection, assets| {
        info!("Player {} wants to login...", this.name);
        connection.username = this.name.clone();
        connection.verify_token = vec!(rand::random::<u8>(), rand::random::<u8>(), rand::random::<u8>(), rand::random::<u8>());
        if assets.online {
            let res = client::EncryptionReq::new("".into(), assets.pub_key.clone().into(), connection.verify_token.clone().into());
            Ok(vec![ClientPackets::EncryptionReq(res)])
        } else {
            let res = client::LoginSuccess::new(name_uuid(format!("OfflinePlayer:{}", this.name)), this.name.clone(), VarInt::from(0));
            Ok(vec![ClientPackets::LoginSuccess(res)])
        }
    }
);

packet!(
    EncryptionResponse 0x01 {
        shared_secret: SizedVec<u8>,
        verify_token: SizedVec<u8>,
    },
    handler |this, stream, connection, assets| {
        let mut verify_token_plain = vec![0; assets.key.size() as usize];
        let num = assets.key.private_decrypt(&this.verify_token.vec, &mut verify_token_plain, Padding::PKCS1).or_else(|err| Err(format!("{err}")))?;
        let verify_token_plain = &verify_token_plain[0..num];
        assert_eq!(verify_token_plain, connection.verify_token);
        let mut shared_secret_plain = vec![0; assets.key.size() as usize];
        let num = assets.key.private_decrypt(&this.shared_secret.vec, &mut shared_secret_plain, Padding::PKCS1).or_else(|err| Err(format!("{err}")))?;
        let shared_secret_plain = &shared_secret_plain[0..num];

        let mut cipher1 = openssl::symm::Cipher::aes_128_cfb8();
        let mut cipher2 = openssl::symm::Cipher::aes_128_cfb8();
        let mut encrypter = openssl::symm::Crypter::new(cipher1, openssl::symm::Mode::Encrypt, &shared_secret_plain.to_vec(), Some(&shared_secret_plain.to_vec())).unwrap();
        let mut decrypter = openssl::symm::Crypter::new(cipher2, openssl::symm::Mode::Decrypt, &shared_secret_plain.to_vec(), Some(&shared_secret_plain.to_vec())).unwrap();
        connection.encrypter = Some(encrypter);
        connection.decrypter = Some(decrypter);
        debug!("Encryption enabled.");
        let uuid = encrypt(shared_secret_plain, assets, connection).await?;
        let res = client::LoginSuccess::new(uuid, connection.username.clone(), VarInt::from(0));
        Ok(vec![ClientPackets::LoginSuccess(res)])
    }
);

packet!(
    LoginAck 0x03 {},
    handler |this, stream, connection, assets| {
        connection.set_state(ConnectionState::Configuration);
        let res = client::RegistryData::new(assets.registry.clone());
        Ok(vec![ClientPackets::RegistryData(res)])
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
        let res = client::ConfigurationFinish::new();
        Ok(vec![ClientPackets::ConfigurationFinish(res)])
    }
);

packet!(
    ConfigurationFinish 0x02 {},
    handler |_this, stream, connection, assets| {
        connection.set_state(ConnectionState::Play);
        let res = client::PlayLogin::new(
            0,
            false,
            vec!["world".into()].into(),
            VarInt::from(2),
            VarInt::from(5),
            VarInt::from(3),
            false,
            false,
            false,
            "minecraft:overworld".into(),
            "minecraft:overworld".into(),
            0, 0, 0, false, false, false, None, None, 0.into());
        Ok(vec![ClientPackets::PlayLogin(res)])
    }
);

packet!(
    ConfigurationKeepAlive 0x03 {
        id: i64,
    },
    handler |this, stream, connection, assets| {
        if this.id != connection.keep_alive_id {
            Err(KeepAliveIdMismatch(connection.keep_alive_id, this.id))
        } else {
            Ok(vec![])
        }
    }
);

packet!(
    PlayKeepAlive 0x14 {
        id: i64,
    },
    handler |this, stream, connection, assets| {
        if this.id != connection.keep_alive_id {
            Err(KeepAliveIdMismatch(connection.keep_alive_id, this.id))
        } else {
            Ok(vec![])
        }
    }
);

packet!(
    PlayerSession 0x06 {
        session_id: Uuid,
        key_expires_at: i64,
        key: SizedVec<u8>,
        key_signature: SizedVec<u8>
    },
    handler |_this, stream, connection, assets| {
        let p1 = client::SetHeldItem::new(0);
        let p2 = client::UpdateRecipes::new(vec![].into());
        let p3 = client::SynchronizePlayerPosition::new(
            0.0, 0.0, 0.0, 0.0, 0.0, 0u8,
            rand::random::<usize>().into(),
        );
        Ok(vec![ClientPackets::SetHeldItem(p1), ClientPackets::UpdateRecipes(p2), ClientPackets::SynchronizePlayerPosition(p3)])
    }
);

packet!(
    SetPlayerPositionAndRotation 0x17 {
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    },
    handler |_this, stream, connection, assets| {
        Ok(vec![])
    }
);

packet!(
    SetPlayerPosition 0x16 {
        x: f64,
        y: f64,
        z: f64,
        on_ground: bool,
    },
    handler |_this, stream, connection, assets| {
        Ok(vec![])
    }
);

packet!(
    ConfirmTeleportation 0x00 {
        teleport_id: VarInt,
    },
    handler |this, stream, connection, assets| {
        if connection.teleport_id == this.teleport_id {
            Ok(vec![])
        } else {
            Err(TeleportIdMismatch(connection.teleport_id, this.teleport_id))
        }
    }
);
