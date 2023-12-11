use crate::protocol_types::compound::Position;
use crate::protocol_types::primitives::VarInt;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};

#[derive(Debug)]
pub(crate) enum ProtError {
    InvalidNextState(VarInt),
    ChannelClosed,
    KeepAliveIdMismatch(i64, i64),
    TeleportIdMismatch(VarInt, VarInt),
    PositionOutOfBounds(Position),
    Any(String),
}

impl ProtError {
    pub(crate) fn is_fatal(&self) -> bool {
        match self {
            ProtError::InvalidNextState(_) => true,
            ProtError::KeepAliveIdMismatch(_, _) => true,
            ProtError::TeleportIdMismatch(_, _) => true,
            ProtError::PositionOutOfBounds(_) => true,
            ProtError::ChannelClosed => true,
            ProtError::Any(_) => false,
        }
    }
}

impl Display for ProtError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtError::InvalidNextState(v) => write!(f, "Invalid next state: {}", v),
            ProtError::KeepAliveIdMismatch(v1, v2) => {
                write!(f, "Keep alive id mismatch: {} != {}", v1, v2)
            }
            ProtError::TeleportIdMismatch(v1, v2) => {
                write!(f, "Teleport id mismatch: {} != {}", v1, v2)
            }
            ProtError::PositionOutOfBounds(v) => write!(f, "Position out of bounds: {:?}", v),
            ProtError::ChannelClosed => write!(f, "Channel closed"),
            ProtError::Any(v) => write!(f, "{}", v),
        }
    }
}

impl Error for ProtError {}

impl From<String> for ProtError {
    fn from(s: String) -> Self {
        ProtError::Any(s)
    }
}
