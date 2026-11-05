# Performance qualification gates

**Status:** Proposed targets, no measured results. Do not mark any gate passed from CI, a desktop screenshot, or a successful compilation alone.

## Target device

Windows 11, RTX 5070 Ti 16 GB, Ryzen 7 7800X3D. Include a second lower-end system before claiming general usability. Record OS, GPU driver, FFmpeg build and exact media/model checkpoints with every result.

## UI and media tests

1. Import 20 images and 10 video clips; verify app stays interactive, handles duplicate names, restarts without data loss, and can recover from disk-full/corrupted project indexes.
2. Play a standard 1080p H.264 30 fps local clip, no edits, for 5 minutes. Capture dropped frames, RAM, VRAM, CPU, GPU decode use and audio drift. Proposed target: smooth subjective playback and less than 1% dropped frames on the reference machine, **pending validation**.
3. Scrub a 30-second clip over 100 seeks; record p50/p95 seek-to-present time and response lag. **Provisional p95 target: 250 ms** with a generated proxy; compare against Qt if missed.
4. Drag an image adjustment control and resize editor panels under load. **Provisional target: p95 input feedback under 100 ms**, no UI hang longer than 500 ms.
5. Queue a heavy GPU model job while preview playback occurs. Evaluate GPU contention and VRAM reserve. The app must reduce preview workload or pause generation with a clear explanation rather than hang, crash or corrupt a project.
6. Preview non-standard media: variable-frame-rate videos, 4K input, unusual frame rates, rotation metadata, missing audio, short clips and invalid files. Native proxy conversion should produce stable preview where permitted.
7. Cancel a model worker mid-generation, force-kill it, restart app. Originals and saved project must remain intact, job state recover or fail clearly, stale model files cannot appear as accepted output.
8. Export an edited sequence through a native FFmpeg pipeline; verify actual video duration, dimensions, audio sync, frame count and manifest-to-output equivalence. Confirm encoder fallback when NVENC unavailable.

These are **proposed acceptance thresholds** rather than data or guarantees.

## Tooling and qualification

- Capture on-device traces using Windows Performance Recorder/Task Manager, FFmpeg logs and DevTools performance.
- Test at least one actual Windows Tauri binary, not just Vite in a standalone browser.
- No model weight downloads or remote calls by default.
- Measure memory and disk consumption against system capacity before default recommendations.
- Qualify ComfyUI only as an optional adapter; no workflow graph is required of users.

## Go/no-go

Do not declare Tauri unsuitable based on theoretical WebView overhead, nor suitable based on a static UI. A failed seek/preview gate should trigger a narrow native Qt comparison before a product-wide rewrite.
