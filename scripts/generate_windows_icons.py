#!/usr/bin/env python3
"""Generate Optic Rift's Windows/Tauri icons from owner-approved art.

Pillow is the only dependency. This does not qualify Windows packaging or
installed taskbar/Start/Alt+Tab appearances.
"""
from __future__ import annotations

import argparse
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
MASTER = ROOT / "assets/branding/optic-rift-approved-master.png"
FALLBACK = ROOT / "public/branding/optic-rift-approved-256.webp"
DEFAULT_OUT = ROOT / "src-tauri/icons"
ICO_SIZES = (16, 24, 32, 48, 64, 128, 256)
PNG_SIZES = (*ICO_SIZES, 512)
TAURI_ALIASES = {32: "32x32.png", 128: "128x128.png", 256: "128x128@2x.png", 512: "icon.png"}


def generate(source: Path, out: Path) -> None:
    """Build losslessly encoded icon derivatives; never alter approved art."""
    if not source.is_file():
        raise FileNotFoundError(f"Approved source artwork not found: {source}")
    with Image.open(source) as original:
        original.load()
        if original.width != original.height or original.width < 256:
            raise ValueError("Approved icon must be square and at least 256x256 pixels")
        image = original.convert("RGBA")
    out.mkdir(parents=True, exist_ok=True)
    for size in PNG_SIZES:
        resized = image.resize((size, size), Image.Resampling.LANCZOS)
        resized.save(out / f"optic-rift-{size}.png", format="PNG", optimize=True)
        if size in TAURI_ALIASES:
            resized.save(out / TAURI_ALIASES[size], format="PNG", optimize=True)
    image.save(
        out / "icon.ico", format="ICO",
        sizes=[(size, size) for size in ICO_SIZES], bitmap_format="png",
    )
    with Image.open(out / "icon.ico") as ico:
        actual = set(ico.info.get("sizes", ()))
    expected = {(size, size) for size in ICO_SIZES}
    if actual != expected:
        raise RuntimeError(f"ICO size verification failed: expected {expected}, got {actual}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, help="Approved PNG or WebP source (default: full master if present)")
    parser.add_argument("--output", type=Path, default=DEFAULT_OUT, help="Directory for generated Tauri icons")
    args = parser.parse_args()
    source = args.source or (MASTER if MASTER.is_file() else FALLBACK)
    if source == FALLBACK:
        print("NOTICE: Using 256px WebP fallback. Import the approved full-res PNG for highest-quality icons.")
    generate(source, args.output)
    print(f"Generated and checked {len(ICO_SIZES)} ICO sizes from {source}")
    print("Windows installation, taskbar, Alt+Tab, Start and DPI checks remain outstanding.")


if __name__ == "__main__":
    main()
