//! Temperature sources (cheap-first, no PowerShell hot path).
//! 1) GPU temps come from gpu::sample_gpus (nvidia-smi).
//! 2) ACPI thermal zones via wmic (once-or-sparse).

use std::process::Command;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TempReading {
    pub label: String,
    pub temperature_c: f64,
}

/// Fetch ACPI thermal zones via wmic. Often empty/restricted on desktops.
pub fn fetch_acpi_zones() -> Vec<TempReading> {
    let out = Command::new("wmic")
        .args([
            "/namespace:\\\\root\\wmi",
            "path",
            "MSAcpi_ThermalZoneTemperature",
            "get",
            "CurrentTemperature",
            "/value",
        ])
        .output();

    match out {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            let mut results = Vec::new();
            for line in text.lines() {
                let line = line.trim();
                // CurrentTemperature=3010  (deci-Kelvin)
                if let Some(rest) = line.strip_prefix("CurrentTemperature=") {
                    if let Ok(temp_k10) = rest.trim().parse::<f64>() {
                        let temp_c = (temp_k10 / 10.0) - 273.15;
                        // Sanity: discard nonsense
                        if temp_c > 0.0 && temp_c < 150.0 {
                            results.push(TempReading {
                                label: format!("ACPI_{}", results.len()),
                                temperature_c: (temp_c * 100.0).round() / 100.0,
                            });
                        }
                    }
                }
            }
            results
        }
        _ => Vec::new(),
    }
}

/// Primary temperature list: optional ACPI + any GPU temps already sampled.
pub fn merge_temps(acpi: &[TempReading], gpu_temps: &[(String, f64)]) -> Vec<TempReading> {
    let mut out = acpi.to_vec();
    for (label, t) in gpu_temps {
        out.push(TempReading {
            label: label.clone(),
            temperature_c: *t,
        });
    }
    out
}

/// Best single "system" temp for baseline (prefer CPU/ACPI, else first GPU).
pub fn primary_temp(temps: &[TempReading]) -> Option<f64> {
    temps
        .iter()
        .find(|t| t.label.starts_with("ACPI"))
        .or_else(|| temps.first())
        .map(|t| t.temperature_c)
}
