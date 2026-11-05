# Optic Rift documentation and authority index

**Purpose:** This is the single filing index, not a second governance policy, architectural decision, release approval or product roadmap. A linked proposal does not become accepted merely because it appears here.

## Source of truth by claim

| Question | Accountable source | Status / boundary |
| --- | --- | --- |
| What should Optic Rift be? | [Project README](../README.md) and owner-accepted decisions | Product intent; not implemented-feature proof |
| Which rules govern repository changes? | [Qortara Logic repository governance manifest](../governance/manifest.json) | Repository policy proposal until merged and host enforcement qualified |
| What do Qortara contracts mean? | [Canonical Qortara Logic engine and schemas](https://github.com/MythologIQ-Labs-LLC/qortara-logic/tree/6ee2d7b15a1c5732e1ae2d2485e2fded0bd100ac/src/qortara_logic) | Schema provenance at source commit `6ee2d7b`; hosted engine version must be checked independently |
| How do agents start work? | [AGENTS.md](../AGENTS.md) | Portable index pointer; no duplicated policy rules |
| What architecture is proposed? | [Desktop foundation PR #1](https://github.com/Knapp-Kevin/optic-rift/pull/1), including `docs/architecture/ADR-0001-windows-desktop.md` | **Draft**, not accepted or merged to main |
| What product constraints are proposed? | [PR #1](https://github.com/Knapp-Kevin/optic-rift/pull/1) model-discovery, adult-preview and performance docs | Draft design and code, not tested Windows behavior |
| Which changes are outstanding? | Live GitHub [issues](https://github.com/Knapp-Kevin/optic-rift/issues) and [pull requests](https://github.com/Knapp-Kevin/optic-rift/pulls) | Status at time of access; no duplicated issue tracker |
| Which behaviors actually passed? | Exact-version executed test/Windows evidence and Qortara runtime records | Absent until genuinely executed and authenticated |
| What is published? | Signed release artifacts and real deployment/installation checks | No release proven by documentation |

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

