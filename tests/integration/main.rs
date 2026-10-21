use reqwest::header::RANGE;
use reqwest::StatusCode;
use std::sync::Arc;
use tempfile::NamedTempFile;
use watch_together_tests::mock_media::*;
use watch_together_tests::network_mock::*;
use watch_together_tests::protocol::*;
use watch_together_tests::room::*;
use watch_together_tests::sync::*;
use watch_together_tests::test_server::*;

fn sample_video() -> VideoMetadata {
    VideoMetadata {
        filename: "test_combo.mkv".into(),
        file_size_bytes: 1_000_000_000,
        duration_ms: 7_200_000, // 2 hours
        mime_type: "video/x-matroska".into(),
        width: Some(3840),
        height: Some(2160),
    }
}

// ============================================================================
// TIER 3: CROSS-FEATURE COMBINATIONS (PAIRWISE COMBINATORIAL TESTING)
// ============================================================================

#[test]
fn test_tier3_combo_seek_while_buffering() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    // 1. Participant enters buffering status
    room.playback.status = PlaybackStatus::Buffering;
    assert_eq!(room.playback.status, PlaybackStatus::Buffering);

    // 2. Seek command arrives while buffering
    let updated = room.apply_seek(&host_id, 2, 450_000, 105_000).expect("Seek should succeed");
    assert_eq!(updated.position_ms, 450_000);
    assert_eq!(updated.revision, 2);

    // 3. Projected position should match target seek position (not advance)
    assert_eq!(room.current_projected_position(110_000), 450_000);
}

#[test]
fn test_tier3_combo_speed_change_under_host_only_guest_vs_host() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::HostOnly, 100_000);
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();

    // Guest attempts speed change -> rejected
    let guest_res = room.apply_speed(&guest.id, 2, 1.5, 102_000);
    assert_eq!(guest_res, Err(ErrorCode::PermissionDenied));
    assert_eq!(room.playback.speed, 1.0);

    // Host executes speed change -> succeeds
    let host_res = room.apply_speed(&host_id, 2, 1.5, 103_000);
    assert!(host_res.is_ok());
    assert_eq!(room.playback.speed, 1.5);
}

#[tokio::test]
async fn test_tier3_combo_audio_track_switch_during_active_streaming() {
    let temp_file = NamedTempFile::new().unwrap();
    let file_bytes = create_synthetic_media_file(temp_file.path(), 128 * 1024).unwrap();
    let token = "a".repeat(64);

    let server = TestMediaServer::start(temp_file.path(), &token).await.unwrap();
    let mut player = MockPlayer::new_headless();
    player.open_file(&server.stream_url(), 60_000);
    player.play();

    // 1. Request chunk over HTTP
    let client = reqwest::Client::new();
    let resp = client.get(server.stream_url()).header(RANGE, "bytes=0-1023").send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);

    // 2. Concurrently switch audio track locally
    let switch_ok = player.select_audio(2);
    assert!(switch_ok);
    assert!(player.is_playing);

    // 3. Complete HTTP byte stream validation
    let bytes = resp.bytes().await.unwrap();
    assert_eq!(compute_sha256(&bytes), compute_range_sha256(&file_bytes, 0, 1023));

    server.shutdown().await;
}

#[test]
fn test_tier3_combo_late_joiner_connecting_while_host_seeking() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);

    // Host seeks to 1:30:00 (5_400_000 ms)
    room.apply_seek(&host_id, 2, 5_400_000, 101_000).unwrap();

    // Guest joins immediately at 101_050
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_050).unwrap();

    // Guest must start at the newly sought position
    assert_eq!(guest.current_position_ms, 5_400_000);
}

#[test]
fn test_tier3_combo_host_changes_permission_mode_while_guest_cmd_in_flight() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();

    // Host changes mode to HostOnly
    room.set_permission_mode(&host_id, PermissionMode::HostOnly).unwrap();

    // Guest in-flight command arrives after mode switch -> rejected
    let res = room.apply_play(&guest.id, 2, 102_000);
    assert_eq!(res, Err(ErrorCode::PermissionDenied));
}

