use watch_together_tests::protocol::{PlaybackState, PlaybackStatus};
use watch_together_tests::sync::*;

// ============================================================================
// TIER 1: FEATURE COVERAGE (NTP MATH, PROJECTION & DRIFT ENGINE)
// ============================================================================

#[test]
fn test_tier1_ntp_symmetric_exchange() {
    let mut estimator = ClockEstimator::default_config();

    // Client send: 1000ms
    // Server receive: 2050ms (Server clock +1000ms ahead, network delay 50ms)
    // Server send: 2060ms (Server processing time 10ms)
    // Client receive: 1110ms (Network delay 50ms)
    let m = NtpMeasurement {
        t0_client_send: 1000,
        t1_server_recv: 2050,
        t2_server_send: 2060,
        t3_client_recv: 1110,
    };

    let sample = estimator.record_measurement(&m).expect("Valid measurement");

    // RTT = (t3 - t0) - (t2 - t1) = (1110 - 1000) - (2060 - 2050) = 110 - 10 = 100ms
    assert_eq!(sample.rtt_ms, 100);

    // Offset = ((t1 - t0) + (t2 - t3)) / 2 = ((2050 - 1000) + (2060 - 1110)) / 2 = (1050 + 950) / 2 = 1000ms
    assert_eq!(sample.offset_ms, 1000);

    assert_eq!(estimator.smoothed_offset_ms(), Some(1000));
    assert_eq!(estimator.smoothed_rtt_ms(), Some(100));

    // Server time prediction: Client time 5000 + 1000 offset = 6000
    assert_eq!(estimator.estimate_server_time(5000), Some(6000));
}

#[test]
fn test_tier1_ntp_ema_smoothing_convergence() {
    let mut estimator = ClockEstimator::default_config(); // alpha = 0.2

    // Initial sample: offset 100
    estimator.record_sample(NtpSample { rtt_ms: 50, offset_ms: 100 }).unwrap();
    assert_eq!(estimator.smoothed_offset_ms(), Some(100));

    // Sample 2: offset 200 -> (1 - 0.2)*100 + 0.2*200 = 80 + 40 = 120
    estimator.record_sample(NtpSample { rtt_ms: 50, offset_ms: 200 }).unwrap();
    assert_eq!(estimator.smoothed_offset_ms(), Some(120));

    // Sample 3: offset 200 -> (1 - 0.2)*120 + 0.2*200 = 96 + 40 = 136
    estimator.record_sample(NtpSample { rtt_ms: 50, offset_ms: 200 }).unwrap();
    assert_eq!(estimator.smoothed_offset_ms(), Some(136));

    // Sample 4: offset 200 -> 0.8*136 + 0.2*200 = 108.8 + 40 = 148.8 -> round to 149
    estimator.record_sample(NtpSample { rtt_ms: 50, offset_ms: 200 }).unwrap();
    assert_eq!(estimator.smoothed_offset_ms(), Some(149));
}

#[test]
fn test_tier1_target_position_projection_playing() {
    let state = PlaybackState {
        revision: 1,
        status: PlaybackStatus::Playing,
        position_ms: 10_000,
        speed: 1.0,
        updated_at_server_ms: 100_000,
        duration_ms: 600_000,
    };

    // 5 seconds elapsed on server at 1.0x speed
    let pos_1x = project_state_position(&state, 105_000);
    assert_eq!(pos_1x, 15_000);

    // 4 seconds elapsed at 1.5x speed
    let mut state_1_5x = state.clone();
    state_1_5x.speed = 1.5;
    let pos_1_5x = project_state_position(&state_1_5x, 104_000);
    assert_eq!(pos_1_5x, 16_000); // 10_000 + 4_000 * 1.5 = 16_000

    // 3 seconds elapsed at 2.0x speed
    let mut state_2x = state.clone();
    state_2x.speed = 2.0;
    let pos_2x = project_state_position(&state_2x, 103_000);
    assert_eq!(pos_2x, 16_000); // 10_000 + 3_000 * 2.0 = 16_000
}

