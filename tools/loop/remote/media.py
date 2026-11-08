#!/usr/bin/env python3
"""usage: p7_media.py <sha8> <dir-with-gifs> [editor.png path]"""
import re, shutil, sys
from pathlib import Path
from PIL import Image
sha, src = sys.argv[1], Path(sys.argv[2])
png = Path(sys.argv[3]) if len(sys.argv) > 3 else None
repo = Path.home() / "bevy_grimoire"
readme = repo / "docs/media/README.md"
lines = readme.read_text().split("\n")
def row_of(name):
    for i, l in enumerate(lines):
        if l.startswith(f"| `{name}` |"):
            return i
    raise SystemExit(f"no README row for {name}")
def set_captured(i):
    cells = lines[i].split(" | ")
    cells[2] = f"2026-11-08 {sha}"
    lines[i] = " | ".join(cells)
for gif in sorted(src.glob("*.gif")):
    i = row_of(gif.name)
    im = Image.open(gif)
    n, (w, h) = im.n_frames, im.size
    new = re.sub(r"\d+ frames, \d+ x \d+,", f"{n} frames, {w} x {h},", lines[i], count=1)
    if new == lines[i] and f"{n} frames, {w} x {h}," not in lines[i]:
        raise SystemExit(f"{gif.name}: no `N frames, W x H,` in its row")
    lines[i] = new
    set_captured(i)
    shutil.copyfile(gif, repo / "docs/media" / gif.name)
    print(gif.name, n, w, h)
if png:
    i = row_of("editor.png")
    set_captured(i)
    shutil.copyfile(png, repo / "docs/media/editor.png")
    print("editor.png copied")
readme.write_text("\n".join(lines))
