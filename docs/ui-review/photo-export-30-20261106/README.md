# PR #30 Photo Studio export: Windows evidence (2026-11-06)

Evidence only; no source changes. Not a review, approval or Qortara receipt.

App: PR #30 (`feature/photo-adjusted-png-export-20261106-no-ci`), isolated QA profile, Windows 11 Pro 10.0.26300, WebView2 157, actual 100 % scale, WebView-only capture. Fixtures are synthetic quadrant test patterns (top-left red, top-right green, bottom-left blue, bottom-right grey, white centre bar); there are no personal photos.

| File | State |
|---|---|
| `01-BEFORE-067885c-landscape-rot90-clipped` | 1200×800 at 90°: top and bottom cut off (≈1:1 instead of 2:3) |
| `02-AFTER-landscape-rot90-whole` | Same at `aa86872`: whole image at 2:3 |
| `03-AFTER-portrait-rot90-whole` | 800×1200 JPEG at 90°: whole image at 3:2 |
| `04-E2-preview-b150-c130-rot90` | Edited preview (brightness 150 %, contrast 130 %, 90°) |
| `05-E2-exported-png-reopened` | The exported PNG reopened unedited in the same preview |
| `06-layout--1280x720/1440x900/1920x1080` | Edit media with a rotated photo: page equals the viewport, inspector scrolls, Save visible |
