"""Validate an existing Optic Rift-generated PNG for image-to-image conditioning.

This does not authorize arbitrary filesystem imports. The native host must first check
the UUID's app-owned output/provenance pair and supply that digest. The worker then
independently rechecks the regular file, size, format, dimensions and SHA-256 before use.
"""
from __future__ import annotations

import hashlib
import io
import math
import os
import re
import stat
import struct
import uuid

MAX_REFERENCE_BYTES = 32 * 1024 * 1024
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")


class ReferenceError(ValueError):
    """Unsafe or invalid image-to-image reference."""


def check_strength(value):
    if isinstance(value, bool) or not isinstance(value, (float, int)):
        raise ReferenceError("image strength must be a number between 0.05 and 0.95")
    value = float(value)
    if not math.isfinite(value) or not 0.05 <= value <= 0.95:
        raise ReferenceError("image strength must be between 0.05 and 0.95")
    return value


def _regular_unlinked(path, *, directory=False):
    metadata = os.lstat(path)
    is_correct = stat.S_ISDIR(metadata.st_mode) if directory else stat.S_ISREG(metadata.st_mode)
    if not is_correct or stat.S_ISLNK(metadata.st_mode):
        raise ReferenceError("reference is linked or not a regular local output")
    # On Windows reject junction/reparse-point redirection, not just symbolic links.
    if getattr(metadata, "st_file_attributes", 0) & 0x400:
        raise ReferenceError("reference is a Windows reparse point")
    return metadata


def prepare_reference(reference, width, height):
    """Return a decoded RGB Pillow image from a verified local generated output.

    Never accept input from browser-supplied arbitrary paths. The native host owns
    the path and must have checked the matching stored output provenance first.
    """
    if not isinstance(reference, dict):
        raise ReferenceError("image-to-image requires a verified local source output")
    job_id, path, digest = (reference.get(k) for k in ("job_id", "path", "sha256"))
    if not isinstance(job_id, str):
        raise ReferenceError("invalid reference job id")
    try:
        parsed = uuid.UUID(job_id)
    except (ValueError, AttributeError) as exc:
        raise ReferenceError("invalid reference job id") from exc
    if str(parsed) != job_id:
        raise ReferenceError("reference job id must be canonical")
    if not isinstance(path, str) or not os.path.isabs(path) or not path.endswith(".png"):
        raise ReferenceError("reference path must be an absolute PNG path")
    if os.path.basename(path) != f"{job_id}.png" or os.path.basename(os.path.dirname(path)) != "outputs":
        raise ReferenceError("reference must be a named Optic Rift local output")
    if not isinstance(digest, str) or SHA256_RE.fullmatch(digest) is None:
        raise ReferenceError("invalid reference digest")
    if not all(isinstance(n, int) and not isinstance(n, bool) and 256 <= n <= 1536 and n % 8 == 0
               for n in (width, height)):
        raise ReferenceError("invalid requested reference dimensions")

    _regular_unlinked(os.path.dirname(path), directory=True)
    metadata = _regular_unlinked(path)
    if metadata.st_size < 24 or metadata.st_size > MAX_REFERENCE_BYTES:
        raise ReferenceError("reference image is missing, empty or too large")
    with open(path, "rb") as handle:
        data = handle.read(MAX_REFERENCE_BYTES + 1)
    if len(data) != metadata.st_size or len(data) > MAX_REFERENCE_BYTES:
        raise ReferenceError("reference image changed during verification")
    if hashlib.sha256(data).hexdigest() != digest:
        raise ReferenceError("reference image hash does not match its recorded provenance")
    if data[:8] != PNG_SIGNATURE or data[12:16] != b"IHDR":
        raise ReferenceError("reference image is not a PNG")
    actual = struct.unpack(">II", data[16:24])
    if actual != (width, height):
        raise ReferenceError("reference and output must use the same dimensions")

    # Decode from the bytes we just hashed, never reopen the path after verification.
    from PIL import Image, UnidentifiedImageError

    try:
        with Image.open(io.BytesIO(data)) as image:
            if image.format != "PNG" or image.size != actual:
                raise ReferenceError("reference PNG has unexpected content")
            image.load()
            return image.convert("RGB")
    except (OSError, ValueError, UnidentifiedImageError) as exc:
        raise ReferenceError("reference image could not be decoded safely") from exc
