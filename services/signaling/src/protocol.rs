use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeerRole {
    Host,
    Guest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IceCandidateType {
    Host,
    ServerReflexive,
    Relay,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IceCandidate {
    pub candidate_type: IceCandidateType,
    pub ip: String,
    pub port: u16,
    pub protocol: String,
    pub priority: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionDescription {
    pub sdp_type: String, // "offer" or "answer"
    pub sdp: String,
}

/// Inbound messages from clients to signaling server
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ClientSignal {
    #[serde(rename = "signal.join")]
    Join {
        room_code: String,
        peer_id: String,
        #[serde(default)]
        room_token: String,
        #[serde(default)]
        role: Option<PeerRole>,
    },
    #[serde(rename = "signal.offer")]
    Offer {
        sender_id: String,
        target_id: String,
        desc: SessionDescription,
    },
    #[serde(rename = "signal.answer")]
    Answer {
        sender_id: String,
        target_id: String,
        desc: SessionDescription,
    },
    #[serde(rename = "signal.ice_candidate")]
    IceCandidate {
        sender_id: String,
        target_id: String,
        candidate: IceCandidate,
    },
    #[serde(rename = "signal.peer_disconnected")]
    PeerDisconnected { peer_id: String },
}

/// Outbound messages from signaling server to clients
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ServerSignal {
    #[serde(rename = "signal.peer_joined")]
    PeerJoined {
        peer_id: String,
        role: PeerRole,
    },
    #[serde(rename = "signal.offer")]
    Offer {
        sender_id: String,
        target_id: String,
        desc: SessionDescription,
    },
    #[serde(rename = "signal.answer")]
    Answer {
        sender_id: String,
        target_id: String,
        desc: SessionDescription,
    },
    #[serde(rename = "signal.ice_candidate")]
    IceCandidate {
        sender_id: String,
        target_id: String,
        candidate: IceCandidate,
    },
    #[serde(rename = "signal.peer_disconnected")]
    PeerDisconnected { peer_id: String },
    #[serde(rename = "signal.peer_left")]
    PeerLeft {
        peer_id: String,
        reason: String,
    },
    #[serde(rename = "signal.room_closed")]
    RoomClosed { reason: String },
    #[serde(rename = "signal.error")]
    Error { code: String, message: String },
}

#[derive(Debug, thiserror::Error)]
pub enum SignalError {
    #[error("Room not found: {0}")]
    RoomNotFound(String),
    #[error("Room capacity reached")]
    RoomFull,
    #[error("Invalid room code: {0}")]
    InvalidRoomCode(String),
    #[error("Invalid room token")]
    InvalidToken,
    #[error("Participant not found: {0}")]
    ParticipantNotFound(String),
    #[error("Invalid message: {0}")]
    InvalidMessage(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

impl SignalError {
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::RoomNotFound(_) => "ROOM_NOT_FOUND",
            Self::RoomFull => "ROOM_FULL",
            Self::InvalidRoomCode(_) => "INVALID_ROOM_CODE",
            Self::InvalidToken => "INVALID_TOKEN",
            Self::ParticipantNotFound(_) => "PARTICIPANT_NOT_FOUND",
            Self::InvalidMessage(_) => "INVALID_MESSAGE",
            Self::Internal(_) => "INTERNAL_ERROR",
        }
    }
}
