#!/usr/bin/env python3
"""Launch the built game and check it actually runs.

    python3 .github/scripts/smoke.py path/to/fablegame [out_dir]

Two scripted screenshots, each in a fresh process: the start menu, and a
flight scene (the C172 in the air, chase camera). Each run must exit by
itself with status 0, without a panic, and leave a PNG that is not a blank
or single-colour frame. It runs offline, muted, with the MCP server off and
at low graphics, so it needs no network, no audio device and only a software
GPU (lavapipe on Linux, WARP on Windows, the runner's Metal on macOS).

Standard library only, so it runs on every CI runner as it is.
"""

import os
import struct
import subprocess
import sys
import zlib

# Per run. Software GPUs compile the big shaders on the CPU the first time
# they draw with them, which takes minutes.
TIMEOUT_S = int(os.environ.get("SMOKE_TIMEOUT", "1500"))

COMMON = [
    "--offline", "--mute", "--no-mcp", "--headless",
    "--graphics", "low", "--width", "640", "--height", "360",
    "--lang", "en",
]

# name, extra arguments (the shot is added below), seconds before the shot
RUNS = [
    ("menu", ["--menu", "--menu-screen", "main"], 4),
    ("scene", ["--no-menu", "--aircraft", "c172", "--lat", "37.6", "--lon", "-122.37",
               "--alt", "600", "--time", "12", "--clouds", "0.3"], 6),
]


def read_png_rgb(path):
    """Decode an 8-bit, non-interlaced RGB or RGBA PNG to (w, h, channels, rows)."""
    with open(path, "rb") as f:
        data = f.read()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("not a PNG")
    pos, idat, w = 8, b"", None
    while pos < len(data):
        n, kind = struct.unpack(">I4s", data[pos:pos + 8])
        body = data[pos + 8:pos + 8 + n]
        pos += 12 + n
        if kind == b"IHDR":
            w, h, depth, ctype, _, _, interlace = struct.unpack(">IIBBBBB", body)
            if depth != 8 or ctype not in (2, 6) or interlace:
                raise ValueError(f"unexpected PNG format (depth {depth}, colour type {ctype}, interlace {interlace})")
            ch = 3 if ctype == 2 else 4
        elif kind == b"IDAT":
            idat += body
        elif kind == b"IEND":
            break
    if w is None:
        raise ValueError("no IHDR")
    raw = zlib.decompress(idat)
    stride = w * ch
    rows, prev = [], bytearray(stride)
    for y in range(h):
        ft = raw[y * (stride + 1)]
        line = bytearray(raw[y * (stride + 1) + 1:(y + 1) * (stride + 1)])
        for i in range(stride):
            a = line[i - ch] if i >= ch else 0
            b = prev[i]
            c = prev[i - ch] if i >= ch else 0
            if ft == 1:
                line[i] = (line[i] + a) & 255
            elif ft == 2:
                line[i] = (line[i] + b) & 255
            elif ft == 3:
                line[i] = (line[i] + (a + b) // 2) & 255
            elif ft == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        rows.append(line)
        prev = line
    return w, h, ch, rows


def image_problems(path):
    """Why the frame looks broken, or an empty list."""
    w, h, ch, rows = read_png_rgb(path)
    colours, lum = set(), []
    for y in range(0, h, 4):
        r = rows[y]
        for x in range(0, w, 4):
            p = r[x * ch:x * ch + 3]
            colours.add(bytes(p))
            lum.append(0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2])
    mean = sum(lum) / len(lum)
    sd = (sum((v - mean) ** 2 for v in lum) / len(lum)) ** 0.5
    print(f"    {w}x{h}, {len(colours)} distinct colours (sampled), luminance mean {mean:.0f} sd {sd:.1f}")
    bad = []
    if len(colours) < 64:
        bad.append(f"only {len(colours)} distinct colours: a blank or flat frame")
    if sd < 4.0:
        bad.append(f"luminance spread {sd:.1f}: nothing drawn")
    if mean < 3 or mean > 252:
        bad.append(f"mean luminance {mean:.0f}: all black or all white")
    return bad


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        return 2
    exe = os.path.abspath(sys.argv[1])
    out = os.path.abspath(sys.argv[2] if len(sys.argv) > 2 else "smoke-shots")
    os.makedirs(out, exist_ok=True)
    # FABLE_NO_PAD: no gamepads in scripted runs (main.rs, open_gamepads),
    # so a pad plugged into a developer's machine can't steer the shot.
    env = dict(os.environ, RUST_BACKTRACE="1", FABLE_NO_PAD="1", RUST_LOG=os.environ.get("RUST_LOG", "warn,fablegame=info"))
    # SMOKE_RUNS=menu picks runs by name (Linux CI: lavapipe needs many
    # minutes per frame for the flight scene's shaders, so it checks the menu)
    only = [n for n in os.environ.get("SMOKE_RUNS", "").split(",") if n]
    failed = []
    for name, extra, at in RUNS:
        if only and name not in only:
            print(f"== {name}: skipped (SMOKE_RUNS={','.join(only)})")
            continue
        shot = os.path.join(out, f"{name}.png")
        if os.path.exists(shot):
            os.remove(shot)
        cmd = [exe] + COMMON + extra + ["--shot", f"{shot}@{at}"]
        print(f"== {name}: {' '.join(cmd)}", flush=True)
        log_path = os.path.join(out, f"{name}.log")
        try:
            with open(log_path, "w", encoding="utf-8", errors="replace") as log:
                code = subprocess.run(cmd, stdout=log, stderr=subprocess.STDOUT, timeout=TIMEOUT_S, env=env).returncode
        except subprocess.TimeoutExpired:
            code = "timeout"
        with open(log_path, encoding="utf-8", errors="replace") as f:
            log_text = f.read()
        problems = []
        if code != 0:
            problems.append(f"exit status {code}")
        if "panicked" in log_text:
            problems.append("panicked")
        if not os.path.exists(shot):
            problems.append("no screenshot written")
        else:
            try:
                problems += image_problems(shot)
            except Exception as e:  # a truncated or odd PNG
                problems.append(f"screenshot unreadable: {e}")
        if problems:
            print(f"   FAIL: {'; '.join(problems)}\n--- log (last 80 lines) ---")
            print("\n".join(log_text.splitlines()[-80:]))
            print("---")
            failed.append(name)
        else:
            print("   ok")
    if failed:
        print(f"smoke test failed: {', '.join(failed)}")
        return 1
    print("smoke test passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
