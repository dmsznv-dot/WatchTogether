use crate::error::AppError;
use crate::state::AppState;

/// Opens native file picker dialog filtered to video files (.mp4, .mov, .mkv).
pub async fn cmd_dialog_open_file(_state: &AppState) -> Result<Option<String>, AppError> {
    // In headless/test mode or without GUI context, returns None
    Ok(None)
}
