//! Shared protocol definitions, models, messages, and errors for the Watch Together application.

pub mod errors;
pub mod messages;
pub mod models;

pub use errors::{ErrorCode, ProtocolError};
pub use messages::{ClientMessage, ServerMessage};
pub use models::{
    Participant, ParticipantId, ParticipantRole, PermissionMode, PlaybackAction, PlaybackState,
    PlaybackStatus, RoomCode, RoomId, RoomStateSnapshot, RoomToken, VideoMetadata,
};
