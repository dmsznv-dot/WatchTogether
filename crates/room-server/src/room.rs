use crate::error::RoomServerError;
use crate::participant::ParticipantRegistry;
use crate::permissions::PermissionPolicy;
use media_stream::MediaServerHandle;
use rand::Rng;
use shared_protocol::messages::ServerMessage;
use shared_protocol::models::{
    Participant, ParticipantId, PermissionMode, PlaybackAction, PlaybackState, PlaybackStatus,
    RoomCode, RoomId, RoomStateSnapshot, RoomToken, VideoMetadata,
};
use subtle::ConstantTimeEq;
use sync_engine::project_target_position;
use tokio::sync::broadcast;
use tokio::sync::mpsc::UnboundedSender;

pub const CROCKFORD_CHARSET: &[u8] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
pub const ALLOWED_PLAYBACK_SPEEDS: [f32; 6] = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0];

pub fn generate_room_code() -> RoomCode {
    let mut rng = rand::thread_rng();
    let code: String = (0..6)
        .map(|_| {
            let idx = rng.gen_range(0..CROCKFORD_CHARSET.len());
            CROCKFORD_CHARSET[idx] as char
        })
        .collect();
    RoomCode::new(code)
}

pub fn generate_room_token() -> RoomToken {
    let mut rng = rand::thread_rng();
    let random_bytes: [u8; 32] = rng.gen();
    let mut hex_token = String::with_capacity(64);
    for b in random_bytes {
        hex_token.push_str(&format!("{:02x}", b));
    }
    RoomToken::new(hex_token)
}

pub fn is_valid_playback_speed(speed: f32) -> bool {
    ALLOWED_PLAYBACK_SPEEDS.iter().any(|&s| (s - speed).abs() < 1e-4)
}

pub struct Room {
    pub room_id: RoomId,
    pub room_code: RoomCode,
    pub room_token: RoomToken,
    pub host_id: ParticipantId,
    pub video: VideoMetadata,
    pub playback: PlaybackState,
    pub participants: ParticipantRegistry,
    pub permissions: PermissionPolicy,
    pub is_closed: bool,
    pub broadcast_tx: broadcast::Sender<ServerMessage>,
    pub media_server_handle: Option<MediaServerHandle>,
}

