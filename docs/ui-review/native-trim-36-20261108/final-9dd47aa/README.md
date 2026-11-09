# PR #37 final qualification at `9dd47aa`

**Head under test:** `9dd47aa434d1db33f482b130ddff915686e696db` (`feature/native-video-trim-20261108-no-ci`).

- It contains PR #35's head `d8c1c34`, and its source tree is identical to `1f2b450`, which was tested earlier (same tree hash).
- **Environment:** Windows 11 Pro 10.0.26300, WebView2.
- **App build:** a debug build in the isolated QA profile `studio.mythologiq.opticrift.qa` (exe SHA-256 `B7EC52D3…73C2`), driven over a loopback-only DevTools port.
- **Test media:** all synthetic. No GPU, model, network or paid service was used.

## 1. Gates (`logs/gates-9dd47aa.txt`, `logs/cargo-test-9dd47aa.txt`)

| Gate | Result |
|---|---|
| `npm ci` / `npm run build` | exit 0 / exit 0 |
| `npm run test:content` | **97/97** |
| `cargo fmt --check` / `cargo check --locked` | exit 0 / exit 0 |
| `cargo test --locked` | **115 passed**, 5 ignored, 0 failed; doc test 1 passed. All 11 native trim tests use real FFmpeg |
| `cargo clippy --all-targets --locked -D warnings` | exit 0 |

No fixes were needed, so nothing was pushed to the PR branch.

## 2. Photo Studio crop: real Windows qualification (`logs/crop-results-9dd47aa.json`, `shots/crop/`)

**Fixtures:** each pixel encodes its own source coordinates (R = x low byte, G = y low byte, B = high bits), so every exported pixel can be traced back to exactly one source pixel.

**Analysis:** the independent analyzer (`pngkit.mjs`, its own PNG decoder) reads every exported pixel. From those it works out the rotation and crop rectangle actually applied, and checks that **every pixel** is one exact rotate-plus-crop of the original.

**How each case was set:**
- Preset cases clicked the preset button.
- The edge sliders were moved with real arrow-key presses (25 basis points per press).
- Rotation used the Rotate button.

| Case | Source | Output | Crop applied (px L/T/R/B, rotated image) | Pixels off-map | Preview = export pixels |
|---|---|---|---|---|---|
| Square 1:1 | 1200×900 | 900×900 | 150/0/150/0 | **0** | **identical** (RGBA SHA-256) |
| Wide 16:9 | 1200×900 | 1200×676 | 0/112/0/112 | **0** | identical |
| Portrait 4:5 | 1200×900 | 720×900 | 240/0/240/0 | **0** | identical |
| Individual edges by keyboard (L 10%, T 5%, R 3%, B 2%) | 1200×900 | 1044×837 | 120/45/36/18 | **0** | identical |
| Rotate 90° + Square + left edge 10% | 1200×900 | 810×900 | 90/150/0/150 (rotation detected: 90) | **0** | identical |
| Rotate 270° + Wide 16:9 | 1200×900 | 900×504 | 0/348/0/348 (rotation detected: 270) | **0** | identical |
| Portrait source + Wide 16:9 | 900×1200 | 900×504 | 0/348/0/348 | **0** | identical |
| Large source, bounded preview + Square | 2400×1600 | 1596×1600 | 402/0/402/0 | **0** | preview is a 1397×1400 proxy with the same aspect, as designed |

- **Edge insets:** every one equals floor(size × basis points ÷ 10000) of the rotated image, matching the on-screen percentage readouts.
- **Provenance:** every export writes `.provenance.json` with:
  - rotation;
  - crop insets in basis points;
  - output width, height, bytes and SHA-256, matching the file;
  - the source SHA-256, matching the fixture.

  No receipt contains a path.
- **Originals:** the SHA-256 of all three source PNGs is identical before and after.
- **Declined consent:** "Save adjusted PNG copy" → decline gives no new file and no notice; nothing was written.
- **Requests:** the WebView made only two IPC calls (`generation_outputs`, `photo_export_copy`) and no other requests.
- **Consent dialog:** `window.confirm` was answered by the test harness, which also recorded the exact consent text. The native dialog itself was not clicked.

**Screenshots:** the bottom of each Photo Studio screenshot is cropped off at 772 px. That area holds the QA profile's "Recent generations" list of the owner's own prompts, which is not test material.

## 3. Native video trim/export recheck (`logs/trim-run-9dd47aa.txt`, `logs/trim-mutate-rerun-9dd47aa.txt`, `shots/trim/`)

