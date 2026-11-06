# Optic Rift visual identity

Status: **Owner-approved composition** with a published 256px WebP derivative. Native Windows icons remain unqualified until they appear in a tested Windows build.

## Approved artwork and provenance

- Companion to Sonic Rift: deep-navy rounded square and cyan-to-violet broken circular rift.
- Center: glossy refracted optical lens with horizontal glint. The aperture/shutter candidate was rejected.
- The approved full-resolution master is a 1254 x 1254 PNG with SHA-256: f94ceeaf961c17313d7f4aaaefe2b63bb9cd96f80694a07850e4f136ee2d27dc.
- The master has been packaged for owner download but is **not yet committed to GitHub**. When transferring the exact approved binary, put it at assets/branding/optic-rift-approved-master.png and verify the SHA-256.
- The [published 256px WebP](../../public/branding/optic-rift-approved-256.webp) is a derivative, not the full-resolution master. Its SHA-256 is dae5767d888bff06870957f975f562324dde5ed83467e6ed23bbb71f642d927c.

## Windows/Tauri icon generation

Requires Python 3 and Pillow. From repository root:

    python scripts/generate_windows_icons.py

The generator prefers assets/branding/optic-rift-approved-master.png if present; it otherwise uses the published 256px WebP and prints a warning. To supply the master explicitly:

    python scripts/generate_windows_icons.py --source /path/to/optic-rift-approved-master.png

It generates src-tauri/icons/icon.ico with 16, 24, 32, 48, 64, 128 and 256px frames, plus per-size PNGs and the common Tauri icon names: icon.png, 32x32.png, 128x128.png and 128x128@2x.png.

Validate the generation logic locally:

    python -m unittest discover -s tests -p 'test_branding_icons.py' -v

The [unmerged desktop foundation PR #1](https://github.com/Knapp-Kevin/optic-rift/pull/1) contains a **placeholder** Windows icon. Running this script in the integrated source checkout creates candidate assets but is not proof of installer adoption. Verify the exact resulting app and installer on Windows: Start, taskbar, Alt+Tab, titlebar, high-DPI modes and installed-app icon metadata. Keep merge, installer signing and release as separate owner-gated decisions.

## Ownership

The [Architectural Index](../../docs/ARCHITECTURE_INDEX.md) owns the architectural routing map. This guide owns visual identity provenance and reproducible icon generation, not release qualification.
