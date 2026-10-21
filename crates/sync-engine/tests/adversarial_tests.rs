//! Adversarial stress tests and boundary verification for sync-engine.
//! Authored by challenger_m1_2.

use shared_protocol::models::{PlaybackState, PlaybackStatus};
use sync_engine::clock::{ClockEstimator, ClockSyncError, NtpMeasurement, NtpSample};
use sync_engine::coordinator::ClientSyncCoordinator;
use sync_engine::drift::{Correction, DriftEngine};
use sync_engine::projection::project_target_position;
use sync_engine::revision::{RevisionError, RevisionTracker};

// ============================================================================
// 1. NTP Clock Sync Robustness
// ============================================================================

#[test]
fn test_adversarial_extreme_rtt_spikes() {
    let mut estimator = ClockEstimator::default_config();

    // Establish normal baseline with 5 good samples at 40ms RTT, offset +200ms
    for _ in 0..5 {
        estimator.record_sample(NtpSample { rtt_ms: 40, offset_ms: 200 }).unwrap();
    }
    assert_eq!(estimator.smoothed_rtt_ms(), Some(40));
    assert_eq!(estimator.smoothed_offset_ms(), Some(200));
    let initial_rejected = estimator.rejected_samples();

    // Attack 1: Massive 5000ms RTT spike
    let res = estimator.record_sample(NtpSample { rtt_ms: 5000, offset_ms: 1000 });
    assert!(matches!(res, Err(ClockSyncError::OutlierRejected { rtt_ms: 5000, threshold_ms: 2000 })));

    // Attack 2: Just over absolute ceiling (2001ms)
    let res = estimator.record_sample(NtpSample { rtt_ms: 2001, offset_ms: -500 });
    assert!(matches!(res, Err(ClockSyncError::OutlierRejected { rtt_ms: 2001, threshold_ms: 2000 })));

    // Attack 3: Under absolute ceiling but above dynamic threshold
    // Dynamic threshold: 40 * 2.5 + 150 = 250ms
    let res = estimator.record_sample(NtpSample { rtt_ms: 251, offset_ms: 800 });
    assert!(matches!(res, Err(ClockSyncError::OutlierRejected { rtt_ms: 251, threshold_ms: 250 })));

    // Verification: EMA offset and RTT MUST remain completely unaffected
    assert_eq!(estimator.smoothed_rtt_ms(), Some(40));
    assert_eq!(estimator.smoothed_offset_ms(), Some(200));
    assert_eq!(estimator.rejected_samples(), initial_rejected + 3);
}

#[test]
fn test_adversarial_causality_and_negative_rtt() {
    let mut estimator = ClockEstimator::default_config();

    // Violation 1: Client clock jumped backwards during flight (t3 < t0)
    let m_client_backward = NtpMeasurement {
        t0_client_send: 5000,
        t1_server_recv: 6000,
        t2_server_send: 6010,
        t3_client_recv: 4999, // t3 < t0
    };
    assert!(matches!(
        estimator.record_measurement(&m_client_backward),
        Err(ClockSyncError::CausalityViolation { t0: 5000, t3: 4999, .. })
    ));

    // Violation 2: Server clock jumped backwards during flight (t2 < t1)
    let m_server_backward = NtpMeasurement {
        t0_client_send: 1000,
        t1_server_recv: 2050,
        t2_server_send: 2040, // t2 < t1
        t3_client_recv: 1100,
    };
    assert!(matches!(
        estimator.record_measurement(&m_server_backward),
        Err(ClockSyncError::CausalityViolation { t1: 2050, t2: 2040, .. })
    ));

    // Violation 3: Negative RTT: client elapsed (5ms) < server elapsed (20ms)
    let m_neg_rtt = NtpMeasurement {
        t0_client_send: 1000,
        t1_server_recv: 2000,
        t2_server_send: 2020, // server elapsed = 20ms
        t3_client_recv: 1005, // client elapsed = 5ms -> RTT = -15ms
    };
    assert!(matches!(
        estimator.record_measurement(&m_neg_rtt),
        Err(ClockSyncError::NegativeRtt { client_elapsed: 5, server_elapsed: 20 })
    ));

    // Boundary Test: Zero RTT (instantaneous network, client elapsed == server elapsed)
    let m_zero_rtt = NtpMeasurement {
        t0_client_send: 1000,
        t1_server_recv: 2000,
        t2_server_send: 2010,
        t3_client_recv: 1010, // client_elapsed = 10ms, server_elapsed = 10ms -> rtt = 0ms
    };
    let sample = estimator.record_measurement(&m_zero_rtt).unwrap();
    assert_eq!(sample.rtt_ms, 0);
    assert_eq!(sample.offset_ms, 1000);
}

