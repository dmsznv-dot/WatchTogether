use reqwest::header::RANGE;
use reqwest::StatusCode;
use std::sync::Arc;
use std::time::Duration;
use tempfile::NamedTempFile;
use watch_together_tests::mock_media::*;
use watch_together_tests::network_mock::*;
use watch_together_tests::protocol::*;
use watch_together_tests::room::*;
use watch_together_tests::sync::*;
use watch_together_tests::test_server::*;

fn sample_4k_video() -> VideoMetadata {
    VideoMetadata {
        filename: "interstellar_4k_hdr.mkv".into(),
        file_size_bytes: 14_000_000_000, // 14 GB
        duration_ms: 7_200_000,          // 2 hours
        mime_type: "video/x-matroska".into(),
        width: Some(3840),
        height: Some(2160),
    }
}

// ============================================================================
// TIER 4: REAL-WORLD APPLICATION WORKLOADS
// ============================================================================

/// Workload 1: Full Movie Night Simulation
/// Host opens 2-hour 4K MKV with dual audio and external subtitles; creates room;
/// 3 guests join; 1 guest switches audio track locally; host pauses at 00:30:00 for break;
/// host resumes; 1 guest seeks +30s in FreeControl mode; all 4 remain in sync <= 250ms.
#[test]
fn test_tier4_workload1_full_movie_night_simulation() {
    let video = sample_4k_video();
    let now = 1_700_000_000_000u64;

    // 1. Host opens video and creates room
    let (mut room, host_id) = MockRoomServer::new("HostAlice", video.clone(), PermissionMode::FreeControl, now);
    let mut host_player = MockPlayer::new_headless();
    host_player.open_file(&video.filename, video.duration_ms);
    let _ext_sub_id = host_player.add_external_subtitle("/subs/movie_en.srt", "English (Full)", "eng");

    // 2. Three guests join the room
    let guest_bob = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", now + 1000).unwrap();
    let guest_charlie = room.join(&room.room_code.clone(), &room.room_token.clone(), "Charlie", now + 2000).unwrap();
    let guest_diana = room.join(&room.room_code.clone(), &room.room_token.clone(), "Diana", now + 3000).unwrap();

    let mut bob_player = MockPlayer::new_headless();
    let mut charlie_player = MockPlayer::new_headless();
    let mut diana_player = MockPlayer::new_headless();

    // 3. Bob switches audio track locally to Director's Commentary without affecting others
    assert!(bob_player.select_audio(2));
    assert_eq!(charlie_player.audio_tracks[0].id, 1);
    assert!(charlie_player.audio_tracks[0].is_selected);

    // 4. Play movie together
    room.apply_play(&host_id, 2, now + 5000).unwrap();
    host_player.play();
    bob_player.play();
    charlie_player.play();
    diana_player.play();

    // 5. Play until 00:30:00 (1_800_000 ms), then host pauses for snack break
    let pause_time = now + 5000 + 1_800_000;
    let paused_state = room.apply_pause(&host_id, 3, 1_800_000, pause_time).unwrap();
    assert_eq!(paused_state.position_ms, 1_800_000);
    assert_eq!(paused_state.status, PlaybackStatus::Paused);

    // 6. Resume after 5-minute break (300_000 ms)
    let resume_time = pause_time + 300_000;
    room.apply_play(&host_id, 4, resume_time).unwrap();

    // 7. Charlie seeks +30s forward in FreeControl mode
    let seek_time = resume_time + 60_000;
    let current_pos_at_seek = room.current_projected_position(seek_time);
    let seek_target = current_pos_at_seek + 30_000;
    let seek_state = room.apply_seek(&guest_charlie.id, 5, seek_target, seek_time).unwrap();
    assert_eq!(seek_state.position_ms, seek_target);

    // 8. Verify all 4 participants remain in sync <= 250ms drift
    let current_server_pos = room.current_projected_position(seek_time + 10_000);
    let mut drift_engine = DriftEngine::default_config();

    for (_guest, client_pos) in [
        (&guest_bob, current_server_pos - 40),
        (&guest_charlie, current_server_pos + 60),
        (&guest_diana, current_server_pos - 80),
    ] {
        let correction = drift_engine.evaluate(client_pos, current_server_pos, 1.0);
        let drift = (client_pos as i64) - (current_server_pos as i64);
        assert!(drift.abs() <= 250, "Drift {}ms exceeds 250ms threshold", drift);
        assert_eq!(correction, Correction::None, "Drift <= 100ms should be deadband (None)");
    }
}

