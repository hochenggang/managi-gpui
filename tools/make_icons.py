#!/usr/bin/env python3
"""Generate platform icon assets from a single square PNG.

Usage:
    python tools/make_icons.py <source.png> [output-dir]

Produces, under ``assets/branding`` by default:

    icon-16.png ... icon-512.png   raster sizes used by Linux/desktop entries
    icon.png                      512px copy kept for generic use
    app.ico                       Windows multi-size icon (16..256)
    app.icns                      macOS icon (16..512, PNG payloads)

The source is normalised to RGBA and padded to a square with transparency,
so artwork does not need to be exactly 1024x1024.
"""

from __future__ import annotations

import struct
import sys
from pathlib import Path

from PIL import Image

PNG_SIZES = (16, 24, 32, 48, 64, 128, 256, 512)
ICO_SIZES = ((16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256))

# ICNS element type -> pixel size. PNG payloads are valid for every one of them.
ICNS_TYPES = {
    b"icp4": 16,
    b"icp5": 32,
    b"ic07": 128,
    b"ic08": 256,
    b"ic09": 512,
}


def load_square(source: Path) -> Image.Image:
    image = Image.open(source).convert("RGBA")
    side = max(image.size)
    canvas = Image.new("RGBA", (side, side), (0, 0, 0, 0))
    canvas.paste(image, ((side - image.width) // 2, (side - image.height) // 2))
    return canvas


def resize(image: Image.Image, size: int) -> Image.Image:
    return image.resize((size, size), Image.LANCZOS)


def write_pngs(base: Image.Image, out: Path) -> None:
    for size in PNG_SIZES:
        resize(base, size).save(out / f"icon-{size}.png", optimize=True)
    resize(base, 512).save(out / "icon.png", optimize=True)


def write_ico(base: Image.Image, out: Path) -> None:
    resize(base, 256).save(out / "app.ico", format="ICO", sizes=ICO_SIZES)


def write_icns(base: Image.Image, out: Path) -> None:
    from io import BytesIO

    chunks = []
    for kind, size in ICNS_TYPES.items():
        buffer = BytesIO()
        resize(base, size).save(buffer, format="PNG", optimize=True)
        payload = buffer.getvalue()
        chunks.append(kind + struct.pack(">I", len(payload) + 8) + payload)

    body = b"".join(chunks)
    (out / "app.icns").write_bytes(b"icns" + struct.pack(">I", len(body) + 8) + body)


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print(__doc__)
        return 2

    source = Path(argv[1])
    out = Path(argv[2]) if len(argv) > 2 else Path("assets/branding")
    out.mkdir(parents=True, exist_ok=True)

    base = load_square(source)
    write_pngs(base, out)
    write_ico(base, out)
    write_icns(base, out)

    for name in ("app.ico", "app.icns", "icon.png"):
        print(f"{name}: {(out / name).stat().st_size} bytes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
