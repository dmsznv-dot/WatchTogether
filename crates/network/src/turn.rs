use crate::p2p::{IceCandidate, IceCandidateType};
use crate::stun::compute_ice_candidate_priority;

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

impl TurnConfig {
    pub fn validate_credentials(&self, username: &str, credential: &str) -> bool {
        self.username == username && self.credential == credential
    }

    pub fn create_relay_candidate(&self, ip: String, port: u16) -> IceCandidate {
        IceCandidate {
            candidate_type: IceCandidateType::Relay,
            ip,
            port,
            protocol: "udp".into(),
            priority: compute_ice_candidate_priority(&IceCandidateType::Relay),
        }
    }

    pub fn fallback_candidate(&self, stun_responsive: bool) -> IceCandidateType {
        if stun_responsive {
            IceCandidateType::ServerReflexive
        } else {
            IceCandidateType::Relay
        }
    }
}
