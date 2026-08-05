#!/usr/bin/env python3
"""
Listen for FSOT PC Monitor interoception on UDP 9876.

Accepts:
  - JSON plant packets (utf-8)
  - Binary FSMT frames (neuron-zig metric_inject ABI)

Writes latest plant state for neuron / scripts:
  - tools/plant_latest.json
  - tools/plant_metric.fsmt   (binary FSMT)
  - tools/plant_metric.txt    (sys_metric inject line for inject_io)

Usage:
  python tools/plant_listen.py
  python tools/plant_listen.py --port 9876 --out-dir tools
"""

from __future__ import annotations

import argparse
import json
import socket
import struct
import sys
from pathlib import Path


def parse_fsmt(data: bytes) -> list[float] | None:
    if len(data) < 8 or data[0:4] != b"FSMT":
        return None
    n = data[5]
    need = 8 + n * 4
    if len(data) < need:
        return None
    out = []
    for i in range(n):
        (v,) = struct.unpack_from("<f", data, 8 + i * 4)
        out.append(float(v))
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description="FSOT plant interoception listener")
    ap.add_argument("--port", type=int, default=9876)
    ap.add_argument("--out-dir", type=Path, default=Path(__file__).resolve().parent)
    ap.add_argument("--bind", default="127.0.0.1")
    args = ap.parse_args()

    args.out_dir.mkdir(parents=True, exist_ok=True)
    json_path = args.out_dir / "plant_latest.json"
    fsmt_path = args.out_dir / "plant_metric.fsmt"
    txt_path = args.out_dir / "plant_metric.txt"

    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind((args.bind, args.port))
    print(f"listening UDP {args.bind}:{args.port}")
    print(f"writes → {json_path.name}, {fsmt_path.name}, {txt_path.name}")

    n_json = 0
    n_fsmt = 0
    while True:
        data, _addr = sock.recvfrom(65535)
        if data[:4] == b"FSMT":
            ch = parse_fsmt(data)
            if ch is None:
                continue
            n_fsmt += 1
            fsmt_path.write_bytes(data)
            # inject_io style: sys_metric strength feat...
            # channels: cpu mem disk net temp
            while len(ch) < 5:
                ch.append(0.0)
            line = (
                f"sys_metric 1.0 {ch[0]:.6f} {ch[1]:.6f} {ch[2]:.6f} "
                f"{ch[3]:.6f} {ch[4]:.6f}\n"
            )
            txt_path.write_text(line, encoding="utf-8")
            if n_fsmt % 10 == 1:
                print(f"FSMT #{n_fsmt} cpu={ch[0]:.3f} mem={ch[1]:.3f} temp={ch[4]:.3f}")
            continue

        # JSON
        try:
            text = data.decode("utf-8")
            obj = json.loads(text)
        except (UnicodeDecodeError, json.JSONDecodeError):
            continue
        n_json += 1
        json_path.write_text(json.dumps(obj, indent=2), encoding="utf-8")
        if n_json % 10 == 1:
            print(
                f"JSON #{n_json} load={obj.get('load', 0):.3f} "
                f"S={obj.get('scalar_s', 0):+.3f} "
                f"cpu={obj.get('cpu_frac', 0):.3f}"
            )


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        print("\nbye")
        raise SystemExit(0)
