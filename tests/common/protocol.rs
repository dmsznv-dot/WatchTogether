use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RoomId(pub String);

impl RoomId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl fmt::Display for RoomId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RoomCode(pub String);

impl RoomCode {
    pub fn new(code: impl Into<String>) -> Self {
        Self(code.into())
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomToken(pub String);

impl RoomToken {
    pub fn new(token: impl Into<String>) -> Self {
        Self(token.into())
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ParticipantId(pub String);

impl ParticipantId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl fmt::Display for ParticipantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticipantRole {
    Host,
    Guest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    FreeControl,
    HostOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackStatus {
    Playing,
    Paused,
    Buffering,
}

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaybackState {
    pub revision: u64,
    pub status: PlaybackStatus,
    pub position_ms: u64,
    pub speed: f32,
    pub updated_at_server_ms: u64,
    pub duration_ms: u64,
}

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ClientMessage {
    #[serde(rename = "room.create")]
    RoomCreate {
        host_name: String,
        video: VideoMetadata,
        permission_mode: PermissionMode,
    },
    #[serde(rename = "room.join")]
    RoomJoin {
        room_code: RoomCode,
        room_token: RoomToken,
        display_name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        participant_id: Option<ParticipantId>,
    },
    #[serde(rename = "playback.play")]
    PlaybackPlay {
        revision: u64,
        client_timestamp_ms: u64,
    },
    #[serde(rename = "playback.pause")]
    PlaybackPause {
        revision: u64,
        position_ms: u64,
        client_timestamp_ms: u64,
    },
    #[serde(rename = "playback.seek")]
    PlaybackSeek {
        revision: u64,
        target_position_ms: u64,
        client_timestamp_ms: u64,
    },
    #[serde(rename = "playback.speed")]
    PlaybackSpeed {
        revision: u64,
        speed: f32,
        client_timestamp_ms: u64,
    },
    #[serde(rename = "participant.status")]
    ParticipantStatus {
        participant_id: ParticipantId,
        buffered_position_ms: u64,
        current_position_ms: u64,
        is_buffering: bool,
        ping_ms: u64,
    },
    #[serde(rename = "sync.ping")]
    SyncPing {
        ping_id: u64,
        client_send_time_ms: u64,
    },
    #[serde(rename = "room.set_permissions")]
    RoomSetPermissions {
        permission_mode: PermissionMode,
    },
    #[serde(rename = "participant.kick")]
    ParticipantKick {
        target_id: ParticipantId,
    },
    #[serde(rename = "room.leave")]
    RoomLeave {
        participant_id: ParticipantId,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ServerMessage {
    #[serde(rename = "room.state")]
    RoomState(RoomStateSnapshot),
    #[serde(rename = "playback.updated")]
    PlaybackUpdated {
        revision: u64,
        status: PlaybackStatus,
        position_ms: u64,
        speed: f32,
        updated_at_server_ms: u64,
        triggered_by: ParticipantId,
        action: String,
    },
    #[serde(rename = "participant.joined")]
    ParticipantJoined(Participant),
    #[serde(rename = "participant.left")]
    ParticipantLeft {
        participant_id: ParticipantId,
        reason: String,
    },
    #[serde(rename = "participant.kicked")]
    ParticipantKicked {
        participant_id: ParticipantId,
        reason: String,
    },
    #[serde(rename = "sync.pong")]
    SyncPong {
        ping_id: u64,
        client_send_time_ms: u64,
        server_receive_time_ms: u64,
        server_send_time_ms: u64,
    },
    #[serde(rename = "room.closed")]
    RoomClosed {
        reason: String,
    },
    #[serde(rename = "room.error")]
    Error {
        code: String,
        message: String,
    },
}

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

#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum RevisionError {
    #[error("Stale command revision: incoming revision {incoming} <= current revision {current}")]
    StaleCommand { incoming: u64, current: u64 },
    #[error("Stale state broadcast: incoming state revision {incoming} <= client revision {current}")]
    StaleStateBroadcast { incoming: u64, current: u64 },
    #[error("Revision counter overflow")]
    RevisionOverflow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionTracker {
    current: u64,
}

impl RevisionTracker {
    pub fn new(initial: u64) -> Self {
        Self { current: initial }
    }

    pub fn current(&self) -> u64 {
        self.current
    }

    pub fn gate_command(&self, cmd_rev: u64) -> Result<(), RevisionError> {
        if cmd_rev <= self.current {
            Err(RevisionError::StaleCommand {
                incoming: cmd_rev,
                current: self.current,
            })
        } else {
            Ok(())
        }
    }

    pub fn accept_command_and_advance(&mut self, cmd_rev: u64) -> Result<u64, RevisionError> {
        self.gate_command(cmd_rev)?;
        self.current = self.current.checked_add(1).ok_or(RevisionError::RevisionOverflow)?;
        Ok(self.current)
    }

    pub fn apply_incoming_broadcast(&mut self, broadcast_rev: u64) -> Result<(), RevisionError> {
        if broadcast_rev <= self.current {
            Err(RevisionError::StaleStateBroadcast {
                incoming: broadcast_rev,
                current: self.current,
            })
        } else {
            self.current = broadcast_rev;
            Ok(())
        }
    }

    pub fn advance(&mut self) -> Result<u64, RevisionError> {
        self.current = self.current.checked_add(1).ok_or(RevisionError::RevisionOverflow)?;
        Ok(self.current)
    }
}
