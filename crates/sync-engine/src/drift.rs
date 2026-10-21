//! Three-tier drift correction engine with hysteresis.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Correction {
    /// Drift is within deadband (<= 100ms) or within hysteresis convergence zone (<= 30ms).
    None,
    /// Gradual speed adjustment (100ms - 500ms): nominal_speed * 1.04 (lagging) or 0.96 (ahead).
    AdjustSpeed {
        speed: f32,
        drift_ms: i64,
    },
    /// Hard seek (> 500ms): instantaneous jump to target position.
    HardSeek {
        target_position_ms: u64,
        drift_ms: i64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DriftEngineConfig {
    pub deadband_ms: u64,         // 100 ms
    pub hard_seek_ms: u64,        // 500 ms
    pub convergence_ms: u64,      // 30 ms (hysteresis exit)
    pub speed_up_factor: f32,     // 1.04
    pub slow_down_factor: f32,    // 0.96
}

impl Default for DriftEngineConfig {
    fn default() -> Self {
        Self {
            deadband_ms: 100,
            hard_seek_ms: 500,
            convergence_ms: 30,
            speed_up_factor: 1.04,
            slow_down_factor: 0.96,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DriftEngine {
    config: DriftEngineConfig,
    is_adjusting: bool,
}

impl DriftEngine {
    pub fn new(config: DriftEngineConfig) -> Self {
        Self {
            config,
            is_adjusting: false,
        }
    }

    pub fn default_config() -> Self {
        Self::new(DriftEngineConfig::default())
    }

    pub fn is_adjusting(&self) -> bool {
        self.is_adjusting
    }

    pub fn reset(&mut self) {
        self.is_adjusting = false;
    }

    /// Evaluates client drift and returns the appropriate correction action.
    ///
    /// Drift formula:
    /// delta = P_client - P_expected
    /// - delta > 0: client is ahead (running too fast) -> apply nominal * 0.96
    /// - delta < 0: client is behind (lagging) -> apply nominal * 1.04
    pub fn evaluate(
        &mut self,
        client_position_ms: u64,
        expected_position_ms: u64,
        nominal_speed: f32,
    ) -> Correction {
        let drift_ms = (client_position_ms as i64) - (expected_position_ms as i64);
        let abs_drift = drift_ms.unsigned_abs();

        // Tier 3: Hard Seek (> 500ms)
        if abs_drift > self.config.hard_seek_ms {
            self.is_adjusting = false;
            return Correction::HardSeek {
                target_position_ms: expected_position_ms,
                drift_ms,
            };
        }

        // Tier 2 with Hysteresis
        if self.is_adjusting {
            // Once in AdjustSpeed mode, stay adjusting until abs_drift <= convergence_ms (30ms)
            if abs_drift <= self.config.convergence_ms {
                self.is_adjusting = false;
                Correction::None
            } else {
                let speed = if drift_ms < 0 {
                    nominal_speed * self.config.speed_up_factor
                } else {
                    nominal_speed * self.config.slow_down_factor
                };
                Correction::AdjustSpeed { speed, drift_ms }
            }
        } else {
            // Tier 1: Deadband (0 - 100ms)
            if abs_drift <= self.config.deadband_ms {
                Correction::None
            } else {
                // 100ms < abs_drift <= 500ms: Enter gradual speed correction
                self.is_adjusting = true;
                let speed = if drift_ms < 0 {
                    nominal_speed * self.config.speed_up_factor
                } else {
                    nominal_speed * self.config.slow_down_factor
                };
                Correction::AdjustSpeed { speed, drift_ms }
            }
        }
    }
}
