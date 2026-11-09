#!/usr/bin/env python3
"""Render a record.py timeline (events.json) into a GIF: crop, cursor marker, keycaps, click pulses,
captions, an optional title card, and the last frame held. Needs Pillow and ffmpeg.

usage: render.py <events.json> --out <file.gif> [--hold 1.5] [--settle 0.5] [--colors 256] [--sheet <file.png>] [--stills t1,t2]

Frames are sampled on a clock at the storyboard's fps: each output tick shows the latest editor
frame taken at or before it, so every frame is shown for its real time and the GIF is uniform.
Overlays are drawn at each tick's time from the events. Stills, a contact sheet and a full-size frame
at a time let a person check the render before it is committed.
"""
import argparse
import bisect
import json
import os
import shutil
import subprocess
import sys
import tempfile

from PIL import Image, ImageDraw, ImageFont

FONT_PATHS = [
    "/System/Library/Fonts/Menlo.ttc",
    "/System/Library/Fonts/Supplemental/Courier New.ttf",
]
KEYCAP_SECONDS = 0.8
SETTLE_S = 0.5
CLICK_SECONDS = 0.4
CAPTION_BAR = 56
ACCENT = (255, 200, 60)
INK = (240, 240, 240)
BAR = (18, 18, 18)
KEYCAP_FILL = (28, 28, 28)

KEY_LABELS = {
    "SuperLeft": "⌘", "SuperRight": "⌘", "ShiftLeft": "⇧", "ShiftRight": "⇧",
    "AltLeft": "⌥", "AltRight": "⌥", "ControlLeft": "⌃", "ControlRight": "⌃",
    "Backslash": "\\", "Home": "Home", "End": "End", "Enter": "Enter", "Escape": "Esc",
    "Space": "Space", "Backspace": "Delete", "ArrowLeft": "←", "ArrowRight": "→",
    "ArrowUp": "↑", "ArrowDown": "↓",
}


def key_label(key):
    if key in KEY_LABELS:
        return KEY_LABELS[key]
    if key.startswith("Key"):
        return key[3:]
    if key.startswith("Digit"):
        return key[5:]
    return key


def keycap_text(keys):
    """Modifiers first, then the key, with no spaces: ["SuperLeft", "KeyZ"] -> "⌘Z"."""
    return "".join(key_label(k) for k in keys)


def load_font(size):
    for path in FONT_PATHS:
        if os.path.exists(path):
            try:
                return ImageFont.truetype(path, size, index=0)
            except OSError:
                continue
    return ImageFont.load_default(size=size)


def timeline_end(rec, settle=SETTLE_S):
    """Where the GIF's action ends: the last event's aligned time plus a settle, never past the
    last captured frame. Frames captured after this are overrun from a slow chunk, so they are dropped."""
    last_frame = rec["frames"][-1]["t"]
    if not rec["events"]:
        return last_frame
    last_event = max(e["t"] for e in rec["events"])
    return min(last_event + settle, last_frame)


def last_before(events, types, t):
    """The latest event of the given types at or before t, or None."""
    found = None
    for e in events:
        if e["t"] > t:
            break
        if e["type"] in types:
            found = e
    return found


