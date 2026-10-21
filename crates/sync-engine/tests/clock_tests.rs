use sync_engine::clock::{ClockEstimator, ClockSyncError, NtpMeasurement, NtpSample};

#[test]
fn test_ntp_symmetric_exchange() {
    let mut estimator = ClockEstimator::default_config();
    // Client sends at 1000, server receives at 2050 (offset +1000, delay 50)
    // Server replies at 2060 (proc 10), client receives at 1110 (delay 50)
    let m = NtpMeasurement {
        t0_client_send: 1000,
        t1_server_recv: 2050,
        t2_server_send: 2060,
        t3_client_recv: 1110,
    };
    let sample = estimator.record_measurement(&m).unwrap();
    assert_eq!(sample.rtt_ms, 100);
    assert_eq!(sample.offset_ms, 1000);
    assert_eq!(estimator.smoothed_offset_ms(), Some(1000));
    assert_eq!(estimator.smoothed_rtt_ms(), Some(100));
}

#[test]
fn test_ntp_ema_smoothing() {
    let mut estimator = ClockEstimator::default_config();
    // Sample 1: offset 100
    estimator.record_sample(NtpSample { rtt_ms: 50, offset_ms: 100 }).unwrap();
    assert_eq!(estimator.smoothed_offset_ms(), Some(100));

    // Sample 2: offset 200 -> 0.8 * 100 + 0.2 * 200 = 120
    estimator.record_sample(NtpSample { rtt_ms: 50, offset_ms: 200 }).unwrap();
    assert_eq!(estimator.smoothed_offset_ms(), Some(120));

    // Sample 3: offset 200 -> 0.8 * 120 + 0.2 * 200 = 136
    estimator.record_sample(NtpSample { rtt_ms: 50, offset_ms: 200 }).unwrap();
    assert_eq!(estimator.smoothed_offset_ms(), Some(136));
}

#[test]
fn test_ntp_causality_rejection() {
    let mut estimator = ClockEstimator::default_config();
    // t3 < t0 (client clock went backward)
    let m = NtpMeasurement {
        t0_client_send: 1000,
        t1_server_recv: 2050,
        t2_server_send: 2060,
        t3_client_recv: 990,
    };
    assert!(matches!(estimator.record_measurement(&m), Err(ClockSyncError::CausalityViolation { .. })));
}

#[test]
fn test_negative_rtt_rejection() {
    let mut estimator = ClockEstimator::default_config();
    // client_elapsed (10) < server_elapsed (20)
    let m = NtpMeasurement {
        t0_client_send: 1000,
        t1_server_recv: 2050,
        t2_server_send: 2070,
        t3_client_recv: 1010,
    };
    assert!(matches!(estimator.record_measurement(&m), Err(ClockSyncError::NegativeRtt { .. })));
}

#[test]
fn test_ntp_outlier_spike_rejection() {
    let mut estimator = ClockEstimator::default_config();
    // Seed 4 good samples at 40ms RTT, offset 500
    for _ in 0..4 {
        estimator.record_sample(NtpSample { rtt_ms: 40, offset_ms: 500 }).unwrap();
    }
    assert_eq!(estimator.smoothed_offset_ms(), Some(500));

    // Sudden network stall: RTT spikes to 2500ms with skewed offset 800
    let err = estimator.record_sample(NtpSample { rtt_ms: 2500, offset_ms: 800 });
    assert!(matches!(err, Err(ClockSyncError::OutlierRejected { .. })));
    // Smoothed offset must remain unaffected
    assert_eq!(estimator.smoothed_offset_ms(), Some(500));
}

#[test]
fn test_estimate_server_time() {
    let mut estimator = ClockEstimator::default_config();
    estimator.record_sample(NtpSample { rtt_ms: 20, offset_ms: 1500 }).unwrap();

    let client_time = 10_000;
    let est_server = estimator.estimate_server_time(client_time).unwrap();
    assert_eq!(est_server, 11_500);
}
