//! Sampling engine: hardware → preregistered plant bridge → scalar → science → log.

use crate::bridge::{self, HardwareFracs, V0Fracs};
use crate::certificate::{self, SessionCertificate};
use crate::fold::*;
use crate::gpu;
use crate::scalar;
use crate::science::{ScienceMetrics, ScienceTracker};
use crate::stream::InteroceptionStream;
use crate::thermal;
use crate::trit;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

pub const HISTORY_LEN: usize = 180;
pub const SAMPLE_INTERVAL: Duration = Duration::from_millis(1000);
const ACPI_EVERY: u64 = 20;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryRow {
    pub timestamp: String,
    pub pin_tag: String,
    pub fold_id: String,
    pub bridge_version: String,
    pub hardware: HardwarePoint,
    pub route: RoutePoint,
    pub fsot: FsotPoint,
    pub delta_from_v0: DeltaPoint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwarePoint {
    pub cpu_usage_pct: f32,
    pub cpu_per_core: Vec<f32>,
    pub mem_used_gb: f64,
    pub mem_total_gb: f64,
    pub mem_frac: f64,
    pub temperatures: Vec<thermal::TempReading>,
    pub gpus: Vec<GpuPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuPoint {
    pub name: String,
    pub vram_mb: u64,
    pub usage_pct: f32,
    pub temp_c: Option<f64>,
    pub source: String,
}

/// Preregistered route slots snapshot (for log identity).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutePoint {
    pub d_eff: f64,
    pub n: f64,
    pub p: f64,
    pub recent_hits: f64,
    pub delta_psi: f64,
    pub delta_theta: f64,
    pub rho: f64,
    pub load: f64,
    pub load_v0: f64,
    pub frac_cpu: f64,
    pub frac_mem: f64,
    pub frac_gpu: f64,
    pub frac_thermal: f64,
    pub observed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsotPoint {
    pub delta_psi: f64,
    pub delta_theta: f64,
    pub recent_hits: f64,
    pub rho: f64,
    pub load: f64,
    pub scalar_s: f64,
    pub t1: f64,
    pub t2: f64,
    pub t3: f64,
    pub s_ref: f64,
    pub trit_cpu: i8,
    pub trit_mem: i8,
    pub trit_gpu: i8,
    pub trit_temp: i8,
    pub trit_s: i8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaPoint {
    pub cpu_pct_points: f32,
    pub mem_gb: f64,
    pub temp_c: Option<f64>,
    pub load: f64,
}

#[derive(Debug, Clone)]
pub struct V0Baseline {
    pub cpu_usage: f32,
    pub mem_used_gb: f64,
    pub mem_total_gb: f64,
    pub mem_frac: f64,
    pub gpu_pct: f32,
    pub temp_c: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct Sample {
    pub timestamp: String,
    pub cpu_pct: f32,
    pub cpu_per_core: Vec<f32>,
    pub mem_used_gb: f64,
    pub mem_total_gb: f64,
    pub mem_frac: f64,
    pub gpu_pct: f32,
    pub gpu_name: String,
    pub gpu_vram_mb: u64,
    pub gpu_temp: Option<f64>,
    pub gpu_source: String,
    pub temp_c: Option<f64>,
    pub temps: Vec<thermal::TempReading>,
    pub gpus: Vec<GpuPoint>,
    pub fsot: FsotPoint,
    pub delta: DeltaPoint,
}

#[derive(Debug, Clone, Default)]
pub struct History {
    pub cpu: VecDeque<f64>,
    pub mem: VecDeque<f64>,
    pub gpu: VecDeque<f64>,
    pub temp: VecDeque<f64>,
    pub scalar_s: VecDeque<f64>,
    pub t1: VecDeque<f64>,
    pub t2: VecDeque<f64>,
    pub t3: VecDeque<f64>,
    pub load: VecDeque<f64>,
}

impl History {
    fn push(&mut self, s: &Sample) {
        push_capped(&mut self.cpu, s.cpu_pct as f64);
        push_capped(&mut self.mem, s.mem_frac * 100.0);
        push_capped(&mut self.gpu, s.gpu_pct as f64);
        push_capped(&mut self.temp, s.temp_c.unwrap_or(f64::NAN));
        push_capped(&mut self.scalar_s, s.fsot.scalar_s);
        push_capped(&mut self.t1, s.fsot.t1);
        push_capped(&mut self.t2, s.fsot.t2);
        push_capped(&mut self.t3, s.fsot.t3);
        push_capped(&mut self.load, s.fsot.load * 100.0);
    }
}

fn push_capped(q: &mut VecDeque<f64>, v: f64) {
    if q.len() >= HISTORY_LEN {
        q.pop_front();
    }
    q.push_back(v);
}

pub struct Engine {
    sys: System,
    pub v0: V0Baseline,
    pub history: History,
    pub latest: Option<Sample>,
    pub tick: u64,
    cached_acpi: Vec<thermal::TempReading>,
    log: Option<File>,
    log_path: PathBuf,
    last_sample: Instant,
    pub host_name: String,
    pub cpu_brand: String,
    pub session_start: String,
    pub s_ref: f64,
    science: ScienceTracker,
    pub stream: InteroceptionStream,
    pub last_export: Option<PathBuf>,
    pub last_export_msg: Option<String>,
}

impl Engine {
    pub fn new() -> Self {
        Self::new_with_stream(false)
    }

    pub fn new_with_stream(interoception: bool) -> Self {
        let mut sys = System::new_with_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );

        sys.refresh_cpu_specifics(CpuRefreshKind::everything());
        std::thread::sleep(Duration::from_millis(600));
        sys.refresh_cpu_specifics(CpuRefreshKind::everything());
        sys.refresh_memory_specifics(MemoryRefreshKind::everything());

        let gpus = gpu::sample_gpus();
        let acpi = thermal::fetch_acpi_zones();
        let gpu_temps: Vec<(String, f64)> = gpus
            .iter()
            .filter_map(|g| g.temp_c.map(|t| (format!("GPU_{}", sanitize_label(&g.name)), t)))
            .collect();
        let temps = thermal::merge_temps(&acpi, &gpu_temps);
        let temp_base = thermal::primary_temp(&temps);

        let cpu = sys.global_cpu_info().cpu_usage();
        let mem_used = bytes_to_gb(sys.used_memory());
        let mem_total = bytes_to_gb(sys.total_memory());
        let mem_frac = (mem_used / mem_total.max(0.001)).clamp(0.0, 1.0);
        let gpu_pct = gpus.first().map(|g| g.usage_pct).unwrap_or(0.0);
        let cpu_brand = sys
            .cpus()
            .first()
            .map(|c| c.brand().to_string())
            .unwrap_or_else(|| "CPU".into());
        let host_name = System::host_name().unwrap_or_else(|| "this-pc".into());

        let v0 = V0Baseline {
            cpu_usage: cpu,
            mem_used_gb: mem_used,
            mem_total_gb: mem_total,
            mem_frac,
            gpu_pct,
            temp_c: temp_base,
        };

        let s_ref = scalar::reference_s(mem_frac);
        let log_path = PathBuf::from("logs").join("fsot_telemetry.log");
        let session_start = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let mut log = open_log(&log_path).ok();
        if let Some(ref mut f) = log {
            let header = serde_json::json!({
                "type": "session_header",
                "pin_tag": PIN_TAG,
                "pin_sha256": PIN_SHA256,
                "fold_id": FOLD_ID,
                "bridge_version": BRIDGE_VERSION,
                "instrument_version": INSTRUMENT_VERSION,
                "host": host_name,
                "cpu_brand": cpu_brand,
                "session_start": session_start,
                "plant_d_eff": PLANT_D_EFF,
                "plant_n": PLANT_N,
                "plant_p": PLANT_P,
                "s_ref": s_ref,
                "v0": {
                    "cpu_pct": cpu,
                    "mem_frac": mem_frac,
                    "gpu_pct": gpu_pct,
                    "temp_c": temp_base,
                }
            });
            let _ = writeln!(f, "{header}");
            let _ = f.flush();
        }

        Self {
            sys,
            v0,
            history: History::default(),
            latest: None,
            tick: 0,
            cached_acpi: acpi,
            log,
            log_path,
            last_sample: Instant::now() - SAMPLE_INTERVAL,
            host_name,
            cpu_brand,
            session_start,
            s_ref,
            science: ScienceTracker::new(s_ref),
            stream: InteroceptionStream::new(interoception),
            last_export: None,
            last_export_msg: None,
        }
    }

    pub fn science(&self) -> ScienceMetrics {
        self.science.snapshot()
    }

    pub fn certificate(&self) -> SessionCertificate {
        certificate::build_certificate(
            &self.host_name,
            &self.cpu_brand,
            &self.session_start,
            &self.science.snapshot(),
            &self.log_path,
        )
    }

    pub fn export_run(&mut self) -> Result<PathBuf, String> {
        let cert = self.certificate();
        match certificate::export_run(&cert, &self.log_path) {
            Ok(dir) => {
                self.last_export = Some(dir.clone());
                self.last_export_msg = Some(format!("Exported → {}", dir.display()));
                Ok(dir)
            }
            Err(e) => {
                self.last_export_msg = Some(format!("Export failed: {e}"));
                Err(e.to_string())
            }
        }
    }

    pub fn maybe_sample(&mut self) -> bool {
        if self.last_sample.elapsed() < SAMPLE_INTERVAL {
            return false;
        }
        self.sample_now();
        true
    }

    pub fn sample_now(&mut self) {
        self.tick += 1;
        self.last_sample = Instant::now();

        self.sys
            .refresh_cpu_specifics(CpuRefreshKind::everything());
        self.sys
            .refresh_memory_specifics(MemoryRefreshKind::everything());

        let cpu_pct = self.sys.global_cpu_info().cpu_usage();
        let cpu_per_core: Vec<f32> = self.sys.cpus().iter().map(|c| c.cpu_usage()).collect();
        let mem_used = bytes_to_gb(self.sys.used_memory());
        let mem_total = bytes_to_gb(self.sys.total_memory());
        let mem_frac = (mem_used / mem_total.max(0.001)).clamp(0.0, 1.0);

        let gpus_raw = gpu::sample_gpus();
        let gpu_pct = gpus_raw.first().map(|g| g.usage_pct).unwrap_or(0.0);
        let gpu0 = gpus_raw.first();

        if self.tick % ACPI_EVERY == 0 {
            self.cached_acpi = thermal::fetch_acpi_zones();
        }
        let gpu_temps: Vec<(String, f64)> = gpus_raw
            .iter()
            .filter_map(|g| g.temp_c.map(|t| (format!("GPU_{}", sanitize_label(&g.name)), t)))
            .collect();
        let temps = thermal::merge_temps(&self.cached_acpi, &gpu_temps);
        let temp_now = thermal::primary_temp(&temps);
        let temp_delta = match (temp_now, self.v0.temp_c) {
            (Some(now), Some(base)) => Some(now - base),
            _ => None,
        };

        // --- preregistered bridge (no ad-hoc weights) ---
        let hw = HardwareFracs {
            cpu_frac: (cpu_pct as f64 / 100.0).clamp(0.0, 1.0),
            mem_frac,
            gpu_frac: (gpu_pct as f64 / 100.0).clamp(0.0, 1.0),
            temp_c: temp_now,
        };
        let v0f = V0Fracs {
            cpu_frac: (self.v0.cpu_usage as f64 / 100.0).clamp(0.0, 1.0),
            mem_frac: self.v0.mem_frac,
            gpu_frac: (self.v0.gpu_pct as f64 / 100.0).clamp(0.0, 1.0),
            temp_c: self.v0.temp_c,
        };
        let route = bridge::bridge(hw, v0f);
        let parts = scalar::from_route(&route);

        let trit_cpu = trit::from_fraction(route.frac_cpu).as_i8();
        let trit_mem = trit::from_fraction(route.frac_mem).as_i8();
        let trit_gpu = trit::from_fraction(route.frac_gpu).as_i8();
        let trit_temp = match temp_delta {
            Some(d) => trit::temp_delta_to_trit(d).as_i8(),
            None => 0,
        };
        // Trit on deviation from analytic S_ref (band ±K), not raw S level.
        let trit_s = trit::s_to_trit(parts.s - self.s_ref).as_i8();

        self.science.push(
            parts.s,
            route.frac_cpu,
            route.frac_gpu,
            temp_now,
            trit_s,
            route.load,
        );

        let gpu_points: Vec<GpuPoint> = gpus_raw
            .iter()
            .map(|g| GpuPoint {
                name: g.name.clone(),
                vram_mb: g.vram_mb,
                usage_pct: g.usage_pct,
                temp_c: g.temp_c,
                source: g.source.clone(),
            })
            .collect();

        let fsot = FsotPoint {
            delta_psi: route.delta_psi,
            delta_theta: route.delta_theta,
            recent_hits: route.recent_hits,
            rho: route.rho,
            load: route.load,
            scalar_s: parts.s,
            t1: parts.t1,
            t2: parts.t2,
            t3: parts.t3,
            s_ref: self.s_ref,
            trit_cpu,
            trit_mem,
            trit_gpu,
            trit_temp,
            trit_s,
        };

        let delta = DeltaPoint {
            cpu_pct_points: cpu_pct - self.v0.cpu_usage,
            mem_gb: mem_used - self.v0.mem_used_gb,
            temp_c: temp_delta,
            load: route.load - route.load_v0,
        };

        let sample = Sample {
            timestamp: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            cpu_pct,
            cpu_per_core,
            mem_used_gb: mem_used,
            mem_total_gb: mem_total,
            mem_frac,
            gpu_pct,
            gpu_name: gpu0.map(|g| g.name.clone()).unwrap_or_else(|| "GPU".into()),
            gpu_vram_mb: gpu0.map(|g| g.vram_mb).unwrap_or(0),
            gpu_temp: gpu0.and_then(|g| g.temp_c),
            gpu_source: gpu0
                .map(|g| g.source.clone())
                .unwrap_or_else(|| "none".into()),
            temp_c: temp_now,
            temps: temps.clone(),
            gpus: gpu_points.clone(),
            fsot: fsot.clone(),
            delta: delta.clone(),
        };

        let row = TelemetryRow {
            timestamp: sample.timestamp.clone(),
            pin_tag: PIN_TAG.into(),
            fold_id: FOLD_ID.into(),
            bridge_version: BRIDGE_VERSION.into(),
            hardware: HardwarePoint {
                cpu_usage_pct: cpu_pct,
                cpu_per_core: sample.cpu_per_core.clone(),
                mem_used_gb: mem_used,
                mem_total_gb: mem_total,
                mem_frac,
                temperatures: temps,
                gpus: gpu_points,
            },
            route: RoutePoint {
                d_eff: route.d_eff,
                n: route.n,
                p: route.p,
                recent_hits: route.recent_hits,
                delta_psi: route.delta_psi,
                delta_theta: route.delta_theta,
                rho: route.rho,
                load: route.load,
                load_v0: route.load_v0,
                frac_cpu: route.frac_cpu,
                frac_mem: route.frac_mem,
                frac_gpu: route.frac_gpu,
                frac_thermal: route.frac_thermal,
                observed: route.observed,
            },
            fsot,
            delta_from_v0: delta,
        };

        if let Some(ref mut f) = self.log {
            if let Ok(line) = serde_json::to_string(&row) {
                let _ = writeln!(f, "{line}");
                let _ = f.flush();
            }
        }

        self.stream.send_sample(&sample);
        self.history.push(&sample);
        self.latest = Some(sample);
    }
}

fn bytes_to_gb(bytes: u64) -> f64 {
    bytes as f64 / 1_073_741_824.0
}

fn sanitize_label(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .take(32)
        .collect()
}

fn open_log(path: &Path) -> std::io::Result<File> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }
    OpenOptions::new().create(true).append(true).open(path)
}
