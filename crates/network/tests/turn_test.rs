use network::{IceCandidateType, TurnConfig};

#[test]
fn test_turn_default_config() {
    let turn = TurnConfig::default();
    assert_eq!(turn.server_url, "turn:relay.example.com:3478");
    assert_eq!(turn.username, "guest");
    assert_eq!(turn.credential, "secret_turn_token_2026");
    assert_eq!(turn.realm, "watch-together.local");
}

#[test]
fn test_turn_credential_validation() {
    let turn = TurnConfig::default();
    assert!(turn.validate_credentials("guest", "secret_turn_token_2026"));
    assert!(!turn.validate_credentials("guest", "wrong_token"));
    assert!(!turn.validate_credentials("admin", "secret_turn_token_2026"));
}

#[test]
fn test_turn_relay_candidate_creation() {
    let turn = TurnConfig::default();
    let cand = turn.create_relay_candidate("198.51.100.1".into(), 3478);
    assert_eq!(cand.candidate_type, IceCandidateType::Relay);
    assert_eq!(cand.ip, "198.51.100.1");
    assert_eq!(cand.port, 3478);
    assert_eq!(cand.priority, 16777215);
}

#[test]
fn test_fallback_selection() {
    let turn = TurnConfig::default();
    assert_eq!(
        turn.fallback_candidate(true),
        IceCandidateType::ServerReflexive
    );
    assert_eq!(
        turn.fallback_candidate(false),
        IceCandidateType::Relay
    );
}
