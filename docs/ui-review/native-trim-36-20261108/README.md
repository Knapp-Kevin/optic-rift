# Issue #36 native video trim/export: Windows evidence

Head under test: `4f84d20d4843739072c6eee30627a3063cf3aba8` (`feature/native-video-trim-20261108-no-ci`).

The branch starts from PR #35's qualified head `49b589b` and merges PR #15's media module (`a07d37d`) in commit `0790b8a`. Neither of those branches was modified.

Environment:
- Windows 11 Pro 10.0.26300, WebView2 157.
- Debug build in the isolated QA profile (`studio.mythologiq.opticrift.qa`), driven over a loopback-only DevTools port at 100% scaling.
- FFmpeg 9.0.1 (gyan.dev essentials build) from PATH. It is a developer install, **not bundled**, and is labelled that way in every provenance receipt.
- All media is synthetic.

## Gates at `4f84d20`

| Gate | Result |
|---|---|
| `npm ci` | exit 0 |
| `npm run test:content` | **91/91** |
| `npm run build` | exit 0 |
| `cargo fmt --check` / `cargo check --locked` | exit 0 / exit 0 |
| `cargo test --locked` | **115 passed**, 5 ignored; 1 passed |
| `cargo clippy --all-targets --locked -D warnings` | exit 0 |

The native trim suite runs real FFmpeg: 11 tests, all passing. It covers:
- an exact non-keyframe cut on a B-frame clip;
- a VFR cut;
- rotation with audio, and refusal of two audio tracks;
- refusals that write nothing;
- HDR refusal;
- source mutation;
- cancel;
- restart reconciliation;
- FFmpeg substitution;
- junction refusal;
- four simultaneous starts giving one job;
- damaged source frames;
- plain failure text.

## Fixtures (`logs/fixture-hashes.txt`)

The counter clips are 640×360 at 30 fps for 4 s (120 frames). Each frame has colour bars on top, and a block whose grey level is 2 × the frame index.

| Fixture | What it tests |
|---|---|
| `h264-bframes-counter.mp4` | H.264 with B-frames (`-bf 3`) and keyframes every 48 frames, so the in/out points are non-keyframe |
| `vp9-counter.webm` | VP9 |
| `vfr-counter.mkv` | Variable frame rate: even frames only in the first 2 s, then every frame |
| `rotated-audio-counter.mp4` | A 90° display matrix plus one AAC track |
| `two-audio.mkv` | Two audio tracks |
| `hdr-tagged.mp4` | PQ / BT.2020 tags written into the bitstream |
| `odd-641x361.mp4` | Odd dimensions |
| `corrupt.mp4` | Random bytes |
| `damaged-middle.mp4` | Valid container with corrupted bytes mid-stream |
| `long-1080p-120s.mp4` | 3600 frames, for Cancel and crash tests |

An independent analyzer (`an36.mjs`) decodes each exported file and each source at 1/8 scale. It matches every exported frame to the nearest source frame; the adjacent-frame margin is ≥ 0.22, with no ambiguity.

## Results at `4f84d20` (`logs/app-run-4f84d20.txt`, `logs/app-rerun-4f84d20.txt`)

### Exact exports (UI markers → native frames → new MP4)

| Source | Markers | Exported source frames | Size | Audio | Receipt |
|---|---|---|---|---|---|
| H.264 B-frames | 1.233 → 2.767 s | **37…83** (47, every one in order) | 640×360 | none | hash matches; range 37–83 |
| VP9 | 0.5 → 2.0 s | **15…60** (46) | 640×360 | none | matches |
| VFR | 0.5 → 2.5 s | **7…45** (39, real timing kept: 2.067 s) | 640×360 | none | matches |
| Rotated + audio | 1.0 → 3.0 s | **30…90** (61) | **360×640 upright** | 1 AAC track, 2.033 s, the same as the video | matches |
| Same as above, through the **real Windows dialog** (no seam) | 1.0 → 2.0 s | **30…60** (31) | 360×640 | 1 track, 1.033 s | matches |

