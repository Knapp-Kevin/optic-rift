#!/usr/bin/env python3
"""Generate Optic Rift's Windows/Tauri icons from owner-approved art.

Pillow is the only dependency. This does not qualify Windows packaging or
installed taskbar/Start/Alt+Tab appearances.
"""
from __future__ import annotations

import argparse
import hashlib
import tempfile
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
MASTER = ROOT / "assets/branding/optic-rift-approved-master.png"
FALLBACK = ROOT / "public/branding/optic-rift-approved-256.webp"
DEFAULT_OUT = ROOT / "src-tauri/icons"
ICO_SIZES = (16, 24, 32, 48, 64, 128, 256)
PNG_SIZES = (*ICO_SIZES, 512)
TAURI_ALIASES = {32: "32x32.png", 128: "128x128.png", 256: "128x128@2x.png", 512: "icon.png"}
APPROVED_SHA256 = {
    "optic-rift-approved-master.png": "f94ceeaf961c17313d7f4aaaefe2b63bb9cd96f80694a07850e4f136ee2d27dc",
    "optic-rift-approved-256.webp": "dae5767d888bff06870957f975f562324dde5ed83467e6ed23bbb71f642d927c",
}


def assert_approved_source(source: Path) -> None:
    expected = APPROVED_SHA256.get(source.name)
    observed = hashlib.sha256(source.read_bytes()).hexdigest()
    if not expected or observed != expected:
        raise ValueError(
            "Source is not the owner-approved Optic Rift logo; "
            f"actual SHA-256: {observed}"
        )



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




def verify(source: Path, out: Path, *, require_approved: bool = True) -> None:
    """Read-only comparison of installed Tauri icon inputs against approved art.

    Regenerates into a disposable temp directory, then checks every required
    PNG and all ICO frames by decoded RGBA pixels. Does not overwrite the
    repository's actual icon files.
    """
    if require_approved:
        assert_approved_source(source)
    if not out.is_dir():
        raise FileNotFoundError(f"Native icon directory missing: {out}")
    with tempfile.TemporaryDirectory(prefix="optic-rift-icons-check-") as td:
        expected = Path(td)
        generate(source, expected)
        paths = {f"optic-rift-{size}.png" for size in PNG_SIZES}
        paths.update(TAURI_ALIASES.values())
        for name in sorted(paths):
            actual_path = out / name
            if not actual_path.is_file():
                raise FileNotFoundError(f"Required native icon missing: {actual_path}")
            with Image.open(actual_path) as actual, Image.open(expected / name) as original:
                if (actual.size != original.size or
                    actual.convert("RGBA").tobytes() != original.convert("RGBA").tobytes()):
                    raise ValueError(f"Native icon does not match approved artwork: {actual_path}")
        ico_path = out / "icon.ico"
        if not ico_path.is_file():
            raise FileNotFoundError(f"Native Windows ICO missing: {ico_path}")
        with Image.open(ico_path) as actual, Image.open(expected / "icon.ico") as original:
            required = {(size, size) for size in ICO_SIZES}
            if set(actual.info.get("sizes", ())) != required:
                raise ValueError("Native Windows ICO does not contain all required icon sizes")
            for size in sorted(required):
                a = actual.ico.getimage(size).convert("RGBA")
                b = original.ico.getimage(size).convert("RGBA")
                if a.size != b.size or a.tobytes() != b.tobytes():
                    raise ValueError(f"Native ICO frame {size} does not match approved artwork")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, help="Approved PNG or WebP source (default: full master if present)")
    parser.add_argument("--output", type=Path, default=DEFAULT_OUT, help="Directory for generated Tauri icons")
    parser.add_argument("--verify", action="store_true", help="Read-only check of committed icons against approved artwork")
    args = parser.parse_args()
    source = args.source or (MASTER if MASTER.is_file() else FALLBACK)
    if source == FALLBACK:
        print("NOTICE: Using 256px WebP fallback. Import the approved full-res PNG for highest-quality icons.")
    assert_approved_source(source)
    if args.verify:
        verify(source, args.output)
        print(f"PASS: All committed native icon pixels match {source}")
    else:
        generate(source, args.output)
        verify(source, args.output)
        print(f"Generated and checked {len(ICO_SIZES)} ICO sizes from {source}")
    print("Windows installation, taskbar, Alt+Tab, Start and DPI checks remain outstanding.")


if __name__ == "__main__":
    main()
