#!/usr/bin/env python3
"""Fail-closed pre-build identity check for the pinned Windows FFmpeg pair.

Does not execute binaries, validate signatures, or grant license/patent clearance.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import sys

ARCHIVE_SHA256 = "60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba"
TOOLS = {"ffmpeg.exe": "OPTIC_RIFT_FFMPEG_SHA256", "ffprobe.exe": "OPTIC_RIFT_FFPROBE_SHA256"}
HASH = re.compile(r"[0-9a-fA-F]{64}\Z")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def validate_pe64(path: Path) -> None:
    """Validate PE/COFF structure and AMD64 architecture, not Authenticode trust."""
    with path.open("rb") as stream:
        dos = stream.read(64)
        if len(dos) < 64 or dos[:2] != b"MZ":
            raise ValueError(f"Not a Windows PE executable: {path.name}")
        pe_offset = int.from_bytes(dos[0x3C:0x40], "little")
        if pe_offset < 64 or pe_offset > 1024 * 1024 or pe_offset + 26 > path.stat().st_size:
            raise ValueError(f"Invalid PE header offset: {path.name}")
        stream.seek(pe_offset)
        coff = stream.read(26)
        if len(coff) != 26 or coff[:4] != b"PE\x00\x00":
            raise ValueError(f"Invalid PE signature: {path.name}")
        machine = int.from_bytes(coff[4:6], "little")
        sections = int.from_bytes(coff[6:8], "little")
        optional_size = int.from_bytes(coff[20:22], "little")
        characteristics = int.from_bytes(coff[22:24], "little")
        magic = int.from_bytes(coff[24:26], "little")
        if machine != 0x8664 or magic != 0x20B:
            raise ValueError(f"Expected PE32+ AMD64 executable: {path.name}")
        if not (1 <= sections <= 96) or optional_size < 0x70 or not characteristics & 0x0002:
            raise ValueError(f"Invalid PE/COFF executable headers: {path.name}")
        if pe_offset + 24 + optional_size + sections * 40 > path.stat().st_size:
            raise ValueError(f"Truncated PE section table: {path.name}")


def verify(folder: Path, environment: dict[str, str] | None = None) -> dict:
    env = os.environ if environment is None else environment
    if folder.is_symlink() or not folder.is_dir():
        raise ValueError("Staging folder must be a real existing directory")
    provenance = folder / "provenance.json"
    if provenance.is_symlink() or not provenance.is_file():
        raise ValueError("Missing regular provenance.json")
    if provenance.stat().st_size > 65536:
        raise ValueError("Oversized provenance file")
    try:
        data = json.loads(provenance.read_text(encoding="utf-8"))
    except (UnicodeError, json.JSONDecodeError) as exc:
        raise ValueError("Invalid provenance file") from exc
    if not isinstance(data, dict) or data.get("archive_sha256") != ARCHIVE_SHA256:
        raise ValueError("Supplier archive identity is not the approved version")
    if data.get("license") != "GPLv3":
        raise ValueError("Unexpected distribution license record")
    recorded = data.get("tools")
    if not isinstance(recorded, dict) or set(recorded) != set(TOOLS):
        raise ValueError("Provenance must list exactly the two approved tools")
    verified = {}
    for name, variable in TOOLS.items():
        path = folder / name
        if path.is_symlink() or not path.is_file():
            raise ValueError(f"Missing regular staged executable: {name}")
        if path.stat().st_size < 2 or path.stat().st_size > 300 * 1024 * 1024:
            raise ValueError(f"Unexpected executable size: {name}")
        validate_pe64(path)
        recorded_hash = recorded[name]
        pin = env.get(variable, "")
        if not isinstance(recorded_hash, str) or not HASH.fullmatch(recorded_hash):
            raise ValueError(f"Missing valid provenance hash: {name}")
        if not HASH.fullmatch(pin):
            raise ValueError(f"Missing or invalid build pin: {variable}")
        actual = sha256(path)
        if actual != recorded_hash.lower() or actual != pin.lower():
            raise ValueError(f"Hash mismatch: {name}")
        verified[name] = actual
    extras = sorted(path.name for path in folder.iterdir()
                    if path.name not in (*TOOLS, "provenance.json"))
    if extras:
        raise ValueError("Unexpected resource files: " + ", ".join(extras))
    return {"status": "staged-hashes-verified", "tools": verified,
            "limitations": "Not signature, license, codec, or installed-app acceptance"}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--resources", type=Path,
                        default=Path(__file__).resolve().parent.parent / "src-tauri" / "resources" / "ffmpeg")
    args = parser.parse_args()
    try:
        result = verify(args.resources)
    except (OSError, ValueError) as exc:
        print(f"FFmpeg release preflight refused: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