#[test]
fn test_tier1_target_position_projection_paused_and_buffering() {
    let state_paused = PlaybackState {
        revision: 1,
        status: PlaybackStatus::Paused,
        position_ms: 45_000,
        speed: 1.0,
        updated_at_server_ms: 100_000,
        duration_ms: 600_000,
    };

    // Position must NOT advance when Paused
    assert_eq!(project_state_position(&state_paused, 120_000), 45_000);

    let state_buffering = PlaybackState {
        revision: 2,
        status: PlaybackStatus::Buffering,
        position_ms: 45_000,
        speed: 1.0,
        updated_at_server_ms: 100_000,
        duration_ms: 600_000,
    };

    // Position must NOT advance when Buffering
    assert_eq!(project_state_position(&state_buffering, 120_000), 45_000);
}

#[test]
fn test_tier1_target_position_projection_duration_clamping() {
    let state = PlaybackState {
        revision: 1,
        status: PlaybackStatus::Playing,
        position_ms: 590_000,
        speed: 1.0,
        updated_at_server_ms: 100_000,
        duration_ms: 600_000, // Duration 10 mins
    };

    // 30 seconds elapsed -> would be 620_000, but clamped to 600_000
    let pos = project_state_position(&state, 130_000);
    assert_eq!(pos, 600_000);
}

#[test]
fn test_tier1_drift_engine_three_tiers_happy_path() {
    let mut engine = DriftEngine::default_config();

    // Tier 1: 50ms drift -> None (deadband)
    let c1 = engine.evaluate(1050, 1000, 1.0);
    assert_eq!(c1, Correction::None);

    // Tier 2: 250ms drift ahead -> AdjustSpeed (0.96)
    let c2 = engine.evaluate(1250, 1000, 1.0);
    if let Correction::AdjustSpeed { speed, drift_ms } = c2 {
        assert_eq!(drift_ms, 250);
        assert!((speed - 0.96).abs() < 1e-4);
    } else {
        panic!("Expected AdjustSpeed");
    }

    // Reset engine
    engine.reset();

    // Tier 3: 750ms drift -> HardSeek
    let c3 = engine.evaluate(1750, 1000, 1.0);
    if let Correction::HardSeek { target_position_ms, drift_ms } = c3 {
        assert_eq!(target_position_ms, 1000);
        assert_eq!(drift_ms, 750);
    } else {
        panic!("Expected HardSeek");
    }
}

// ============================================================================
// TIER 2: BOUNDARY VALUE ANALYSIS & CORNER CASES
// ============================================================================

#[test]
fn test_tier2_drift_deadband_exact_boundaries() {
    let mut engine = DriftEngine::default_config();

    // 0ms drift
    assert_eq!(engine.evaluate(1000, 1000, 1.0), Correction::None);

    // +99ms drift
    assert_eq!(engine.evaluate(1099, 1000, 1.0), Correction::None);

    // +100ms drift (boundary limit)
    assert_eq!(engine.evaluate(1100, 1000, 1.0), Correction::None);

    // -99ms drift
    assert_eq!(engine.evaluate(901, 1000, 1.0), Correction::None);

    // -100ms drift (boundary limit)
    assert_eq!(engine.evaluate(900, 1000, 1.0), Correction::None);
}

#[test]
fn test_tier2_drift_speed_adjust_threshold_transitions() {
    let mut engine = DriftEngine::default_config();

    // +101ms drift (ahead -> slow down: nominal * 0.96)
    let c_ahead = engine.evaluate(1101, 1000, 1.0);
    if let Correction::AdjustSpeed { speed, drift_ms } = c_ahead {
        assert_eq!(drift_ms, 101);
        assert!((speed - 0.96).abs() < 1e-4);
    } else {
        panic!("Expected AdjustSpeed for +101ms");
    }

    engine.reset();

    // -101ms drift (lagging -> speed up: nominal * 1.04)
    let c_lag = engine.evaluate(899, 1000, 1.0);
    if let Correction::AdjustSpeed { speed, drift_ms } = c_lag {
        assert_eq!(drift_ms, -101);
        assert!((speed - 1.04).abs() < 1e-4);
    } else {
        panic!("Expected AdjustSpeed for -101ms");
    }

    engine.reset();

    // +499ms drift (upper bound of speed adjust)
    let c_499 = engine.evaluate(1499, 1000, 1.0);
    assert!(matches!(c_499, Correction::AdjustSpeed { drift_ms: 499, .. }));

    engine.reset();

    // +500ms drift (upper boundary limit)
    let c_500 = engine.evaluate(1500, 1000, 1.0);
    assert!(matches!(c_500, Correction::AdjustSpeed { drift_ms: 500, .. }));

    engine.reset();

    // -500ms drift (lower boundary limit)
    let c_neg500 = engine.evaluate(500, 1000, 1.0);
    assert!(matches!(c_neg500, Correction::AdjustSpeed { drift_ms: -500, .. }));
}

