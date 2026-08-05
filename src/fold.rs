//! Preregistered domain fold: `plant_host_v1`.
//!
//! One law (pin D1D38A), many folds. This is the **silicon host plant** fold —
//! not the neuroscience fold (D_eff=13). All route slots and bridge constants
//! here are preregistered or seed-derived. **Zero free least-squares parameters.**
//!
//! See `BRIDGE.md` for the full formula map and non-claims.

use crate::seeds::{K, PHI, PI};

/// Authority pin (SHA-256 of archive `vendor/fsot_compute.py`).
pub const PIN_SHA256: &str =
    "D1D38A185487B452E470AC68ECE2EB45AEB1CA9CE25FC9BF9564C19633FFBE70";

/// Short pin tag for HUD.
pub const PIN_TAG: &str = "D1D38A";

/// Named fold identity — must appear on every telemetry row and certificate.
pub const FOLD_ID: &str = "plant_host_v1";

/// Bridge formula version (bump only when mapping equations change).
pub const BRIDGE_VERSION: &str = "1.0.0";

/// Application / instrument version.
pub const INSTRUMENT_VERSION: &str = "0.4.0";

// ---------------------------------------------------------------------------
// Plant domain fold (preregistered route slots)
// ---------------------------------------------------------------------------

/// Effective dimension — seed-lawful: φ²·π (not neuro 13).
pub const PLANT_D_EFF: f64 = PHI * PHI * PI;

/// Host channels: CPU, memory, GPU, thermal activity.
pub const PLANT_N: f64 = 4.0;

/// Process / pathway count proxy — seed: φ.
pub const PLANT_P: f64 = PHI;

/// Equal channel weight = 1/N (honest uniform plant mix; not LSQ-fit).
pub const CHANNEL_WEIGHT: f64 = 1.0 / PLANT_N;

// ---------------------------------------------------------------------------
// Bridge scales (preregistered; match formula-corpus D/25 convention where noted)
// ---------------------------------------------------------------------------

/// Celsius span that maps |ΔT| → unit thermal activity / |δθ|→1.
/// Matches scalar corpus reference scale 25 (D/25 term).
pub const TEMP_PHASE_SCALE_C: f64 = 25.0;

/// Trit cut points on capacity fractions [0,1] — classic trinary thirds.
pub const TRIT_FRAC_LO: f64 = 1.0 / 3.0;
pub const TRIT_FRAC_HI: f64 = 2.0 / 3.0;

/// Trit band on temperature delta (°C) — seed e.
pub const TEMP_TRIT_BAND_C: f64 = crate::seeds::E;

/// Trit deadzone on scalar S — seed K.
pub const S_TRIT_LO: f64 = -K;
pub const S_TRIT_HI: f64 = K;

/// Excursion: |S − S_ref| above this counts as an event — seed K.
pub const EXCURSION_BAND: f64 = K;

/// Recovery: S is "settled" when |S − S_ref| ≤ this (same band).
pub const RECOVERY_BAND: f64 = K;

/// Idle calibration window (samples) after V0 for noise floor.
pub const IDLE_CAL_SAMPLES: usize = 30;

/// Lag window (samples) for Pearson correlations.
pub const CORR_MAX_LAG: usize = 5;

/// Optional interoception UDP port (neuron-zig plant ingest).
pub const INTEROCEPTION_PORT: u16 = 9876;
