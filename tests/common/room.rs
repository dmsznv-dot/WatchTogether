use std::collections::HashMap;
use sha2::{Digest, Sha256};
use crate::protocol::*;
use crate::sync::project_target_position;

/// Generates a valid Crockford-style 6-character uppercase alphanumeric room code.
pub fn generate_room_code(seed: u64) -> RoomCode {
    const CHARSET: &[u8] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
    let mut code = String::with_capacity(6);
    let mut val = seed;
    for _ in 0..6 {
        let idx = (val % (CHARSET.len() as u64)) as usize;
        code.push(CHARSET[idx] as char);
        val = val.wrapping_mul(6364136223846793005).wrapping_add(1);
    }
    RoomCode::new(code)
}

/// Generates a valid 64-character hex CSPRNG room token.
pub fn generate_room_token(seed: &str) -> RoomToken {
    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    let digest = hasher.finalize();
    let mut s = String::with_capacity(64);
    for b in digest {
        s.push_str(&format!("{:02x}", b));
    }
    RoomToken::new(s)
}

/// Supported discrete playback speeds.
pub const ALLOWED_PLAYBACK_SPEEDS: [f32; 6] = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0];

pub fn is_valid_playback_speed(speed: f32) -> bool {
    ALLOWED_PLAYBACK_SPEEDS
        .iter()
        .any(|&s| (s - speed).abs() < 1e-4)
}

/// Mock room server model enforcing capacity, permissions, revision monotonicity, and lifecycle.
#[derive(Debug, Clone)]
pub struct MockRoomServer {
    pub room_id: RoomId,
    pub room_code: RoomCode,
    pub room_token: RoomToken,
    pub host_id: ParticipantId,
    pub permission_mode: PermissionMode,
    pub max_capacity: usize,
    pub participants: HashMap<ParticipantId, Participant>,
    pub playback: PlaybackState,
    pub video: VideoMetadata,
    pub is_closed: bool,
    pub participant_counter: u64,
}

impl MockRoomServer {
    pub fn new(
        host_name: &str,
        video: VideoMetadata,
        permission_mode: PermissionMode,
        now_ms: u64,
    ) -> (Self, ParticipantId) {
        let host_id = ParticipantId::new("part-host-001");
        let room_code = generate_room_code(now_ms);
        let room_token = generate_room_token(&format!("room-{}-{}", host_name, now_ms));

        let host = Participant {
            id: host_id.clone(),
            display_name: host_name.to_string(),
            role: ParticipantRole::Host,
            is_connected: true,
            buffered_position_ms: video.duration_ms,
            current_position_ms: 0,
            ping_ms: 0,
            joined_at_ms: now_ms,
        };

        let mut participants = HashMap::new();
        participants.insert(host_id.clone(), host);

        let playback = PlaybackState {
            revision: 1,
            status: PlaybackStatus::Paused,
            position_ms: 0,
            speed: 1.0,
            updated_at_server_ms: now_ms,
            duration_ms: video.duration_ms,
        };

        let server = Self {
            room_id: RoomId::new(format!("room-{}", now_ms)),
            room_code,
            room_token,
            host_id: host_id.clone(),
            permission_mode,
            max_capacity: 5,
            participants,
            playback,
            video,
            is_closed: false,
            participant_counter: 1,
        };

        (server, host_id)
    }

    pub fn join(
        &mut self,
        code: &RoomCode,
        token: &RoomToken,
        display_name: &str,
        now_ms: u64,
    ) -> Result<Participant, ErrorCode> {
        if self.is_closed {
            return Err(ErrorCode::RoomNotFound);
        }

        if &self.room_code != code {
            return Err(ErrorCode::InvalidRoomCode);
        }

        if &self.room_token != token {
            return Err(ErrorCode::InvalidToken);
        }

        let connected_count = self.participants.values().filter(|p| p.is_connected).count();
        if connected_count >= self.max_capacity {
            return Err(ErrorCode::RoomFull);
        }

        self.participant_counter += 1;
        let guest_id = ParticipantId::new(format!("part-guest-{:03}", self.participant_counter));

        // Late joiner starts at current projected position
        let current_pos = self.current_projected_position(now_ms);

        let guest = Participant {
            id: guest_id.clone(),
            display_name: display_name.to_string(),
            role: ParticipantRole::Guest,
            is_connected: true,
            buffered_position_ms: current_pos,
            current_position_ms: current_pos,
            ping_ms: 20,
            joined_at_ms: now_ms,
        };

        self.participants.insert(guest_id, guest.clone());
        Ok(guest)
    }

    pub fn leave(&mut self, participant_id: &ParticipantId) -> Result<(), ErrorCode> {
        if self.is_closed {
            return Err(ErrorCode::RoomNotFound);
        }

        if participant_id == &self.host_id {
            // When host quits, room closes for everyone
            self.is_closed = true;
            return Ok(());
        }

        if self.participants.remove(participant_id).is_some() {
            Ok(())
        } else {
            Err(ErrorCode::ParticipantNotFound)
        }
    }