#[test]
fn test_adversarial_backward_clock_step_and_server_estimation() {
    let mut estimator = ClockEstimator::default_config();

    // Seed initial clock state
    estimator.record_sample(NtpSample { rtt_ms: 30, offset_ms: 500 }).unwrap();
    assert_eq!(estimator.estimate_server_time(10_000), Some(10_500));

    // Large negative offset (server is behind client by 8000ms)
    // Client clock step: client jumps from 10_000 to 18_000
    // With alpha = 0.2, each sample eliminates 20% of the remaining gap.
    // After 10 samples, gap is (0.8^10) = ~10.7% -> offset ~ -7087ms
    for _ in 0..10 {
        estimator.record_sample(NtpSample { rtt_ms: 30, offset_ms: -8000 }).unwrap();
    }
    let intermediate_offset = estimator.smoothed_offset_ms().unwrap();
    assert!((intermediate_offset - (-7087)).abs() < 5);

    // After 25 more samples (total 35), gap is (0.8^35) = 0.04% -> within 5ms of -8000
    for _ in 0..25 {
        estimator.record_sample(NtpSample { rtt_ms: 30, offset_ms: -8000 }).unwrap();
    }
    let final_offset = estimator.smoothed_offset_ms().unwrap();
    assert!((final_offset - (-8000)).abs() < 5);

    // If client time is smaller than negative offset (underflow protection)
    // client_time = 2000, offset = -8000 -> 2000 - 8000 = -6000 -> must saturate at 0
    let est = estimator.estimate_server_time(2000).unwrap();
    assert_eq!(est, 0);
}

#[test]
fn test_adversarial_rapid_alternating_latency_and_jitter() {
    let mut estimator = ClockEstimator::default_config();

    // Rapid latency jitter: 30ms, 120ms, 30ms, 120ms...
    for i in 0..20 {
        let rtt = if i % 2 == 0 { 30 } else { 120 };
        estimator.record_sample(NtpSample { rtt_ms: rtt, offset_ms: 100 }).unwrap();
    }

    // Smoothed RTT must be bounded within [30, 120] and converge around the average
    let smoothed_rtt = estimator.smoothed_rtt_ms().unwrap();
    assert!((30..=120).contains(&smoothed_rtt));
    assert_eq!(estimator.smoothed_offset_ms(), Some(100));

    // High frequency outlier burst: interleaved 30ms and 3000ms
    let baseline_samples = estimator.total_samples();
    for i in 0..10 {
        if i % 2 == 0 {
            assert!(estimator.record_sample(NtpSample { rtt_ms: 30, offset_ms: 100 }).is_ok());
        } else {
            assert!(estimator.record_sample(NtpSample { rtt_ms: 3000, offset_ms: 100 }).is_err());
        }
    }
    assert_eq!(estimator.rejected_samples(), 5);
    assert_eq!(estimator.total_samples(), baseline_samples + 10);
}

// ============================================================================
// 2. Monotonic Revision Gating
// ============================================================================

#[test]
fn test_adversarial_revision_replay_and_stale_rejection() {
    let mut tracker = RevisionTracker::new(50);

    // Stale replay attack: older revisions
    for stale_rev in [0, 1, 10, 48, 49] {
        assert_eq!(
            tracker.gate_command(stale_rev),
            Err(RevisionError::StaleCommand { incoming: stale_rev, current: 50 })
        );
        assert!(tracker.accept_command_and_advance(stale_rev).is_err());
    }

    // Exact match replay (rev == current)
    assert_eq!(
        tracker.gate_command(50),
        Err(RevisionError::StaleCommand { incoming: 50, current: 50 })
    );
    assert_eq!(tracker.current(), 50);

    // Client broadcast stale replay
    let mut client_tracker = RevisionTracker::new(50);
    for stale_rev in [0, 25, 49, 50] {
        assert_eq!(
            client_tracker.apply_incoming_broadcast(stale_rev),
            Err(RevisionError::StaleStateBroadcast { incoming: stale_rev, current: 50 })
        );
    }
    assert_eq!(client_tracker.current(), 50);
}

