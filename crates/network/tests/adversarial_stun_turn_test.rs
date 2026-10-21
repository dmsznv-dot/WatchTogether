use network::{
    compute_ice_candidate_priority, resolve_server_reflexive_candidate, IceCandidateType,
    NetworkError, StunConfig, TurnConfig, BINDING_RESPONSE_SUCCESS, STUN_MAGIC_COOKIE,
};
use std::time::Duration;
use tokio::net::UdpSocket;

#[tokio::test]
async fn test_stun_timeout_on_packet_drop() {
    // Bind a UDP socket that drops all packets (never responds)
    let drop_server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let drop_addr = drop_server.local_addr().unwrap();

    let client_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let config = StunConfig {
        server_url: format!("stun:{}", drop_addr),
        timeout: Duration::from_millis(50),
    };

    let start = std::time::Instant::now();
    let result = resolve_server_reflexive_candidate(&client_socket, &config).await;
    let elapsed = start.elapsed();

    assert!(elapsed >= Duration::from_millis(45));
    match result {
        Err(NetworkError::StunTimeout(t)) => {
            assert_eq!(t, Duration::from_millis(50));
        }
        other => panic!("Expected StunTimeout, got {:?}", other),
    }

    // Verify TURN fallback logic under STUN timeout
    let turn = TurnConfig::default();
    assert_eq!(turn.fallback_candidate(false), IceCandidateType::Relay);
}

#[tokio::test]
async fn test_stun_delayed_response_exceeding_timeout() {
    // Bind a UDP socket that delays response past timeout
    let slow_server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let slow_addr = slow_server.local_addr().unwrap();

    let server_handle = tokio::spawn(async move {
        let mut buf = [0u8; 128];
        let (len, src) = slow_server.recv_from(&mut buf).await.unwrap();
        // Delay 150ms (timeout is 50ms)
        tokio::time::sleep(Duration::from_millis(150)).await;
        let _ = slow_server.send_to(&buf[..len], src).await;
    });

    let client_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let config = StunConfig {
        server_url: format!("stun:{}", slow_addr),
        timeout: Duration::from_millis(50),
    };

    let result = resolve_server_reflexive_candidate(&client_socket, &config).await;
    match result {
        Err(NetworkError::StunTimeout(_)) => {}
        other => panic!("Expected StunTimeout, got {:?}", other),
    }

    server_handle.await.unwrap();
}

#[tokio::test]
async fn test_stun_corrupted_response_truncated() {
    // Server sends back truncated packet (<20 bytes)
    let bad_server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let bad_addr = bad_server.local_addr().unwrap();

    let server_handle = tokio::spawn(async move {
        let mut buf = [0u8; 128];
        let (_, src) = bad_server.recv_from(&mut buf).await.unwrap();
        // Send back 10 bytes only
        let _ = bad_server.send_to(&[0u8; 10], src).await;
    });

    let client_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let config = StunConfig {
        server_url: format!("stun:{}", bad_addr),
        timeout: Duration::from_millis(200),
    };

    let result = resolve_server_reflexive_candidate(&client_socket, &config).await;
    match result {
        Err(NetworkError::Stun(msg)) => {
            assert!(msg.contains("smaller than 20-byte"));
        }
        other => panic!("Expected NetworkError::Stun, got {:?}", other),
    }

    server_handle.await.unwrap();
}

#[tokio::test]
async fn test_stun_corrupted_response_tx_id_mismatch() {
    let bad_server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let bad_addr = bad_server.local_addr().unwrap();

    let server_handle = tokio::spawn(async move {
        let mut buf = [0u8; 128];
        let (_, src) = bad_server.recv_from(&mut buf).await.unwrap();

        // Craft response with wrong transaction ID
        let mut resp = [0u8; 32];
        resp[0..2].copy_from_slice(&BINDING_RESPONSE_SUCCESS.to_be_bytes());
        resp[2..4].copy_from_slice(&0u16.to_be_bytes());
        resp[4..8].copy_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
        resp[8..20].copy_from_slice(&[0xff; 12]); // Wrong tx_id
        let _ = bad_server.send_to(&resp, src).await;
    });

    let client_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let config = StunConfig {
        server_url: format!("stun:{}", bad_addr),
        timeout: Duration::from_millis(200),
    };

    let result = resolve_server_reflexive_candidate(&client_socket, &config).await;
    match result {
        Err(NetworkError::Stun(msg)) => {
            assert!(msg.contains("transaction ID mismatch"));
        }
        other => panic!("Expected NetworkError::Stun, got {:?}", other),
    }

    server_handle.await.unwrap();
}

