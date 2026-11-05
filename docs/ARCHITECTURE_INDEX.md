# Optic Rift Architectural Index

**Canonical path:** `docs/ARCHITECTURE_INDEX.md`
**Status:** Established as the owner's required **product-truth routing index** on 2026-11-05. Registered designs retain their own Proposed/Accepted/Implemented/Qualified maturity; this index does not approve them.

**This is the single architectural entry point.** The [root README](../README.md), [documentation README](README.md) and [agent instructions](../AGENTS.md) point here rather than maintaining separate architecture registers. The index is authoritative for **where product truth lives**, not a replacement for the sources and evidence that establish truth.

## How to establish a product claim

| Claim being made | Accountable source | What it does not establish |
| --- | --- | --- |
| Product purpose and intended user value | [Product charter](../README.md) and explicitly owner-accepted scope decisions | Implemented features |
| Repository change authority and required gates | [Qortara governance manifest](../governance/manifest.json), host rulesets and authentic lifecycle receipts | Enforced policy from a JSON file alone |
| Qortara policy interpretation | [Qortara Logic canonical engine/schema](https://github.com/MythologIQ-Labs-LLC/qortara-logic/tree/6ee2d7b15a1c5732e1ae2d2485e2fded0bd100ac/src/qortara_logic) | Host availability or permission to merge |
| Architecture and product design | Owner-accepted, scope-bound ADR/contract registered below | Code, test or release acceptance |
| Implemented APIs and behavior | Exact versioned source, schemas and real callers | An accepted conflicting design or qualified device result |
| Qualification and user-experienced behavior | Executed checks, actual Windows device tests, user journey evidence, exact subject revision | Other environments, later source changes or deployment authority |
| Published reality | Signed release artifacts, installation checks and actual publication records | Merely having a build or merged PR |
| Outstanding decisions and work | [Wayfinder map](https://github.com/Knapp-Kevin/optic-rift/issues/4), [issues](https://github.com/Knapp-Kevin/optic-rift/issues) and [pull requests](https://github.com/Knapp-Kevin/optic-rift/pulls) | Accepted design or delivered implementation |
| Historical/superseded material | Pinned original and explicit supersession decision | Current authority without an accepted handoff |

## Architecture owner register

**Current `main` baseline:** No `docs/architecture/*.md` design has yet been merged. This first index deliberately does **not** link to nonexistent local documents or claim the proposed designs are accepted. When architecture pages are adopted, **register each exactly once here** in the same change.

| Scoped ownership | Present owning location | Maturity |
| --- | --- | --- |
| Windows desktop stack, native media preview and performance fallback | [ADR-0001: Windows-first desktop](architecture/ADR-0001-windows-desktop.md) ([draft foundation PR #1](https://github.com/Knapp-Kevin/optic-rift/pull/1)) | Proposed; this stacked branch, unqualified |
| Whole studio composition, project lineage, media pipeline, transformations/jobs, model trust, studio UX, outliers and qualification | [Architecture proposal PR #11](https://github.com/Knapp-Kevin/optic-rift/pull/11) | Proposed; eight topic files, not merged or accepted |
| User-journey-to-decision routing | [Wayfinder architectural decision map](https://github.com/Knapp-Kevin/optic-rift/issues/4) | Planning questions, not implementation approval |
| Product requirements | [README charter](../README.md) | Owner intent; no release qualification |
| Governance and change authority | [Qortara manifest](../governance/manifest.json), [enforcement gap issue](https://github.com/Knapp-Kevin/optic-rift/issues/2) | Manifest merged; actual GitHub/runtime enforcement incomplete |

**When PR #11 becomes eligible for adoption**, replace its aggregate row with direct, unique links to each topic owner's `docs/architecture/*.md` file. Keep the external draft PR as history. Do not create a second authoritative register in `docs/README.md`.

## Claim and maturity rules

- **Proposed:** exploratory design; neither accepted nor implemented.
- **Accepted:** accepted design for a precisely defined scope and revision; not code proof.
- **Implemented:** observed source and contracts at a named revision; not device qualification.
- **Qualified:** executed acceptance evidence for that subject/environment/revision.
- **Historical / Superseded:** provenance with explicit effective replacement.
- **Published:** release/deployment proof held by its own artifact owner, not an architecture page.

When code contradicts an accepted decision, document **drift** with paths, revisions, decision authority and affected dependents. Do not automatically rewrite the decision to fit code, and do not equate more recent text with greater authority.

## Maintenance and supersession protocol

1. **Before design or code:** read this index and the existing source owner. No new architecture page unless no existing document owns the claim or a genuinely distinct scope warrants it.
2. **One owner per claim:** put the new document under the correct shelf and register it exactly once here **in the same PR**. Root README, docs README and AGENTS are pointers only.
3. **Status stays with the owner:** each `docs/architecture/*.md` must carry the parser-readable `Status: Proposed`, `Accepted`, `Implemented`, `Qualified`, `Historical` or `Superseded`. Index links confer no acceptance.
4. **Decisions need authority:** Wayfinder questions resolve into recorded owner decisions and scoped contracts/ADRs. Required independent review, exact-revision evaluation and Qortara lifecycle gates remain separate.
5. **Supersession needs a record:** cite old claim/path, new owner, scope, decision and authority, effective revision/date, reason, and dependent interfaces/tests to requalify. Keep old history discoverable.
6. **Keep evidence with its source:** tests, Windows performance, licenses, model revisions, independent reviews, actual release/downloads and Qortara ledger events stay in their accountable sources, not duplicated here.
7. **Verify the index on each changed revision:** inspect local links, unique registrations for the actual `docs/architecture/` directory, status parsing, competing owners, cross-contract drift and pending evidence. A successful structural scan is not semantic acceptance.

## Qortara integration boundary

This file follows the [Qortara architectural index protocol](https://github.com/MythologIQ-Labs-LLC/qortara-logic/blob/6ee2d7b15a1c5732e1ae2d2485e2fded0bd100ac/skill-source/references/architecture-index.md). The stock Qortara catalog check uses `docs/README.md` by default, while Optic Rift's **explicit owner-directed canonical index is `docs/ARCHITECTURE_INDEX.md`**. An authorized native Qortara host must configure its index check for this exact path; checking the default README and calling it a PASS would be invalid. No native index-gate receipt is asserted here.

Repository `main` was observed without branch rulesets, and no independent review or authenticated Qortara receipt has been recorded for the bootstrap. [Governance issue #2](https://github.com/Knapp-Kevin/optic-rift/issues/2) remains open. No architecture, Windows application, installed model, edited media export or signed release is qualified by creating this index.