#[test]
fn test_adversarial_revision_identical_spam() {
    let mut tracker = RevisionTracker::new(100);

    // 500 identical command spams at current revision
    for _ in 0..500 {
        assert!(tracker.gate_command(100).is_err());
        assert!(tracker.accept_command_and_advance(100).is_err());
    }
    assert_eq!(tracker.current(), 100);

    // 500 identical broadcast spams at current revision
    let mut client_tracker = RevisionTracker::new(100);
    for _ in 0..500 {
        assert!(client_tracker.apply_incoming_broadcast(100).is_err());
    }
    assert_eq!(client_tracker.current(), 100);
}

#[test]
fn test_adversarial_revision_out_of_order_bursts() {
    let mut client_tracker = RevisionTracker::new(10);

    // Bursts arriving in scrambled order
    let arrivals = [15, 12, 18, 14, 16, 25, 20, 22, 30, 28, 29, 31];
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();

    for &rev in &arrivals {
        match client_tracker.apply_incoming_broadcast(rev) {
            Ok(()) => accepted.push(rev),
            Err(_) => rejected.push(rev),
        }
    }

    // Only strictly increasing arrivals must be accepted:
    // 15 accepted (curr=15)
    // 12 rejected (12 <= 15)
    // 18 accepted (curr=18)
    // 14 rejected (14 <= 18)
    // 16 rejected (16 <= 18)
    // 25 accepted (curr=25)
    // 20 rejected (20 <= 25)
    // 22 rejected (22 <= 25)
    // 30 accepted (curr=30)
    // 28 rejected (28 <= 30)
    // 29 rejected (29 <= 30)
    // 31 accepted (curr=31)
    assert_eq!(accepted, vec![15, 18, 25, 30, 31]);
    assert_eq!(rejected, vec![12, 14, 16, 20, 22, 28, 29]);
    assert_eq!(client_tracker.current(), 31);
}

#[test]
fn test_adversarial_revision_u64_max_overflow() {
    let mut tracker = RevisionTracker::new(u64::MAX - 1);
    assert_eq!(tracker.advance().unwrap(), u64::MAX);
    assert_eq!(tracker.current(), u64::MAX);

    // Overflow attempt on advance
    assert_eq!(tracker.advance(), Err(RevisionError::RevisionOverflow));

    // Overflow attempt on accept_command_and_advance
    // (cannot even gate because no u64 can be > u64::MAX)
    assert_eq!(
        tracker.gate_command(u64::MAX),
        Err(RevisionError::StaleCommand { incoming: u64::MAX, current: u64::MAX })
    );
}

// ============================================================================
// 3. Drift Boundary Testing & Hysteresis
// ============================================================================

