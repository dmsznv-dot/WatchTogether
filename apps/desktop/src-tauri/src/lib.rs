//! Watch Together Desktop Application Backend (Milestone 3).

pub mod commands;
pub mod error;
pub mod events;
pub mod state;

pub use error::AppError;
pub use events::{EventSink, MockEventSink, TauriEventSink};
pub use state::{AppState, SessionState};

/// Initializes desktop application runtime backend.
pub fn run() {
    tracing::info!("Watch Together Desktop Initializing...");
}
