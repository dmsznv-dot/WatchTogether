//! Embedded media player engine abstraction and libmpv integration for Watch Together.

pub mod dimensions;
pub mod error;
pub mod events;
pub mod headless;
pub mod mpv;
pub mod shortcuts;
pub mod state;
pub mod track;
pub mod traits;

pub use dimensions::VideoDimensions;
pub use error::MediaEngineError;
pub use events::PlayerEvent;
pub use headless::HeadlessPlayer;
pub use mpv::MpvPlayer;
pub use shortcuts::{handle_shortcut, KeyAction};
pub use state::PlayerState;
pub use track::{MediaTrack, TrackType};
pub use traits::MediaPlayer;

use std::sync::Arc;

/// Creates an active media player instance.
/// Attempts to initialize `MpvPlayer` with hardware acceleration; if `libmpv`
/// is not installed on the host system, falls back to deterministic `HeadlessPlayer`.
pub fn create_player() -> Arc<dyn MediaPlayer> {
    match MpvPlayer::new() {
        Ok(player) => Arc::new(player),
        Err(err) => {
            tracing::info!(
                "Initializing HeadlessPlayer fallback (libmpv: {})",
                err
            );
            Arc::new(HeadlessPlayer::new())
        }
    }
}

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
