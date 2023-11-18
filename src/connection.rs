use log::debug;

#[derive(Debug, Copy, Clone)]
pub(crate) enum ConnectionState {
    Handshake,
    Status,
    Login,
    Configuration,
    Play
}

pub(crate) struct Connection {
    state: ConnectionState,
    pub(crate) verify_token: Vec<u8>,
    pub(crate) shared_secret: Vec<u8>,
    pub(crate) player: String,
    pub(crate) keep_alive_id: i64,
    pub(crate) closed: bool,
}

impl Connection {
    pub(crate) fn new() -> Connection {
        Connection { state: ConnectionState::Handshake, verify_token: vec!(0, 0, 0, 0), shared_secret: vec![], player: "".to_string(), keep_alive_id: 0, closed: false}
    }

    pub(crate) fn set_state(&mut self, state: ConnectionState) {
        debug!("Connection state is now: {:?}", state);
        self.state = state;
    }
    pub(crate) fn state(&self) -> &ConnectionState {
        &self.state
    }
}