#[test]
fn test_tier3_combo_connection_drop_during_high_speed_playback() {
    let mut sm = ReconnectionStateMachine::default_config();
    sm.set_connected("token".into(), 100_000);

    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    room.apply_play(&host_id, 2, 100_000).unwrap();
    room.apply_speed(&host_id, 3, 2.0, 100_000).unwrap();

    // Connection drops for 4 seconds
    assert!(sm.check_heartbeat_timeout(104_000));
    assert_eq!(sm.state, ConnectionState::Reconnecting);

    // Reconnection succeeds at 105_000
    sm.recover_connection(105_000);
    assert_eq!(sm.state, ConnectionState::Connected);

    // Timecode projected during disconnect advanced by 5s * 2.0x = 10s
    let projected = room.current_projected_position(105_000);
    assert_eq!(projected, 10_000);
}

#[test]
fn test_tier3_combo_pause_while_fast_forwarding_at_2x() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    room.apply_play(&host_id, 2, 100_000).unwrap();
    room.apply_speed(&host_id, 3, 2.0, 100_000).unwrap();

    // Pause after 10s (position reached 20_000)
    let updated = room.apply_pause(&host_id, 4, 20_000, 110_000).unwrap();
    assert_eq!(updated.status, PlaybackStatus::Paused);
    assert_eq!(updated.position_ms, 20_000);
    assert_eq!(updated.speed, 2.0);

    // In paused state, position must no longer advance
    assert_eq!(room.current_projected_position(120_000), 20_000);
}

#[tokio::test]
async fn test_tier3_combo_seek_to_eof_while_streaming() {
    let temp_file = NamedTempFile::new().unwrap();
    let file_size = 64 * 1024;
    let file_bytes = create_synthetic_media_file(temp_file.path(), file_size).unwrap();
    let token = "b".repeat(64);

    let server = TestMediaServer::start(temp_file.path(), &token).await.unwrap();
    let client = reqwest::Client::new();

    // Seek directly to last 128 bytes
    let start_offset = file_size - 128;
    let resp = client
        .get(server.stream_url())
        .header(RANGE, format!("bytes={}-", start_offset))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    let bytes = resp.bytes().await.unwrap();
    assert_eq!(bytes.len(), 128);
    assert_eq!(
        compute_sha256(&bytes),
        compute_range_sha256(&file_bytes, start_offset, file_size - 1)
    );

    server.shutdown().await;
}

#[test]
fn test_tier3_combo_mute_toggle_during_active_playback() {
    let mut player = MockPlayer::new_headless();
    player.open_file("/path/to/vid.mp4", 100_000);
    player.play();
    player.seek(20_000);

    player.set_mute(true);
    assert!(player.is_muted);
    assert!(player.is_playing);
    assert_eq!(player.position_ms, 20_000);

    player.set_mute(false);
    assert!(!player.is_muted);
    assert!(player.is_playing);
}

#[test]
fn test_tier3_combo_external_subtitle_addition_during_active_playback() {
    let mut player = MockPlayer::new_headless();
    player.open_file("/path/to/vid.mp4", 100_000);
    player.play();

    let sub_id = player.add_external_subtitle("/subs/commentary.srt", "Commentary", "eng");
    assert!(player.is_playing);
    assert!(player.subtitle_tracks.iter().find(|t| t.id == sub_id).unwrap().is_selected);
}

#[test]
fn test_tier3_combo_rapid_consecutive_seeks_in_free_control() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();

    // Alternating seeks between host and guest
    room.apply_seek(&host_id, 2, 10_000, 102_000).unwrap();
    room.apply_seek(&guest.id, 3, 20_000, 103_000).unwrap();
    room.apply_seek(&host_id, 4, 30_000, 104_000).unwrap();
    let final_state = room.apply_seek(&guest.id, 5, 40_000, 105_000).unwrap();

    assert_eq!(final_state.position_ms, 40_000);
    assert_eq!(final_state.revision, 5);
}

#[test]
fn test_tier3_combo_capacity_limit_reached_while_late_joiner_connecting() {
    let (mut room, _) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    // Fill to 5
    for name in &["Bob", "Charlie", "Diana", "Evan"] {
        room.join(&room.room_code.clone(), &room.room_token.clone(), name, 101_000).unwrap();
    }

    // 6th late joiner arrives
    let late_res = room.join(&room.room_code.clone(), &room.room_token.clone(), "Frank", 102_000);
    assert_eq!(late_res, Err(ErrorCode::RoomFull));
}