#[test]
fn test_adversarial_drift_exact_boundaries() {
    // Exact testing of 99ms, 100ms, 101ms, 499ms, 500ms, 501ms
    // for both positive (ahead) and negative (lagging) drift.
    let base = 100_000u64;

    // --- POSITIVE DRIFT (Client ahead -> slows down by 0.96) ---

    // 0ms: Deadband
    let mut e = DriftEngine::default_config();
    assert_eq!(e.evaluate(base, base, 1.0), Correction::None);
    assert!(!e.is_adjusting());

    // +99ms: Deadband
    let mut e = DriftEngine::default_config();
    assert_eq!(e.evaluate(base + 99, base, 1.0), Correction::None);
    assert!(!e.is_adjusting());

    // +100ms: Deadband boundary
    let mut e = DriftEngine::default_config();
    assert_eq!(e.evaluate(base + 100, base, 1.0), Correction::None);
    assert!(!e.is_adjusting());

    // +101ms: Enter speed adjustment (slow down to 0.96)
    let mut e = DriftEngine::default_config();
    match e.evaluate(base + 101, base, 1.0) {
        Correction::AdjustSpeed { speed, drift_ms } => {
            assert!((speed - 0.96).abs() < 1e-4);
            assert_eq!(drift_ms, 101);
        }
        other => panic!("Expected AdjustSpeed, got {:?}", other),
    }
    assert!(e.is_adjusting());

    // +499ms: Speed adjustment upper boundary - 1
    let mut e = DriftEngine::default_config();
    match e.evaluate(base + 499, base, 1.0) {
        Correction::AdjustSpeed { speed, drift_ms } => {
            assert!((speed - 0.96).abs() < 1e-4);
            assert_eq!(drift_ms, 499);
        }
        other => panic!("Expected AdjustSpeed, got {:?}", other),
    }
    assert!(e.is_adjusting());

    // +500ms: Speed adjustment exact upper boundary
    let mut e = DriftEngine::default_config();
    match e.evaluate(base + 500, base, 1.0) {
        Correction::AdjustSpeed { speed, drift_ms } => {
            assert!((speed - 0.96).abs() < 1e-4);
            assert_eq!(drift_ms, 500);
        }
        other => panic!("Expected AdjustSpeed, got {:?}", other),
    }
    assert!(e.is_adjusting());

    // +501ms: Hard Seek trigger (> 500ms)
    let mut e = DriftEngine::default_config();
    assert_eq!(
        e.evaluate(base + 501, base, 1.0),
        Correction::HardSeek { target_position_ms: base, drift_ms: 501 }
    );
    assert!(!e.is_adjusting());

    // --- NEGATIVE DRIFT (Client lagging -> speeds up by 1.04) ---

    // -99ms: Deadband
    let mut e = DriftEngine::default_config();
    assert_eq!(e.evaluate(base - 99, base, 1.0), Correction::None);
    assert!(!e.is_adjusting());

    // -100ms: Deadband boundary
    let mut e = DriftEngine::default_config();
    assert_eq!(e.evaluate(base - 100, base, 1.0), Correction::None);
    assert!(!e.is_adjusting());

    // -101ms: Enter speed adjustment (speed up to 1.04)
    let mut e = DriftEngine::default_config();
    match e.evaluate(base - 101, base, 1.0) {
        Correction::AdjustSpeed { speed, drift_ms } => {
            assert!((speed - 1.04).abs() < 1e-4);
            assert_eq!(drift_ms, -101);
        }
        other => panic!("Expected AdjustSpeed, got {:?}", other),
    }
    assert!(e.is_adjusting());

    // -499ms: Speed adjustment
    let mut e = DriftEngine::default_config();
    match e.evaluate(base - 499, base, 1.0) {
        Correction::AdjustSpeed { speed, drift_ms } => {
            assert!((speed - 1.04).abs() < 1e-4);
            assert_eq!(drift_ms, -499);
        }
        other => panic!("Expected AdjustSpeed, got {:?}", other),
    }
    assert!(e.is_adjusting());

    // -500ms: Speed adjustment exact boundary
    let mut e = DriftEngine::default_config();
    match e.evaluate(base - 500, base, 1.0) {
        Correction::AdjustSpeed { speed, drift_ms } => {
            assert!((speed - 1.04).abs() < 1e-4);
            assert_eq!(drift_ms, -500);
        }
        other => panic!("Expected AdjustSpeed, got {:?}", other),
    }
    assert!(e.is_adjusting());

    // -501ms: Hard Seek trigger (< -500ms)
    let mut e = DriftEngine::default_config();
    assert_eq!(
        e.evaluate(base - 501, base, 1.0),
        Correction::HardSeek { target_position_ms: base, drift_ms: -501 }
    );
    assert!(!e.is_adjusting());
}

#[test]
fn test_adversarial_drift_hysteresis_anti_flapping() {
    let mut engine = DriftEngine::default_config();
    let base = 50_000u64;

    // Trigger speed adjustment at +110ms
    let corr = engine.evaluate(base + 110, base, 1.0);
    assert!(matches!(corr, Correction::AdjustSpeed { .. }));
    assert!(engine.is_adjusting());

    // Anti-flapping test: Drift oscillates between 95ms and 105ms
    // Without hysteresis, 95ms would return Correction::None and flap!
    // With hysteresis, it MUST stay in AdjustSpeed!
    for _ in 0..10 {
        let c1 = engine.evaluate(base + 95, base, 1.0);
        assert!(matches!(c1, Correction::AdjustSpeed { .. }), "Failed hysteresis at +95ms");
        assert!(engine.is_adjusting());

        let c2 = engine.evaluate(base + 105, base, 1.0);
        assert!(matches!(c2, Correction::AdjustSpeed { .. }), "Failed hysteresis at +105ms");
        assert!(engine.is_adjusting());
    }

    // Step-by-step convergence towards <= 30ms:
    // +70ms -> still adjusting
    assert!(matches!(engine.evaluate(base + 70, base, 1.0), Correction::AdjustSpeed { .. }));
    // +50ms -> still adjusting
    assert!(matches!(engine.evaluate(base + 50, base, 1.0), Correction::AdjustSpeed { .. }));
    // +31ms -> still adjusting
    assert!(matches!(engine.evaluate(base + 31, base, 1.0), Correction::AdjustSpeed { .. }));
    assert!(engine.is_adjusting());

    // +30ms -> CONVERGED! Exits adjusting mode
    let c_exit = engine.evaluate(base + 30, base, 1.0);
    assert_eq!(c_exit, Correction::None);
    assert!(!engine.is_adjusting());

    // Post-convergence: drift rises back to +80ms (within deadband <= 100ms)
    // Must remain Correction::None!
    let c_deadband = engine.evaluate(base + 80, base, 1.0);
    assert_eq!(c_deadband, Correction::None);
    assert!(!engine.is_adjusting());
}

