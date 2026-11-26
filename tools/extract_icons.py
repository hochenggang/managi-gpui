#!/usr/bin/env python3
"""Extract the Material-style SVG paths used by managi-v3 into app assets.

Usage:
    python tools/extract_icons.py <v3-icons-dir> <output-dir>
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ICON_MAP = {
    "IconTerm.vue": "terminal.svg",
    "IconFinder.vue": "folder.svg",
    "IconSetting.vue": "setting.svg",
    "IconBatch.vue": "batch.svg",
}

EXTRA = {
    "plus.svg": (
        '24',
        ["M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z"],
    ),
    "chevron-right.svg": (
        '24',
        ["M8.59 16.59L13.17 12 8.59 7.41 10 6l6 6-6 6-1.41-1.41z"],
    ),
    "chevron-down.svg": (
        '24',
        ["M16.59 8.59L12 13.17l-4.59-4.58L6 10l6 6 6-6-1.41-1.41z"],
    ),
    "chevron-left.svg": (
        '24',
        ["M15.41 7.41L14 6l-6 6 6 6 1.41-1.41L10.83 12z"],
    ),
}


def write_svg(out: Path, name: str, view_box: str, paths: list[str]) -> None:
    body = "\n".join(f'  <path d="{d}" />' for d in paths)
    text = (
        '<svg xmlns="http://www.w3.org/2000/svg" '
        f'viewBox="0 0 {view_box} {view_box}" fill="currentColor">\n'
        f"{body}\n</svg>\n"
    )
    (out / name).write_text(text, encoding="utf-8")


def main(argv: list[str]) -> int:
    if len(argv) < 3:
        print(__doc__)
        return 2

    source = Path(argv[1])
    out = Path(argv[2])
    out.mkdir(parents=True, exist_ok=True)

    for vue_name, svg_name in ICON_MAP.items():
        text = (source / vue_name).read_text(encoding="utf-8")
        view_box = re.search(r'viewBox="0 0 ([0-9]+) [0-9]+"', text)
        paths = re.findall(r'<path\s+d="([^"]+)"', text, re.S)
        if not view_box or not paths:
            print(f"skip {vue_name}: no viewBox/path")
            continue
        write_svg(out, svg_name, view_box.group(1), [p.strip() for p in paths])
        print(f"{svg_name}: {len(paths)} paths")

    for name, (view_box, paths) in EXTRA.items():
        write_svg(out, name, view_box, paths)
        print(f"{name}: {len(paths)} paths")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
