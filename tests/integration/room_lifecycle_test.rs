use watch_together_tests::protocol::*;
use watch_together_tests::room::{MockRoomServer, ALLOWED_PLAYBACK_SPEEDS};

fn sample_video() -> VideoMetadata {
    VideoMetadata {
        filename: "test_movie.mkv".into(),
        file_size_bytes: 2_000_000_000,
        duration_ms: 7_200_000, // 2 hours
        mime_type: "video/x-matroska".into(),
        width: Some(1920),
        height: Some(1080),
    }
}

// ============================================================================
// FEATURE 8: ROOM CREATION & CODE/TOKEN GENERATION
// ============================================================================

#[test]
fn test_tier1_f8_create_room_generates_valid_code() {
    let (room, _) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 1_000_000);
    assert!(room.room_code.is_valid());
    assert_eq!(room.room_code.0.len(), 6);
}

#[test]
fn test_tier1_f8_create_room_generates_valid_token() {
    let (room, _) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 1_000_000);
    assert!(room.room_token.is_valid());
    assert_eq!(room.room_token.0.len(), 64);
}

#[test]
fn test_tier1_f8_create_room_assigns_host_role() {
    let (room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 1_000_000);
    let host = room.participants.get(&host_id).unwrap();
    assert_eq!(host.role, ParticipantRole::Host);
    assert_eq!(host.display_name, "Alice");
    assert!(host.is_connected);
}

#[test]
fn test_tier1_f8_create_room_default_permission_mode() {
    let (room_free, _) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 1_000_000);
    assert_eq!(room_free.permission_mode, PermissionMode::FreeControl);

    let (room_host_only, _) = MockRoomServer::new("Alice", sample_video(), PermissionMode::HostOnly, 1_000_000);
    assert_eq!(room_host_only.permission_mode, PermissionMode::HostOnly);
}

#[test]
fn test_tier1_f8_create_room_initial_revision_and_status() {
    let (room, _) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 1_000_000);
    assert_eq!(room.playback.revision, 1);
    assert_eq!(room.playback.status, PlaybackStatus::Paused);
    assert_eq!(room.playback.position_ms, 0);
    assert_eq!(room.playback.speed, 1.0);
}

#[test]
fn test_tier2_f8_join_with_invalid_code_rejected() {
    let (mut room, _) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 1_000_000);
    let wrong_code = RoomCode::new("WRONG9");
    let token = room.room_token.clone();
    let res = room.join(&wrong_code, &token, "Bob", 1_001_000);

    assert_eq!(res, Err(ErrorCode::InvalidRoomCode));
}

#[test]
fn test_tier2_f8_join_with_invalid_token_rejected() {
    let (mut room, _) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 1_000_000);
    let wrong_token = RoomToken::new("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff");
    let code = room.room_code.clone();
    let res = room.join(&code, &wrong_token, "Bob", 1_001_000);

    assert_eq!(res, Err(ErrorCode::InvalidToken));
}

#[test]
fn test_tier2_f8_join_with_empty_code_or_token_rejected() {
    let (mut room, _) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 1_000_000);
    let token = room.room_token.clone();
    let code = room.room_code.clone();
    assert_eq!(
        room.join(&RoomCode::new(""), &token, "Bob", 1_001_000),
        Err(ErrorCode::InvalidRoomCode)
    );
    assert_eq!(
        room.join(&code, &RoomToken::new(""), "Bob", 1_001_000),
        Err(ErrorCode::InvalidToken)
    );
}

#[test]
fn test_tier2_f8_multiple_rooms_generate_unique_codes() {
    let mut codes = std::collections::HashSet::new();
    for i in 1..=20 {
        let (room, _) = MockRoomServer::new("Host", sample_video(), PermissionMode::FreeControl, 1_000_000 + i * 997);
        assert!(codes.insert(room.room_code.0), "Room codes must be unique");
    }
}

#[test]
fn test_tier2_f8_multiple_rooms_generate_unique_tokens() {
    let mut tokens = std::collections::HashSet::new();
    for i in 1..=20 {
        let (room, _) = MockRoomServer::new("Host", sample_video(), PermissionMode::FreeControl, 1_000_000 + i);
        assert!(tokens.insert(room.room_token.0), "Room tokens must be unique");
    }
}

