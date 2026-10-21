use room_server::{
    generate_room_code, generate_room_token, is_valid_playback_speed, Room, RoomServerError,
    ALLOWED_PLAYBACK_SPEEDS, CROCKFORD_CHARSET,
};
use shared_protocol::models::{
    ParticipantRole, PermissionMode, PlaybackStatus, RoomCode, RoomToken, VideoMetadata,
};

fn sample_video() -> VideoMetadata {
    VideoMetadata {
        filename: "test_video.mp4".into(),
        duration_ms: 120_000,
        file_size_bytes: 50_000_000,
        mime_type: "video/mp4".into(),
        width: Some(1920),
        height: Some(1080),
    }
}

#[test]
fn test_crockford_code_generation() {
    for _ in 0..100 {
        let code = generate_room_code();
        assert_eq!(code.as_str().len(), 6);
        assert!(code.is_valid());
        for c in code.as_str().bytes() {
            assert!(
                CROCKFORD_CHARSET.contains(&c),
                "Character {} not in Crockford charset",
                c as char
            );
        }
    }
}

#[test]
fn test_room_token_generation() {
    for _ in 0..50 {
        let token = generate_room_token();
        assert_eq!(token.as_str().len(), 64);
        assert!(token.is_valid());
        assert!(token.as_str().chars().all(|c| c.is_ascii_hexdigit()));
    }
}

#[test]
fn test_discrete_speeds() {
    for &speed in &ALLOWED_PLAYBACK_SPEEDS {
        assert!(is_valid_playback_speed(speed));
    }
    assert!(!is_valid_playback_speed(0.1));
    assert!(!is_valid_playback_speed(0.8));
    assert!(!is_valid_playback_speed(3.0));
}

#[test]
fn test_stale_revision_rejected() {
    let (mut room, host_id) = Room::new("Host", sample_video(), PermissionMode::FreeControl, 5, 1000);

    // Initial revision is 1
    assert_eq!(room.playback.revision, 1);

    // Play with rev 2
    assert!(room.apply_play(&host_id, 2, 1100).is_ok());
    assert_eq!(room.playback.revision, 2);

    // Stale command with rev 2 or lower rejected
    let res = room.apply_play(&host_id, 2, 1200);
    assert!(res.is_err());
    match res.unwrap_err() {
        RoomServerError::StaleRevision { command_revision, current_revision } => {
            assert_eq!(command_revision, 2);
            assert_eq!(current_revision, 2);
        }
        other => panic!("Expected StaleRevision, got {:?}", other),
    }

    let res_stale = room.apply_pause(&host_id, 1, 5000, 1300);
    assert!(res_stale.is_err());
}

#[test]
fn test_invalid_speed_rejected() {
    let (mut room, host_id) = Room::new("Host", sample_video(), PermissionMode::FreeControl, 5, 1000);

    let res = room.apply_speed(&host_id, 2, 1.8, 1100);
    assert!(res.is_err());
    match res.unwrap_err() {
        RoomServerError::InvalidMessage(msg) => {
            assert!(msg.contains("Unsupported speed: 1.8"));
        }
        other => panic!("Expected InvalidMessage, got {:?}", other),
    }

    // Valid speed works
    let ok_res = room.apply_speed(&host_id, 2, 1.5, 1100);
    assert!(ok_res.is_ok());
    assert_eq!(room.playback.speed, 1.5);
}

#[test]
fn test_late_joiner_projection() {
    let (mut room, host_id) = Room::new("Host", sample_video(), PermissionMode::FreeControl, 5, 1000);

    // Start playing at t = 1000, pos = 0, speed = 1.0
    room.apply_play(&host_id, 2, 1000).unwrap();

    // Join at t = 5000 (4000ms elapsed at 1.0x -> pos 4000)
    let code = room.room_code.clone();
    let token = room.room_token.clone();
    let guest = room.join(&code, &token, "Guest", None, 5000).unwrap();

    assert_eq!(guest.current_position_ms, 4000);
    assert_eq!(guest.buffered_position_ms, 4000);
    assert_eq!(guest.role, ParticipantRole::Guest);

    let snapshot = room.get_snapshot(5000);
    assert_eq!(snapshot.playback.position_ms, 4000);
    assert_eq!(snapshot.playback.status, PlaybackStatus::Playing);
}

#[test]
fn test_room_teardown_and_post_close_invalidation() {
    let (mut room, host_id) = Room::new("Host", sample_video(), PermissionMode::FreeControl, 5, 1000);
    let code = room.room_code.clone();
    let token = room.room_token.clone();

    // Host closes room
    assert!(room.close(&host_id).is_ok());
    assert!(room.is_closed);

    // Close again is idempotent
    assert!(room.close(&host_id).is_ok());

    // Subsequent join fails with RoomNotFound
    let join_res = room.join(&code, &token, "Late Guest", None, 2000);
    assert!(join_res.is_err());
    match join_res.unwrap_err() {
        RoomServerError::RoomNotFound(_) => {}
        other => panic!("Expected RoomNotFound, got {:?}", other),
    }

    // Playback commands fail with RoomNotFound
    assert!(room.apply_play(&host_id, 2, 2000).is_err());
    assert!(room.apply_pause(&host_id, 2, 0, 2000).is_err());
    assert!(room.apply_seek(&host_id, 2, 1000, 2000).is_err());
    assert!(room.apply_speed(&host_id, 2, 1.25, 2000).is_err());
}

#[test]
fn test_invalid_code_or_token_rejected() {
    let (mut room, _) = Room::new("Host", sample_video(), PermissionMode::FreeControl, 5, 1000);
    let code = room.room_code.clone();
    let token = room.room_token.clone();

    let invalid_code = RoomCode::new("ABCDEF");
    let invalid_token = RoomToken::new("0".repeat(64));

    // Wrong code
    assert!(room.join(&invalid_code, &token, "Guest", None, 2000).is_err());

    // Wrong token
    assert!(room.join(&code, &invalid_token, "Guest", None, 2000).is_err());
}
