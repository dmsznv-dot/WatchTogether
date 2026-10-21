use serde_json::{json, Value};
use watch_together_tests::protocol::*;

// ============================================================================
// TIER 1: FEATURE COVERAGE (SERIALIZATION & WIRE SCHEMAS)
// ============================================================================

#[test]
fn test_tier1_client_messages_roundtrip_all_11_variants() {
    let video = VideoMetadata {
        filename: "matrix_reloaded.mkv".into(),
        file_size_bytes: 4_294_967_296,
        duration_ms: 8_280_000,
        mime_type: "video/x-matroska".into(),
        width: Some(3840),
        height: Some(2160),
    };

    let client_messages = vec![
        ClientMessage::RoomCreate {
            host_name: "Alice".into(),
            video: video.clone(),
            permission_mode: PermissionMode::FreeControl,
        },
        ClientMessage::RoomJoin {
            room_code: RoomCode::new("K7M2P9"),
            room_token: RoomToken::new("a1b2c3d4e5f60718293a4b5c6d7e8f901234567890abcdef"),
            display_name: "Bob".into(),
            participant_id: Some(ParticipantId::new("part-002")),
        },
        ClientMessage::PlaybackPlay {
            revision: 42,
            client_timestamp_ms: 1_700_000_000_100,
        },
        ClientMessage::PlaybackPause {
            revision: 43,
            position_ms: 125_400,
            client_timestamp_ms: 1_700_000_000_200,
        },
        ClientMessage::PlaybackSeek {
            revision: 44,
            target_position_ms: 300_000,
            client_timestamp_ms: 1_700_000_000_300,
        },
        ClientMessage::PlaybackSpeed {
            revision: 45,
            speed: 1.5,
            client_timestamp_ms: 1_700_000_000_400,
        },
        ClientMessage::ParticipantStatus {
            participant_id: ParticipantId::new("part-002"),
            buffered_position_ms: 320_000,
            current_position_ms: 300_000,
            is_buffering: false,
            ping_ms: 24,
        },
        ClientMessage::SyncPing {
            ping_id: 101,
            client_send_time_ms: 1_700_000_000_500,
        },
        ClientMessage::RoomSetPermissions {
            permission_mode: PermissionMode::HostOnly,
        },
        ClientMessage::ParticipantKick {
            target_id: ParticipantId::new("part-003"),
        },
        ClientMessage::RoomLeave {
            participant_id: ParticipantId::new("part-002"),
        },
    ];

    assert_eq!(client_messages.len(), 11, "Must test exactly 11 client message variants");

    for original in client_messages {
        let json_str = serde_json::to_string(&original).expect("Serialization failed");
        // Verify wire envelope schema
        let v: Value = serde_json::from_str(&json_str).expect("Valid JSON");
        assert!(v.get("type").is_some(), "Must contain 'type' field");
        assert!(v.get("payload").is_some(), "Must contain 'payload' field");

        let deserialized: ClientMessage = serde_json::from_str(&json_str).expect("Deserialization failed");
        assert_eq!(original, deserialized);
    }
}