// ============================================================================
// FEATURE 10: PLAYBACK STATE SYNCHRONIZATION (PLAY/PAUSE)
// ============================================================================

#[test]
fn test_tier1_f10_host_play_advances_revision_and_status() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let updated = room.apply_play(&host_id, 2, 101_000).expect("Play failed");

    assert_eq!(updated.revision, 2);
    assert_eq!(updated.status, PlaybackStatus::Playing);
    assert_eq!(updated.updated_at_server_ms, 101_000);
}

#[test]
fn test_tier1_f10_host_pause_updates_state_and_advances_revision() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    room.apply_play(&host_id, 2, 101_000).unwrap();

    let updated = room.apply_pause(&host_id, 3, 50_000, 151_000).expect("Pause failed");
    assert_eq!(updated.revision, 3);
    assert_eq!(updated.status, PlaybackStatus::Paused);
    assert_eq!(updated.position_ms, 50_000);
}

#[test]
fn test_tier1_f10_guest_play_accepted_in_free_control() {
    let (mut room, _) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();

    let updated = room.apply_play(&guest.id, 2, 102_000).expect("Guest play should succeed");
    assert_eq!(updated.revision, 2);
    assert_eq!(updated.status, PlaybackStatus::Playing);
}

#[test]
fn test_tier1_f10_guest_pause_accepted_in_free_control() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();
    room.apply_play(&host_id, 2, 102_000).unwrap();

    let updated = room.apply_pause(&guest.id, 3, 10_000, 112_000).expect("Guest pause should succeed");
    assert_eq!(updated.revision, 3);
    assert_eq!(updated.status, PlaybackStatus::Paused);
}

#[test]
fn test_tier1_f10_state_broadcast_contains_triggering_participant() {
    let msg = ServerMessage::PlaybackUpdated {
        revision: 5,
        status: PlaybackStatus::Playing,
        position_ms: 12_000,
        speed: 1.0,
        updated_at_server_ms: 200_000,
        triggered_by: ParticipantId::new("part-guest-002"),
        action: "play".into(),
    };

    let json_val = serde_json::to_value(&msg).unwrap();
    assert_eq!(json_val["payload"]["triggered_by"], "part-guest-002");
    assert_eq!(json_val["payload"]["action"], "play");
}

#[test]
fn test_tier2_f10_play_command_with_stale_revision_rejected() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    room.apply_play(&host_id, 10, 101_000).unwrap();

    // Rev 8 < 10 -> StaleRevision
    let res = room.apply_play(&host_id, 8, 102_000);
    assert_eq!(res, Err(ErrorCode::StaleRevision));
}

#[test]
fn test_tier2_f10_pause_command_with_stale_revision_rejected() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    room.apply_play(&host_id, 5, 101_000).unwrap();

    let res = room.apply_pause(&host_id, 4, 1000, 102_000);
    assert_eq!(res, Err(ErrorCode::StaleRevision));
}

#[test]
fn test_tier2_f10_play_command_with_equal_revision_rejected() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    room.apply_play(&host_id, 5, 101_000).unwrap();

    let res = room.apply_play(&host_id, 5, 102_000);
    assert_eq!(res, Err(ErrorCode::StaleRevision));
}

#[test]
fn test_tier2_f10_pause_at_negative_position_handled() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let updated = room.apply_pause(&host_id, 2, 0, 101_000).unwrap();
    assert_eq!(updated.position_ms, 0);
}

#[test]
fn test_tier2_f10_pause_beyond_duration_clamped() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let updated = room.apply_pause(&host_id, 2, 999_999_999, 101_000).unwrap();
    assert_eq!(updated.position_ms, sample_video().duration_ms);
}

// ============================================================================
// FEATURE 11: SEEK SYNCHRONIZATION
// ============================================================================

#[test]
fn test_tier1_f11_host_seek_updates_position_and_advances_revision() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let updated = room.apply_seek(&host_id, 2, 300_000, 101_000).unwrap();

    assert_eq!(updated.revision, 2);
    assert_eq!(updated.position_ms, 300_000);
}

