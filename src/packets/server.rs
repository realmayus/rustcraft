use core::fmt::Debug;
use core::fmt::Display;
use std::sync::{Arc, RwLock};

use async_nbt::NbtCompound;
use async_trait::async_trait;
use log::{debug, info};
use openssl::rsa::Padding;
use tokio::io::AsyncRead;
use uuid::Uuid;

use crate::chunk::section::ChunkSection;
use crate::chunk::COLUMN_HEIGHT;
use crate::connection::ConnectionInfo;
use crate::connection::ConnectionState;
use crate::encryption::encrypt;
use crate::err::ProtError;
use crate::err::ProtError::{KeepAliveIdMismatch, TeleportIdMismatch};
use crate::packet;
use crate::packet_base;
use crate::packets::client;
use crate::packets::client::ClientPackets;
use crate::protocol_types::compound::{BitSet, Position};
use crate::protocol_types::primitives::SizedVec;
use crate::protocol_types::primitives::VarInt;
use crate::protocol_types::traits::{ReadProt, ReadProtPacket, ServerPacket, SizedProt, WriteProt};
use crate::protocol_util::name_uuid;
use crate::Assets;

packet!(
    Handshake 0x00 {
        prot_version: VarInt,
        server_address: String,
        server_port: u16,
        next_state: VarInt,
    },
    handler |this, connection, _assets| {
        connection.write().as_mut().unwrap().set_state(if this.next_state.value == 1 {
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
    handler |_this, connection, assets| {
        let res = client::StatusRes::new(assets.motd.clone());
        Ok(vec![ClientPackets::StatusRes(res)])
    }
);

packet!(
    PingReq 0x01 {
        payload: i64,
    },
    handler |this, connection, assets| {
        let res = client::PingRes::new(this.payload);
        Ok(vec![ClientPackets::PingRes(res)])
    }
);

packet!(
    LoginStart 0x00 {
        name: String,
    },
    handler |this, connection, assets| {
        info!("Player {} wants to login...", this.name);
        let mut con = connection.write();
        let con = con.as_mut().unwrap();
        con.username = this.name.clone();
        con.verify_token = vec!(rand::random::<u8>(), rand::random::<u8>(), rand::random::<u8>(), rand::random::<u8>());
        if assets.online {
            let res = client::EncryptionReq::new("".into(), assets.pub_key.clone().into(), con.verify_token.clone().into());
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
    handler |this, connection, assets| {
        let mut verify_token_plain = vec![0; assets.key.size() as usize];
        let num = assets.key.private_decrypt(&this.verify_token.vec, &mut verify_token_plain, Padding::PKCS1).or_else(|err| Err(format!("{err}")))?;
        let verify_token_plain = &verify_token_plain[0..num];

        assert_eq!(verify_token_plain, connection.read().unwrap().verify_token);
        let mut shared_secret_plain = vec![0; assets.key.size() as usize];
        let num = assets.key.private_decrypt(&this.shared_secret.vec, &mut shared_secret_plain, Padding::PKCS1).or_else(|err| Err(format!("{err}")))?;
        let shared_secret_plain = &shared_secret_plain[0..num];

        let mut cipher1 = openssl::symm::Cipher::aes_128_cfb8();
        let mut cipher2 = openssl::symm::Cipher::aes_128_cfb8();
        let mut encrypter = openssl::symm::Crypter::new(cipher1, openssl::symm::Mode::Encrypt, &shared_secret_plain.to_vec(), Some(&shared_secret_plain.to_vec())).unwrap();
        let mut decrypter = openssl::symm::Crypter::new(cipher2, openssl::symm::Mode::Decrypt, &shared_secret_plain.to_vec(), Some(&shared_secret_plain.to_vec())).unwrap();
        {
            let mut con = connection.write();
            let con = con.as_mut().unwrap();
            con.encrypter = Some(encrypter);
            con.decrypter = Some(decrypter);
        }
        debug!("Encryption enabled.");
        let username = connection.read().unwrap().username.clone();
        let uuid = encrypt(shared_secret_plain, assets, username.clone()).await?;
        let res = client::LoginSuccess::new(uuid, username, VarInt::from(0));
        Ok(vec![ClientPackets::LoginSuccess(res)])
    }
);

packet!(
    LoginAck 0x03 {},
    handler |this, connection, assets| {
        connection.write().as_mut().unwrap().set_state(ConnectionState::Configuration);
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
    handler |_this, connection, assets| {
        let res = client::ConfigurationFinish::new();
        Ok(vec![ClientPackets::ConfigurationFinish(res)])
    }
);

packet!(
    ConfigurationFinish 0x02 {},
    handler |_this, connection, assets| {
        connection.write().as_mut().unwrap().set_state(ConnectionState::Play);
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
    handler |this, connection, assets| {
        let keep_alive_id = connection.read().unwrap().keep_alive_id;
        if this.id != keep_alive_id {
            Err(KeepAliveIdMismatch(keep_alive_id, this.id))
        } else {
            Ok(vec![])
        }
    }
);

packet!(
    PlayKeepAlive 0x14 {
        id: i64,
    },
    handler |this, connection, assets| {
        let keep_alive_id = connection.read().unwrap().keep_alive_id;
        if this.id != keep_alive_id {
            Err(KeepAliveIdMismatch(keep_alive_id, this.id))
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
    handler |_this, connection, assets| {
        let mut guard = connection.write();
        let guard = guard.as_mut().unwrap();
        let p1 = client::SetHeldItem::new(0);
        let p2 = client::UpdateRecipes::new(vec![].into());
        guard.teleport_id = rand::random::<usize>().into();
        let p3 = client::SynchronizePlayerPosition::new(
            0.0, 0.0, 0.0, 0.0, 0.0, 0u8,
            guard.teleport_id.clone(),
        );
        Ok(vec![ClientPackets::SetHeldItem(p1), ClientPackets::UpdateRecipes(p2), ClientPackets::SynchronizePlayerPosition(p3)])
    }
);

packet!(
    SetPlayerPosition 0x16 {
        x: f64,
        y: f64,
        z: f64,
        on_ground: bool,
    },
    handler |this, connection, assets| {
        let mut guard = connection.write();
        let guard = guard.as_mut().unwrap();
        guard.position.x = this.x;
        guard.position.y = this.y;
        guard.position.z = this.z;
        guard.position.on_ground = this.on_ground;
        Ok(vec![])
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
    handler |this, connection, assets| {
        let mut guard = connection.write();
        let guard = guard.as_mut().unwrap();
        guard.position.x = this.x;
        guard.position.y = this.y;
        guard.position.z = this.z;
        guard.position.pitch = this.pitch as f64;
        guard.position.yaw = this.yaw as f64;
        guard.position.on_ground = this.on_ground;
        Ok(vec![])
    }
);

packet!(
    SetPlayerRotation 0x18 {
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    },
    handler |this, connection, assets| {
        let mut guard = connection.write();
        let guard = guard.as_mut().unwrap();
        guard.position.pitch = this.pitch as f64;
        guard.position.yaw = this.yaw as f64;
        guard.position.on_ground = this.on_ground;
        Ok(vec![])
    }
);

async fn get_chunks() -> Vec<u8> {
    let mut chunks = vec![];
    for _ in 0..COLUMN_HEIGHT {
        let mut chunk = ChunkSection::new();
        chunk.fill(8);
        chunk.set_block(Position::new(5, 5, 5), 1).unwrap();
        let mut buf = vec![];
        chunk.write(&mut buf).await.unwrap();
        chunks.append(&mut buf);
        // }
    }
    chunks
}
packet!(
    ConfirmTeleportation 0x00 {
        teleport_id: VarInt,
    },
    handler |this, connection, assets| {
        let expected_id = connection.read().unwrap().teleport_id;
        if expected_id == this.teleport_id {
            let stone = get_chunks().await;
            let mut chunks: Vec<client::ChunkDataAndUpdateLight> = vec![];
            for i in -3..=3 {
                for j in -3..=3 {
                    let p = client::ChunkDataAndUpdateLight::new(
                        i,
                        j,
                        NbtCompound::new(),
                        stone.clone().into(),
                        vec![].into(),
                        BitSet(vec![].into()),
                        BitSet(vec![].into()),
                        BitSet(vec![].into()),
                        BitSet(vec![].into()),
                        vec![].into(),
                        vec![].into()
                    );
                    chunks.push(p);
                }
            }


            let p6 = client::SetDefaultSpawnPosition::new(Position {x:0, y:0, z:0}, 0.0);
            let p7 = client::SetCenterChunk::new(0.into(), 0.into());
            println!("Sending chunks");
            let mut to_send = vec![ClientPackets::SetDefaultSpawnPosition(p6), ClientPackets::SetCenterChunk(p7)];
            for chunk in chunks {
                to_send.push(ClientPackets::ChunkDataAndUpdateLight(chunk));
            }
            println!("Sending {} packets", to_send.len());
            Ok(to_send)
        } else {
            Err(TeleportIdMismatch(expected_id, this.teleport_id))
        }
    }
);

packet!(
    PlayerCommand 0x21 {
        entity: VarInt,
        action: VarInt,
        jump_boost: VarInt,
    },
    handler |_this, connection, assets| {
        Ok(vec![])
    }
);
