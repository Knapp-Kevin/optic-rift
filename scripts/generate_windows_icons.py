#!/usr/bin/env python3
"""Build Windows PNG/ICO derivatives from Optic Rift approved raster. Requires Pillow."""
from pathlib import Path
try:
    from PIL import Image
except ImportError as exc:
    raise SystemExit("Pillow required: python -m pip install Pillow") from exc
ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "public/branding/optic-rift-approved-256.webp"
OUT = ROOT / "src-tauri/icons"
SIZES = (16, 24, 32, 48, 64, 128, 256)

def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    with Image.open(SOURCE) as source:
        image = source.convert("RGBA")
        for size in SIZES:
            image.resize((size, size), Image.Resampling.LANCZOS).save(OUT / f"optic-rift-{size}.png", optimize=True)
        image.save(OUT / "icon.ico", format="ICO", sizes=[(size, size) for size in SIZES])
    print("Generated candidate icons. Review on Windows before accepting.")

if __name__ == "__main__":
    main()
