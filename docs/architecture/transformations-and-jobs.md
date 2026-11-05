# Cross-media transformations and durable job boundaries

- Status: **PROPOSED**, not a committed API or implemented scheduler.
- Claim owner: operation interface, job lifecycle and transformation coordination.
- Wayfinder decision: [Transformation job boundary](https://github.com/Knapp-Kevin/optic-rift/issues/7).
- Companion owners: [project identity](project-asset-lineage.md), [media implementation](media-pipeline.md), [model trust](model-runtime-and-trust.md).

## A user action is not a model command

The same creative intent may have a **deterministic** executor (crop, extract captured frame, trim, export) or an **optional generative** executor (inpaint, upscale, generate alternate angle, animate, video restoration). The studio first expresses *what the user wants*, then an authorized capability/adapter selects *how to do it*.

Proposed normalized request fields:
- request ID and project/revision precondition;
- operation kind (extract-frame, crop, image-to-image, image-to-video, video-to-video, enhance-video, export-sequence, etc.);
- typed input asset IDs plus role-qualified reference edges (composition, style, character, start frame, end frame);
- chosen preset and validated parameters with versioned recipe;
- optional model+adapter identity at immutable revision, rights/consent evidence when required;
- output expectation, local destination scope, user-visible privacy classification and job priority.

Output record: operation/attempt ID, source and reference versions, executor identity, timings and environment facts as available, output byte hashes/metadata, diagnostics and completion status. Unknown is valid metadata, not a fabricated pass.

## Candidate state machine

~~~mermaid
stateDiagram-v2
 [*] --> Planned
 Planned --> Blocked: missing rights/adapter/disk/VRAM
 Planned --> Queued: validated and authorized
 Blocked --> Planned: user resolves blocker
 Queued --> Running
 Running --> Cancelled: cancel acknowledged
 Running --> Failed: worker crash or validation error
 Running --> Staging: output produced
 Staging --> Verified: output integrity and format checks
 Staging --> Failed: invalid/partial output
 Verified --> Committed: atomic project transaction
 Committed --> [*]
 Cancelled --> [*]
 Failed --> Planned: explicit retry as new attempt
~~~

The source remains intact across all states. Cancellation is a **request**, not proof a worker stopped; use verified process termination/cleanup. A retry is a distinct attempt and must not silently double-charge providers or duplicate a stored output. If a network response is ambiguous, surface it rather than blindly repeating a paid job.

## Candidate executor interfaces

- **Deterministic media executor:** FFmpeg/native image operations, no Python or model terms. Probe source and selected stream before decoding.
- **Model worker adapter:** bounded sandboxed process, explicit model family and revision, capability-specific parameters, health, progress, cancel, timeout, GPU/memory budget and verified output. One installed model may implement several *verified* task capabilities.
- **Other providers (future optional):** same intent interface but separate consent, egress, billing, privacy and idempotency policy; not included in initial offline-first release.
- **Job coordinator:** durable queue and staging directories, priority arbitration between preview, export and GPU generation, type-safe error classification; does not become another media decoder or model registry.

## Creative chain: image to video to image

1. Asset P0 photograph, with immutable source record.
2. Transform T1 creates concept art C1 using P0 as composition reference.
3. Transform T2 generates shot V1 using C1 as structural/start-frame reference; actual model constraints documented.
4. T3 extracts frame F1 from V1 at **source presentation timestamp**; this is deterministic and produces an image asset.
5. T4 creates enhanced cropped still H1 from F1. Users can select deterministic crop and optional qualified upscale separately.
6. T5 uses H1 as reference for shot V2; sources and per-model settings recorded.
7. Edit recipe combines V1/V2 with audio; native exporter yields validated final output E1.

This is one project and several jobs, not seven manually transferred files. Generated camera viewpoints cannot honestly promise hidden geometry consistency from a single still.

## Key design choices

| Question | Candidate answer | Decision gate |
| --- | --- | --- |
| Node graph or simple actions? | Typed operation graph internally; **contextual plain-language actions** externally | User validation ticket #8 |
| Job persistence? | Durable native journal/DB records with content-verified commit | Crash/kill test and asset decision #5 |
| API? | In-process bounded native IPC; no localhost server by default | Security review, host qualification |
| Model invocation? | Explicit adapter without arbitrary upstream code execution | Trust review ticket #9 |
| Multi-shot automatic planner? | Compose operations as an optional future orchestration layer | Outlier roadmap, not alpha blocker |

## Acceptance probes

- A chain can reuse a derived frame as a new source without exporting/reimporting a file.
- Two operations using the same original cannot overwrite it or each other.
- Cancellation, crash, timeout, exhaustion, checksum mismatch and partial artifact never create a completed asset.
- Model execution stays unavailable when license/rights, adapter readiness or device constraints are unproven.
- A user can continue editing old assets while the model environment is unavailable.
- Operations can be versioned and re-run deliberately; generated differences are recorded rather than claimed bitwise deterministic.

All contracts here are candidates for an owner-reviewed spec, not declared executable behavior.

