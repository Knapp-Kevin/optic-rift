#!/usr/bin/env python3
"""Verify the approved GPLv3 Windows FFmpeg archive and stage its executable pair.

This does not download or execute binaries. Release packaging must provide both
SHA-256 build pins and verify executable capabilities on an isolated Windows host.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import shutil
import sys
import tempfile
import zipfile

SUPPLIER = "Gyan Doshi FFmpeg 9.0.2 essentials_build Windows x64 GPLv3"
SOURCE_URL = "https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-9.0.2-essentials_build.zip"
ARCHIVE_SHA256 = "60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba"
REQUIRED = ("ffmpeg.exe", "ffprobe.exe")


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def stage(archive: pathlib.Path, output_dir: pathlib.Path, expected_archive_sha: str = ARCHIVE_SHA256) -> dict:
    if not archive.is_file():
        raise ValueError("The approved supplier archive does not exist")
    actual_archive_sha = sha256_file(archive)
    if actual_archive_sha != expected_archive_sha:
        raise ValueError("Supplier ZIP SHA-256 mismatch; refusing to extract executables")
    selected = {}
    with zipfile.ZipFile(archive) as zf:
        for item in zf.infolist():
            normalized = pathlib.PurePosixPath(item.filename.replace("\\", "/"))
            if item.is_dir() or normalized.is_absolute() or ".." in normalized.parts:
                continue
            if item.external_attr >> 16 & 0o170000 == 0o120000:
                continue
            if len(normalized.parts) < 3 or normalized.parts[-2] != "bin":
                continue
            if normalized.name in REQUIRED:
                if normalized.name in selected:
                    raise ValueError("Duplicate FFmpeg tool in supplier archive")
                selected[normalized.name] = item
        if set(selected) != set(REQUIRED):
            raise ValueError("Supplier archive lacks the required ffmpeg.exe/ffprobe.exe pair")
        output_dir.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="optic-ffmpeg-", dir=output_dir.parent) as tmp:
            staging = pathlib.Path(tmp)
            hashes = {}
            for name in REQUIRED:
                item = selected[name]
                if item.file_size <= 0 or item.file_size > 300 * 1024 * 1024:
                    raise ValueError("Unexpected executable size")
                path = staging / name
                with zf.open(item) as source, path.open("wb") as dest:
                    shutil.copyfileobj(source, dest, length=1024 * 1024)
                if path.stat().st_size != item.file_size:
                    raise ValueError("Extracted executable size mismatch")
                hashes[name] = sha256_file(path)
            metadata = {
                "supplier": SUPPLIER,
                "source_url": SOURCE_URL,
                "archive_sha256": actual_archive_sha,
                "license": "GPLv3",
                "tools": hashes,
                "ffmpeg_source": "https://github.com/FFmpeg/FFmpeg/commit/946fcce07b",
            }
            (staging / "provenance.json").write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
            if output_dir.exists():
                raise ValueError("Staging destination exists; remove it explicitly before restaging")
            staging.rename(output_dir)
            return metadata


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path,
                        default=pathlib.Path(__file__).resolve().parent.parent / "src-tauri" / "resources" / "ffmpeg")
    args = parser.parse_args()
    try:
        info = stage(args.archive, args.output)
    except (ValueError, OSError, zipfile.BadZipFile) as exc:
        print(f"FFmpeg staging refused: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(info, indent=2))
    print("Set BOTH OPTIC_RIFT_FFMPEG_SHA256 and OPTIC_RIFT_FFPROBE_SHA256 from this receipt before the release build.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
