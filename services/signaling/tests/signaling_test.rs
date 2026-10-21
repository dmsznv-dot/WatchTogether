use futures_util::{SinkExt, StreamExt};
use signaling::{
    ClientSignal, IceCandidate, IceCandidateType, PeerRole, ServerSignal, SessionDescription,
    SignalingServer,
};
use std::time::Duration;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

const VALID_ROOM_CODE: &str = "ABC234";
const VALID_TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[tokio::test]
async fn test_health_endpoint() {
    let handle = SignalingServer::start_default("127.0.0.1:0".parse().unwrap())
        .await
        .expect("Server starts");

    let client = reqwest::Client::new();
    let health_url = format!("http://{}/health", handle.local_addr);
    let resp = client.get(&health_url).send().await.unwrap();
    assert_eq!(resp.status(), 200);

    let text = resp.text().await.unwrap();
    let body: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(body["status"], "ok");
    assert_eq!(body["active_rooms"], 0);
    assert_eq!(body["active_peers"], 0);

    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_signaling_full_p2p_handshake_and_lifecycle() {
    let handle = SignalingServer::start_default("127.0.0.1:0".parse().unwrap())
        .await
        .expect("Server starts");
    let ws_url = format!("ws://{}/ws", handle.local_addr);

    // 1. Host registers room
    let (mut host_ws, _) = connect_async(&ws_url).await.expect("Host connects");
    let host_join = ClientSignal::Join {
        room_code: VALID_ROOM_CODE.into(),
        peer_id: "host-peer".into(),
        room_token: VALID_TOKEN.into(),
        role: Some(PeerRole::Host),
    };
    host_ws
        .send(Message::Text(serde_json::to_string(&host_join).unwrap()))
        .await
        .unwrap();

    // 2. Guest connects and joins
    let (mut guest_ws, _) = connect_async(&ws_url).await.expect("Guest connects");
    let guest_join = ClientSignal::Join {
        room_code: VALID_ROOM_CODE.into(),
        peer_id: "guest-peer".into(),
        room_token: VALID_TOKEN.into(),
        role: Some(PeerRole::Guest),
    };
    guest_ws
        .send(Message::Text(serde_json::to_string(&guest_join).unwrap()))
        .await
        .unwrap();

    // Host receives peer_joined notification
    let host_rx = host_ws.next().await.unwrap().unwrap();
    let peer_joined_msg: ServerSignal =
        serde_json::from_str(host_rx.to_text().unwrap()).unwrap();
    match peer_joined_msg {
        ServerSignal::PeerJoined { peer_id, role } => {
            assert_eq!(peer_id, "guest-peer");
            assert_eq!(role, PeerRole::Guest);
        }
        other => panic!("Expected PeerJoined, got {:?}", other),
    }

    // Guest receives existing host peer
    let guest_rx = guest_ws.next().await.unwrap().unwrap();
    let guest_peer_msg: ServerSignal =
        serde_json::from_str(guest_rx.to_text().unwrap()).unwrap();
    match guest_peer_msg {
        ServerSignal::PeerJoined { peer_id, .. } => {
            assert_eq!(peer_id, "host-peer");
        }
        other => panic!("Expected PeerJoined for host, got {:?}", other),
    }

    // 3. Host sends SDP Offer to Guest
    let offer_desc = SessionDescription {
        sdp_type: "offer".into(),
        sdp: "v=0\r\no=host 123 456 IN IP4 127.0.0.1\r\ns=WatchTogether\r\n".into(),
    };
    let offer_msg = ClientSignal::Offer {
        sender_id: "host-peer".into(),
        target_id: "guest-peer".into(),
        desc: offer_desc.clone(),
    };
    host_ws
        .send(Message::Text(serde_json::to_string(&offer_msg).unwrap()))
        .await
        .unwrap();

    // Guest receives SDP Offer
    let guest_offer_rx = guest_ws.next().await.unwrap().unwrap();
    let recv_offer: ServerSignal =
        serde_json::from_str(guest_offer_rx.to_text().unwrap()).unwrap();
    match recv_offer {
        ServerSignal::Offer {
            sender_id,
            target_id,
            desc,
        } => {
            assert_eq!(sender_id, "host-peer");
            assert_eq!(target_id, "guest-peer");
            assert_eq!(desc, offer_desc);
        }
        other => panic!("Expected Offer, got {:?}", other),
    }

    // 4. Guest sends SDP Answer to Host
    let answer_desc = SessionDescription {
        sdp_type: "answer".into(),
        sdp: "v=0\r\no=guest 789 101 IN IP4 127.0.0.1\r\ns=WatchTogether\r\n".into(),
    };
    let answer_msg = ClientSignal::Answer {
        sender_id: "guest-peer".into(),
        target_id: "host-peer".into(),
        desc: answer_desc.clone(),
    };
    guest_ws
        .send(Message::Text(serde_json::to_string(&answer_msg).unwrap()))
        .await
        .unwrap();

    // Host receives SDP Answer
    let host_answer_rx = host_ws.next().await.unwrap().unwrap();
    let recv_answer: ServerSignal =
        serde_json::from_str(host_answer_rx.to_text().unwrap()).unwrap();
    match recv_answer {
        ServerSignal::Answer {
            sender_id,
            target_id,
            desc,
        } => {
            assert_eq!(sender_id, "guest-peer");
            assert_eq!(target_id, "host-peer");
            assert_eq!(desc, answer_desc);
        }
        other => panic!("Expected Answer, got {:?}", other),
    }

    // 5. ICE Candidate exchange
    let candidate = IceCandidate {
        candidate_type: IceCandidateType::ServerReflexive,
        ip: "203.0.113.45".into(),
        port: 54321,
        protocol: "udp".into(),
        priority: 1694498815,
    };
    let ice_msg = ClientSignal::IceCandidate {
        sender_id: "guest-peer".into(),
        target_id: "host-peer".into(),
        candidate: candidate.clone(),
    };
    guest_ws
        .send(Message::Text(serde_json::to_string(&ice_msg).unwrap()))
        .await
        .unwrap();

    // Host receives ICE candidate
    let host_ice_rx = host_ws.next().await.unwrap().unwrap();
    let recv_ice: ServerSignal =
        serde_json::from_str(host_ice_rx.to_text().unwrap()).unwrap();
    match recv_ice {
        ServerSignal::IceCandidate {
            sender_id,
            target_id,
            candidate: c,
        } => {
            assert_eq!(sender_id, "guest-peer");
            assert_eq!(target_id, "host-peer");
            assert_eq!(c, candidate);
        }
        other => panic!("Expected IceCandidate, got {:?}", other),
    }

    // 6. Unknown target returns PARTICIPANT_NOT_FOUND
    let bad_ice = ClientSignal::IceCandidate {
        sender_id: "guest-peer".into(),
        target_id: "non-existent-peer".into(),
        candidate: candidate.clone(),
    };
    guest_ws
        .send(Message::Text(serde_json::to_string(&bad_ice).unwrap()))
        .await
        .unwrap();

    let err_rx = guest_ws.next().await.unwrap().unwrap();
    let recv_err: ServerSignal = serde_json::from_str(err_rx.to_text().unwrap()).unwrap();
    match recv_err {
        ServerSignal::Error { code, .. } => {
            assert_eq!(code, "PARTICIPANT_NOT_FOUND");
        }
        other => panic!("Expected Error PARTICIPANT_NOT_FOUND, got {:?}", other),
    }

    // 7. Host disconnects -> Guest receives RoomClosed and room is purged
    drop(host_ws);

    let guest_closed_rx = guest_ws.next().await.unwrap().unwrap();
    let recv_closed: ServerSignal =
        serde_json::from_str(guest_closed_rx.to_text().unwrap()).unwrap();
    match recv_closed {
        ServerSignal::RoomClosed { reason } => {
            assert_eq!(reason, "host_disconnected");
        }
        other => panic!("Expected RoomClosed, got {:?}", other),
    }

    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(handle.registry.active_rooms_count().await, 0);

    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_signaling_error_conditions() {
    let handle = SignalingServer::start_default("127.0.0.1:0".parse().unwrap())
        .await
        .expect("Server starts");
    let ws_url = format!("ws://{}/ws", handle.local_addr);

    // 1. Join non-existent room code
    let (mut ws, _) = connect_async(&ws_url).await.expect("Connects");
    let bad_join = ClientSignal::Join {
        room_code: "XYZ999".into(),
        peer_id: "guest-1".into(),
        room_token: VALID_TOKEN.into(),
        role: Some(PeerRole::Guest),
    };
    ws.send(Message::Text(serde_json::to_string(&bad_join).unwrap()))
        .await
        .unwrap();

    let resp = ws.next().await.unwrap().unwrap();
    let err: ServerSignal = serde_json::from_str(resp.to_text().unwrap()).unwrap();
    match err {
        ServerSignal::Error { code, .. } => assert_eq!(code, "ROOM_NOT_FOUND"),
        other => panic!("Expected ROOM_NOT_FOUND, got {:?}", other),
    }

    // 2. Malformed JSON message
    ws.send(Message::Text("not valid json".into()))
        .await
        .unwrap();
    let resp2 = ws.next().await.unwrap().unwrap();
    let err2: ServerSignal = serde_json::from_str(resp2.to_text().unwrap()).unwrap();
    match err2 {
        ServerSignal::Error { code, .. } => assert_eq!(code, "INVALID_MESSAGE"),
        other => panic!("Expected INVALID_MESSAGE, got {:?}", other),
    }

    // 3. Register host with valid code and token
    let (mut host_ws, _) = connect_async(&ws_url).await.unwrap();
    host_ws
        .send(Message::Text(
            serde_json::to_string(&ClientSignal::Join {
                room_code: VALID_ROOM_CODE.into(),
                peer_id: "host-1".into(),
                room_token: VALID_TOKEN.into(),
                role: Some(PeerRole::Host),
            })
            .unwrap(),
        ))
        .await
        .unwrap();

    // 4. Guest with invalid token
    let (mut guest_ws, _) = connect_async(&ws_url).await.unwrap();
    guest_ws
        .send(Message::Text(
            serde_json::to_string(&ClientSignal::Join {
                room_code: VALID_ROOM_CODE.into(),
                peer_id: "guest-bad-token".into(),
                room_token: "bad_token_that_is_at_least_32_characters_long_123".into(),
                role: Some(PeerRole::Guest),
            })
            .unwrap(),
        ))
        .await
        .unwrap();

    let resp3 = guest_ws.next().await.unwrap().unwrap();
    let err3: ServerSignal = serde_json::from_str(resp3.to_text().unwrap()).unwrap();
    match err3 {
        ServerSignal::Error { code, .. } => assert_eq!(code, "INVALID_TOKEN"),
        other => panic!("Expected INVALID_TOKEN, got {:?}", other),
    }

    handle.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_capacity_cap_rejection() {
    let handle = SignalingServer::start_default("127.0.0.1:0".parse().unwrap())
        .await
        .expect("Server starts");
    let ws_url = format!("ws://{}/ws", handle.local_addr);

    // Host
    let (mut host_ws, _) = connect_async(&ws_url).await.unwrap();
    host_ws
        .send(Message::Text(
            serde_json::to_string(&ClientSignal::Join {
                room_code: VALID_ROOM_CODE.into(),
                peer_id: "host-1".into(),
                room_token: VALID_TOKEN.into(),
                role: Some(PeerRole::Host),
            })
            .unwrap(),
        ))
        .await
        .unwrap();

    // Add 4 guests (total 5)
    let mut guests = Vec::new();
    for i in 1..=4 {
        let (mut g_ws, _) = connect_async(&ws_url).await.unwrap();
        g_ws.send(Message::Text(
            serde_json::to_string(&ClientSignal::Join {
                room_code: VALID_ROOM_CODE.into(),
                peer_id: format!("guest-{}", i),
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

    // 6th participant rejected with ROOM_FULL
    let (mut extra_ws, _) = connect_async(&ws_url).await.unwrap();
    extra_ws
        .send(Message::Text(
            serde_json::to_string(&ClientSignal::Join {
                room_code: VALID_ROOM_CODE.into(),
                peer_id: "guest-6".into(),
                room_token: VALID_TOKEN.into(),
                role: Some(PeerRole::Guest),
            })
            .unwrap(),
        ))
        .await
        .unwrap();

    let resp = extra_ws.next().await.unwrap().unwrap();
    let err: ServerSignal = serde_json::from_str(resp.to_text().unwrap()).unwrap();
    match err {
        ServerSignal::Error { code, .. } => assert_eq!(code, "ROOM_FULL"),
        other => panic!("Expected ROOM_FULL, got {:?}", other),
    }

    handle.shutdown().await.unwrap();
}