#[test]
fn test_tier3_combo_host_kicks_guest_with_inflight_seek() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();

    // Host kicks Bob
    room.kick(&host_id, &guest.id).unwrap();

    // Bob's in-flight seek arrives -> rejected
    let res = room.apply_seek(&guest.id, 2, 500_000, 102_000);
    assert_eq!(res, Err(ErrorCode::ParticipantNotFound));
}

#[test]
fn test_tier3_combo_drift_correction_hard_seek_while_buffering() {
    let mut engine = DriftEngine::default_config();

    // Client reported buffering at 10_000ms while server is at 50_000ms
    let c = engine.evaluate(10_000, 50_000, 1.0);
    assert_eq!(
        c,
        Correction::HardSeek {
            target_position_ms: 50_000,
            drift_ms: -40_000,
        }
    );
}

#[test]
fn test_tier3_combo_speed_adjust_active_while_nominal_speed_changes() {
    let mut engine = DriftEngine::default_config();

    // Trigger speed adjustment at 200ms lag with nominal 1.0x -> 1.04x
    let c1 = engine.evaluate(800, 1000, 1.0);
    if let Correction::AdjustSpeed { speed, .. } = c1 {
        assert!((speed - 1.04).abs() < 1e-4);
    } else {
        panic!("Expected AdjustSpeed");
    }

    // User changes nominal speed to 1.5x while drift engine still adjusting
    let c2 = engine.evaluate(850, 1000, 1.5);
    if let Correction::AdjustSpeed { speed, .. } = c2 {
        // 1.5 * 1.04 = 1.56
        assert!((speed - 1.56).abs() < 1e-4);
    } else {
        panic!("Expected AdjustSpeed with updated nominal speed");
    }
}

#[test]
fn test_tier3_combo_aspect_ratio_resize_while_fullscreen() {
    let video = VideoDimensions::new(1920, 1080);
    let is_fullscreen = true;

    // Display resolution changes during fullscreen (e.g. external monitor plugged in: 3840x2160)
    let (fit_w, fit_h) = video.fit_in_viewport(3840, 2160);
    assert_eq!(fit_w, 3840);
    assert_eq!(fit_h, 2160);
    assert!(is_fullscreen);

    // Switch to ultrawide 3440x1440
    let (fit_uw_w, fit_uw_h) = video.fit_in_viewport(3440, 1440);
    assert_eq!(fit_uw_h, 1440);
    assert_eq!(fit_uw_w, 2560);
}

#[tokio::test]
async fn test_tier3_combo_multiple_participants_streaming_while_host_pauses() {
    let temp_file = NamedTempFile::new().unwrap();
    let file_bytes = Arc::new(create_synthetic_media_file(temp_file.path(), 256 * 1024).unwrap());
    let token = "c".repeat(64);

    let server = Arc::new(TestMediaServer::start(temp_file.path(), &token).await.unwrap());
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    room.apply_play(&host_id, 2, 100_000).unwrap();

    // Concurrently fetch different ranges for 3 simulated guests
    let mut tasks = Vec::new();
    for i in 0..3 {
        let stream_url = server.stream_url();
        let bytes_ref = Arc::clone(&file_bytes);
        let start = i * 4096;
        let end = start + 4095;

        tasks.push(tokio::spawn(async move {
            let client = reqwest::Client::new();
            let resp = client
                .get(&stream_url)
                .header(RANGE, format!("bytes={}-{}", start, end))
                .send()
                .await
                .unwrap();

            assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
            let b = resp.bytes().await.unwrap();
            assert_eq!(compute_sha256(&b), compute_range_sha256(&bytes_ref, start, end));
        }));
    }

    // Host pauses room state during active streaming
    let paused_state = room.apply_pause(&host_id, 3, 25_000, 102_000).unwrap();
    assert_eq!(paused_state.status, PlaybackStatus::Paused);

    for t in tasks {
        t.await.unwrap();
    }

    let server_handle = Arc::try_unwrap(server).unwrap_or_else(|_| panic!("Server unwrap"));
    server_handle.shutdown().await;
}

