//! NTP-style clock offset estimation, latency tracking, and EMA smoothing.

use std::collections::VecDeque;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ClockSyncError {
    #[error("Causality violation: t3 ({t3}) < t0 ({t0}) or t2 ({t2}) < t1 ({t1})")]
    CausalityViolation { t0: u64, t1: u64, t2: u64, t3: u64 },
    #[error("Processing time exceeds RTT: (t3 - t0) = {client_elapsed} < (t2 - t1) = {server_elapsed}")]
    NegativeRtt { client_elapsed: u64, server_elapsed: u64 },
    #[error("Outlier RTT rejected: rtt {rtt_ms}ms exceeds threshold")]
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
    pub alpha: f64,                     // Default: 0.2
    pub max_rtt_threshold_ms: u64,      // Default: 2000 ms
    pub outlier_multiplier: f64,        // Default: 2.5x smoothed RTT
    pub min_samples_for_outlier: usize, // Default: 3
    pub outlier_abs_margin_ms: u64,     // Default: 150 ms
    pub history_capacity: usize,        // Default: 16
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
    total_samples: u64,
    rejected_samples: u64,
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

        // 1. Absolute RTT ceiling
        if sample.rtt_ms > self.config.max_rtt_threshold_ms {
            self.rejected_samples += 1;
            return Err(ClockSyncError::OutlierRejected {
                rtt_ms: sample.rtt_ms,
                threshold_ms: self.config.max_rtt_threshold_ms,
            });
        }

        // 2. Relative RTT outlier filter
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

        // 3. Update EMA
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

    pub fn total_samples(&self) -> u64 {
        self.total_samples
    }

    pub fn rejected_samples(&self) -> u64 {
        self.rejected_samples
    }

    pub fn reset(&mut self) {
        self.smoothed_offset_ms = None;
        self.smoothed_rtt_ms = None;
        self.history.clear();
        self.total_samples = 0;
        self.rejected_samples = 0;
    }
}
