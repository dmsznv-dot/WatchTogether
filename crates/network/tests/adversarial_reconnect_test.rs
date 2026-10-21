use network::{ConnectionState, ReconnectionConfig, ReconnectionStateMachine};
use std::time::Duration;

#[test]
fn test_exact_3000ms_heartbeat_boundary_conditions() {
    let mut sm = ReconnectionStateMachine::default_config();
    let connect_time = 500_000u64;
    sm.set_connected("reconnect_token_xyz".into(), connect_time);

    assert_eq!(sm.state, ConnectionState::Connected);
    assert_eq!(sm.last_heartbeat_rx_ms, connect_time);

    // 1ms before timeout (2999ms elapsed) -> ALIVE
    assert!(
        !sm.check_heartbeat_timeout(connect_time + 2999),
        "At 2999ms, connection should still be alive"
    );
    assert_eq!(sm.state, ConnectionState::Connected);

    // Exactly at timeout boundary (3000ms elapsed) -> ALIVE (timeout_ms is not strictly greater)
    assert!(
        !sm.check_heartbeat_timeout(connect_time + 3000),
        "At exactly 3000ms, connection should not be timed out"
    );
    assert_eq!(sm.state, ConnectionState::Connected);

    // 1ms after timeout boundary (3001ms elapsed) -> TIMED OUT
    assert!(
        sm.check_heartbeat_timeout(connect_time + 3001),
        "At 3001ms, connection must be timed out"
    );
    assert_eq!(sm.state, ConnectionState::Reconnecting);

    // Further checks while already Reconnecting should return false
    assert!(
        !sm.check_heartbeat_timeout(connect_time + 10_000),
        "check_heartbeat_timeout should return false when not Connected"
    );
    assert_eq!(sm.state, ConnectionState::Reconnecting);
}

#[test]
fn test_exponential_backoff_sequence_full_with_30s_cap() {
    let mut sm = ReconnectionStateMachine::new(ReconnectionConfig {
        initial_delay: Duration::from_millis(1000),
        max_delay: Duration::from_secs(30),
        backoff_factor: 2.0,
        max_attempts: 10,
        heartbeat_interval: Duration::from_millis(1500),
        heartbeat_timeout: Duration::from_millis(3000),
    });

    sm.set_connected("token_abc".into(), 100_000);
    sm.check_heartbeat_timeout(103_001);
    assert_eq!(sm.state, ConnectionState::Reconnecting);

    // Attempt 1: 1s
    assert_eq!(sm.next_reconnect_delay(), Some(Duration::from_millis(1000)));
    assert_eq!(sm.attempt_count, 1);
    assert_eq!(sm.state, ConnectionState::Reconnecting);

    // Attempt 2: 2s
    assert_eq!(sm.next_reconnect_delay(), Some(Duration::from_millis(2000)));
    assert_eq!(sm.attempt_count, 2);

    // Attempt 3: 4s
    assert_eq!(sm.next_reconnect_delay(), Some(Duration::from_millis(4000)));
    assert_eq!(sm.attempt_count, 3);

    // Attempt 4: 8s
    assert_eq!(sm.next_reconnect_delay(), Some(Duration::from_millis(8000)));
    assert_eq!(sm.attempt_count, 4);

    // Attempt 5: 16s
    assert_eq!(sm.next_reconnect_delay(), Some(Duration::from_millis(16000)));
    assert_eq!(sm.attempt_count, 5);

    // Attempt 6: 30s (capped from 32s)
    assert_eq!(sm.next_reconnect_delay(), Some(Duration::from_millis(30000)));
    assert_eq!(sm.attempt_count, 6);

    // Attempt 7: 30s (remains capped)
    assert_eq!(sm.next_reconnect_delay(), Some(Duration::from_millis(30000)));
    assert_eq!(sm.attempt_count, 7);

    // Attempt 8: 30s
    assert_eq!(sm.next_reconnect_delay(), Some(Duration::from_millis(30000)));
    assert_eq!(sm.attempt_count, 8);

    // Attempt 9: 30s
    assert_eq!(sm.next_reconnect_delay(), Some(Duration::from_millis(30000)));
    assert_eq!(sm.attempt_count, 9);

    // Attempt 10: 30s
    assert_eq!(sm.next_reconnect_delay(), Some(Duration::from_millis(30000)));
    assert_eq!(sm.attempt_count, 10);

    // Attempt 11: Exhausted -> None, transitions to Failed
    assert_eq!(sm.next_reconnect_delay(), None);
    assert_eq!(sm.state, ConnectionState::Failed);
}

#[test]
fn test_token_preservation_and_state_resync_cycle() {
    let mut sm = ReconnectionStateMachine::default_config();
    let original_token = "cryptographically_strong_random_token_1234567890".to_string();

    sm.set_connected(original_token.clone(), 1_000_000);
    assert_eq!(sm.reconnection_token.as_ref(), Some(&original_token));

    // Simulate drop at 1_004_000
    assert!(sm.check_heartbeat_timeout(1_004_000));
    assert_eq!(sm.state, ConnectionState::Reconnecting);
    assert_eq!(sm.reconnection_token.as_ref(), Some(&original_token));

    // Advance 2 retry attempts
    let d1 = sm.next_reconnect_delay();
    assert_eq!(d1, Some(Duration::from_millis(1000)));
    let d2 = sm.next_reconnect_delay();
    assert_eq!(d2, Some(Duration::from_millis(2000)));
    assert_eq!(sm.attempt_count, 2);

    // Token must STILL be preserved during retries
    assert_eq!(sm.reconnection_token.as_ref(), Some(&original_token));

    // Reconnection succeeds and recovers
    sm.recover_connection(1_007_000);
    assert_eq!(sm.state, ConnectionState::Connected);
    assert_eq!(sm.attempt_count, 0);
    assert_eq!(sm.current_delay, Duration::from_millis(1000));
    assert_eq!(sm.last_heartbeat_rx_ms, 1_007_000);
    assert_eq!(sm.reconnection_token.as_ref(), Some(&original_token));

    // Next heartbeat arrives
    sm.record_heartbeat(1_008_500);
    assert_eq!(sm.last_heartbeat_rx_ms, 1_008_500);
    assert!(!sm.check_heartbeat_timeout(1_011_000));
}