#[test]
fn test_tier2_drift_hard_seek_threshold_transitions() {
    let mut engine = DriftEngine::default_config();

    // +501ms drift -> HardSeek
    let c_seek_ahead = engine.evaluate(1501, 1000, 1.0);
    assert_eq!(
        c_seek_ahead,
        Correction::HardSeek {
            target_position_ms: 1000,
            drift_ms: 501
        }
    );

    engine.reset();

    // -501ms drift -> HardSeek
    let c_seek_behind = engine.evaluate(499, 1000, 1.0);
    assert_eq!(
        c_seek_behind,
        Correction::HardSeek {
            target_position_ms: 1000,
            drift_ms: -501
        }
    );

    engine.reset();

    // Late joiner / major seek: +60,000ms drift
    let c_huge = engine.evaluate(70_000, 10_000, 1.0);
    assert_eq!(
        c_huge,
        Correction::HardSeek {
            target_position_ms: 10_000,
            drift_ms: 60_000
        }
    );
}

#[test]
fn test_tier2_drift_hysteresis_behavior() {
    let mut engine = DriftEngine::default_config();

    // 1. Enter AdjustSpeed at +150ms
    let c1 = engine.evaluate(1150, 1000, 1.0);
    assert!(matches!(c1, Correction::AdjustSpeed { .. }));
    assert!(engine.is_adjusting());

    // 2. Drift decreases to +80ms (normally within deadband <=100ms, but hysteresis holds it)
    let c2 = engine.evaluate(1080, 1000, 1.0);
    assert!(
        matches!(c2, Correction::AdjustSpeed { drift_ms: 80, .. }),
        "Hysteresis must keep AdjustSpeed active at 80ms"
    );

    // 3. Drift decreases to +35ms (> 30ms convergence threshold)
    let c3 = engine.evaluate(1035, 1000, 1.0);
    assert!(
        matches!(c3, Correction::AdjustSpeed { drift_ms: 35, .. }),
        "Hysteresis must keep AdjustSpeed active at 35ms"
    );

    // 4. Drift reaches +30ms (<= 30ms convergence threshold) -> exits to None
    let c4 = engine.evaluate(1030, 1000, 1.0);
    assert_eq!(c4, Correction::None);
    assert!(!engine.is_adjusting(), "Engine must have exited adjusting mode");

    // 5. Once exited, regular deadband applies again: drift at +70ms is None
    let c5 = engine.evaluate(1070, 1000, 1.0);
    assert_eq!(c5, Correction::None);
}

#[test]
fn test_tier2_drift_escalation_while_adjusting() {
    let mut engine = DriftEngine::default_config();

    // Start adjusting at +200ms
    let c1 = engine.evaluate(1200, 1000, 1.0);
    assert!(matches!(c1, Correction::AdjustSpeed { .. }));
    assert!(engine.is_adjusting());

    // Drift escalates to +550ms -> must escalate immediately to HardSeek
    let c2 = engine.evaluate(1550, 1000, 1.0);
    assert_eq!(
        c2,
        Correction::HardSeek {
            target_position_ms: 1000,
            drift_ms: 550
        }
    );
    assert!(!engine.is_adjusting());
}

#[test]
fn test_tier2_drift_speed_adjust_with_non_unity_nominal_speeds() {
    let mut engine = DriftEngine::default_config();

    // Nominal speed 1.5x, lagging (-200ms) -> 1.5 * 1.04 = 1.56
    let c_1_5 = engine.evaluate(800, 1000, 1.5);
    if let Correction::AdjustSpeed { speed, .. } = c_1_5 {
        assert!((speed - 1.56).abs() < 1e-4);
    } else {
        panic!("Expected AdjustSpeed");
    }

    engine.reset();

    // Nominal speed 0.5x, ahead (+200ms) -> 0.5 * 0.96 = 0.48
    let c_0_5 = engine.evaluate(1200, 1000, 0.5);
    if let Correction::AdjustSpeed { speed, .. } = c_0_5 {
        assert!((speed - 0.48).abs() < 1e-4);
    } else {
        panic!("Expected AdjustSpeed");
    }
}

