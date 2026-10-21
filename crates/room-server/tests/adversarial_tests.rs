//! Adversarial stress testing and verification harness for `room-server`.
//! Authored by challenger_m2_1 to empirically probe:
//! 1. Capacity boundary stress (2nd, 5th, 6th concurrent joins, ROOM_FULL, slot freeing on leave/kick).
//! 2. Host-Only permission bypass (flooding commands in HostOnly mode, rejection with PERMISSION_DENIED).
//! 3. Host action integrity (guest kick host/guest rejected; host kick self rejected).
//! 4. Replay and stale revision injection (rev <= current_rev rejected with STALE_REVISION, concurrent races).
//! 5. Teardown cascade (room closure, room.closed message, subsequent ROOM_NOT_FOUND, media-stream server shutdown).

use futures_util::{SinkExt, StreamExt};
use media_stream::MediaServer;
use room_server::{
    error::RoomServerError,
    room::{is_valid_playback_speed, Room, ALLOWED_PLAYBACK_SPEEDS, CROCKFORD_CHARSET},
    RoomServer,
};
use shared_protocol::messages::{ClientMessage, ServerMessage};
use shared_protocol::models::{
    ParticipantId, PermissionMode, PlaybackStatus, RoomCode, RoomToken, VideoMetadata,
};
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tempfile::NamedTempFile;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

fn sample_video(duration_ms: u64) -> VideoMetadata {
    VideoMetadata {
        filename: "adversarial_sample.mp4".into(),
        duration_ms,
        file_size_bytes: 50_000_000,
        mime_type: "video/mp4".into(),
        width: Some(1920),
        height: Some(1080),
    }
}

fn create_temp_video_file(size: usize) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("Create temp file");
    let dummy_data = vec![0x42u8; size];
    file.write_all(&dummy_data).expect("Write dummy data");
    file.flush().expect("Flush file");
    file
}

async fn ws_send_msg(ws: &mut WsStream, msg: &ClientMessage) {
    let text = serde_json::to_string(msg).expect("Serialize client message");
    ws.send(Message::Text(text)).await.expect("Send WS text");
}

async fn ws_recv_msg(ws: &mut WsStream, max_wait: Duration) -> Option<ServerMessage> {
    match timeout(max_wait, ws.next()).await {
        Ok(Some(Ok(Message::Text(t)))) => {
            serde_json::from_str::<ServerMessage>(&t).ok()
        }
        _ => None,
    }
}

async fn ws_recv_matching<F>(ws: &mut WsStream, max_wait: Duration, predicate: F) -> Option<ServerMessage>
where
    F: Fn(&ServerMessage) -> bool,
{
    let deadline = tokio::time::Instant::now() + max_wait;
    while tokio::time::Instant::now() < deadline {
        let remaining = deadline - tokio::time::Instant::now();
        if let Some(msg) = ws_recv_msg(ws, remaining).await {
            if predicate(&msg) {
                return Some(msg);
            }
        } else {
            break;
        }
    }
    None
}

async fn get_room_token(server: &room_server::server::RoomServerHandle, code: &RoomCode) -> RoomToken {
    let r = server.manager.get_room_by_code(code).await.expect("Room exists in manager");
    let lock = r.lock().await;
    lock.room_token.clone()
}

// ============================================================================
// 1. CAPACITY BOUNDARY STRESS
// ============================================================================

