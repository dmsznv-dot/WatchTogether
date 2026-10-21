use std::sync::atomic::{AtomicBool, Ordering};
use crate::error::AppError;
use crate::state::AppState;

static IS_FULLSCREEN: AtomicBool = AtomicBool::new(false);

/// Toggles fullscreen window state and returns new fullscreen boolean.
pub async fn cmd_window_toggle_fullscreen(_state: &AppState) -> Result<bool, AppError> {
    let prev = IS_FULLSCREEN.fetch_xor(true, Ordering::SeqCst);
    Ok(!prev)
}
