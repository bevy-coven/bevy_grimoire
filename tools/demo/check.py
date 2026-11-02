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


def change_fraction(scenes, fps, t, span=0.0):
    """Largest scene change between the last frame before t and any frame up to CHANGE_WINDOW_S after t.

    The editor acknowledges a discrete input within about 100 ms, and each frame is quantised to 1/fps, so the change
    may land a frame or two after the aligned event frame. The window covers that; the threshold does not move.
    """
    n = len(scenes)
    before = min(n - 1, max(0, math.ceil(t * fps - 1e-9) - 1))
    window = CHANGE_WINDOW_S + span
    after = [i for i in range(before + 1, n) if i / fps <= t + window + 1e-9]
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
            # **A press that starts a drag is followed by cursor MOVES; a click is not** (#270).
            #
            # `record.py`'s click step moves the cursor to the point and *then* presses, so the click
            # carries a cursor at its own timestamp — and the rule used to accept `0 <= t - e["t"]`,
            # which that self-event always satisfies. No click could ever be a change moment, so a
            # storyboard that demonstrates a widget (the journal's Check view, a palette row) could
            # never pass this check; measured on `check_view.json`, `change_moments` returned `[]`
            # for a click at 1.46 s with cursors [1.084, 1.461].
            #
            # A `drag` step is a different shape: the click is stamped first and the eight moves
            # follow it (`do_step`). So the test is strictly AFTER the press, and a drag's own
            # post-press moves still disqualify it — including one in the same frame's later tick.
            if not any(1e-6 < t - e["t"] <= 0.1 for t in cursors):
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
        elif e["type"] == "settle":
            # **A change the storyboard said would arrive, and has.** A fade is not a step: the picture
            # at the operation's timestamp is still the old one, so the fifth-of-a-second window above
            # measures the wrong pair of frames.
            #
            # **A gradual change needs a span, not a moment.** The editor now walks a clock edit over a
            # second (#271), so the sky goes from noon to dusk as a *gradient*: any fifth of a second of
            # it moves a tenth of the picture, which no 5 % threshold can see. A settle that names the
            # moment the change *starts* carries the seconds it takes as `span`, and the comparison is
            # against its end.
            span = float(e.get("span", 0.0) or 0.0)
            # The moment stays at the change's **start**: `change_fraction` compares the frame before it
            # with the frames after, and `span` widens that second window.
            moments.append(("settle", e["t"], span) if span > 0 else ("settle", e["t"]))
    settled = [m for m in moments if m[0] == "settle"]
    # **A settle stands in for the operation it answers.** An `agent` moment is the operation's own
    # timestamp, and for a change the editor fades in that picture is still the old one — measuring it
    # says "the operation changed nothing", which is false. A settle within a few seconds after an
    # agent moment is that moment's picture arrived, so the agent moment is dropped in its favour;
    # an agent moment with no settle after it is measured as before.
    if settled:
        moments = [m for m in moments
                   if m[0] != "agent" or not any(0 < s[1] - m[1] <= 3.0 for s in settled)]
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
    for moment in change_moments(rec):
        kind, t = moment[0], moment[1]
        span = moment[2] if len(moment) > 2 else 0.0
        frac = change_fraction(scenes, fps, t, span)
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
