use sync_engine::drift::{Correction, DriftEngine};

#[test]
fn test_drift_exact_zero() {
    let mut engine = DriftEngine::default_config();
    let corr = engine.evaluate(1000, 1000, 1.0);
    assert_eq!(corr, Correction::None);
    assert!(!engine.is_adjusting());
}

#[test]
fn test_drift_deadband_boundaries() {
    let mut engine = DriftEngine::default_config();

    // +100ms is deadband upper limit
    let corr = engine.evaluate(1100, 1000, 1.0);
    assert_eq!(corr, Correction::None);
    assert!(!engine.is_adjusting());

    // -100ms is deadband lower limit
    let corr = engine.evaluate(900, 1000, 1.0);
    assert_eq!(corr, Correction::None);
    assert!(!engine.is_adjusting());
}

#[test]
fn test_drift_speed_adjust_boundaries() {
    let mut engine = DriftEngine::default_config();

    // -101ms triggers speed up (lagging)
    let corr = engine.evaluate(899, 1000, 1.0);
    match corr {
        Correction::AdjustSpeed { speed, drift_ms } => {
            assert!((speed - 1.04).abs() < 1e-4);
            assert_eq!(drift_ms, -101);
        }
        _ => panic!("Expected AdjustSpeed"),
    }
    assert!(engine.is_adjusting());

    let mut engine2 = DriftEngine::default_config();
    // +101ms triggers slow down (ahead)
    let corr2 = engine2.evaluate(1101, 1000, 1.0);
    match corr2 {
        Correction::AdjustSpeed { speed, drift_ms } => {
            assert!((speed - 0.96).abs() < 1e-4);
            assert_eq!(drift_ms, 101);
        }
        _ => panic!("Expected AdjustSpeed"),
    }
    assert!(engine2.is_adjusting());

    let mut engine3 = DriftEngine::default_config();
    // +500ms is upper limit for gradual speed adjustment
    let corr3 = engine3.evaluate(1500, 1000, 1.0);
    match corr3 {
        Correction::AdjustSpeed { speed, drift_ms } => {
            assert!((speed - 0.96).abs() < 1e-4);
            assert_eq!(drift_ms, 500);
        }
        _ => panic!("Expected AdjustSpeed"),
    }
}

#[test]
fn test_drift_hard_seek_boundaries() {
    let mut engine = DriftEngine::default_config();

    // +501ms triggers hard seek
    let corr = engine.evaluate(1501, 1000, 1.0);
    assert_eq!(
        corr,
        Correction::HardSeek {
            target_position_ms: 1000,
            drift_ms: 501,
        }
    );
    assert!(!engine.is_adjusting());

    // -501ms triggers hard seek
    let corr2 = engine.evaluate(499, 1000, 1.0);
    assert_eq!(
        corr2,
        Correction::HardSeek {
            target_position_ms: 1000,
            drift_ms: -501,
        }
    );
    assert!(!engine.is_adjusting());
}

#[test]
fn test_drift_hysteresis_behavior() {
    let mut engine = DriftEngine::default_config();

    // 1. Enter speed adjust mode at 150ms drift
    let corr1 = engine.evaluate(1150, 1000, 1.0);
    assert!(matches!(corr1, Correction::AdjustSpeed { .. }));
    assert!(engine.is_adjusting());

    // 2. Drift decreases to 80ms: in regular deadband (<=100ms), but hysteresis keeps adjusting (>30ms)
    let corr2 = engine.evaluate(1080, 1000, 1.0);
    assert!(matches!(corr2, Correction::AdjustSpeed { .. }));
    assert!(engine.is_adjusting());

    // 3. Drift decreases to 30ms: reaches convergence threshold (<=30ms) -> exits adjusting mode
    let corr3 = engine.evaluate(1030, 1000, 1.0);
    assert_eq!(corr3, Correction::None);
    assert!(!engine.is_adjusting());

    // 4. Once exited, standard deadband (<=100ms) re-applies: 70ms drift -> None
    let corr4 = engine.evaluate(1070, 1000, 1.0);
    assert_eq!(corr4, Correction::None);
    assert!(!engine.is_adjusting());
}

#[test]
fn test_drift_escalation_while_adjusting() {
    let mut engine = DriftEngine::default_config();

    // Enter adjusting mode
    engine.evaluate(1200, 1000, 1.0);
    assert!(engine.is_adjusting());

    // Severe network spike pushes drift to 650ms
    let corr = engine.evaluate(1650, 1000, 1.0);
    assert_eq!(
        corr,
        Correction::HardSeek {
            target_position_ms: 1000,
            drift_ms: 650,
        }
    );
    assert!(!engine.is_adjusting());
}