#[test]
fn test_tier1_f11_guest_seek_accepted_in_free_control() {
    let (mut room, _) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();

    let updated = room.apply_seek(&guest.id, 2, 150_000, 102_000).unwrap();
    assert_eq!(updated.revision, 2);
    assert_eq!(updated.position_ms, 150_000);
}

#[test]
fn test_tier1_f11_seek_preserves_playing_status() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    room.apply_play(&host_id, 2, 101_000).unwrap();

    let updated = room.apply_seek(&host_id, 3, 200_000, 105_000).unwrap();
    assert_eq!(updated.status, PlaybackStatus::Playing);
    assert_eq!(updated.position_ms, 200_000);
}

#[test]
fn test_tier1_f11_seek_preserves_paused_status() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    assert_eq!(room.playback.status, PlaybackStatus::Paused);

    let updated = room.apply_seek(&host_id, 2, 500_000, 101_000).unwrap();
    assert_eq!(updated.status, PlaybackStatus::Paused);
    assert_eq!(updated.position_ms, 500_000);
}

#[test]
fn test_tier1_f11_seek_broadcast_reaches_all_participants() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();
    room.join(&room.room_code.clone(), &room.room_token.clone(), "Charlie", 102_000).unwrap();

    let state = room.apply_seek(&host_id, 2, 1_000_000, 103_000).unwrap();
    let snapshot = room.get_snapshot(103_000);

    assert_eq!(snapshot.playback.position_ms, 1_000_000);
    assert_eq!(snapshot.participants.len(), 3);
    assert_eq!(state.position_ms, 1_000_000);
}

#[test]
fn test_tier2_f11_seek_to_start_boundary_zero() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    room.apply_seek(&host_id, 2, 500_000, 101_000).unwrap();
    let updated = room.apply_seek(&host_id, 3, 0, 102_000).unwrap();

    assert_eq!(updated.position_ms, 0);
}

#[test]
fn test_tier2_f11_seek_to_exact_duration_boundary() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let dur = sample_video().duration_ms;
    let updated = room.apply_seek(&host_id, 2, dur, 101_000).unwrap();

    assert_eq!(updated.position_ms, dur);
}

#[test]
fn test_tier2_f11_seek_beyond_duration_clamped_to_duration() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let dur = sample_video().duration_ms;
    let updated = room.apply_seek(&host_id, 2, dur + 1_000_000, 101_000).unwrap();

    assert_eq!(updated.position_ms, dur);
}

#[test]
fn test_tier2_f11_seek_with_stale_revision_rejected() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    room.apply_seek(&host_id, 5, 100_000, 101_000).unwrap();

    assert_eq!(
        room.apply_seek(&host_id, 3, 200_000, 102_000),
        Err(ErrorCode::StaleRevision)
    );
}

#[test]
fn test_tier2_f11_consecutive_rapid_seeks_advance_revisions_monotonically() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    for rev in 2..=10 {
        let updated = room.apply_seek(&host_id, rev, rev * 10_000, 100_000 + rev).unwrap();
        assert_eq!(updated.revision, rev);
        assert_eq!(updated.position_ms, rev * 10_000);
    }
}

// ============================================================================
// FEATURE 12: SPEED SYNCHRONIZATION
// ============================================================================

#[test]
fn test_tier1_f12_host_change_speed_to_1_5x() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let updated = room.apply_speed(&host_id, 2, 1.5, 101_000).unwrap();

    assert_eq!(updated.revision, 2);
    assert_eq!(updated.speed, 1.5);
}

#[test]
fn test_tier1_f12_guest_change_speed_in_free_control() {
    let (mut room, _) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();

    let updated = room.apply_speed(&guest.id, 2, 1.25, 102_000).unwrap();
    assert_eq!(updated.speed, 1.25);
}

#[test]
fn test_tier1_f12_speed_change_updates_server_timestamp() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let updated = room.apply_speed(&host_id, 2, 2.0, 105_000).unwrap();

    assert_eq!(updated.updated_at_server_ms, 105_000);
}

#[test]
fn test_tier1_f12_speed_change_preserves_projected_position() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    room.apply_play(&host_id, 2, 100_000).unwrap();

    // 10 seconds elapsed at 1.0x -> position 10_000
    let updated = room.apply_speed(&host_id, 3, 1.5, 110_000).unwrap();
    assert_eq!(updated.position_ms, 10_000);
    assert_eq!(updated.speed, 1.5);
}

