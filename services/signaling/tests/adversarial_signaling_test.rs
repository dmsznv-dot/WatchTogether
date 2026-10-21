use futures_util::{SinkExt, StreamExt};
use signaling::{
    ClientSignal, IceCandidate, IceCandidateType, PeerRole, ServerSignal, SessionDescription,
    SignalingServer,
};
use std::time::Duration;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

const VALID_CODE: &str = "XYZ789";
const VALID_TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[tokio::test]
async fn test_adversarial_malformed_json_flooding() {
    let handle = SignalingServer::start_default("127.0.0.1:0".parse().unwrap())
        .await
        .expect("Server starts");
    let ws_url = format!("ws://{}/ws", handle.local_addr);

    let (mut ws, _) = connect_async(&ws_url).await.expect("Connect");

    let malformed_payloads = vec![
        "{{not_json",
        "{",
        "}",
        "12345",
        "\"raw string\"",
        "true",
        "null",
        "[]",
        "[1, 2, 3]",
        r#"{"type": "unknown.signal", "payload": {}}"#,
        r#"{"type": "signal.join"}"#, // missing payload
        r#"{"type": "signal.offer", "payload": null}"#,
        r#"{"type": "signal.join", "payload": {"room_code": 123}}"#, // wrong type
        r#"{"type": "signal.ice_candidate", "payload": {}}"#,
    ];

    for payload in malformed_payloads {
        ws.send(Message::Text(payload.into())).await.unwrap();

        let resp = ws.next().await.unwrap().unwrap();
        let sig: ServerSignal = serde_json::from_str(resp.to_text().unwrap()).unwrap();
        match sig {
            ServerSignal::Error { code, .. } => {
                assert_eq!(code, "INVALID_MESSAGE");
            }
            other => panic!("Expected Error INVALID_MESSAGE, got {:?}", other),
        }
    }

    // Now send 30 rapid malformed messages in a loop
    for i in 0..30 {
        ws.send(Message::Text(format!("garbage payload number {}", i)))
            .await
            .unwrap();
        let resp = ws.next().await.unwrap().unwrap();
        let sig: ServerSignal = serde_json::from_str(resp.to_text().unwrap()).unwrap();
        match sig {
            ServerSignal::Error { code, .. } => {
                assert_eq!(code, "INVALID_MESSAGE");
            }
            other => panic!("Expected Error INVALID_MESSAGE, got {:?}", other),
        }
    }

    // Verify socket is STILL ALIVE and can perform a valid Join
    let valid_join = ClientSignal::Join {
        room_code: VALID_CODE.into(),
        peer_id: "host-1".into(),
        room_token: VALID_TOKEN.into(),
        role: Some(PeerRole::Host),
    };
    ws.send(Message::Text(serde_json::to_string(&valid_join).unwrap()))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(handle.registry.active_rooms_count().await, 1);

    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_adversarial_invalid_room_tokens() {
    let handle = SignalingServer::start_default("127.0.0.1:0".parse().unwrap())
        .await
        .expect("Server starts");
    let ws_url = format!("ws://{}/ws", handle.local_addr);

    // 1. Host tries with token < 32 chars
    let (mut host_ws, _) = connect_async(&ws_url).await.unwrap();
    let short_token_join = ClientSignal::Join {
        room_code: VALID_CODE.into(),
        peer_id: "host-short".into(),
        room_token: "short_token_only_24_chars".into(),
        role: Some(PeerRole::Host),
    };
    host_ws
        .send(Message::Text(
            serde_json::to_string(&short_token_join).unwrap(),
        ))
        .await
        .unwrap();

    let resp = host_ws.next().await.unwrap().unwrap();
    let sig: ServerSignal = serde_json::from_str(resp.to_text().unwrap()).unwrap();
    match sig {
        ServerSignal::Error { code, .. } => assert_eq!(code, "INVALID_TOKEN"),
        other => panic!("Expected INVALID_TOKEN, got {:?}", other),
    }

    // Host tries with empty token
    let empty_token_join = ClientSignal::Join {
        room_code: VALID_CODE.into(),
        peer_id: "host-empty".into(),
        room_token: "".into(),
        role: Some(PeerRole::Host),
    };
    host_ws
        .send(Message::Text(
            serde_json::to_string(&empty_token_join).unwrap(),
        ))
        .await
        .unwrap();

    let resp = host_ws.next().await.unwrap().unwrap();
    let sig: ServerSignal = serde_json::from_str(resp.to_text().unwrap()).unwrap();
    match sig {
        ServerSignal::Error { code, .. } => assert_eq!(code, "INVALID_TOKEN"),
        other => panic!("Expected INVALID_TOKEN, got {:?}", other),
    }

    // Valid host registers room
    let valid_host_join = ClientSignal::Join {
        room_code: VALID_CODE.into(),
        peer_id: "legit-host".into(),
        room_token: VALID_TOKEN.into(),
        role: Some(PeerRole::Host),
    };
    host_ws
        .send(Message::Text(
            serde_json::to_string(&valid_host_join).unwrap(),
        ))
        .await
        .unwrap();

    // 2. Guest tries with token < 32 chars
    let (mut guest_ws, _) = connect_async(&ws_url).await.unwrap();
    let guest_short = ClientSignal::Join {
        room_code: VALID_CODE.into(),
        peer_id: "guest-short".into(),
        room_token: "too_short".into(),
        role: Some(PeerRole::Guest),
    };
    guest_ws
        .send(Message::Text(serde_json::to_string(&guest_short).unwrap()))
        .await
        .unwrap();

    let resp = guest_ws.next().await.unwrap().unwrap();
    let sig: ServerSignal = serde_json::from_str(resp.to_text().unwrap()).unwrap();
    match sig {
        ServerSignal::Error { code, .. } => assert_eq!(code, "INVALID_TOKEN"),
        other => panic!("Expected INVALID_TOKEN, got {:?}", other),
    }

    // 3. Guest tries with 64-char token but 1 character different
    let mut bad_token = VALID_TOKEN.to_string();
    bad_token.replace_range(0..1, "f"); // modify first byte
    let guest_mismatch = ClientSignal::Join {
        room_code: VALID_CODE.into(),
        peer_id: "guest-mismatch".into(),
        room_token: bad_token,
        role: Some(PeerRole::Guest),
    };
    guest_ws
        .send(Message::Text(
            serde_json::to_string(&guest_mismatch).unwrap(),
        ))
        .await
        .unwrap();

    let resp = guest_ws.next().await.unwrap().unwrap();
    let sig: ServerSignal = serde_json::from_str(resp.to_text().unwrap()).unwrap();
    match sig {
        ServerSignal::Error { code, .. } => assert_eq!(code, "INVALID_TOKEN"),
        other => panic!("Expected INVALID_TOKEN, got {:?}", other),
    }

    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_adversarial_invalid_room_codes() {
    let handle = SignalingServer::start_default("127.0.0.1:0".parse().unwrap())
        .await
        .expect("Server starts");
    let ws_url = format!("ws://{}/ws", handle.local_addr);

    let (mut ws, _) = connect_async(&ws_url).await.unwrap();

    // Invalid characters: 0, 1, I, O (from 32-char [2-9A-HJ-NP-Z]), lowercase, length != 6
    let invalid_codes = vec![
        "ABC",      // too short
        "ABCDEFG",  // too long
        "ABC0EF",   // contains 0
        "ABC1EF",   // contains 1
        "ABCIEF",   // contains I
        "ABCOEF",   // contains O
        "abc234",   // lowercase
        "ABC-23",   // punctuation
        "ABC 23",   // space
    ];

    for code in invalid_codes {
        let join = ClientSignal::Join {
            room_code: code.into(),
            peer_id: "peer-1".into(),
            room_token: VALID_TOKEN.into(),
            role: Some(PeerRole::Host),
        };
        ws.send(Message::Text(serde_json::to_string(&join).unwrap()))
            .await
            .unwrap();

        let resp = ws.next().await.unwrap().unwrap();
        let sig: ServerSignal = serde_json::from_str(resp.to_text().unwrap()).unwrap();
        match sig {
            ServerSignal::Error { code, .. } => assert_eq!(code, "INVALID_ROOM_CODE"),
            other => panic!("Expected INVALID_ROOM_CODE for {}, got {:?}", code, other),
        }
    }

    // Host registers valid code
    let valid_join = ClientSignal::Join {
        room_code: VALID_CODE.into(),
        peer_id: "host-orig".into(),
        room_token: VALID_TOKEN.into(),
        role: Some(PeerRole::Host),
    };
    ws.send(Message::Text(serde_json::to_string(&valid_join).unwrap()))
        .await
        .unwrap();

    // Ensure host is registered in registry before attempting duplicate
    while handle.registry.active_rooms_count().await == 0 {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // Second host tries to register the SAME room code while active
    let (mut ws2, _) = connect_async(&ws_url).await.unwrap();
    let dup_join = ClientSignal::Join {
        room_code: VALID_CODE.into(),
        peer_id: "host-imposter".into(),
        room_token: VALID_TOKEN.into(),
        role: Some(PeerRole::Host),
    };
    ws2.send(Message::Text(serde_json::to_string(&dup_join).unwrap()))
        .await
        .unwrap();

    let resp = ws2.next().await.unwrap().unwrap();
    let sig: ServerSignal = serde_json::from_str(resp.to_text().unwrap()).unwrap();
    match sig {
        ServerSignal::Error { code, .. } => assert_eq!(code, "INVALID_ROOM_CODE"),
        other => panic!("Expected INVALID_ROOM_CODE for duplicate host, got {:?}", other),
    }

    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_adversarial_unknown_target_and_sender_peers() {
    let handle = SignalingServer::start_default("127.0.0.1:0".parse().unwrap())
        .await
        .expect("Server starts");
    let ws_url = format!("ws://{}/ws", handle.local_addr);

    // 1. Unjoined client tries to send Offer
    let (mut unjoined_ws, _) = connect_async(&ws_url).await.unwrap();
    let premature_offer = ClientSignal::Offer {
        sender_id: "ghost".into(),
        target_id: "someone".into(),
        desc: SessionDescription {
            sdp_type: "offer".into(),
            sdp: "v=0...".into(),
        },
    };
    unjoined_ws
        .send(Message::Text(
            serde_json::to_string(&premature_offer).unwrap(),
        ))
        .await
        .unwrap();

    let resp = unjoined_ws.next().await.unwrap().unwrap();
    let sig: ServerSignal = serde_json::from_str(resp.to_text().unwrap()).unwrap();
    match sig {
        ServerSignal::Error { code, .. } => assert_eq!(code, "PARTICIPANT_NOT_FOUND"),
        other => panic!("Expected PARTICIPANT_NOT_FOUND, got {:?}", other),
    }

    // 2. Host joins room
    let (mut host_ws, _) = connect_async(&ws_url).await.unwrap();
    host_ws
        .send(Message::Text(
            serde_json::to_string(&ClientSignal::Join {
                room_code: VALID_CODE.into(),
                peer_id: "host-real".into(),
                room_token: VALID_TOKEN.into(),
                role: Some(PeerRole::Host),
            })
            .unwrap(),
        ))
        .await
        .unwrap();

    // Host sends Offer to unknown target peer
    let ghost_target_offer = ClientSignal::Offer {
        sender_id: "host-real".into(),
        target_id: "non-existent-target".into(),
        desc: SessionDescription {
            sdp_type: "offer".into(),
            sdp: "v=0...".into(),
        },
    };
    host_ws
        .send(Message::Text(
            serde_json::to_string(&ghost_target_offer).unwrap(),
        ))
        .await
        .unwrap();

    let resp = host_ws.next().await.unwrap().unwrap();
    let sig: ServerSignal = serde_json::from_str(resp.to_text().unwrap()).unwrap();
    match sig {
        ServerSignal::Error { code, .. } => assert_eq!(code, "PARTICIPANT_NOT_FOUND"),
        other => panic!("Expected PARTICIPANT_NOT_FOUND, got {:?}", other),
    }

    // Host sends Answer to unknown target peer
    let ghost_target_answer = ClientSignal::Answer {
        sender_id: "host-real".into(),
        target_id: "non-existent-target".into(),
        desc: SessionDescription {
            sdp_type: "answer".into(),
            sdp: "v=0...".into(),
        },
    };
    host_ws
        .send(Message::Text(
            serde_json::to_string(&ghost_target_answer).unwrap(),
        ))
        .await
        .unwrap();

    let resp = host_ws.next().await.unwrap().unwrap();
    let sig: ServerSignal = serde_json::from_str(resp.to_text().unwrap()).unwrap();
    match sig {
        ServerSignal::Error { code, .. } => assert_eq!(code, "PARTICIPANT_NOT_FOUND"),
        other => panic!("Expected PARTICIPANT_NOT_FOUND, got {:?}", other),
    }

    // Host sends ICE candidate with imposter sender_id
    let imposter_ice = ClientSignal::IceCandidate {
        sender_id: "imposter-peer".into(),
        target_id: "host-real".into(),
        candidate: IceCandidate {
            candidate_type: IceCandidateType::Host,
            ip: "127.0.0.1".into(),
            port: 9000,
            protocol: "udp".into(),
            priority: 100,
        },
    };
    host_ws
        .send(Message::Text(
            serde_json::to_string(&imposter_ice).unwrap(),
        ))
        .await
        .unwrap();

    let resp = host_ws.next().await.unwrap().unwrap();
    let sig: ServerSignal = serde_json::from_str(resp.to_text().unwrap()).unwrap();
    match sig {
        ServerSignal::Error { code, .. } => assert_eq!(code, "PARTICIPANT_NOT_FOUND"),
        other => panic!("Expected PARTICIPANT_NOT_FOUND, got {:?}", other),
    }

    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_adversarial_host_disconnect_cascade() {
    let handle = SignalingServer::start_default("127.0.0.1:0".parse().unwrap())
        .await
        .expect("Server starts");
    let ws_url = format!("ws://{}/ws", handle.local_addr);

    // Host registers room
    let (mut host_ws, _) = connect_async(&ws_url).await.unwrap();
    host_ws
        .send(Message::Text(
            serde_json::to_string(&ClientSignal::Join {
                room_code: VALID_CODE.into(),
                peer_id: "host-cascade".into(),
                room_token: VALID_TOKEN.into(),
                role: Some(PeerRole::Host),
            })
            .unwrap(),
        ))
        .await
        .unwrap();

    // 4 guests join (room reaches capacity 5)
    let mut guest_sockets = Vec::new();
    for i in 1..=4 {
        let (mut g_ws, _) = connect_async(&ws_url).await.unwrap();
        g_ws.send(Message::Text(
            serde_json::to_string(&ClientSignal::Join {
                room_code: VALID_CODE.into(),
                peer_id: format!("guest-{}", i),
                room_token: VALID_TOKEN.into(),
                role: Some(PeerRole::Guest),
            })
            .unwrap(),
        ))
        .await
        .unwrap();
        guest_sockets.push(g_ws);
    }

    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(handle.registry.active_rooms_count().await, 1);
    assert_eq!(handle.registry.active_peers_count().await, 5);

    // Drop Host socket abruptly
    drop(host_ws);

    // Every connected guest must receive signal.room_closed with reason "host_disconnected"
    for mut g_ws in guest_sockets {
        let mut got_room_closed = false;
        while let Some(Ok(msg)) = g_ws.next().await {
            if let Ok(text) = msg.to_text() {
                if let Ok(ServerSignal::RoomClosed { reason }) = serde_json::from_str::<ServerSignal>(text) {
                    assert_eq!(reason, "host_disconnected");
                    got_room_closed = true;
                    break;
                }
            }
        }
        assert!(
            got_room_closed,
            "Guest must receive RoomClosed upon host disconnect"
        );
    }

    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(handle.registry.active_rooms_count().await, 0);
    assert_eq!(handle.registry.active_peers_count().await, 0);

    // Verify room code can now be re-registered by a new host
    let (mut new_host_ws, _) = connect_async(&ws_url).await.unwrap();
    new_host_ws
        .send(Message::Text(
            serde_json::to_string(&ClientSignal::Join {
                room_code: VALID_CODE.into(),
                peer_id: "new-host".into(),
                room_token: VALID_TOKEN.into(),
                role: Some(PeerRole::Host),
            })
            .unwrap(),
        ))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(handle.registry.active_rooms_count().await, 1);

    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_adversarial_guest_disconnect_and_slot_recycling() {
    let handle = SignalingServer::start_default("127.0.0.1:0".parse().unwrap())
        .await
        .expect("Server starts");
    let ws_url = format!("ws://{}/ws", handle.local_addr);

    // Host
    let (mut host_ws, _) = connect_async(&ws_url).await.unwrap();
    host_ws
        .send(Message::Text(
            serde_json::to_string(&ClientSignal::Join {
                room_code: VALID_CODE.into(),
                peer_id: "host-slot".into(),
                room_token: VALID_TOKEN.into(),
                role: Some(PeerRole::Host),
            })
            .unwrap(),
        ))
        .await
        .unwrap();

    // 4 Guests
    let mut guests = Vec::new();
    for i in 1..=4 {
        let (mut g_ws, _) = connect_async(&ws_url).await.unwrap();
        g_ws.send(Message::Text(
            serde_json::to_string(&ClientSignal::Join {
                room_code: VALID_CODE.into(),
                peer_id: format!("guest-slot-{}", i),
                room_token: VALID_TOKEN.into(),
                role: Some(PeerRole::Guest),
            })
            .unwrap(),
        ))
        .await
        .unwrap();
        guests.push(g_ws);
    }

    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(handle.registry.active_peers_count().await, 5);

    // 5th Guest rejected
    let (mut extra_ws, _) = connect_async(&ws_url).await.unwrap();
    extra_ws
        .send(Message::Text(
            serde_json::to_string(&ClientSignal::Join {
                room_code: VALID_CODE.into(),
                peer_id: "guest-extra".into(),
                room_token: VALID_TOKEN.into(),
                role: Some(PeerRole::Guest),
            })
            .unwrap(),
        ))
        .await
        .unwrap();

    let resp = extra_ws.next().await.unwrap().unwrap();
    let sig: ServerSignal = serde_json::from_str(resp.to_text().unwrap()).unwrap();
    match sig {
        ServerSignal::Error { code, .. } => assert_eq!(code, "ROOM_FULL"),
        other => panic!("Expected ROOM_FULL, got {:?}", other),
    }

    // Guest 2 disconnects
    let g2 = guests.remove(1);
    drop(g2);

    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(handle.registry.active_peers_count().await, 4);

    // Now extra guest can join
    let (mut success_ws, _) = connect_async(&ws_url).await.unwrap();
    success_ws
        .send(Message::Text(
            serde_json::to_string(&ClientSignal::Join {
                room_code: VALID_CODE.into(),
                peer_id: "guest-replacement".into(),
                room_token: VALID_TOKEN.into(),
                role: Some(PeerRole::Guest),
            })
            .unwrap(),
        ))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(handle.registry.active_peers_count().await, 5);

    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_adversarial_full_ice_candidate_matrix() {
    let handle = SignalingServer::start_default("127.0.0.1:0".parse().unwrap())
        .await
        .expect("Server starts");
    let ws_url = format!("ws://{}/ws", handle.local_addr);

    let (mut host_ws, _) = connect_async(&ws_url).await.unwrap();
    host_ws
        .send(Message::Text(
            serde_json::to_string(&ClientSignal::Join {
                room_code: VALID_CODE.into(),
                peer_id: "host-ice".into(),
                room_token: VALID_TOKEN.into(),
                role: Some(PeerRole::Host),
            })
            .unwrap(),
        ))
        .await
        .unwrap();

    let (mut guest_ws, _) = connect_async(&ws_url).await.unwrap();
    guest_ws
        .send(Message::Text(
            serde_json::to_string(&ClientSignal::Join {
                room_code: VALID_CODE.into(),
                peer_id: "guest-ice".into(),
                room_token: VALID_TOKEN.into(),
                role: Some(PeerRole::Guest),
            })
            .unwrap(),
        ))
        .await
        .unwrap();

    // Drain peer_joined
    let _ = host_ws.next().await.unwrap().unwrap();
    let _ = guest_ws.next().await.unwrap().unwrap();

    let candidates = vec![
        IceCandidate {
            candidate_type: IceCandidateType::Host,
            ip: "192.168.1.100".into(),
            port: 50000,
            protocol: "udp".into(),
            priority: 2130706431,
        },
        IceCandidate {
            candidate_type: IceCandidateType::ServerReflexive,
            ip: "203.0.113.10".into(),
            port: 54321,
            protocol: "udp".into(),
            priority: 1694498815,
        },
        IceCandidate {
            candidate_type: IceCandidateType::Relay,
            ip: "198.51.100.5".into(),
            port: 3478,
            protocol: "udp".into(),
            priority: 16777215,
        },
    ];

    for cand in candidates {
        let msg = ClientSignal::IceCandidate {
            sender_id: "guest-ice".into(),
            target_id: "host-ice".into(),
            candidate: cand.clone(),
        };
        guest_ws
            .send(Message::Text(serde_json::to_string(&msg).unwrap()))
            .await
            .unwrap();

        let host_rx = host_ws.next().await.unwrap().unwrap();
        let sig: ServerSignal = serde_json::from_str(host_rx.to_text().unwrap()).unwrap();
        match sig {
            ServerSignal::IceCandidate {
                sender_id,
                target_id,
                candidate,
            } => {
                assert_eq!(sender_id, "guest-ice");
                assert_eq!(target_id, "host-ice");
                assert_eq!(candidate, cand);
            }
            other => panic!("Expected IceCandidate, got {:?}", other),
        }
    }

    handle.shutdown().await.unwrap();
}