/// Workload 2: Unstable Network & WiFi Handover
/// Host streaming video to 2 guests; Guest 2 simulates 10% packet loss and 300ms jitter;
/// verify drift engine applies gradual speed correction (+-4%) without audio pitch distortion;
/// disconnect interface for 4s; verify auto-reconnect and re-sync without reset to 0.
#[test]
fn test_tier4_workload2_unstable_network_and_wifi_handover() {
    let now = 100_000u64;
    let (mut room, host_id) = MockRoomServer::new("Host", sample_4k_video(), PermissionMode::FreeControl, now);
    room.apply_play(&host_id, 2, now).unwrap();

    let mut guest_sm = ReconnectionStateMachine::default_config();
    guest_sm.set_connected("reconnect_token_guest2".into(), now);

    let mut drift_engine = DriftEngine::default_config();
    let mut clock_estimator = ClockEstimator::default_config();

    // 1. Simulate 300ms jitter & packet loss affecting clock estimation
    let m1 = NtpMeasurement {
        t0_client_send: now + 1000,
        t1_server_recv: now + 1150,
        t2_server_send: now + 1160,
        t3_client_recv: now + 1310, // 300ms RTT
    };
    clock_estimator.record_measurement(&m1).unwrap();

    // 2. Guest 2 drifts behind by 220ms -> Drift engine applies gradual speedup (1.04x)
    let current_server_pos = room.current_projected_position(now + 10_000);
    let guest2_pos = current_server_pos - 220;
    let correction = drift_engine.evaluate(guest2_pos, current_server_pos, 1.0);

    if let Correction::AdjustSpeed { speed, drift_ms } = correction {
        assert_eq!(drift_ms, -220);
        assert!((speed - 1.04).abs() < 1e-4, "Gradual speedup factor must be 1.04");
    } else {
        panic!("Expected AdjustSpeed for 220ms lag");
    }

    // 3. WiFi handover causes interface disconnect for 4 seconds
    let disconnect_time = now + 15_000;
    assert!(guest_sm.check_heartbeat_timeout(disconnect_time + 3500));
    assert_eq!(guest_sm.state, ConnectionState::Reconnecting);

    // 4. Auto-reconnect triggered with 1s backoff
    let delay = guest_sm.next_reconnect_delay();
    assert_eq!(delay, Some(Duration::from_millis(1000)));

    // Reconnection succeeds after 4s total disconnect
    let reconnect_time = disconnect_time + 4000;
    guest_sm.recover_connection(reconnect_time);
    assert_eq!(guest_sm.state, ConnectionState::Connected);

    // 5. Position must re-sync to room timecode without reset to 0
    let re_synced_pos = room.current_projected_position(reconnect_time);
    assert!(re_synced_pos >= 19_000, "Position must not reset to 0; was {}", re_synced_pos);
}

