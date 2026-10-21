use room_server::{PermissionPolicy, RoomServerError};
use shared_protocol::models::{ParticipantId, PermissionMode};

#[test]
fn test_host_only_rejects_guest_commands() {
    let host = ParticipantId::new("host-1");
    let guest = ParticipantId::new("guest-1");
    let policy = PermissionPolicy::new(host.clone(), PermissionMode::HostOnly);

    // Host is allowed
    assert!(policy.can_control_playback(&host, true).is_ok());

    // Guest is rejected
    let res = policy.can_control_playback(&guest, true);
    assert!(res.is_err());
    match res.unwrap_err() {
        RoomServerError::PermissionDenied(_) => {}
        other => panic!("Expected PermissionDenied, got {:?}", other),
    }
}

#[test]
fn test_free_control_accepts_all_commands() {
    let host = ParticipantId::new("host-1");
    let guest = ParticipantId::new("guest-1");
    let policy = PermissionPolicy::new(host.clone(), PermissionMode::FreeControl);

    assert!(policy.can_control_playback(&host, true).is_ok());
    assert!(policy.can_control_playback(&guest, true).is_ok());

    // Unregistered participant is rejected even in free control
    let unknown = ParticipantId::new("unknown");
    assert!(policy.can_control_playback(&unknown, false).is_err());
}

#[test]
fn test_mode_toggle_only_by_host() {
    let host = ParticipantId::new("host-1");
    let guest = ParticipantId::new("guest-1");
    let mut policy = PermissionPolicy::new(host.clone(), PermissionMode::FreeControl);

    // Host can change
    assert!(policy.set_mode(&host, PermissionMode::HostOnly).is_ok());
    assert_eq!(policy.mode(), PermissionMode::HostOnly);

    // Guest cannot change
    let res = policy.set_mode(&guest, PermissionMode::FreeControl);
    assert!(res.is_err());
    assert_eq!(policy.mode(), PermissionMode::HostOnly);
}

#[test]
fn test_kick_host_self_rejected() {
    let host = ParticipantId::new("host-1");
    let policy = PermissionPolicy::new(host.clone(), PermissionMode::FreeControl);

    let res = policy.can_kick(&host, &host);
    assert!(res.is_err());
    match res.unwrap_err() {
        RoomServerError::PermissionDenied(msg) => {
            assert!(msg.contains("Host cannot kick self"));
        }
        other => panic!("Expected PermissionDenied, got {:?}", other),
    }
}

#[test]
fn test_kick_guest_by_guest_rejected() {
    let host = ParticipantId::new("host-1");
    let guest1 = ParticipantId::new("guest-1");
    let guest2 = ParticipantId::new("guest-2");
    let policy = PermissionPolicy::new(host.clone(), PermissionMode::FreeControl);

    // Guest cannot kick peer
    let res = policy.can_kick(&guest1, &guest2);
    assert!(res.is_err());

    // Host can kick guest
    assert!(policy.can_kick(&host, &guest1).is_ok());
}

#[test]
fn test_close_only_by_host() {
    let host = ParticipantId::new("host-1");
    let guest = ParticipantId::new("guest-1");
    let policy = PermissionPolicy::new(host.clone(), PermissionMode::FreeControl);

    assert!(policy.can_close(&host).is_ok());
    assert!(policy.can_close(&guest).is_err());
}
