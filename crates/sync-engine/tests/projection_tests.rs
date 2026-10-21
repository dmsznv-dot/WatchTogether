use shared_protocol::models::{PlaybackState, PlaybackStatus};
use sync_engine::projection::{project_state_position, project_target_position};

#[test]
fn test_target_position_projection_playing() {
    // 1. Playing at 1.0x speed: 5 seconds elapsed -> +5000ms
    let pos = project_target_position(PlaybackStatus::Playing, 10_000, 1.0, 100_000, 105_000, 60_000);
    assert_eq!(pos, 15_000);

    // 2. Playing at 1.5x speed: 4 seconds elapsed -> +6000ms
    let pos2 = project_target_position(PlaybackStatus::Playing, 10_000, 1.5, 100_000, 104_000, 60_000);
    assert_eq!(pos2, 16_000);

    // 3. Playing at 0.5x speed: 4 seconds elapsed -> +2000ms
    let pos3 = project_target_position(PlaybackStatus::Playing, 10_000, 0.5, 100_000, 104_000, 60_000);
    assert_eq!(pos3, 12_000);
}

#[test]
fn test_target_position_projection_paused_and_buffering() {
    // Paused state: position does not advance
    let pos_paused = project_target_position(PlaybackStatus::Paused, 10_000, 1.0, 100_000, 120_000, 60_000);
    assert_eq!(pos_paused, 10_000);

    // Buffering state: position does not advance
    let pos_buffering = project_target_position(PlaybackStatus::Buffering, 10_000, 1.0, 100_000, 120_000, 60_000);
    assert_eq!(pos_buffering, 10_000);
}

#[test]
fn test_target_position_duration_clamping() {
    // Media duration is 60_000ms, calculated position would be 65_000ms -> clamped to 60_000ms
    let pos = project_target_position(PlaybackStatus::Playing, 55_000, 1.0, 100_000, 110_000, 60_000);
    assert_eq!(pos, 60_000);
}

#[test]
fn test_target_position_clock_noise_past() {
    // If current_server_time < updated_at_server_ms due to slight clock noise -> elapsed is 0, position unchanged
    let pos = project_target_position(PlaybackStatus::Playing, 20_000, 1.0, 100_000, 99_950, 60_000);
    assert_eq!(pos, 20_000);
}

#[test]
fn test_project_state_position_convenience() {
    let state = PlaybackState {
        revision: 1,
        status: PlaybackStatus::Playing,
        position_ms: 10_000,
        speed: 2.0,
        updated_at_server_ms: 1_000_000,
        duration_ms: 500_000,
    };

    let pos = project_state_position(&state, 1_002_000);
    assert_eq!(pos, 14_000); // 10_000 + 2000 * 2.0 = 14_000
}
