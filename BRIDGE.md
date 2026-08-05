# FSOT PC Monitor — Bridge & Fold Identity

| | |
|--|--|
| **Fold ID** | `plant_host_v1` |
| **Bridge version** | `1.0.0` |
| **Instrument** | `0.4.0` |
| **Pin** | `D1D38A…` |
| **Law** | \(S = K(T_1+T_2+T_3)\) clamped \([-3,+3]\) |

**One law, many folds.** Same seed-derived scalar spine as neuron-zig, Lean, market fold, etc. This instrument is the **PC host plant**: live sensors → preregistered bridge → \(S\).

No free least-squares parameters on the law path. Bridge weights are preregistered (not fitted per session).

---

## Plant fold constants

| Symbol | Value | Origin |
|--------|--------|--------|
| \(D_\mathrm{eff}\) | \(\varphi^2 \pi\) | Seed-derived plant scale |
| \(N\) | \(4\) | Channels: CPU, mem, GPU, thermal |
| \(P\) | \(\varphi\) | Seed-derived pathway proxy |
| Channel weight | \(1/N = 1/4\) | Equal-weight plant mix |
| Temp phase scale | \(25^\circ\mathrm{C}\) | Scalar corpus \(D/25\) convention |
| Trit frac cuts | \(1/3,\ 2/3\) | Trinary ontology |
| Temp trit band | \(\pm e\ ^\circ\mathrm{C}\) | Seed \(e\) |
| \(S\) trit band | \(\pm K\) on \((S - S_\mathrm{ref})\) | Seed \(K\) |
| Excursion band | \(K\) | \(\lvert S - S_\mathrm{ref}\rvert > K\) |

Seeds match **fsot-neuron-zig** / pin **D1D38A**.

---

## Bridge (hardware → route)

\[
f_c=\mathrm{clamp}(\mathrm{cpu\%}/100),\quad
f_m=\mathrm{mem\_used}/\mathrm{mem\_total},\quad
f_g=\mathrm{clamp}(\mathrm{gpu\%}/100)
\]

\[
f_t =
\begin{cases}
\mathrm{clamp}(\lvert T - T_{V_0}\rvert / 25,\ 0,\ 1) & \text{temp present}\\
0 & \text{else}
\end{cases}
\]

\[
L = \tfrac14(f_c+f_m+f_g+f_t)
\]

\begin{align*}
\delta\psi &= \mathrm{clamp}(L - L_{V_0},\,-1,\,1)\\
\delta\theta &= \mathrm{clamp}((T-T_{V_0})/25,\,-1,\,1)\ \text{or }0\\
\mathrm{recent\_hits} &= \mathrm{clamp}(L\cdot N,\,0,\,N)\\
\rho &= f_m\\
\mathrm{scale}=1,\ \mathrm{amplitude}=1,\ \mathrm{trend\_bias}=0,\ \mathrm{observed}=\mathrm{true}
\end{align*}

Full `computeScalar` (Zig parity).  
\(S_\mathrm{ref} = S(\mathrm{hits}{=}0,\delta\psi{=}0,\delta\theta{=}0,\rho{=}f_m^{V_0})\).

---

## Session science

| Metric | Definition |
|--------|------------|
| Noise floor \(\sigma\) | Std of \(S\) over first 30 idle samples (\(L < 0.35\)) |
| Excursion | \(\lvert S-S_\mathrm{ref}\rvert > K\); rate per hour |
| Recovery \(\tau\) | Mean excursion duration until return |
| Trit dwell | Fraction of samples with trit\((S-S_\mathrm{ref})\in\{-1,0,+1\}\) |
| Correlations | Pearson \(r\) cpu/gpu/temp vs \(S\); best lag cpu→S |

---

## Export

`Export run` → `exports/session_YYYYMMDD_HHMMSS/`:

- `certificate.json` — pin, fold, bridge, host, science, log SHA-256  
- `telemetry.jsonl` — full stream  
- `science.csv` — flat metrics  

---

## Neuron interoception

UDP `127.0.0.1:9876` when stream enabled:

1. JSON plant packet  
2. Binary **FSMT** (`metric_inject.zig`: cpu, mem, disk, net, temp as f32)

Listener: `python tools/plant_listen.py`  
Protocols: `protocols\idle_10min.bat`, `step_load.bat`, `work_day.bat`

---

## Change control

Bump `BRIDGE_VERSION` when equations or preregistered constants change. Do not fit channel weights per machine.
