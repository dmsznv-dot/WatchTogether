use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IceCandidateType {
    Host,
    ServerReflexive, // STUN
    Relay,           // TURN
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IceCandidate {
    pub candidate_type: IceCandidateType,
    pub ip: String,
    pub port: u16,
    pub protocol: String, // "udp" or "tcp"
    pub priority: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionDescription {
    pub sdp_type: String, // "offer" or "answer"
    pub sdp: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum SignalingMessage {
    #[serde(rename = "signal.join")]
    Join { room_code: String, peer_id: String },
    #[serde(rename = "signal.offer")]
    Offer { sender_id: String, target_id: String, desc: SessionDescription },
    #[serde(rename = "signal.answer")]
    Answer { sender_id: String, target_id: String, desc: SessionDescription },
    #[serde(rename = "signal.ice_candidate")]
    IceCandidate { sender_id: String, target_id: String, candidate: IceCandidate },
    #[serde(rename = "signal.peer_disconnected")]
    PeerDisconnected { peer_id: String },
}

#[derive(Debug, Clone)]
pub struct StunConfig {
    pub server_url: String,
    pub timeout: Duration,
}

impl Default for StunConfig {
    fn default() -> Self {
        Self {
            server_url: "stun:stun.l.google.com:19302".into(),
            timeout: Duration::from_millis(1500),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TurnConfig {
    pub server_url: String,
    pub username: String,
    pub credential: String,
    pub realm: String,
}

impl Default for TurnConfig {
    fn default() -> Self {
        Self {
            server_url: "turn:relay.example.com:3478".into(),
            username: "guest".into(),
            credential: "secret_turn_token_2026".into(),
            realm: "watch-together.local".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed,
}

#[derive(Debug, Clone)]
pub struct ReconnectionConfig {
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_factor: f64,
    pub max_attempts: usize,
    pub heartbeat_interval: Duration,
    pub heartbeat_timeout: Duration,
}

impl Default for ReconnectionConfig {
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_millis(1000),
            max_delay: Duration::from_secs(30),
            backoff_factor: 2.0,
            max_attempts: 5,
            heartbeat_interval: Duration::from_millis(1500),
            heartbeat_timeout: Duration::from_millis(3000),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReconnectionStateMachine {
    pub config: ReconnectionConfig,
    pub state: ConnectionState,
    pub attempt_count: usize,
    pub current_delay: Duration,
    pub last_heartbeat_rx_ms: u64,
    pub reconnection_token: Option<String>,
}

impl ReconnectionStateMachine {
    pub fn new(config: ReconnectionConfig) -> Self {
        let initial_delay = config.initial_delay;
        Self {
            config,
            state: ConnectionState::Disconnected,
            attempt_count: 0,
            current_delay: initial_delay,
            last_heartbeat_rx_ms: 0,
            reconnection_token: None,
        }
    }

    pub fn default_config() -> Self {
        Self::new(ReconnectionConfig::default())
    }

    pub fn set_connected(&mut self, token: String, now_ms: u64) {
        self.state = ConnectionState::Connected;
        self.attempt_count = 0;
        self.current_delay = self.config.initial_delay;
        self.last_heartbeat_rx_ms = now_ms;
        self.reconnection_token = Some(token);
    }

    pub fn record_heartbeat(&mut self, now_ms: u64) {
        self.last_heartbeat_rx_ms = now_ms;
    }

    pub fn check_heartbeat_timeout(&mut self, now_ms: u64) -> bool {
        if self.state != ConnectionState::Connected {
            return false;
        }

        let timeout_ms = self.config.heartbeat_timeout.as_millis() as u64;
        if now_ms.saturating_sub(self.last_heartbeat_rx_ms) > timeout_ms {
            self.state = ConnectionState::Reconnecting;
            true
        } else {
            false
        }
    }

    pub fn next_reconnect_delay(&mut self) -> Option<Duration> {
        if self.attempt_count >= self.config.max_attempts {
            self.state = ConnectionState::Failed;
            return None;
        }

        self.attempt_count += 1;
        self.state = ConnectionState::Reconnecting;

        let delay = self.current_delay;
        let next_raw = delay.as_secs_f64() * self.config.backoff_factor;
        let next_duration = Duration::from_secs_f64(next_raw).min(self.config.max_delay);
        self.current_delay = next_duration;

        Some(delay)
    }

    pub fn recover_connection(&mut self, now_ms: u64) {
        self.state = ConnectionState::Connected;
        self.attempt_count = 0;
        self.current_delay = self.config.initial_delay;
        self.last_heartbeat_rx_ms = now_ms;
    }
}
