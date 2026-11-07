# Photo Canvas Preview / Export Parity: Windows Qualification

**Current status:** code committed on a follow-up branch based on PR #30 `aa86872bd53b96f12e964c99d9eafaee16ed930c`. This work is **NOT** Windows-qualified, independently approved, merged, or accepted as color managed. The original PR #30 remains untouched and qualified at its original head.

## Goal

Close the observed quality gap in [PR #30's Windows report](https://github.com/Knapp-Kevin/optic-rift/pull/30#issuecomment-6698348229): live image preview originally used CSS brightness/contrast and rotation, while PNG export used a canvas painter. Saturated colors diverged by as much as 12/255 when comparing the on-screen edited result with a reopened exported image.

This change creates one shared `paintAdjustedPhoto` canvas path for preview and full-size PNG output. It decodes the source image once per selection, maintains a bounded 1400px temporary in-memory preview, redraws on slider change with a short debounce, and closes the ImageBitmap on selection change/unmount. The preview is aspect-contained without CSS rotation transforms. The full-resolution Save still uses PR #30's exact same consent-gated native export IPC.

**Important:** A common renderer does not automatically prove ICC/color-managed WYSIWYG across different OS monitors, image profiles, output bit depth, HDR or browser display pipelines. Verify before claiming a specific tolerance.

## Source and security contracts

- Existing images remain immutable. No additional disk storage, thumbnail file, cache, remote request, model download or GPU inference worker was added.
- Header-first PNG/JPEG/WebP validation and photo memory limits remain intact. Preview 1400px-long edge, never upscaled. Do not repeatedly decode large 20MP sources on each slider change.
- The preview's bitmap is closed when its source changes. An async load from a superseded selection must not display stale images or leak private content.
- Unsupported image formats remain viewable in the legacy read-only image stage; photo adjustments and export are not offered for those formats. They must not be silently reclassified as editable.
- Existing native Pictures export and OneDrive warning are unchanged; a preview is not a saved project version, and Pictures can sync externally.
- No standalone audio creative features.

## Local Windows qualification (CPU-only, no GPU work)

1. Use a **separate clean worktree** based on the current follow-up branch. Record exact head and compare it with PR #30 before changing anything. Do not modify other active PR branches.
2. Run the full normal gates and repair failures:
```powershell
npm ci
npm run test:content
npm run build
cargo fmt --all --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml --locked
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
```
Run pinned Python worker tests if integration changed their inputs. Do not infer passing Windows gates from isolated structural/pure-function checks.
3. Repeat PR #30's **same source images, same display, same adjustment combinations and pixel samples** (including saturated primaries and neutral grays), and measure: (a) shared preview canvas pixels, (b) the PNG re-opened through the WebView, and (c) the actual exported PNG's decoded RGB. Publish per-channel max/mean deltas for each, explicitly reporting any remaining conversion issue. Preserve source/output SHA-256 and no-overwrite guarantees. If possible, compare with a second color profile/monitor and state the ICC assumptions.
4. Functional: 0/90/180/270 degree crop-free display, portrait/landscape/square aspect, zoom-to-fit, adjustment slider input visibility at 1280x720, 1440x900 and 1920x1080 (actual Windows 125% scale only if owner authorizes changing the system setting).
5. Performance: 20MP synthetic photo and rapid slider changes; verify only one source decode per asset selection, no repeated full-res render on every adjustment, bounded proxy canvas dimensions, timely UI updates, and release of bitmap/memory after switching images. Do not perform GPU stress/OOM tests.
6. Privacy/security: switch photos while a decode is pending; no stale prior asset visible. Import an unsupported SVG safely and verify read-only preview without Save, with no script execution or remote fetching. Test PNG/JPEG/WebP header mismatch, malformed images, very large dimensions and animated WebP. Direct native export refuses invalid consent/IPC as before. Preview never writes files.
7. Existing workflows: imported, generated, reopened local image, direct image-to-image pipeline, navigation to Create and back, and any other affected photo action must keep its behavior. Real Windows after-screenshots must be reviewed alongside PR #30 evidence.
8. Publish exact SHA, commands and pass/fail totals, preview/export numeric deltas, screenshots, memory and failure observations, and independent reviewer/Qortara receipts (or explicit missing blockers). Correct implementation/test defects in this PR and rerun final head.

## Merge and closure

This PR is stacked on #30, which is stacked on #25 and its upstream chain. The independent human-review and authentic Qortara gates on Issue #2 remain binding; no self-approval or GitHub ruleset bypass. Do not alter the qualified head of #30 merely to make this follow-up look merged.

Merge only when authorized in dependency order. Verify photo behavior in the integrated Windows build on `main`. Keep [P0 design issue #29](https://github.com/Knapp-Kevin/optic-rift/issues/29) open: full professional Photo Studio, Video Studio and cross-media workflows are not delivered by this preview refinement.

**Done = reviewed, governed, merged to main, and post-merge Windows-verified.**
