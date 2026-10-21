use crate::error::RoomServerError;
use shared_protocol::models::{ParticipantId, PermissionMode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionPolicy {
    host_id: ParticipantId,
    mode: PermissionMode,
}

impl PermissionPolicy {
    pub fn new(host_id: ParticipantId, mode: PermissionMode) -> Self {
        Self { host_id, mode }
    }

    pub fn mode(&self) -> PermissionMode {
        self.mode
    }

    pub fn set_mode(
        &mut self,
        requester: &ParticipantId,
        new_mode: PermissionMode,
    ) -> Result<(), RoomServerError> {
        if requester != &self.host_id {
            return Err(RoomServerError::PermissionDenied(
                "Only host can change permissions".into(),
            ));
        }
        self.mode = new_mode;
        Ok(())
    }

    pub fn can_control_playback(
        &self,
        requester: &ParticipantId,
        is_registered: bool,
    ) -> Result<(), RoomServerError> {
        if requester == &self.host_id {
            return Ok(());
        }
        if !is_registered {
            return Err(RoomServerError::ParticipantNotFound(requester.to_string()));
        }
        match self.mode {
            PermissionMode::FreeControl => Ok(()),
            PermissionMode::HostOnly => Err(RoomServerError::PermissionDenied(
                "Host-only control active; guest commands rejected".into(),
            )),
        }
    }

    pub fn can_kick(
        &self,
        requester: &ParticipantId,
        target: &ParticipantId,
    ) -> Result<(), RoomServerError> {
        if requester != &self.host_id {
            return Err(RoomServerError::PermissionDenied(
                "Only host can kick participants".into(),
            ));
        }
        if target == &self.host_id {
            return Err(RoomServerError::PermissionDenied(
                "Host cannot kick self".into(),
            ));
        }
        Ok(())
    }

    pub fn can_close(&self, requester: &ParticipantId) -> Result<(), RoomServerError> {
        if requester != &self.host_id {
            return Err(RoomServerError::PermissionDenied(
                "Only host can close the room".into(),
            ));
        }
        Ok(())
    }
}
