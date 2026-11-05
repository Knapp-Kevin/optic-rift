# Architecture rollout paths and deferred capability register

- Status: **PROPOSED**. Sequencing and scope only; not a completed roadmap or owner approval.
- Claim owner: staged architecture admission, outlier register and revisit triggers.
- Wayfinder: [First releasable journey decision](https://github.com/Knapp-Kevin/optic-rift/issues/10).
- Product charter: [README](../../README.md). All phases preserve the same project/asset identity and editor surface.

## Candidate stages (not calendar promises)

| Stage | User value proven | Required architecture | Exit evidence |
| --- | --- | --- | --- |
| F0: trust and native shell qualification | App installs, opens, can handle media without destroying files | Windows host, narrow IPC, project source protection, codec/proxy spike | Recorded app/driver/codec run; governance and supply chain gaps visible |
| F1: truly useful offline editor | Import -> project save/reopen -> preview -> precise frame selection/extraction -> crop/trim -> real export | Asset graph, native media service, persistence, deterministic job path | **Sam + Priya** journeys run with real media, no model required |
| F2: image and reference creation | Create or import image -> branch concept variations -> edit/compare/export | Typed asset reference/recipes, first qualified image adapter, rights/consent | **Eli** journey, original/source lineage intact |
| F3: cross-media creative chain | Image -> short video -> frame -> new image -> new video -> timeline export | Image-to-video adapter, GPU scheduling and durable jobs, frame-accurate provenance | **Alex/Avery** chain verified; no silent model substitution |
| F4: deeper media quality and storytelling | Restore old videos, contact sheet selection, multishot editing and reliable enhancement | Media repair strategies, alternative models, qualified codecs, scalable render job scheduling | **Morgan/Maya** outcomes on actual devices and source material |
| Beyond: advanced studio extensions | Character/scene control, automatic story planning, interoperable workflows, optional cloud/collaboration | Extend existing capability/asset/job/plugin seams, not a parallel project system | Independent owner-approved requirements and measured qualification |

A stage can split into smaller releases, and a capability may move earlier if it is cheaply delivered and actually qualified. **Do not move a nonworking feature earlier merely because a mockup looks impressive.**

## Foundational versus outlier classification

### Foundational (must be designed from day one)

- Durable projects, immutable originals, versioned derived assets, reference edges, source-frame provenance and resumable navigation.
- Ordinary image editing, video preview, exact frame selection/extraction, trim and output export without AI.
- Typed task/capability selection, job staging, error and cancellation behavior, explicit rights and downloads.
- Model-independent user experience, GPU contention isolation, no arbitrary code execution from discovery.
- Re-using any supported derived asset as a standard input for another operation.
- Local privacy and clear adult-content discovery/preview choices as separate dimensions.

### Outliers: deliberately deferred, never forgotten

| Capability | Why deferred initially | Architectural seam preserved | Revisit when |
| --- | --- | --- | --- |
| Consistent characters across many videos | Multi-model identity control and drift tests | Reference roles and per-shot provenance | Basic image-to-video chain passes |
| New camera angle from one photograph | Hallucinated unseen geometry, uncertain reconstruction | Transform capability + generated-view label | Reference quality and controls are tested |
| Full 3D / depth scene reconstruction | Complex geometry, storage and renderer | Optional derived scene-asset type / adapter | Specific use case and resource budget |
| Automated long-form film direction | Planning, continuity, generation costs | Job compositions and shot list | Manual multishot workflow reliable |
| Advanced VFX / rotoscoping / tracking | Separate interaction and acceleration complexity | Mask/motion tracking media operations | Timeline/core asset editor stable |
| Facial reenactment / voice-linked performance | Identity/consent harms and legal restrictions | Separate reviewed capability and output metadata | Targeted safeguards and authority proven |
| Full video relighting, object replacement and interpolation | Temporal consistency, GPU and artifact quality | Video-to-video transform and validation | Benchmarked adapters available |
| Upscale/restoration model families | Model variability and hallucinated detail | Enhancement operation + original comparison | Deterministic media F1 complete |
| Professional interchange (EDL/OTIO/project export) | External schema fidelity and dependencies | Versioned timeline and export adapters | Basic timeline format stabilizes |
| Live streaming, capture and screen recording | OS/device permissions, real-time scheduling | Media ingest interface | Offline import/playback fully reliable |
| Multi-user sync and cloud collaboration | Auth/conflict resolution/privacy/hosting | Stable project identity and explicit provider boundary | Separate product/owner case |
| Remote/paid rendering providers | Egress, costs, terms, legal handling | Optional executor provider interface | Local path qualified; explicit owner decision |
| Linux/macOS distributions | Codec, hardware, packaging matrix | Host-neutral core; platform-specific adapters | Windows acceptance met |
| Model marketplace / arbitrary ComfyUI graphs | Untrusted workflows, broad dependency attack surface | Audited optional adapters | Model catalog and isolated execution proven |
| Image animation between multiple keyframes | Variable engine capabilities, temporal control | Start/end frame reference roles | Single-reference video generation qualified |
| Automated quality ranking and shot selection | Inference uncertainty and subjective quality | Contact-sheet/filter extension, never silent deletion | Reliable contact sheet/frame selection |
| Model-specific commercial distribution entitlements | Heterogeneous code/weight/base terms | Separate rights claims and immutable revisions | Candidate checkpoint legal reviews completed |

**Not deferred as mere outliers:** frame-to-photo, ordinary zoom/crop, project reuse, basic photo/video export, no-model editing, import/reopen, and safe provenance. These are user-journey foundation tests.

## Decision rules for phase changes

1. Reference a ticket resolving the user value, owner and actual resource budget; no unreviewed scope creep.
2. State which existing architectural owner is extended and which contract changes. Avoid another duplicate index or project store.
3. Test current phase regressions: media original preservation, preview response, offline no-model edit and export.
4. Do not claim generation/model readiness or commercial rights from metadata only.
5. Update this register after a capability graduates and point to its actual accepted design/issue. The register itself is not acceptance authority.

