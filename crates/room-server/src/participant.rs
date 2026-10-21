use crate::error::RoomServerError;
use shared_protocol::messages::ServerMessage;
use shared_protocol::models::{Participant, ParticipantId, ParticipantRole};
use std::collections::HashMap;
use tokio::sync::mpsc::UnboundedSender;

pub struct ParticipantSession {
    pub participant: Participant,
    pub sender: Option<UnboundedSender<ServerMessage>>,
    pub last_seen_ms: u64,
}

pub struct ParticipantRegistry {
    participants: HashMap<ParticipantId, ParticipantSession>,
    host_id: ParticipantId,
    max_capacity: usize,
    participant_counter: u64,
}

impl ParticipantRegistry {
    pub const DEFAULT_MAX_CAPACITY: usize = 5;

    pub fn new(
        host_id: ParticipantId,
        host_name: String,
        max_capacity: usize,
        now_ms: u64,
    ) -> Self {
        let host = Participant {
            id: host_id.clone(),
            display_name: host_name,
            role: ParticipantRole::Host,
            is_connected: true,
            buffered_position_ms: 0,
            current_position_ms: 0,
            ping_ms: 0,
            joined_at_ms: now_ms,
        };

        let mut participants = HashMap::new();
        participants.insert(
            host_id.clone(),
            ParticipantSession {
                participant: host,
                sender: None,
                last_seen_ms: now_ms,
            },
        );

        Self {
            participants,
            host_id,
            max_capacity,
            participant_counter: 1,
        }
    }

    pub fn host_id(&self) -> &ParticipantId {
        &self.host_id
    }

    pub fn max_capacity(&self) -> usize {
        self.max_capacity
    }

    pub fn connected_count(&self) -> usize {
        self.participants
            .values()
            .filter(|s| s.participant.is_connected)
            .count()
    }

    pub fn is_full(&self) -> bool {
        self.connected_count() >= self.max_capacity
    }

    pub fn register_guest(
        &mut self,
        display_name: &str,
        current_pos_ms: u64,
        sender: Option<UnboundedSender<ServerMessage>>,
        now_ms: u64,
    ) -> Result<Participant, RoomServerError> {
        if self.is_full() {
            return Err(RoomServerError::RoomFull {
                capacity: self.max_capacity,
            });
        }

        self.participant_counter += 1;
        let guest_id = ParticipantId::new(format!("part-guest-{:03}", self.participant_counter));

        let guest = Participant {
            id: guest_id.clone(),
            display_name: display_name.to_string(),
            role: ParticipantRole::Guest,
            is_connected: true,
            buffered_position_ms: current_pos_ms,
            current_position_ms: current_pos_ms,
            ping_ms: 20,
            joined_at_ms: now_ms,
        };

        self.participants.insert(
            guest_id,
            ParticipantSession {
                participant: guest.clone(),
                sender,
                last_seen_ms: now_ms,
            },
        );

        Ok(guest)
    }

    pub fn remove(&mut self, id: &ParticipantId) -> Result<Participant, RoomServerError> {
        self.participants
            .remove(id)
            .map(|s| s.participant)
            .ok_or_else(|| RoomServerError::ParticipantNotFound(id.to_string()))
    }

    pub fn get(&self, id: &ParticipantId) -> Option<&Participant> {
        self.participants.get(id).map(|s| &s.participant)
    }

    pub fn get_mut(&mut self, id: &ParticipantId) -> Option<&mut ParticipantSession> {
        self.participants.get_mut(id)
    }

    pub fn is_registered(&self, id: &ParticipantId) -> bool {
        self.participants.contains_key(id)
    }

    pub fn list(&self) -> Vec<Participant> {
        let mut list: Vec<Participant> = self.participants.values().map(|s| s.participant.clone()).collect();
        // Keep host first, then guests in order of joined_at_ms
        list.sort_by(|a, b| {
            if a.id == self.host_id {
                std::cmp::Ordering::Less
            } else if b.id == self.host_id {
                std::cmp::Ordering::Greater
            } else {
                a.joined_at_ms.cmp(&b.joined_at_ms)
            }
        });
        list
    }

    pub fn update_status(
        &mut self,
        id: &ParticipantId,
        buffered: u64,
        current: u64,
        _is_buffering: bool,
        ping: u64,
        now_ms: u64,
    ) {
        if let Some(session) = self.participants.get_mut(id) {
            session.participant.buffered_position_ms = buffered;
            session.participant.current_position_ms = current;
            session.participant.ping_ms = ping;
            session.last_seen_ms = now_ms;
        }
    }

    pub fn set_sender(
        &mut self,
        id: &ParticipantId,
        sender: Option<UnboundedSender<ServerMessage>>,
    ) {
        if let Some(session) = self.participants.get_mut(id) {
            session.sender = sender;
        }
    }
}
