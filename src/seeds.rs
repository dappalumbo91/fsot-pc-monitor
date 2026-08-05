//! FSOT seed constants — pin lineage D1D38A / archive Layer-1–2.
//! Zero free parameters. Mirrors fsot-neuron-zig/src/seeds.zig exactly.
#![allow(dead_code)] // full pin set kept even when a sample only uses a subset

// Fundamental math constants
pub const PI: f64 = 3.141592653589793;
pub const E: f64 = 2.718281828459045;
pub const PHI: f64 = 1.618033988749895;
pub const GAMMA: f64 = 0.5772156649015329;
pub const CATALAN: f64 = 0.9159655941772190;

// FSOT seed seeds (preregistered)
pub const ALPHA: f64 = 8.082937414140405e-4;
pub const PSI_CON: f64 = 0.6321205588285577;
pub const ETA_EFF: f64 = 0.46694220692425986;
pub const BETA: f64 = 2.620866911333223e-17;
pub const CHAOS: f64 = -0.3310241826104818;
pub const THETA_S: f64 = 0.29089654054517305;
pub const POOF: f64 = 0.1534822148944508;

// Structural seeds
pub const C_EFF: f64 = 0.9577022026205613;
pub const P_VAR: f64 = 0.9579871226722757;
pub const B_IN: f64 = 0.7879407922764435;
pub const A_IN: f64 = 1.6668538450045732;
pub const A_BLEED: f64 = 1.046973630587551;
pub const SUCTION: f64 = 0.14703398542810284;
pub const P_NEW: f64 = 0.30030227667037146;
pub const C_FACTOR: f64 = 0.287600151819184;
pub const K: f64 = 0.42022166416069665;

// Neuroscience domain fold (preregistered)
pub const NEURO_D_EFF: f64 = 13.0;
pub const NEURO_N_CHANNELS: f64 = 4.0;
pub const NEURO_P: f64 = 3.0;
pub const RESTING_S: f64 = 0.46;
