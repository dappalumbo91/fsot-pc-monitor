//! Session-level scientific metrics derived from the plant fold stream.
//! Noise floor, excursions, recovery τ, trit dwell, lag correlations.

use crate::fold::{CORR_MAX_LAG, EXCURSION_BAND, IDLE_CAL_SAMPLES, RECOVERY_BAND};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

const RING: usize = 3600; // up to 1h at 1 Hz for session stats

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScienceMetrics {
    pub samples: u64,
    pub elapsed_s: f64,
    /// Std-dev of S over idle calibration window (instrument noise floor).
    pub noise_floor_s: Option<f64>,
    /// Mean S during idle calibration.
    pub idle_mean_s: Option<f64>,
    /// Reference S at V0-like rest (analytic).
    pub s_ref: f64,
    /// Excursions per hour (|S−S_ref| > band).
    pub excursion_rate_per_h: f64,
    pub excursion_count: u64,
    /// Recovery time constant estimate (seconds); None if never recovered.
    pub recovery_tau_s: Option<f64>,
    /// Fraction of samples in each trit state for S.
    pub trit_dwell_s_neg: f64,
    pub trit_dwell_s_zero: f64,
    pub trit_dwell_s_pos: f64,
    /// Lag-0 Pearson r(cpu_frac, S), r(temp, S) if available.
    pub corr_cpu_s: Option<f64>,
    pub corr_temp_s: Option<f64>,
    pub corr_gpu_s: Option<f64>,
    /// Best |r| lag for cpu→S in 0..CORR_MAX_LAG.
    pub corr_cpu_s_best_lag: Option<i32>,
    pub corr_cpu_s_best_r: Option<f64>,
}

#[derive(Debug, Clone)]
struct SamplePt {
    s: f64,
    cpu: f64,
    gpu: f64,
    temp: Option<f64>,
    trit_s: i8,
    load: f64,
}

pub struct ScienceTracker {
    pub s_ref: f64,
    started: std::time::Instant,
    pts: VecDeque<SamplePt>,
    idle_s: Vec<f64>,
    excursion_count: u64,
    /// Last time we were in excursion; for recovery measurement.
    excursion_start: Option<std::time::Instant>,
    recovery_samples: Vec<f64>,
    trit_neg: u64,
    trit_zero: u64,
    trit_pos: u64,
    calibrated: bool,
    noise_floor: Option<f64>,
    idle_mean: Option<f64>,
}

impl ScienceTracker {
    pub fn new(s_ref: f64) -> Self {
        Self {
            s_ref,
            started: std::time::Instant::now(),
            pts: VecDeque::with_capacity(RING),
            idle_s: Vec::with_capacity(IDLE_CAL_SAMPLES),
            excursion_count: 0,
            excursion_start: None,
            recovery_samples: Vec::new(),
            trit_neg: 0,
            trit_zero: 0,
            trit_pos: 0,
            calibrated: false,
            noise_floor: None,
            idle_mean: None,
        }
    }

    pub fn push(&mut self, s: f64, cpu_frac: f64, gpu_frac: f64, temp: Option<f64>, trit_s: i8, load: f64) {
        // Idle calibration: low load samples until window full
        if !self.calibrated {
            if load < 0.35 {
                self.idle_s.push(s);
            }
            if self.idle_s.len() >= IDLE_CAL_SAMPLES {
                let mean = mean(&self.idle_s);
                let sd = stddev(&self.idle_s, mean);
                self.idle_mean = Some(mean);
                self.noise_floor = Some(sd);
                self.calibrated = true;
            }
        }

        let exc = (s - self.s_ref).abs() > EXCURSION_BAND;
        if exc {
            if self.excursion_start.is_none() {
                self.excursion_count += 1;
                self.excursion_start = Some(std::time::Instant::now());
            }
        } else if let Some(start) = self.excursion_start.take() {
            let tau = start.elapsed().as_secs_f64();
            if tau > 0.0 && (s - self.s_ref).abs() <= RECOVERY_BAND {
                self.recovery_samples.push(tau);
            }
        }

        match trit_s {
            -1 => self.trit_neg += 1,
            1 => self.trit_pos += 1,
            _ => self.trit_zero += 1,
        }

        if self.pts.len() >= RING {
            self.pts.pop_front();
        }
        self.pts.push_back(SamplePt {
            s,
            cpu: cpu_frac,
            gpu: gpu_frac,
            temp,
            trit_s,
            load,
        });
    }