#[tokio::test]
async fn test_stun_successful_resolution_and_turn_priority() {
    let stun_server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let server_addr = stun_server.local_addr().unwrap();

    let server_handle = tokio::spawn(async move {
        let mut buf = [0u8; 128];
        let (len, src) = stun_server.recv_from(&mut buf).await.unwrap();
        assert!(len >= 20);

        let tx_id = &buf[8..20];

        // Craft valid STUN Binding Response with XOR-MAPPED-ADDRESS
        // Attribute: type=0x0020, len=8
        // Payload: 0x00, 0x01 (IPv4), xor_port, xor_ip
        let mut resp = Vec::new();
        resp.extend_from_slice(&BINDING_RESPONSE_SUCCESS.to_be_bytes());
        resp.extend_from_slice(&12u16.to_be_bytes()); // attr header (4) + attr payload (8) = 12
        resp.extend_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
        resp.extend_from_slice(tx_id);

        // Attr header
        resp.extend_from_slice(&0x0020u16.to_be_bytes());
        resp.extend_from_slice(&8u16.to_be_bytes());

        // Target mapped addr: 198.51.100.77:4321
        let mapped_port: u16 = 4321;
        let xor_port = mapped_port ^ ((STUN_MAGIC_COOKIE >> 16) as u16);
        let mapped_ip: u32 = u32::from_be_bytes([198, 51, 100, 77]);
        let xor_ip = mapped_ip ^ STUN_MAGIC_COOKIE;

        resp.push(0x00);
        resp.push(0x01); // IPv4
        resp.extend_from_slice(&xor_port.to_be_bytes());
        resp.extend_from_slice(&xor_ip.to_be_bytes());

        stun_server.send_to(&resp, src).await.unwrap();
    });

    let client_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let config = StunConfig {
        server_url: format!("stun:{}", server_addr),
        timeout: Duration::from_millis(500),
    };

    let candidate = resolve_server_reflexive_candidate(&client_socket, &config)
        .await
        .expect("STUN resolves successfully");

    assert_eq!(candidate.candidate_type, IceCandidateType::ServerReflexive);
    assert_eq!(candidate.ip, "198.51.100.77");
    assert_eq!(candidate.port, 4321);
    assert_eq!(candidate.protocol, "udp");
    assert_eq!(candidate.priority, 1694498815);

    // Verify TURN fallback candidate logic when STUN succeeds
    let turn = TurnConfig::default();
    assert_eq!(
        turn.fallback_candidate(true),
        IceCandidateType::ServerReflexive
    );

    // Verify priority relationships
    let host_pri = compute_ice_candidate_priority(&IceCandidateType::Host);
    let srflx_pri = compute_ice_candidate_priority(&IceCandidateType::ServerReflexive);
    let relay_pri = compute_ice_candidate_priority(&IceCandidateType::Relay);
    assert!(host_pri > srflx_pri);
    assert!(srflx_pri > relay_pri);

    server_handle.await.unwrap();
}

#[test]
fn test_turn_adversarial_credential_matrix() {
    let turn = TurnConfig {
        server_url: "turn:relay.example.com:3478".into(),
        username: "alice".into(),
        credential: "secure_random_credential_token_99".into(),
        realm: "watch-together.local".into(),
    };

    assert!(turn.validate_credentials("alice", "secure_random_credential_token_99"));
    assert!(!turn.validate_credentials("Alice", "secure_random_credential_token_99")); // case-sensitive
    assert!(!turn.validate_credentials("alice", "SECURE_RANDOM_CREDENTIAL_TOKEN_99"));
    assert!(!turn.validate_credentials("", "secure_random_credential_token_99"));
    assert!(!turn.validate_credentials("alice", ""));
    assert!(!turn.validate_credentials("", ""));
    assert!(!turn.validate_credentials("bob", "wrong_pass"));
}
