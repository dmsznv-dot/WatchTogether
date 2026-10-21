use std::collections::VecDeque;
use thiserror::Error;
use crate::protocol::{PlaybackState, PlaybackStatus};

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ClockSyncError {
    #[error("Causality violation: t3 ({t3}) < t0 ({t0}) or t2 ({t2}) < t1 ({t1})")]
    CausalityViolation { t0: u64, t1: u64, t2: u64, t3: u64 },
    #[error("Processing time exceeds RTT: (t3 - t0) = {client_elapsed} < (t2 - t1) = {server_elapsed}")]
    NegativeRtt { client_elapsed: u64, server_elapsed: u64 },
    #[error("Outlier RTT rejected: rtt {rtt_ms}ms exceeds threshold {threshold_ms}ms")]
    OutlierRejected { rtt_ms: u64, threshold_ms: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NtpMeasurement {
    pub t0_client_send: u64,
    pub t1_server_recv: u64,
    pub t2_server_send: u64,
    pub t3_client_recv: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NtpSample {
    pub rtt_ms: u64,
    pub offset_ms: i64,
}

#[derive(Debug, Clone)]
pub struct ClockSyncConfig {
    pub alpha: f64,
    pub max_rtt_threshold_ms: u64,
    pub outlier_multiplier: f64,
    pub min_samples_for_outlier: usize,
    pub outlier_abs_margin_ms: u64,
    pub history_capacity: usize,
}

impl Default for ClockSyncConfig {
    fn default() -> Self {
        Self {
            alpha: 0.2,
            max_rtt_threshold_ms: 2000,
            outlier_multiplier: 2.5,
            min_samples_for_outlier: 3,
            outlier_abs_margin_ms: 150,
            history_capacity: 16,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClockEstimator {
    config: ClockSyncConfig,
    smoothed_offset_ms: Option<f64>,
    smoothed_rtt_ms: Option<f64>,
    history: VecDeque<NtpSample>,
    pub total_samples: u64,
    pub rejected_samples: u64,
}

impl ClockEstimator {
    pub fn new(config: ClockSyncConfig) -> Self {
        let cap = config.history_capacity;
        Self {
            config,
            smoothed_offset_ms: None,
            smoothed_rtt_ms: None,
            history: VecDeque::with_capacity(cap),
            total_samples: 0,
            rejected_samples: 0,
        }
    }

    pub fn default_config() -> Self {
        Self::new(ClockSyncConfig::default())
    }

    pub fn compute_raw_sample(m: &NtpMeasurement) -> Result<NtpSample, ClockSyncError> {
        if m.t3_client_recv < m.t0_client_send || m.t2_server_send < m.t1_server_recv {
            return Err(ClockSyncError::CausalityViolation {
                t0: m.t0_client_send,
                t1: m.t1_server_recv,
                t2: m.t2_server_send,
                t3: m.t3_client_recv,
            });
        }

        let client_elapsed = m.t3_client_recv - m.t0_client_send;
        let server_elapsed = m.t2_server_send - m.t1_server_recv;

        if client_elapsed < server_elapsed {
            return Err(ClockSyncError::NegativeRtt {
                client_elapsed,
                server_elapsed,
            });
        }

        let rtt_ms = client_elapsed - server_elapsed;
        let diff1 = (m.t1_server_recv as i64) - (m.t0_client_send as i64);
        let diff2 = (m.t2_server_send as i64) - (m.t3_client_recv as i64);
        let offset_ms = (diff1 + diff2) / 2;

        Ok(NtpSample { rtt_ms, offset_ms })
    }

    pub fn record_measurement(&mut self, m: &NtpMeasurement) -> Result<NtpSample, ClockSyncError> {
        let sample = Self::compute_raw_sample(m)?;
        self.record_sample(sample)
    }

    pub fn record_sample(&mut self, sample: NtpSample) -> Result<NtpSample, ClockSyncError> {
        self.total_samples += 1;

        if sample.rtt_ms > self.config.max_rtt_threshold_ms {
            self.rejected_samples += 1;
            return Err(ClockSyncError::OutlierRejected {
                rtt_ms: sample.rtt_ms,
                threshold_ms: self.config.max_rtt_threshold_ms,
            });
        }

        if self.history.len() >= self.config.min_samples_for_outlier {
            if let Some(smoothed_rtt) = self.smoothed_rtt_ms {
                let dynamic_threshold = (smoothed_rtt * self.config.outlier_multiplier)
                    + (self.config.outlier_abs_margin_ms as f64);
                if (sample.rtt_ms as f64) > dynamic_threshold {
                    self.rejected_samples += 1;
                    return Err(ClockSyncError::OutlierRejected {
                        rtt_ms: sample.rtt_ms,
                        threshold_ms: dynamic_threshold as u64,
                    });
                }
            }
        }

        let alpha = self.config.alpha;
        match (self.smoothed_offset_ms, self.smoothed_rtt_ms) {
            (Some(prev_offset), Some(prev_rtt)) => {
                self.smoothed_offset_ms = Some((1.0 - alpha) * prev_offset + alpha * (sample.offset_ms as f64));
                self.smoothed_rtt_ms = Some((1.0 - alpha) * prev_rtt + alpha * (sample.rtt_ms as f64));
            }
            _ => {
                self.smoothed_offset_ms = Some(sample.offset_ms as f64);
                self.smoothed_rtt_ms = Some(sample.rtt_ms as f64);
            }
        }

        if self.history.len() >= self.config.history_capacity {
            self.history.pop_front();
        }
        self.history.push_back(sample);

        Ok(sample)
    }

    pub fn smoothed_offset_ms(&self) -> Option<i64> {
        self.smoothed_offset_ms.map(|v| v.round() as i64)
    }

    pub fn smoothed_rtt_ms(&self) -> Option<u64> {
        self.smoothed_rtt_ms.map(|v| v.round().max(0.0) as u64)
    }

    pub fn estimate_server_time(&self, client_time_ms: u64) -> Option<u64> {
        let offset = self.smoothed_offset_ms()?;
        let server_time = (client_time_ms as i64) + offset;
        Some(server_time.max(0) as u64)
    }

    pub fn is_synchronized(&self) -> bool {
        self.history.len() >= 2 && self.smoothed_offset_ms.is_some()
    }
}

pub fn project_target_position(
    status: PlaybackStatus,
    position_ms: u64,
    speed: f32,
    updated_at_server_ms: u64,
    current_server_time_ms: u64,
    duration_ms: u64,
) -> u64 {
    match status {
        PlaybackStatus::Paused | PlaybackStatus::Buffering => {
            if duration_ms > 0 {
                position_ms.min(duration_ms)
            } else {
                position_ms
            }
        }
        PlaybackStatus::Playing => {
            let elapsed_ms = current_server_time_ms.saturating_sub(updated_at_server_ms);

            let safe_speed = if speed > 0.0 { speed as f64 } else { 1.0 };
            let projected_delta = ((elapsed_ms as f64) * safe_speed).round() as u64;
            let target_pos = position_ms.saturating_add(projected_delta);

            if duration_ms > 0 {
                target_pos.min(duration_ms)
            } else {
                target_pos
            }
        }
    }
}

pub fn project_state_position(state: &PlaybackState, current_server_time_ms: u64) -> u64 {
    project_target_position(
        state.status,
        state.position_ms,
        state.speed,
        state.updated_at_server_ms,
        current_server_time_ms,
        state.duration_ms,
    )
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Correction {
    None,
    AdjustSpeed {
        speed: f32,
        drift_ms: i64,
    },
    HardSeek {
        target_position_ms: u64,
        drift_ms: i64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DriftEngineConfig {
    pub deadband_ms: u64,
    pub hard_seek_ms: u64,
    pub convergence_ms: u64,
    pub speed_up_factor: f32,
    pub slow_down_factor: f32,
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

    pub fn evaluate(
        &mut self,
        client_position_ms: u64,
        expected_position_ms: u64,
        nominal_speed: f32,
    ) -> Correction {
        let drift_ms = (client_position_ms as i64) - (expected_position_ms as i64);
        let abs_drift = drift_ms.unsigned_abs();

        if abs_drift > self.config.hard_seek_ms {
            self.is_adjusting = false;
            return Correction::HardSeek {
                target_position_ms: expected_position_ms,
                drift_ms,
            };
        }

        if self.is_adjusting {
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
            if abs_drift <= self.config.deadband_ms {
                Correction::None
            } else {
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
