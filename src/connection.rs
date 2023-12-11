use std::fmt::{Debug, Formatter};

use log::debug;
use openssl::symm::Crypter;
use tokio::sync::mpsc;

use crate::packets::client::ClientPackets;
use crate::protocol_types::primitives::VarInt;

#[derive(Debug, Copy, Clone)]
pub(crate) enum ConnectionState {
    Handshake,
    Status,
    Login,
    Configuration,
    Play,
}

#[derive(Debug, Clone)]
pub(crate) struct Position {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) z: f64,
    pub(crate) pitch: f64,
    pub(crate) yaw: f64,
    pub(crate) on_ground: bool,
}

pub(crate) struct ConnectionInfo {
    state: ConnectionState,
    pub(crate) verify_token: Vec<u8>,
    pub(crate) encrypter: Option<Crypter>,
    pub(crate) decrypter: Option<Crypter>,
    pub(crate) username: String,
    pub(crate) teleport_id: VarInt,
    pub(crate) keep_alive_id: i64,
    closed: bool,
    pub(crate) position: Position,
    tx: Option<mpsc::Sender<ClientPackets>>,
}
impl Debug for ConnectionInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionInfo")
            .field("state", &self.state)
            .field("username", &self.username)
            .field("closed", &self.closed)
            .field("position", &self.position)
            .finish()
    }
}

impl ConnectionInfo {
    pub(crate) fn new() -> ConnectionInfo {
        ConnectionInfo {
            state: ConnectionState::Handshake,
            verify_token: vec![0, 0, 0, 0],
            encrypter: None,
            decrypter: None,
            username: "".to_string(),
            teleport_id: 0.into(),
            keep_alive_id: 0,
            closed: false,
            position: Position {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                pitch: 0.0,
                yaw: 0.0,
                on_ground: false,
            },
            tx: None,
        }
    }

    pub(crate) fn set_state(&mut self, state: ConnectionState) {
        debug!("Connection state is now: {:?}", state);
        self.state = state;
    }
    pub(crate) fn state(&self) -> &ConnectionState {
        &self.state
    }

    pub(crate) fn close(&mut self) {
        self.closed = true;
        self.tx = None;
    }

    pub(crate) fn closed(&self) -> bool {
        self.closed
    }
}