    pub fn kick(
        &mut self,
        requester: &ParticipantId,
        target: &ParticipantId,
    ) -> Result<(), ErrorCode> {
        if self.is_closed {
            return Err(ErrorCode::RoomNotFound);
        }

        if requester != &self.host_id {
            return Err(ErrorCode::PermissionDenied);
        }

        if target == &self.host_id {
            return Err(ErrorCode::PermissionDenied);
        }

        if self.participants.remove(target).is_some() {
            Ok(())
        } else {
            Err(ErrorCode::ParticipantNotFound)
        }
    }

    pub fn set_permission_mode(
        &mut self,
        requester: &ParticipantId,
        mode: PermissionMode,
    ) -> Result<(), ErrorCode> {
        if self.is_closed {
            return Err(ErrorCode::RoomNotFound);
        }

        if requester != &self.host_id {
            return Err(ErrorCode::PermissionDenied);
        }

        self.permission_mode = mode;
        Ok(())
    }

    pub fn check_permission(&self, requester: &ParticipantId) -> Result<(), ErrorCode> {
        if requester == &self.host_id {
            return Ok(());
        }

        match self.permission_mode {
            PermissionMode::FreeControl => {
                if self.participants.contains_key(requester) {
                    Ok(())
                } else {
                    Err(ErrorCode::ParticipantNotFound)
                }
            }
            PermissionMode::HostOnly => Err(ErrorCode::PermissionDenied),
        }
    }

    pub fn current_projected_position(&self, now_ms: u64) -> u64 {
        project_target_position(
            self.playback.status,
            self.playback.position_ms,
            self.playback.speed,
            self.playback.updated_at_server_ms,
            now_ms,
            self.playback.duration_ms,
        )
    }

    pub fn apply_play(
        &mut self,
        requester: &ParticipantId,
        cmd_rev: u64,
        now_ms: u64,
    ) -> Result<PlaybackState, ErrorCode> {
        if self.is_closed {
            return Err(ErrorCode::RoomNotFound);
        }
        self.check_permission(requester)?;

        if cmd_rev <= self.playback.revision {
            return Err(ErrorCode::StaleRevision);
        }

        let current_pos = self.current_projected_position(now_ms);

        self.playback.revision = cmd_rev;
        self.playback.status = PlaybackStatus::Playing;
        self.playback.position_ms = current_pos;
        self.playback.updated_at_server_ms = now_ms;

        Ok(self.playback.clone())
    }

    pub fn apply_pause(
        &mut self,
        requester: &ParticipantId,
        cmd_rev: u64,
        position_ms: u64,
        now_ms: u64,
    ) -> Result<PlaybackState, ErrorCode> {
        if self.is_closed {
            return Err(ErrorCode::RoomNotFound);
        }
        self.check_permission(requester)?;

        if cmd_rev <= self.playback.revision {
            return Err(ErrorCode::StaleRevision);
        }

        let clamped_pos = position_ms.min(self.video.duration_ms);

        self.playback.revision = cmd_rev;
        self.playback.status = PlaybackStatus::Paused;
        self.playback.position_ms = clamped_pos;
        self.playback.updated_at_server_ms = now_ms;

        Ok(self.playback.clone())
    }

    pub fn apply_seek(
        &mut self,
        requester: &ParticipantId,
        cmd_rev: u64,
        target_position_ms: u64,
        now_ms: u64,
    ) -> Result<PlaybackState, ErrorCode> {
        if self.is_closed {
            return Err(ErrorCode::RoomNotFound);
        }
        self.check_permission(requester)?;

        if cmd_rev <= self.playback.revision {
            return Err(ErrorCode::StaleRevision);
        }

        let clamped_pos = target_position_ms.min(self.video.duration_ms);

        self.playback.revision = cmd_rev;
        self.playback.position_ms = clamped_pos;
        self.playback.updated_at_server_ms = now_ms;

        Ok(self.playback.clone())
    }

    pub fn apply_speed(
        &mut self,
        requester: &ParticipantId,
        cmd_rev: u64,
        speed: f32,
        now_ms: u64,
    ) -> Result<PlaybackState, ErrorCode> {
        if self.is_closed {
            return Err(ErrorCode::RoomNotFound);
        }
        self.check_permission(requester)?;

        if cmd_rev <= self.playback.revision {
            return Err(ErrorCode::StaleRevision);
        }

        if !is_valid_playback_speed(speed) {
            return Err(ErrorCode::InvalidMessage);
        }

        let current_pos = self.current_projected_position(now_ms);

        self.playback.revision = cmd_rev;
        self.playback.position_ms = current_pos;
        self.playback.speed = speed;
        self.playback.updated_at_server_ms = now_ms;

        Ok(self.playback.clone())
    }

    pub fn close(&mut self, requester: &ParticipantId) -> Result<(), ErrorCode> {
        if requester != &self.host_id {
            return Err(ErrorCode::PermissionDenied);
        }
        self.is_closed = true;
        Ok(())
    }

    pub fn get_snapshot(&self, now_ms: u64) -> RoomStateSnapshot {
        let current_pos = self.current_projected_position(now_ms);
        let mut projected_playback = self.playback.clone();
        projected_playback.position_ms = current_pos;

        let participants = self.participants.values().cloned().collect();

        RoomStateSnapshot {
            room_id: self.room_id.clone(),
            room_code: self.room_code.clone(),
            host_id: self.host_id.clone(),
            permission_mode: self.permission_mode,
            participants,
            playback: projected_playback,
            video: self.video.clone(),
        }
    }
}
