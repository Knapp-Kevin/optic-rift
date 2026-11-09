# Video Studio → Photo Studio: Frame Capture Windows Qualification

**Status:** new cross-media implementation based on [PR #32](https://github.com/Knapp-Kevin/optic-rift/pull/32) head `39fd7ca03bc43a091965ef7056ec95c0ef62c84a`. Not independently reviewed, real-Windows-qualified, merged or post-merge verified. The qualified parent was **not changed**.

## Actual supported user outcome

1. In Video Studio import/open an **existing locally owned** MP4/WebM file already decoded by the Windows WebView.
2. Play or seek to a frame. The user explicitly selects **Capture frame to Photo Studio**.
3. The running video is paused. `videoFrameCapture.ts` validates the **exact selected blob URL**, readiness, not actively seeking, finite timestamp, decoded width/height (maximum **8192 px per edge / 20 megapixels**), then copies the currently decoded frame via the browser's 2D canvas. Refuse an invalid/oversized or unsupported frame without creating an asset.
4. A **PNG of at most 32 MiB** is added as an *in-memory, session-only* photo and opened in Photo Studio. The source video is not modified, decoded/downloaded again or re-encoded. No model/GPU inference, FFmpeg/audio processing, external network, remote fetch, native filesystem write or unsolicited preview is involved.
5. The Media Library and Photo Studio display the clip filename and frame time as **ephemeral session lineage**, explicitly not a persistent project record. If the user removes the source video, the extracted photo stays in session until removed/restart, but its source relationship is not a signed/provenance assertion.
6. The resulting PNG can be edited with the existing qualified shared photo canvas and saved only by separately accepting **Pictures/OneDrive export**, which may sync outside the device. Capture alone is never an export.
7. The selected video and photo are kept separately across navigation; capture uses the exact live video element and discards late encoder output if the user changes clips or leaves Video Studio.

**Out of scope:** timeline assembly, video trim export, animation, optical flow, AI image-to-video, frame-by-frame animation, color-managed grading, durable project assets, soundtrack/audio creative tooling, private vault and guaranteed frame-exact codec timestamps. Captured position uses `video.currentTime` with millisecond reporting; test against the displayed frame rather than claiming frame-perfect extraction for all codecs.

## Security, privacy and resource boundary

- Only browser-owned `blob:` local video sources and an exact `video.currentSrc === expectedBlobUrl` match. No arbitrary file paths or URL fetch.
- Require `readyState >= HAVE_CURRENT_DATA`, `!seeking`, no media error, and valid timestamp/dimensions. The button is disabled while loading/seeking or an encoding is in progress.
- The image canvas allocation is bounded to 20 megapixels; PNG output is limited to 32 MiB before any object URL or asset registration. A failed draw, tainted canvas, empty/wrong output or file switch must produce an error with **no new asset**.
- Capture is triggered by an explicit button; neither Photo nor Media Library load unsolicited video frame previews. React escapes source filenames, and only ephemeral source/timestamp metadata is held in app memory.
- Source video object URL is never revoked merely by capturing; removing the extracted image revokes only its object URL. Original user files remain untouched.
- Test failure modes without stress-loading the GPU and without opening private/mature media in screenshots. Synthetic non-sensitive samples only, with any adult discovery opt-out unaffected.

## Mandatory Windows acceptance, exact final SHA

1. Use a separate clean worktree and isolated QA profile. Fetch live PR #32 and this branch. Preserve the Windows-qualified parent at `39fd7ca`. Do not rewrite any parent branch or run GPU inference.
2. Execute real gates:
```powershell
npm ci
npm run test:content
npm run build
cargo fmt --all --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml --locked
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
```
Correct any errors, rerun the exact final SHA, and report all failures separately.
3. In the actual Windows app, import 2 synthetic non-sensitive clips (MP4/H.264 and WebM/VP9 if codecs supported), navigate to Video Studio, play/seek and click Capture at a nonzero frame. Verify the correct displayed image appears in Photo Studio, PNG dimensions are exactly the decoded frame's width/height, and source timestamp is shown. Include orientation and 90° Photo edit, export and exact output hash if owner explicitly accepts the Pictures/OneDrive dialog.
4. Compare decoded source frame against the captured PNG in a known static-color/grid fixture; measure max/mean color differences. A WebView canvas capture is a *new rasterization*, not necessarily source-lossless or ICC-preserving. Any codec-dependent mismatch is reported, not assumed zero. Measured on WebView2 157 (Windows 11, synthetic BT.709-tagged grid): VP9/WebM captures within 2 levels of the generated source on flat areas; H.264/MP4 captures shift saturated colours (pure red 255 → 233, green 0 → 20) although ffmpeg decodes the same file within 4 levels, so the shift is in the WebView's H.264 decode path, not in the capture code. Frame timing matched the encoded frame index in every case.
5. Test Preview → Photo → Video → Photo: the extracted photo and its edit values persist during the session; switching photos does not overwrite other edits. Reopening project after restart does **not** resurrect session-only frame assets. Removing a source clip must not silently delete a separately captured frame.
6. Rapid seek/capture: capture disabled while seeking/loading; read-only/unsupported format, corrupt video, no frame, zero dimensions, 8193px edge, 20MP+ frame, tainted draw, encoder null/wrong type/over 32MB, and stale video while `toBlob` is pending must not create a photo or unexpectedly switch pages. Verify failure text. No unexpected object URL leaks.
7. Confirm **Capture alone** writes no PNG/provenance files in Pictures, no model/runtime/GPU inference, and no outside network connections. Only after the separate Photo Studio Save consent should any Pictures export occur, and user can decline it with zero writes.
8. Test keyboard focus and action visibility at 1280×720, 1440×900 and 1920×1080, plus 1100×700 icon-only navigation and global model-download Cancel behavior with synthetic IPC. 125% system scaling only with explicit owner authorization to change Windows settings.
9. Publish screenshots (only synthetic media), source/asset hashes, measured frame time/geometry, negative tests, command results and exact commit SHA on the feature PR. Distinguish source/unit checks from a functioning Windows UI.

## Integration, merge and closure

This branch is stacked on PR #32 → #31 → #30 → #25 → #20 → #18 → #16 → #13 → #1, with PRs #26/#28 integrated into PR #32. The high-risk Qortara gate still requires a **distinct human reviewer and authorized provider/branch rules** ([Issue #2](https://github.com/Knapp-Kevin/optic-rift/issues/2)). No author self-approval, bypass, or fabricated receipt.

The full professional Photo Studio, Video Studio and cross-media workflow remains tracked by [Issue #29](https://github.com/Knapp-Kevin/optic-rift/issues/29) and stays open. The frame-capture increment is **not complete** until Windows-qualified, independently reviewed, Qortara-authorized, merged into `main`, and post-merge verified. Do not use a successful draft as a reason to close the broader design issue.