#[test]
fn test_tier1_server_messages_roundtrip_all_8_variants() {
    let participant = Participant {
        id: ParticipantId::new("part-001"),
        display_name: "Alice".into(),
        role: ParticipantRole::Host,
        is_connected: true,
        buffered_position_ms: 100_000,
        current_position_ms: 50_000,
        ping_ms: 12,
        joined_at_ms: 1_700_000_000_000,
    };

    let snapshot = RoomStateSnapshot {
        room_id: RoomId::new("room-uuid-001"),
        room_code: RoomCode::new("K7M2P9"),
        host_id: ParticipantId::new("part-001"),
        permission_mode: PermissionMode::FreeControl,
        participants: vec![participant.clone()],
        playback: PlaybackState {
            revision: 10,
            status: PlaybackStatus::Playing,
            position_ms: 50_000,
            speed: 1.0,
            updated_at_server_ms: 1_700_000_000_050,
            duration_ms: 7_200_000,
        },
        video: VideoMetadata {
            filename: "interstellar.mp4".into(),
            file_size_bytes: 5_000_000_000,
            duration_ms: 7_200_000,
            mime_type: "video/mp4".into(),
            width: Some(1920),
            height: Some(1080),
        },
    };

    let server_messages = vec![
        ServerMessage::RoomState(snapshot),
        ServerMessage::PlaybackUpdated {
            revision: 11,
            status: PlaybackStatus::Paused,
            position_ms: 55_000,
            speed: 1.0,
            updated_at_server_ms: 1_700_000_000_100,
            triggered_by: ParticipantId::new("part-001"),
            action: "pause".into(),
        },
        ServerMessage::ParticipantJoined(participant),
        ServerMessage::ParticipantLeft {
            participant_id: ParticipantId::new("part-002"),
            reason: "client_disconnected".into(),
        },
        ServerMessage::ParticipantKicked {
            participant_id: ParticipantId::new("part-003"),
            reason: "kicked_by_host".into(),
        },
        ServerMessage::SyncPong {
            ping_id: 101,
            client_send_time_ms: 1_700_000_000_500,
            server_receive_time_ms: 1_700_000_000_525,
            server_send_time_ms: 1_700_000_000_526,
        },
        ServerMessage::RoomClosed {
            reason: "host_quit".into(),
        },
        ServerMessage::Error {
            code: "ROOM_FULL".into(),
            message: "Room capacity reached (max 5 participants)".into(),
        },
    ];

    assert_eq!(server_messages.len(), 8, "Must test exactly 8 server message variants");

    for original in server_messages {
        let json_str = serde_json::to_string(&original).expect("Serialization failed");
        let v: Value = serde_json::from_str(&json_str).expect("Valid JSON");
        assert!(v.get("type").is_some(), "Must contain 'type' field");
        assert!(v.get("payload").is_some(), "Must contain 'payload' field");

        let deserialized: ServerMessage = serde_json::from_str(&json_str).expect("Deserialization failed");
        assert_eq!(original, deserialized);
    }
}

#[test]
fn test_tier1_wire_schema_event_names_exact_matching() {
    // Verify wire type names strictly adhere to ORIGINAL_REQUEST.md Protocol Events table
    let event_cases = vec![
        (
            ClientMessage::PlaybackPlay { revision: 1, client_timestamp_ms: 100 },
            "playback.play",
        ),
        (
            ClientMessage::PlaybackPause { revision: 2, position_ms: 100, client_timestamp_ms: 100 },
            "playback.pause",
        ),
        (
            ClientMessage::PlaybackSeek { revision: 3, target_position_ms: 500, client_timestamp_ms: 100 },
            "playback.seek",
        ),
        (
            ClientMessage::PlaybackSpeed { revision: 4, speed: 1.25, client_timestamp_ms: 100 },
            "playback.speed",
        ),
        (
            ClientMessage::SyncPing { ping_id: 1, client_send_time_ms: 100 },
            "sync.ping",
        ),
        (
            ClientMessage::RoomSetPermissions { permission_mode: PermissionMode::HostOnly },
            "room.set_permissions",
        ),
    ];

    for (msg, expected_type) in event_cases {
        let json_val: Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(
            json_val["type"].as_str().unwrap(),
            expected_type,
            "Event wire type mismatch"
        );
    }
}

#[test]
fn test_tier1_error_codes_serialization() {
    let error_codes = vec![
        (ErrorCode::RoomNotFound, "ROOM_NOT_FOUND"),
        (ErrorCode::RoomFull, "ROOM_FULL"),
        (ErrorCode::InvalidRoomCode, "INVALID_ROOM_CODE"),
        (ErrorCode::InvalidToken, "INVALID_TOKEN"),
        (ErrorCode::PermissionDenied, "PERMISSION_DENIED"),
        (ErrorCode::StaleRevision, "STALE_REVISION"),
        (ErrorCode::ParticipantNotFound, "PARTICIPANT_NOT_FOUND"),
        (ErrorCode::InvalidMessage, "INVALID_MESSAGE"),
        (ErrorCode::InvalidRange, "INVALID_RANGE"),
        (ErrorCode::HostQuit, "HOST_QUIT"),
        (ErrorCode::InternalError, "INTERNAL_ERROR"),
    ];

    for (code, expected_str) in error_codes {
        assert_eq!(code.as_str(), expected_str);
        let serialized = serde_json::to_string(&code).unwrap();
        assert_eq!(serialized, format!("\"{}\"", expected_str));
    }
}