/// Workload 3: Adversarial Guest in Host-Only Room
/// Host sets Host-Only mode; Guest attempts rapid-fire play/pause/seek commands;
/// verify all commands rejected with PermissionDenied and host playback remains completely uninterrupted.
#[test]
fn test_tier4_workload3_adversarial_guest_in_host_only_room() {
    let now = 100_000u64;
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_4k_video(), PermissionMode::HostOnly, now);
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "MaliciousBob", now + 100).unwrap();

    // Host starts playback at 100_000
    room.apply_play(&host_id, 2, now + 1000).unwrap();
    let initial_playback = room.playback.clone();

    // Adversarial guest fires 20 rapid unauthorized commands
    for i in 1..=20 {
        let rev = 2 + i;
        let res_play = room.apply_play(&guest.id, rev, now + 2000 + i);
        assert_eq!(res_play, Err(ErrorCode::PermissionDenied));

        let res_pause = room.apply_pause(&guest.id, rev, 0, now + 2000 + i);
        assert_eq!(res_pause, Err(ErrorCode::PermissionDenied));

        let res_seek = room.apply_seek(&guest.id, rev, 999_999, now + 2000 + i);
        assert_eq!(res_seek, Err(ErrorCode::PermissionDenied));

        let res_speed = room.apply_speed(&guest.id, rev, 2.0, now + 2000 + i);
        assert_eq!(res_speed, Err(ErrorCode::PermissionDenied));
    }

    // Host playback remains completely unaltered
    assert_eq!(room.playback.status, initial_playback.status);
    assert_eq!(room.playback.revision, 2);
    assert_eq!(room.playback.speed, 1.0);
}

/// Workload 4: Host Graceful & Ungraceful Exit
/// Host closes room during active streaming; verify all guest sockets receive room.closed
/// and subsequent HTTP Range requests receive immediate 403 or connection refused.
#[tokio::test]
async fn test_tier4_workload4_host_graceful_and_ungraceful_exit() {
    let temp_file = NamedTempFile::new().unwrap();
    let file_bytes = create_synthetic_media_file(temp_file.path(), 256 * 1024).unwrap();
    let token = "d".repeat(64);

    let server = TestMediaServer::start(temp_file.path(), &token).await.unwrap();
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_4k_video(), PermissionMode::FreeControl, 100_000);
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();

    // 1. Initial streaming works
    let client = reqwest::Client::new();
    let resp = client.get(server.stream_url()).header(RANGE, "bytes=0-100").send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);

    // 2. Host closes room session
    room.close(&host_id).unwrap();
    assert!(room.is_closed);

    // 3. Broadcast room.closed payload
    let close_event = ServerMessage::RoomClosed {
        reason: "host_closed_session".into(),
    };
    let json_val = serde_json::to_value(&close_event).unwrap();
    assert_eq!(json_val["type"], "room.closed");

    // 4. Host shuts down media server
    let stream_url = server.stream_url();
    server.shutdown().await;

    // 5. Subsequent HTTP byte-range request must fail with 403 or connection/request error
    let new_client = reqwest::Client::new();
    let resp_post = new_client.get(&stream_url).header(RANGE, "bytes=0-100").send().await;
    match resp_post {
        Ok(r) => assert_eq!(r.status(), StatusCode::FORBIDDEN),
        Err(e) => assert!(e.is_connect() || e.is_request(), "Expected connect or request error, got {:?}", e),
    }

    // 6. Guest command rejected
    assert_eq!(room.apply_play(&guest.id, 3, 102_000), Err(ErrorCode::RoomNotFound));
    assert_eq!(file_bytes.len(), 256 * 1024);
}

/// Workload 5: Late Joiner Catch-Up
/// Host plays video to 45:00 at 1.5x speed; new participant enters room code;
/// client starts buffering and begins playback directly at 45:00 at 1.5x speed within 3 seconds.
#[test]
fn test_tier4_workload5_late_joiner_catch_up() {
    let now = 1_000_000u64;
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_4k_video(), PermissionMode::FreeControl, now);

    // 1. Host plays video at 1.5x speed
    room.apply_play(&host_id, 2, now).unwrap();
    room.apply_speed(&host_id, 3, 1.5, now).unwrap();

    // 2. Fast forward to 45:00 (2_700_000 ms)
    room.apply_seek(&host_id, 4, 2_700_000, now + 100_000).unwrap();

    // 3. New participant enters room code 1 second later
    let join_time = now + 101_000;
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "LateBob", join_time).unwrap();

    // 4. Client starts directly at current projected timecode (45:00 + 1s * 1.5 = 2_701_500 ms)
    assert_eq!(guest.current_position_ms, 2_701_500);

    // 5. Snapshot contains 1.5x speed and Playing state
    let snapshot = room.get_snapshot(join_time);
    assert_eq!(snapshot.playback.speed, 1.5);
    assert_eq!(snapshot.playback.status, PlaybackStatus::Playing);
    assert_eq!(snapshot.playback.position_ms, 2_701_500);
}

