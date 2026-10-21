use proptest::prelude::*;
use sync_engine::drift::{Correction, DriftEngine};

proptest! {
    #[test]
    fn prop_deadband_invariant(delta in -100i64..=100i64) {
        let mut engine = DriftEngine::default_config();
        let client_pos = 100_000i64 + delta;
        let expected_pos = 100_000u64;
        let corr = engine.evaluate(client_pos as u64, expected_pos, 1.0);
        prop_assert_eq!(corr, Correction::None);
    }

    #[test]
    fn prop_hard_seek_invariant(delta in 501i64..100_000i64) {
        let mut engine = DriftEngine::default_config();
        let client_pos = 200_000i64 + delta;
        let expected_pos = 200_000u64;
        let corr = engine.evaluate(client_pos as u64, expected_pos, 1.0);
        let is_hard_seek = matches!(corr, Correction::HardSeek { .. });
        prop_assert!(is_hard_seek);
    }

    #[test]
    fn prop_hysteresis_convergence_invariant(drift in -30i64..=30i64) {
        let mut engine = DriftEngine::default_config();
        // First trigger speed adjust
        engine.evaluate(100_200, 100_000, 1.0);
        prop_assert!(engine.is_adjusting());

        // Then provide sample within convergence zone
        let client_pos = 100_000i64 + drift;
        let corr = engine.evaluate(client_pos as u64, 100_000, 1.0);
        prop_assert_eq!(corr, Correction::None);
        prop_assert!(!engine.is_adjusting());
    }
}
