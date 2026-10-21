use shared_protocol::models::{PermissionMode, PlaybackStatus};
use std::time::Duration;
use tokio::time::sleep;

use watch_together_desktop::commands::dialog::*;
use watch_together_desktop::commands::player::*;
use watch_together_desktop::commands::room::*;
use watch_together_desktop::commands::tracks::*;
use watch_together_desktop::commands::window::*;
use watch_together_desktop::error::AppError;
use watch_together_desktop::events::MockEventSink;
use watch_together_desktop::state::{AppState, TimePosThrottler};

#[tokio::test]
async fn test_desktop_player_lifecycle_mock() {
    let state = AppState::new_mock();

    // 1. Open file
    player_open_file(&state, "/tmp/synthetic_movie.mp4".into())
        .await
        .unwrap();
    let initial_state = player_get_state(&state).await.unwrap();
    assert_eq!(initial_state.status, PlaybackStatus::Paused);
    assert_eq!(initial_state.position_ms, 0);

    // 2. Play
    player_play(&state).await.unwrap();
    let playing_state = player_get_state(&state).await.unwrap();
    assert_eq!(playing_state.status, PlaybackStatus::Playing);

    // 3. Seek (42.5 seconds = 42,500 ms)
    player_seek(&state, 42.5, true).await.unwrap();
    let seek_state = player_get_state(&state).await.unwrap();
    assert_eq!(seek_state.position_ms, 42_500);

    // 4. Speed adjustment (discrete 1.5x)
    player_set_speed(&state, 1.5).await.unwrap();
    let speed_state = player_get_state(&state).await.unwrap();
    assert_eq!(speed_state.speed, 1.5);

    // 5. Invalid speeds rejected
    assert!(player_set_speed(&state, 5.0).await.is_err());
    assert!(player_set_speed(&state, -1.0).await.is_err());

    // 6. Local volume and mute isolation
    player_set_volume(&state, 0.75).await.unwrap();
    player_set_mute(&state, true).await.unwrap();
    let local_state = cmd_player_get_state(&state).await.unwrap();
    assert_eq!(local_state.volume, 0.75);
    assert!(local_state.is_muted);

    // 7. Pause
    player_pause(&state).await.unwrap();
    let paused_state = player_get_state(&state).await.unwrap();
    assert_eq!(paused_state.status, PlaybackStatus::Paused);
}

#[tokio::test]
async fn test_desktop_room_creation_and_teardown_cascade() {
    let state = AppState::new_mock();
    player_open_file(&state, "/tmp/synthetic_movie.mp4".into())
        .await
        .unwrap();

    // Host creates room
    let (room_code, room_token) = room_create(&state).await.unwrap();
    assert_eq!(room_code.len(), 6);
    assert!(room_token.len() >= 32);

    // Verify room state is active
    let room_state = room_get_state(&state).await.unwrap();
    assert_eq!(room_state.permission_mode, PermissionMode::FreeControl);
    assert_eq!(room_state.participants.len(), 1);

    // Host toggles host-only
    room_set_permissions(&state, PermissionMode::HostOnly)
        .await
        .unwrap();
    assert_eq!(
        room_get_state(&state).await.unwrap().permission_mode,
        PermissionMode::HostOnly
    );

    // Host leaves -> room teardown cascades to shut down file server & reset state
    room_leave(&state).await.unwrap();
    assert!(room_get_state(&state).await.is_err());
}

#[tokio::test]
async fn test_desktop_room_code_normalization() {
    let state = AppState::new_mock();

    // Valid 32-char token
    let valid_token = "0123456789ABCDEF0123456789ABCDEF".to_string();

    // 1. Lowercase code "k7m2p9" should be normalized to uppercase "K7M2P9"
    let join_res = cmd_room_join(&state, "k7m2p9".into(), valid_token.clone(), None, None).await;
    assert!(join_res.is_ok());
    assert_eq!(join_res.unwrap().room_code, "K7M2P9");

    // Clean up
    room_leave(&state).await.unwrap();

    // 2. Whitespace padding "  k7m2p9  " should be trimmed and normalized
    let join_res2 =
        cmd_room_join(&state, "  k7m2p9  ".into(), valid_token.clone(), None, None).await;
    assert!(join_res2.is_ok());
    assert_eq!(join_res2.unwrap().room_code, "K7M2P9");

    room_leave(&state).await.unwrap();

    // 3. Invalid codes rejected (e.g. contains '0' or '1' or wrong length)
    assert!(matches!(
        cmd_room_join(&state, "K7M2P0".into(), valid_token.clone(), None, None).await,
        Err(AppError::InvalidRoomCode(_))
    ));
    assert!(matches!(
        cmd_room_join(&state, "TOO_LONG_CODE".into(), valid_token.clone(), None, None).await,
        Err(AppError::InvalidRoomCode(_))
    ));

    // 4. Invalid token (<32 chars) rejected
    assert!(matches!(
        cmd_room_join(&state, "K7M2P9".into(), "too_short".into(), None, None).await,
        Err(AppError::InvalidToken)
    ));
}

