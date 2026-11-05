# Optic Rift visual identity

Status: Owner-approved icon composition; native Windows packaging icons not yet regenerated or qualified.

## Approved artwork

- Product companion to Sonic Rift: same deep-navy rounded square and cyan-to-violet circular rift family.
- Center: a refracted glossy optical lens with a horizontal glint, not the rejected aperture shutter.
- Versioned app/README derivative: [optimized 256px WebP](../../public/branding/optic-rift-approved-256.webp).
- Approved original was a 1254x1254 PNG in the ChatGPT artwork context; this GitHub asset is a faithful downscale, **not** the original full-resolution master.
- WebP SHA-256: dae5767d888bff06870957f975f562324dde5ed83467e6ed23bbb71f642d927c.
- The original ChatGPT artifact path is /mnt/data/a_polished_high_contrast_app_icon_in_the_same_sty.png; it is **not** available automatically in Claude's Windows checkout.

## Windows application icon

The source app shell still references src-tauri/icons/icon.ico, which has **not** been regenerated from this artwork. This asset is wired to the application sidebar but Windows installer, taskbar, Alt+Tab and Start icons remain unqualified.

Run `python scripts/generate_windows_icons.py` in a local checkout after installing Pillow; inspect the PNG/ICO output at 16, 24, 32, 48, 128 and 256 pixels and validate native Windows packaging. The full master must be imported separately for high-resolution brand/marketing use.

## Ownership

The [canonical Architectural Index](../../docs/ARCHITECTURE_INDEX.md) routes product truth. This document owns branding source/provenance but cannot itself approve an installer or release.
