"""Offline preflight tests. Synthetic PE-like fixtures, no third-party binaries."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from verify_ffmpeg_release import ARCHIVE_SHA256, TOOLS, verify


class ReleasePreflightTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.folder = Path(self.temp.name) / "ffmpeg"
        self.folder.mkdir()
        hashes = {}
        self.env = {}
        for name, variable in TOOLS.items():
            data = b"MZ" + name.encode()
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
        with self.assertRaisesRegex(ValueError, "Hash mismatch"):
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
        with self.assertRaisesRegex(ValueError, "Not a Windows executable"):
            verify(self.folder, self.env)

    def test_symlink(self):
        (self.folder / "ffprobe.exe").unlink()
        (self.folder / "ffprobe.exe").symlink_to(self.folder / "ffmpeg.exe")
        with self.assertRaisesRegex(ValueError, "Missing regular"):
            verify(self.folder, self.env)


if __name__ == "__main__":
    unittest.main()
