use network::{ConnectionState, ReconnectionStateMachine};
use std::time::Duration;

#[test]
fn test_reconnect_exact_3s_timeout_boundary() {
    let mut sm = ReconnectionStateMachine::default_config();
    sm.set_connected("tok_123".into(), 100_000);

    // Exactly 3000ms: not timed out
    assert!(!sm.check_heartbeat_timeout(103_000));
    assert_eq!(sm.state, ConnectionState::Connected);

    // 3001ms: timed out
    assert!(sm.check_heartbeat_timeout(103_001));
    assert_eq!(sm.state, ConnectionState::Reconnecting);
}

#[test]
fn test_exponential_backoff_and_capping() {
    let mut sm = ReconnectionStateMachine::default_config();
    sm.set_connected("tok_123".into(), 100_000);

    // 1s
    assert_eq!(sm.next_reconnect_delay(), Some(Duration::from_millis(1000)));
    // 2s
    assert_eq!(sm.next_reconnect_delay(), Some(Duration::from_millis(2000)));
    // 4s
    assert_eq!(sm.next_reconnect_delay(), Some(Duration::from_millis(4000)));
    // 8s
    assert_eq!(sm.next_reconnect_delay(), Some(Duration::from_millis(8000)));
    // 16s
    assert_eq!(sm.next_reconnect_delay(), Some(Duration::from_millis(16000)));

    // Max attempts exhausted (default max_attempts is 5)
    assert_eq!(sm.next_reconnect_delay(), None);
    assert_eq!(sm.state, ConnectionState::Failed);
}

#[test]
fn test_token_preservation_and_recovery() {
    let mut sm = ReconnectionStateMachine::default_config();
    sm.set_connected("my_secret_token".into(), 100_000);

    sm.check_heartbeat_timeout(104_000);
    assert_eq!(sm.state, ConnectionState::Reconnecting);
    assert_eq!(sm.reconnection_token.as_deref(), Some("my_secret_token"));

    sm.recover_connection(105_000);
    assert_eq!(sm.state, ConnectionState::Connected);
    assert_eq!(sm.attempt_count, 0);
    assert_eq!(sm.current_delay, Duration::from_millis(1000));
    assert_eq!(sm.reconnection_token.as_deref(), Some("my_secret_token"));
}
