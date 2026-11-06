# PR #25 Create Studio: Windows after-screenshots (2026-11-06)

Evidence only. No source changes on this branch. This is not a review, approval or Qortara gate receipt.

| | |
|---|---|
| Application | PR #25 (`feature/create-studio-workspace-20261106-no-ci`), `tauri build --debug --no-bundle`. Screens 01–06: source tree of `0178749`. Screen 07: that tree plus the cancel fix, before the consent `max-height` rule. The final head `af9d9e8` differs from 01–06 only by Cancel handling and that one dialog rule. |
| Host | Windows 11 Pro 10.0.26300, RTX 5070 Ti, WebView2 157.0.4341.42 |
| Display scaling | **Actual 100 %** (window DPI 96). 125 % was **not** tested: changing Windows display scale is an owner action. |
| Viewports | Exact CSS content area 1280×720, 1440×900, 1920×1080 (client area set with `SetWindowPos`, verified with `innerWidth/innerHeight`) |
| Capture | WebView-only capture (`Page.captureScreenshot` over a loopback DevTools port added for the QA run only). No other window can appear. |
| Content | Screens 01–06 use an isolated QA profile (`studio.mythologiq.opticrift.qa`) with synthetic test-pattern PNGs (circle + grid + orange border) and valid provenance. A round circle and a fully visible border prove no stretch and no crop. Screen 07 uses the owner profile with history previews concealed. No real generations, tokens or personal paths are shown. |

## Screens

| File | State |
|---|---|
| `01-create-t2i-previews-concealed` | Create, text to image, history previews concealed (default) |
| `02-create-t2i-previews-shown` | After a deliberate **Show previews**: native ≤256 px thumbnails, letterboxed |
| `03-create-t2i-landscape-on-stage` | 1216×832 opened on the stage |
| `04-create-t2i-portrait-on-stage` | 832×1216 opened on the stage |
| `05-create-i2i-source-chosen` | Image to image, starting image beside the result slot |
| `06-edit-media` | Edit media: editor and timeline preserved |
| `07-gpu-consent-SIMULATED--1280x720` | GPU consent dialog. **Simulated**: the `gpu_check` IPC response was substituted and `generation_generate` was blocked; nothing ran on the GPU. |

QA profile note: the QA profile has no image runtime, so Create shows the honest "Set up an image model" state and Generate stays disabled. Create-as-default was verified separately in the owner profile (SSD-1B + runtime → Create; QA profile without runtime → Edit media).

## Measured results (all three sizes)

- Create view: page `scrollWidth`/`scrollHeight` equal the viewport (no page scroll, no horizontal overflow); settings panel scrolls on its own.
- Thumbnails decode at 256×107, 256×256, 256×175, 175×256 for 1536×640, 1024², 1216×832, 832×1216 sources; every rendered thumbnail is inside its tile.
- Stage: `object-fit: contain`; 832×1216 renders at 282×412 in an 848×412 box, 1536×640 at 848×353.
- Edit media fits 1440×900 and 1920×1080. At 1280×720 the existing editor inspector (history list) still makes the page 879 px tall (pre-existing editor layout).
- Keyboard: nav → Studio tabs → prompt → Generate → filmstrip → settings; visible 2 px focus ring. The hidden workspace is not focusable.

Defects found in the first pass and fixed in PR #25 commit `0178749`: clipped filmstrip tiles and labels, cropped thumbnails, Create state lost on Edit media switch, phantom image-to-image source in the dropdown, red error on an untouched prompt, consent focus, a 4 px settings overflow and the editor page overflowing by the tab-bar height.

## GPU smoke test (owner-consented, 2026-11-06, idle GPU)

Owner profile, app built from `af9d9e8`; screen 12 from `14c5748`. Fictional prompts only.

| File | State |
|---|---|
| `gpu-smoke/08-t2i-in-progress-BEFORE-overlap-fix` | SSD-1B text to image at step 4 of 25 (welcome copy showing through: fixed in `14c5748`) |
| `gpu-smoke/09-t2i-result` | Result `94e3a3f9`, 1024×1024, seed 3369929774 |
| `gpu-smoke/10-i2i-source-and-result` | Image to image from `94e3a3f9`, strength 0.65, 25 steps (16 effective) → `e7cbbd3b` |
| `gpu-smoke/11-cancelled-mid-denoise` | Cancel at step 6 of 25: stopped in 836 ms, no output |
| `gpu-smoke/12-progress-overlay-after-fix` | Progress card alone on the stage (job cancelled during verification, no GPU load) |