impl Room {
    pub fn new(
        host_name: &str,
        video: VideoMetadata,
        permission_mode: PermissionMode,
        max_capacity: usize,
        now_ms: u64,
    ) -> (Self, ParticipantId) {
        let host_id = ParticipantId::new("part-host-001");
        let room_code = generate_room_code();
        let room_token = generate_room_token();
        let (broadcast_tx, _) = broadcast::channel(128);

        let playback = PlaybackState {
            revision: 1,
            status: PlaybackStatus::Paused,
            position_ms: 0,
            speed: 1.0,
            updated_at_server_ms: now_ms,
            duration_ms: video.duration_ms,
        };

        let participants = ParticipantRegistry::new(
            host_id.clone(),
            host_name.to_string(),
            max_capacity,
            now_ms,
        );
        let permissions = PermissionPolicy::new(host_id.clone(), permission_mode);

        let room = Self {
            room_id: RoomId::new(format!("room-{}", now_ms)),
            room_code,
            room_token,
            host_id: host_id.clone(),
            video,
            playback,
            participants,
            permissions,
            is_closed: false,
            broadcast_tx,
            media_server_handle: None,
        };

        (room, host_id)
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

    pub fn get_snapshot(&self, now_ms: u64) -> RoomStateSnapshot {
        let current_pos = self.current_projected_position(now_ms);
        let mut projected_playback = self.playback.clone();
        projected_playback.position_ms = current_pos;

        RoomStateSnapshot {
            room_id: self.room_id.clone(),
            room_code: self.room_code.clone(),
            host_id: self.host_id.clone(),
            permission_mode: self.permissions.mode(),
            participants: self.participants.list(),
            playback: projected_playback,
            video: self.video.clone(),
        }
    }

    pub fn validate_credentials(
        &self,
        code: &RoomCode,
        token: &RoomToken,
    ) -> Result<(), RoomServerError> {
        if self.is_closed {
            return Err(RoomServerError::RoomNotFound(self.room_id.to_string()));
        }
        if !code.is_valid() || &self.room_code != code {
            return Err(RoomServerError::InvalidRoomCode(code.to_string()));
        }
        if !token.is_valid()
            || self.room_token.as_str().len() != token.as_str().len()
            || !bool::from(
                self.room_token
                    .as_str()
                    .as_bytes()
                    .ct_eq(token.as_str().as_bytes()),
            )
        {
            return Err(RoomServerError::InvalidToken);
        }
        Ok(())
    }

    pub fn join(
        &mut self,
        code: &RoomCode,
        token: &RoomToken,
        display_name: &str,
        sender: Option<UnboundedSender<ServerMessage>>,
        now_ms: u64,
    ) -> Result<Participant, RoomServerError> {
        self.validate_credentials(code, token)?;
        let current_pos = self.current_projected_position(now_ms);
        let guest = self
            .participants
            .register_guest(display_name, current_pos, sender, now_ms)?;
        let _ = self
            .broadcast_tx
            .send(ServerMessage::ParticipantJoined(guest.clone()));
        Ok(guest)
    }

    pub fn leave(&mut self, participant_id: &ParticipantId) -> Result<(), RoomServerError> {
        if self.is_closed {
            return Err(RoomServerError::RoomNotFound(self.room_id.to_string()));
        }
        if participant_id == &self.host_id {
            self.is_closed = true;
            let _ = self
                .broadcast_tx
                .send(ServerMessage::RoomClosed {
                    reason: "host_closed_session".into(),
                });
            return Ok(());
        }
        self.participants.remove(participant_id)?;
        let _ = self
            .broadcast_tx
            .send(ServerMessage::ParticipantLeft {
                participant_id: participant_id.clone(),
                reason: "left".into(),
            });
        Ok(())
    }

    pub fn kick(
        &mut self,
        requester: &ParticipantId,
        target: &ParticipantId,
    ) -> Result<(), RoomServerError> {
        if self.is_closed {
            return Err(RoomServerError::RoomNotFound(self.room_id.to_string()));
        }
        self.permissions.can_kick(requester, target)?;
        self.participants.remove(target)?;
        let _ = self
            .broadcast_tx
            .send(ServerMessage::ParticipantKicked {
                participant_id: target.clone(),
                reason: "kicked_by_host".into(),
            });
        Ok(())
    }

    pub fn set_permission_mode(
        &mut self,
        requester: &ParticipantId,
        mode: PermissionMode,
    ) -> Result<(), RoomServerError> {
        if self.is_closed {
            return Err(RoomServerError::RoomNotFound(self.room_id.to_string()));
        }
        self.permissions.set_mode(requester, mode)?;
        Ok(())
    }

    pub fn apply_play(
        &mut self,
        requester: &ParticipantId,
        cmd_rev: u64,
        now_ms: u64,
    ) -> Result<PlaybackState, RoomServerError> {
        if self.is_closed {
            return Err(RoomServerError::RoomNotFound(self.room_id.to_string()));
        }
        self.permissions
            .can_control_playback(requester, self.participants.is_registered(requester))?;
        if cmd_rev <= self.playback.revision {
            return Err(RoomServerError::StaleRevision {
                command_revision: cmd_rev,
                current_revision: self.playback.revision,
            });
        }

        let current_pos = self.current_projected_position(now_ms);
        self.playback.revision = cmd_rev;
        self.playback.status = PlaybackStatus::Playing;
        self.playback.position_ms = current_pos;
        self.playback.updated_at_server_ms = now_ms;

        let _ = self
            .broadcast_tx
            .send(ServerMessage::PlaybackUpdated {
                revision: cmd_rev,
                status: PlaybackStatus::Playing,
                position_ms: current_pos,
                speed: self.playback.speed,
                updated_at_server_ms: now_ms,
                triggered_by: requester.clone(),
                action: PlaybackAction::Play.to_string(),
            });

        Ok(self.playback.clone())
    }

    pub fn apply_pause(
        &mut self,
        requester: &ParticipantId,
        cmd_rev: u64,
        position_ms: u64,
        now_ms: u64,
    ) -> Result<PlaybackState, RoomServerError> {
        if self.is_closed {
            return Err(RoomServerError::RoomNotFound(self.room_id.to_string()));
        }
        self.permissions
            .can_control_playback(requester, self.participants.is_registered(requester))?;
        if cmd_rev <= self.playback.revision {
            return Err(RoomServerError::StaleRevision {
                command_revision: cmd_rev,
                current_revision: self.playback.revision,
            });
        }

        let clamped_pos = position_ms.min(self.video.duration_ms);
        self.playback.revision = cmd_rev;
        self.playback.status = PlaybackStatus::Paused;
        self.playback.position_ms = clamped_pos;
        self.playback.updated_at_server_ms = now_ms;

        let _ = self
            .broadcast_tx
            .send(ServerMessage::PlaybackUpdated {
                revision: cmd_rev,
                status: PlaybackStatus::Paused,
                position_ms: clamped_pos,
                speed: self.playback.speed,
                updated_at_server_ms: now_ms,
                triggered_by: requester.clone(),
                action: PlaybackAction::Pause.to_string(),
            });

        Ok(self.playback.clone())
    }

    pub fn apply_seek(
        &mut self,
        requester: &ParticipantId,
        cmd_rev: u64,
        target_position_ms: u64,
        now_ms: u64,
    ) -> Result<PlaybackState, RoomServerError> {
        if self.is_closed {
            return Err(RoomServerError::RoomNotFound(self.room_id.to_string()));
        }
        self.permissions
            .can_control_playback(requester, self.participants.is_registered(requester))?;
        if cmd_rev <= self.playback.revision {
            return Err(RoomServerError::StaleRevision {
                command_revision: cmd_rev,
                current_revision: self.playback.revision,
            });
        }

        let clamped_pos = target_position_ms.min(self.video.duration_ms);
        self.playback.revision = cmd_rev;
        self.playback.position_ms = clamped_pos;
        self.playback.updated_at_server_ms = now_ms;

        let _ = self
            .broadcast_tx
            .send(ServerMessage::PlaybackUpdated {
                revision: cmd_rev,
                status: self.playback.status,
                position_ms: clamped_pos,
                speed: self.playback.speed,
                updated_at_server_ms: now_ms,
                triggered_by: requester.clone(),
                action: PlaybackAction::Seek.to_string(),
            });

        Ok(self.playback.clone())
    }

    pub fn apply_speed(
        &mut self,
        requester: &ParticipantId,
        cmd_rev: u64,
        speed: f32,
        now_ms: u64,
    ) -> Result<PlaybackState, RoomServerError> {
        if self.is_closed {
            return Err(RoomServerError::RoomNotFound(self.room_id.to_string()));
        }
        self.permissions
            .can_control_playback(requester, self.participants.is_registered(requester))?;
        if cmd_rev <= self.playback.revision {
            return Err(RoomServerError::StaleRevision {
                command_revision: cmd_rev,
                current_revision: self.playback.revision,
            });
        }
        if !is_valid_playback_speed(speed) {
            return Err(RoomServerError::InvalidMessage(format!(
                "Unsupported speed: {}",
                speed
            )));
        }

        let current_pos = self.current_projected_position(now_ms);
        self.playback.revision = cmd_rev;
        self.playback.position_ms = current_pos;
        self.playback.speed = speed;
        self.playback.updated_at_server_ms = now_ms;

        let _ = self
            .broadcast_tx
            .send(ServerMessage::PlaybackUpdated {
                revision: cmd_rev,
                status: self.playback.status,
                position_ms: current_pos,
                speed,
                updated_at_server_ms: now_ms,
                triggered_by: requester.clone(),
                action: PlaybackAction::Speed.to_string(),
            });

        Ok(self.playback.clone())
    }

    pub fn close(&mut self, requester: &ParticipantId) -> Result<(), RoomServerError> {
        self.permissions.can_close(requester)?;
        if self.is_closed {
            return Ok(());
        }
        self.is_closed = true;
        let _ = self
            .broadcast_tx
            .send(ServerMessage::RoomClosed {
                reason: "host_closed_session".into(),
            });
        Ok(())
    }

    pub async fn teardown(&mut self, requester: &ParticipantId) -> Result<(), RoomServerError> {
        self.close(requester)?;
        if let Some(media_handle) = self.media_server_handle.take() {
            media_handle.shutdown().await?;
        }
        Ok(())
    }
}