- **Repeated exports:** exporting the same range again gives a new `optic-rift-trim-<uuid>.mp4`; earlier outputs are unchanged.
- **Sources:** the SHA-256 of every fixture is identical before and after all runs.
- **Provenance:** each receipt records:
  - the source file name, size and SHA-256 (verified natively before and after encoding);
  - the frame indices, PTS and time base;
  - the output SHA-256, size, frame count and duration;
  - the audio policy and the tool identity (version, FFmpeg/ffprobe SHA-256, and where it came from).

  It never records the source's full path.

### Refusals: nothing written, no job started

- Declining the Pictures/OneDrive consent: "Export declined. Nothing was written."
- Real dialog **Cancel**, and the seam's cancelled dialog: "No file was chosen. Nothing was exported."
- A chosen file that is not the clip open in Video Studio (different size): refused, nothing granted.
- Two audio tracks, HDR/BT.2020, 641×361 and a corrupt file: each refused with its own explanation. A corrupt file can't preview, so Save is disabled.
- **Damaged mid-stream frames:** FFmpeg reported 404 decoding problems, so the job **failed** with "The source video has damaged frames in this range…". Nothing was published.
- **Source changed after the grant:** "The source video changed on disk… choose it again." The next Save reopens the dialog, and the changed file is then refused as not matching. A 50 ms trace was captured at the final build.
- **Forged IPC:** all of these are refused:
  - an unknown grant;
  - an extra `path` field on start or grant;
  - a traversal name;
  - a non-number time;
  - an unknown job for reveal or cancel;
  - a live grant used without consent;
  - a live grant with the wrong asset ID.
- **Simultaneous starts:** two starts sent together produce exactly one job; the other is refused as already running.

### Job lifecycle

- **Cancel:** pressed from **Projects** through the top-bar chip, which was topmost. The job reached `cancelled` in 0.27 s; no FFmpeg was left, no staging, and nothing published.
- **FFmpeg killed mid-encode:** `failed` with "FFmpeg stopped unexpectedly before the export finished (exit code -1). No file was saved." No staging was left and nothing was published.
- **Optic Rift killed mid-export:** the Job Object ended FFmpeg with the app (pid gone). On the next start the job was marked `interrupted`, the 17 MB staging file was deleted, no partial reached Pictures, and Video Studio shows a dismissible notice.

### UI

- At 1280×720, 1440×900, 1920×1080 and 1100×700, Tab reaches **Save trimmed video** (27 presses from the nav) with a 2 px focus ring.
- At 1100×700 the tools panel stacks under the stage, and focus scrolls the button into view.
- The **Video export** chip in the top bar is separate from the model-download card, and stays on every page while an export runs.
- No non-IPC WebView request occurred in any run.

## Defects found during qualification and fixed on this branch

| Commit | Defect |
|---|---|
| `80c2eb0` | The FFmpeg-pin test raced the job tests and made one of them fail intermittently |
| `7a3e7fb` | BT.2020 wide-gamut sources were treated as SDR. A refusal notice also stayed after switching clips |
| `47c59da` | Two simultaneous `video_trim_start` calls both ran: the check and the registration were not atomic |
| `e020fdd` | After a source changed, Save kept reusing the stale grant, so the dialog never reappeared |
| `e49b492` | A killed FFmpeg read "could not encode this video (exit Some(-1))" |
| `f53647f` | **FFmpeg concealed corrupt source frames and the export counted as verified.** Any decoder message now fails the job |
| `4f84d20` | The after-restart interrupted notice could not be dismissed |

## Not covered or not done

- **FFmpeg packaging and rights:** FFmpeg is not bundled. The resource-folder lookup exists, but no build is chosen, licensed or shipped. A clean offline Windows install therefore cannot trim yet. This is the main open item.
- **Release build and signing:** only debug builds were tested. The dialog seam (`OPTIC_RIFT_QA_PICK_VIDEO`) exists only in debug builds. Most app runs used it; the real dialog was verified separately for both Cancel and choose.
- **Untested failure paths:** disk-full, the 30-minute wall clock, the 120 s stall limit and the output-size limit have code paths, but none was triggered live.
- **Narrator and 125% display scaling:** not tested.
- **Project assets:** the exported copy is not a project asset (decision #5).
- **Refused by design:** HDR or BT.2020, odd dimensions, more than one audio track, 10,000 frames or more, and files over 8 GB. Each is refused with a message.
- **Debug-build speed:** the source dialog takes about 4 s, because the pinned FFmpeg and ffprobe hashes are recomputed in unoptimised code.
