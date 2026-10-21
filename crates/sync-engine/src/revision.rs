//! Monotonic revision tracking and command gating.

use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RevisionError {
    #[error("Stale command revision: incoming revision {incoming} <= current revision {current}")]
    StaleCommand { incoming: u64, current: u64 },
    #[error("Stale state broadcast: incoming state revision {incoming} <= client revision {current}")]
    StaleStateBroadcast { incoming: u64, current: u64 },
    #[error("Revision counter overflow")]
    RevisionOverflow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionTracker {
    current: u64,
}

impl RevisionTracker {
    pub fn new(initial: u64) -> Self {
        Self { current: initial }
    }

    pub fn current(&self) -> u64 {
        self.current
    }

    /// Gate function: Rejects incoming commands where rev_cmd <= rev_current.
    /// Used by room server to guard state changes.
    pub fn gate_command(&self, cmd_rev: u64) -> Result<(), RevisionError> {
        if cmd_rev <= self.current {
            Err(RevisionError::StaleCommand {
                incoming: cmd_rev,
                current: self.current,
            })
        } else {
            Ok(())
        }
    }

    /// Accepts the command and increments the authoritative revision by 1.
    pub fn accept_command_and_advance(&mut self, cmd_rev: u64) -> Result<u64, RevisionError> {
        self.gate_command(cmd_rev)?;
        self.current = self.current.checked_add(1).ok_or(RevisionError::RevisionOverflow)?;
        Ok(self.current)
    }

    /// Client-side gate: verifies that incoming server broadcast is newer than current known state.
    pub fn apply_incoming_broadcast(&mut self, broadcast_rev: u64) -> Result<(), RevisionError> {
        if broadcast_rev <= self.current {
            Err(RevisionError::StaleStateBroadcast {
                incoming: broadcast_rev,
                current: self.current,
            })
        } else {
            self.current = broadcast_rev;
            Ok(())
        }
    }

    /// Server-side direct advance (e.g. for host-initiated play/pause/seek without prior client command).
    pub fn advance(&mut self) -> Result<u64, RevisionError> {
        self.current = self.current.checked_add(1).ok_or(RevisionError::RevisionOverflow)?;
        Ok(self.current)
    }
}
