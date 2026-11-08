# Real Photo Studio crop: Windows acceptance (November 8, 2026)

**Status:** implemented on the EXISTING Studio PR #35, not merged or Windows-qualified yet. Parent branch also contains the approved HTML/favicon reference and an opt-in Git worktree audit script. Native taskbar/Start icon remains a separate PR #34 responsibility.

## Current functional slice

- Basic brightness and contrast correction (25–175%), 90° rotation, and **crop after rotation**.
- Crop inset controls for left/top/right/bottom in 0.25% increments, with Square 1:1, Wide 16:9, Portrait 4:5, and Reset. Presets center the crop using decoded source dimensions. Percentages are **basis points of the rotated image**. This is a functional crop, not CSS clipping; all source pixels outside the rectangle are excluded from the saved PNG.
- Crop is stored per photo in app session state. Photo A's crop does not overwrite Photo B's crop. It survives navigation during one session, **not restart or project persistence**.
- The common 2D Canvas raster painter serves both bounded proxy preview and full-resolution PNG export. It decodes the real source once for preview and separately during deliberate Save; saving never modifies the original.
- Native export accepts and validates optional four-edge crop metadata in basis points. The consent-gated, no-overwrite Pictures PNG and provenance receipt record crop settings. Native validates PNG width, height, decoded memory and output type; source file hash remains **reported by WebView, not independently attested by native**. No arbitrary filesystem path is accepted.
- No generated image, video model or inference GPU is required for these edits.
- Limitations: **no mouse-drag crop marquee/handles yet** (edge sliders/presets only), no arbitrary-angle straightening, flip, resizing recipe, exposure/highlights/whites/color balance controls, layers/masks/curves/heal, version history, native project asset persistence or managed HDR/ICC. These remain tracked under Issue #29.

## Must-run exact-head verification

1. Use a separate Windows worktree of the live PR #35 head, synthetic RGB(A) gradient/grid photos in PNG/JPEG/WebP and the previously qualified non-sensitive captured video frame. Do not change the owner's primary profile or re-download models.
2. Run clean `npm ci`, `npm run test:content` (including `tests/photoCrop.test.mjs`, `tests/photoEdits.test.mjs`, favicon test), `npm run build`, Cargo fmt/check/tests/clippy. Record SHA, suite totals and regressions; no pre-existing CI green substitutes for this pass.
3. Open a 4000×3000 image. Center Square 1:1 should output 3000×3000 with the correct pixels and no stretch. Rotation 90° then Square must crop according to the **rotated** axes, not the original JPEG axes. Wide and Portrait presets must match the visible framing within the declared 0.25%-step precision.
4. Move individual Left/Top/Right/Bottom crop sliders and compare exported PNG dimensions against `photoCropBounds` with nearest integer pixel calculations. Verify opposing inset ranges cannot delete all pixels, even for 1–2px synthetic images. A bad request (>9500 basis points, left+right>=10000 or top+bottom>=10000, wrong type) must fail **before writing**.
5. Compare the cropped/rotated/brightness+contrast preview against the full PNG raster after reopening, using exact geometry/pixel checks appropriate to preview downscaling. Do not report the proxy image as byte-identical to full-resolution PNG unless deliberately re-rendered at common size/color path. Include alpha and high-saturation fixture checks; record actual color profile differences.
6. Photo A crop/rotation → open Photo B and set different crop → switch to Video Studio and Media Library → return to A. Confirm both adjustment states remain independent. Removing A must drop only its edit state and release source URL; restarting must **not** magically restore session edits.
7. Click Save and **decline** the Pictures/OneDrive warning: zero files written. On explicit QA-approved acceptance only, verify new unique PNG+provenance files, correct crop basis points/dimensions/hash, original file byte and timestamp unchanged, no leftover partials, and no source filename or video clip filename in provenance.
8. Test unsupported SVG and oversized PNG (25MP/9000px) remain read-only and cannot call native export. Test corrupt headers, empty PNG, forged native IPC crop fields, and a symlink/junctioned export destination. Error messages must accurately say what failed.
9. Check Photo Studio Save and Crop controls are keyboard reachable and not obscured at **1280×720, 1440×900, 1920×1080, 1100×700**. Inspect 125% scaling only with owner permission. Compare visual treatment against the accepted Photo Studio wireframe; missing drag handles and professional tools must be reported as gaps, not marked complete.
10. Verify the approved favicon in the same exact #35 Windows build using `index.html` and `tests/brandingFavicon.test.mjs`. **The Tauri/native `src-tauri/icons/icon.ico` remains a separate generator/package task** and should not be shown as fixed by this HTML change.
11. Run the opt-in worktree inventory `scripts/audit_qa_worktrees.ps1` in read-only mode. Attach before/after disk details and only remove exact approved, clean retired worktrees. Don't delete QA model copies or OneDrive files on assumption.
12. Post exact-head logs, before/after screenshots and image fixtures with hashes to PR #35 and Issue #29. Fix defects on #35 without opening another PR. Request independent actual human approval/Qortara GO; merge only in the authorized chain and rerun after main integration.

## Professional Photo Studio next implementation blocks

**P0 basic manipulation**: visual drag-to-crop overlay/handles plus full resolution presets, straighten, flip, resize and undo/redo, exposure/contrast/highlights/shadows/white balance/saturation, side-by-side comparison and per-photo versioning without overwriting originals.

**P1 core editor**: curves/levels, masks and selections, layers, retouch/heal, clone, perspective, color-space-safe history and output-profile handling, reusable recipes and project-owned versions.

**P2 deeper work**: batch workflows, AI inpainting/upscaling under qualified model/licensing/consent, private folder, advanced metadata handling, HDR, nondestructive durable scene/project asset graph.

A pass on these crop controls is a **usable incremental vertical slice**, not a qualified full professional Photo Studio. Keep Issue #29 open.
