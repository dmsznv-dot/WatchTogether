use media_engine::headless::is_valid_speed;
use media_engine::state::PlayerState;
use shared_protocol::messages::ClientMessage;
use shared_protocol::models::PlaybackState;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::AppError;
use crate::state::{AppState, SessionState};

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Opens a video file or stream URL.
pub async fn cmd_player_open_file(state: &AppState, path: String) -> Result<PlayerState, AppError> {
    if state.live_room.lock().await.is_some() {
        return Err(AppError::Internal(
            "Leave the room before changing the movie".into(),
        ));
    }
    state.player.open_file(&path)?;
    let mut p_state = state.player.get_state()?;
    if p_state.current_file.is_none() {
        p_state.current_file = Some(path);
    }
    Ok(p_state)
}

pub async fn player_open_file(state: &AppState, path: String) -> Result<PlayerState, AppError> {
    cmd_player_open_file(state, path).await
}

/// Plays the media stream. Synchronizes with active room if present.
pub async fn cmd_player_play(state: &AppState) -> Result<(), AppError> {
    if let Some(session) = state.live_room.lock().await.clone() {
        return session
            .command(ClientMessage::PlaybackPlay {
                revision: 0,
                client_timestamp_ms: current_time_ms(),
            })
            .await
            .map_err(AppError::Internal);
    }

    let now_ms = current_time_ms();
    let mut session_guard = state.session.write().await;
    match &mut *session_guard {
        SessionState::Host(host) => {
            let room_arc = host
                .room_manager
                .get_room_by_id(&host.room_id)
                .await
                .ok_or(AppError::NoActiveSession)?;
            let mut room = room_arc.lock().await;
            let next_rev = room.playback.revision + 1;
            room.apply_play(&host.host_id, next_rev, now_ms)?;
            state.player.play()?;
        }
        SessionState::Guest(guest) => {
            if let Some(sender) = guest.ws_sender.as_ref() {
                sender
                    .send(ClientMessage::PlaybackPlay {
                        revision: 1,
                        client_timestamp_ms: now_ms,
                    })
                    .map_err(|e| AppError::Internal(e.to_string()))?;
            }
            state.player.play()?;
        }
        SessionState::Solo => {
            state.player.play()?;
        }
    }
    Ok(())
}

pub async fn player_play(state: &AppState) -> Result<(), AppError> {
    cmd_player_play(state).await
}

/// Pauses the media stream. Synchronizes with active room if present.
pub async fn cmd_player_pause(state: &AppState) -> Result<(), AppError> {
    if let Some(session) = state.live_room.lock().await.clone() {
        return session
            .command(ClientMessage::PlaybackPause {
                revision: 0,
                position_ms: state.player.get_state()?.position_ms,
                client_timestamp_ms: current_time_ms(),
            })
            .await
            .map_err(AppError::Internal);
    }

    let now_ms = current_time_ms();
    let cur_pos = state.player.get_state()?.position_ms;
    let mut session_guard = state.session.write().await;
    match &mut *session_guard {
        SessionState::Host(host) => {
            let room_arc = host
                .room_manager
                .get_room_by_id(&host.room_id)
                .await
                .ok_or(AppError::NoActiveSession)?;
            let mut room = room_arc.lock().await;
            let next_rev = room.playback.revision + 1;
            room.apply_pause(&host.host_id, next_rev, cur_pos, now_ms)?;
            state.player.pause()?;
        }
        SessionState::Guest(guest) => {
            if let Some(sender) = guest.ws_sender.as_ref() {
                sender
                    .send(ClientMessage::PlaybackPause {
                        revision: 1,
                        position_ms: cur_pos,
                        client_timestamp_ms: now_ms,
                    })
                    .map_err(|e| AppError::Internal(e.to_string()))?;
            }
            state.player.pause()?;
        }
        SessionState::Solo => {
            state.player.pause()?;
        }
    }
    Ok(())
}

pub async fn player_pause(state: &AppState) -> Result<(), AppError> {
    cmd_player_pause(state).await
}

