# ADR-0001: Windows-first desktop architecture

- Date: 2026-11-05
Status: Proposed
- Qualification: initial Windows host validation exists at specified revisions, but this architecture is not accepted for a signed release.
- Decision owner: repository owner
- Scope: initial Optic Rift alpha, not a release claim

## Decision

Use **Tauri 2** for the desktop application boundary, **React + TypeScript** for the creative studio interface, **Rust** for native coordination/project operations, **FFmpeg** for native media processing, and **isolated Python/PyTorch workers** for each qualified inference engine. Windows 11 first. Projects and media remain local by default.

This is a DESKTOP application using the OS WebView2 for interface composition, not a hosted website or localhost server product. There is no requirement to run a network-facing server.

## Why

A user should interact with an image/video workspace, integrated player, timeline and model library without knowing node graphs. Rapid iteration of accessible creative controls favors a TypeScript component UI. Native file management, job control and crash isolation favor Rust. Model execution remains Python because upstream CUDA/video diffusion support is predominantly maintained there.

**Do not decode or transcode a video on the TypeScript main thread.** Provide playable preview media and frame thumbnails through a background native pipeline, and use hardware acceleration when supported. Preview caches should be bounded, purgeable and separate from originals. Implement actual media/export operations in the native layer rather than treating CSS image filters as exported edits.

## Alternatives

| Option | Advantage | Tradeoff | Decision |
| --- | --- | --- | --- |
| Web browser with local server | Easy previews and sharing | Browser-centric local file and process lifecycle; distribution complexity | No |
| PySide6 + QtMultimedia, as Sonic Rift | Mature, low-level frames/player, proven team code pattern | More custom editor UI effort; Qt licensing/packaging review | **Fallback** |
| Rust-only GUI (egui/iced) | One native language and broad native control | Less mature and less approachable video editor/timeline design ecosystem | No |
| Electron + React | Familiar desktop UI ecosystem | Bundled Chromium/Node; no useful inference advantage | No |
| **Tauri 2 + Rust + React** | Native coordination plus flexible UI; Windows WebView2 hardware acceleration | Must qualify scrub/frame timing, memory, high-DPI behavior | **Provisional yes** |

If the video/player benchmark fails with WebView2, **do not discard the Rust model/project core**. Evaluate a Qt Quick/QML native playback frontend, or a narrowly scoped native video preview surface, at the same project/API boundary. Do not rewrite everything on an intuition.

## Core boundaries

1. React/TypeScript: accessible visual studio, media bin, project workspace, capability/task selection, responsive controls. No model weights or unbounded binary media in React state.
2. Rust: trusted IPC, project transactions, path isolation, undoable edit manifest, durable job state, worker supervision, media discovery and playback proxy management.
3. FFmpeg: decode/thumbnail/proxy/transcode/render in a subprocess or native worker. Discover installed hardware codec support at runtime; CPU fallback.
4. Per-model Python workers: pinned dependencies, separate environments, explicit launch/cancel/progress/health contract. No arbitrary Hugging Face repository code execution from discovery.
5. Hugging Face discovery: metadata and explicit authentication (OS credential storage). Capability tags are **unverified hints**, not adapters or guarantees.
6. Project persistence: SQLite index (planned) plus a versioned non-destructive edit manifest and assets on disk. Native project files never silently overwrite user originals.

## Security and licensing

Tauri commands require validated arguments and narrow capabilities. No unrestricted shell/FS from the webview. Never silently install Python packages, run model-card scripts, execute remote code, download weights, accept model terms, or make cloud inference calls. Token must not enter project manifests, app logs or analytics. Show gated/license/weight-base lineage before consent; distinguish display-only discovery from locally qualified inference. Safe formats and FFmpeg binary license/source must be audited before distribution.

A model is not operational merely because Hugging Face indexes it or because one developer ran an adjacent checkpoint.

## Local first-run UX

Open the app into a ready-to-use studio. Import a local image/video; preview it. Show an editable sequence only when an edit is real and can be exported. Generative actions explain which missing engine/model prevents execution and provide a non-destructive path to install with user approval. Provide presets and human descriptions before exposing advanced controls.

## Content presentation and model freedom

The studio supports an opt-in adult-oriented model discovery experience, with content labels separate from license, task capability, execution readiness, and model-family metadata. The underlying architecture does not require an adult-content filter on ordinary local editing. The UI must not display explicit model thumbnails by default or accidentally reveal them through recommended-model surfaces, notifications, recent-history panels, or background previews. See [content contract](../contracts/adult-content-and-preview.md). Protect minors and real people against nonconsensual sexual material and meet applicable law and model license terms. The pre-alpha preference is session-only and is not an age-verification system.

## Qualification before a release recommendation

See [performance gates](../engineering/performance-gates.md). Measure on the owner Windows 11 / RTX 5070 Ti 16 GB machine, **not** hypothetical Rust/Tauri benchmarks. Compare Qt only if native playback measurements or usability justify the extra implementation.

## References

- Sonic Rift model/worker architecture: https://github.com/Knapp-Kevin/sonic-rift
- Tauri architecture: https://v2.tauri.app/concept/architecture/
- Tauri security capabilities: https://v2.tauri.app/security/capabilities/
- Microsoft WebView2 performance: https://learn.microsoft.com/microsoft-edge/webview2/concepts/performance
- Qt Multimedia: https://doc.qt.io/qt-6/videooverview.html
- FFmpeg: https://ffmpeg.org/ffmpeg-all.html
- Hugging Face Hub API: https://huggingface.co/docs/huggingface_hub/main/package_reference/hf_api
