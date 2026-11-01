#!/usr/bin/env python3
"""Numeric acceptance for a rendered demo GIF, measured from its own frames and events.json.

Checks, each printed with its measured value:
  * every change moment (agent op, click, ⌘Z, Home, MCP view change) changes >= 5% of the scene pixels
    (per-channel diff > 24) between the last frame before it and the largest diff among frames up to 0.2 s after it;
  * in every frame >= 40% of scene pixels are not near-black (max channel > 24);
  * no run of frames longer than 1.5 s without motion (the final hold counts only 1.5 s of its length), where a frame
    is "moving" when it differs from the previous frame on >= 0.5% of scene pixels;
  * file size <= --limit-mb.

Usage: check.py <events.json> <out.gif> [--limit-mb 3]
"""
import argparse
import json
import math
import os
import sys

import numpy as np
from PIL import Image, ImageSequence

CHANGE_DIFF = 24
CHANGE_MIN = 0.05
CHANGE_WINDOW_S = 0.2  # the editor acknowledges a discrete input within about 100 ms; frame quantisation adds up to 1/fps
SCENE_MIN = 0.40
STATIC_MAX_S = 1.5
FINAL_HOLD_S = 1.5  # render.py's default --hold: the only frozen tail the GIF may end with
MOTION_MIN = 0.005
CAPTION_BAR = 56
VIEWPORT_X = (470, 950)  # the 3D viewport's logical x range in the editor window (see storyboards' crop)


def frame_diff(a, b):
    """Fraction of scene pixels that differ by more than CHANGE_DIFF in any channel."""
    return float((np.abs(a - b).max(axis=2) > CHANGE_DIFF).mean())


def change_fraction(scenes, fps, t):
    """Largest scene change between the last frame before t and any frame up to CHANGE_WINDOW_S after t.

    The editor acknowledges a discrete input within about 100 ms, and each frame is quantised to 1/fps, so the change
    may land a frame or two after the aligned event frame. The window covers that; the threshold does not move.
    """
    n = len(scenes)
    before = min(n - 1, max(0, math.ceil(t * fps - 1e-9) - 1))
    after = [i for i in range(before + 1, n) if i / fps <= t + CHANGE_WINDOW_S + 1e-9]
    if not after:
        after = [min(n - 1, before + 1)]
    return max(frame_diff(scenes[before], scenes[i]) for i in after)


UNDO_MODS = ("SuperLeft", "SuperRight", "ControlLeft", "ControlRight")


def change_moments(rec):
    """(kind, t) for agent ops, MCP view changes, clicks, ⌘Z and Home, from the recorded chunks and events."""
    moments = []
    for c in rec.get("chunks", []):
        if c.get("operation_result") is not None:
            moments.append(("agent", c["start"] + (c.get("operation_applied_t_s") or 0.0)))
    cursors = [e["t"] for e in rec["events"] if e["type"] == "cursor"]
    for e in rec["events"]:
        if e["type"] == "click" and VIEWPORT_X[0] <= e["x"] <= VIEWPORT_X[1]:
            # a press that starts a drag (snip box) is followed at once by cursor moves: not a placement
            if not any(0 < t - e["t"] <= 0.1 for t in cursors):
                moments.append(("place", e["t"]))
        elif e["type"] == "key":
            keys = e.get("keys", [])
            # the person's undo: Cmd+Z on a Mac, Ctrl+Z on Linux and Windows (keys.rs `MOD_KEYS`)
            if "KeyZ" in keys and any(m in keys for m in UNDO_MODS):
                moments.append(("undo", e["t"]))
            elif "Home" in keys:
                moments.append(("home", e["t"]))
        elif e["type"] == "agent" and e.get("tool") == "grimoire_frame_view":
            moments.append(("view", e["t"]))
    return sorted(moments, key=lambda m: m[1])


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("events")
    ap.add_argument("gif")
    ap.add_argument("--limit-mb", type=float, default=3.0)
    args = ap.parse_args()

    with open(args.events) as fh:
        rec = json.load(fh)
    fps = rec["fps"]
    frames = [np.asarray(f.convert("RGB"), dtype=np.int16)
              for f in ImageSequence.Iterator(Image.open(args.gif))]
    n = len(frames)
    h = frames[0].shape[0]
    scene_h = h - CAPTION_BAR
    scenes = [f[:scene_h] for f in frames]
    ok = True

    # 1. Scene fraction in every frame.
    worst_scene = min(float((s.max(axis=2) > 24).mean()) for s in scenes)
    scene_ok = worst_scene >= SCENE_MIN
    ok &= scene_ok
    print(f"scene fraction (min over frames): {worst_scene * 100:.1f}% "
          f"(need >= {SCENE_MIN * 100:.0f}%) {'PASS' if scene_ok else 'FAIL'}")

    # 2. Change moments: the last frame before the moment vs the largest change up to CHANGE_WINDOW_S after it.
    for kind, t in change_moments(rec):
        frac = change_fraction(scenes, fps, t)
        passed = frac >= CHANGE_MIN
        ok &= passed
        print(f"change {kind:6s} at {t:6.2f}s: {frac * 100:5.2f}% of scene pixels changed "
              f"(need >= {CHANGE_MIN * 100:.0f}%) {'PASS' if passed else 'FAIL'}")

    # 3. Static runs, with the final hold exempt only up to FINAL_HOLD_S.
    moving = [False] + [frame_diff(scenes[i - 1], scenes[i]) >= MOTION_MIN for i in range(1, n)]
    last_move = 0
    worst = 0.0
    for i in range(1, n):
        if moving[i]:
            worst = max(worst, (i - last_move) / fps)
            last_move = i
    tail = (n - 1 - last_move) / fps
    # The final hold is exempt only up to FINAL_HOLD_S; a longer frozen tail is a static run too.
    worst = max(worst, tail - FINAL_HOLD_S)
    static_ok = worst <= STATIC_MAX_S
    ok &= static_ok
    print(f"longest static run before the final hold: {worst:.2f}s "
          f"(need <= {STATIC_MAX_S}s) {'PASS' if static_ok else 'FAIL'}; final hold {tail:.2f}s")

    # 4. Size.
    mb = os.path.getsize(args.gif) / 1e6
    size_ok = mb <= args.limit_mb
    ok &= size_ok
    print(f"size: {mb:.2f} MB (limit {args.limit_mb} MB) {'PASS' if size_ok else 'FAIL'}")

    print("RESULT:", "PASS" if ok else "FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
