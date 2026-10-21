use sync_engine::revision::{RevisionError, RevisionTracker};

#[test]
fn test_revision_gate_and_reorder() {
    let mut tracker = RevisionTracker::new(10);

    // Stale command: rev 9 < 10
    assert!(tracker.gate_command(9).is_err());
    // Stale command: rev 10 == 10
    assert!(tracker.gate_command(10).is_err());

    // Valid command: rev 11 > 10
    assert!(tracker.gate_command(11).is_ok());
    let new_rev = tracker.accept_command_and_advance(11).unwrap();
    assert_eq!(new_rev, 11);
    assert_eq!(tracker.current(), 11);

    // Client broadcast receiving:
    let mut client_tracker = RevisionTracker::new(5);
    // Broadcast 5 (duplicate) -> rejected
    assert!(client_tracker.apply_incoming_broadcast(5).is_err());
    // Broadcast 7 -> accepted
    assert!(client_tracker.apply_incoming_broadcast(7).is_ok());
    // Delayed broadcast 6 arriving after 7 -> rejected
    assert!(client_tracker.apply_incoming_broadcast(6).is_err());
}

#[test]
fn test_revision_advance() {
    let mut tracker = RevisionTracker::new(100);
    assert_eq!(tracker.advance().unwrap(), 101);
    assert_eq!(tracker.advance().unwrap(), 102);
    assert_eq!(tracker.current(), 102);
}

#[test]
fn test_revision_overflow() {
    let mut tracker = RevisionTracker::new(u64::MAX);
    assert!(matches!(tracker.advance(), Err(RevisionError::RevisionOverflow)));
}
