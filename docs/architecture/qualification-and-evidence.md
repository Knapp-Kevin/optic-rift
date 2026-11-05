# Architecture qualification and user-outcome evidence

- Status: **PROPOSED** verification contract; no stage/benchmark claims are accepted here.
- Claim owner: architecture-wide acceptance traceability and evidence handoff, not the test implementation or lifecycle ledger.
- Actual per-target thresholds remain owned by [draft performance gates in PR #1](https://github.com/Knapp-Kevin/optic-rift/pull/1), pending adoption.
- Qortara governance source: [manifest](../../governance/manifest.json), not this document.

## Required evidence classes

1. **Repository facts:** exact branch/tree/commit and files, pinned dependencies and build environment. No claims from screenshots alone.
2. **Deterministic tests:** executable probes with exit codes for project save/reopen, input validation, source protection, frame time, export media properties, cancellation and recovery.
3. **Windows device acceptance:** actual installed binary, Windows version, GPU/driver, codec samples, p50/p95 seek, dropped frames, audio sync, memory and resource contention when generation runs.
4. **User outcome walkthroughs:** a novice can complete a journey and inspect the final file, not just click a disabled button. Record failure/unclear points and corrective acceptance.
5. **Independent review and release authority:** reviewer identity distinct from author when required, revision-bound verdict, owner acceptance and Qortara lifecycle recording through an authorized host. None may be inferred from a policy text.

## Proposed journey coverage matrix

| Journey | Minimum verification | Critical refusal case |
| --- | --- | --- |
| No-AI editor | Import 2 clips, arrange/trim, export MP4, close/reopen project | No model installed: editor still functional |
| Exact frame to photo | Choose VFR source, extract frame, crop/zoom, export still; provenance links exact PTS | Timecode mismatch or unsupported stream yields clear error |
| Photo to concept | Create/refine two versions without touching original; saved reference roles | Missing model/rights never silently install |
| Image to video | Create selected image-to-video job, preview and cancel, save result | GPU OOM leaves project and source intact |
| Video -> frame -> next video | Reuse generated frame as new image reference and produce second clip; create sequence | No lost source edges across restart |
| Old footage restoration | Compare original, enhanced revision, extracted still; warnings on synthetic detail | Unsupported codec or model never claims success |
| Adult model browse | Adult discovery off by default, explicit reveal opt-in, independent rights state | No explicit autoplay or unexpected thumbnails |
| Model exploration | Discovered/unsupported/installed/ready reported separately, pinned version | Unverified model never auto-executes |
| Recovery | Kill app/worker in each job phase, reopen and inspect committed project | Partial output cannot be accepted asset |

## Technical acceptance contract

- Each test binds environment + artifact hash + commit/tree + command/tool version + observed result; change of source invalidates prior qualification where relevant.
- Frame extraction compares decoded pixel content, stream, presentation timestamp and crop geometry, not a screenshot of a paused player.
- Export validates decoded duration, resolution, frame count/timebase where applicable, audio sync and edits represented in the source timeline manifest.
- Verify malicious media, oversized frames, symlinks/path traversal, corrupt saved metadata, disk full and interrupted worker cleanup.
- Validate accessibility using keyboard and assistive technology alongside visual and performance probes.
- Capture license/terms, network egress and adult-preview behaviors as separately attributable tests; legal rights unresolved means not eligible for the corresponding intended use.

## Architecture review gates

- Proposed design in the single index -> Wayfinder decision issue and evidence -> owner acceptance / ADR where warranted -> implementation spec and tickets -> native and end-to-end tests -> independent assurance -> explicit publication decision.
- A proposed path, structurally valid manifest, mergeable PR or successful compilation does not imply all the later gates passed.
- A changed contract needs source-linked impacted subsystems and requalification, not copied old status.
- The program remains **NO-GO for production** until executable editor, signed packaging and required protections have been measured and accepted on exact artifacts.

## Current known absences (2026-11-05 source inspection)

- Main holds governance and README, not a compiled Optic Rift application.
- Desktop UI/ADR/performance ideas are on draft PR #1; no Windows app, media decoder/FFmpeg service or durable project is qualified there.
- No working model catalog, installed image/video adapter or actual cross-media job chain has been verified.
- Qortara issue #2 still tracks unimplemented hosted/native governance and GitHub enforcement; do not call it complete.