#[test]
fn test_tier1_f12_speed_all_discrete_speeds_accepted() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    for (rev, &s) in (2..).zip(ALLOWED_PLAYBACK_SPEEDS.iter()) {
        let updated = room.apply_speed(&host_id, rev, s, 100_000 + rev).unwrap();
        assert_eq!(updated.speed, s);
    }
}

#[test]
fn test_tier2_f12_speed_minimum_boundary_0_5x() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let updated = room.apply_speed(&host_id, 2, 0.5, 101_000).unwrap();
    assert_eq!(updated.speed, 0.5);
}

#[test]
fn test_tier2_f12_speed_maximum_boundary_2_0x() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let updated = room.apply_speed(&host_id, 2, 2.0, 101_000).unwrap();
    assert_eq!(updated.speed, 2.0);
}

#[test]
fn test_tier2_f12_speed_invalid_speed_rejected() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    assert_eq!(
        room.apply_speed(&host_id, 2, 0.8, 101_000),
        Err(ErrorCode::InvalidMessage)
    );
    assert_eq!(
        room.apply_speed(&host_id, 3, 3.0, 102_000),
        Err(ErrorCode::InvalidMessage)
    );
}

#[test]
fn test_tier2_f12_speed_change_with_stale_revision_rejected() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    room.apply_speed(&host_id, 5, 1.5, 101_000).unwrap();

    assert_eq!(
        room.apply_speed(&host_id, 4, 1.0, 102_000),
        Err(ErrorCode::StaleRevision)
    );
}

#[test]
fn test_tier2_f12_speed_change_while_paused_retains_paused_status() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    assert_eq!(room.playback.status, PlaybackStatus::Paused);

    let updated = room.apply_speed(&host_id, 2, 1.5, 101_000).unwrap();
    assert_eq!(updated.status, PlaybackStatus::Paused);
    assert_eq!(updated.speed, 1.5);
}

// ============================================================================
// FEATURE 13: LATE JOINER TIMECODE SYNC
// ============================================================================

#[test]
fn test_tier1_f13_late_joiner_gets_projected_timecode_when_playing() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    room.apply_play(&host_id, 2, 100_000).unwrap();

    // Bob joins 30 seconds later (playing at 1.0x -> pos 30_000)
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 130_000).unwrap();
    assert_eq!(guest.current_position_ms, 30_000);
}

#[test]
fn test_tier1_f13_late_joiner_gets_paused_position_when_paused() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    room.apply_seek(&host_id, 2, 45_000, 100_000).unwrap();

    // Bob joins 10 minutes later while paused
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 700_000).unwrap();
    assert_eq!(guest.current_position_ms, 45_000);
}

#[test]
fn test_tier1_f13_late_joiner_receives_current_speed() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    room.apply_speed(&host_id, 2, 1.5, 101_000).unwrap();

    let _guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 102_000).unwrap();
    let snapshot = room.get_snapshot(102_000);
    assert_eq!(snapshot.playback.speed, 1.5);
}

#[test]
fn test_tier1_f13_late_joiner_receives_current_revision() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    room.apply_play(&host_id, 2, 101_000).unwrap();
    room.apply_seek(&host_id, 3, 20_000, 102_000).unwrap();

    let _guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 103_000).unwrap();
    let snapshot = room.get_snapshot(103_000);
    assert_eq!(snapshot.playback.revision, 3);
}

#[test]
fn test_tier1_f13_late_joiner_receives_complete_participant_roster() {
    let (mut room, _) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();
    room.join(&room.room_code.clone(), &room.room_token.clone(), "Charlie", 102_000).unwrap();

    let snapshot = room.get_snapshot(103_000);
    assert_eq!(snapshot.participants.len(), 3);
}

#[test]
fn test_tier2_f13_late_joiner_near_end_clamped_at_duration() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let dur = sample_video().duration_ms; // 7_200_000
    room.apply_seek(&host_id, 2, dur - 10_000, 100_000).unwrap();
    room.apply_play(&host_id, 3, 100_000).unwrap();

    // Guest joins 60 seconds later (exceeds duration)
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 160_000).unwrap();
    assert_eq!(guest.current_position_ms, dur);
}

