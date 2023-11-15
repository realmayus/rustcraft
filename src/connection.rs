
#[derive(Debug)]
pub(crate) enum ConnectionState {
    Handshake,
    Status,
    Login,
    Play
}

pub(crate) struct Connection {
    pub(crate) state: ConnectionState,
}