/// Seeks to a specific position (in seconds or ms).
pub async fn cmd_player_seek(
    state: &AppState,
    position_seconds: f64,
    exact: bool,
) -> Result<(), AppError> {
    if let Some(session) = state.live_room.lock().await.clone() {
        return session
            .command(ClientMessage::PlaybackSeek {
                revision: 0,
                target_position_ms: (position_seconds * 1000.0).max(0.0) as u64,
                client_timestamp_ms: current_time_ms(),
            })
            .await
            .map_err(AppError::Internal);
    }

    let position_ms = (position_seconds * 1000.0).max(0.0) as u64;
    let now_ms = current_time_ms();
    let mut session_guard = state.session.write().await;
    match &mut *session_guard {
        SessionState::Host(host) => {
            let room_arc = host
                .room_manager
                .get_room_by_id(&host.room_id)
                .await
                .ok_or(AppError::NoActiveSession)?;
            let mut room = room_arc.lock().await;
            let next_rev = room.playback.revision + 1;
            room.apply_seek(&host.host_id, next_rev, position_ms, now_ms)?;
            state.player.seek(position_ms, exact)?;
        }
        SessionState::Guest(guest) => {
            if let Some(sender) = guest.ws_sender.as_ref() {
                sender
                    .send(ClientMessage::PlaybackSeek {
                        revision: 1,
                        target_position_ms: position_ms,
                        client_timestamp_ms: now_ms,
                    })
                    .map_err(|e| AppError::Internal(e.to_string()))?;
            }
            state.player.seek(position_ms, exact)?;
        }
        SessionState::Solo => {
            state.player.seek(position_ms, exact)?;
        }
    }
    state.time_pos_throttler.lock().unwrap().force_emit();
    Ok(())
}

pub async fn player_seek(
    state: &AppState,
    position_seconds: f64,
    exact: bool,
) -> Result<(), AppError> {
    cmd_player_seek(state, position_seconds, exact).await
}

/// Sets playback speed multiplier (0.5x, 0.75x, 1x, 1.25x, 1.5x, 2x).
pub async fn cmd_player_set_speed(state: &AppState, speed: f64) -> Result<(), AppError> {
    if let Some(session) = state.live_room.lock().await.clone() {
        return session
            .command(ClientMessage::PlaybackSpeed {
                revision: 0,
                speed: speed as f32,
                client_timestamp_ms: current_time_ms(),
            })
            .await
            .map_err(AppError::Internal);
    }

    let speed_f32 = speed as f32;
    if !is_valid_speed(speed_f32) {
        return Err(AppError::InvalidSpeed(speed_f32));
    }

    let now_ms = current_time_ms();
    let mut session_guard = state.session.write().await;
    match &mut *session_guard {
        SessionState::Host(host) => {
            let room_arc = host
                .room_manager
                .get_room_by_id(&host.room_id)
                .await
                .ok_or(AppError::NoActiveSession)?;
            let mut room = room_arc.lock().await;
            let next_rev = room.playback.revision + 1;
            room.apply_speed(&host.host_id, next_rev, speed_f32, now_ms)?;
            state.player.set_speed(speed_f32)?;
        }
        SessionState::Guest(guest) => {
            if let Some(sender) = guest.ws_sender.as_ref() {
                sender
                    .send(ClientMessage::PlaybackSpeed {
                        revision: 1,
                        speed: speed_f32,
                        client_timestamp_ms: now_ms,
                    })
                    .map_err(|e| AppError::Internal(e.to_string()))?;
            }
            state.player.set_speed(speed_f32)?;
        }
        SessionState::Solo => {
            state.player.set_speed(speed_f32)?;
        }
    }
    Ok(())
}

pub async fn player_set_speed(state: &AppState, speed: f64) -> Result<(), AppError> {
    cmd_player_set_speed(state, speed).await
}

/// Sets local volume (0.0 to 1.0). Volume is strictly local and never synchronized.
pub async fn cmd_player_set_volume(state: &AppState, volume: f64) -> Result<(), AppError> {
    state.player.set_volume(volume)?;
    Ok(())
}

pub async fn player_set_volume(state: &AppState, volume: f64) -> Result<(), AppError> {
    cmd_player_set_volume(state, volume).await
}

/// Sets local mute state. Mute is strictly local and never synchronized.
pub async fn cmd_player_set_mute(state: &AppState, muted: bool) -> Result<(), AppError> {
    state.player.set_mute(muted)?;
    Ok(())
}

pub async fn player_set_mute(state: &AppState, muted: bool) -> Result<(), AppError> {
    cmd_player_set_mute(state, muted).await
}

/// Gets local player state.
pub async fn cmd_player_get_state(state: &AppState) -> Result<PlayerState, AppError> {
    let p_state = state.player.get_state()?;
    Ok(p_state)
}

pub async fn player_get_state(state: &AppState) -> Result<PlaybackState, AppError> {
    let pb_state = state.player.playback_state()?;
    Ok(pb_state)
}
