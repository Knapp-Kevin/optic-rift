# Optic Rift Architectural Index

**Index identity:** `docs/ARCHITECTURE_INDEX.md` is Optic Rift's **one canonical architectural filing and product-truth routing index**. This identity was explicitly directed by the repository owner on 2026-11-05. All project READMEs and agent entry points route here; no README or Wayfinder issue is a competing architecture index.

**Adoption history:** The explicit index path was established on `main` through [PR #12](https://github.com/Knapp-Kevin/optic-rift/pull/12) at merge commit `2230f43459fe8e10a1c895a2189a7e13ab2969eb`. This proposal extends the owner register with eight still-**Proposed** design pages; their acceptance remains separately gated.

**Index status:** Established as the agreed *routing location*, **not** an independent architecture approval, runtime qualification, release authorization, or replacement for individual source owners. Individual documents own their actual claims. The source/decision/evidence supporting a claim is the truth for that claim, not a copied summary here.

**Reading path:** [root README](../README.md) → **this index** → relevant owner source → executed evidence, where applicable.

**Authority boundary:** This index routes each product/architecture claim to one accountable owner. It is not a second governance policy, architectural decision, code contract, qualification record, release approval or living roadmap. A registered proposal is still a proposal.

## Product truth and claim precedence

Use this index as the **only canonical map of product truth**. It assigns ownership, not automatic correctness or maturity.

- **Product intent and scope** belong to the [product charter](../README.md) and any expressly owner-accepted decisions. Intent alone does not prove delivery.
- **Design authority** belongs to an accepted, scope-bound ADR/contract and its recorded owner decision. A draft diagram is a proposal even if indexed.
- **Implemented behavior** belongs to versioned source/schema and their real callers, not to aspirations in the charter.
- **Qualified behavior** belongs to executed tests, real Windows/user acceptance, environment and exact subject revision. A green doc linter does not qualify the app.
- **Published behavior** belongs to installer/release artifact and actual activation observations, not to a merged PR.
- **Next work** belongs to live tickets and PRs, including the Wayfinder map. They do not establish adopted design.
- **Historical origin** remains accessible with explicit supersession/lineage; recency alone does not transfer authority.

If these sources disagree, record **drift with exact paths and revisions**, identify the accountable owner and leave the conflict unresolved until an authorized decision. Do not silently revise the ADR to match code or the charter to match a mockup.

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

This file is the **only** filing register for Optic Rift architecture. The listed documents own non-overlapping architectural questions; they are **proposals under review**, not accepted ADRs, runtime contracts or evidence. Each proposed design's status and acceptance live at its destination, not in a duplicated approval list. Resolve decisions through the [Wayfinder architecture map](https://github.com/Knapp-Kevin/optic-rift/issues/4) and its issue tickets; the map indexes decisions, while this table indexes architecture claim owners. Neither substitutes for Qortara lifecycle enforcement.

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

**Status rule:** A diagram or recommended path does not establish implementation, schema acceptance, CI, independent review or production readiness. The current `main` tree does not contain the draft desktop app from PR #1. No design in this section should be marked Accepted until the corresponding Wayfinder question is resolved, required review/owner acceptance occurs and the precise accepted artifact is recorded. If a decision changes the shell, update the existing ADR-0001 at its owner rather than creating ADR-0001 again.

**How designs graduate:** (1) resolve a named decision ticket with observed evidence and owner input where required; (2) update its one architectural owner and any affected contracts/ADR with a revision-bound supersession note; (3) have the single index point to that accepted version; (4) only then prepare an implementation spec and feature tickets. Deferred features stay in the outlier register (registered above), not a forgotten pile or implied alpha promises.

## Index maintenance contract (required for every architectural change)

1. **Start here** before planning or editing architecture. Resolve the intended claim and its existing owner. Never create a second index or a second owner for the same architectural claim.
2. **File each new architecture document once** under its owning shelf in this file, in the **same PR** that adds it. Update root `README.md`, `docs/README.md` and `AGENTS.md` only as routing pointers, not as duplicated owner tables.
3. **Declare maturity at the destination:** `Status: Proposed`, `Accepted`, `Implemented`, `Qualified`, `Historical` or `Superseded`, scoped to the relevant claim. Status is not inferred from merge, most recent edit or filename.
4. **Attach real decision authority:** an architecture change becomes accepted only when its accountable owner decision, affected contract/ADR and revision are recorded. Wayfinder resolution alone is insufficient when independent review or Qortara gate requirements still apply.
5. **Do not overwrite history.** A supersession records old claim/path, replacement owner, scope, reason, decision/approval authority, effective revision/date and dependent contracts/implementations/tests that require revalidation.
6. **Keep evidence outside the index.** Bind test results, benchmark/device reports, independent reviews, releases and Qortara lifecycle receipts to their exact artifacts in their own owners. This file links to them and must never become another evidence ledger.
7. **Revalidate registration:** all existing `docs/architecture/*.md` pages must be registered exactly once **in this file**, all local links must resolve, and every page must use a recognized status declaration. Validate on the target revision. A structural PASS does not establish semantic acceptance.
8. **Preserve governance:** protect `governance/manifest.json` and the native Qortara ledger; only approved runtime/governance changes can alter those. Owner-directed index location does not grant a policy-evaluation result.
9. **Audit architectural drift:** when source changes, report which contracts, plans, references, renderers and tests may be affected. Update their accountable sources and this index together rather than leaving stale claims.

**Qortara catalog integration:** Qortara Logic's baseline checker defaults to `docs/README.md` in its own repository. Optic Rift intentionally assigns the index to **`docs/ARCHITECTURE_INDEX.md`** instead. The actual local Qortara host/check configuration must point to this path before claiming a valid catalog PASS. A default check against `docs/README.md` is **not equivalent** and must not be reported as successful catalog verification. Until native configuration and an executed check exist, automated enforcement is **not established**.

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