An independent analyzer decodes every exported frame and matches it against the source. Every exported frame matched its source frame; the adjacent-frame margin was ≥ 0.22, and the receipt SHA matched the file.

| Source | Exported source frames | Size | Audio |
|---|---|---|---|
| H.264 with B-frames, non-keyframe cut | **37–83** (47, in order) | 640×360 | none |
| VP9 | **15–60** (46) | 640×360 | none |
| VFR | **7–45** (39, real timing kept: 2.067 s) | 640×360 | none |
| Rotated + one AAC track | **30–90** (61) | 360×640 upright | 1 track, 2.033 s, the same as the video |
| The same, through the **real Windows file dialog** (no QA seam) | **30–60** (31) | 360×640 | 1 track, 1.033 s |

**Repeat exports:** exporting the same range again gives a new file name, with identical bytes.

**Source permission:**
- Real dialog Cancel: "No file was chosen."
- A different file chosen: "not the clip open in Video Studio… Nothing was granted."
- **Source changed after the grant** (rerun alone): "The source video changed on disk after you chose it. Nothing was exported; choose it again." The next Save asks again and refuses the changed file.

  The first attempt in the full run overlapped a still-running damaged-file job and was refused as "another export running", so it was rerun on its own.
- Forged IPC, all refused:
  - an unknown grant;
  - an extra `path` field;
  - a traversal name;
  - a non-number time;
  - an unknown job;
  - no consent;
  - the wrong asset;
  - a simultaneous second start.

**Refusals that wrote nothing:**
- declined consent;
- two audio tracks;
- HDR or BT.2020;
- odd 641×361 size;
- a corrupt file (Save disabled);
- a damaged mid-stream range: `failed`, 404 decoder errors, nothing published.

**Cancel:** pressed from Projects through the top-bar chip, which was topmost. The job reached `cancelled` in **0.265 s**. Afterwards there were 0 FFmpeg processes, 0 staging folders and 0 new files.

**FFmpeg killed:** `failed` with "FFmpeg stopped unexpectedly… No file was saved". 0 staging, 0 files.

**App killed mid-export:**
- The Job Object ended FFmpeg with the app.
- The 11.8 MB staging file was deleted on restart.
- The job was marked `interrupted`, with a dismissible notice.
- **0 partial files in Pictures.**

**Keyboard:** Save trimmed video is reached in 27 Tab presses with a 2 px focus ring at 1280×720, 1440×900, 1920×1080 and 1100×700. It is topmost, and the page does not scroll sideways.

**Sources:** the SHA-256 of every fixture is identical before and after.

## 4. FFmpeg packaging, identity and licensing audit

| Question | Finding |
|---|---|
| Is FFmpeg packaged? | **No.** `tauri.conf.json` has `bundle.active: false` and no `resources`. No installer can be built from this branch, so a **clean offline Windows install was not testable**: no installer exists. |
| Clean-machine behaviour (simulated: every FFmpeg folder removed from PATH, no env override) | Save trimmed video refuses with "FFmpeg is not installed for Optic Rift, so videos cannot be trimmed yet." No job starts and no file is written (`logs/no-ffmpeg-9dd47aa.txt`, `shots/trim/no-ffmpeg-refusal.png`). |
| Executable identity | The SHA-256 is **established on first use** each session, from the resource folder, `OPTIC_RIFT_FFMPEG_DIR` or PATH, then re-verified before every job. This detects a swap mid-session, but it **does not prove** a trusted binary. There is no expected hash compiled into the app, no signature check and no version allowlist. |
| Release-build exposure | The PATH and `OPTIC_RIFT_FFMPEG_DIR` fallbacks are **not limited to debug builds**. A release build would run whatever `ffmpeg.exe` is first on PATH. |
| Binary tested | gyan.dev `ffmpeg-9.0.1-essentials_build` from WinGet. ffmpeg SHA-256 `72A489EC…6AA3`, ffprobe SHA-256 `19202B23…C52F`. **Authenticode: NotSigned.** |
| Licence of that build | `--enable-gpl --enable-version3`, statically linked with libx264, libx265 and others, so the binary is **GPLv3**. |

What redistributing that build would require:
- GPLv3 compliance for the FFmpeg binary: licence text, and the corresponding source or a written offer.
- A decision on H.264 and AAC encoder patent licensing.
- A MythologIQ Azure Artifact Signing signature on every shipped `.exe`, per the Windows signing standard.