#[test]
fn test_tier2_ntp_causality_violations_rejected() {
    // 1. Client clock went backwards (t3 < t0)
    let m1 = NtpMeasurement {
        t0_client_send: 1000,
        t1_server_recv: 2000,
        t2_server_send: 2010,
        t3_client_recv: 990,
    };
    assert!(matches!(
        ClockEstimator::compute_raw_sample(&m1),
        Err(ClockSyncError::CausalityViolation { .. })
    ));

    // 2. Server clock went backwards (t2 < t1)
    let m2 = NtpMeasurement {
        t0_client_send: 1000,
        t1_server_recv: 2000,
        t2_server_send: 1990,
        t3_client_recv: 1050,
    };
    assert!(matches!(
        ClockEstimator::compute_raw_sample(&m2),
        Err(ClockSyncError::CausalityViolation { .. })
    ));

    // 3. Processing time exceeds total RTT (client_elapsed < server_elapsed)
    let m3 = NtpMeasurement {
        t0_client_send: 1000,
        t1_server_recv: 2000,
        t2_server_send: 2050, // server elapsed 50ms
        t3_client_recv: 1040, // client elapsed 40ms
    };
    assert!(matches!(
        ClockEstimator::compute_raw_sample(&m3),
        Err(ClockSyncError::NegativeRtt { .. })
    ));
}

#[test]
fn test_tier2_ntp_outlier_rejection_ceiling() {
    let mut estimator = ClockEstimator::default_config();

    // RTT exceeds absolute ceiling of 2000ms
    let sample = NtpSample {
        rtt_ms: 2500,
        offset_ms: 500,
    };
    let res = estimator.record_sample(sample);
    assert!(matches!(res, Err(ClockSyncError::OutlierRejected { .. })));
    assert_eq!(estimator.rejected_samples, 1);
    assert!(estimator.smoothed_offset_ms().is_none());
}

#[test]
fn test_tier2_ntp_dynamic_outlier_spike_rejection() {
    let mut estimator = ClockEstimator::default_config();

    // Seed 4 good samples at 30ms RTT, offset 400
    for _ in 0..4 {
        estimator.record_sample(NtpSample { rtt_ms: 30, offset_ms: 400 }).unwrap();
    }
    assert_eq!(estimator.smoothed_offset_ms(), Some(400));
    assert_eq!(estimator.smoothed_rtt_ms(), Some(30));

    // Sudden spike: RTT = 500ms (threshold is 30 * 2.5 + 150 = 225ms)
    let spike_sample = NtpSample { rtt_ms: 500, offset_ms: 900 };
    let res = estimator.record_sample(spike_sample);
    assert!(matches!(res, Err(ClockSyncError::OutlierRejected { .. })));

    // Smoothed offset must remain preserved at 400ms without contamination
    assert_eq!(estimator.smoothed_offset_ms(), Some(400));
}

// Additional Tier 1 Sync & Projection Tests

#[test]
fn test_tier1_ntp_synchronization_flag_transition() {
    let mut estimator = ClockEstimator::default_config();
    assert!(!estimator.is_synchronized());

    estimator.record_sample(NtpSample { rtt_ms: 40, offset_ms: 100 }).unwrap();
    // 1 sample is not yet synchronized
    assert!(!estimator.is_synchronized());

    estimator.record_sample(NtpSample { rtt_ms: 40, offset_ms: 100 }).unwrap();
    // 2 samples reaches synchronized status
    assert!(estimator.is_synchronized());
}

#[test]
fn test_tier1_projection_zero_elapsed_time() {
    let state = PlaybackState {
        revision: 1,
        status: PlaybackStatus::Playing,
        position_ms: 50_000,
        speed: 1.0,
        updated_at_server_ms: 100_000,
        duration_ms: 500_000,
    };
    // 0 elapsed time
    assert_eq!(project_state_position(&state, 100_000), 50_000);
}

#[test]
fn test_tier1_drift_engine_reset_clears_adjusting_state() {
    let mut engine = DriftEngine::default_config();
    engine.evaluate(1200, 1000, 1.0); // +200ms enters adjusting mode
    assert!(engine.is_adjusting());

    engine.reset();
    assert!(!engine.is_adjusting());
}

