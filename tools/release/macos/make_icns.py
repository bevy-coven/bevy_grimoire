#!/usr/bin/env python3
"""Build Cauldron.icns from the 256 px Retro Diffusion icon.

The pixel art is scaled 3x with NEAREST (so pixels stay crisp), masked with a
rounded rectangle, centred on a transparent 1024 px canvas (the macOS icon grid
leaves a margin around the artwork), then every iconset size is resized from
that canvas and packed by `iconutil`.

Runs on macOS only: `iconutil` ships with it. Needs Pillow.

    python3 make_icns.py crates/bevy_cauldron/macos/icon-256.png --out Cauldron.icns
"""

import argparse
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image, ImageDraw

SOURCE_SIZE = 256
SCALE = 3
ARTWORK_SIZE = SOURCE_SIZE * SCALE  # 768
CANVAS_SIZE = 1024
ARTWORK_OFFSET = (CANVAS_SIZE - ARTWORK_SIZE) // 2  # 128
CORNER_RADIUS = 168
ICONSET_POINT_SIZES = (16, 32, 128, 256, 512)
RETINA_FACTOR = 2
# At or above this many pixels the art is pixel art at a whole-number scale, so
# NEAREST keeps it crisp; below it LANCZOS averages instead of dropping pixels.
NEAREST_MIN_PX = 256
ICONUTIL = "/usr/bin/iconutil"
ICONSET_NAME = "Cauldron.iconset"


def build_canvas(source: Path) -> Image.Image:
    art = Image.open(source).convert("RGBA")
    if art.size != (SOURCE_SIZE, SOURCE_SIZE):
        sys.exit(f"{source} is {art.width}x{art.height}; expected {SOURCE_SIZE}x{SOURCE_SIZE}")
    art = art.resize((ARTWORK_SIZE, ARTWORK_SIZE), Image.NEAREST)
    mask = Image.new("L", art.size, 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        (0, 0, ARTWORK_SIZE - 1, ARTWORK_SIZE - 1), radius=CORNER_RADIUS, fill=255
    )
    canvas = Image.new("RGBA", (CANVAS_SIZE, CANVAS_SIZE), (0, 0, 0, 0))
    canvas.paste(art, (ARTWORK_OFFSET, ARTWORK_OFFSET), mask)
    return canvas


def write_iconset(canvas: Image.Image, iconset: Path) -> None:
    iconset.mkdir(parents=True)
    for points in ICONSET_POINT_SIZES:
        for factor in (1, RETINA_FACTOR):
            px = points * factor
            resample = Image.NEAREST if px >= NEAREST_MIN_PX else Image.LANCZOS
            suffix = "" if factor == 1 else f"@{factor}x"
            canvas.resize((px, px), resample).save(iconset / f"icon_{points}x{points}{suffix}.png")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("source", type=Path, help="256x256 icon PNG")
    parser.add_argument("--out", type=Path, required=True, help="output .icns path")
    args = parser.parse_args()

    canvas = build_canvas(args.source)
    with tempfile.TemporaryDirectory() as tmp:
        iconset = Path(tmp) / ICONSET_NAME
        write_iconset(canvas, iconset)
        args.out.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run([ICONUTIL, "-c", "icns", str(iconset), "-o", str(args.out)], check=True)


if __name__ == "__main__":
    main()
