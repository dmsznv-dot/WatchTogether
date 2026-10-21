//! Core domain models and value objects shared across client and server.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique room identifier (internal UUID string).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RoomId(pub String);

impl RoomId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RoomId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 6-character uppercase alphanumeric room code displayed to users (e.g. "K7M2P9").
/// Uses Crockford-inspired un-ambiguous charset (excludes 0, O, 1, I).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RoomCode(pub String);

impl RoomCode {
    pub fn new(code: impl Into<String>) -> Self {
        Self(code.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_valid(&self) -> bool {
        self.0.len() == 6
            && self
                .0
                .chars()
                .all(|c| matches!(c, '2'..='9' | 'A'..='H' | 'J'..='N' | 'P'..='Z'))
    }
}

impl fmt::Display for RoomCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Cryptographically random token (>= 32 chars / 256 bits entropy) required for authorization.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RoomToken(pub String);

impl RoomToken {
    pub fn new(token: impl Into<String>) -> Self {
        Self(token.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_valid(&self) -> bool {
        self.0.len() >= 32
    }
}

impl fmt::Display for RoomToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique participant identifier (UUID or session ID string).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ParticipantId(pub String);

impl ParticipantId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ParticipantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Role of participant within a room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticipantRole {
    Host,
    Guest,
}

/// Room control permission policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    /// Any participant can pause, play, seek, or change speed.
    FreeControl,
    /// Only the host participant can control playback.
    HostOnly,
}

/// Discrete playback status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackStatus {
    Playing,
    Paused,
    Buffering,
}

/// Action trigger name for playback state updates.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackAction {
    Play,
    Pause,
    Seek,
    Speed,
}

impl fmt::Display for PlaybackAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Play => write!(f, "play"),
            Self::Pause => write!(f, "pause"),
            Self::Seek => write!(f, "seek"),
            Self::Speed => write!(f, "speed"),
        }
    }
}

/// State representation of a participant in a room session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Participant {
    pub id: ParticipantId,
    pub display_name: String,
    pub role: ParticipantRole,
    pub is_connected: bool,
    pub buffered_position_ms: u64,
    pub current_position_ms: u64,
    pub ping_ms: u64,
    pub joined_at_ms: u64,
}

/// Authoritative room playback state synchronized across all clients.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaybackState {
    /// Monotonically increasing revision number.
    pub revision: u64,
    /// Current playback status (Playing, Paused, Buffering).
    pub status: PlaybackStatus,
    /// Authoritative playback position in milliseconds.
    pub position_ms: u64,
    /// Playback rate (0.5, 0.75, 1.0, 1.25, 1.5, 2.0).
    pub speed: f32,
    /// Server UNIX timestamp in milliseconds when this state was updated.
    pub updated_at_server_ms: u64,
    /// Total duration of the video in milliseconds.
    pub duration_ms: u64,
}

/// Technical metadata describing the media file being streamed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoMetadata {
    pub filename: String,
    pub file_size_bytes: u64,
    pub duration_ms: u64,
    pub mime_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

/// Complete room state snapshot transmitted to newly joined or reconnected participants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoomStateSnapshot {
    pub room_id: RoomId,
    pub room_code: RoomCode,
    pub host_id: ParticipantId,
    pub permission_mode: PermissionMode,
    pub participants: Vec<Participant>,
    pub playback: PlaybackState,
    pub video: VideoMetadata,
}