#[test]
fn test_tier3_combo_stale_command_arriving_during_rapid_seek_burst() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);

    // Host seeks: rev 2, 3, 4
    room.apply_seek(&host_id, 2, 10_000, 101_000).unwrap();
    room.apply_seek(&host_id, 3, 20_000, 102_000).unwrap();
    room.apply_seek(&host_id, 4, 30_000, 103_000).unwrap();

    // Stale command rev 2 arrives -> rejected
    let res = room.apply_seek(&host_id, 2, 5_000, 104_000);
    assert_eq!(res, Err(ErrorCode::StaleRevision));
    assert_eq!(room.playback.position_ms, 30_000);
    assert_eq!(room.playback.revision, 4);
}

#[test]
fn test_tier3_combo_speed_change_during_backward_seek() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    room.apply_seek(&host_id, 2, 60_000, 101_000).unwrap();
    room.apply_speed(&host_id, 3, 0.75, 102_000).unwrap();
    let updated = room.apply_seek(&host_id, 4, 15_000, 103_000).unwrap();

    assert_eq!(updated.position_ms, 15_000);
    assert_eq!(updated.speed, 0.75);
    assert_eq!(updated.revision, 4);
}

#[test]
fn test_tier3_combo_late_joiner_with_different_local_volume_and_subs() {
    let (mut room, _) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let _guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();

    // Guest has independent local volume and subtitles
    let mut guest_player = MockPlayer::new_headless();
    guest_player.set_volume(0.3);
    guest_player.add_external_subtitle("/subs/bob.srt", "Bob Subs", "eng");

    assert_eq!(guest_player.volume, 0.3);
    assert_eq!(guest_player.subtitle_tracks.len(), 2);

    // Host room state remains unpolluted
    let snapshot = room.get_snapshot(101_000);
    assert_eq!(snapshot.playback.speed, 1.0);
}

#[test]
fn test_tier3_combo_room_closed_during_heartbeat_loss() {
    let mut sm = ReconnectionStateMachine::default_config();
    sm.set_connected("token".into(), 100_000);

    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);

    // Heartbeat lost -> Reconnecting
    sm.check_heartbeat_timeout(104_000);
    assert_eq!(sm.state, ConnectionState::Reconnecting);

    // Host explicitly closes the room
    room.close(&host_id).unwrap();

    // Client reconnection attempts must receive RoomNotFound
    let res = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 105_000);
    assert_eq!(res, Err(ErrorCode::RoomNotFound));
}

#[test]
fn test_tier3_combo_simultaneous_play_and_pause_race() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();

    // Host sends Play with rev 2
    let s1 = room.apply_play(&host_id, 2, 102_000).unwrap();
    assert_eq!(s1.status, PlaybackStatus::Playing);

    // Guest concurrently sent Pause also with rev 2 -> rejected due to revision conflict
    let res = room.apply_pause(&guest.id, 2, 0, 102_001);
    assert_eq!(res, Err(ErrorCode::StaleRevision));
}

#[test]
fn test_tier3_combo_all_discrete_speeds_under_free_control() {
    let (mut room, _) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();

    for (rev, &s) in (2..).zip([0.5, 0.75, 1.0, 1.25, 1.5, 2.0].iter()) {
        let updated = room.apply_speed(&guest.id, rev, s, 100_000 + rev).unwrap();
        assert_eq!(updated.speed, s);
    }
}

#[test]
fn test_tier3_combo_consecutive_leave_and_rejoin_while_playing() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    room.apply_play(&host_id, 2, 100_000).unwrap();

    let g1 = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();
    room.leave(&g1.id).unwrap();

    // 20s later, Bob rejoins
    let g1_rejoin = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 120_000).unwrap();
    assert_eq!(g1_rejoin.current_position_ms, 20_000);
}

#[test]
fn test_tier3_combo_fullscreen_escape_during_active_seek() {
    let mut player = MockPlayer::new_headless();
    player.open_file("/path/to/movie.mp4", 100_000);
    let mut is_fs = true;

    handle_shortcut(&mut player, KeyAction::SeekRelative(15_000), &mut is_fs);
    handle_shortcut(&mut player, KeyAction::ExitFullscreen, &mut is_fs);

    assert_eq!(player.position_ms, 15_000);
    assert!(!is_fs);
}
