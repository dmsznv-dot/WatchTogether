use serde::{Serialize, Serializer};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Room server error: {0}")]
    RoomServer(#[from] room_server::RoomServerError),

    #[error("Media stream error: {0}")]
    MediaStream(#[from] media_stream::MediaStreamError),

    #[error("Network error: {0}")]
    Network(#[from] network::NetworkError),

    #[error("Protocol error: {0}")]
    Protocol(#[from] shared_protocol::errors::ProtocolError),

    #[error("Revision error: {0}")]
    Revision(#[from] sync_engine::RevisionError),

    #[error("Clock sync error: {0}")]
    ClockSync(#[from] sync_engine::ClockSyncError),

    #[error("Media engine error: {0}")]
    MediaEngine(#[from] media_engine::MediaEngineError),

    #[error("Invalid room code: {0}")]
    InvalidRoomCode(String),

    #[error("Invalid room token")]
    InvalidToken,

    #[error("Invalid playback speed: {0}. Allowed: 0.5, 0.75, 1.0, 1.25, 1.5, 2.0")]
    InvalidSpeed(f32),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("No active room session")]
    NoActiveSession,

    #[error("Session already active")]
    SessionAlreadyActive,

    #[error("Internal error: {0}")]
    Internal(String),
}

#[derive(Serialize)]
struct ErrorResponse {
    code: String,
    message: String,
}

impl AppError {
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::InvalidRoomCode(_) => "INVALID_ROOM_CODE",
            Self::InvalidToken => "INVALID_TOKEN",
            Self::InvalidSpeed(_) => "INVALID_MESSAGE",
            Self::PermissionDenied(_) => "PERMISSION_DENIED",
            Self::NoActiveSession => "ROOM_NOT_FOUND",
            Self::SessionAlreadyActive => "SESSION_ALREADY_ACTIVE",
            Self::RoomServer(e) => e.to_error_code().as_str(),
            Self::Protocol(e) => e.error_code().as_str(),
            _ => "INTERNAL_ERROR",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ErrorResponse {
            code: self.error_code().to_string(),
            message: self.to_string(),
        }
        .serialize(serializer)
    }
}
