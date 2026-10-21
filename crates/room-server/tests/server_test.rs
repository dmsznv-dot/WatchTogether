use futures_util::{SinkExt, StreamExt};
use room_server::RoomServer;
use shared_protocol::messages::{ClientMessage, ServerMessage};
use shared_protocol::models::{PermissionMode, PlaybackStatus, VideoMetadata};
use std::time::Duration;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

fn sample_video() -> VideoMetadata {
    VideoMetadata {
        filename: "test_movie.mp4".into(),
        duration_ms: 60_000,
        file_size_bytes: 10_000_000,
        mime_type: "video/mp4".into(),
        width: Some(1920),
        height: Some(1080),
    }
}

#[tokio::test]
async fn test_room_server_ws_lifecycle_and_events() {
    let handle = RoomServer::start_default("127.0.0.1:0".parse().unwrap())
        .await
        .expect("Server starts");
    let ws_url = format!("ws://{}/ws", handle.local_addr);

    // 1. Connect Host WebSocket
    let (mut host_ws, _) = connect_async(&ws_url).await.expect("Host connects");

    // Host sends RoomCreate
    let create_msg = ClientMessage::RoomCreate {
        host_name: "HostUser".into(),
        video: sample_video(),
        permission_mode: PermissionMode::FreeControl,
    };
    host_ws
        .send(Message::Text(serde_json::to_string(&create_msg).unwrap()))
        .await
        .unwrap();

    // Host receives RoomState snapshot
    let host_resp = host_ws.next().await.unwrap().unwrap();
    let host_state: ServerMessage = serde_json::from_str(host_resp.to_text().unwrap()).unwrap();
    let (room_code, _filename) = match host_state {
        ServerMessage::RoomState(snap) => {
            assert_eq!(snap.participants.len(), 1);
            assert_eq!(snap.participants[0].display_name, "HostUser");
            (snap.room_code, snap.video.filename)
        }
        other => panic!("Expected RoomState, got {:?}", other),
    };

    // Grab actual token from manager
    let room_arc = handle
        .manager
        .get_room_by_code(&room_code)
        .await
        .expect("Room exists in manager");
    let real_token = {
        let room = room_arc.lock().await;
        room.room_token.clone()
    };

    // 2. Connect Guest WebSocket
    let (mut guest_ws, _) = connect_async(&ws_url).await.expect("Guest connects");

    // Guest sends RoomJoin
    let join_msg = ClientMessage::RoomJoin {
        room_code: room_code.clone(),
        room_token: real_token.clone(),
        display_name: "GuestUser".into(),
        participant_id: None,
    };
    guest_ws
        .send(Message::Text(serde_json::to_string(&join_msg).unwrap()))
        .await
        .unwrap();

    // Guest receives RoomState
    let guest_resp = guest_ws.next().await.unwrap().unwrap();
    let guest_state: ServerMessage = serde_json::from_str(guest_resp.to_text().unwrap()).unwrap();
    let guest_id = match guest_state {
        ServerMessage::RoomState(snap) => {
            assert_eq!(snap.participants.len(), 2);
            let g = snap
                .participants
                .iter()
                .find(|p| p.display_name == "GuestUser")
                .expect("Guest present");
            g.id.clone()
        }
        other => panic!("Expected RoomState for guest, got {:?}", other),
    };

    // Host receives ParticipantJoined
    let host_joined_msg = host_ws.next().await.unwrap().unwrap();
    let parsed_joined: ServerMessage =
        serde_json::from_str(host_joined_msg.to_text().unwrap()).unwrap();
    match parsed_joined {
        ServerMessage::ParticipantJoined(p) => {
            assert_eq!(p.display_name, "GuestUser");
            assert_eq!(p.id, guest_id);
        }
        other => panic!("Expected ParticipantJoined, got {:?}", other),
    }

    // 3. Playback play from host
    let play_msg = ClientMessage::PlaybackPlay {
        revision: 2,
        client_timestamp_ms: 1000,
    };
    host_ws
        .send(Message::Text(serde_json::to_string(&play_msg).unwrap()))
        .await
        .unwrap();

    // Both host and guest receive PlaybackUpdated
    let host_play_update = host_ws.next().await.unwrap().unwrap();
    let guest_play_update = guest_ws.next().await.unwrap().unwrap();

    let hp: ServerMessage = serde_json::from_str(host_play_update.to_text().unwrap()).unwrap();
    let gp: ServerMessage = serde_json::from_str(guest_play_update.to_text().unwrap()).unwrap();

    match hp {
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
        other => panic!("Expected PlaybackUpdated, got {:?}", other),
    }
    match gp {
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
        other => panic!("Expected PlaybackUpdated, got {:?}", other),
    }

    // 4. Guest sends SyncPing, receives SyncPong
    let ping_msg = ClientMessage::SyncPing {
        ping_id: 42,
        client_send_time_ms: 1234,
    };
    guest_ws
        .send(Message::Text(serde_json::to_string(&ping_msg).unwrap()))
        .await
        .unwrap();

    let guest_pong = guest_ws.next().await.unwrap().unwrap();
    let pong: ServerMessage = serde_json::from_str(guest_pong.to_text().unwrap()).unwrap();
    match pong {
        ServerMessage::SyncPong {
            ping_id,
            client_send_time_ms,
            ..
        } => {
            assert_eq!(ping_id, 42);
            assert_eq!(client_send_time_ms, 1234);
        }
        other => panic!("Expected SyncPong, got {:?}", other),
    }

    // 5. Host changes permission to HostOnly
    let perm_msg = ClientMessage::RoomSetPermissions {
        permission_mode: PermissionMode::HostOnly,
    };
    host_ws
        .send(Message::Text(serde_json::to_string(&perm_msg).unwrap()))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Guest attempts to pause in HostOnly -> receives Error
    let guest_pause = ClientMessage::PlaybackPause {
        revision: 3,
        position_ms: 5000,
        client_timestamp_ms: 2000,
    };
    guest_ws
        .send(Message::Text(serde_json::to_string(&guest_pause).unwrap()))
        .await
        .unwrap();

    let guest_err = guest_ws.next().await.unwrap().unwrap();
    let err_msg: ServerMessage = serde_json::from_str(guest_err.to_text().unwrap()).unwrap();
    match err_msg {
        ServerMessage::Error { code, .. } => {
            assert_eq!(code, "PERMISSION_DENIED");
        }
        other => panic!("Expected Error PERMISSION_DENIED, got {:?}", other),
    }

    // 6. Host kicks guest
    let kick_msg = ClientMessage::ParticipantKick {
        target_id: guest_id.clone(),
    };
    host_ws
        .send(Message::Text(serde_json::to_string(&kick_msg).unwrap()))
        .await
        .unwrap();

    // Guest receives ParticipantKicked and disconnects
    let guest_kicked = guest_ws.next().await.unwrap().unwrap();
    let kicked_msg: ServerMessage = serde_json::from_str(guest_kicked.to_text().unwrap()).unwrap();
    match kicked_msg {
        ServerMessage::ParticipantKicked {
            participant_id,
            reason,
        } => {
            assert_eq!(participant_id, guest_id);
            assert_eq!(reason, "kicked_by_host");
        }
        other => panic!("Expected ParticipantKicked, got {:?}", other),
    }

    // Shutdown server handle
    handle.shutdown().await.expect("Server shuts down cleanly");
}
