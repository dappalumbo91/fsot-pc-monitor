//! Interoception stream for neuron-zig / other plant consumers.
//!
//! Two UDP payloads on 127.0.0.1:9876 when enabled:
//! 1. JSON line (rich plant + FSOT fields) — easy for scripts
//! 2. Binary FSMT frame (metric_inject.zig ABI) — cpu, mem, disk, net, temp

use crate::fold::{FOLD_ID, INTEROCEPTION_PORT, PIN_TAG};
use crate::monitor::Sample;
use serde::Serialize;
use std::net::UdpSocket;

#[derive(Debug, Serialize)]
struct InteroceptionPacket<'a> {
    pin_tag: &'a str,
    fold_id: &'a str,
    timestamp: &'a str,
    cpu_frac: f64,
    mem_frac: f64,
    gpu_frac: f64,
    temp_c: Option<f64>,
    /// Normalized temp activity [0,1] when sensor present (ΔT/25 style already in load).
    temp_frac: f64,
    load: f64,
    delta_psi: f64,
    delta_theta: f64,
    recent_hits: f64,
    rho: f64,
    scalar_s: f64,
    t1: f64,
    t2: f64,
    t3: f64,
    trit_cpu: i8,
    trit_mem: i8,
    trit_gpu: i8,
    trit_temp: i8,
    trit_s: i8,
}

pub struct InteroceptionStream {
    sock: Option<UdpSocket>,
    enabled: bool,
}

impl InteroceptionStream {
    pub fn new(enabled: bool) -> Self {
        let sock = if enabled { bind_sock() } else { None };
        Self { sock, enabled }
    }

    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
        if on && self.sock.is_none() {
            self.sock = bind_sock();
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn send_sample(&self, sample: &Sample) {
        if !self.enabled {
            return;
        }
        let Some(ref sock) = self.sock else { return };
        let addr = format!("127.0.0.1:{INTEROCEPTION_PORT}");

        let cpu = (sample.cpu_pct as f64 / 100.0).clamp(0.0, 1.0);
        let mem = sample.mem_frac.clamp(0.0, 1.0);
        let gpu = (sample.gpu_pct as f64 / 100.0).clamp(0.0, 1.0);
        // Map temp °C into a gentle [0,1] for MetricF.temp (0 at 20°C, 1 at 100°C)
        let temp_frac = sample
            .temp_c
            .map(|t| ((t - 20.0) / 80.0).clamp(0.0, 1.0))
            .unwrap_or(0.0);

        let pkt = InteroceptionPacket {
            pin_tag: PIN_TAG,
            fold_id: FOLD_ID,
            timestamp: &sample.timestamp,
            cpu_frac: cpu,
            mem_frac: mem,
            gpu_frac: gpu,
            temp_c: sample.temp_c,
            temp_frac,
            load: sample.fsot.load,
            delta_psi: sample.fsot.delta_psi,
            delta_theta: sample.fsot.delta_theta,
            recent_hits: sample.fsot.recent_hits,
            rho: sample.fsot.rho,
            scalar_s: sample.fsot.scalar_s,
            t1: sample.fsot.t1,
            t2: sample.fsot.t2,
            t3: sample.fsot.t3,
            trit_cpu: sample.fsot.trit_cpu,
            trit_mem: sample.fsot.trit_mem,
            trit_gpu: sample.fsot.trit_gpu,
            trit_temp: sample.fsot.trit_temp,
            trit_s: sample.fsot.trit_s,
        };
        if let Ok(bytes) = serde_json::to_vec(&pkt) {
            let _ = sock.send_to(&bytes, &addr);
        }

        // FSMT binary (neuron metric_inject.zig): cpu, mem, disk=0, net=gpu proxy, temp
        let fsmt = pack_fsmt(&[
            cpu as f32,
            mem as f32,
            0.0_f32, // disk (not sampled yet)
            gpu as f32,
            temp_frac as f32,
        ]);
        let _ = sock.send_to(&fsmt, &addr);
    }
}

fn bind_sock() -> Option<UdpSocket> {
    UdpSocket::bind("127.0.0.1:0").ok().map(|s| {
        let _ = s.set_nonblocking(true);
        s
    })
}

/// metric_inject.zig layout: magic FSMT, ver=1, n, reserved, n×f32 LE.
fn pack_fsmt(channels: &[f32]) -> Vec<u8> {
    let n = channels.len().min(255) as u8;
    let mut buf = Vec::with_capacity(8 + channels.len() * 4);
    buf.extend_from_slice(b"FSMT");
    buf.push(1); // version
    buf.push(n);
    buf.push(0);
    buf.push(0);
    for &c in channels.iter().take(n as usize) {
        buf.extend_from_slice(&c.to_le_bytes());
    }
    buf
}