#[test]
fn test_tier2_f13_late_joiner_at_exact_zero() {
    let (mut room, _) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 100_000).unwrap();
    assert_eq!(guest.current_position_ms, 0);
}

#[test]
fn test_tier2_f13_late_joiner_at_high_speed_2x_projection() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    room.apply_play(&host_id, 2, 100_000).unwrap();
    room.apply_speed(&host_id, 3, 2.0, 100_000).unwrap();

    // 20s elapsed at 2x -> pos 40_000
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 120_000).unwrap();
    assert_eq!(guest.current_position_ms, 40_000);
}

#[test]
fn test_tier2_f13_late_joiner_at_half_speed_0_5x_projection() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    room.apply_play(&host_id, 2, 100_000).unwrap();
    room.apply_speed(&host_id, 3, 0.5, 100_000).unwrap();

    // 20s elapsed at 0.5x -> pos 10_000
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 120_000).unwrap();
    assert_eq!(guest.current_position_ms, 10_000);
}

#[test]
fn test_tier2_f13_late_joiner_sync_drift_engine_init() {
    use watch_together_tests::sync::{Correction, DriftEngine};

    let mut engine = DriftEngine::default_config();
    // Late joiner starts at pos 0 while expected is 120_000 -> hard seek
    let c = engine.evaluate(0, 120_000, 1.0);
    assert_eq!(
        c,
        Correction::HardSeek {
            target_position_ms: 120_000,
            drift_ms: -120_000,
        }
    );
}

// ============================================================================
// FEATURE 14: PERMISSION MODES (FREE VS HOST-ONLY)
// ============================================================================

#[test]
fn test_tier1_f14_host_toggles_mode_to_host_only() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    assert_eq!(room.permission_mode, PermissionMode::FreeControl);

    room.set_permission_mode(&host_id, PermissionMode::HostOnly).unwrap();
    assert_eq!(room.permission_mode, PermissionMode::HostOnly);
}

#[test]
fn test_tier1_f14_host_only_guest_play_rejected() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::HostOnly, 100_000);
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();

    let res = room.apply_play(&guest.id, 2, 102_000);
    assert_eq!(res, Err(ErrorCode::PermissionDenied));
    // Host play still works
    assert!(room.apply_play(&host_id, 2, 102_000).is_ok());
}

#[test]
fn test_tier1_f14_host_only_guest_pause_rejected() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::HostOnly, 100_000);
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();
    room.apply_play(&host_id, 2, 102_000).unwrap();

    let res = room.apply_pause(&guest.id, 3, 10_000, 105_000);
    assert_eq!(res, Err(ErrorCode::PermissionDenied));
}

#[test]
fn test_tier1_f14_host_only_guest_seek_rejected() {
    let (mut room, _) = MockRoomServer::new("Alice", sample_video(), PermissionMode::HostOnly, 100_000);
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();

    let res = room.apply_seek(&guest.id, 2, 50_000, 102_000);
    assert_eq!(res, Err(ErrorCode::PermissionDenied));
}

#[test]
fn test_tier1_f14_host_only_guest_speed_rejected() {
    let (mut room, _) = MockRoomServer::new("Alice", sample_video(), PermissionMode::HostOnly, 100_000);
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();

    let res = room.apply_speed(&guest.id, 2, 1.5, 102_000);
    assert_eq!(res, Err(ErrorCode::PermissionDenied));
}

#[test]
fn test_tier2_f14_host_only_host_commands_succeed() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::HostOnly, 100_000);
    assert!(room.apply_play(&host_id, 2, 101_000).is_ok());
    assert!(room.apply_seek(&host_id, 3, 100_000, 102_000).is_ok());
    assert!(room.apply_speed(&host_id, 4, 1.5, 103_000).is_ok());
    assert!(room.apply_pause(&host_id, 5, 100_000, 104_000).is_ok());
}

