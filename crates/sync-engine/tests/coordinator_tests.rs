use shared_protocol::models::{PlaybackState, PlaybackStatus};
use sync_engine::clock::NtpMeasurement;
use sync_engine::coordinator::ClientSyncCoordinator;
use sync_engine::drift::Correction;

#[test]
fn test_client_coordinator_lifecycle() {
    let mut coordinator = ClientSyncCoordinator::new(0);

    // Initial tick before receiving state or ping
    assert_eq!(coordinator.tick(1000, 0), None);

    // Receive NTP pong
    let m = NtpMeasurement {
        t0_client_send: 1000,
        t1_server_recv: 2050,
        t2_server_send: 2060,
        t3_client_recv: 1110,
    };
    coordinator.handle_pong(&m).unwrap();

    // Receive state broadcast
    let state = PlaybackState {
        revision: 1,
        status: PlaybackStatus::Playing,
        position_ms: 10_000,
        speed: 1.0,
        updated_at_server_ms: 2060,
        duration_ms: 600_000,
    };
    coordinator.handle_state_broadcast(state).unwrap();

    // At client time 1160, server time est is 1160 + 1000 = 2160.
    // Elapsed since 2060 is 100ms. Expected pos = 10_000 + 100 = 10_100ms.
    // If player is at 10_100ms, drift = 0 -> Correction::None.
    let corr = coordinator.tick(1160, 10_100);
    assert_eq!(corr, Some(Correction::None));

    // If player is lagging at 9_900ms (drift = -200ms) -> AdjustSpeed
    let corr_lag = coordinator.tick(1160, 9_900);
    assert!(matches!(corr_lag, Some(Correction::AdjustSpeed { .. })));

    // If player is severely lagging at 5_000ms (drift = -5100ms) -> HardSeek
    let corr_hard = coordinator.tick(1160, 5_000);
    assert!(matches!(corr_hard, Some(Correction::HardSeek { .. })));
}
