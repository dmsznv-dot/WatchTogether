//! Synchronization coordinators for client and server.

use crate::clock::{ClockEstimator, ClockSyncError, NtpMeasurement, NtpSample};
use crate::drift::{Correction, DriftEngine};
use crate::projection::project_state_position;
use crate::revision::{RevisionError, RevisionTracker};
use shared_protocol::models::{PlaybackState, PlaybackStatus};

pub struct ClientSyncCoordinator {
    pub clock: ClockEstimator,
    pub revision: RevisionTracker,
    pub drift: DriftEngine,
    current_state: Option<PlaybackState>,
}

impl ClientSyncCoordinator {
    pub fn new(initial_rev: u64) -> Self {
        Self {
            clock: ClockEstimator::default_config(),
            revision: RevisionTracker::new(initial_rev),
            drift: DriftEngine::default_config(),
            current_state: None,
        }
    }

    pub fn handle_pong(&mut self, m: &NtpMeasurement) -> Result<NtpSample, ClockSyncError> {
        self.clock.record_measurement(m)
    }

    pub fn handle_state_broadcast(&mut self, state: PlaybackState) -> Result<(), RevisionError> {
        self.revision.apply_incoming_broadcast(state.revision)?;
        self.current_state = Some(state);
        self.drift.reset(); // Reset drift hysteresis on new authoritative state
        Ok(())
    }

    pub fn tick(
        &mut self,
        client_now_ms: u64,
        actual_player_position_ms: u64,
    ) -> Option<Correction> {
        let state = self.current_state.as_ref()?;
        let est_server_time = self.clock.estimate_server_time(client_now_ms)?;
        let expected_pos = project_state_position(state, est_server_time);

        match state.status {
            PlaybackStatus::Paused | PlaybackStatus::Buffering => {
                // If paused or buffering, speed adjustment cannot apply
                let drift_ms = (actual_player_position_ms as i64) - (expected_pos as i64);
                if drift_ms.unsigned_abs() > 500 {
                    Some(Correction::HardSeek {
                        target_position_ms: expected_pos,
                        drift_ms,
                    })
                } else {
                    Some(Correction::None)
                }
            }
            PlaybackStatus::Playing => {
                Some(self.drift.evaluate(actual_player_position_ms, expected_pos, state.speed))
            }
        }
    }

    pub fn current_state(&self) -> Option<&PlaybackState> {
        self.current_state.as_ref()
    }
}
