# PR #37 FFmpeg trust hardening at `145ce61`

This commit (`145ce61c228ef6370c6b320f7433ae0bd8e78c5b`) changes only `src-tauri/src/video_trim.rs` and `src-tauri/build.rs`, on top of `9dd47aa`.

## What changed

- **Release builds** (`not(debug_assertions)`) run only `<resources>/ffmpeg/ffmpeg.exe` and `ffprobe.exe`. They do not use PATH or `OPTIC_RIFT_FFMPEG_DIR`.
- Both files must match SHA-256 values **compiled into the build**, from `OPTIC_RIFT_FFMPEG_SHA256` and `OPTIC_RIFT_FFPROBE_SHA256` set at build time.
- `build.rs` reruns the build when either value changes.
- **Hashes are checked before the binary is first run.** They are hashed again once FFmpeg has identified itself, and before every job.
- **Refusals:**
  - With no hashes compiled in, or a malformed one, a release build refuses: "This build of Optic Rift does not include an approved FFmpeg, so videos cannot be trimmed."
  - With hashes compiled in but a substituted file, it refuses: "The bundled FFmpeg does not match the copy approved for this build; export refused."
- **Debug builds** keep the developer search order (bundled folder, environment variable, PATH), and enforce hashes when a build carries them.

## Gates (`logs/gates-hardened.txt`, `logs/hard-build.txt`)

| Gate | Result |
|---|---|
| `npm ci`, build, `test:content` | exit 0, **97/97** |
| `cargo fmt --check` / `check --locked` / `clippy --all-targets -D warnings` | exit 0 |
| `cargo test --locked` | **116 passed**, 5 ignored, 0 failed; includes the new `release_policy_runs_only_the_bundled_pinned_ffmpeg` |
| `cargo clippy --release --locked -D warnings` | exit 0 |

## Real release builds on Windows (`logs/release-run.txt`, `shots/`)

Two optimised release builds were made in the QA profile:
- **Build A:** no hashes compiled in. Exe SHA-256 `98BDC2DA…54BE`.
- **Build B:** the hashes of the tested gyan.dev 9.0.1 build compiled in. Exe SHA-256 `47E81160…1710`.

FFmpeg was on PATH for every run. In each case Save trimmed video was pressed in the real app.

| Case | Result |
|---|---|
| A, no bundled copy | Refused: "does not include an approved FFmpeg". The file dialog never opened, no job ran, nothing was written |
| A, FFmpeg copied into the bundle folder | Refused in the same way |
| B, no bundled copy | Refused; PATH was ignored |
| B, `ffprobe.exe` replaced by another binary | Refused: "does not match the copy approved for this build"; 0 FFmpeg processes, nothing written |
| B, approved bundle, through the **real Windows dialog** | Exported source frames **30–60** (31 frames, upright 360×640, 1 AAC track of 1.033 s). The receipt's tool source is `"bundled with Optic Rift"`, with the pinned hashes. The output's SHA-256 (`8d69212f…`) is identical to the debug build's export of the same range |

**Debug smoke test after the change** (`logs/debug-smoke-hardened.txt`): an H.264 export still gives exact frames **37–83**, and its receipt SHA matches the file.

## Still open (owner decisions)

- Which FFmpeg build to ship, plus its licence and patent position. The tested build is GPLv3 with libx264.
- Adding it to `tauri.conf.json` `bundle.resources`.
- Setting the two hashes in the release pipeline.
- Signing the bundled executables with MythologIQ Azure Artifact Signing.
- The Video Studio note "Needs FFmpeg on this PC" will need new wording once FFmpeg ships inside the app.