#[test]
fn test_tier2_f14_free_control_all_guest_commands_succeed() {
    let (mut room, _) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();

    assert!(room.apply_play(&guest.id, 2, 102_000).is_ok());
    assert!(room.apply_seek(&guest.id, 3, 50_000, 103_000).is_ok());
    assert!(room.apply_speed(&guest.id, 4, 1.25, 104_000).is_ok());
    assert!(room.apply_pause(&guest.id, 5, 50_000, 105_000).is_ok());
}

#[test]
fn test_tier2_f14_guest_attempt_to_change_permission_mode_rejected() {
    let (mut room, _) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();

    let res = room.set_permission_mode(&guest.id, PermissionMode::HostOnly);
    assert_eq!(res, Err(ErrorCode::PermissionDenied));
    assert_eq!(room.permission_mode, PermissionMode::FreeControl);
}

#[test]
fn test_tier2_f14_toggle_back_to_free_control_restores_guest_access() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::HostOnly, 100_000);
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();

    assert!(room.apply_play(&guest.id, 2, 102_000).is_err());

    // Host toggles back to FreeControl
    room.set_permission_mode(&host_id, PermissionMode::FreeControl).unwrap();
    assert!(room.apply_play(&guest.id, 2, 103_000).is_ok());
}

#[test]
fn test_tier2_f14_permission_denied_preserves_current_state_unaltered() {
    let (mut room, _) = MockRoomServer::new("Alice", sample_video(), PermissionMode::HostOnly, 100_000);
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();
    let initial_playback = room.playback.clone();

    let _ = room.apply_seek(&guest.id, 2, 999_000, 102_000);
    assert_eq!(room.playback, initial_playback);
}

// ============================================================================
// FEATURE 15 & 16: CAPACITY, KICKING & REVISION MONOTONICITY
// ============================================================================

#[test]
fn test_tier1_f15_room_capacity_accepts_up_to_5_participants() {
    let (mut room, _) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    // 1 host already in room -> can add 4 guests to reach 5 total
    let g1 = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000);
    let g2 = room.join(&room.room_code.clone(), &room.room_token.clone(), "Charlie", 102_000);
    let g3 = room.join(&room.room_code.clone(), &room.room_token.clone(), "Diana", 103_000);
    let g4 = room.join(&room.room_code.clone(), &room.room_token.clone(), "Evan", 104_000);

    assert!(g1.is_ok());
    assert!(g2.is_ok());
    assert!(g3.is_ok());
    assert!(g4.is_ok());
    assert_eq!(room.participants.len(), 5);
}

#[test]
fn test_tier1_f15_room_capacity_rejects_6th_participant_with_room_full() {
    let (mut room, _) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    for name in &["Bob", "Charlie", "Diana", "Evan"] {
        room.join(&room.room_code.clone(), &room.room_token.clone(), name, 101_000).unwrap();
    }
    assert_eq!(room.participants.len(), 5);

    // 6th participant rejected
    let res = room.join(&room.room_code.clone(), &room.room_token.clone(), "Frank", 105_000);
    assert_eq!(res, Err(ErrorCode::RoomFull));
}

#[test]
fn test_tier1_f16_host_kicks_guest_successfully() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();

    let kick_res = room.kick(&host_id, &guest.id);
    assert!(kick_res.is_ok());
    assert!(!room.participants.contains_key(&guest.id));
}

#[test]
fn test_tier1_f16_kicked_guest_cannot_issue_commands() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();
    room.kick(&host_id, &guest.id).unwrap();

    let res = room.apply_play(&guest.id, 2, 102_000);
    assert_eq!(res, Err(ErrorCode::ParticipantNotFound));
}

#[test]
fn test_tier1_f16_guest_cannot_kick_host() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();

    let res = room.kick(&guest.id, &host_id);
    assert_eq!(res, Err(ErrorCode::PermissionDenied));
    assert!(room.participants.contains_key(&host_id));
}

#[test]
fn test_tier2_f15_capacity_freed_after_guest_leaves() {
    let (mut room, _) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let mut guest_ids = Vec::new();
    for name in &["Bob", "Charlie", "Diana", "Evan"] {
        let g = room.join(&room.room_code.clone(), &room.room_token.clone(), name, 101_000).unwrap();
        guest_ids.push(g.id);
    }
    assert_eq!(room.participants.len(), 5);

    // Guest leaves
    room.leave(&guest_ids[0]).unwrap();
    assert_eq!(room.participants.len(), 4);

    // Now a new guest can join
    let new_guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Frank", 105_000);
    assert!(new_guest.is_ok());
    assert_eq!(room.participants.len(), 5);
}

