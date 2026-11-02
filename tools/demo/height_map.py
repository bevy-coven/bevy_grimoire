#!/usr/bin/env python3
"""Write the 16-bit height map `tools/demo/storyboards/terrain_import.json` imports (#99).

    python3 tools/demo/height_map.py --out <project>/assets/grimoire/maps/hills16.png

A 65x65 16-bit greyscale PNG: sample 0 is `range_m.0` on the Terrain line and 65535 is `range_m.1`. The
shape is a dome with a flat middle, so the change the clip shows is unmistakable — the ground was a plane
before the import and is this after it.

`terrain_import` accepts nothing but a 16-bit greyscale PNG (a 16-bit PNG of another colour type is
refused by name), which is why this writes the bytes itself rather than going through a screenshot tool.
"""
import argparse
import sys


def dome(n: int, flat: float = 0.22, rise: float = 0.30, gamma: float = 1.6) -> "list[list[int]]":
    """`n`x`n` samples: flat out to `flat` of the half-diagonal, rising to 65535 by `flat + rise`."""
    rows = []
    for y in range(n):
        row = []
        for x in range(n):
            dx = x / (n - 1) - 0.5
            dy = y / (n - 1) - 0.5
            d = (dx * dx + dy * dy) ** 0.5
            h = max(0.0, min(1.0, (d - flat) / rise)) ** gamma
            row.append(int(round(h * 65535)))
        rows.append(row)
    return rows


def write_png16(path: str, rows: "list[list[int]]") -> None:
    """A greyscale-16 PNG, by hand: PIL cannot build one from an array in every version."""
    import struct
    import zlib

    h = len(rows)
    w = len(rows[0])
    raw = b"".join(b"\x00" + b"".join(struct.pack(">H", v) for v in row) for row in rows)

    def chunk(tag: bytes, data: bytes) -> bytes:
        return (struct.pack(">I", len(data)) + tag + data
                + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF))

    ihdr = struct.pack(">IIBBBBB", w, h, 16, 0, 0, 0, 0)  # 16 bits, colour type 0 (greyscale)
    with open(path, "wb") as fh:
        fh.write(b"\x89PNG\r\n\x1a\n")
        fh.write(chunk(b"IHDR", ihdr))
        fh.write(chunk(b"IDAT", zlib.compress(raw, 9)))
        fh.write(chunk(b"IEND", b""))


def main(argv: "list[str]") -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", required=True, help="where to write the PNG")
    ap.add_argument("--size", type=int, default=65, help="samples a side (default 65)")
    args = ap.parse_args(argv)
    if args.size < 2:
        sys.exit("--size must be at least 2")
    write_png16(args.out, dome(args.size))
    print(f"{args.out}: {args.size}x{args.size} 16-bit greyscale")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
