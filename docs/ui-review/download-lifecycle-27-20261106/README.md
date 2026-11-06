# Issue #27 model-download lifecycle: Windows evidence (2026-11-06)

Evidence only; no source changes. Not a review, approval or Qortara receipt.

App: branch `feature/model-download-lifecycle-20261106-no-ci` at `8e25456` (stacked on PR #26 `a2cf033`), `tauri build --debug --no-bundle`, isolated QA profile, Windows 11 Pro 10.0.26300, WebView2 157, 1440×900 CSS, WebView-only capture.

**Synthetic IPC only.** `catalog_search`, `catalog_inspect`, `models_library`, `model_install`, `model_install_cancel` and `model_install_status` were answered or held at the network layer. `model_install` was held and later answered with a Tauri error response, so no request reached native install code and no model was downloaded. Progress events were emitted locally for the captured install id. The app opened **no non-loopback TCP connection** during the run.

| File | State |
|---|---|
| `nav-studio-optin` / `nav-projects-optin` / `nav-settings-optin` | Held download (1 GB of 4 GB) stays visible with Cancel in the navigation rail on every page |
| `studio-optout-2gb` | Adult discovery turned off mid-download: name hidden ("a model"), a 2 GB progress event still arrives |
| `models-other-model-inspected` | Back on Models with another model inspected: one status card, no rail duplicate, second install disabled |
| `studio-cancelling` | Cancel from the Studio rail card: exactly the live install id sent to `model_install_cancel` |
| `studio-adopted-after-reload` | WebView reloaded mid-download: the UI adopts the active download from `model_install_status` and can cancel it |

## PR #28 review edge cases (2026-11-06)

Same method: synthetic IPC only, emulating native `model_install_status` / `model_install_cancel`. Native was emulated as each version behaves: `source_flagged: false` before inspection at `8e25456`, `null` at `13e803a`. Fixture model `qa-fixture/flagged-video-model`, file `transformer/qa-sensitive-name.safetensors`.

| File | State |
|---|---|
| `edge-cases/09-BEFORE-8e25456-…` | After a reload during pre-inspection with discovery off: repo name **and** file path shown |
| `edge-cases/10-BEFORE-8e25456-…` | Native rejected Cancel: button stuck on "Cancelling…" |
| `edge-cases/11-AFTER-13e803a-…` | Same moment: "Downloading a model · file 2 of 6 · 1.00 GB of 4.00 GB", Cancel available |
| `edge-cases/12-AFTER-13e803a-…` | Native rejected Cancel: button re-enabled |