#[tokio::test]
async fn test_desktop_track_commands() {
    let state = AppState::new_mock();

    // Audio tracks
    let audio_tracks = cmd_player_get_audio_tracks(&state).await.unwrap();
    assert_eq!(audio_tracks.len(), 2);
    assert_eq!(audio_tracks[0].id, 1);
    assert!(audio_tracks[0].is_selected);

    // Switch audio track
    cmd_player_select_audio(&state, 2).await.unwrap();
    let updated_audio = cmd_player_get_audio_tracks(&state).await.unwrap();
    assert!(!updated_audio[0].is_selected);
    assert!(updated_audio[1].is_selected);

    // Subtitle tracks
    let sub_tracks = cmd_player_get_subtitle_tracks(&state).await.unwrap();
    assert_eq!(sub_tracks.len(), 1);

    // Select embedded subtitle
    cmd_player_select_subtitle(&state, 1).await.unwrap();
    assert!(cmd_player_get_subtitle_tracks(&state).await.unwrap()[0].is_selected);

    // Disable subtitles (-1)
    cmd_player_select_subtitle(&state, -1).await.unwrap();
    assert!(!cmd_player_get_subtitle_tracks(&state).await.unwrap()[0].is_selected);

    // Load external subtitle (.srt)
    let ext_track = cmd_player_load_external_subtitle(&state, "/subs/french.srt".into())
        .await
        .unwrap();
    assert!(ext_track.is_external);
    assert!(ext_track.is_selected);

    let all_tracks = player_get_tracks(&state).await.unwrap();
    assert_eq!(all_tracks.len(), 4); // 2 audio + 2 subtitle
}

#[tokio::test]
async fn test_desktop_guest_permission_enforcement() {
    let state = AppState::new_mock();
    let valid_token = "0123456789ABCDEF0123456789ABCDEF".to_string();

    // Join as guest
    room_join(&state, "K7M2P9".into(), valid_token).await.unwrap();

    // Guest cannot change permissions
    let perm_res = room_set_permissions(&state, PermissionMode::HostOnly).await;
    assert!(matches!(perm_res, Err(AppError::PermissionDenied(_))));

    // Guest cannot kick participants
    let kick_res = room_kick_participant(&state, "part-host-001".into()).await;
    assert!(matches!(kick_res, Err(AppError::PermissionDenied(_))));

    room_leave(&state).await.unwrap();
}

#[test]
fn test_desktop_time_pos_throttler() {
    let mut throttler = TimePosThrottler::new();

    // First emission allowed
    assert!(throttler.should_emit());

    // Immediate second emission (< 250ms) throttled
    assert!(!throttler.should_emit());

    // Force emit overrides
    throttler.force_emit();
    assert!(!throttler.should_emit());
}

#[tokio::test]
async fn test_desktop_event_sink_mock() {
    let event_sink = std::sync::Arc::new(MockEventSink::new());
    let player = std::sync::Arc::new(media_engine::HeadlessPlayer::new());
    let state = AppState::new(player, event_sink.clone());

    // Play generates event
    cmd_player_play(&state).await.unwrap();
    sleep(Duration::from_millis(50)).await;

    assert!(event_sink.has_event("player://state-change"));
    let last = event_sink.last_event("player://state-change").unwrap();
    assert_eq!(last["is_playing"], true);
}

#[tokio::test]
async fn test_desktop_window_and_dialog_commands() {
    let state = AppState::new_mock();

    // Fullscreen toggle
    let fs1 = cmd_window_toggle_fullscreen(&state).await.unwrap();
    let fs2 = cmd_window_toggle_fullscreen(&state).await.unwrap();
    assert_ne!(fs1, fs2);

    // Dialog open file
    let file = cmd_dialog_open_file(&state).await.unwrap();
    assert_eq!(file, None);
}
