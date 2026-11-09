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

## Native WebView2 privacy regression gate (observed Windows result)

**Evidence scope (2026-11-05):** Claude's owner-hosted Windows walkthrough at desktop foundation `984bd670c0441300b7566836ac0d89e0b398e9d3` (included in integrated media descendant `de1663820f9c241b7dc6e4bd1f4e433391df76bb`) recorded:

- Windows 11 Pro build 26300, WebView2 runtime `156.0.4314.58`; on the native **release build** launched using application configuration, not a Vite-only browser.
- WebView2 process tree: no observed TCP or UDP endpoints in a 120-second soak; no `oneauth.dll` loaded. Prior runtime behavior included Microsoft-account and background-network requests.
- Interactive 26/26 walkthrough passed on the combined branch; seek p50/p95 28/87 ms, 0 dropped frames in the observed sample.
- Source: [PR #1 local Windows qualification](https://github.com/Knapp-Kevin/optic-rift/pull/1#issuecomment-6672658605). These are **specific observed results**, not universal privacy or performance guarantees. Full logs remain on the tested host.

**Release and runtime-update acceptance gate:**

1. Identify executable commit, signed/unsigned package state, Windows build, user profile, WebView2 **actual installed runtime version**, Tauri version, FFmpeg, driver and environment. A code-only test is not enough.
2. Launch a **release build** with no explicit Hugging Face or other network action. Confirm the browser command-line switches are effective and OneAuth/WAM components are not loaded. Cover both initial startup and media import/playback.
3. Observe **all app-owned processes including WebView2 helpers** for at least 120 seconds, measuring outbound TCP and UDP at startup, while idle and during a complete click-through. Distinguish no *observed socket endpoints* from a comprehensive packet/ETW trace, since periodic polling can miss short-lived connections. Document the collection method and uncertainty.
4. Treat any unexpected egress as a privacy regression until explained and owner-approved. Deliberate user-initiated external actions, including **Get Hugging Face token** and **Test connection**, must have separate, explicit authorization and observations; they do not make passive background traffic acceptable.
5. Run `npm run test:content` with `tests/webviewPrivacy.test.mjs` and validate that removal of **each** required startup switch fails the configuration guard. This test checks configuration only; it cannot prove the runtime honored undocumented Edge flags.
6. **Repeat the release-build soak after every WebView2/Tauri runtime update and for every release candidate.** Undocumented `ms*` feature names can disappear or change without a compile failure. Re-evaluate whether disabling component updates has security/patching consequences; do not assume privacy and patch security are interchangeable.

**Current qualification:** the measured configuration qualified on the named host/runtime at `984bd67`; the stronger per-switch mutation guard was subsequently added and must be re-run on the current branch head. The project is **not** signed/packaged or generally qualified for other WebView2 versions by this observation.
