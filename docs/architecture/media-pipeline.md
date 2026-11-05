# Media playback, precision editing, extraction and export

- Status: **PROPOSED**; actual native playback and performance are unmeasured.
- Claim owner: video/image media processing and timeline/render equivalence.
- Decision: [Windows media boundary qualification](https://github.com/Knapp-Kevin/optic-rift/issues/6).
- Depends on [project lineage](project-asset-lineage.md). Existing performance goals and desktop fallback belong to [draft PR #1](https://github.com/Knapp-Kevin/optic-rift/pull/1), not to this file.

## One consistent media path

~~~mermaid
flowchart LR
 A[Imported image/video] --> P[Probe metadata and codec]
 P --> Q[Bounded proxy and thumbnail cache]
 Q --> UI[Preview and frame navigator]
 UI --> R[Non-destructive edit recipe]
 R --> C[Native composition and export]
 A --> C
 C --> V[Validate decoded output]
 V --> O[Derived asset + project provenance]
~~~

**Media service API (candidate):** probe, requestPreview, seekToPresentationTime, enumerateFrames, extractFrame, renderImageRecipe, renderTimeline, cancel, inspectJob. Requests include project/asset ID and expected revision, never arbitrary shell strings. A response reports actual frame PTS/source stream and error category.

## Exact-frame versus enhanced-frame semantics

- **Original frame:** decode the actual source video stream at the requested presentation position, honoring stream selection, rotation/color metadata and variable frame timing. Do not claim there is one frame at every nominal 1/30-second instant.
- **Enhanced frame:** extract from a named enhanced video revision, or run an independently named enhancement on the original decoded frame. Never blur provenance.
- **Zoomed photo from video:** source-frame selection -> crop rectangle in source pixel coordinates (account for rotation/display aspect) -> optional scale/enhancement -> exported image with referenced original frame and crop recipe.
- **Contact sheet:** progressive bounded thumbnail requests over a selected interval; users may inspect sharper source frames before committing a still.
- **Frame interpolation:** outputs generated intermediate frames, distinct from captured footage. Label as synthetic and retain algorithm/model metadata.

## Preview and export separation

Preview can use hardware decode and lower-resolution proxies for responsiveness; **export must render source inputs and exact accepted edit recipes**. A CSS filter or UI transform is never accepted as the exported artifact. Explicitly qualify audio resampling, timebases, framerates, VFR-to-CFR conversion, aspect ratios, rotation, color spaces, transitions, subtitle paths and licensing of the bundled FFmpeg build.

Provisional data flows:
- Still image -> crop/rotate/basic correction -> export still.
- Existing video -> trim/concatenate/reframe/audio -> export video.
- Extract video frame -> ordinary image asset -> reference/edit/export.
- Video enhancement -> offline job with checkpointed input -> validated new video asset (not mutation of original).
- Generated video -> same player, bin and timeline as imported footage.

## Boundary alternatives

1. **Preferred:** responsive UI shell + Rust-owned media commands + controlled FFmpeg subprocesses/proxy cache. Tauri/WebView2 player only where measured qualified.
2. **Fallback:** Qt native playback or a native embedded preview surface if repeated p95 seeks/frame presentation fail. Keep project/media interfaces and export logic; avoid rewriting model execution.
3. **Not viable:** UI-thread decode, persisting only CSS preview effects, extracting frames via desktop screenshot, or treating a browser video element's approximate currentTime as frame-accurate capture.

## Physical and resource limits

- Constrain decoder concurrency, memory/VRAM budget, proxy size and thumbnail burst; priority to user controls and playback over background thumbnails.
- Distinguish CPU fallback from GPU encode/decode. Never assume NVENC exists based solely on an NVIDIA GPU.
- Local caches are disposable; source files, project records and completed outputs are not.
- Never trust video metadata alone: malformed files, huge frame dimensions, decompression bombs, path traversal in names and FFmpeg parser failures must be bounded.

## Qualification, not speculation

On an actual Windows Tauri binary, measure playback, p95 scrubbing, UI responsiveness, frame-source accuracy, timeline-render parity, output audio sync and memory while a GPU job runs. Use the exact acceptance thresholds and device receipts in draft PR #1's performance-gates owner; do not duplicate numeric thresholds here. Compare Qt only after observed failure. Include visual regression and independent users for ambiguous image-quality claims.

**No benchmark was run or claimed by this proposal.** The native shell decision remains provisional until the Wayfinder prototype/qualification ticket is resolved.

