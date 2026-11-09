"""Offline tests for the pinned FFmpeg staging step (no binary execution)."""
import hashlib
import pathlib
import tempfile
import unittest
import zipfile

from stage_ffmpeg import stage


class StageFFmpegTests(unittest.TestCase):
    def make_archive(self, folder, entries):
        target = folder / "vendor.zip"
        with zipfile.ZipFile(target, "w") as zf:
            for name, data in entries.items():
                zf.writestr(name, data)
        digest = hashlib.sha256(target.read_bytes()).hexdigest()
        return target, digest

    def test_pair_hashes_and_existing_output_refusal(self):
        with tempfile.TemporaryDirectory() as root:
            base = pathlib.Path(root)
            archive, digest = self.make_archive(base, {
                "ffmpeg-9.0.2-essentials_build/bin/ffmpeg.exe": b"MZ fake ffmpeg",
                "ffmpeg-9.0.2-essentials_build/bin/ffprobe.exe": b"MZ fake ffprobe",
            })
            dst = base / "resources" / "ffmpeg"
            info = stage(archive, dst, digest)
            self.assertEqual(info["tools"]["ffmpeg.exe"], hashlib.sha256(b"MZ fake ffmpeg").hexdigest())
            self.assertTrue((dst / "provenance.json").is_file())
            with self.assertRaisesRegex(ValueError, "exists"):
                stage(archive, dst, digest)

    def test_archive_checksum_failure_does_not_extract(self):
        with tempfile.TemporaryDirectory() as root:
            base = pathlib.Path(root)
            archive, _ = self.make_archive(base, {"root/bin/ffmpeg.exe": b"a"})
            with self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
                stage(archive, base / "out", "0" * 64)
            self.assertFalse((base / "out").exists())

    def test_rejects_missing_and_duplicate_tools(self):
        with tempfile.TemporaryDirectory() as root:
            base = pathlib.Path(root)
            archive, digest = self.make_archive(base, {"root/bin/ffmpeg.exe": b"a"})
            with self.assertRaisesRegex(ValueError, "lacks the required"):
                stage(archive, base / "out", digest)
            archive, digest = self.make_archive(base, {
                "one/bin/ffmpeg.exe": b"a", "two/bin/ffmpeg.exe": b"b",
                "one/bin/ffprobe.exe": b"c"})
            with self.assertRaisesRegex(ValueError, "Duplicate"):
                stage(archive, base / "out", digest)


if __name__ == "__main__":
    unittest.main()
