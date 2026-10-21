//! NTP clock synchronization, monotonic revision tracking, and 3-tier drift correction for Watch Together.

pub mod clock;
pub mod coordinator;
pub mod drift;
pub mod projection;
pub mod revision;

pub use clock::{ClockEstimator, ClockSyncConfig, ClockSyncError, NtpMeasurement, NtpSample};
pub use coordinator::ClientSyncCoordinator;
pub use drift::{Correction, DriftEngine, DriftEngineConfig};
pub use projection::{project_state_position, project_target_position};
pub use revision::{RevisionError, RevisionTracker};
