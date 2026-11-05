# Studio composition and architectural paths

Status: Proposed, not owner-accepted, implemented, qualified or released.
- Claim owner: system boundaries and architectural alternatives only.
- Decision path: [Wayfinder map](https://github.com/Knapp-Kevin/optic-rift/issues/4); native-shell qualification [media decision](https://github.com/Knapp-Kevin/optic-rift/issues/6).
- Basis: user journeys, [product charter](../../README.md), Qortara architectural-index protocol; provisional desktop [ADR-0001 in draft PR #1](https://github.com/Knapp-Kevin/optic-rift/pull/1).

## Destination

Optic Rift is one local-first creative **project workspace** where a person imports, generates, edits, enhances, extracts, references, and reuses image and video assets. Every supported transformation feeds a usable asset back into the same project. The program remains an ordinary functioning media editor without any AI engine.

## Architectural paths

| Path | What it prioritizes | Advantages | Cost / risk | Disposition |
| --- | --- | --- | --- | --- |
| A. Desktop shell + native media/project core + isolated optional model workers | Coherent creative experience, offline editing, reliable data | Independent preview and export; crash isolation; replaceable inference adapters | IPC, packaging, decode/scrub benchmarks, multiple toolchains | **Preferred hypothesis** |
| B. Python-first monolithic GUI and inference | Rapid use of existing Python ML and Qt multimedia | Fewer cross-language interfaces; potentially simpler native video surface | Editor stability coupled to model dependencies; less clean execution boundary without isolation | **Fallback UI candidate only**, keep workers isolated |
| C. Browser-only workflow-graph application | Broad model extension ecosystem | Fast model experimentation | Fragments project work; weak default user journey, complex local storage, workflow graphs exposed | **Reject for primary UX**, optional adapter later |
| D. Hybrid model-generation app with an external editor | Minimizes scope | Faster isolated image/video demos | Repeated import/export, source loss, no full creative journey | **Reject as product destination** |

Path A is deliberately provisional. The shell mechanism (Tauri 2/React + Rust vs Qt native playback) is owned by the existing **unmerged** ADR-0001. This document does not accept or supersede it. A witnessed seek/playback benchmark may change UI technology while preserving the project, media-service and transformation contracts.

## Component boundaries

~~~mermaid
flowchart TD
 U[Windows creator] --> UI[Studio UI and player]
 UI --> API[Validated native command boundary]
 API --> PJ[Project and asset service]
 API --> ED[Media preview, edit and export service]
 API --> JOB[Durable transformation job service]
 PJ --> FS[(Local originals and derived assets)]
 PJ --> IX[(Versioned project index)]
 ED --> MX[FFmpeg and qualified codecs]
 JOB --> D[Capability and adapter registry]
 D --> HF[Optional Hugging Face metadata]
 JOB --> MW[Isolated model worker per adapter]
 ED --> PJ
 MW --> JOB
 JOB --> PJ
~~~

- **UI owns:** user intent, screen state, accessible playback controls, project navigation, previews, undo interactions, explicit approvals, displays of evidence and progress. It does not own binary media, model installations, long-running jobs or primary project truth.
- **Project service owns:** durable project identity, asset records, immutable source links, revisions, reference edges, recoverable transactions, cached-asset separation.
- **Media service owns:** probe, decode, seek, frame extraction, crop/scale, timeline render, thumbnail/proxy caches, audio/video sync and true file export. CSS-only adjustments must never masquerade as exported edits.
- **Job service owns:** operation requests, dependency/job state, cancellation, scheduling, output staging, failure recovery and committed provenance. It invokes deterministic workers and isolated inference adapters without coupling to a particular model.
- **Registry owns:** task capability declarations, compatibility evidence, model and weight provenance, rights/consent, optional download and worker availability. Discovery is never execution authorization.
- **Security boundary:** all external media, model cards and assets are untrusted inputs. Rust/native IPC validates paths, permissions and operation schemas. The UI/webview is not a privileged shell.

## Proposed executable routes

1. **No-model edit:** select local file -> probe and index -> local preview -> non-destructive edit recipe -> deterministic render/export -> new output asset.
2. **Generate from prompt:** choose task -> discover qualified optional adapter -> license/rights and device checks -> deliberate model activation -> queued isolated job -> staged artifact validation -> derived project asset.
3. **Reuse across media:** source image -> references and transform -> image variants -> image-to-video -> precise frame extraction with source time -> optional crop/enhance -> image-to-video again -> edited sequence -> export.
4. **Failure:** model crash or disk-full -> job failed/recoverable -> original immutable -> incomplete staging isolated and cleaned or resumed with integrity checks; no published asset from partial output.

These flows are user contracts to validate, not implemented flows on main.

## Non-negotiable seams

- Separate source asset identity, transform recipe, execution attempt, output asset identity and project placement. One image may feed many scenes; one video frame may feed another generation.
- Transform inputs and outputs are typed. A capability may be discovered but not usable if no qualified adapter/rights/device.
- Every heavy activity stays outside the UI thread; playback/export must not depend on Python inference.
- Deterministic edit/export does not require model installation, login or internet access.
- Project metadata and originals must survive cancellation, GPU exhaustion and app crash.

## Decision and acceptance

This is a **candidate system composition**, not an accepted ADR. Re-evaluate after [asset identity](https://github.com/Knapp-Kevin/optic-rift/issues/5), [media qualification](https://github.com/Knapp-Kevin/optic-rift/issues/6), [job seam](https://github.com/Knapp-Kevin/optic-rift/issues/7) and [UX path](https://github.com/Knapp-Kevin/optic-rift/issues/8) decisions. Approval belongs to owner-accepted decisions at their exact revisions, not this drawing.

