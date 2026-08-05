//! Session certificate + export kit (reproducible run bundle).

use crate::fold::*;
use crate::science::ScienceMetrics;
use chrono::Local;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCertificate {
    pub pin_sha256: String,
    pub pin_tag: String,
    pub fold_id: String,
    pub bridge_version: String,
    pub instrument_version: String,
    pub host_name: String,
    pub cpu_brand: String,
    pub session_start: String,
    pub session_end: String,
    pub log_path: String,
    pub log_sha256: Option<String>,
    pub sample_count: u64,
    pub plant_d_eff: f64,
    pub plant_n: f64,
    pub plant_p: f64,
    pub channel_weight: f64,
    pub temp_phase_scale_c: f64,
    pub excursion_band: f64,
    pub science: ScienceMetrics,
    /// One-line identity note (fold purpose), not a list of disclaimers.
    pub notes: String,
}

pub fn build_certificate(
    host_name: &str,
    cpu_brand: &str,
    session_start: &str,
    science: &ScienceMetrics,
    log_path: &Path,
) -> SessionCertificate {
    let log_sha = hash_file(log_path).ok();
    SessionCertificate {
        pin_sha256: PIN_SHA256.into(),
        pin_tag: PIN_TAG.into(),
        fold_id: FOLD_ID.into(),
        bridge_version: BRIDGE_VERSION.into(),
        instrument_version: INSTRUMENT_VERSION.into(),
        host_name: host_name.into(),
        cpu_brand: cpu_brand.into(),
        session_start: session_start.into(),
        session_end: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        log_path: log_path.display().to_string(),
        log_sha256: log_sha,
        sample_count: science.samples,
        plant_d_eff: PLANT_D_EFF,
        plant_n: PLANT_N,
        plant_p: PLANT_P,
        channel_weight: CHANNEL_WEIGHT,
        temp_phase_scale_c: TEMP_PHASE_SCALE_C,
        excursion_band: EXCURSION_BAND,
        science: science.clone(),
        notes: format!(
            "FSOT plant_host_v1 session — same scalar spine (pin {PIN_TAG}); \
             equal-weight 4-channel bridge; missing temp ⇒ thermal activity 0."
        ),
    }
}

pub fn hash_file(path: &Path) -> std::io::Result<String> {
    let mut f = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Write `exports/session_YYYYMMDD_HHMMSS/{certificate.json, telemetry.jsonl, science.csv}`.
pub fn export_run(
    cert: &SessionCertificate,
    log_path: &Path,
) -> std::io::Result<PathBuf> {
    let stamp = Local::now().format("%Y%m%d_%H%M%S");
    let dir = PathBuf::from("exports").join(format!("session_{stamp}"));
    fs::create_dir_all(&dir)?;

    let cert_path = dir.join("certificate.json");
    let mut cf = File::create(&cert_path)?;
    writeln!(cf, "{}", serde_json::to_string_pretty(cert).unwrap_or_default())?;

    // Copy telemetry
    let dest_log = dir.join("telemetry.jsonl");
    if log_path.exists() {
        fs::copy(log_path, &dest_log)?;
    } else {
        File::create(&dest_log)?;
    }

    // Science CSV summary
    let csv_path = dir.join("science.csv");
    let mut csv = File::create(&csv_path)?;
    writeln!(csv, "metric,value")?;
    let s = &cert.science;
    writeln!(csv, "samples,{}", s.samples)?;
    writeln!(csv, "elapsed_s,{:.3}", s.elapsed_s)?;
    writeln!(
        csv,
        "noise_floor_s,{}",
        s.noise_floor_s
            .map(|v| format!("{v:.6}"))
            .unwrap_or_else(|| "na".into())
    )?;
    writeln!(
        csv,
        "idle_mean_s,{}",
        s.idle_mean_s
            .map(|v| format!("{v:.6}"))
            .unwrap_or_else(|| "na".into())
    )?;
    writeln!(csv, "s_ref,{:.6}", s.s_ref)?;
    writeln!(csv, "excursion_rate_per_h,{:.4}", s.excursion_rate_per_h)?;
    writeln!(csv, "excursion_count,{}", s.excursion_count)?;
    writeln!(
        csv,
        "recovery_tau_s,{}",
        s.recovery_tau_s
            .map(|v| format!("{v:.3}"))
            .unwrap_or_else(|| "na".into())
    )?;
    writeln!(csv, "trit_dwell_s_neg,{:.4}", s.trit_dwell_s_neg)?;
    writeln!(csv, "trit_dwell_s_zero,{:.4}", s.trit_dwell_s_zero)?;
    writeln!(csv, "trit_dwell_s_pos,{:.4}", s.trit_dwell_s_pos)?;
    writeln!(
        csv,
        "corr_cpu_s,{}",
        s.corr_cpu_s
            .map(|v| format!("{v:.4}"))
            .unwrap_or_else(|| "na".into())
    )?;
    writeln!(
        csv,
        "corr_gpu_s,{}",
        s.corr_gpu_s
            .map(|v| format!("{v:.4}"))
            .unwrap_or_else(|| "na".into())
    )?;
    writeln!(
        csv,
        "corr_temp_s,{}",
        s.corr_temp_s
            .map(|v| format!("{v:.4}"))
            .unwrap_or_else(|| "na".into())
    )?;
    writeln!(csv, "fold_id,{}", cert.fold_id)?;
    writeln!(csv, "pin_tag,{}", cert.pin_tag)?;
    writeln!(csv, "bridge_version,{}", cert.bridge_version)?;

    // Manifest
    let mut man = File::create(dir.join("MANIFEST.txt"))?;
    writeln!(man, "FSOT PC Monitor export")?;
    writeln!(man, "fold={} pin={}", FOLD_ID, PIN_TAG)?;
    writeln!(man, "bridge={}", BRIDGE_VERSION)?;
    writeln!(man, "files: certificate.json telemetry.jsonl science.csv")?;
    writeln!(man, "See BRIDGE.md in project root for formula identity.")?;

    Ok(dir)
}