**Owner decision needed:** one of these options (not a legal opinion):
- (a) an LGPL FFmpeg build using Windows Media Foundation encoders (`h264_mf`, `aac_mf`);
- (b) a GPL x264 build with full compliance;
- (c) download on demand, which fails the offline requirement.

**Required code change before any release, not made in this pass so the evidence stays exact-head:**
- In release builds, accept only the bundled binary.
- Check it against SHA-256 values compiled in at build time.
- Remove the PATH and env fallbacks from release builds.

## 5. Worktree hygiene (`logs/hygiene-*.txt`, `logs/storage-inventory.txt`)

**Audit:** `scripts/audit_qa_worktrees.ps1` was run read-only first.

| | Before | After |
|---|---|---|
| Git worktrees (including the primary) | **19** | **14** |
| Clean and eligible | 7 | 2 (`pr36` active; `ev36` holds this evidence) |
| Free space on C: | 1,095,393,685,504 B | 1,095,671,377,920 B (**≈265 MB reclaimed**) |

**Removed** after a `-WhatIf` dry run, using the script's opt-in `-Apply` with exact paths, Git's no-force `worktree remove`:

| Worktree | Head | Origin branch containing it |
|---|---|---|
| `ev33` | `1c729a9` | `origin/docs/ui-review-frame-capture-33…` |
| `ev35` | `bec5ffa` | `origin/docs/ui-review-whole-studio-35…` |
| `pr33` | `9e84a60` | `origin/feature/video-frame-to-photo…` |
| `pr34` | `3a99e6c` | `origin/fix/approved-optic-rift-favicon…` |
| `pr35` | `49b589b` | `origin/feature/video-shot-composer…` (and `origin/feature/native-video-trim…`) |

- Each was clean, including untracked files, and had 0 stashes.
- Its head was contained in that origin branch.
- Its evidence was already posted.
- All five local branches remain. No branch was deleted, no prune ran, and no force was used.

**Retained:**
- **11 leftover folders** (`ev32`, `integ`, `main`, `pr1`, `pr18`, `pr20`, `pr26`, `pr27`, `pr30`, `pr31`, `pr32`).
  - They are no longer Git worktrees: the `.git` file is gone. They contain about 0 MB, as empty folder skeletons.
  - The script mislabels them `DIRTY: retain` rather than `MISSING`; this is a minor script defect that fails safe.
  - Removing them requires an approved folder deletion, which the script deliberately does not do.
  - `git worktree prune --dry-run` lists their stale metadata.
- **Retained pending owner approval:**
  - the QA profile `%LOCALAPPDATA%\studio.mythologiq.opticrift.qa`, 14.4 GB, including model copies;
  - the real profile `%LOCALAPPDATA%\studio.mythologiq.opticrift`, 7.7 GB, which is never removed;
  - `src-tauri\target`, 25.7 GB of build output still in use;
  - `OneDrive\Pictures\Optic Rift`, 104 synthetic test exports, 29.6 MB;
  - the scratchpad fixtures and logs, 574 MB.

## 6. Qortara review gate (`logs/qortara-review-37.txt`)

```
qortara_logic_runtime.cli review evaluate --pr 37 --author Knapp-Kevin --surface highRisk --manifest governance/manifest.json
→ satisfied: false; quorum 1 not met (N_eff 0.00); unmet: deterministic-gate, adversarial-empirical, human-approval; predicate distinct-identity unmet
```

**No independent human reviewer exists:**
- The manifest's actor registry lists only `Knapp-Kevin` (maintainer).
- The repository's only collaborator is `Knapp-Kevin`, who is also the PR author.
- PR #37 has 0 review requests and 0 reviews.

## Remaining defects

- **P0 for release only, not reachable in debug QA:**
  - FFmpeg is unpackaged, unlicensed and unsigned.
  - FFmpeg identity is trust-on-first-use, with PATH and env fallbacks in release builds.
- **No P0 found in the tested behaviour.**
- **Minor:**
  - Aspect presets use 25-basis-point steps, so "Square 1:1" on 2400×1600 gives 1596×1600, and 16:9 on 1200×900 gives 1200×676.
  - The trim consent text numbers frames from 1 (38–84), while receipts use 0-based indices (37–83).
  - "Save adjusted PNG copy" sits above the crop controls.
  - The hygiene script labels folders without a `.git` file as DIRTY.

## Not covered

- Release build and installer.
- Narrator and 125% scaling.
- Disk-full, and the 30-minute, stall and size limits triggered live.
- The native confirm dialog for photo export was answered by the harness.
