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

## PR #31 shared canvas preview (2026-11-06)

PR #31 (`feature/photo-canvas-preview-parity-20261106-no-ci`). Parity captures at `6f6649b`; layout and read-only captures at `3b9abce`. Same fixtures, display and settings as above.

| File | State |
|---|---|
| `pr31-canvas-parity/07-E2-canvas-preview-b150-c130-rot90` | Edited preview through the shared canvas painter |
| `pr31-canvas-parity/08-E2-exported-png-reopened` | The exported PNG reopened unedited: identical pixels on this display |
| `pr31-canvas-parity/09-svg-read-only--1440x900` | SVG: read-only, no adjustments or Save |
| `pr31-canvas-parity/10-layout--*` | Edit media with a rotated portrait at 1280×720 / 1440×900 / 1920×1080 |

## End-to-end photo journeys (2026-11-06)

QA profile; synthetic fixtures only (no inference, no personal outputs). Create and image-to-image use **simulated** generation IPC; history, export and reopen use the **real** native code. Captures from `3b9abce` (J1-J3) and `71642e3` (J5, restart).

| File | State |
|---|---|
| `journeys/j1-create-result` | Create result (simulated generation) on the stage |
| `journeys/j1-edit-exported` | Opened in Edit media, adjusted 130/120/90 deg and exported |
| `journeys/j2-history-edit-exported` | Stored output opened from Recent generations, adjusted 80/150/180 deg and exported |
| `journeys/j3-i2i-source-result` | Image to image: real stored source beside the simulated result |
| `journeys/j5-wrong-mime-refused` | Non-PNG result refused at the stage (fix `71642e3`) |
| `journeys/j-restart-reopened` | After an app restart, the three exports re-imported into Edit media |
