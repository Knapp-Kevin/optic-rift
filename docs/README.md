# Optic Rift documentation and authority index

**Purpose:** This is the single filing index, not a second governance policy, architectural decision, release approval or product roadmap. A linked proposal does not become accepted merely because it appears here.

## Source of truth by claim

| Question | Accountable source | Status / boundary |
| --- | --- | --- |
| What should Optic Rift be? | [Project README](../README.md) and owner-accepted decisions | Product intent; not implemented-feature proof |
| Which rules govern repository changes? | [Qortara Logic repository governance manifest](../governance/manifest.json) | **Merged on main**, but GitHub ruleset/native enforcement remains unqualified ([issue #2](https://github.com/Knapp-Kevin/optic-rift/issues/2)) |
| What do Qortara contracts mean? | [Canonical Qortara Logic engine and schemas](https://github.com/MythologIQ-Labs-LLC/qortara-logic/tree/6ee2d7b15a1c5732e1ae2d2485e2fded0bd100ac/src/qortara_logic) | Schema provenance at source commit `6ee2d7b`; hosted engine version must be checked independently |
| How do agents start work? | [AGENTS.md](../AGENTS.md) | Portable index pointer; no duplicated policy rules |
| What architecture is proposed? | [Architecture shelves below](#architecture-shelves-proposed), plus [desktop foundation PR #1](https://github.com/Knapp-Kevin/optic-rift/pull/1) | **Proposals** on separate review branches, not accepted implementations |
| What product constraints are proposed? | [PR #1](https://github.com/Knapp-Kevin/optic-rift/pull/1) model-discovery, adult-preview and performance docs | Draft design and code, not tested Windows behavior |
| Which changes are outstanding? | Live GitHub [issues](https://github.com/Knapp-Kevin/optic-rift/issues) and [pull requests](https://github.com/Knapp-Kevin/optic-rift/pulls) | Status at time of access; no duplicated issue tracker |
| Which behaviors actually passed? | Exact-version executed test/Windows evidence and Qortara runtime records | Absent until genuinely executed and authenticated |
| What is published? | Signed release artifacts and real deployment/installation checks | No release proven by documentation |

## Architecture shelves (proposed)

This is the **only** filing register for Optic Rift architecture. The listed documents own non-overlapping architectural questions; they are **proposals under review**, not accepted ADRs, runtime contracts or evidence. Each proposed design's status and acceptance live at its destination, not in a duplicated approval list. Resolve decisions through the [Wayfinder architecture map](https://github.com/Knapp-Kevin/optic-rift/issues/4) and its issue tickets; the map indexes decisions, while this table indexes architecture claim owners. Neither substitutes for Qortara lifecycle enforcement.

| Architectural shelf / ownership | Single owner document | Decision / maturity |
| --- | --- | --- |
| Whole studio and architecture paths | [Studio composition and alternatives](architecture/studio-composition.md) | Proposed; revisit against [media boundary](https://github.com/Knapp-Kevin/optic-rift/issues/6) |
| Stable project identity, immutable source media and derivation history | [Project and asset lineage](architecture/project-asset-lineage.md) | Proposed; [asset decision](https://github.com/Knapp-Kevin/optic-rift/issues/5) |
| Video/image preview, frame accuracy, ordinary media editing and export | [Media pipeline](architecture/media-pipeline.md) | Proposed; [Windows prototype decision](https://github.com/Knapp-Kevin/optic-rift/issues/6) |
| Transformation request, queue, state machine, cancellation and commit | [Transformations and durable jobs](architecture/transformations-and-jobs.md) | Proposed; [job boundary](https://github.com/Knapp-Kevin/optic-rift/issues/7) |
| Model discovery-to-runtime trust boundary and source consent integration | [Model runtime and trust](architecture/model-runtime-and-trust.md) | Proposed; [model/rights decision](https://github.com/Knapp-Kevin/optic-rift/issues/9). Detailed catalog/content rules belong to PR #1 contracts |
| Studio workspace UX, navigation and journey actions | [Studio interactions](architecture/studio-interactions.md) | Proposed; [interaction decision](https://github.com/Knapp-Kevin/optic-rift/issues/8) |
| Stage boundaries and explicit deferred/outlier capabilities | [Roadmap and outlier register](architecture/roadmap-and-outliers.md) | Proposed; [alpha scope decision](https://github.com/Knapp-Kevin/optic-rift/issues/10) |
| Outcome acceptance, test/evidence and handoff criteria | [Qualification and evidence](architecture/qualification-and-evidence.md) | Proposed; actual measurements absent |
| Desktop shell selection, current native UI/IPC source | [ADR-0001 within draft desktop PR #1](https://github.com/Knapp-Kevin/optic-rift/blob/feature/windows-desktop-foundation-20261105-no-ci/docs/architecture/ADR-0001-windows-desktop.md) | **Provisional, unmerged**; not reproduced in this branch |
| Concrete model/catalog/content and provisional media benchmark contracts | [Draft desktop PR #1](https://github.com/Knapp-Kevin/optic-rift/pull/1) | Unmerged candidate contracts; their own files are topic owners |

**Status rule:** A diagram or recommended path does not establish implementation, schema acceptance, CI, independent review or production readiness. The current \`main\` tree does not contain the draft desktop app from PR #1. No design in this section should be marked Accepted until the corresponding Wayfinder question is resolved, required review/owner acceptance occurs and the precise accepted artifact is recorded. If a decision changes the shell, update the existing ADR-0001 at its owner rather than creating ADR-0001 again.

**How designs graduate:** (1) resolve a named decision ticket with observed evidence and owner input where required; (2) update its one architectural owner and any affected contracts/ADR with a revision-bound supersession note; (3) have the single index point to that accepted version; (4) only then prepare an implementation spec and feature tickets. Deferred features stay in [the outlier register](architecture/roadmap-and-outliers.md), not a forgotten pile or implied alpha promises.

## Filing and conflict rules

1. Keep product intent in the existing README and scoped decisions in `docs/architecture/`. Register an accepted ADR here once, after its decision/owner status is clear.
2. Keep model/security contracts in `docs/contracts/`; engine semantics are owned by Qortara Logic, not copied here.
3. Keep governance requirements in the Qortara manifest only. Do not add a separate rules/CONSTITUTION/framework with overlapping authority. Repository enforcement must be implemented in GitHub protection/rulesets and trusted host gates; the JSON manifest alone does not enforce anything.
4. Keep executable proof and reviewer signatures outside summaries, bound to exact commits and qualified actors. Missing or conflicting evidence remains unresolved.
5. A changed policy or architecture requires a proposed change, a review with the applicable independence condition, and source-accurate supersession. Recency does not create authority.
6. Native Qortara runtime, not an agent editing by hand, owns the hash-chained ledger at `governance/ledger.jsonl`. An absent ledger cannot be called intact.

## Governance bootstrap scope

Source schema: `urn:qortara-logic:schema:policy:governance-manifest` at Qortara Logic `6ee2d7b15a1c5732e1ae2d2485e2fded0bd100ac`; hosted plugin reports read-only contract 1.3 and a separately reported backend revision. The source and deployed versions must not be assumed identical without verification.

Current GitHub observation at initial assessment (2026-11-05): `main` was reported unprotected, no repository rulesets existed, and the detailed branch protection API returned HTTP 403 for the connector. The manifest declares intended review/protection controls but cannot enact them. It also requires CI evidence, yet no CI workflow or qualifying status checks exist; do **not** invent either. An owner must enable protections via an authorized GitHub administration surface and require checks only once they genuinely exist. The single owner cannot self-certify an independent approval.

**No native lifecycle ledger or independent audit receipt is created by this bootstrap.** Track adoption and gaps in [issue #2](https://github.com/Knapp-Kevin/optic-rift/issues/2).

