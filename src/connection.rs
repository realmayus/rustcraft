
#[derive(Debug)]
pub(crate) enum ConnectionState {
    Handshake,
    Status,
    Login,
    Configuration,
    Play
}

pub(crate) struct Connection {
    pub(crate) state: ConnectionState,
    pub(crate) verify_token: Vec<u8>,
    pub(crate) shared_secret: Vec<u8>,
    pub(crate) player: String,
}