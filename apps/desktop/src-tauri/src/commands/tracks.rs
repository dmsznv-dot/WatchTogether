use media_engine::track::MediaTrack;

use crate::error::AppError;
use crate::state::AppState;

/// Gets all available audio tracks.
pub async fn cmd_player_get_audio_tracks(state: &AppState) -> Result<Vec<MediaTrack>, AppError> {
    let tracks = state.player.audio_tracks()?;
    Ok(tracks)
}

pub async fn player_get_audio_tracks(state: &AppState) -> Result<Vec<MediaTrack>, AppError> {
    cmd_player_get_audio_tracks(state).await
}

/// Selects active audio track by track ID.
pub async fn cmd_player_select_audio(state: &AppState, track_id: i64) -> Result<(), AppError> {
    state.player.select_audio(track_id)?;
    Ok(())
}

pub async fn player_select_audio(state: &AppState, track_id: i64) -> Result<(), AppError> {
    cmd_player_select_audio(state, track_id).await
}

/// Gets all available subtitle tracks.
pub async fn cmd_player_get_subtitle_tracks(state: &AppState) -> Result<Vec<MediaTrack>, AppError> {
    let subs = state.player.subtitle_tracks()?;
    Ok(subs)
}

pub async fn player_get_subtitle_tracks(state: &AppState) -> Result<Vec<MediaTrack>, AppError> {
    cmd_player_get_subtitle_tracks(state).await
}

/// Selects active subtitle track by track ID (-1 to disable).
pub async fn cmd_player_select_subtitle(state: &AppState, track_id: i64) -> Result<(), AppError> {
    state.player.select_subtitle(track_id)?;
    Ok(())
}

pub async fn player_select_subtitle(state: &AppState, track_id: i64) -> Result<(), AppError> {
    cmd_player_select_subtitle(state, track_id).await
}

/// Loads an external subtitle file (.srt, .ass) locally for this participant.
pub async fn cmd_player_load_external_subtitle(
    state: &AppState,
    path: String,
) -> Result<MediaTrack, AppError> {
    let track_id = state.player.add_external_subtitle(&path, None, None)?;
    let all_subs = state.player.subtitle_tracks()?;
    let track = all_subs
        .into_iter()
        .find(|t| t.id == track_id)
        .ok_or(AppError::Internal("Failed to find added subtitle track".into()))?;
    Ok(track)
}

pub async fn cmd_player_add_subtitle(state: &AppState, path: String) -> Result<(), AppError> {
    cmd_player_load_external_subtitle(state, path).await.map(|_| ())
}

pub async fn player_add_subtitle(state: &AppState, path: String) -> Result<(), AppError> {
    cmd_player_add_subtitle(state, path).await
}

/// Helper returning all tracks (audio + subtitle).
pub async fn player_get_tracks(state: &AppState) -> Result<Vec<MediaTrack>, AppError> {
    let mut tracks = state.player.audio_tracks()?;
    tracks.extend(state.player.subtitle_tracks()?);
    Ok(tracks)
}