    pub fn snapshot(&self) -> ScienceMetrics {
        let samples = self.trit_neg + self.trit_zero + self.trit_pos;
        let elapsed = self.started.elapsed().as_secs_f64().max(1e-6);
        let hours = elapsed / 3600.0;
        let total_t = samples.max(1) as f64;

        let s_vals: Vec<f64> = self.pts.iter().map(|p| p.s).collect();
        let cpu_vals: Vec<f64> = self.pts.iter().map(|p| p.cpu).collect();
        let gpu_vals: Vec<f64> = self.pts.iter().map(|p| p.gpu).collect();
        // Align temp with s only where temp present
        let (s_t, t_t): (Vec<f64>, Vec<f64>) = self
            .pts
            .iter()
            .filter_map(|p| p.temp.map(|t| (p.s, t)))
            .unzip();

        let corr_cpu = pearson(&cpu_vals, &s_vals);
        let corr_gpu = pearson(&gpu_vals, &s_vals);
        let corr_temp = if s_t.len() >= 8 {
            pearson(&t_t, &s_t)
        } else {
            None
        };

        let (best_lag, best_r) = best_lagged_corr(&cpu_vals, &s_vals, CORR_MAX_LAG);

        let recovery_tau = if self.recovery_samples.is_empty() {
            None
        } else {
            Some(mean(&self.recovery_samples))
        };

        ScienceMetrics {
            samples,
            elapsed_s: elapsed,
            noise_floor_s: self.noise_floor,
            idle_mean_s: self.idle_mean,
            s_ref: self.s_ref,
            excursion_rate_per_h: self.excursion_count as f64 / hours,
            excursion_count: self.excursion_count,
            recovery_tau_s: recovery_tau,
            trit_dwell_s_neg: self.trit_neg as f64 / total_t,
            trit_dwell_s_zero: self.trit_zero as f64 / total_t,
            trit_dwell_s_pos: self.trit_pos as f64 / total_t,
            corr_cpu_s: corr_cpu,
            corr_temp_s: corr_temp,
            corr_gpu_s: corr_gpu,
            corr_cpu_s_best_lag: best_lag,
            corr_cpu_s_best_r: best_r,
        }
    }
}

fn mean(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    xs.iter().sum::<f64>() / xs.len() as f64
}

fn stddev(xs: &[f64], mean: f64) -> f64 {
    if xs.len() < 2 {
        return 0.0;
    }
    let v = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (xs.len() as f64 - 1.0);
    v.sqrt()
}

fn pearson(x: &[f64], y: &[f64]) -> Option<f64> {
    let n = x.len().min(y.len());
    if n < 8 {
        return None;
    }
    let x = &x[x.len() - n..];
    let y = &y[y.len() - n..];
    let mx = mean(x);
    let my = mean(y);
    let mut num = 0.0;
    let mut dx = 0.0;
    let mut dy = 0.0;
    for i in 0..n {
        let a = x[i] - mx;
        let b = y[i] - my;
        num += a * b;
        dx += a * a;
        dy += b * b;
    }
    let den = (dx * dy).sqrt();
    if den < 1e-12 {
        return None;
    }
    Some((num / den).clamp(-1.0, 1.0))
}

fn best_lagged_corr(x: &[f64], y: &[f64], max_lag: usize) -> (Option<i32>, Option<f64>) {
    let mut best_r = 0.0_f64;
    let mut best_lag: Option<i32> = None;
    for lag in 0..=max_lag {
        if x.len() <= lag + 8 || y.len() <= lag + 8 {
            break;
        }
        // x leads y by lag: compare x[0..n-lag] with y[lag..]
        let n = x.len().min(y.len()) - lag;
        let xs: Vec<f64> = x[x.len() - n - lag..x.len() - lag].to_vec();
        let ys: Vec<f64> = y[y.len() - n..].to_vec();
        if let Some(r) = pearson(&xs, &ys) {
            if best_lag.is_none() || r.abs() > best_r.abs() {
                best_r = r;
                best_lag = Some(lag as i32);
            }
        }
    }
    (best_lag, best_lag.map(|_| best_r))
}