#[test]
fn test_tier2_f15_guest_can_rejoin_after_slot_freed() {
    let (mut room, _) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let g1 = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();
    room.leave(&g1.id).unwrap();

    let g1_rejoin = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 102_000);
    assert!(g1_rejoin.is_ok());
}

#[test]
fn test_tier2_f16_guest_cannot_kick_other_guests() {
    let (mut room, _) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let g1 = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();
    let g2 = room.join(&room.room_code.clone(), &room.room_token.clone(), "Charlie", 102_000).unwrap();

    let res = room.kick(&g1.id, &g2.id);
    assert_eq!(res, Err(ErrorCode::PermissionDenied));
    assert!(room.participants.contains_key(&g2.id));
}

#[test]
fn test_tier2_f16_kick_non_existent_participant_fails() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let non_existent = ParticipantId::new("part-ghost-999");

    let res = room.kick(&host_id, &non_existent);
    assert_eq!(res, Err(ErrorCode::ParticipantNotFound));
}

#[test]
fn test_tier2_f16_host_cannot_kick_self() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let res = room.kick(&host_id, &host_id);
    assert_eq!(res, Err(ErrorCode::PermissionDenied));
    assert!(room.participants.contains_key(&host_id));
}

// ============================================================================
// FEATURE 20: ROOM TEARDOWN & POST-CLOSE INVALIDATION
// ============================================================================

#[test]
fn test_tier1_f20_host_closes_room_sets_is_closed() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    assert!(!room.is_closed);

    room.close(&host_id).unwrap();
    assert!(room.is_closed);
}

#[test]
fn test_tier1_f20_post_close_join_fails_with_room_not_found() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    room.close(&host_id).unwrap();

    let res = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000);
    assert_eq!(res, Err(ErrorCode::RoomNotFound));
}

#[test]
fn test_tier1_f20_post_close_play_fails() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    room.close(&host_id).unwrap();

    let res = room.apply_play(&host_id, 2, 101_000);
    assert_eq!(res, Err(ErrorCode::RoomNotFound));
}

#[test]
fn test_tier1_f20_post_close_seek_fails() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    room.close(&host_id).unwrap();

    let res = room.apply_seek(&host_id, 2, 100_000, 101_000);
    assert_eq!(res, Err(ErrorCode::RoomNotFound));
}

#[test]
fn test_tier1_f20_host_quitting_automatically_closes_room() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();

    // Host leaves -> room automatically closes
    room.leave(&host_id).unwrap();
    assert!(room.is_closed);
}

#[test]
fn test_tier2_f20_guest_cannot_close_room() {
    let (mut room, _) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();

    let res = room.close(&guest.id);
    assert_eq!(res, Err(ErrorCode::PermissionDenied));
    assert!(!room.is_closed);
}

#[test]
fn test_tier2_f20_guest_quitting_does_not_close_room() {
    let (mut room, _) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();

    room.leave(&guest.id).unwrap();
    assert!(!room.is_closed);
}

#[test]
fn test_tier2_f20_post_close_speed_change_fails() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    room.close(&host_id).unwrap();

    let res = room.apply_speed(&host_id, 2, 1.5, 101_000);
    assert_eq!(res, Err(ErrorCode::RoomNotFound));
}

#[test]
fn test_tier2_f20_post_close_kick_fails() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    let guest = room.join(&room.room_code.clone(), &room.room_token.clone(), "Bob", 101_000).unwrap();
    room.close(&host_id).unwrap();

    let res = room.kick(&host_id, &guest.id);
    assert_eq!(res, Err(ErrorCode::RoomNotFound));
}

#[test]
fn test_tier2_f20_close_already_closed_room_is_idempotent() {
    let (mut room, host_id) = MockRoomServer::new("Alice", sample_video(), PermissionMode::FreeControl, 100_000);
    assert!(room.close(&host_id).is_ok());
    assert!(room.close(&host_id).is_ok());
    assert!(room.is_closed);
}
