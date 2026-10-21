//! Target playback position projection.

use shared_protocol::models::{PlaybackState, PlaybackStatus};

/// Projects the authoritative playback position to a target server timestamp.
/// 
/// Inputs:
/// - status: PlaybackStatus (Playing, Paused, Buffering)
/// - position_ms: base position at updatedAtServerMs
/// - speed: playback rate (e.g. 0.5, 1.0, 1.5, 2.0)
/// - updated_at_server_ms: timestamp when the status was recorded
/// - current_server_time_ms: estimated current server timestamp
/// - duration_ms: total media length (0 if unknown or live)
pub fn project_target_position(
    status: PlaybackStatus,
    position_ms: u64,
    speed: f32,
    updated_at_server_ms: u64,
    current_server_time_ms: u64,
    duration_ms: u64,
) -> u64 {
    match status {
        PlaybackStatus::Paused | PlaybackStatus::Buffering => {
            // Position does not advance while paused or buffering
            if duration_ms > 0 {
                position_ms.min(duration_ms)
            } else {
                position_ms
            }
        }
        PlaybackStatus::Playing => {
            let elapsed_ms = current_server_time_ms.saturating_sub(updated_at_server_ms);

            let safe_speed = if speed > 0.0 { speed as f64 } else { 1.0 };
            let projected_delta = ((elapsed_ms as f64) * safe_speed).round() as u64;
            let target_pos = position_ms.saturating_add(projected_delta);

            if duration_ms > 0 {
                target_pos.min(duration_ms)
            } else {
                target_pos
            }
        }
    }
}

/// Convenience extension method for PlaybackState
pub fn project_state_position(state: &PlaybackState, current_server_time_ms: u64) -> u64 {
    project_target_position(
        state.status,
        state.position_ms,
        state.speed,
        state.updated_at_server_ms,
        current_server_time_ms,
        state.duration_ms,
    )
}
