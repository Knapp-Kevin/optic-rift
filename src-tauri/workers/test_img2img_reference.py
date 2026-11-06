"""GPU-free image-to-image input and integrity tests.

Run: python -m unittest discover -s src-tauri/workers -p "test_img2img_reference.py" -v
Requires Pillow, used by the already-pinned image runtime.
"""
from __future__ import annotations

import hashlib
import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
import uuid

try:
    from PIL import Image
except ImportError:
    Image = None

TARGET = Path(__file__).with_name("img2img_reference.py")
SPEC = importlib.util.spec_from_file_location("img2img_reference", TARGET)
assert SPEC and SPEC.loader
reference = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(reference)


@unittest.skipUnless(Image is not None, "Pillow is needed for reference decoding")
class ReferenceTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        folder = Path(self.tmp.name) / "outputs"
        folder.mkdir()
        self.job_id = str(uuid.uuid4())
        self.path = folder / f"{self.job_id}.png"
        Image.new("RGB", (512, 512), (24, 100, 180)).save(self.path)
        self.payload = {
            "job_id": self.job_id, "path": str(self.path),
            "sha256": hashlib.sha256(self.path.read_bytes()).hexdigest(),
        }

    def test_valid_reference_is_rgb_and_exactly_sized(self):
        image = reference.prepare_reference(self.payload, 512, 512)
        self.assertEqual(image.size, (512, 512))
        self.assertEqual(image.mode, "RGB")

    def test_mutation_after_provenance_is_refused(self):
        with self.path.open("ab") as handle:
            handle.write(b"modified")
        with self.assertRaisesRegex(reference.ReferenceError, "hash"):
            reference.prepare_reference(self.payload, 512, 512)

    def test_wrong_dimensions_and_unapproved_path_refused(self):
        with self.assertRaisesRegex(reference.ReferenceError, "same dimensions"):
            reference.prepare_reference(self.payload, 1024, 1024)
        for bad in ("not-an-id", self.job_id.upper()):
            with self.subTest(bad=bad):
                with self.assertRaises(reference.ReferenceError):
                    reference.prepare_reference({**self.payload, "job_id": bad}, 512, 512)
        with self.assertRaises(reference.ReferenceError):
            reference.prepare_reference({**self.payload, "path": self.tmp.name}, 512, 512)

    def test_linked_file_refused(self):
        real = self.path.with_suffix(".saved")
        self.path.rename(real)
        try:
            os.symlink(real, self.path)
        except (OSError, NotImplementedError):
            self.skipTest("Creating test symlinks is not allowed on this host")
        with self.assertRaisesRegex(reference.ReferenceError, "linked"):
            reference.prepare_reference(self.payload, 512, 512)

    @unittest.skipUnless(os.name == "nt", "directory junctions are a Windows feature")
    def test_junctioned_outputs_folder_refused(self):
        # Junctions need no special privilege on Windows, unlike symbolic links.
        import subprocess

        outputs = self.path.parent
        elsewhere = Path(self.tmp.name) / "elsewhere"
        outputs.rename(elsewhere)
        created = subprocess.run(["cmd", "/C", "mklink", "/J", str(outputs), str(elsewhere)],
                                 capture_output=True, text=True)
        self.assertEqual(created.returncode, 0, created.stdout + created.stderr)
        self.addCleanup(lambda: os.rmdir(outputs) if outputs.exists() else None)
        self.assertTrue((outputs / self.path.name).exists(), "junction resolves to the real file")
        with self.assertRaisesRegex(reference.ReferenceError, "linked|reparse"):
            reference.prepare_reference(self.payload, 512, 512)

    def test_reference_size_bound(self):
        with self.path.open("ab") as handle:
            handle.truncate(reference.MAX_REFERENCE_BYTES + 1)
        self.payload["sha256"] = hashlib.sha256(b"placeholder").hexdigest()
        with self.assertRaisesRegex(reference.ReferenceError, "too large"):
            reference.prepare_reference(self.payload, 512, 512)


class StrengthTests(unittest.TestCase):
    def test_strength_accepts_only_safe_finite_range(self):
        self.assertEqual(reference.check_strength(0.65), 0.65)
        for bad in (0, 1, -0.5, float("nan"), float("inf"), True, None, "0.7"):
            with self.subTest(bad=bad):
                with self.assertRaises(reference.ReferenceError):
                    reference.check_strength(bad)


if __name__ == "__main__":
    unittest.main()