#[test]
fn test_tier1_discrete_playback_speeds_serialization() {
    let speeds = vec![0.5f32, 0.75f32, 1.0f32, 1.25f32, 1.5f32, 2.0f32];
    for speed in speeds {
        let msg = ClientMessage::PlaybackSpeed {
            revision: 1,
            speed,
            client_timestamp_ms: 1000,
        };
        let json_str = serde_json::to_string(&msg).unwrap();
        let decoded: ClientMessage = serde_json::from_str(&json_str).unwrap();
        if let ClientMessage::PlaybackSpeed { speed: dec_speed, .. } = decoded {
            assert!((dec_speed - speed).abs() < 1e-6);
        } else {
            panic!("Decoded variant mismatch");
        }
    }
}

// ============================================================================
// TIER 2: BOUNDARY VALUE ANALYSIS & INVARIANTS
// ============================================================================

#[test]
fn test_tier2_monotonic_revision_tracking_gate() {
    let mut tracker = RevisionTracker::new(10);
    assert_eq!(tracker.current(), 10);

    // Stale command: rev 9 < 10 -> rejected
    assert!(matches!(
        tracker.gate_command(9),
        Err(RevisionError::StaleCommand { incoming: 9, current: 10 })
    ));

    // Stale command: rev 10 == 10 -> rejected
    assert!(matches!(
        tracker.gate_command(10),
        Err(RevisionError::StaleCommand { incoming: 10, current: 10 })
    ));

    // Valid command: rev 11 > 10 -> accepted and advanced
    assert!(tracker.gate_command(11).is_ok());
    let new_rev = tracker.accept_command_and_advance(11).unwrap();
    assert_eq!(new_rev, 11);
    assert_eq!(tracker.current(), 11);

    // After advance, rev 11 is now stale
    assert!(tracker.gate_command(11).is_err());
}

#[test]
fn test_tier2_monotonic_revision_client_broadcast_gate() {
    let mut client_tracker = RevisionTracker::new(5);

    // Duplicate broadcast: rev 5 -> rejected
    assert!(matches!(
        client_tracker.apply_incoming_broadcast(5),
        Err(RevisionError::StaleStateBroadcast { incoming: 5, current: 5 })
    ));

    // Older broadcast arriving late: rev 3 -> rejected
    assert!(matches!(
        client_tracker.apply_incoming_broadcast(3),
        Err(RevisionError::StaleStateBroadcast { incoming: 3, current: 5 })
    ));

    // Newer broadcast: rev 8 -> accepted
    assert!(client_tracker.apply_incoming_broadcast(8).is_ok());
    assert_eq!(client_tracker.current(), 8);

    // Delayed broadcast 7 arriving after 8 was processed -> rejected
    assert!(client_tracker.apply_incoming_broadcast(7).is_err());
}

#[test]
fn test_tier2_revision_counter_overflow_guard() {
    let mut tracker = RevisionTracker::new(u64::MAX);
    assert!(matches!(
        tracker.advance(),
        Err(RevisionError::RevisionOverflow)
    ));
}

