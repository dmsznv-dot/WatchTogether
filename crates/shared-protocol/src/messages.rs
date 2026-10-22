//! Strongly-typed WebSocket protocol messages with externally-tagged envelope:
//! {"type": "<event>", "payload": { ... }}

use crate::models::{
    Participant, ParticipantId, PermissionMode, PlaybackStatus,
    RoomCode, RoomStateSnapshot, RoomToken, VideoMetadata,
};
use serde::{Deserialize, Serialize};

/// Messages originating from client and sent to host/server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ClientMessage {
    #[serde(rename = "session.identify")]
    SessionIdentify,
    #[serde(rename = "room.get_state")]
    RoomGetState,
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

/// Messages originating from host/server and broadcast or dispatched to client(s).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ServerMessage {
    #[serde(rename = "session.identity")]
    SessionIdentity { participant_id: ParticipantId },
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
