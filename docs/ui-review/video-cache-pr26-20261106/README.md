# PR #26 video metadata cache: Windows evidence (2026-11-06)

Evidence only; no source changes. Not a review, approval or Qortara receipt.

App: PR #26 `37742be` (`tauri build --debug --no-bundle`, isolated QA profile `studio.mythologiq.opticrift.qa`), Windows 11 Pro 10.0.26300, WebView2 157, 100 % scale, 1440×900 CSS viewport, WebView-only capture. Public model names only; no images, previews or weights.

| File | State |
|---|---|
| `01-cached-after-restart` | Wan-AI/Wan2.1-T2V-1.3B-Diffusers@0fad780a after restart: cached and verified, not installed, Clear offered |
| `02-consent-offered` | Lightricks/LTX-Video (image-to-video, public): license + transfer consent, button disabled until both are ticked |
| `03-consent-ticked-by-keyboard` | Both boxes ticked with Space/Tab only; Cache button focused with 2 px ring (not clicked) |
| `04-gated-refused` | stabilityai/stable-video-diffusion-img2vid-xt-1-1 (gated: auto): refusal copy, no controls |

## Adult-discovery opt-out regression (frontend, 2026-11-06)

QA profile, synthetic IPC fixtures only (`qa-fixture/*`; no real search, inspection, model download or images). `catalog_search`, `catalog_inspect`, `models_library`, `model_install` and `model_install_cancel` were answered or held at the network layer; progress was emitted locally.

| File | State |
|---|---|
| `adult-optout/05-optin-flagged-fixture-installing` | Opted in; flagged fixture inspected; fixture download at 1 GB of 4 GB |
| `adult-optout/06-BEFORE-ec9e7f9-…` | **Defect at `ec9e7f9`**: after opt-out the flagged detail is gone, but so are the progress bar and every Cancel control while the download is still running |
| `adult-optout/07-AFTER-a2cf033-…` | Fixed in `a2cf033`: "Downloading a model" (name hidden), progress continues to 2 GB, Cancel download reaches native |
| `adult-optout/08-local-installed-flagged-…` | Discovery off; an installed local flagged model is still inspectable via Details |