#[test]
fn test_tier2_room_code_crockford_charset_boundaries() {
    // Valid Crockford-style uppercase alphanumeric codes (len 6, no 0, O, 1, I)
    let valid_codes = vec!["234567", "ABCDEF", "GHJKLM", "NPQRST", "UVWXYZ", "K7M2P9"];
    for code in valid_codes {
        assert!(RoomCode::new(code).is_valid(), "Code {} should be valid", code);
    }

    // Invalid codes containing ambiguous characters
    let ambiguous_codes = vec![
        "0ABCDE", // Contains '0'
        "OABCDE", // Contains 'O'
        "1ABCDE", // Contains '1'
        "IABCDE", // Contains 'I'
    ];
    for code in ambiguous_codes {
        assert!(!RoomCode::new(code).is_valid(), "Ambiguous code {} should be invalid", code);
    }

    // Invalid lengths
    assert!(!RoomCode::new("").is_valid());
    assert!(!RoomCode::new("ABCDE").is_valid());  // 5 chars
    assert!(!RoomCode::new("ABCDEFG").is_valid()); // 7 chars

    // Invalid lowercase or symbols
    assert!(!RoomCode::new("k7m2p9").is_valid());
    assert!(!RoomCode::new("K7-2P9").is_valid());
    assert!(!RoomCode::new("K7 2P9").is_valid());
}

#[test]
fn test_tier2_room_token_entropy_boundary() {
    // Valid tokens (>= 32 chars)
    let token_32 = "a".repeat(32);
    let token_64 = "b".repeat(64);
    assert!(RoomToken::new(token_32).is_valid());
    assert!(RoomToken::new(token_64).is_valid());

    // Invalid short tokens (< 32 chars)
    let token_31 = "c".repeat(31);
    let token_16 = "d".repeat(16);
    let token_empty = "";
    assert!(!RoomToken::new(token_31).is_valid());
    assert!(!RoomToken::new(token_16).is_valid());
    assert!(!RoomToken::new(token_empty).is_valid());
}

#[test]
fn test_tier2_deserialization_missing_type_tag_fails() {
    let raw_json = json!({
        "payload": {
            "revision": 1,
            "client_timestamp_ms": 100
        }
    });

    let res: Result<ClientMessage, _> = serde_json::from_value(raw_json);
    assert!(res.is_err(), "Must fail when 'type' tag is missing");
}

#[test]
fn test_tier2_deserialization_missing_payload_fails() {
    let raw_json = json!({
        "type": "playback.play"
    });

    let res: Result<ClientMessage, _> = serde_json::from_value(raw_json);
    assert!(res.is_err(), "Must fail when 'payload' field is missing");
}

#[test]
fn test_tier2_deserialization_unknown_type_fails() {
    let raw_json = json!({
        "type": "unknown.custom.event",
        "payload": {}
    });

    let res: Result<ClientMessage, _> = serde_json::from_value(raw_json);
    assert!(res.is_err(), "Must fail on unknown event type");
}

#[test]
fn test_tier2_optional_fields_serialization() {
    // When participant_id is None, it should be omitted from JSON
    let msg_without_id = ClientMessage::RoomJoin {
        room_code: RoomCode::new("K7M2P9"),
        room_token: RoomToken::new("0123456789abcdef0123456789abcdef"),
        display_name: "Alice".into(),
        participant_id: None,
    };
    let json_val = serde_json::to_value(&msg_without_id).unwrap();
    assert!(json_val["payload"].get("participant_id").is_none());

    // When participant_id is Some, it must be present
    let msg_with_id = ClientMessage::RoomJoin {
        room_code: RoomCode::new("K7M2P9"),
        room_token: RoomToken::new("0123456789abcdef0123456789abcdef"),
        display_name: "Alice".into(),
        participant_id: Some(ParticipantId::new("part-001")),
    };
    let json_val_with = serde_json::to_value(&msg_with_id).unwrap();
    assert_eq!(json_val_with["payload"]["participant_id"], "part-001");
}

#[test]
fn test_tier2_extreme_timestamp_and_position_bounds() {
    let msg = ClientMessage::PlaybackSeek {
        revision: u64::MAX - 1,
        target_position_ms: u64::MAX,
        client_timestamp_ms: u64::MAX,
    };
    let json_str = serde_json::to_string(&msg).unwrap();
    let decoded: ClientMessage = serde_json::from_str(&json_str).unwrap();
    assert_eq!(msg, decoded);
}

// Additional Tier 1 Protocol Serialization Tests

