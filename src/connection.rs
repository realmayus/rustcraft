use crate::protocol_types::primitives::VarInt;
use log::debug;
use openssl::symm::Crypter;

#[derive(Debug, Copy, Clone)]
pub(crate) enum ConnectionState {
    Handshake,
    Status,
    Login,
    Configuration,
    Play,
}

pub(crate) struct Connection {
    state: ConnectionState,
    pub(crate) verify_token: Vec<u8>,
    pub(crate) encrypter: Option<Crypter>,
    pub(crate) decrypter: Option<Crypter>,
    pub(crate) username: String,
    pub(crate) teleport_id: VarInt,
    pub(crate) keep_alive_id: i64,
    pub(crate) closed: bool,
}

impl Connection {
    pub(crate) fn new() -> Connection {
        Connection {
            state: ConnectionState::Handshake,
            verify_token: vec![0, 0, 0, 0],
            encrypter: None,
            decrypter: None,
            username: "".to_string(),
            teleport_id: 0.into(),
            keep_alive_id: 0,
            closed: false,
        }
    }

    pub(crate) fn set_state(&mut self, state: ConnectionState) {
        debug!("Connection state is now: {:?}", state);
        self.state = state;
    }
    pub(crate) fn state(&self) -> &ConnectionState {
        &self.state
    }
}
