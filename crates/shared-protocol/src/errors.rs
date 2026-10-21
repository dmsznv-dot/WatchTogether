//! Protocol errors and standardized error codes.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Standardized machine-readable error codes sent over WebSocket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    RoomNotFound,
    RoomFull,
    InvalidRoomCode,
    InvalidToken,
    PermissionDenied,
    StaleRevision,
    ParticipantNotFound,
    InvalidMessage,
    InvalidRange,
    HostQuit,
    InternalError,
}

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RoomNotFound => "ROOM_NOT_FOUND",
            Self::RoomFull => "ROOM_FULL",
            Self::InvalidRoomCode => "INVALID_ROOM_CODE",
            Self::InvalidToken => "INVALID_TOKEN",
            Self::PermissionDenied => "PERMISSION_DENIED",
            Self::StaleRevision => "STALE_REVISION",
            Self::ParticipantNotFound => "PARTICIPANT_NOT_FOUND",
            Self::InvalidMessage => "INVALID_MESSAGE",
            Self::InvalidRange => "INVALID_RANGE",
            Self::HostQuit => "HOST_QUIT",
            Self::InternalError => "INTERNAL_ERROR",
        }
    }
}

/// Domain errors encountered during protocol parsing and room enforcement.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum ProtocolError {
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

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Invalid message format: {0}")]
    InvalidMessage(String),

    #[error("Invalid HTTP Range header: {0}")]
    InvalidRangeHeader(String),
}

impl ProtocolError {
    pub fn error_code(&self) -> ErrorCode {
        match self {
            Self::RoomNotFound(_) => ErrorCode::RoomNotFound,
            Self::RoomFull { .. } => ErrorCode::RoomFull,
            Self::InvalidRoomCode(_) => ErrorCode::InvalidRoomCode,
            Self::InvalidToken => ErrorCode::InvalidToken,
            Self::PermissionDenied(_) => ErrorCode::PermissionDenied,
            Self::StaleRevision { .. } => ErrorCode::StaleRevision,
            Self::ParticipantNotFound(_) => ErrorCode::ParticipantNotFound,
            Self::Serialization(_) | Self::InvalidMessage(_) => ErrorCode::InvalidMessage,
            Self::InvalidRangeHeader(_) => ErrorCode::InvalidRange,
        }
    }
}
