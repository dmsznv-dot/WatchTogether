use shared_protocol::*;

#[test]
fn test_all_client_messages_roundtrip() {
    let video = VideoMetadata {
        filename: "matrix.mp4".into(),
        file_size_bytes: 1_048_576_000,
        duration_ms: 7_200_000,
        mime_type: "video/mp4".into(),
        width: Some(1920),
        height: Some(1080),
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
            target_id: ParticipantId::new("part-002"),
        },
        ClientMessage::RoomLeave {
            participant_id: ParticipantId::new("part-002"),
        },
    ];

    for original in client_messages {
        let json_str = serde_json::to_string(&original).expect("Serialization failed");
        // Verify envelope keys exist
        let val: serde_json::Value = serde_json::from_str(&json_str).expect("Valid JSON");
        assert!(val.get("type").is_some(), "Must contain 'type' field: {}", json_str);
        assert!(val.get("payload").is_some(), "Must contain 'payload' field: {}", json_str);

        let deserialized: ClientMessage = serde_json::from_str(&json_str).expect("Deserialization failed");
        assert_eq!(original, deserialized);
    }
}

#[test]
fn test_all_server_messages_roundtrip() {
    let participant = Participant {
        id: ParticipantId::new("part-001"),
        display_name: "Alice".into(),
        role: ParticipantRole::Host,
        is_connected: true,
        buffered_position_ms: 50_000,
        current_position_ms: 45_000,
        ping_ms: 12,
        joined_at_ms: 1_700_000_000_000,
    };

    let playback = PlaybackState {
        revision: 10,
        status: PlaybackStatus::Playing,
        position_ms: 45_000,
        speed: 1.0,
        updated_at_server_ms: 1_700_000_000_050,
        duration_ms: 3_600_000,
    };

    let video = VideoMetadata {
        filename: "sample.mkv".into(),
        file_size_bytes: 500_000_000,
        duration_ms: 3_600_000,
        mime_type: "video/x-matroska".into(),
        width: None,
        height: None,
    };

    let snapshot = RoomStateSnapshot {
        room_id: RoomId::new("room-uuid-1234"),
        room_code: RoomCode::new("R4Z8K2"),
        host_id: ParticipantId::new("part-001"),
        permission_mode: PermissionMode::FreeControl,
        participants: vec![participant.clone()],
        playback: playback.clone(),
        video: video.clone(),
    };

    let server_messages = vec![
        ServerMessage::RoomState(snapshot),
        ServerMessage::PlaybackUpdated {
            revision: 11,
            status: PlaybackStatus::Paused,
            position_ms: 46_000,
            speed: 1.0,
            updated_at_server_ms: 1_700_000_000_100,
            triggered_by: ParticipantId::new("part-001"),
            action: "pause".into(),
        },
        ServerMessage::ParticipantJoined(participant.clone()),
        ServerMessage::ParticipantLeft {
            participant_id: ParticipantId::new("part-002"),
            reason: "connection_timeout".into(),
        },
        ServerMessage::ParticipantKicked {
            participant_id: ParticipantId::new("part-002"),
            reason: "kicked_by_host".into(),
        },
        ServerMessage::SyncPong {
            ping_id: 101,
            client_send_time_ms: 1_700_000_000_000,
            server_receive_time_ms: 1_700_000_000_015,
            server_send_time_ms: 1_700_000_000_016,
        },
        ServerMessage::RoomClosed {
            reason: "host_quit".into(),
        },
        ServerMessage::Error {
            code: "ROOM_FULL".into(),
            message: "Room capacity reached (max 5)".into(),
        },
    ];

    for original in server_messages {
        let json_str = serde_json::to_string(&original).expect("Serialization failed");
        let val: serde_json::Value = serde_json::from_str(&json_str).expect("Valid JSON");
        assert!(val.get("type").is_some(), "Must contain 'type' field: {}", json_str);
        assert!(val.get("payload").is_some(), "Must contain 'payload' field: {}", json_str);

        let deserialized: ServerMessage = serde_json::from_str(&json_str).expect("Deserialization failed");
        assert_eq!(original, deserialized);
    }
}

#[test]
fn test_wire_json_exact_matching() {
    let raw_join_json = r#"{
        "type": "room.join",
        "payload": {
            "room_code": "ABC234",
            "room_token": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "display_name": "Charlie"
        }
    }"#;

    let msg: ClientMessage = serde_json::from_str(raw_join_json).expect("Must parse standard JSON");
    match msg {
        ClientMessage::RoomJoin { room_code, room_token, display_name, participant_id } => {
            assert_eq!(room_code.as_str(), "ABC234");
            assert!(room_code.is_valid());
            assert!(room_token.is_valid());
            assert_eq!(display_name, "Charlie");
            assert_eq!(participant_id, None);
        }
        _ => panic!("Expected RoomJoin variant"),
    }
}

#[test]
fn test_invalid_json_rejection() {
    // Missing payload
    let bad_json_1 = r#"{"type": "playback.play"}"#;
    assert!(serde_json::from_str::<ClientMessage>(bad_json_1).is_err());

    // Unknown type
    let bad_json_2 = r#"{"type": "non.existent.event", "payload": {}}"#;
    assert!(serde_json::from_str::<ClientMessage>(bad_json_2).is_err());

    // Malformed enum value
    let bad_json_3 = r#"{
        "type": "room.create",
        "payload": {
            "host_name": "Alice",
            "video": { "filename": "v.mp4", "file_size_bytes": 10, "duration_ms": 10, "mime_type": "video/mp4" },
            "permission_mode": "invalid_mode"
        }
    }"#;
    assert!(serde_json::from_str::<ClientMessage>(bad_json_3).is_err());
}

#[test]
fn test_room_code_validation() {
    assert!(RoomCode::new("ABC234").is_valid());
    assert!(RoomCode::new("K7M2P9").is_valid());
    // 0, O, 1, I are disallowed
    assert!(!RoomCode::new("AB0234").is_valid());
    assert!(!RoomCode::new("ABO234").is_valid());
    assert!(!RoomCode::new("AB1234").is_valid());
    assert!(!RoomCode::new("ABI234").is_valid());
    // Length must be exactly 6
    assert!(!RoomCode::new("ABC23").is_valid());
    assert!(!RoomCode::new("ABC2345").is_valid());
}

#[test]
fn test_room_token_validation() {
    assert!(RoomToken::new("12345678901234567890123456789012").is_valid()); // 32 chars
    assert!(RoomToken::new("12345678901234567890123456789012345").is_valid()); // 35 chars
    assert!(!RoomToken::new("1234567890123456789012345678901").is_valid()); // 31 chars
}

#[test]
fn test_error_code_mapping() {
    let err = ProtocolError::RoomFull { capacity: 5 };
    assert_eq!(err.error_code(), ErrorCode::RoomFull);
    assert_eq!(ErrorCode::RoomFull.as_str(), "ROOM_FULL");

    let err2 = ProtocolError::StaleRevision {
        command_revision: 5,
        current_revision: 10,
    };
    assert_eq!(err2.error_code(), ErrorCode::StaleRevision);
    assert_eq!(ErrorCode::StaleRevision.as_str(), "STALE_REVISION");
}
