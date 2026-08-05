# Plant interoception tools

## Listen while the monitor streams

Terminal A:

```bat
fsot-pc-monitor.exe --stream
```

Terminal B:

```bat
python tools\plant_listen.py
```

Outputs (refreshed each packet):

| File | Purpose |
|------|---------|
| `plant_latest.json` | Full FSOT plant packet |
| `plant_metric.fsmt` | Binary FSMT (neuron-zig `metric_inject.zig`) |
| `plant_metric.txt` | `sys_metric` line for `inject_io` / host bus |

## Neuron-zig hook (same pin, plant channels)

`metric_inject.zig` expects:

```
magic "FSMT" | ver=1 | n_channels | reserved | n × f32 LE
channels: cpu, mem, disk, net, temp   (each in [0,1])
```

The PC monitor sends that on UDP **9876** whenever **Neuron stream** is on (GUI toggle or `--stream`).

Wire path ideas:

1. Run `plant_listen.py` → copy `plant_metric.fsmt` or `plant_metric.txt` into the neuron host inject path you already use.
2. Or extend neuron host loop to `recvfrom(9876)` and call `metric_inject.readChannels`.

## Protocols

From repo root / `protocols\`:

| Script | What |
|--------|------|
| `idle_10min.bat` | 10 min quiet → export |
| `step_load.bat` | idle → CPU step load → export |
| `work_day.bat` | 8 h log + stream → export |
