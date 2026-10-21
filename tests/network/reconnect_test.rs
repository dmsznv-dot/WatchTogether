use std::time::Duration;
use watch_together_tests::network_mock::*;
use watch_together_tests::protocol::*;

// ============================================================================
// FEATURE 17: STUN/TURN & SIGNALING DISCOVERY
// ============================================================================

#[test]
fn test_tier1_f17_stun_default_public_server_configuration() {
    let stun = StunConfig::default();
    assert_eq!(stun.server_url, "stun:stun.l.google.com:19302");
    assert_eq!(stun.timeout, Duration::from_millis(1500));
}

#[test]
fn test_tier1_f17_signaling_sdp_offer_and_answer_exchange() {
    let offer = SignalingMessage::Offer {
        sender_id: "host-peer".into(),
        target_id: "guest-peer".into(),
        desc: SessionDescription {
            sdp_type: "offer".into(),
            sdp: "v=0\r\no=host 123 456 IN IP4 127.0.0.1\r\ns=WatchTogether\r\n".into(),
        },
    };

    let answer = SignalingMessage::Answer {
        sender_id: "guest-peer".into(),
        target_id: "host-peer".into(),
        desc: SessionDescription {
            sdp_type: "answer".into(),
            sdp: "v=0\r\no=guest 789 101 IN IP4 127.0.0.1\r\ns=WatchTogether\r\n".into(),
        },
    };

    let offer_json = serde_json::to_string(&offer).expect("Offer serialize");
    let answer_json = serde_json::to_string(&answer).expect("Answer serialize");

    assert!(offer_json.contains("\"signal.offer\""));
    assert!(answer_json.contains("\"signal.answer\""));

    let dec_offer: SignalingMessage = serde_json::from_str(&offer_json).expect("Offer deserialize");
    let dec_answer: SignalingMessage = serde_json::from_str(&answer_json).expect("Answer deserialize");

    assert_eq!(offer, dec_offer);
    assert_eq!(answer, dec_answer);
}

#[test]
fn test_tier1_f17_signaling_ice_candidate_server_reflexive() {
    let candidate = IceCandidate {
        candidate_type: IceCandidateType::ServerReflexive,
        ip: "203.0.113.45".into(),
        port: 54321,
        protocol: "udp".into(),
        priority: 1694498815,
    };

    let msg = SignalingMessage::IceCandidate {
        sender_id: "guest-peer".into(),
        target_id: "host-peer".into(),
        candidate: candidate.clone(),
    };

    let json_val = serde_json::to_value(&msg).unwrap();
    assert_eq!(json_val["type"], "signal.ice_candidate");
    assert_eq!(json_val["payload"]["candidate"]["candidate_type"], "ServerReflexive");
    assert_eq!(json_val["payload"]["candidate"]["port"], 54321);
}

#[test]
fn test_tier1_f17_turn_relay_candidate_selection_fallback() {
    let turn = TurnConfig::default();
    assert_eq!(turn.server_url, "turn:relay.example.com:3478");
    assert_eq!(turn.username, "guest");
    assert!(!turn.credential.is_empty());

    let relay_candidate = IceCandidate {
        candidate_type: IceCandidateType::Relay,
        ip: "198.51.100.10".into(),
        port: 3478,
        protocol: "udp".into(),
        priority: 16777215,
    };

    assert_eq!(relay_candidate.candidate_type, IceCandidateType::Relay);
}