#[test]
fn test_tier1_participant_role_and_permission_mode_serialization() {
    assert_eq!(serde_json::to_string(&ParticipantRole::Host).unwrap(), "\"host\"");
    assert_eq!(serde_json::to_string(&ParticipantRole::Guest).unwrap(), "\"guest\"");
    assert_eq!(serde_json::to_string(&PermissionMode::FreeControl).unwrap(), "\"free_control\"");
    assert_eq!(serde_json::to_string(&PermissionMode::HostOnly).unwrap(), "\"host_only\"");
}

#[test]
fn test_tier1_playback_status_serialization_all_three_states() {
    assert_eq!(serde_json::to_string(&PlaybackStatus::Playing).unwrap(), "\"playing\"");
    assert_eq!(serde_json::to_string(&PlaybackStatus::Paused).unwrap(), "\"paused\"");
    assert_eq!(serde_json::to_string(&PlaybackStatus::Buffering).unwrap(), "\"buffering\"");
}

#[test]
fn test_tier1_video_metadata_with_and_without_dimensions() {
    let with_dims = VideoMetadata {
        filename: "test.mp4".into(),
        file_size_bytes: 1000,
        duration_ms: 10000,
        mime_type: "video/mp4".into(),
        width: Some(1920),
        height: Some(1080),
    };
    let json_with = serde_json::to_value(&with_dims).unwrap();
    assert_eq!(json_with["width"], 1920);
    assert_eq!(json_with["height"], 1080);

    let without_dims = VideoMetadata {
        filename: "test.mp4".into(),
        file_size_bytes: 1000,
        duration_ms: 10000,
        mime_type: "video/mp4".into(),
        width: None,
        height: None,
    };
    let json_without = serde_json::to_value(&without_dims).unwrap();
    assert!(json_without.get("width").is_none());
    assert!(json_without.get("height").is_none());
}

#[test]
fn test_tier1_room_code_and_token_display_trait() {
    let code = RoomCode::new("K7M2P9");
    assert_eq!(format!("{}", code), "K7M2P9");

    let token = RoomToken::new("abc123token456");
    assert_eq!(format!("{}", token), "abc123token456");
}

#[test]
fn test_tier1_participant_id_and_room_id_display_trait() {
    let pid = ParticipantId::new("part-001");
    assert_eq!(format!("{}", pid), "part-001");

    let rid = RoomId::new("room-uuid-42");
    assert_eq!(format!("{}", rid), "room-uuid-42");
}

// Additional Tier 2 Boundary & Error Handling Tests

#[test]
fn test_tier2_invalid_crockford_characters_explicit_matrix() {
    // 0, O, 1, I must be rejected in room codes
    for c in &['0', 'O', '1', 'I', 'o', 'i', 'l'] {
        let code_str = format!("23456{}", c);
        assert!(!RoomCode::new(code_str).is_valid(), "Character {} must be invalid in RoomCode", c);
    }
}

#[test]
fn test_tier2_empty_string_identifiers_validation() {
    assert!(!RoomCode::new("").is_valid());
    assert!(!RoomToken::new("").is_valid());
}

#[test]
fn test_tier2_deserialization_unexpected_data_types_fails() {
    // String instead of integer for revision
    let invalid_json = serde_json::json!({
        "type": "playback.play",
        "payload": {
            "revision": "not_a_number",
            "client_timestamp_ms": 100
        }
    });
    let res: Result<ClientMessage, _> = serde_json::from_value(invalid_json);
    assert!(res.is_err());
}

#[test]
fn test_tier2_negative_integers_in_unsigned_fields_fails() {
    let invalid_json = serde_json::json!({
        "type": "playback.pause",
        "payload": {
            "revision": 1,
            "position_ms": -500,
            "client_timestamp_ms": 100
        }
    });
    let res: Result<ClientMessage, _> = serde_json::from_value(invalid_json);
    assert!(res.is_err());
}

#[test]
fn test_tier2_serde_rejects_truncated_json_tokens() {
    let truncated_str = "{\"type\": \"playback.play\", \"payload\": { \"revi";
    let res: Result<ClientMessage, _> = serde_json::from_str(truncated_str);
    assert!(res.is_err());
}
