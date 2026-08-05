//! FSOT scalar engine S = K*(T1+T2+T3) clamped [-3,+3].
//! Mirrors fsot-neuron-zig/src/scalar.zig `computeScalar` (full signature).

use crate::bridge::RouteSlots;
use crate::seeds::*;

/// Full term breakdown for telemetry.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct ScalarParts {
    pub s: f64,
    pub t1: f64,
    pub t2: f64,
    pub t3: f64,
}

/// Full fluid scalar — matches Zig `computeScalar`.
pub fn compute_scalar(
    n: f64,
    p: f64,
    d_eff: f64,
    recent_hits: f64,
    delta_psi: f64,
    delta_theta: f64,
    rho: f64,
    scale: f64,
    amplitude: f64,
    trend_bias: f64,
    observed: bool,
) -> ScalarParts {
    let d = if d_eff < 1e-6 { 1e-6 } else { d_eff };
    let nn = if n < 1e-6 { 1e-6 } else { n };

    let growth = (ALPHA * (1.0 - recent_hits / nn) * GAMMA / PHI).exp();
    let mut t1 = (n * p / d.sqrt())
        * ((PSI_CON + delta_psi) / ETA_EFF).cos()
        * (-ALPHA * recent_hits / nn + rho + B_IN * delta_psi).exp()
        * (1.0 + growth * C_EFF);
    t1 *= 1.0 + P_NEW * (d / 25.0).ln();
    if observed {
        t1 *= (C_FACTOR * P_VAR).exp() * (delta_psi + P_VAR).cos();
    }

    let t2 = scale * amplitude + trend_bias;

    let valve = BETA
        * delta_psi.cos()
        * (n * p / d.sqrt())
        * (1.0 + CHAOS * (d - 25.0) / 25.0)
        * (1.0 + POOF * (THETA_S + PI).cos() + SUCTION * THETA_S.sin());
    let acoustic = 1.0
        + A_BLEED * delta_theta.sin().powi(2) / PHI
        + A_IN * delta_theta.cos().powi(2) / PHI;
    let phase = 1.0 + B_IN * P_VAR;
    let t3 = valve * acoustic * phase;

    let s = (K * (t1 + t2 + t3)).clamp(-3.0, 3.0);
    ScalarParts { s, t1, t2, t3 }
}

/// Evaluate scalar from preregistered plant route slots.
pub fn from_route(r: &RouteSlots) -> ScalarParts {
    compute_scalar(
        r.n,
        r.p,
        r.d_eff,
        r.recent_hits,
        r.delta_psi,
        r.delta_theta,
        r.rho,
        r.scale,
        r.amplitude,
        r.trend_bias,
        r.observed,
    )
}

/// Reference S at V0-like rest for this fold (δψ=0, δθ=0, hits=0, ρ=v0_mem).
pub fn reference_s(rho_v0: f64) -> f64 {
    compute_scalar(
        crate::fold::PLANT_N,
        crate::fold::PLANT_P,
        crate::fold::PLANT_D_EFF,
        0.0,
        0.0,
        0.0,
        rho_v0.clamp(0.0, 1.0),
        1.0,
        1.0,
        0.0,
        true,
    )
    .s
}
