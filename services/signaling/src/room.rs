use crate::protocol::*;
use std::collections::HashMap;
use std::sync::Arc;
use subtle::ConstantTimeEq;
use tokio::sync::{mpsc, RwLock};

pub const MAX_ROOM_CAPACITY: usize = 5;

pub struct RoomSession {
    pub room_code: String,
    pub room_token: String,
    pub host_peer_id: String,
    pub peers: HashMap<String, mpsc::UnboundedSender<ServerSignal>>,
    pub max_capacity: usize,
    pub created_at_ms: u64,
}

#[derive(Clone, Default)]
pub struct RoomRegistry {
    sessions: Arc<RwLock<HashMap<String, RoomSession>>>,
}

impl RoomRegistry {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn is_valid_room_code(code: &str) -> bool {
        code.len() == 6
            && code
                .chars()
                .all(|c| matches!(c, '2'..='9' | 'A'..='H' | 'J'..='N' | 'P'..='Z'))
    }

    pub async fn register_or_join(
        &self,
        room_code: String,
        room_token: String,
        peer_id: String,
        role: PeerRole,
        tx: mpsc::UnboundedSender<ServerSignal>,
        now_ms: u64,
    ) -> Result<Vec<String>, SignalError> {
        if !Self::is_valid_room_code(&room_code) {
            return Err(SignalError::InvalidRoomCode(room_code));
        }

        let mut lock = self.sessions.write().await;

        match role {
            PeerRole::Host => {
                if room_token.len() < 32 {
                    return Err(SignalError::InvalidToken);
                }
                if let Some(existing) = lock.get(&room_code) {
                    if !existing.peers.is_empty() {
                        return Err(SignalError::InvalidRoomCode(
                            "Code already in use by active host".into(),
                        ));
                    }
                }
                let mut peers = HashMap::new();
                peers.insert(peer_id.clone(), tx);
                lock.insert(
                    room_code.clone(),
                    RoomSession {
                        room_code,
                        room_token,
                        host_peer_id: peer_id,
                        peers,
                        max_capacity: MAX_ROOM_CAPACITY,
                        created_at_ms: now_ms,
                    },
                );
                Ok(Vec::new())
            }
            PeerRole::Guest => {
                let session = lock
                    .get_mut(&room_code)
                    .ok_or_else(|| SignalError::RoomNotFound(room_code.clone()))?;

                if room_token.len() < 32
                    || session.room_token.len() != room_token.len()
                    || !bool::from(
                        session
                            .room_token
                            .as_bytes()
                            .ct_eq(room_token.as_bytes()),
                    )
                {
                    return Err(SignalError::InvalidToken);
                }

                if session.peers.len() >= session.max_capacity {
                    return Err(SignalError::RoomFull);
                }

                let existing_peer_ids: Vec<String> = session.peers.keys().cloned().collect();

                // Broadcast signal.peer_joined to existing peers
                let join_event = ServerSignal::PeerJoined {
                    peer_id: peer_id.clone(),
                    role: PeerRole::Guest,
                };
                for peer_tx in session.peers.values() {
                    let _ = peer_tx.send(join_event.clone());
                }

                session.peers.insert(peer_id, tx);
                Ok(existing_peer_ids)
            }
        }
    }

    pub async fn forward_signal(
        &self,
        room_code: &str,
        sender_id: &str,
        target_id: &str,
        signal: ServerSignal,
    ) -> Result<(), SignalError> {
        let lock = self.sessions.read().await;
        let session = lock
            .get(room_code)
            .ok_or_else(|| SignalError::RoomNotFound(room_code.to_string()))?;

        if !session.peers.contains_key(sender_id) {
            return Err(SignalError::ParticipantNotFound(sender_id.to_string()));
        }

        if let Some(target_tx) = session.peers.get(target_id) {
            target_tx
                .send(signal)
                .map_err(|_| SignalError::Internal("Target channel dropped".into()))?;
            Ok(())
        } else {
            Err(SignalError::ParticipantNotFound(target_id.to_string()))
        }
    }

    pub async fn remove_peer(&self, room_code: &str, peer_id: &str) {
        let mut lock = self.sessions.write().await;
        if let Some(mut session) = lock.remove(room_code) {
            session.peers.remove(peer_id);
            if peer_id == session.host_peer_id {
                // Host left: close room and evict all guests
                let close_msg = ServerSignal::RoomClosed {
                    reason: "host_disconnected".into(),
                };
                for peer_tx in session.peers.values() {
                    let _ = peer_tx.send(close_msg.clone());
                }
            } else {
                // Guest left: notify remaining peers
                let disc_msg = ServerSignal::PeerDisconnected {
                    peer_id: peer_id.to_string(),
                };
                let left_msg = ServerSignal::PeerLeft {
                    peer_id: peer_id.to_string(),
                    reason: "disconnected".into(),
                };
                for peer_tx in session.peers.values() {
                    let _ = peer_tx.send(disc_msg.clone());
                    let _ = peer_tx.send(left_msg.clone());
                }
                if !session.peers.is_empty() {
                    lock.insert(room_code.to_string(), session);
                }
            }
        }
    }

    pub async fn active_rooms_count(&self) -> usize {
        self.sessions.read().await.len()
    }

    pub async fn active_peers_count(&self) -> usize {
        self.sessions
            .read()
            .await
            .values()
            .map(|s| s.peers.len())
            .sum()
    }
}