#[test]
fn test_adversarial_drift_hysteresis_negative_convergence() {
    let mut engine = DriftEngine::default_config();
    let base = 50_000u64;

    // Trigger speed adjustment lagging at -120ms
    assert!(matches!(engine.evaluate(base - 120, base, 1.0), Correction::AdjustSpeed { .. }));
    assert!(engine.is_adjusting());

    // Lagging anti-flapping between -95ms and -105ms
    for _ in 0..5 {
        assert!(matches!(engine.evaluate(base - 95, base, 1.0), Correction::AdjustSpeed { .. }));
        assert!(matches!(engine.evaluate(base - 105, base, 1.0), Correction::AdjustSpeed { .. }));
    }

    // Converging: -31ms still adjusting
    assert!(matches!(engine.evaluate(base - 31, base, 1.0), Correction::AdjustSpeed { .. }));

    // -30ms: Exit hysteresis
    assert_eq!(engine.evaluate(base - 30, base, 1.0), Correction::None);
    assert!(!engine.is_adjusting());

    // Post-convergence: -80ms is within deadband
    assert_eq!(engine.evaluate(base - 80, base, 1.0), Correction::None);
}

// ============================================================================
// 4. Position Projection Under Extreme Parameters
// ============================================================================

#[test]
fn test_adversarial_projection_speed_extremes() {
    let base_pos = 10_000u64;
    let t_update = 100_000u64;
    let t_now = 102_000u64; // 2000ms elapsed
    let duration = 60_000u64;

    // speed = 0.5x: 2000ms * 0.5 = 1000ms -> pos = 11_000
    let pos_05 = project_target_position(PlaybackStatus::Playing, base_pos, 0.5, t_update, t_now, duration);
    assert_eq!(pos_05, 11_000);

    // speed = 0.75x: 2000ms * 0.75 = 1500ms -> pos = 11_500
    let pos_075 = project_target_position(PlaybackStatus::Playing, base_pos, 0.75, t_update, t_now, duration);
    assert_eq!(pos_075, 11_500);

    // speed = 1.0x: 2000ms * 1.0 = 2000ms -> pos = 12_000
    let pos_10 = project_target_position(PlaybackStatus::Playing, base_pos, 1.0, t_update, t_now, duration);
    assert_eq!(pos_10, 12_000);

    // speed = 1.25x: 2000ms * 1.25 = 2500ms -> pos = 12_500
    let pos_125 = project_target_position(PlaybackStatus::Playing, base_pos, 1.25, t_update, t_now, duration);
    assert_eq!(pos_125, 12_500);

    // speed = 1.5x: 2000ms * 1.5 = 3000ms -> pos = 13_000
    let pos_15 = project_target_position(PlaybackStatus::Playing, base_pos, 1.5, t_update, t_now, duration);
    assert_eq!(pos_15, 13_000);

    // speed = 2.0x: 2000ms * 2.0 = 4000ms -> pos = 14_000
    let pos_20 = project_target_position(PlaybackStatus::Playing, base_pos, 2.0, t_update, t_now, duration);
    assert_eq!(pos_20, 14_000);

    // Malicious / degenerate speed values:
    // speed = 0.0 -> fallback to 1.0
    let pos_zero = project_target_position(PlaybackStatus::Playing, base_pos, 0.0, t_update, t_now, duration);
    assert_eq!(pos_zero, 12_000);

    // speed = -1.5 -> fallback to 1.0
    let pos_neg = project_target_position(PlaybackStatus::Playing, base_pos, -1.5, t_update, t_now, duration);
    assert_eq!(pos_neg, 12_000);

    // speed = NaN -> fallback to 1.0
    let pos_nan = project_target_position(PlaybackStatus::Playing, base_pos, f32::NAN, t_update, t_now, duration);
    assert_eq!(pos_nan, 12_000);
}