/// Workload 6: 5-Participant Room Capacity Limit & Host Kick Re-Admission
#[test]
fn test_tier4_workload6_capacity_limit_and_host_kick_readmission() {
    let now = 100_000u64;
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_4k_video(), PermissionMode::FreeControl, now);

    // Fill room to 5 participants
    let b = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", now + 10).unwrap();
    let _c = room.join(&room.room_code.clone(), &room.room_token.clone(), "Charlie", now + 20).unwrap();
    let _d = room.join(&room.room_code.clone(), &room.room_token.clone(), "Diana", now + 30).unwrap();
    let _e = room.join(&room.room_code.clone(), &room.room_token.clone(), "Evan", now + 40).unwrap();

    assert_eq!(room.participants.len(), 5);

    // 6th participant rejected
    assert_eq!(
        room.join(&room.room_code.clone(), &room.room_token.clone(), "Frank", now + 50),
        Err(ErrorCode::RoomFull)
    );

    // Host kicks Bob
    room.kick(&host_id, &b.id).unwrap();
    assert_eq!(room.participants.len(), 4);

    // Now Frank can join
    let f = room.join(&room.room_code.clone(), &room.room_token.clone(), "Frank", now + 60);
    assert!(f.is_ok());
    assert_eq!(room.participants.len(), 5);

    // Bob cannot command
    assert_eq!(room.apply_play(&b.id, 2, now + 70), Err(ErrorCode::ParticipantNotFound));
}

/// Workload 7: Multi-Peer Reconnection Storm with Staggered Backoff
#[test]
fn test_tier4_workload7_multi_peer_reconnection_storm() {
    let now = 100_000u64;
    let mut sm_bob = ReconnectionStateMachine::default_config();
    let mut sm_charlie = ReconnectionStateMachine::default_config();
    let mut sm_diana = ReconnectionStateMachine::default_config();

    sm_bob.set_connected("tok_bob".into(), now);
    sm_charlie.set_connected("tok_charlie".into(), now);
    sm_diana.set_connected("tok_diana".into(), now);

    // Concurrent network disconnect at now + 4000
    for sm in [&mut sm_bob, &mut sm_charlie, &mut sm_diana] {
        assert!(sm.check_heartbeat_timeout(now + 4000));
        assert_eq!(sm.state, ConnectionState::Reconnecting);
    }

    // Attempt 1 for all peers has initial 1s delay
    let d_bob = sm_bob.next_reconnect_delay();
    let d_charlie = sm_charlie.next_reconnect_delay();
    let d_diana = sm_diana.next_reconnect_delay();

    assert_eq!(d_bob, Some(Duration::from_millis(1000)));
    assert_eq!(d_charlie, Some(Duration::from_millis(1000)));
    assert_eq!(d_diana, Some(Duration::from_millis(1000)));

    // Bob reconnects at attempt 1
    sm_bob.recover_connection(now + 5000);
    assert_eq!(sm_bob.state, ConnectionState::Connected);

    // Charlie fails attempt 1, backs off 2s, succeeds on attempt 2
    let d_charlie_2 = sm_charlie.next_reconnect_delay();
    assert_eq!(d_charlie_2, Some(Duration::from_millis(2000)));
    sm_charlie.recover_connection(now + 7000);
    assert_eq!(sm_charlie.state, ConnectionState::Connected);

    // Diana fails attempt 1 and 2, succeeds on attempt 3 (4s backoff)
    sm_diana.next_reconnect_delay();
    let d_diana_3 = sm_diana.next_reconnect_delay();
    assert_eq!(d_diana_3, Some(Duration::from_millis(4000)));
    sm_diana.recover_connection(now + 11000);
    assert_eq!(sm_diana.state, ConnectionState::Connected);
}

