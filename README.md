# FSOT PC Monitor

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)

Build pin **D1D38A** is retained for this build; current theory authority is [FSOT-2.1-Lean](https://github.com/dappalumbo91/FSOT-2.1-Lean) pin **AEB2AD** (`vendor/fsot_compute.py`).

PC host-plant instrument under pin **D1D38A**, fold **`plant_host_v1`**, with a performance HUD.

**One law, many folds** — same FSOT scalar spine as neuron-zig and the rest of your stack; this fold is your machine’s live sensors.

| | |
|--|--|
| **License** | [Apache-2.0](LICENSE) |
| **Author** | Damian Arthur Palumbo |
| **Pin** | D1D38A |
| **Fold** | `plant_host_v1` |

Full mapping: **[BRIDGE.md](BRIDGE.md)**.

## Identity

| Field | Value |
|-------|--------|
| Pin | `D1D38A…` |
| Fold | `plant_host_v1` |
| Bridge | `1.0.0` |
| Law | \(S=K(T_1+T_2+T_3)\) |
| Free LSQ params | **0** |

## Features

- CPU (per-core), RAM, GPU (`nvidia-smi`), temperature  
- Preregistered equal-weight 4-channel bridge → \((\delta\psi,\delta\theta,\mathrm{hits},\rho)\)  
- Scalar + trits; science metrics (noise, excursions, recovery, correlations)  
- Session certificate + **Export run**  
- **Neuron stream** (UDP JSON + FSMT binary on port 9876)  
- Graphs, compact mode, always-on-top, focus tabs  

## Run

```bat
cargo build --release
copy /Y target\release\fsot-pc-monitor.exe .
start_monitor.bat
```

```bat
fsot-pc-monitor.exe --stream
fsot-pc-monitor.exe --headless --duration 120 --export
```

## Protocols

| Script | Purpose |
|--------|---------|
| `protocols\idle_10min.bat` | 10 min idle noise floor → export |
| `protocols\step_load.bat` | Step CPU load → excursion / recovery |
| `protocols\work_day.bat` | 8 h work session + stream → export |

## Neuron plant feed

```bat
REM terminal A
fsot-pc-monitor.exe --stream

REM terminal B
python tools\plant_listen.py
```

See `tools\README.md`.

## Project layout

```
src/fold.rs bridge.rs scalar.rs science.rs certificate.rs stream.rs
protocols/   idle_10min.bat  step_load.bat  work_day.bat
tools/       plant_listen.py
BRIDGE.md
```
