//! FSOT PC Monitor — host-plant instrument + performance HUD.
//!
//! ```text
//! fsot-pc-monitor.exe
//! fsot-pc-monitor.exe --stream
//! fsot-pc-monitor.exe --headless --duration 600 --export
//! fsot-pc-monitor.exe --protocol idle|step|work
//! ```

mod seeds;
mod fold;
mod bridge;
mod trit;
mod scalar;
mod thermal;
mod gpu;
mod science;
mod certificate;
mod stream;
mod monitor;
mod ui;

use monitor::Engine;
use std::env;
use std::io::{self, Write};
use std::time::{Duration, Instant};

fn main() -> eframe::Result<()> {
    let args: Vec<String> = env::args().collect();
    let headless = flag(&args, "--headless") || flag(&args, "-c");
    let stream = flag(&args, "--stream") || flag(&args, "-s");
    let export = flag(&args, "--export") || flag(&args, "-e");
    let duration = arg_u64(&args, "--duration").or_else(|| {
        // protocols set a default duration
        match arg_str(&args, "--protocol").as_deref() {
            Some("idle") => Some(600),   // 10 min
            Some("step") => Some(300),   // 5 min (includes load window)
            Some("work") => Some(28800), // 8 h work-day
            _ => None,
        }
    });
    let protocol = arg_str(&args, "--protocol");

    if headless || protocol.is_some() {
        run_headless(stream, export || protocol.is_some(), duration, protocol.as_deref());
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 780.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("FSOT PC Monitor — plant_host_v1"),
        ..Default::default()
    };

    eframe::run_native(
        "FSOT PC Monitor",
        options,
        Box::new(move |cc| {
            let mut app = ui::MonitorApp::new(cc);
            if stream {
                app.engine.stream.set_enabled(true);
            }
            Ok(Box::new(app))
        }),
    )
}

fn run_headless(
    stream: bool,
    export_at_end: bool,
    duration_s: Option<u64>,
    protocol: Option<&str>,
) {
    println!("FSOT PC Monitor v{}", fold::INSTRUMENT_VERSION);
    println!(
        "pin {}  fold {}  bridge {}",
        fold::PIN_TAG,
        fold::FOLD_ID,
        fold::BRIDGE_VERSION
    );
    if let Some(p) = protocol {
        println!("protocol: {p}");
    }
    if let Some(d) = duration_s {
        println!("duration: {d}s");
    }

    let mut engine = Engine::new_with_stream(stream);
    engine.sample_now();
    println!(
        "[V0] CPU {:.1}%  RAM {:.2}/{:.2} GB  temp {:?}  S_ref {:+.4}",
        engine.v0.cpu_usage,
        engine.v0.mem_used_gb,
        engine.v0.mem_total_gb,
        engine.v0.temp_c,
        engine.s_ref
    );
    println!("log → logs/fsot_telemetry.log");
    if stream {
        println!(
            "interoception UDP → 127.0.0.1:{} (JSON + FSMT)",
            fold::INTEROCEPTION_PORT
        );
    }

    // step protocol: spawn a short CPU load mid-run if possible
    let step_load_at = if protocol == Some("step") {
        Some(Duration::from_secs(60))
    } else {
        None
    };
    let mut load_started = false;
    let mut load_child: Option<std::process::Child> = None;

    let start = Instant::now();
    let limit = duration_s.map(Duration::from_secs);

    loop {
        if engine.maybe_sample() {
            if let Some(s) = engine.latest.as_ref() {
                print!(
                    "\r[plant] CPU {:>5.1}%  GPU {:>4.0}%  load {:.3}  S {:>+6.3}  δψ {:>+6.3}  n={}   ",
                    s.cpu_pct,
                    s.gpu_pct,
                    s.fsot.load,
                    s.fsot.scalar_s,
                    s.fsot.delta_psi,
                    engine.science().samples,
                );
                let _ = io::stdout().flush();
            }
        }

        if let Some(at) = step_load_at {
            if !load_started && start.elapsed() >= at {
                load_started = true;
                println!("\n[protocol step] starting ~90s CPU load…");
                // Portable-ish load: PowerShell busy loop on many cores
                load_child = std::process::Command::new("powershell")
                    .args([
                        "-NoProfile",
                        "-Command",
                        "$jobs = 1..([Math]::Max(2,[Environment]::ProcessorCount)) | ForEach-Object { \
                         Start-Job { $e = [datetime]::UtcNow.AddSeconds(90); while([datetime]::UtcNow -lt $e){ $x=0; for($i=0;$i -lt 50000;$i++){ $x += [math]::Sqrt($i) } } }; \
                         $jobs | Wait-Job | Out-Null; $jobs | Remove-Job",
                    ])
                    .spawn()
                    .ok();
            }
        }

        if let Some(lim) = limit {
            if start.elapsed() >= lim {
                println!("\n[done] duration reached.");
                break;
            }
        }

        std::thread::sleep(Duration::from_millis(100));
    }

    if let Some(mut c) = load_child {
        let _ = c.kill();
    }

    let sci = engine.science();
    println!(
        "session: {} samples · σ_idle={} · excursions={} · τ_rec={}",
        sci.samples,
        sci.noise_floor_s
            .map(|v| format!("{v:.5}"))
            .unwrap_or_else(|| "n/a".into()),
        sci.excursion_count,
        sci.recovery_tau_s
            .map(|v| format!("{v:.1}s"))
            .unwrap_or_else(|| "n/a".into()),
    );

    if export_at_end {
        match engine.export_run() {
            Ok(dir) => println!("exported → {}", dir.display()),
            Err(e) => eprintln!("export failed: {e}"),
        }
    }
}

fn flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn arg_str(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == name)
        .map(|w| w[1].clone())
}

fn arg_u64(args: &[String], name: &str) -> Option<u64> {
    arg_str(args, name)?.parse().ok()
}
