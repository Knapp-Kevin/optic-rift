"""Offline preflight tests. Synthetic PE-like fixtures, no third-party binaries."""
import hashlib
import json
import struct
from pathlib import Path
import tempfile
import unittest

from verify_ffmpeg_release import ARCHIVE_SHA256, TOOLS, verify


def pe64_fixture(name: str) -> bytes:
    data = bytearray(512)
    data[0:2] = b"MZ"
    struct.pack_into("<I", data, 0x3c, 0x80)
    data[0x80:0x84] = b"PE\0\0"
    struct.pack_into("<HHIIIHH", data, 0x84, 0x8664, 1, 0, 0, 0, 0xF0, 0x0002)
    struct.pack_into("<H", data, 0x98, 0x20B)
    data[0x180:0x180 + len(name)] = name.encode()
    return bytes(data)


class ReleasePreflightTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.folder = Path(self.temp.name) / "ffmpeg"
        self.folder.mkdir()
        hashes = {}
        self.env = {}
        for name, variable in TOOLS.items():
            data = pe64_fixture(name)
            (self.folder / name).write_bytes(data)
            digest = hashlib.sha256(data).hexdigest()
            hashes[name] = digest
            self.env[variable] = digest
        self.provenance = {"archive_sha256": ARCHIVE_SHA256, "license": "GPLv3", "tools": hashes}
        self.write_manifest()

    def write_manifest(self):
        (self.folder / "provenance.json").write_text(json.dumps(self.provenance), encoding="utf-8")

    def test_valid_pair(self):
        self.assertEqual(verify(self.folder, self.env)["status"], "staged-hashes-verified")

    def test_missing_pin(self):
        del self.env["OPTIC_RIFT_FFPROBE_SHA256"]
        with self.assertRaisesRegex(ValueError, "build pin"):
            verify(self.folder, self.env)

    def test_modified_binary(self):
        (self.folder / "ffmpeg.exe").write_bytes(b"MZtampered")
        with self.assertRaisesRegex(ValueError, "Not a Windows PE"):
            verify(self.folder, self.env)

    def test_wrong_archive(self):
        self.provenance["archive_sha256"] = "0" * 64
        self.write_manifest()
        with self.assertRaisesRegex(ValueError, "archive identity"):
            verify(self.folder, self.env)

    def test_missing_binary(self):
        (self.folder / "ffprobe.exe").unlink()
        with self.assertRaisesRegex(ValueError, "Missing regular"):
            verify(self.folder, self.env)

    def test_extra_untracked_file(self):
        (self.folder / "dependency.dll").write_bytes(b"MZhello")
        with self.assertRaisesRegex(ValueError, "Unexpected resource files"):
            verify(self.folder, self.env)

    def test_not_pe(self):
        data = b"XXno-PE"
        (self.folder / "ffprobe.exe").write_bytes(data)
        digest = hashlib.sha256(data).hexdigest()
        self.provenance["tools"]["ffprobe.exe"] = digest
        self.env["OPTIC_RIFT_FFPROBE_SHA256"] = digest
        self.write_manifest()
        with self.assertRaisesRegex(ValueError, "Not a Windows PE"):
            verify(self.folder, self.env)

    def test_wrong_architecture(self):
        data = bytearray(pe64_fixture("ffmpeg.exe"))
        struct.pack_into("<H", data, 0x84, 0x14c)
        self.set_fixture("ffmpeg.exe", bytes(data))
        with self.assertRaisesRegex(ValueError, "AMD64"):
            verify(self.folder, self.env)

    def test_invalid_pe_signature(self):
        data = bytearray(pe64_fixture("ffmpeg.exe"))
        data[0x80:0x84] = b"FAKE"
        self.set_fixture("ffmpeg.exe", bytes(data))
        with self.assertRaisesRegex(ValueError, "PE signature"):
            verify(self.folder, self.env)

    def set_fixture(self, name, data):
        (self.folder / name).write_bytes(data)
        digest = hashlib.sha256(data).hexdigest()
        self.provenance["tools"][name] = digest
        self.env[TOOLS[name]] = digest
        self.write_manifest()

    def test_symlink(self):
        (self.folder / "ffprobe.exe").unlink()
        (self.folder / "ffprobe.exe").symlink_to(self.folder / "ffmpeg.exe")
        with self.assertRaisesRegex(ValueError, "Missing regular"):
            verify(self.folder, self.env)


if __name__ == "__main__":
    unittest.main()