#[test]
fn test_tier1_ntp_asymmetric_network_delay() {
    // Uplink 30ms, downlink 70ms -> RTT 100ms
    let m = NtpMeasurement {
        t0_client_send: 1000,
        t1_server_recv: 2030, // server clock offset 1000 + 30ms uplink
        t2_server_send: 2040, // server elapsed 10ms
        t3_client_recv: 1120, // client recv: 1040 + 70ms downlink + 10ms server
    };
    let sample = ClockEstimator::compute_raw_sample(&m).unwrap();
    // RTT = (1120 - 1000) - (2040 - 2030) = 120 - 10 = 110ms
    assert_eq!(sample.rtt_ms, 110);
    // Offset = ((2030 - 1000) + (2040 - 1120)) / 2 = (1030 + 920) / 2 = 975ms
    assert_eq!(sample.offset_ms, 975);
}

#[test]
fn test_tier1_clock_estimator_history_capacity_window() {
    let mut estimator = ClockEstimator::new(ClockSyncConfig {
        history_capacity: 5,
        ..Default::default()
    });

    for i in 1..=10 {
        estimator.record_sample(NtpSample { rtt_ms: 20, offset_ms: i * 10 }).unwrap();
    }
    assert_eq!(estimator.total_samples, 10);
}

// Additional Tier 2 Boundary & Corner Case Tests

#[test]
fn test_tier2_projection_server_clock_in_past_no_negative_overflow() {
    let state = PlaybackState {
        revision: 1,
        status: PlaybackStatus::Playing,
        position_ms: 50_000,
        speed: 1.0,
        updated_at_server_ms: 100_000,
        duration_ms: 500_000,
    };
    // Current server time is reported earlier than updated_at (e.g. clock adjustment backwards)
    let projected = project_state_position(&state, 90_000);
    assert_eq!(projected, 50_000); // delta is 0, no underflow
}

#[test]
fn test_tier2_drift_engine_extreme_lag_multi_minute_hard_seek() {
    let mut engine = DriftEngine::default_config();
    // 10 minutes lag (600_000 ms)
    let c = engine.evaluate(0, 600_000, 1.0);
    assert_eq!(
        c,
        Correction::HardSeek {
            target_position_ms: 600_000,
            drift_ms: -600_000,
        }
    );
}

#[test]
fn test_tier2_ntp_sample_with_zero_rtt_processing() {
    let m = NtpMeasurement {
        t0_client_send: 1000,
        t1_server_recv: 1000,
        t2_server_send: 1000,
        t3_client_recv: 1000,
    };
    let sample = ClockEstimator::compute_raw_sample(&m).unwrap();
    assert_eq!(sample.rtt_ms, 0);
    assert_eq!(sample.offset_ms, 0);
}

#[test]
fn test_tier2_clock_estimator_all_samples_rejected_retains_none() {
    let mut estimator = ClockEstimator::default_config();
    // Feed 3 samples with RTT > 2000ms
    assert!(estimator.record_sample(NtpSample { rtt_ms: 2500, offset_ms: 100 }).is_err());
    assert!(estimator.record_sample(NtpSample { rtt_ms: 3000, offset_ms: 200 }).is_err());
    assert!(estimator.record_sample(NtpSample { rtt_ms: 4000, offset_ms: 300 }).is_err());

    assert_eq!(estimator.total_samples, 3);
    assert_eq!(estimator.rejected_samples, 3);
    assert!(estimator.smoothed_offset_ms().is_none());
}

#[test]
fn test_tier2_drift_speed_adjust_alternating_drift_directions() {
    let mut engine = DriftEngine::default_config();

    // 1. Lagging by 200ms -> speedup
    let c1 = engine.evaluate(800, 1000, 1.0);
    if let Correction::AdjustSpeed { speed, drift_ms } = c1 {
        assert_eq!(drift_ms, -200);
        assert!((speed - 1.04).abs() < 1e-4);
    } else {
        panic!("Expected speedup");
    }

    // 2. Swings to ahead by 200ms -> slowdown
    let c2 = engine.evaluate(1200, 1000, 1.0);
    if let Correction::AdjustSpeed { speed, drift_ms } = c2 {
        assert_eq!(drift_ms, 200);
        assert!((speed - 0.96).abs() < 1e-4);
    } else {
        panic!("Expected slowdown");
    }
}