#[test]
fn test_adversarial_projection_duration_clamping_and_overflow() {
    let duration = 60_000u64;

    // Normal overshoot: 55_000 + 10_000 = 65_000 -> clamped to 60_000
    let pos_overshoot = project_target_position(PlaybackStatus::Playing, 55_000, 1.0, 100_000, 110_000, duration);
    assert_eq!(pos_overshoot, 60_000);

    // Position already past duration at broadcast time -> clamped to duration
    let pos_already_past = project_target_position(PlaybackStatus::Playing, 70_000, 1.0, 100_000, 101_000, duration);
    assert_eq!(pos_already_past, 60_000);

    // Paused state already past duration -> clamped to duration
    let pos_paused_past = project_target_position(PlaybackStatus::Paused, 80_000, 1.0, 100_000, 110_000, duration);
    assert_eq!(pos_paused_past, 60_000);

    // Live stream / unknown duration (duration == 0) -> no clamping
    let pos_live = project_target_position(PlaybackStatus::Playing, 55_000, 1.0, 100_000, 110_000, 0);
    assert_eq!(pos_live, 65_000);

    // Saturated arithmetic with u64::MAX
    let pos_sat = project_target_position(
        PlaybackStatus::Playing,
        u64::MAX - 100,
        2.0,
        100_000,
        200_000, // 100_000ms elapsed * 2.0 = 200_000
        0,
    );
    // u64::MAX - 100 + 200_000 saturates at u64::MAX without panicking
    assert_eq!(pos_sat, u64::MAX);
}

#[test]
fn test_adversarial_projection_paused_buffering_vs_playing() {
    let t_update = 100_000u64;
    let t_now = 200_000u64; // 100 seconds elapsed!

    // In Paused: position must remain strictly 15_000 regardless of 100s elapsed
    let pos_paused = project_target_position(PlaybackStatus::Paused, 15_000, 1.5, t_update, t_now, 600_000);
    assert_eq!(pos_paused, 15_000);

    // In Buffering: position must remain strictly 15_000 regardless of 100s elapsed
    let pos_buffering = project_target_position(PlaybackStatus::Buffering, 15_000, 1.5, t_update, t_now, 600_000);
    assert_eq!(pos_buffering, 15_000);

    // In Playing: advances by 100_000 * 1.5 = 150_000 -> 165_000
    let pos_playing = project_target_position(PlaybackStatus::Playing, 15_000, 1.5, t_update, t_now, 600_000);
    assert_eq!(pos_playing, 165_000);
}

#[test]
fn test_adversarial_coordinator_paused_drift_handling() {
    let mut coordinator = ClientSyncCoordinator::new(0);

    // 1. Establish clock synchronization (offset = 0)
    let m = NtpMeasurement {
        t0_client_send: 1000,
        t1_server_recv: 1020,
        t2_server_send: 1020,
        t3_client_recv: 1040,
    };
    coordinator.handle_pong(&m).unwrap();

    // 2. Room is PAUSED at 30_000ms
    let paused_state = PlaybackState {
        revision: 1,
        status: PlaybackStatus::Paused,
        position_ms: 30_000,
        speed: 1.0,
        updated_at_server_ms: 1020,
        duration_ms: 600_000,
    };
    coordinator.handle_state_broadcast(paused_state).unwrap();

    // Player position is at 30_200ms (drift = +200ms)
    // When paused, drift between 100ms and 500ms MUST NOT adjust speed! (Player is paused!)
    let corr_paused_drift = coordinator.tick(1040, 30_200);
    assert_eq!(corr_paused_drift, Some(Correction::None));

    // Player position is at 31_000ms (drift = +1000ms > 500ms)
    // Severe drift during pause MUST trigger HardSeek back to 30_000ms
    let corr_paused_hard_seek = coordinator.tick(1040, 31_000);
    assert_eq!(
        corr_paused_hard_seek,
        Some(Correction::HardSeek {
            target_position_ms: 30_000,
            drift_ms: 1000,
        })
    );
}
