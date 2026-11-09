"""Regression checks for Windows icon generation; runs locally with Pillow."""
from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path

from PIL import Image

MODULE = Path(__file__).resolve().parents[1] / "scripts/generate_windows_icons.py"
spec = importlib.util.spec_from_file_location("optic_rift_icons", MODULE)
assert spec is not None and spec.loader is not None
icons = importlib.util.module_from_spec(spec)
spec.loader.exec_module(icons)


class IconGenerationTests(unittest.TestCase):
    def test_generates_required_windows_and_tauri_images(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            source = root / "approved.png"
            Image.new("RGB", (400, 400), (15, 26, 65)).save(source)
            output = root / "src-tauri/icons"
            icons.generate(source, output)
            for size in icons.PNG_SIZES:
                with self.subTest(size=size), Image.open(output / f"optic-rift-{size}.png") as img:
                    self.assertEqual(img.size, (size, size))
            for size, alias in icons.TAURI_ALIASES.items():
                with self.subTest(alias=alias), Image.open(output / alias) as img:
                    self.assertEqual(img.size, (size, size))
            with Image.open(output / "icon.ico") as ico:
                self.assertEqual(set(ico.info["sizes"]), {(s, s) for s in icons.ICO_SIZES})

    def test_rejects_small_or_non_square_art(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            out = root / "out"
            for dimensions in ((255, 255), (256, 300)):
                with self.subTest(dimensions=dimensions):
                    source = root / "bad.png"
                    Image.new("RGB", dimensions).save(source)
                    with self.assertRaisesRegex(ValueError, "square and at least 256"):
                        icons.generate(source, out)
            self.assertFalse(out.exists())

    def test_verify_passes_for_generated_art_and_rejects_wrong_pixels(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            source = root / "approved.png"
            Image.new("RGB", (512, 512), (30, 90, 180)).save(source)
            output = root / "icons"
            icons.generate(source, output)
            icons.verify(source, output, require_approved=False)
            Image.new("RGBA", (128, 128), (255, 20, 0, 255)).save(output / "128x128.png")
            with self.assertRaisesRegex(ValueError, "does not match approved artwork"):
                icons.verify(source, output, require_approved=False)

    def test_verify_rejects_placeholder_ico_and_missing_sizes(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            source = root / "approved.png"
            Image.new("RGB", (512, 512), (30, 90, 180)).save(source)
            output = root / "icons"
            icons.generate(source, output)
            Image.new("RGBA", (256, 256), (255, 20, 0, 255)).save(
                output / "icon.ico", format="ICO", sizes=[(32, 32), (256, 256)])
            with self.assertRaisesRegex(ValueError, "required icon sizes"):
                icons.verify(source, output, require_approved=False)

    def test_only_canonical_approved_art_can_run_in_normal_mode(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            source = root / "optic-rift-approved-256.webp"
            Image.new("RGB", (256, 256), (30, 90, 180)).save(source, format="WEBP")
            with self.assertRaisesRegex(ValueError, "not the owner-approved"):
                icons.assert_approved_source(source)

    def test_rejects_missing_source(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            with self.assertRaises(FileNotFoundError):
                icons.generate(root / "missing.png", root / "out")


if __name__ == "__main__":
    unittest.main()
