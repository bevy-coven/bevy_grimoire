#!/usr/bin/env python3
"""Measure the editor's responsiveness on a real project, and write the numbers as JSON.

usage: perf.py --source <project assets dir> --out <json>

The loop skipped measurement repeatedly because the tool lived in `/tmp/grimoire_live/lat.py` and was
one reboot from gone. This is that tool, in the repo, with the same five numbers:

  frame_ms_median   the display's own frame rate, five 2 s windows
  key_p95_ms        how long the keyboard takes to change what the editor says is armed
  file_ms           how long a file edit takes to appear in the world
  mcp_ms            how long an agent's operation takes to appear in the world
  load_avg_1m       what the machine was doing, because every number above is about it

The editor is launched on a scratch copy under /tmp — never the source — and closed by its own pid.
Exit 1 if a latency came back None: a report with a hole in it is worse than a failed script, because
the hole gets quoted.
"""
import argparse
import json
import math
import os
import statistics
import sys
import time
import urllib.error
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from common import Client, Editor, binary, brp, make_scratch  # noqa: E402

# The map `--door map` opens first: the one the perf runs have always used. A placement in it whose
# world translation is stable, so the poll can find it without reading the whole map.
WATCHED_TRANSLATION = (-137.1617, -97.5372)
TOLERANCE_DEG = 0.05


def frame_ms(port, windows=5, seconds=2.0):
    """The display's own frame rate, median over windows.

    `bevy_debugger/status` carries the frame counter, so this measures what the editor actually
    drew rather than what it was asked to draw.
    """
    rates = []
    for _ in range(windows):
        first = brp(port, "bevy_debugger/status", {})["result"]["frame"]
        t0 = time.time()
        time.sleep(seconds)
        last = brp(port, "bevy_debugger/status", {})["result"]["frame"]
        elapsed = time.time() - t0
        rates.append((last - first) / elapsed)
    median = statistics.median(rates)
    return (1000.0 / median if median > 0 else None), rates


def key_ms(port, presses=10):
    """Time from a keystroke to the editor's state saying the armed tool changed.

    `KeyB` cycles the tool, and `grimoire/state` reports which tool is armed — so the stop condition
    is the editor's own answer, not a guess about a frame count.
    """
    times = []
    for _ in range(presses):
        before = brp(port, "grimoire/state", {})["result"]["editor"]["tool"]
        t0 = time.time()
        brp(port, "bevy_debugger/input", {"kind": "Keyboard", "key": "KeyB", "action": "Tap"})
        while time.time() - t0 < 3.0:
            now = brp(port, "grimoire/state", {})["result"]["editor"]["tool"]
            if now != before:
                times.append((time.time() - t0) * 1000.0)
                break
        else:
            times.append(None)
        time.sleep(0.15)
    good = [t for t in times if t is not None]
    if not good:
        return None, times
    good.sort()
    idx = min(len(good) - 1, int(round(0.95 * (len(good) - 1))))
    return good[idx], times


def poll_yaw(port, target_deg, translation=WATCHED_TRANSLATION, limit=10.0):
    """Wait until the watched transform's yaw is `target_deg`, and return the moment it is.

    Read out of `world.query`: the rotation comes back as a quaternion, and a yaw about Y is
    `2 * atan2(q.y, q.w)`. Every piece in these maps is upright, so a yaw is the whole story.
    """
    t0 = time.time()
    while time.time() - t0 < limit:
        try:
            answer = brp(port, "world.query", {
                "data": {
                    "components": [
                        "bevy_transform::components::transform::Transform",
                        "bevy_ecs::name::Name",
                    ],
                },
                "filter": {"with": ["bevy_transform::components::transform::Transform"]},
            })
        except (urllib.error.URLError, ConnectionError, OSError):
            time.sleep(0.05)
            continue
        for row in answer["result"]:
            # Each row is `{"entity": n, "components": {"<type path>": {...}}}`; the transform
            # carries `translation` as `[x, y, z]` and `rotation` as `[x, y, z, w]`.
            t = (row.get("components") or {}).get(
                "bevy_transform::components::transform::Transform")
            if not isinstance(t, dict):
                continue
            tr = t.get("translation")
            if not isinstance(tr, list) or len(tr) != 3:
                continue
            if abs(tr[0] - translation[0]) > 0.5 or abs(tr[2] - translation[1]) > 0.5:
                continue
            q = t.get("rotation")
            if not isinstance(q, list) or len(q) != 4:
                continue
            yaw = math.degrees(2.0 * math.atan2(q[1], q[3]))
            yaw = (yaw + 180.0) % 360.0 - 180.0
            if abs(yaw - target_deg) < TOLERANCE_DEG:
                return (time.time() - t0) * 1000.0
        time.sleep(0.02)
    return None


