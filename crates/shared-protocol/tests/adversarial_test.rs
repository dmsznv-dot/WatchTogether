use serde_json::json;
use shared_protocol::*;

// =========================================================================
// 1. Missing Fields Fuzzing
// =========================================================================

#[test]
fn test_missing_fields_client_messages() {
    // 1. Missing top-level envelope fields
    assert!(serde_json::from_str::<ClientMessage>(r#"{}"#).is_err());
    assert!(serde_json::from_str::<ClientMessage>(r#"{"type": "playback.play"}"#).is_err());
    assert!(serde_json::from_str::<ClientMessage>(r#"{"payload": {}}"#).is_err());

    // 2. room.create missing host_name
    let json = json!({
        "type": "room.create",
        "payload": {
            "video": {
                "filename": "test.mp4",
                "file_size_bytes": 100,
                "duration_ms": 100,
                "mime_type": "video/mp4"
            },
            "permission_mode": "free_control"
        }
    });
    assert!(serde_json::from_value::<ClientMessage>(json).is_err());

    // 3. room.create missing video
    let json = json!({
        "type": "room.create",
        "payload": {
            "host_name": "Host",
            "permission_mode": "free_control"
        }
    });
    assert!(serde_json::from_value::<ClientMessage>(json).is_err());

    // 4. room.join missing room_code
    let json = json!({
        "type": "room.join",
        "payload": {
            "room_token": "a".repeat(32),
            "display_name": "Guest"
        }
    });
    assert!(serde_json::from_value::<ClientMessage>(json).is_err());

    // 5. room.join missing room_token
    let json = json!({
        "type": "room.join",
        "payload": {
            "room_code": "ABC234",
            "display_name": "Guest"
        }
    });
    assert!(serde_json::from_value::<ClientMessage>(json).is_err());

    // 6. playback.seek missing target_position_ms
    let json = json!({
        "type": "playback.seek",
        "payload": {
            "revision": 1,
            "client_timestamp_ms": 100
        }
    });
    assert!(serde_json::from_value::<ClientMessage>(json).is_err());

    // 7. playback.speed missing speed
    let json = json!({
        "type": "playback.speed",
        "payload": {
            "revision": 1,
            "client_timestamp_ms": 100
        }
    });
    assert!(serde_json::from_value::<ClientMessage>(json).is_err());

    // 8. participant.kick missing target_id
    let json = json!({
        "type": "participant.kick",
        "payload": {}
    });
    assert!(serde_json::from_value::<ClientMessage>(json).is_err());

    // 9. sync.ping missing client_send_time_ms
    let json = json!({
        "type": "sync.ping",
        "payload": {
            "ping_id": 1
        }
    });
    assert!(serde_json::from_value::<ClientMessage>(json).is_err());
}

#[test]
fn test_missing_fields_server_messages() {
    // 1. playback.updated missing action
    let json = json!({
        "type": "playback.updated",
        "payload": {
            "revision": 1,
            "status": "playing",
            "position_ms": 100,
            "speed": 1.0,
            "updated_at_server_ms": 100,
            "triggered_by": "part-1"
        }
    });
    assert!(serde_json::from_value::<ServerMessage>(json).is_err());

    // 2. sync.pong missing server_send_time_ms
    let json = json!({
        "type": "sync.pong",
        "payload": {
            "ping_id": 1,
            "client_send_time_ms": 10,
            "server_receive_time_ms": 20
        }
    });
    assert!(serde_json::from_value::<ServerMessage>(json).is_err());

    // 3. room.error missing code
    let json = json!({
        "type": "room.error",
        "payload": {
            "message": "Something went wrong"
        }
    });
    assert!(serde_json::from_value::<ServerMessage>(json).is_err());

    // 4. room.closed missing reason
    let json = json!({
        "type": "room.closed",
        "payload": {}
    });
    assert!(serde_json::from_value::<ServerMessage>(json).is_err());
}

// =========================================================================
// 2. Type Confusion & Malformed Payloads
// =========================================================================

#[test]
fn test_type_confusion_rejection() {
    // String where u64 expected
    let json = json!({
        "type": "playback.play",
        "payload": {
            "revision": "42",
            "client_timestamp_ms": 100
        }
    });
    assert!(serde_json::from_value::<ClientMessage>(json).is_err());

    // Number where string expected
    let json = json!({
        "type": "participant.kick",
        "payload": {
            "target_id": 12345
        }
    });
    assert!(serde_json::from_value::<ClientMessage>(json).is_err());

    // Negative integer for unsigned field
    let json = json!({
        "type": "playback.play",
        "payload": {
            "revision": -1,
            "client_timestamp_ms": 100
        }
    });
    assert!(serde_json::from_value::<ClientMessage>(json).is_err());

    // Float for integer field
    let json = json!({
        "type": "playback.play",
        "payload": {
            "revision": 42.5,
            "client_timestamp_ms": 100
        }
    });
    assert!(serde_json::from_value::<ClientMessage>(json).is_err());

    // Null for required field
    let json = json!({
        "type": "playback.pause",
        "payload": {
            "revision": 1,
            "position_ms": null,
            "client_timestamp_ms": 100
        }
    });
    assert!(serde_json::from_value::<ClientMessage>(json).is_err());

    // Array for object payload
    let json = json!({
        "type": "playback.play",
        "payload": [1, 2, 3]
    });
    assert!(serde_json::from_value::<ClientMessage>(json).is_err());

    // Unknown enum variant in permission_mode
    let json = json!({
        "type": "room.set_permissions",
        "payload": {
            "permission_mode": "super_admin_mode"
        }
    });
    assert!(serde_json::from_value::<ClientMessage>(json).is_err());

    // Unknown enum variant in playback status
    let json = json!({
        "type": "playback.updated",
        "payload": {
            "revision": 1,
            "status": "hyperdrive",
            "position_ms": 100,
            "speed": 1.0,
            "updated_at_server_ms": 100,
            "triggered_by": "part-1",
            "action": "play"
        }
    });
    assert!(serde_json::from_value::<ServerMessage>(json).is_err());
}

// =========================================================================
// 3. Oversized Payloads & String Length Robustness
// =========================================================================

#[test]
fn test_oversized_string_handling() {
    let large_string = "X".repeat(1_000_000); // 1 MB string

    // 1. Massive display name in room.join
    let json = json!({
        "type": "room.join",
        "payload": {
            "room_code": "ABC234",
            "room_token": "a".repeat(32),
            "display_name": large_string
        }
    });
    let parsed: Result<ClientMessage, _> = serde_json::from_value(json);
    assert!(parsed.is_ok(), "Serde must safely parse 1MB string without crash");

    // 2. Massive error message
    let json = json!({
        "type": "room.error",
        "payload": {
            "code": "INTERNAL_ERROR",
            "message": large_string
        }
    });
    let parsed: Result<ServerMessage, _> = serde_json::from_value(json);
    assert!(parsed.is_ok(), "Serde must safely parse 1MB error message without crash");
}

// =========================================================================
// 4. Out-of-Bounds & Extreme Speed Values
// =========================================================================

#[test]
fn test_speed_edge_values() {
    // 1. Extreme float values: negative speed, zero speed, huge speed
    let extreme_speeds = [-10.0f32, -0.5f32, 0.0f32, 100.0f32, 1_000_000.0f32];

    for s in extreme_speeds {
        let msg = ClientMessage::PlaybackSpeed {
            revision: 10,
            speed: s,
            client_timestamp_ms: 1000,
        };
        let serialized = serde_json::to_string(&msg).expect("Serialize float");
        let deserialized: ClientMessage = serde_json::from_str(&serialized).expect("Deserialize float");
        assert_eq!(msg, deserialized);
    }

    // 2. JSON specification forbids raw NaN and Infinity
    let nan_json = r#"{"type": "playback.speed", "payload": {"revision": 1, "speed": NaN, "client_timestamp_ms": 100}}"#;
    assert!(serde_json::from_str::<ClientMessage>(nan_json).is_err());

    let inf_json = r#"{"type": "playback.speed", "payload": {"revision": 1, "speed": Infinity, "client_timestamp_ms": 100}}"#;
    assert!(serde_json::from_str::<ClientMessage>(inf_json).is_err());
}

// =========================================================================
// 5. Domain Boundary Validations (RoomCode, RoomToken, ErrorCode)
// =========================================================================

#[test]
fn test_room_code_adversarial_validation() {
    // Valid Crockford-style codes
    assert!(RoomCode::new("234567").is_valid());
    assert!(RoomCode::new("ABCDEF").is_valid());
    assert!(RoomCode::new("HJNPXZ").is_valid());

    // Disallowed ambiguous characters: 0, O, 1, I
    assert!(!RoomCode::new("234560").is_valid(), "0 disallowed");
    assert!(!RoomCode::new("23456O").is_valid(), "O disallowed");
    assert!(!RoomCode::new("234561").is_valid(), "1 disallowed");
    assert!(!RoomCode::new("23456I").is_valid(), "I disallowed");

    // Disallowed lowercase
    assert!(!RoomCode::new("abcdef").is_valid(), "lowercase disallowed");
    assert!(!RoomCode::new("Abcdef").is_valid(), "mixed case disallowed");

    // Invalid lengths
    assert!(!RoomCode::new("").is_valid(), "empty disallowed");
    assert!(!RoomCode::new("ABCDE").is_valid(), "5 chars disallowed");
    assert!(!RoomCode::new("ABCDEFG").is_valid(), "7 chars disallowed");

    // Special characters & whitespace
    assert!(!RoomCode::new("ABC 23").is_valid());
    assert!(!RoomCode::new("ABC-23").is_valid());
    assert!(!RoomCode::new("ABC_23").is_valid());
}

#[test]
fn test_room_token_adversarial_validation() {
    // 31 characters: exactly 1 char short of 256 bits (32 chars)
    let token_31 = "a".repeat(31);
    assert!(!RoomToken::new(&token_31).is_valid());

    // Empty token
    assert!(!RoomToken::new("").is_valid());

    // Exactly 32 characters
    let token_32 = "a".repeat(32);
    assert!(RoomToken::new(&token_32).is_valid());

    // 64 characters (SHA256 hex length)
    let token_64 = "a".repeat(64);
    assert!(RoomToken::new(&token_64).is_valid());
}

#[test]
fn test_all_protocol_error_mappings() {
    let mappings = [
        (ProtocolError::RoomNotFound("r1".into()), ErrorCode::RoomNotFound, "ROOM_NOT_FOUND"),
        (ProtocolError::RoomFull { capacity: 5 }, ErrorCode::RoomFull, "ROOM_FULL"),
        (ProtocolError::InvalidRoomCode("c".into()), ErrorCode::InvalidRoomCode, "INVALID_ROOM_CODE"),
        (ProtocolError::InvalidToken, ErrorCode::InvalidToken, "INVALID_TOKEN"),
        (ProtocolError::PermissionDenied("d".into()), ErrorCode::PermissionDenied, "PERMISSION_DENIED"),
        (ProtocolError::StaleRevision { command_revision: 1, current_revision: 2 }, ErrorCode::StaleRevision, "STALE_REVISION"),
        (ProtocolError::ParticipantNotFound("p1".into()), ErrorCode::ParticipantNotFound, "PARTICIPANT_NOT_FOUND"),
        (ProtocolError::Serialization("s".into()), ErrorCode::InvalidMessage, "INVALID_MESSAGE"),
        (ProtocolError::InvalidMessage("m".into()), ErrorCode::InvalidMessage, "INVALID_MESSAGE"),
        (ProtocolError::InvalidRangeHeader("h".into()), ErrorCode::InvalidRange, "INVALID_RANGE"),
    ];

    for (domain_err, expected_code, expected_str) in mappings {
        assert_eq!(domain_err.error_code(), expected_code);
        assert_eq!(expected_code.as_str(), expected_str);
    }
}
