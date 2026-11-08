# PR #33 video frame → Photo Studio: Windows evidence

Head under test: `9e84a6052dcd5555a030cf85a299d3ee9f10c8a6` (`feature/video-frame-to-photo-20261108-no-ci`). It is the PR's starting head `1736839` plus one fix commit. Parent PR #32 stays at `39fd7ca` and was not changed.

Environment: Windows 11 Pro 10.0.26300, WebView2 157, debug build in the isolated QA profile (`studio.mythologiq.opticrift.qa`), driven over a loopback-only DevTools port at 100% scaling. All media is synthetic and generated with ffmpeg 9.0.1. Screenshots are WebView captures (`Page.captureScreenshot`).

## Gates

| Gate | `1736839` (start) | `9e84a60` (final) |
|---|---|---|
| `npm ci` | exit 0 | exit 0 |
| `npm run test:content` | 71/71 | 71/71 |
| `npm run build` | exit 0 | exit 0 |
| `cargo fmt --check` | exit 0 | exit 0 |
| `cargo check --locked` | exit 0 | exit 0 |
| `cargo test --locked` | exit 0 | 99 passed, 4 ignored; 1 passed |
| `cargo clippy --all-targets --locked -D warnings` | exit 0 | exit 0 |

The logs are in `logs/gates-*.txt`.

## Fixtures (`logs/fixture-hashes.txt`)

- **Grid clips:** 640×360, 10 fps, 5 s, tagged BT.709. Eighty-pixel colour blocks fill the top. The bottom-left block is grey at a level of 5 × the frame index, so a capture shows exactly which frame it took.
  - `grid-h264-640x360.mp4`: H.264
  - `grid-vp9-640x360.webm`: VP9
- **Rotation:** `rotated-90-320x180.mp4` is coded at 320×180 with a 90° display matrix.
- **Bounds:** `edge-8193x16.webm`, `edge-8192x16.webm` and `pixels-5000x4002.webm` (20.01 MP).
- **Unplayable:** `unsupported-mpeg4.mp4` (MPEG-4 Part 2) and `corrupt.mp4` (random bytes).
- **Ground truth:** `fixtures/truth-f23.png` and `truth-f17.png` are frames 23 and 17 straight from the generator, with no codec involved.

## Results (`logs/final-run-9e84a60.txt`, `logs/fidelity.txt`)

| Check | Result |
|---|---|
| Capture H.264 @ 2.3 s | Opens in Photo Studio as `video-frame-00-00-02-300.png`. 640×360. The grey index reads frame 23, as expected. |
| Capture VP9 @ 1.7 s | `video-frame-00-00-01-700.png`. 640×360. Frame 17, as expected. |
| Capture during playback | Paused at 1.741 s and captured frame 17, which is floor(17.41). |
| Orientation | The rotated clip captures at 180×320, and all four corner colours match ffmpeg's autorotated decode. |
| Fidelity, VP9 | Against the generator, flat areas differ by at most 2 levels (mean 0.33). |
| Fidelity, H.264 | Flat areas differ by up to 22 (mean 4.7): pure red reads 233,0,2. ffmpeg decodes the same file to within 4 of the generator, so the shift comes from WebView2's H.264 decode path, not from the capture code. It is codec-dependent and reported as a limitation. |
| Same renderer | After 140% brightness, 120% contrast and a 90° rotate, the Photo Studio preview canvas PNG and the exported PNG are byte-identical (`a1807c70…`). |
| Lineage | Photo Studio and Media Library show the source clip and time, labelled "Session only" and "Not saved as project provenance". The export provenance records only the captured PNG's hash (`5c33e4c2…`), never the video's name or path. |
| Separate assets | Edits persist across Video → Media Library → Models → Photo. Two frames keep separate values (140/120 and 100 → 60). |
| Removing the source clip | The frame stays, still opens and keeps its edits. Only the clip's object URL is revoked. |
| Restart | The Media Library is empty afterwards: frames and clips do not persist (`capture/restart-library.png`). |
| Source files | SHA-256 of every clip is unchanged after all runs. |
| Disk writes | Capture plus a declined export: Pictures stays at 30 → 30 files, and the QA profile gained no PNGs. |
| One accepted export (start head) | Wrote `optic-rift-photo-efa3b057-….png` (8,636 bytes, 360×640, SHA-256 `a1807c70…`) and its `.provenance.json`. |
| Network, models, GPU | No non-IPC WebView request in any run. The IPC calls were only listings: `generation_outputs`, `models_library`, `catalog_capabilities`, `hf_token_status` and `project_list`. A harness blocker on generation and model commands never fired. Nothing produces audio. |

## Negative and race tests (`negative/`, final head)

| Case | Result |
|---|---|
| Unsupported codec / corrupt file | Shows the honest "can't preview" error, Capture is disabled, no asset is created. |
| 8193 px edge / 20.01 MP | "This video frame is too large to edit safely…" No asset, and the user stays in Video Studio. |
| 8192 px edge | Accepted at the boundary. |
| Failed draw (injected `SecurityError`) | "This video frame could not be read…" No asset. |
| Encoder returns null (injected) | "Could not encode this frame as a PNG." No asset. |
| Encoder returns JPEG or an empty PNG (injected) | "Could not encode this frame as a valid PNG." No asset. **Fixed:** this previously claimed the 32 MB limit was exceeded. |
| Encoder returns 33 MiB (injected) | "The captured PNG exceeds the supported 32 MB limit." No asset. |
| Seeking | The button switches disabled→enabled around every seek (mutation-observed). Clicking in the same task as a seek gives "Wait until the selected video frame is ready". |
| Double click | One asset. |
| Encoder held, then the user navigates away / switches clip / leaves and returns / removes the source | No asset, no page switch, no error message, and the label resets. |

"Injected" means the WebView's canvas `drawImage`/`toBlob` was temporarily replaced through DevTools to force a failure that real media cannot easily produce. These are fault injections, not real decoder failures.

## Layout and downloads

- **`layout/`:** at 1280×720, 1440×900 and 1920×1080 the Capture button is in the viewport, topmost, with no horizontal scroll.
  - Tab from the nav reaches it with a 2 px focus ring (`keyboard-*.png`).
  - At 1100×700 (icon rail) the tools panel stacks under the stage. The page scrolls (1201 px), and keyboard focus scrolls the button into view.
- **`downloads/`:** PR #28's synthetic held model install (IPC fixtures only) at 1100, 1280, 1440 and 1920. Status and Cancel are in the viewport and topmost on all six non-Models destinations, including Video Studio. The opt-out concealment, exact-ID cancel and reload adoption all pass.

## Not covered

- 125% display scaling: changing the Windows setting was not authorized.
- Live Narrator.
- Frame-exact timing on variable-frame-rate or B-frame-heavy real camera footage. Only synthetic constant-rate clips were used.
- The accepted export ran on the starting head's build. The fix commit changes only error wording, which was rerun on the final head.