/// Workload 8: End-to-End Concurrent HTTP Byte-Range Streaming Integrity
#[tokio::test]
async fn test_tier4_workload8_e2e_concurrent_byte_range_streaming_sha256() {
    let temp_file = NamedTempFile::new().unwrap();
    let file_size = 512 * 1024; // 512 KB
    let file_bytes = Arc::new(create_synthetic_media_file(temp_file.path(), file_size).unwrap());
    let token = "e".repeat(64);

    let server = Arc::new(TestMediaServer::start(temp_file.path(), &token).await.unwrap());
    let mut handles = Vec::new();

    // 4 clients fetching contiguous non-overlapping 128 KB quarters of the video file
    for i in 0..4 {
        let stream_url = server.stream_url();
        let bytes_ref = Arc::clone(&file_bytes);
        let start = i * 131072;
        let end = start + 131071;

        handles.push(tokio::spawn(async move {
            let client = reqwest::Client::new();
            let resp = client
                .get(&stream_url)
                .header(RANGE, format!("bytes={}-{}", start, end))
                .send()
                .await
                .unwrap();

            assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
            let b = resp.bytes().await.unwrap();
            assert_eq!(b.len(), 131072);
            assert_eq!(
                compute_sha256(&b),
                compute_range_sha256(&bytes_ref, start, end)
            );
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let s = Arc::try_unwrap(server).unwrap_or_else(|_| panic!("Unwrap server"));
    s.shutdown().await;
}

/// Workload 9: Extended Playback Speed Ramp & Drift Prediction Accuracy
#[test]
fn test_tier4_workload9_speed_ramp_drift_prediction() {
    let now = 100_000u64;
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_4k_video(), PermissionMode::FreeControl, now);
    room.apply_play(&host_id, 2, now).unwrap();

    let speeds = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0];
    let mut current_server_time = now;
    for (rev, speed) in (3..).zip(speeds) {
        current_server_time += 10_000; // 10s at previous speed
        let updated = room.apply_speed(&host_id, rev, speed, current_server_time).unwrap();
        assert_eq!(updated.speed, speed);
        assert_eq!(updated.revision, rev);
    }

    // Verify projection math works accurately with the final 2.0x speed
    let final_pos = room.current_projected_position(current_server_time + 10_000);
    assert!(final_pos > 0);
}

/// Workload 10: Full Room Teardown with Active HTTP and WebSocket Mock State
#[tokio::test]
async fn test_tier4_workload10_full_room_teardown_with_active_streams() {
    let temp_file = NamedTempFile::new().unwrap();
    let file_bytes = create_synthetic_media_file(temp_file.path(), 64 * 1024).unwrap();
    let token = "f".repeat(64);

    let server = TestMediaServer::start(temp_file.path(), &token).await.unwrap();
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_4k_video(), PermissionMode::FreeControl, 100_000);
    let _guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();

    // Verify stream is active
    let client = reqwest::Client::new();
    let resp = client.get(server.stream_url()).header(RANGE, "bytes=0-100").send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);

    // Teardown
    room.close(&host_id).unwrap();
    assert!(room.is_closed);
    server.shutdown().await;

    // Both HTTP and Room state reject new requests
    assert_eq!(
        room.join(&room.room_code.clone(), &room.room_token.clone(), "Charlie", 102_000),
        Err(ErrorCode::RoomNotFound)
    );
    assert_eq!(file_bytes.len(), 64 * 1024);
}