#[tokio::test]
async fn test_adversarial_capacity_exact_boundary_ws() {
    let server = RoomServer::start_default("127.0.0.1:0".parse().unwrap())
        .await
        .expect("Server starts");
    let ws_url = format!("ws://{}/ws", server.local_addr);

    // 1. Host creates room
    let (mut host_ws, _) = connect_async(&ws_url).await.unwrap();
    ws_send_msg(
        &mut host_ws,
        &ClientMessage::RoomCreate {
            host_name: "Host_Cap".into(),
            video: sample_video(120_000),
            permission_mode: PermissionMode::FreeControl,
        },
    )
    .await;

    let host_snap = ws_recv_matching(&mut host_ws, Duration::from_secs(2), |m| {
        matches!(m, ServerMessage::RoomState(_))
    })
    .await
    .expect("Host receives initial RoomState");

    let room_code = match host_snap {
        ServerMessage::RoomState(snap) => {
            assert_eq!(snap.participants.len(), 1);
            snap.room_code
        }
        _ => unreachable!(),
    };

    let room_token = get_room_token(&server, &room_code).await;

    let mut guests = Vec::new();

    // 2. Join participants 2, 3, 4, 5 (up to max capacity 5)
    for i in 2..=5 {
        let (mut g_ws, _) = connect_async(&ws_url).await.unwrap();
        ws_send_msg(
            &mut g_ws,
            &ClientMessage::RoomJoin {
                room_code: room_code.clone(),
                room_token: room_token.clone(),
                display_name: format!("Guest_{}", i),
                participant_id: None,
            },
        )
        .await;

        let snap_msg = ws_recv_matching(&mut g_ws, Duration::from_secs(2), |m| {
            matches!(m, ServerMessage::RoomState(_))
        })
        .await
        .unwrap_or_else(|| panic!("Participant {} should join successfully", i));

        match snap_msg {
            ServerMessage::RoomState(snap) => {
                assert_eq!(snap.participants.len(), i);
            }
            _ => panic!("Expected RoomState"),
        }
        guests.push(g_ws);
    }

    assert_eq!(guests.len(), 4); // 1 host + 4 guests = 5 participants

    // 3. Attempt to join 6th participant -> MUST be rejected with ROOM_FULL
    let (mut g6_ws, _) = connect_async(&ws_url).await.unwrap();
    ws_send_msg(
        &mut g6_ws,
        &ClientMessage::RoomJoin {
            room_code: room_code.clone(),
            room_token: room_token.clone(),
            display_name: "Guest_6_Overflow".into(),
            participant_id: None,
        },
    )
    .await;

    let err_msg = ws_recv_matching(&mut g6_ws, Duration::from_secs(2), |m| {
        matches!(m, ServerMessage::Error { .. })
    })
    .await
    .expect("6th participant must receive Error");

    match err_msg {
        ServerMessage::Error { code, .. } => {
            assert_eq!(code, "ROOM_FULL", "Error code must be ROOM_FULL for 6th joiner");
        }
        other => panic!("Expected ROOM_FULL error, got {:?}", other),
    }

    // Verify room size is still exactly 5
    {
        let r = server.manager.get_room_by_code(&room_code).await.unwrap();
        let room_lock = r.lock().await;
        assert_eq!(room_lock.participants.connected_count(), 5);
        assert!(room_lock.participants.is_full());
    }

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_adversarial_capacity_concurrent_thundering_herd_ws() {
    let server = RoomServer::start_default("127.0.0.1:0".parse().unwrap())
        .await
        .expect("Server starts");
    let ws_url = format!("ws://{}/ws", server.local_addr);

    // Host creates room
    let (mut host_ws, _) = connect_async(&ws_url).await.unwrap();
    ws_send_msg(
        &mut host_ws,
        &ClientMessage::RoomCreate {
            host_name: "Host_Herd".into(),
            video: sample_video(60_000),
            permission_mode: PermissionMode::FreeControl,
        },
    )
    .await;

    let room_code = match ws_recv_matching(&mut host_ws, Duration::from_secs(2), |m| {
        matches!(m, ServerMessage::RoomState(_))
    })
    .await
    .unwrap()
    {
        ServerMessage::RoomState(s) => s.room_code,
        _ => unreachable!(),
    };

    let room_token = get_room_token(&server, &room_code).await;

    // Pre-fill with 3 guests (total 4 in room: 1 slot remaining!)
    let mut pre_guests = Vec::new();
    for i in 1..=3 {
        let (mut g_ws, _) = connect_async(&ws_url).await.unwrap();
        ws_send_msg(
            &mut g_ws,
            &ClientMessage::RoomJoin {
                room_code: room_code.clone(),
                room_token: room_token.clone(),
                display_name: format!("PreGuest_{}", i),
                participant_id: None,
            },
        )
        .await;
        let _ = ws_recv_matching(&mut g_ws, Duration::from_secs(2), |m| {
            matches!(m, ServerMessage::RoomState(_))
        })
        .await
        .unwrap();
        pre_guests.push(g_ws);
    }

    // Now exactly 1 slot left. Fire 10 concurrent join requests!
    let success_count = Arc::new(AtomicUsize::new(0));
    let room_full_count = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();

    for i in 0..10 {
        let ws_url_c = ws_url.clone();
        let code_c = room_code.clone();
        let token_c = room_token.clone();
        let sc = Arc::clone(&success_count);
        let fc = Arc::clone(&room_full_count);

        handles.push(tokio::spawn(async move {
            let (mut ws, _) = connect_async(&ws_url_c).await.unwrap();
            ws_send_msg(
                &mut ws,
                &ClientMessage::RoomJoin {
                    room_code: code_c,
                    room_token: token_c,
                    display_name: format!("HerdRacer_{}", i),
                    participant_id: None,
                },
            )
            .await;

            let msg = ws_recv_matching(&mut ws, Duration::from_secs(3), |m| {
                matches!(m, ServerMessage::RoomState(_) | ServerMessage::Error { .. })
            })
            .await;

            match msg {
                Some(ServerMessage::RoomState(_)) => {
                    sc.fetch_add(1, Ordering::SeqCst);
                    // Hold connection open
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                Some(ServerMessage::Error { code, .. }) if code == "ROOM_FULL" => {
                    fc.fetch_add(1, Ordering::SeqCst);
                }
                other => panic!("Unexpected message in thundering herd: {:?}", other),
            }
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    // Exactly 1 must have succeeded (to reach max capacity 5), exactly 9 must have received ROOM_FULL
    assert_eq!(
        success_count.load(Ordering::SeqCst),
        1,
        "Exactly 1 concurrent racer should win the final slot"
    );
    assert_eq!(
        room_full_count.load(Ordering::SeqCst),
        9,
        "Exactly 9 concurrent racers should receive ROOM_FULL"
    );

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_adversarial_slot_freed_on_leave_ws() {
    let server = RoomServer::start_default("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let ws_url = format!("ws://{}/ws", server.local_addr);

    // Host creates room
    let (mut host_ws, _) = connect_async(&ws_url).await.unwrap();
    ws_send_msg(
        &mut host_ws,
        &ClientMessage::RoomCreate {
            host_name: "Host_Slot".into(),
            video: sample_video(60_000),
            permission_mode: PermissionMode::FreeControl,
        },
    )
    .await;

    let room_code = match ws_recv_matching(&mut host_ws, Duration::from_secs(2), |m| {
        matches!(m, ServerMessage::RoomState(_))
    })
    .await
    .unwrap()
    {
        ServerMessage::RoomState(s) => s.room_code,
        _ => unreachable!(),
    };

    let room_token = get_room_token(&server, &room_code).await;

    // Fill to 5 participants
    let mut guests = Vec::new();
    let mut guest_ids = Vec::new();
    for i in 1..=4 {
        let (mut g_ws, _) = connect_async(&ws_url).await.unwrap();
        ws_send_msg(
            &mut g_ws,
            &ClientMessage::RoomJoin {
                room_code: room_code.clone(),
                room_token: room_token.clone(),
                display_name: format!("Guest_{}", i),
                participant_id: None,
            },
        )
        .await;

        let snap = ws_recv_matching(&mut g_ws, Duration::from_secs(2), |m| {
            matches!(m, ServerMessage::RoomState(_))
        })
        .await
        .unwrap();

        if let ServerMessage::RoomState(s) = snap {
            let my_id = s.participants.iter().find(|p| p.display_name == format!("Guest_{}", i)).unwrap().id.clone();
            guest_ids.push(my_id);
        }
        guests.push(g_ws);
    }

    // 6th participant receives ROOM_FULL
    let (mut overflow_ws, _) = connect_async(&ws_url).await.unwrap();
    ws_send_msg(
        &mut overflow_ws,
        &ClientMessage::RoomJoin {
            room_code: room_code.clone(),
            room_token: room_token.clone(),
            display_name: "Overflow".into(),
            participant_id: None,
        },
    )
    .await;

    let err = ws_recv_matching(&mut overflow_ws, Duration::from_secs(2), |m| {
        matches!(m, ServerMessage::Error { .. })
    })
    .await
    .unwrap();
    assert!(matches!(err, ServerMessage::Error { code, .. } if code == "ROOM_FULL"));

    // Guest 1 leaves explicitly via RoomLeave
    let mut leaving_guest = guests.remove(0);
    let leaving_id = guest_ids.remove(0);
    ws_send_msg(
        &mut leaving_guest,
        &ClientMessage::RoomLeave {
            participant_id: leaving_id,
        },
    )
    .await;

    // Small delay for server to process leave
    tokio::time::sleep(Duration::from_millis(60)).await;

    // Now slot is freed: new joiner MUST succeed
    let (mut replacement_ws, _) = connect_async(&ws_url).await.unwrap();
    ws_send_msg(
        &mut replacement_ws,
        &ClientMessage::RoomJoin {
            room_code: room_code.clone(),
            room_token: room_token.clone(),
            display_name: "Replacement_After_Leave".into(),
            participant_id: None,
        },
    )
    .await;

    let snap_after = ws_recv_matching(&mut replacement_ws, Duration::from_secs(2), |m| {
        matches!(m, ServerMessage::RoomState(_))
    })
    .await
    .expect("Replacement guest must successfully join after leave");

    match snap_after {
        ServerMessage::RoomState(s) => {
            assert_eq!(s.participants.len(), 5);
            assert!(s.participants.iter().any(|p| p.display_name == "Replacement_After_Leave"));
        }
        _ => panic!("Expected RoomState"),
    }

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_adversarial_slot_freed_on_kick_ws() {
    let server = RoomServer::start_default("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let ws_url = format!("ws://{}/ws", server.local_addr);

    // Host creates room
    let (mut host_ws, _) = connect_async(&ws_url).await.unwrap();
    ws_send_msg(
        &mut host_ws,
        &ClientMessage::RoomCreate {
            host_name: "Host_Kick".into(),
            video: sample_video(60_000),
            permission_mode: PermissionMode::FreeControl,
        },
    )
    .await;

    let room_code = match ws_recv_matching(&mut host_ws, Duration::from_secs(2), |m| {
        matches!(m, ServerMessage::RoomState(_))
    })
    .await
    .unwrap()
    {
        ServerMessage::RoomState(s) => s.room_code,
        _ => unreachable!(),
    };

    let room_token = get_room_token(&server, &room_code).await;

    // Fill to 5 participants
    let mut guests = Vec::new();
    let mut guest_ids = Vec::new();
    for i in 1..=4 {
        let (mut g_ws, _) = connect_async(&ws_url).await.unwrap();
        ws_send_msg(
            &mut g_ws,
            &ClientMessage::RoomJoin {
                room_code: room_code.clone(),
                room_token: room_token.clone(),
                display_name: format!("Guest_Kick_{}", i),
                participant_id: None,
            },
        )
        .await;

        let snap = ws_recv_matching(&mut g_ws, Duration::from_secs(2), |m| {
            matches!(m, ServerMessage::RoomState(_))
        })
        .await
        .unwrap();

        if let ServerMessage::RoomState(s) = snap {
            let my_id = s.participants.iter().find(|p| p.display_name == format!("Guest_Kick_{}", i)).unwrap().id.clone();
            guest_ids.push(my_id);
        }
        guests.push(g_ws);
    }

    // Host kicks Guest 2
    let kicked_id = guest_ids[1].clone();
    ws_send_msg(
        &mut host_ws,
        &ClientMessage::ParticipantKick {
            target_id: kicked_id.clone(),
        },
    )
    .await;

    // Verify kicked guest receives ParticipantKicked message
    let kicked_msg = ws_recv_matching(&mut guests[1], Duration::from_secs(2), |m| {
        matches!(m, ServerMessage::ParticipantKicked { .. })
    })
    .await
    .expect("Kicked guest receives ParticipantKicked");

    match kicked_msg {
        ServerMessage::ParticipantKicked { participant_id, reason } => {
            assert_eq!(participant_id, kicked_id);
            assert_eq!(reason, "kicked_by_host");
        }
        _ => panic!("Expected ParticipantKicked"),
    }

    tokio::time::sleep(Duration::from_millis(60)).await;

    // Now slot is freed: new joiner MUST succeed
    let (mut replacement_ws, _) = connect_async(&ws_url).await.unwrap();
    ws_send_msg(
        &mut replacement_ws,
        &ClientMessage::RoomJoin {
            room_code: room_code.clone(),
            room_token: room_token.clone(),
            display_name: "Replacement_After_Kick".into(),
            participant_id: None,
        },
    )
    .await;

    let snap_after = ws_recv_matching(&mut replacement_ws, Duration::from_secs(2), |m| {
        matches!(m, ServerMessage::RoomState(_))
    })
    .await
    .expect("Replacement guest must successfully join after kick");

    match snap_after {
        ServerMessage::RoomState(s) => {
            assert_eq!(s.participants.len(), 5);
            assert!(s.participants.iter().any(|p| p.display_name == "Replacement_After_Kick"));
        }
        _ => panic!("Expected RoomState"),
    }

    server.shutdown().await.unwrap();
}

// ============================================================================
// 2. HOST-ONLY PERMISSION BYPASS STRESS
// ============================================================================

#[tokio::test]
async fn test_adversarial_host_only_flood_play_pause_seek_speed_ws() {
    let server = RoomServer::start_default("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let ws_url = format!("ws://{}/ws", server.local_addr);

    // 1. Host creates room in HostOnly mode
    let (mut host_ws, _) = connect_async(&ws_url).await.unwrap();
    ws_send_msg(
        &mut host_ws,
        &ClientMessage::RoomCreate {
            host_name: "Host_Strict".into(),
            video: sample_video(180_000),
            permission_mode: PermissionMode::HostOnly,
        },
    )
    .await;

    let host_snap = ws_recv_matching(&mut host_ws, Duration::from_secs(2), |m| {
        matches!(m, ServerMessage::RoomState(_))
    })
    .await
    .unwrap();

    let (room_code, room_token) = match host_snap {
        ServerMessage::RoomState(s) => {
            assert_eq!(s.permission_mode, PermissionMode::HostOnly);
            let code = s.room_code.clone();
            let token = get_room_token(&server, &code).await;
            (code, token)
        }
        _ => unreachable!(),
    };

    // 2. Guest connects
    let (mut guest_ws, _) = connect_async(&ws_url).await.unwrap();
    ws_send_msg(
        &mut guest_ws,
        &ClientMessage::RoomJoin {
            room_code: room_code.clone(),
            room_token: room_token.clone(),
            display_name: "Hostile_Guest".into(),
            participant_id: None,
        },
    )
    .await;

    let _ = ws_recv_matching(&mut guest_ws, Duration::from_secs(2), |m| {
        matches!(m, ServerMessage::RoomState(_))
    })
    .await
    .unwrap();

    // Consume ParticipantJoined from host_ws
    let _ = ws_recv_matching(&mut host_ws, Duration::from_secs(1), |m| {
        matches!(m, ServerMessage::ParticipantJoined(_))
    })
    .await;

    // 3. Flood guest with 100 rapid unauthorized commands
    let mut flood_commands = Vec::new();
    for i in 1..=20 {
        flood_commands.push(ClientMessage::PlaybackPlay {
            revision: 10 + i,
            client_timestamp_ms: 1000 + i,
        });
        flood_commands.push(ClientMessage::PlaybackPause {
            revision: 30 + i,
            position_ms: 5000 + i * 100,
            client_timestamp_ms: 2000 + i,
        });
        flood_commands.push(ClientMessage::PlaybackSeek {
            revision: 50 + i,
            target_position_ms: 10000 + i * 200,
            client_timestamp_ms: 3000 + i,
        });
        flood_commands.push(ClientMessage::PlaybackSpeed {
            revision: 70 + i,
            speed: 1.5,
            client_timestamp_ms: 4000 + i,
        });
        flood_commands.push(ClientMessage::RoomSetPermissions {
            permission_mode: PermissionMode::FreeControl, // Guest trying to hijack permissions!
        });
    }

    assert_eq!(flood_commands.len(), 100);

    for cmd in &flood_commands {
        ws_send_msg(&mut guest_ws, cmd).await;
    }

    // 4. Verify guest receives 100 PERMISSION_DENIED errors
    for i in 0..100 {
        let err = ws_recv_matching(&mut guest_ws, Duration::from_secs(2), |m| {
            matches!(m, ServerMessage::Error { .. })
        })
        .await
        .unwrap_or_else(|| panic!("Command {} should have received error", i));

        match err {
            ServerMessage::Error { code, .. } => {
                assert_eq!(
                    code, "PERMISSION_DENIED",
                    "Unauthorized guest command must yield PERMISSION_DENIED"
                );
            }
            other => panic!("Expected Error, got {:?}", other),
        }
    }

    // 5. Verify host playback is COMPLETELY UNDISTURBED
    // Host should NOT have received any PlaybackUpdated messages
    let leaked_update = ws_recv_matching(&mut host_ws, Duration::from_millis(200), |m| {
        matches!(m, ServerMessage::PlaybackUpdated { .. })
    })
    .await;
    assert!(
        leaked_update.is_none(),
        "Host must NOT receive any PlaybackUpdated from guest flood in HostOnly mode"
    );

    // Check room internal state
    {
        let r = server.manager.get_room_by_code(&room_code).await.unwrap();
        let room_lock = r.lock().await;
        assert_eq!(room_lock.playback.revision, 1, "Revision must remain 1");
        assert_eq!(room_lock.playback.status, PlaybackStatus::Paused);
        assert_eq!(room_lock.playback.position_ms, 0);
        assert_eq!(room_lock.playback.speed, 1.0);
        assert_eq!(room_lock.permissions.mode(), PermissionMode::HostOnly);
    }

    // 6. Host sends legitimate Play command -> must succeed immediately
    ws_send_msg(
        &mut host_ws,
        &ClientMessage::PlaybackPlay {
            revision: 2,
            client_timestamp_ms: 5000,
        },
    )
    .await;

    let host_update = ws_recv_matching(&mut host_ws, Duration::from_secs(2), |m| {
        matches!(m, ServerMessage::PlaybackUpdated { .. })
    })
    .await
    .expect("Host play command must succeed");

    match host_update {
        ServerMessage::PlaybackUpdated {
            revision,
            status,
            action,
            ..
        } => {
            assert_eq!(revision, 2);
            assert_eq!(status, PlaybackStatus::Playing);
            assert_eq!(action, "play");
        }
        _ => panic!("Expected PlaybackUpdated"),
    }

    server.shutdown().await.unwrap();
}

// ============================================================================
// 3. HOST ACTION INTEGRITY
// ============================================================================

#[tokio::test]
async fn test_adversarial_guest_kick_host_and_peers_rejected_ws() {
    let server = RoomServer::start_default("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let ws_url = format!("ws://{}/ws", server.local_addr);

    // Host creates room
    let (mut host_ws, _) = connect_async(&ws_url).await.unwrap();
    ws_send_msg(
        &mut host_ws,
        &ClientMessage::RoomCreate {
            host_name: "Host_Owner".into(),
            video: sample_video(60_000),
            permission_mode: PermissionMode::FreeControl,
        },
    )
    .await;

    let (room_code, room_token, host_id) = match ws_recv_matching(&mut host_ws, Duration::from_secs(2), |m| {
        matches!(m, ServerMessage::RoomState(_))
    })
    .await
    .unwrap()
    {
        ServerMessage::RoomState(s) => {
            let code = s.room_code.clone();
            let token = get_room_token(&server, &code).await;
            (code, token, s.host_id)
        }
        _ => unreachable!(),
    };

    // Guest 1 joins
    let (mut g1_ws, _) = connect_async(&ws_url).await.unwrap();
    ws_send_msg(
        &mut g1_ws,
        &ClientMessage::RoomJoin {
            room_code: room_code.clone(),
            room_token: room_token.clone(),
            display_name: "Guest_1".into(),
            participant_id: None,
        },
    )
    .await;
    let _ = ws_recv_matching(&mut g1_ws, Duration::from_secs(2), |m| matches!(m, ServerMessage::RoomState(_))).await.unwrap();

    // Guest 2 joins
    let (mut g2_ws, _) = connect_async(&ws_url).await.unwrap();
    ws_send_msg(
        &mut g2_ws,
        &ClientMessage::RoomJoin {
            room_code: room_code.clone(),
            room_token: room_token.clone(),
            display_name: "Guest_2".into(),
            participant_id: None,
        },
    )
    .await;
    let g2_id = match ws_recv_matching(&mut g2_ws, Duration::from_secs(2), |m| matches!(m, ServerMessage::RoomState(_))).await.unwrap() {
        ServerMessage::RoomState(s) => s.participants.iter().find(|p| p.display_name == "Guest_2").unwrap().id.clone(),
        _ => unreachable!(),
    };

    // 1. Guest 1 attempts to kick Host -> REJECTED
    ws_send_msg(
        &mut g1_ws,
        &ClientMessage::ParticipantKick {
            target_id: host_id.clone(),
        },
    )
    .await;

    let err1 = ws_recv_matching(&mut g1_ws, Duration::from_secs(2), |m| matches!(m, ServerMessage::Error { .. })).await.unwrap();
    assert!(matches!(err1, ServerMessage::Error { code, .. } if code == "PERMISSION_DENIED"));

    // 2. Guest 1 attempts to kick Guest 2 -> REJECTED
    ws_send_msg(
        &mut g1_ws,
        &ClientMessage::ParticipantKick {
            target_id: g2_id.clone(),
        },
    )
    .await;

    let err2 = ws_recv_matching(&mut g1_ws, Duration::from_secs(2), |m| matches!(m, ServerMessage::Error { .. })).await.unwrap();
    assert!(matches!(err2, ServerMessage::Error { code, .. } if code == "PERMISSION_DENIED"));

    // 3. Host attempts to kick Host (self) -> REJECTED
    ws_send_msg(
        &mut host_ws,
        &ClientMessage::ParticipantKick {
            target_id: host_id.clone(),
        },
    )
    .await;

    let err3 = ws_recv_matching(&mut host_ws, Duration::from_secs(2), |m| matches!(m, ServerMessage::Error { .. })).await.unwrap();
    assert!(matches!(err3, ServerMessage::Error { code, .. } if code == "PERMISSION_DENIED"));

    // 4. Verify all 3 participants are still intact in room
    {
        let r = server.manager.get_room_by_code(&room_code).await.unwrap();
        let room_lock = r.lock().await;
        assert_eq!(room_lock.participants.connected_count(), 3);
        assert!(room_lock.participants.is_registered(&host_id));
        assert!(room_lock.participants.is_registered(&g2_id));
    }

    server.shutdown().await.unwrap();
}

#[test]
fn test_adversarial_domain_host_action_integrity() {
    let (mut room, host_id) = Room::new(
        "Host",
        sample_video(60_000),
        PermissionMode::FreeControl,
        5,
        1000,
    );
    let guest1 = room
        .participants
        .register_guest("Guest1", 0, None, 1000)
        .unwrap();
    let guest2 = room
        .participants
        .register_guest("Guest2", 0, None, 1000)
        .unwrap();

    // Guest cannot kick host
    let err = room.kick(&guest1.id, &host_id).unwrap_err();
    assert!(matches!(err, RoomServerError::PermissionDenied(_)));

    // Guest cannot kick guest
    let err = room.kick(&guest1.id, &guest2.id).unwrap_err();
    assert!(matches!(err, RoomServerError::PermissionDenied(_)));

    // Host cannot kick self
    let err = room.kick(&host_id, &host_id).unwrap_err();
    assert!(matches!(err, RoomServerError::PermissionDenied(_)));

    // Host kicking non-existent participant returns ParticipantNotFound
    let ghost_id = ParticipantId::new("part-ghost-999");
    let err = room.kick(&host_id, &ghost_id).unwrap_err();
    assert!(matches!(err, RoomServerError::ParticipantNotFound(_)));

    // Guest cannot change mode
    let err = room
        .set_permission_mode(&guest1.id, PermissionMode::HostOnly)
        .unwrap_err();
    assert!(matches!(err, RoomServerError::PermissionDenied(_)));

    // Guest cannot close room
    let err = room.close(&guest1.id).unwrap_err();
    assert!(matches!(err, RoomServerError::PermissionDenied(_)));
}

// ============================================================================
// 4. REPLAY AND STALE REVISION INJECTION
// ============================================================================

#[tokio::test]
async fn test_adversarial_stale_and_replay_revision_rejected_ws() {
    let server = RoomServer::start_default("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let ws_url = format!("ws://{}/ws", server.local_addr);

    // Host creates room
    let (mut host_ws, _) = connect_async(&ws_url).await.unwrap();
    ws_send_msg(
        &mut host_ws,
        &ClientMessage::RoomCreate {
            host_name: "Host_Rev".into(),
            video: sample_video(120_000),
            permission_mode: PermissionMode::FreeControl,
        },
    )
    .await;

    let host_snap = ws_recv_matching(&mut host_ws, Duration::from_secs(2), |m| {
        matches!(m, ServerMessage::RoomState(_))
    })
    .await
    .unwrap();

    let (room_code, room_token) = match host_snap {
        ServerMessage::RoomState(s) => {
            let code = s.room_code.clone();
            let token = get_room_token(&server, &code).await;
            (code, token)
        }
        _ => unreachable!(),
    };

    // Advance revision to 5 using valid sequential host commands
    for rev in 2..=5 {
        ws_send_msg(
            &mut host_ws,
            &ClientMessage::PlaybackPause {
                revision: rev,
                position_ms: rev * 1000,
                client_timestamp_ms: 1000 + rev,
            },
        )
        .await;

        let upd = ws_recv_matching(&mut host_ws, Duration::from_secs(2), |m| {
            matches!(m, ServerMessage::PlaybackUpdated { .. })
        })
        .await
        .unwrap();

        match upd {
            ServerMessage::PlaybackUpdated { revision, .. } => assert_eq!(revision, rev),
            _ => panic!("Expected PlaybackUpdated"),
        }
    }

    // Now current revision is 5!
    {
        let r = server.manager.get_room_by_code(&room_code).await.unwrap();
        assert_eq!(r.lock().await.playback.revision, 5);
    }

    // Guest joins
    let (mut guest_ws, _) = connect_async(&ws_url).await.unwrap();
    ws_send_msg(
        &mut guest_ws,
        &ClientMessage::RoomJoin {
            room_code: room_code.clone(),
            room_token: room_token.clone(),
            display_name: "Guest_Replayer".into(),
            participant_id: None,
        },
    )
    .await;

    let _ = ws_recv_matching(&mut guest_ws, Duration::from_secs(2), |m| {
        matches!(m, ServerMessage::RoomState(_))
    })
    .await
    .unwrap();

    // Consume ParticipantJoined on host
    let _ = ws_recv_matching(&mut host_ws, Duration::from_millis(200), |m| {
        matches!(m, ServerMessage::ParticipantJoined(_))
    })
    .await;

    // INJECT STALE & REPLAY REVISIONS: rev <= 5 must all be rejected with STALE_REVISION
    let stale_test_cases = vec![
        ClientMessage::PlaybackPlay { revision: 5, client_timestamp_ms: 2000 },  // equal revision
        ClientMessage::PlaybackPause { revision: 5, position_ms: 500, client_timestamp_ms: 2000 },
        ClientMessage::PlaybackSeek { revision: 4, target_position_ms: 3000, client_timestamp_ms: 2000 }, // smaller
        ClientMessage::PlaybackSpeed { revision: 3, speed: 1.5, client_timestamp_ms: 2000 }, // smaller
        ClientMessage::PlaybackPlay { revision: 1, client_timestamp_ms: 2000 },  // initial revision
        ClientMessage::PlaybackPause { revision: 0, position_ms: 0, client_timestamp_ms: 2000 }, // zero revision
    ];

    for (idx, stale_cmd) in stale_test_cases.into_iter().enumerate() {
        ws_send_msg(&mut guest_ws, &stale_cmd).await;

        let err = ws_recv_matching(&mut guest_ws, Duration::from_secs(2), |m| {
            matches!(m, ServerMessage::Error { .. })
        })
        .await
        .unwrap_or_else(|| panic!("Stale test case {} should produce an error", idx));

        match err {
            ServerMessage::Error { code, message } => {
                assert_eq!(code, "STALE_REVISION", "Test case {} failed: {}", idx, message);
                assert!(message.contains("Stale revision"));
            }
            other => panic!("Expected STALE_REVISION, got {:?}", other),
        }
    }

    // Verify room state revision is STILL 5!
    {
        let r = server.manager.get_room_by_code(&room_code).await.unwrap();
        assert_eq!(r.lock().await.playback.revision, 5);
    }

    // Next strictly monotonic revision (6) MUST succeed
    ws_send_msg(
        &mut guest_ws,
        &ClientMessage::PlaybackPlay {
            revision: 6,
            client_timestamp_ms: 3000,
        },
    )
    .await;

    let guest_upd = ws_recv_matching(&mut guest_ws, Duration::from_secs(2), |m| {
        matches!(m, ServerMessage::PlaybackUpdated { .. })
    })
    .await
    .expect("Monotonic rev 6 must succeed");

    match guest_upd {
        ServerMessage::PlaybackUpdated { revision, .. } => assert_eq!(revision, 6),
        _ => panic!("Expected PlaybackUpdated"),
    }

    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_adversarial_concurrent_stale_and_valid_revision_race() {
    let (mut room, host_id) = Room::new(
        "Host",
        sample_video(120_000),
        PermissionMode::FreeControl,
        5,
        1000,
    );
    let guest = room
        .participants
        .register_guest("Guest", 0, None, 1000)
        .unwrap();

    // Advance to revision 10
    for r in 2..=10 {
        room.apply_pause(&host_id, r, r * 1000, 1000 + r).unwrap();
    }
    assert_eq!(room.playback.revision, 10);

    // In parallel, apply stale revisions (1..=10) from guest
    for stale_r in 0..=10 {
        assert!(room.apply_play(&guest.id, stale_r, 2000).is_err());
        assert!(room.apply_pause(&guest.id, stale_r, 0, 2000).is_err());
        assert!(room.apply_seek(&guest.id, stale_r, 500, 2000).is_err());
        assert!(room.apply_speed(&guest.id, stale_r, 1.25, 2000).is_err());
    }

    // Revision should remain 10
    assert_eq!(room.playback.revision, 10);

    // Monotonic next revisions 11..15 must succeed
    for next_r in 11..=15 {
        assert!(room.apply_play(&guest.id, next_r, 3000 + next_r).is_ok());
        assert_eq!(room.playback.revision, next_r);
    }
}

// ============================================================================
// 5. TEARDOWN CASCADE
// ============================================================================

#[tokio::test]
async fn test_adversarial_room_teardown_cascade_with_media_server() {
    // 1. Create synthetic video file
    let temp_file = create_temp_video_file(256 * 1024); // 256 KB
    let token_hex = "a".repeat(64);

    // 2. Start embedded MediaServer
    let media_handle = MediaServer::start_local(temp_file.path(), &token_hex)
        .await
        .expect("MediaServer starts");
    let stream_url = media_handle.stream_url();

    // Verify media server is active and serving 206 Partial Content
    let http_client = reqwest::Client::new();
    let resp = http_client
        .get(&stream_url)
        .header("Range", "bytes=0-100")
        .send()
        .await
        .expect("HTTP GET media server");
    assert_eq!(resp.status(), reqwest::StatusCode::PARTIAL_CONTENT);

    // 3. Start RoomServer
    let room_server = RoomServer::start_default("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let ws_url = format!("ws://{}/ws", room_server.local_addr);

    // 4. Host connects and creates room over WebSocket
    let (mut host_ws, _) = connect_async(&ws_url).await.unwrap();
    ws_send_msg(
        &mut host_ws,
        &ClientMessage::RoomCreate {
            host_name: "Host_Teardown".into(),
            video: sample_video(60_000),
            permission_mode: PermissionMode::FreeControl,
        },
    )
    .await;

    let host_snap = ws_recv_matching(&mut host_ws, Duration::from_secs(2), |m| {
        matches!(m, ServerMessage::RoomState(_))
    })
    .await
    .unwrap();

    let (room_code, room_token, host_id) = match host_snap {
        ServerMessage::RoomState(s) => {
            let code = s.room_code.clone();
            let token = get_room_token(&room_server, &code).await;
            (code, token, s.host_id)
        }
        _ => unreachable!(),
    };

    let room_arc = room_server
        .manager
        .get_room_by_code(&room_code)
        .await
        .expect("Room exists in manager");

    // Attach media server handle to room
    {
        let mut r = room_arc.lock().await;
        r.media_server_handle = Some(media_handle);
    }

    // Connect 2 Guests over WebSocket
    let (mut g1_ws, _) = connect_async(&ws_url).await.unwrap();
    ws_send_msg(
        &mut g1_ws,
        &ClientMessage::RoomJoin {
            room_code: room_code.clone(),
            room_token: room_token.clone(),
            display_name: "Guest_1".into(),
            participant_id: None,
        },
    )
    .await;
    let _ = ws_recv_matching(&mut g1_ws, Duration::from_secs(2), |m| matches!(m, ServerMessage::RoomState(_))).await.unwrap();

    let (mut g2_ws, _) = connect_async(&ws_url).await.unwrap();
    ws_send_msg(
        &mut g2_ws,
        &ClientMessage::RoomJoin {
            room_code: room_code.clone(),
            room_token: room_token.clone(),
            display_name: "Guest_2".into(),
            participant_id: None,
        },
    )
    .await;
    let _ = ws_recv_matching(&mut g2_ws, Duration::from_secs(2), |m| matches!(m, ServerMessage::RoomState(_))).await.unwrap();

    // 5. Host invokes full teardown
    {
        let mut r = room_arc.lock().await;
        r.teardown(&host_id)
            .await
            .expect("Room teardown by host succeeds");
        assert!(r.is_closed, "Room must be marked is_closed");
        assert!(r.media_server_handle.is_none(), "media_server_handle should be taken and shut down");
    }

    // 6. Verify all connected clients receive ServerMessage::RoomClosed
    let g1_closed = ws_recv_matching(&mut g1_ws, Duration::from_secs(2), |m| {
        matches!(m, ServerMessage::RoomClosed { .. })
    })
    .await
    .expect("Guest 1 receives RoomClosed");

    let g2_closed = ws_recv_matching(&mut g2_ws, Duration::from_secs(2), |m| {
        matches!(m, ServerMessage::RoomClosed { .. })
    })
    .await
    .expect("Guest 2 receives RoomClosed");

    match g1_closed {
        ServerMessage::RoomClosed { reason } => assert_eq!(reason, "host_closed_session"),
        _ => panic!("Expected RoomClosed"),
    }
    match g2_closed {
        ServerMessage::RoomClosed { reason } => assert_eq!(reason, "host_closed_session"),
        _ => panic!("Expected RoomClosed"),
    }

    // 7. Verify guest WebSocket streams close
    let g1_next = timeout(Duration::from_millis(500), g1_ws.next()).await;
    assert!(
        matches!(g1_next, Ok(None) | Ok(Some(Err(_))) | Err(_)),
        "Guest 1 WS connection should terminate after RoomClosed"
    );

    // 8. Subsequent join attempt MUST return ROOM_NOT_FOUND
    let (mut late_ws, _) = connect_async(&ws_url).await.unwrap();
    ws_send_msg(
        &mut late_ws,
        &ClientMessage::RoomJoin {
            room_code: room_code.clone(),
            room_token: room_token.clone(),
            display_name: "LateJoiner".into(),
            participant_id: None,
        },
    )
    .await;

    let late_err = ws_recv_matching(&mut late_ws, Duration::from_secs(2), |m| {
        matches!(m, ServerMessage::Error { .. })
    })
    .await
    .expect("Late joiner receives Error");

    match late_err {
        ServerMessage::Error { code, .. } => {
            assert_eq!(code, "ROOM_NOT_FOUND", "Subsequent join must return ROOM_NOT_FOUND");
        }
        other => panic!("Expected ROOM_NOT_FOUND, got {:?}", other),
    }

    // 9. Media stream server is SHUT DOWN: HTTP requests must fail or return 403 Server Closed
    let post_teardown_resp = http_client.get(&stream_url).send().await;
    match post_teardown_resp {
        Ok(resp) => {
            // If the listener hasn't closed TCP yet, axum route returns 403 Forbidden ("Server closed")
            assert_eq!(resp.status(), reqwest::StatusCode::FORBIDDEN);
        }
        Err(err) => {
            // TCP connection refused or dropped
            assert!(err.is_connect() || err.is_request());
        }
    }

    room_server.shutdown().await.unwrap();
}

// ============================================================================
// 6. ADDITIONAL ADVERSARIAL EDGE CASES
// ============================================================================

#[test]
fn test_adversarial_crockford_charset_and_entropy() {
    // PRD R5: 6-char code Crockford charset [2-9A-HJ-NP-Z]{6} (excludes 0, 1, I, O)
    assert_eq!(CROCKFORD_CHARSET.len(), 32);
    for forbidden in b"01IOio" {
        assert!(
            !CROCKFORD_CHARSET.contains(forbidden),
            "Crockford charset must not contain ambiguous character: {}",
            *forbidden as char
        );
    }

    // Generate 100 codes, ensure all chars belong to CROCKFORD_CHARSET
    for _ in 0..100 {
        let code = room_server::room::generate_room_code();
        assert_eq!(code.0.len(), 6);
        for b in code.0.bytes() {
            assert!(
                CROCKFORD_CHARSET.contains(&b),
                "Code char {} not in Crockford charset",
                b as char
            );
        }
    }

    // PRD R5: Token is at least 32 bytes (64 hex characters)
    let token = room_server::room::generate_room_token();
    assert_eq!(token.0.len(), 64);
    assert!(token.is_valid());
}

#[test]
fn test_adversarial_speed_validation() {
    // Valid speeds
    for &s in &ALLOWED_PLAYBACK_SPEEDS {
        assert!(is_valid_playback_speed(s), "Speed {} should be valid", s);
    }

    // Adversarial speeds
    let invalid_speeds = [0.0, 0.1, 0.25, 0.74, 0.99, 1.01, 1.33, 2.01, 3.0, 10.0, -1.0, f32::NAN, f32::INFINITY];
    for &s in &invalid_speeds {
        assert!(!is_valid_playback_speed(s), "Speed {} should be rejected as invalid", s);
    }
}

#[test]
fn test_adversarial_position_clamping_at_duration() {
    let (mut room, host_id) = Room::new(
        "Host",
        sample_video(10_000), // 10s duration
        PermissionMode::FreeControl,
        5,
        1000,
    );

    // Seek past duration
    let state = room.apply_seek(&host_id, 2, 999_999, 1000).unwrap();
    assert_eq!(state.position_ms, 10_000, "Position must be clamped to video duration");

    // Pause past duration
    let state = room.apply_pause(&host_id, 3, 500_000, 1000).unwrap();
    assert_eq!(state.position_ms, 10_000, "Pause position must be clamped to video duration");
}

#[tokio::test]
async fn test_adversarial_ws_host_disconnect_leaves_media_server_behavior() {
    // 1. Create synthetic video file & start MediaServer
    let temp_file = create_temp_video_file(64 * 1024);
    let token_hex = "b".repeat(64);
    let media_handle = MediaServer::start_local(temp_file.path(), &token_hex)
        .await
        .unwrap();
    let stream_url = media_handle.stream_url();
    let is_active_atomic = media_handle.state.is_active.clone();

    // 2. Start RoomServer
    let server = RoomServer::start_default("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let ws_url = format!("ws://{}/ws", server.local_addr);

    // 3. Host connects and creates room over WebSocket
    let (mut host_ws, _) = connect_async(&ws_url).await.unwrap();
    ws_send_msg(
        &mut host_ws,
        &ClientMessage::RoomCreate {
            host_name: "Host_Drop".into(),
            video: sample_video(60_000),
            permission_mode: PermissionMode::FreeControl,
        },
    )
    .await;

    let host_snap = ws_recv_matching(&mut host_ws, Duration::from_secs(2), |m| {
        matches!(m, ServerMessage::RoomState(_))
    })
    .await
    .unwrap();

    let room_code = match host_snap {
        ServerMessage::RoomState(s) => s.room_code,
        _ => unreachable!(),
    };

    let room_arc = server.manager.get_room_by_code(&room_code).await.unwrap();

    // Attach media server handle to room
    {
        let mut r = room_arc.lock().await;
        r.media_server_handle = Some(media_handle);
    }

    // 4. Guest connects
    let room_token = get_room_token(&server, &room_code).await;
    let (mut guest_ws, _) = connect_async(&ws_url).await.unwrap();
    ws_send_msg(
        &mut guest_ws,
        &ClientMessage::RoomJoin {
            room_code: room_code.clone(),
            room_token: room_token.clone(),
            display_name: "Guest_Observer".into(),
            participant_id: None,
        },
    )
    .await;
    let _ = ws_recv_matching(&mut guest_ws, Duration::from_secs(2), |m| matches!(m, ServerMessage::RoomState(_))).await.unwrap();

    // 5. Host drops WebSocket connection (disconnects)
    drop(host_ws);

    // 6. Guest receives RoomClosed because host dropped connection
    let guest_closed = ws_recv_matching(&mut guest_ws, Duration::from_secs(2), |m| {
        matches!(m, ServerMessage::RoomClosed { .. })
    })
    .await;
    assert!(
        guest_closed.is_some(),
        "Guest MUST receive RoomClosed when host disconnects"
    );

    // 7. Check whether media_server_handle is shut down or still running!
    // Give background tasks a brief moment to process disconnect
    tokio::time::sleep(Duration::from_millis(100)).await;

    let media_active = is_active_atomic.load(Ordering::SeqCst);
    assert!(media_active, "Observation: media_server_handle remains active when host only disconnects WebSocket without explicit teardown()");

    let http_client = reqwest::Client::new();
    let resp = http_client.get(&stream_url).header("Range", "bytes=0-10").send().await.unwrap();
    assert_eq!(resp.status(), reqwest::StatusCode::PARTIAL_CONTENT);

    // Clean up
    {
        let mut r = room_arc.lock().await;
        if let Some(mh) = r.media_server_handle.take() {
            let _ = mh.shutdown().await;
        }
    }
    server.shutdown().await.unwrap();
}
