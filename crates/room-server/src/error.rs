use shared_protocol::errors::{ErrorCode, ProtocolError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RoomServerError {
    #[error("Room not found: {0}")]
    RoomNotFound(String),

    #[error("Room capacity reached ({capacity} participants)")]
    RoomFull { capacity: usize },

    #[error("Invalid room code: {0}")]
    InvalidRoomCode(String),

    #[error("Invalid room token")]
    InvalidToken,

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Stale revision: incoming {command_revision} <= current {current_revision}")]
    StaleRevision {
        command_revision: u64,
        current_revision: u64,
    },

    #[error("Participant not found: {0}")]
    ParticipantNotFound(String),

    #[error("Invalid message: {0}")]
    InvalidMessage(String),

    #[error("Room is closed")]
    RoomClosed,

    #[error("Protocol error: {0}")]
    Protocol(#[from] ProtocolError),

    #[error("Media stream error: {0}")]
    MediaStream(#[from] media_stream::MediaStreamError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("WebSocket error: {0}")]
    WebSocket(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl RoomServerError {
    pub fn to_error_code(&self) -> ErrorCode {
        match self {
            Self::RoomNotFound(_) | Self::RoomClosed => ErrorCode::RoomNotFound,
            Self::RoomFull { .. } => ErrorCode::RoomFull,
            Self::InvalidRoomCode(_) => ErrorCode::InvalidRoomCode,
            Self::InvalidToken => ErrorCode::InvalidToken,
            Self::PermissionDenied(_) => ErrorCode::PermissionDenied,
            Self::StaleRevision { .. } => ErrorCode::StaleRevision,
            Self::ParticipantNotFound(_) => ErrorCode::ParticipantNotFound,
            Self::InvalidMessage(_) | Self::Serialization(_) => ErrorCode::InvalidMessage,
            Self::Protocol(pe) => pe.error_code(),
            _ => ErrorCode::InternalError,
        }
    }
}
