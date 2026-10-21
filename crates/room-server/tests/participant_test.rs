use room_server::{ParticipantRegistry, RoomServerError};
use shared_protocol::models::{ParticipantId, ParticipantRole};

#[test]
fn test_capacity_boundary_5_accepted_6_rejected() {
    let host_id = ParticipantId::new("part-host-001");
    let mut registry = ParticipantRegistry::new(host_id.clone(), "Host".to_string(), 5, 1000);

    assert_eq!(registry.connected_count(), 1);
    assert!(!registry.is_full());

    // Add 4 guests (total 5)
    let g1 = registry.register_guest("Guest 1", 0, None, 1001).expect("G1 joins");
    let g2 = registry.register_guest("Guest 2", 0, None, 1002).expect("G2 joins");
    let g3 = registry.register_guest("Guest 3", 0, None, 1003).expect("G3 joins");
    let g4 = registry.register_guest("Guest 4", 0, None, 1004).expect("G4 joins");

    assert_eq!(registry.connected_count(), 5);
    assert!(registry.is_full());

    // 6th participant rejected
    let res = registry.register_guest("Guest 5", 0, None, 1005);
    assert!(res.is_err());
    match res.unwrap_err() {
        RoomServerError::RoomFull { capacity } => assert_eq!(capacity, 5),
        other => panic!("Expected RoomFull, got {:?}", other),
    }

    // Verify participants list contains host + 4 guests
    let list = registry.list();
    assert_eq!(list.len(), 5);
    assert_eq!(list[0].id, host_id);
    assert_eq!(list[0].role, ParticipantRole::Host);
    assert_eq!(list[1].id, g1.id);
    assert_eq!(list[2].id, g2.id);
    assert_eq!(list[3].id, g3.id);
    assert_eq!(list[4].id, g4.id);
}

#[test]
fn test_slot_freed_on_leave() {
    let host_id = ParticipantId::new("part-host-001");
    let mut registry = ParticipantRegistry::new(host_id, "Host".to_string(), 5, 1000);

    let _g1 = registry.register_guest("Guest 1", 0, None, 1001).unwrap();
    let _g2 = registry.register_guest("Guest 2", 0, None, 1002).unwrap();
    let _g3 = registry.register_guest("Guest 3", 0, None, 1003).unwrap();
    let g4 = registry.register_guest("Guest 4", 0, None, 1004).unwrap();

    assert!(registry.is_full());

    // Guest 4 leaves
    let removed = registry.remove(&g4.id).expect("remove G4");
    assert_eq!(removed.id, g4.id);
    assert_eq!(registry.connected_count(), 4);
    assert!(!registry.is_full());

    // Another guest joins
    let g5 = registry.register_guest("Guest 5", 0, None, 1005).expect("G5 joins");
    assert_eq!(registry.connected_count(), 5);
    assert_eq!(g5.display_name, "Guest 5");
}

#[test]
fn test_slot_freed_on_kick() {
    let host_id = ParticipantId::new("part-host-001");
    let mut registry = ParticipantRegistry::new(host_id, "Host".to_string(), 5, 1000);

    let g1 = registry.register_guest("Guest 1", 0, None, 1001).unwrap();
    let _g2 = registry.register_guest("Guest 2", 0, None, 1002).unwrap();
    let _g3 = registry.register_guest("Guest 3", 0, None, 1003).unwrap();
    let _g4 = registry.register_guest("Guest 4", 0, None, 1004).unwrap();

    assert!(registry.is_full());

    // Kick guest 1
    registry.remove(&g1.id).expect("remove G1");
    assert_eq!(registry.connected_count(), 4);

    let g5 = registry.register_guest("Guest 5", 0, None, 1005).expect("G5 joins");
    assert_eq!(registry.connected_count(), 5);
    assert!(registry.is_full());
    assert_eq!(g5.display_name, "Guest 5");
}

#[test]
fn test_status_update() {
    let host_id = ParticipantId::new("part-host-001");
    let mut registry = ParticipantRegistry::new(host_id.clone(), "Host".to_string(), 5, 1000);

    registry.update_status(&host_id, 5000, 4500, false, 15, 2000);
    let host = registry.get(&host_id).unwrap();
    assert_eq!(host.buffered_position_ms, 5000);
    assert_eq!(host.current_position_ms, 4500);
    assert_eq!(host.ping_ms, 15);
}