class Renderer:
    def __init__(self, rec, width):
        self.rec = rec
        self.scale = rec.get("scale", 2)
        crop = rec.get("crop") or [0, 0, 1280, 703]
        self.x0, self.y0, self.x1, self.y1 = crop
        self.width = width
        self.kx = width / (self.x1 - self.x0)
        self.height = round((self.y1 - self.y0) * self.kx)
        self.font_keycap = load_font(26)
        self.font_caption = load_font(22)
        self.font_title = load_font(44)
        self.events = sorted(rec["events"], key=lambda e: e["t"])
        self.frame_times = [f["t"] for f in rec["frames"]]
        self.frames = rec["frames"]
        self._cache = (None, None)

    def source(self, t):
        i = bisect.bisect_right(self.frame_times, t + 1e-6) - 1
        return self.frames[max(i, 0)]["path"]

    def picture(self, path):
        if self._cache[0] != path:
            full = Image.open(path).convert("RGB")
            s = self.scale
            box = (self.x0 * s, self.y0 * s, self.x1 * s, self.y1 * s)
            img = full.crop(box).resize((self.width, self.height), Image.LANCZOS)
            self._cache = (path, img)
        return self._cache[1].copy()

    def to_out(self, x, y):
        return (x - self.x0) * self.kx, (y - self.y0) * self.kx

    def frame(self, t):
        img = self.picture(self.source(t))
        draw = ImageDraw.Draw(img)

        cursor = last_before(self.events, ("cursor", "clear_cursor"), t)
        if cursor and cursor["type"] == "cursor":
            cx, cy = self.to_out(cursor["x"], cursor["y"])
            draw.ellipse((cx - 11, cy - 11, cx + 11, cy + 11), outline=(0, 0, 0), width=4)
            draw.ellipse((cx - 9, cy - 9, cx + 9, cy + 9), outline=ACCENT, width=3)
            draw.ellipse((cx - 3, cy - 3, cx + 3, cy + 3), fill=ACCENT)

        click = last_before(self.events, ("click",), t)
        if click and 0 <= t - click["t"] < CLICK_SECONDS:
            cx, cy = self.to_out(click["x"], click["y"])
            r = 8 + 28 * (t - click["t"]) / CLICK_SECONDS
            draw.ellipse((cx - r, cy - r, cx + r, cy + r), outline=ACCENT, width=3)

        key = last_before(self.events, ("key", "text"), t)
        if key and 0 <= t - key["t"] < KEYCAP_SECONDS:
            label = keycap_text(key["keys"]) if key["type"] == "key" else key["text"]
            left, top, right, bottom = draw.textbbox((0, 0), label, font=self.font_keycap)
            w, h = right - left, bottom - top
            x1, y1 = self.width - 16, 16
            x0, y0 = x1 - w - 24, y1
            draw.rounded_rectangle((x0, y0, x1, y1 + h + 16), radius=10,
                                   fill=KEYCAP_FILL, outline=ACCENT, width=2)
            draw.text((x0 + 12, y0 + 8), label, font=self.font_keycap, fill=INK)

        out = Image.new("RGB", (self.width, self.height + CAPTION_BAR), BAR)
        out.paste(img, (0, 0))
        caption = last_before(self.events, ("caption",), t)
        if caption:
            d2 = ImageDraw.Draw(out)
            d2.text((20, self.height + (CAPTION_BAR - 26) // 2), caption["text"],
                    font=self.font_caption, fill=INK)
        return out

    def title(self, text):
        out = Image.new("RGB", (self.width, self.height + CAPTION_BAR), BAR)
        d = ImageDraw.Draw(out)
        left, top, right, bottom = d.textbbox((0, 0), text, font=self.font_title)
        d.text(((self.width - (right - left)) / 2, (out.height - (bottom - top)) / 2),
               text, font=self.font_title, fill=INK)
        return out


def encode(frames_dir, fps, out_path, colors=256):
    palette = os.path.join(frames_dir, "palette.png")
    pattern = os.path.join(frames_dir, "%05d.png")
    subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-framerate", str(fps), "-i", pattern,
                    "-vf", f"palettegen=stats_mode=diff:max_colors={colors}", palette], check=True)
    subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-framerate", str(fps), "-i", pattern,
                    "-i", palette, "-lavfi", "paletteuse=dither=bayer", "-loop", "0", out_path],
                   check=True)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("events")
    ap.add_argument("--out", required=True)
    ap.add_argument("--width", type=int, default=1100)
    ap.add_argument("--hold", type=float, default=1.5)
    ap.add_argument("--settle", type=float, default=SETTLE_S,
                    help="seconds of picture after the last event before the hold (<= 0.5)")
    ap.add_argument("--colors", type=int, default=256,
                    help="GIF palette size, 2 to 256 (move_the_camera uses 64)")
    ap.add_argument("--sheet", help="write an 8-cell contact sheet of evenly spaced ticks")
    ap.add_argument("--stills", help="comma-separated times (s) to write as full-size PNGs beside --out")
    args = ap.parse_args()

    with open(args.events) as fh:
        rec = json.load(fh)
    fps = rec["fps"]
    r = Renderer(rec, args.width)

    end = timeline_end(rec, args.settle)
    span = end + args.hold
    ticks = [k / fps for k in range(int(span * fps) + 1)]
    work = tempfile.mkdtemp(prefix="render_")
    try:
        if rec.get("title"):
            card = r.title(rec["title"])
            for i in range(fps):
                card.save(os.path.join(work, f"{i:05d}.png"))
        offset = len(os.listdir(work))
        for k, t in enumerate(ticks):
            img = r.frame(min(t, end))
            img.save(os.path.join(work, f"{offset + k:05d}.png"))
        encode(work, fps, args.out, args.colors)
    finally:
        shutil.rmtree(work, ignore_errors=True)

    if args.sheet:
        picks = [ticks[round(i * (len(ticks) - 1) / 7)] for i in range(8)]
        cell_w = 480
        cell_h = round(cell_w * (r.height + CAPTION_BAR) / args.width)
        sheet = Image.new("RGB", (cell_w * 4 + 15, cell_h * 2 + 5), (80, 80, 80))
        for i, t in enumerate(picks):
            cell = r.frame(min(t, end)).resize((cell_w, cell_h), Image.LANCZOS)
            sheet.paste(cell, ((i % 4) * (cell_w + 5), (i // 4) * (cell_h + 5)))
        sheet.save(args.sheet)
        print("sheet ticks:", [round(t, 2) for t in picks])

    if args.stills:
        base, _ = os.path.splitext(args.out)
        for raw in args.stills.split(","):
            t = float(raw)
            r.frame(t).save(f"{base}_t{t:.2f}.png")

    print(f"{args.out}: {len(ticks)} ticks at {fps} fps, {os.path.getsize(args.out) / 1e6:.2f} MB")


if __name__ == "__main__":
    main()
