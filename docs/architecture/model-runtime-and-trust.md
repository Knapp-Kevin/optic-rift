# Model discovery, runtime isolation, licensing and content trust

- Status: **PROPOSED**. Candidate cross-component boundary, not an installed model or executable policy.
- Claim owner: integration boundary between model discovery, compatibility, user consent, worker execution and resulting asset provenance.
- Wayfinder: [Model, license, privacy and content decision](https://github.com/Knapp-Kevin/optic-rift/issues/9).
- Specific model-catalog and adult content UX contracts already exist in **draft PR #1**, [model discovery](https://github.com/Knapp-Kevin/optic-rift/blob/feature/windows-desktop-foundation-20261105-no-ci/docs/contracts/model-discovery-and-execution.md) and [adult previews](https://github.com/Knapp-Kevin/optic-rift/blob/feature/windows-desktop-foundation-20261105-no-ci/docs/contracts/adult-content-and-preview.md). They remain the scoped contract owners; do not duplicate their rules here.

## One model has several independent states

~~~mermaid
flowchart LR
 S[Optional catalog search] --> C[Metadata and source claims]
 C --> A[Adapter-supported capability]
 A --> V[Verified family, device and revision]
 V --> L[License, consent and age/rights checks]
 L --> I[Explicit download and install]
 I --> H[Worker health and resource check]
 H --> X[Authorized local execution]
 X --> P[Staged output validated as project asset]
~~~

The public catalog may return a model even if no worker exists to run it. User-facing state is a collection, not a misleading single green check:
- **Discovered:** source claims indexed; not automatically approved.
- **Supported:** an audited adapter recognizes the explicit model family and capability.
- **Verified:** qualification evidence matches model revision, runtime and hardware class.
- **Installed:** pinned weight files and isolated environment have passed integrity checks.
- **Ready:** adapter health, actual resource budget, permissions and intended-use rights permit this specific attempt.

License, intended audience (general/mature/adult-focused/unverified), generated-output classification and runtime readiness are separate fields. An unknown license never becomes commercial permission; unknown content metadata never becomes a promise of nonexplicit results.

## Trust boundaries

| Boundary | Input / risk | Candidate control | Failure shown to user |
| --- | --- | --- | --- |
| Public model catalog | Untrusted README/model cards, prompt injection | Treat as data, restricted parsing/markup, no executable instructions | Metadata unverified |
| Download | Unknown weights, redirects, huge files, gated access | Explicit user approval, immutable revision, size/disk budget, hash verification and quarantine | Download blocked or unverifiable |
| License acceptance | Noncommercial or base-model restrictions | Separate code/weight/base terms and intended use, record actual consent, never infer rights | Rights not established |
| Worker environment | Python dependency conflicts or malicious checkpoint | Dedicated locked environment, minimal permissions, no trust_remote_code or unsafe deserialization by default | Adapter unavailable |
| GPU and runtime | OOM, model hang, native crash | Resource arbitration, process watchdog, health/progress/cancel, output staging | Recovered job failure without lost project |
| Privacy | User originals/prompts/tokens and local media | Local-only default; secrets in OS credential store, scrub logs; explicit opt-in before external transfer | Clear network/permission request |
| Content discovery | Explicit previews surprise unprepared users | Adult-oriented discovery opt-in; per-preview reveal separate; classification with evidence | Preview concealed/metadata unknown |
| Derived media | Mislabelled or partial outputs | Asset-level provenance and content status separate from model intent; hash/format validation | Output remains unverified |

Do not claim age acknowledgment is legally sufficient age verification. Protect against sexual content involving minors and nonconsensual intimate imagery without silently censoring unrelated user-owned local files. Required safeguards and distribution/legal review are unresolved gates.

## Runtime strategy alternatives

1. **Preferred:** native project/job host starts isolated Python/PyTorch workers per qualified model family, communicating with a fixed typed contract. Native process remains responsive when inference crashes.
2. **Optional future adapter:** ComfyUI or another external local engine only behind the same capability, privacy, license and execution boundary; do not require a node graph or local server for everyday workflows.
3. **Optional future network provider:** separate provider consent/egress/billing/retention policy and idempotency. No implicit substitution when local execution is blocked.
4. **Rejected as default:** run arbitrary repository Python in desktop host, install dependencies dynamically without approval, deserialize untrusted pickle, silently use paid cloud compute or infer compatible engines from model tags.

## Interface and records

Request from the [job coordinator](transformations-and-jobs.md): capability, model-family/adapter version, pinned checkpoint revision, typed inputs/reference roles, parameters/seed, requested privacy and intended use, validated project context. Response: explicit model version, source hashes, execution attempt, output validation and error category, **never** an unverified claim of consent.

## Validation probes

- Discovery of unsupported model remains discoverable and visibly not runnable; choosing it never installs a package.
- Model/card prompt injection cannot change project policies or obtain host command authority.
- A model's source has license restrictions unknown or noncommercial; installation/execution for commercial use remains blocked when not authorized.
- Adult-focused listing opt-in doesn't autoplay explicit previews; unreviewed model metadata is labeled unverified.
- Kill worker and exhaust VRAM mid-generation; editor remains usable, original remains untouched, no partial output becomes a completed asset.
- Revoke access, change source revision or fail checksum; prior compatibility/consent is not carried over silently.

**Gaps:** no production catalog, adapters, end-to-end moderation/protection, signed worker packages, user legal review or GPU receipts are established by this proposal.

