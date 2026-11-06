# PR #26 video metadata cache: Windows evidence (2026-11-06)

Evidence only; no source changes. Not a review, approval or Qortara receipt.

App: PR #26 `37742be` (`tauri build --debug --no-bundle`, isolated QA profile `studio.mythologiq.opticrift.qa`), Windows 11 Pro 10.0.26300, WebView2 157, 100 % scale, 1440×900 CSS viewport, WebView-only capture. Public model names only; no images, previews or weights.

| File | State |
|---|---|
| `01-cached-after-restart` | Wan-AI/Wan2.1-T2V-1.3B-Diffusers@0fad780a after restart: cached and verified, not installed, Clear offered |
| `02-consent-offered` | Lightricks/LTX-Video (image-to-video, public): license + transfer consent, button disabled until both are ticked |
| `03-consent-ticked-by-keyboard` | Both boxes ticked with Space/Tab only; Cache button focused with 2 px ring (not clicked) |
| `04-gated-refused` | stabilityai/stable-video-diffusion-img2vid-xt-1-1 (gated: auto): refusal copy, no controls |