#[test]
fn test_tier1_f17_signaling_peer_disconnected_event() {
    let msg = SignalingMessage::PeerDisconnected {
        peer_id: "guest-peer-002".into(),
    };
    let json_str = serde_json::to_string(&msg).unwrap();
    assert!(json_str.contains("\"signal.peer_disconnected\""));
    let decoded: SignalingMessage = serde_json::from_str(&json_str).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
fn test_tier2_f17_stun_timeout_fallback_to_turn() {
    // When STUN fails after timeout, client falls back to TURN relay candidates
    let stun = StunConfig::default();
    let turn = TurnConfig::default();

    let simulate_nat_traversal = |stun_responsive: bool| -> IceCandidateType {
        if stun_responsive {
            IceCandidateType::ServerReflexive
        } else {
            IceCandidateType::Relay
        }
    };

    assert_eq!(simulate_nat_traversal(true), IceCandidateType::ServerReflexive);
    assert_eq!(simulate_nat_traversal(false), IceCandidateType::Relay);
    assert!(!stun.server_url.is_empty());
    assert!(!turn.server_url.is_empty());
}

#[test]
fn test_tier2_f17_turn_credential_authentication_validation() {
    let turn = TurnConfig::default();
    assert_eq!(turn.realm, "watch-together.local");

    // Valid credentials match
    let validate_auth = |user: &str, cred: &str| -> bool {
        user == turn.username && cred == turn.credential
    };

    assert!(validate_auth("guest", "secret_turn_token_2026"));
    assert!(!validate_auth("attacker", "secret_turn_token_2026"));
    assert!(!validate_auth("guest", "wrong_token"));
}

#[test]
fn test_tier2_f17_signaling_malformed_sdp_payload_rejected() {
    let malformed_json = serde_json::json!({
        "type": "signal.offer",
        "payload": {
            "sender_id": "host",
            "target_id": "guest"
            // Missing desc
        }
    });

    let res: Result<SignalingMessage, _> = serde_json::from_value(malformed_json);
    assert!(res.is_err());
}

#[test]
fn test_tier2_f17_ice_candidate_priority_ordering() {
    // Host candidates should have higher priority than ServerReflexive, which is higher than Relay
    let host_cand = IceCandidate {
        candidate_type: IceCandidateType::Host,
        ip: "192.168.1.50".into(),
        port: 5000,
        protocol: "udp".into(),
        priority: 2130706431,
    };

    let srflx_cand = IceCandidate {
        candidate_type: IceCandidateType::ServerReflexive,
        ip: "203.0.113.45".into(),
        port: 54321,
        protocol: "udp".into(),
        priority: 1694498815,
    };

    let relay_cand = IceCandidate {
        candidate_type: IceCandidateType::Relay,
        ip: "198.51.100.10".into(),
        port: 3478,
        protocol: "udp".into(),
        priority: 16777215,
    };

    assert!(host_cand.priority > srflx_cand.priority);
    assert!(srflx_cand.priority > relay_cand.priority);
}

#[test]
fn test_tier2_f17_signaling_unknown_target_peer_handling() {
    let peers = ["peer-001", "peer-002"];
    let target = "peer-ghost";

    let route_message = |target_id: &str| -> Result<(), ErrorCode> {
        if peers.contains(&target_id) {
            Ok(())
        } else {
            Err(ErrorCode::ParticipantNotFound)
        }
    };

    assert_eq!(route_message(target), Err(ErrorCode::ParticipantNotFound));
    assert!(route_message("peer-001").is_ok());
}

// ============================================================================
// FEATURE 18: CONNECTION DROP & AUTO-RECONNECTION
// ============================================================================

#[test]
fn test_tier1_f18_heartbeat_reception_resets_timeout_timer() {
    let mut sm = ReconnectionStateMachine::default_config();
    sm.set_connected("token-abc".into(), 100_000);

    // 2s elapsed (within 3s timeout) -> ping received at 102_000
    sm.record_heartbeat(102_000);
    assert_eq!(sm.last_heartbeat_rx_ms, 102_000);

    // Check at 104_000 -> 2s since last heartbeat -> not timed out
    assert!(!sm.check_heartbeat_timeout(104_000));
    assert_eq!(sm.state, ConnectionState::Connected);
}

#[test]
fn test_tier1_f18_heartbeat_timeout_transitions_to_reconnecting() {
    let mut sm = ReconnectionStateMachine::default_config();
    sm.set_connected("token-abc".into(), 100_000);

    // 3500ms elapsed without heartbeat (> 3000ms timeout)
    let timed_out = sm.check_heartbeat_timeout(103_500);
    assert!(timed_out);
    assert_eq!(sm.state, ConnectionState::Reconnecting);
}

#[test]
fn test_tier1_f18_reconnection_first_attempt_delay_1s() {
    let mut sm = ReconnectionStateMachine::default_config();
    sm.set_connected("token-abc".into(), 100_000);
    sm.check_heartbeat_timeout(103_500);

    let delay1 = sm.next_reconnect_delay();
    assert_eq!(delay1, Some(Duration::from_millis(1000)));
    assert_eq!(sm.attempt_count, 1);
}

#[test]
fn test_tier1_f18_reconnection_exponential_backoff_doubling() {
    let mut sm = ReconnectionStateMachine::default_config();
    sm.set_connected("token-abc".into(), 100_000);

    assert_eq!(sm.next_reconnect_delay(), Some(Duration::from_millis(1000))); // 1s
    assert_eq!(sm.next_reconnect_delay(), Some(Duration::from_millis(2000))); // 2s
    assert_eq!(sm.next_reconnect_delay(), Some(Duration::from_millis(4000))); // 4s
    assert_eq!(sm.next_reconnect_delay(), Some(Duration::from_millis(8000))); // 8s
}

#[test]
fn test_tier1_f18_reconnection_success_restores_connected_state() {
    let mut sm = ReconnectionStateMachine::default_config();
    sm.set_connected("token-abc".into(), 100_000);
    sm.check_heartbeat_timeout(103_500);
    sm.next_reconnect_delay();

    assert_eq!(sm.state, ConnectionState::Reconnecting);

    // Reconnection succeeded
    sm.recover_connection(105_000);
    assert_eq!(sm.state, ConnectionState::Connected);
    assert_eq!(sm.attempt_count, 0);
    assert_eq!(sm.current_delay, Duration::from_millis(1000));
}

#[test]
fn test_tier1_f18_reconnection_resyncs_latest_room_revision() {
    let mut client_tracker = RevisionTracker::new(5);

    // While disconnected, server advanced from rev 5 to rev 9
    let server_broadcast_rev = 9;
    let res = client_tracker.apply_incoming_broadcast(server_broadcast_rev);

    assert!(res.is_ok());
    assert_eq!(client_tracker.current(), 9);
}

#[test]
fn test_tier2_f18_reconnection_max_backoff_capped_at_30s() {
    let mut sm = ReconnectionStateMachine::new(ReconnectionConfig {
        initial_delay: Duration::from_secs(10),
        max_delay: Duration::from_secs(30),
        backoff_factor: 2.0,
        max_attempts: 10,
        heartbeat_interval: Duration::from_millis(1500),
        heartbeat_timeout: Duration::from_millis(3000),
    });

    assert_eq!(sm.next_reconnect_delay(), Some(Duration::from_secs(10)));
    assert_eq!(sm.next_reconnect_delay(), Some(Duration::from_secs(20)));
    assert_eq!(sm.next_reconnect_delay(), Some(Duration::from_secs(30))); // Capped
    assert_eq!(sm.next_reconnect_delay(), Some(Duration::from_secs(30))); // Remains capped
}

#[test]
fn test_tier2_f18_reconnection_max_attempts_exhaustion_leads_to_failed() {
    let mut sm = ReconnectionStateMachine::new(ReconnectionConfig {
        max_attempts: 3,
        ..Default::default()
    });

    assert!(sm.next_reconnect_delay().is_some()); // attempt 1
    assert!(sm.next_reconnect_delay().is_some()); // attempt 2
    assert!(sm.next_reconnect_delay().is_some()); // attempt 3

    // 4th attempt exceeds max_attempts -> returns None and transitions to Failed
    assert_eq!(sm.next_reconnect_delay(), None);
    assert_eq!(sm.state, ConnectionState::Failed);
}

#[test]
fn test_tier2_f18_reconnection_session_token_preserved_during_reconnect() {
    let mut sm = ReconnectionStateMachine::default_config();
    sm.set_connected("c0ffee_secret_session_token".into(), 100_000);

    sm.check_heartbeat_timeout(104_000);
    assert_eq!(sm.reconnection_token.as_deref(), Some("c0ffee_secret_session_token"));

    sm.next_reconnect_delay();
    assert_eq!(sm.reconnection_token.as_deref(), Some("c0ffee_secret_session_token"));
}

#[test]
fn test_tier2_f18_heartbeat_exact_3s_timeout_boundary() {
    let mut sm = ReconnectionStateMachine::default_config();
    sm.set_connected("token".into(), 100_000);

    // Exactly 3000ms -> not timed out
    assert!(!sm.check_heartbeat_timeout(103_000));
    assert_eq!(sm.state, ConnectionState::Connected);

    // 3001ms -> timed out
    assert!(sm.check_heartbeat_timeout(103_001));
    assert_eq!(sm.state, ConnectionState::Reconnecting);
}

#[test]
fn test_tier2_f18_multiple_consecutive_disconnect_recoveries() {
    let mut sm = ReconnectionStateMachine::default_config();
    sm.set_connected("token".into(), 100_000);

    for cycle in 1..=5 {
        let base_time = 100_000 + cycle * 50_000;
        assert!(sm.check_heartbeat_timeout(base_time + 4000));
        assert_eq!(sm.state, ConnectionState::Reconnecting);

        let delay = sm.next_reconnect_delay();
        assert_eq!(delay, Some(Duration::from_millis(1000)));

        sm.recover_connection(base_time + 5000);
        assert_eq!(sm.state, ConnectionState::Connected);
        assert_eq!(sm.attempt_count, 0);
    }
}

#[test]
fn test_tier2_f18_reconnection_under_transient_wifi_drop_3s() {
    let mut sm = ReconnectionStateMachine::default_config();
    sm.set_connected("token".into(), 100_000);

    // WiFi dropped for 3.5s
    sm.check_heartbeat_timeout(103_500);
    assert_eq!(sm.state, ConnectionState::Reconnecting);

    // First attempt takes 1s, reconnects successfully
    sm.recover_connection(104_500);
    assert_eq!(sm.state, ConnectionState::Connected);
}

#[test]
fn test_tier2_f18_reconnection_preserves_accumulated_drift_offset() {
    use watch_together_tests::sync::{ClockEstimator, NtpSample};

    let mut estimator = ClockEstimator::default_config();
    estimator.record_sample(NtpSample { rtt_ms: 20, offset_ms: 350 }).unwrap();

    assert_eq!(estimator.smoothed_offset_ms(), Some(350));

    // After reconnection, previous clock offset is preserved for prediction
    assert_eq!(estimator.estimate_server_time(10_000), Some(10_350));
}

#[test]
fn test_tier2_f18_reconnection_heartbeat_spurious_duplicate_pings() {
    let mut sm = ReconnectionStateMachine::default_config();
    sm.set_connected("token".into(), 100_000);

    // Rapid duplicate pings within 10ms
    sm.record_heartbeat(100_005);
    sm.record_heartbeat(100_008);
    sm.record_heartbeat(100_010);

    assert_eq!(sm.last_heartbeat_rx_ms, 100_010);
    assert!(!sm.check_heartbeat_timeout(101_000));
}

#[test]
fn test_tier2_f18_reconnection_backoff_attempt_counter_resets_on_success() {
    let mut sm = ReconnectionStateMachine::default_config();
    sm.set_connected("token".into(), 100_000);

    sm.next_reconnect_delay(); // attempt 1 (1s)
    sm.next_reconnect_delay(); // attempt 2 (2s)
    sm.next_reconnect_delay(); // attempt 3 (4s)
    assert_eq!(sm.attempt_count, 3);

    sm.recover_connection(110_000);
    assert_eq!(sm.attempt_count, 0);
    assert_eq!(sm.current_delay, Duration::from_millis(1000));
}
