//! Plant-host bridge: hardware observables → FSOT route slots.
//!
//! All weights and scales are preregistered in `fold.rs`. No session fitting.

use crate::fold::*;
use serde::{Deserialize, Serialize};

/// Route slots consumed by the scalar engine (domain fold inputs).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RouteSlots {
    pub d_eff: f64,
    pub n: f64,
    pub p: f64,
    pub recent_hits: f64,
    pub delta_psi: f64,
    pub delta_theta: f64,
    pub rho: f64,
    pub scale: f64,
    pub amplitude: f64,
    pub trend_bias: f64,
    pub observed: bool,
    /// Uniform plant load in [0,1] (equal-weight 4-channel).
    pub load: f64,
    /// V0 load for the same bridge.
    pub load_v0: f64,
    pub frac_cpu: f64,
    pub frac_mem: f64,
    pub frac_gpu: f64,
    pub frac_thermal: f64,
}

/// V0 observables needed by the bridge (unitless fractions + optional temp °C).
#[derive(Debug, Clone, Copy)]
pub struct V0Fracs {
    pub cpu_frac: f64,
    pub mem_frac: f64,
    pub gpu_frac: f64,
    pub temp_c: Option<f64>,
}

/// Live hardware in bridge units.
#[derive(Debug, Clone, Copy)]
pub struct HardwareFracs {
    pub cpu_frac: f64,
    pub mem_frac: f64,
    pub gpu_frac: f64,
    pub temp_c: Option<f64>,
}

/// Map optional temperature to thermal activity ∈ [0,1] vs V0.
/// Missing sensor → 0 activity (honest: no thermal channel).
fn thermal_activity(temp_now: Option<f64>, temp_v0: Option<f64>) -> f64 {
    match (temp_now, temp_v0) {
        (Some(now), Some(base)) => {
            ((now - base).abs() / TEMP_PHASE_SCALE_C).clamp(0.0, 1.0)
        }
        _ => 0.0,
    }
}

/// Equal-weight 4-channel plant load (CHANNEL_WEIGHT = 1/4 each).
fn plant_load(cpu: f64, mem: f64, gpu: f64, thermal: f64) -> f64 {
    CHANNEL_WEIGHT * (cpu + mem + gpu + thermal)
}

/// Primary bridge: observables + V0 → route slots.
pub fn bridge(hw: HardwareFracs, v0: V0Fracs) -> RouteSlots {
    let frac_cpu = hw.cpu_frac.clamp(0.0, 1.0);
    let frac_mem = hw.mem_frac.clamp(0.0, 1.0);
    let frac_gpu = hw.gpu_frac.clamp(0.0, 1.0);
    let frac_thermal = thermal_activity(hw.temp_c, v0.temp_c);

    let load = plant_load(frac_cpu, frac_mem, frac_gpu, frac_thermal).clamp(0.0, 1.0);

    let v0_thermal = thermal_activity(v0.temp_c, v0.temp_c); // 0 at baseline
    let load_v0 = plant_load(
        v0.cpu_frac.clamp(0.0, 1.0),
        v0.mem_frac.clamp(0.0, 1.0),
        v0.gpu_frac.clamp(0.0, 1.0),
        v0_thermal,
    )
    .clamp(0.0, 1.0);

    // δψ: phase deviation = load excursion from V0 (clamped to unit interval range)
    let delta_psi = (load - load_v0).clamp(-1.0, 1.0);

    // δθ: thermal phase from absolute ΔT / preregistered scale
    let delta_theta = match (hw.temp_c, v0.temp_c) {
        (Some(now), Some(base)) => ((now - base) / TEMP_PHASE_SCALE_C).clamp(-1.0, 1.0),
        _ => 0.0,
    };

    // recent_hits: load pressure scaled into [0, N]
    let recent_hits = (load * PLANT_N).clamp(0.0, PLANT_N);

    // ρ: memory density channel
    let rho = frac_mem;

    RouteSlots {
        d_eff: PLANT_D_EFF,
        n: PLANT_N,
        p: PLANT_P,
        recent_hits,
        delta_psi,
        delta_theta,
        rho,
        scale: 1.0,
        amplitude: 1.0,
        trend_bias: 0.0,
        observed: true,
        load,
        load_v0,
        frac_cpu,
        frac_mem,
        frac_gpu,
        frac_thermal,
    }
}