def rewrite_yaw(path, new_yaw):
    """Set the watched placement's `yaw:` to `new_yaw`, ending the replaced value at `,` or `)`.

    **Both terminators.** The loop's own tool ended at `,` only, and this map closes a placement with
    `)`, so the replacement matched nothing and `file_ms` came back None for an entire iteration —
    which was then reported as a latency regression that did not exist.
    """
    text = path.read_text()
    marker = text.find(f"({WATCHED_TRANSLATION[0]:.4f}")
    if marker < 0:
        # The translation's spelling in the file is the map's, not ours; fall back to the first
        # `yaw:` in the file, which is the same placement on the map these runs use.
        marker = 0
    start = text.find("yaw:", marker)
    if start < 0:
        raise RuntimeError(f"no yaw: in {path}")
    value_at = start + len("yaw:")
    end = value_at
    while end < len(text) and text[end] not in ",)":
        end += 1
    original = text
    replaced = text[:value_at] + f" {new_yaw}" + text[end:]
    path.write_text(replaced)
    return original


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--source", required=True, help="the project's assets directory (read only)")
    ap.add_argument("--out", required=True, help="where to write the JSON")
    ap.add_argument("--door", default="map")
    ap.add_argument("--map", default="town")
    args = ap.parse_args()

    stamp = int(time.time())
    scratch = make_scratch(Path(args.source), Path(f"/tmp/grimoire_perf/run-{stamp}"))
    report = {"source": str(args.source), "scratch": str(scratch), "load_avg_1m": None}
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)

    try:
        report["load_avg_1m"] = os.getloadavg()[0]
        with Editor(scratch, args.map, "--door", args.door) as editor:
            editor.wait_ready(lambda s: s.get("map", {}).get("name") == args.map)
            port = editor.port

            report["frame_ms_median"], report["frame_hz_windows"] = frame_ms(port)
            report["key_p95_ms"], report["key_ms_raw"] = key_ms(port)

            # **A file edit**, then the same value through MCP, with the original restored.
            map_path = Path(scratch) / "assets" / "grimoire" / "maps" / f"{args.map}.map.ron"
            original = rewrite_yaw(map_path, 90.0)
            try:
                report["file_ms"] = poll_yaw(port, 90.0)
            finally:
                map_path.write_text(original)

            client = Client(str(binary("bevy_cauldron_mcp")), str(scratch), port, "perf")
            try:
                t0 = time.time()
                answer = client.json_call("grimoire_apply_operation", {
                    "operation": "placement_set_yaw",
                    "parameters": {"id": "tower_00", "yaw": 135.0},
                })
                report["mcp_call_ms"] = (time.time() - t0) * 1000.0
                report["mcp_outcome"] = answer.get("outcome", answer)
                report["mcp_ms"] = poll_yaw(port, 135.0)
                if report["mcp_ms"] is not None:
                    client.json_call("grimoire_apply_operation", {
                        "operation": "placement_set_yaw",
                        "parameters": {"id": "tower_00", "yaw": 0.0},
                    })
            finally:
                client.close()
    finally:
        out.write_text(json.dumps(report, indent=2))

    print(json.dumps(report, indent=2))
    missing = [k for k in ("file_ms", "mcp_ms", "key_p95_ms") if report.get(k) is None]
    if missing:
        print(f"these latencies came back None: {', '.join(missing)}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
