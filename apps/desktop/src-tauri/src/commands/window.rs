use std::sync::atomic::{AtomicBool, Ordering};
use crate::error::AppError;
use crate::state::AppState;

static IS_FULLSCREEN: AtomicBool = AtomicBool::new(false);

/// Toggles fullscreen window state and returns new fullscreen boolean.
pub async fn cmd_window_toggle_fullscreen(state: &AppState) -> Result<bool, AppError> {
    if let Ok(fs) = state.player.toggle_fullscreen() {
        IS_FULLSCREEN.store(fs, Ordering::SeqCst);
        return Ok(fs);
    }
    let prev = IS_FULLSCREEN.fetch_xor(true, Ordering::SeqCst);
    Ok(!prev)
}
