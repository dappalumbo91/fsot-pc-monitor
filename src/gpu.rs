//! GPU discovery + live usage.
//! Prefer nvidia-smi (fast, no PowerShell). Fall back to static name/VRAM via wmic.

use std::process::Command;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GpuInfo {
    pub index: usize,
    pub name: String,
    pub vram_mb: u64,
    pub usage_pct: f32,
    pub temp_c: Option<f64>,
    pub source: String,
}

/// Live GPU sample. Empty vec if nothing found.
pub fn sample_gpus() -> Vec<GpuInfo> {
    if let Some(gpus) = sample_nvidia() {
        return gpus;
    }
    discover_static()
}

/// nvidia-smi: name, VRAM total (MiB), util %, temp °C.
fn sample_nvidia() -> Option<Vec<GpuInfo>> {
    let out = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total,utilization.gpu,temperature.gpu",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;

    if !out.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let mut gpus = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if parts.len() < 4 {
            continue;
        }
        let name = parts[0].to_string();
        let vram_mb: u64 = parts[1].parse().unwrap_or(0);
        let usage_pct: f32 = parts[2].parse().unwrap_or(0.0);
        let temp_c: Option<f64> = parts[3].parse().ok();
        gpus.push(GpuInfo {
            index: i,
            name,
            vram_mb,
            usage_pct,
            temp_c,
            source: "nvidia-smi".into(),
        });
    }
    if gpus.is_empty() {
        None
    } else {
        Some(gpus)
    }
}

/// One-shot static discovery via wmic (no PowerShell). usage/temp unknown.
fn discover_static() -> Vec<GpuInfo> {
    let out = Command::new("wmic")
        .args([
            "path",
            "Win32_VideoController",
            "get",
            "Name,AdapterRAM",
            "/format:csv",
        ])
        .output();

    let mut gpus = Vec::new();
    match out {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            for line in text.lines().skip(1) {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                // CSV: Node,AdapterRAM,Name
                let parts: Vec<&str> = line.split(',').collect();
                if parts.len() < 3 {
                    continue;
                }
                let vram_bytes: u64 = parts[1].trim().parse().unwrap_or(0);
                let name = parts[2..].join(",").trim().to_string();
                if name.is_empty() || name.eq_ignore_ascii_case("Name") {
                    continue;
                }
                gpus.push(GpuInfo {
                    index: gpus.len(),
                    name,
                    vram_mb: vram_bytes / (1024 * 1024),
                    usage_pct: 0.0,
                    temp_c: None,
                    source: "wmic".into(),
                });
            }
        }
        _ => {}
    }

    if gpus.is_empty() {
        gpus.push(GpuInfo {
            index: 0,
            name: "GPU (not detected)".into(),
            vram_mb: 0,
            usage_pct: 0.0,
            temp_c: None,
            source: "none".into(),
        });
    }
    gpus
}
