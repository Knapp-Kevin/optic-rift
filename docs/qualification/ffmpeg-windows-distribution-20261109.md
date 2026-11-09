# FFmpeg Windows distribution decision (2026-11-09)

**Status:** candidate packaging path implemented on draft PR #37. **NOT** an authorized release. Nothing has yet been signed or qualified as a clean installer.

## Selected source and supported capability

- Supplier: Gyan Doshi, **FFmpeg 9.0.2 Release Essentials, Windows x64 static, GPLv3**, with libx264 and AAC encoding support advertised.
- Exact-version source archive: https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-9.0.2-essentials_build.zip
- Publisher SHA-256: `60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba` (publisher's corresponding .sha256).
- FFmpeg source tree: https://github.com/FFmpeg/FFmpeg/commit/946fcce07b
- Source selection rationale: preserve PR #37's already-tested frame-exact libx264 H.264 + AAC behavior with a smaller 'essentials' toolset rather than expand full-build dependencies or replace the encoder.
- **Never consume the mutable `ffmpeg-release-essentials.zip` alias without comparing its digest against the pinned exact-version archive.**

## Packaging procedure, on an isolated Windows build host

1. Acquire the exact-version supplier ZIP above and verify the publisher checksum through an independently trusted distribution channel. Save its license, source and build information.
2. From repository root run: `py scripts/stage_ffmpeg.py --archive <local-exact-version.zip>`. The script validates the 64-hex pinned archive checksum **before** extracting either executable. It stages only `ffmpeg.exe` and `ffprobe.exe` into `src-tauri/resources/ffmpeg/` with `provenance.json`; it refuses collisions and does not execute any supplier binary.
3. From the generated provenance file, independently recalculate each executable SHA-256 and set `OPTIC_RIFT_FFMPEG_SHA256` and `OPTIC_RIFT_FFPROBE_SHA256` in the release build environment. Both are required. Run `ffmpeg -version`, `ffmpeg -buildconf`, `ffmpeg -encoders`, and `ffprobe -version` **only in an isolated Windows qualification environment** after file identity verification; verify libx264, AAC, program bitness, signatures, PE imports and executable provenance. Static builds may still depend on Windows components.
4. Produce the Tauri Windows bundle through the fail-closed script: `npm run release:windows:ffmpeg`. This runs `python scripts/verify_ffmpeg_release.py` before invoking Tauri with the overlay. It refuses a missing/non-PE/symlinked executable, wrong supplier archive identity, missing build-time pins, unexpected resources, or any staged-binary hash mismatch. The overlay activates NSIS and maps staged paths directly to the expected `<resource-dir>/ffmpeg/ffmpeg.exe` and `ffprobe.exe`. Confirm actual resource paths in the installed output. Do not turn on a release channel until signing and compliance are complete.
5. Verify **the exact executable hashes baked into the application** against the installed files, both before first use and after tampering. Execute a real video trim through the installed app without FFmpeg on PATH. Confirm source unchanged, exact frames, duration, audio policy, provenance and output receipt. Confirm any invalid/missing/unapproved pair refuses without a file dialog or writing any files.
6. Sign the final bundled executables and installer under the approved publisher identity. **Signing changes file hashes:** sign *before* recording final production executable hashes and compiling the app, then validate final installer and installed files. If third-party signing changes the bytes, recalculate the hashes and rebuild; do not disable pin checks.
7. Update the Video Studio copy from 'Needs FFmpeg on this PC' to a bundle-specific message, qualify with Narrator/125% scaling, and test cancel/restart/disk-full/output-size/timeout failures. Post-merge acceptance and user rights checks are separate gates.

## Licensing and distribution obligations

FFmpeg and its libx264-enabled essentials build are GPLv3. Shipping standalone tools requires preserving the exact applicable copyright/license notices, complete corresponding source or compliant offer, source/patch/build configuration, component notices, and a redistribution plan. Verify the supplier's exact build configuration, transitive dependencies, absence of `--enable-nonfree`, and compatibility with Optic Rift's distribution terms. Do not assume that separate-process invocation automatically resolves every GPL boundary question. H.264/AAC patent licensing and commercial distribution exposure require separate evaluation. These are release decisions, **not** waived by a test pass or SHA digest.

Primary references: https://ffmpeg.org/legal.html ; https://www.gyan.dev/ffmpeg/builds/ ; https://v2.tauri.app/develop/resources/

## Explicit acceptance status

- [x] Candidate vendor and exact-version archive selected; published archive hash pinned in code
- [x] Fail-closed local archive extraction and provenance capture implemented
- [x] Resource mapping specified for Tauri NSIS
- [x] Pre-build executable/provenance/pin verification wired to `npm run release:windows:ffmpeg`; eight synthetic offline regression cases authored (not yet rerun on Windows)
- [ ] Approved source/binary redistribution evidence and license notices in installer
- [ ] Windows executable compatibility/codec audit at actual staged SHA
- [ ] Signing and repeatable release build with the two compiled-in executable pins
- [ ] Clean offline installed NSIS test and native FFmpeg tamper/absence refusals
- [ ] Video Studio release wording, accessibility and failure-recovery QA
- [ ] Genuine Qortara acceptance, parent #35 main integration and post-merge check

## Pre-build verification versus release qualification

The new `scripts/verify_ffmpeg_release.py` checks staged executable bytes, provenance, supplier archive identity, and build environment pins. Its checks are deliberately narrower than legal or production authorization. Run `python -m unittest discover -s scripts -p 'test_*ffmpeg*.py'` and then `npm run release:windows:ffmpeg` with explicitly supplied hashes. **Do not claim signing, correct x264/AAC codec configuration, valid supplier authenticity, redistributable source/notices, security of the installed files or successful installer behavior from this preflight alone.** The separate clean Windows VM acceptance and Qortara evidence are mandatory.

## PE/COFF structural release gate (2026-11-09)

The pre-build check now parses DOS and PE signatures, the bounded PE header offset, AMD64 machine type, PE32+ optional-header magic, executable COFF flag, section count, and a complete section table. It rejects a two-byte `MZ` impostor, an x86 executable, and a malformed/truncated PE. This is structural validation only. It does not establish Authenticode identity, security of a publisher, vulnerability status, or actual codec support. Run `python -m unittest discover -s scripts -p 'test_*ffmpeg*.py'` on the exact branch and preserve the output in the release receipt; the development environment exercised ten equivalent synthetic PE tests, but actual Windows release qualification remains outstanding.
