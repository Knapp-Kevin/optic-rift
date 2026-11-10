# Content authenticity, AI disclosure, and encrypted forensic provenance

**Status: Proposed (2026-11-10).** Not accepted, implemented, signed, Windows-qualified, or released.
**Architecture owner:** external content-authenticity / disclosure contract. Source identity, derivation DAG and project storage remain owned by [project and asset lineage](project-asset-lineage.md) and Wayfinder [#5](https://github.com/Knapp-Kevin/optic-rift/issues/5). This document does not replace those owners.
**Decision:** [#58](https://github.com/Knapp-Kevin/optic-rift/issues/58). **Implementation:** [signed exports #59](https://github.com/Knapp-Kevin/optic-rift/issues/59), [encrypted private trace #60](https://github.com/Knapp-Kevin/optic-rift/issues/60), [verification UX #61](https://github.com/Knapp-Kevin/optic-rift/issues/61). Studio work is [photo #42](https://github.com/Knapp-Kevin/optic-rift/issues/42) / [draft #56](https://github.com/Knapp-Kevin/optic-rift/pull/56) and [video #36](https://github.com/Knapp-Kevin/optic-rift/issues/36) / [draft #37](https://github.com/Knapp-Kevin/optic-rift/pull/37).
**Authority boundary:** architecture recommendation for owner review; issue creation, source commentary, SDK capability and positive static tests are not approval, a production signature, or a Qortara receipt.

## Problem statement and scope

Optic Rift generates and edits images and video via multiple inference engines, model revisions, reference media, user edits and native exports. A binary `is_ai_generated` field is insufficient: it hides stages, mislabels hybrid work and can be trivially changed. Metadata that is only encrypted cannot be independently interpreted by a public viewer. Plaintext provenance may expose prompts and user/device identifiers.

The intended outcome is a **publicly legible, cryptographically signed disclosure** of what Optic Rift knows about an exported asset, plus an **owner-controlled encrypted forensic detail record**. The record graph must distinguish machine observations from external assertions and from unknown information. Model use must be disclosed for content generated/AI-modified by Optic Rift, irrespective of the originating model/provider.

This requirement applies to **output operations**. It does not create a universal AI detector. Missing credentials, including due to third-party stripping, do **not** prove that media was made by a human. A valid signature proves attribution to a signer and non-tampering with a claim (subject to trust); it does not establish that every statement is truthful.

## Standards and rationale

- **C2PA Content Credentials 2.5** is the external, interoperable media provenance format: https://spec.c2pa.org/specifications/specifications/2.5/index.html . Use its signed, content-bound manifest, `c2pa.actions`, `c2pa.ingredient.v3` and where conforming `c2pa.ai-disclosure`. A model-generated asset uses the appropriate IPTC `digitalSourceType`, for example `http://cv.iptc.org/newscodes/digitalsourcetype/trainedAlgorithmicMedia`; an edited/composite image may use `http://cv.iptc.org/newscodes/digitalsourcetype/compositeWithTrainedAlgorithmicMedia`. The *operation and source context* determine the category, not a guess from the pixels. For imported photos, existing verified credentials are ingredients where permitted, not rewritten into fictional first-party claims.
- **C2PA synthetic and non-synthetic guidance**, July/August 2026, details source type, AI disclosure, regions of interest, ingredients, and generation actions: https://c2pa.org/a-new-implementation-guide-for-content-credentials/ .
- **AAIF Observability & Traceability** contributes runtime traceability and forensic execution context, not a substitute media-signature format: https://aaif.io/working-groups/observability-traceability and https://aaif.io/blog/can-you-reproduce-an-agent-run (2026-10-13). Record requested model and actual deployment IDs separately where observed, tool/adapter versions, execution attempts and bound inputs. Replayability cannot be promised for nondeterministic model runs.
- **Candidate SDK:** https://github.com/contentauth/c2pa-rs (dual MIT / Apache-2.0 for the crate; dependent licenses, Windows MSVC and concrete C2PA 2.5 assertion feature support must be audited and pinned). Do not implement proprietary incompatible 'encrypted metadata' in place of C2PA.
- **Cryptography:** a digital signature proves claim integrity and signer attribution; authenticated encryption protects confidentiality and integrity for those holding a decryption key; hashing alone authenticates nothing without a trusted binding. A public flag must not be hidden behind an encryption key.

## Proposed domain contract (application data, NOT serialized C2PA syntax)

The native layer assembles provenance from its own observed jobs, qualified imported assertions, and known immutable assets. The WebView may request export and disclosure options but cannot assert trusted capture provenance, choose arbitrary filesystem paths, select signing keys or replace native output hashes.

```text
AssetRevision
  asset_id, revision_id, content_digest, media_type, source_mode
  input_assets[] (asset/revision, verified_digest, role, trust_state)
  operations[] (ordered_operation_id, actor_kind, action, output_effect_scope)
    model_attempt? (requested_model, observed_model_revision/deployment,
                    adapter/version, settings_digest, seed_if_known)
    input_roles[] (asset_ref, prompt_ref, guidance_ref, frame_ref...)
    human_edit? (crop, grading, trim, compositing, keyframe etc.)
    region_or_time_range? (bounds/stream_id, rational timestamps if known)
    source_of_assertion (native_observation | verified_import | user_assertion | unknown)
  export (format, output_digest, derived_from_revision, signer_receipt)
  encrypted_trace_ref? (opaque reference, ciphertext_digest, key_envelope_version)
```

The presence of `model_attempt` indicates *known model involvement*, not universal detection. Multiple models are separate events and can feed one another through asset IDs and validated digests. A model alias is **not** an immutable revision when the provider did not expose one. `actor_kind` is provenance evidence, not a legal determination of authorship.

Separate three axes in the user experience:
1. **Workflow claim:** native AI-generated, native AI-assisted edit, human editing action, mixed chain, unknown import.
2. **Assertion origin:** observed by Optic Rift, asserted by imported credential, user-provided statement, unknown.
3. **Cryptographic verification:** signed/trusted, signed but untrusted issuer, invalid/tampered, missing, or unsupported. A self-signed test claim must not be called externally trusted.

Never assert 'verified human-created' merely because no model event was observed. Even 'human edited' describes only *Optic Rift's editing action*, not the origin of its source pixels.

## Dual-track charter, creator recognition, and trust separation (2026-11-10)

**Status: Proposed; no owner acceptance or feature-build approval.** The [#58 decision discussion](https://github.com/Knapp-Kevin/optic-rift/issues/58) now explicitly distinguishes **P0 mandatory AI-generation/AI-modification disclosure** from **P1 opt-in human-creator process recognition** in [#64](https://github.com/Knapp-Kevin/optic-rift/issues/64). Neither studio's AI-first/AI-mostly identity changes. P1 creator storytelling must neither delay P0 signatures/encryption nor turn process screenshots into authentication certificates. Do not create a creator registry, creator marketplace, monetization flow, artistic-integrity score, AI-free badge or mandatory identity check under this proposal.

For interoperability, design around **six separable evidence questions**, never flatten them into a trusted/untrusted or human/AI Boolean:

| Axis | Acceptable examples | What it cannot imply |
| --- | --- | --- |
| Workflow/creation event | App-observed model generation, user-initiated crop/paint/trim, mixed sequence, unknown input | A manual edit proves all pixels/samples were made without AI |
| Claim source | Locally observed tool event, imported externally signed assertion, artist's own declaration, untrusted import | A locally verified hash independently establishes actor identity or truthful content |
| Content credential integrity | Valid signed hard binding with qualified signer, signature valid but issuer untrusted, invalid, absent, unsupported | Any C2PA signature guarantees factual truth, authorship or training rights |
| Named creator identity | Optional, consented **CAWG** identity with independently qualified credential; user-declared/pseudonymous; not provided | Verified identity guarantees that the named person performed all claimed operations |
| Rights and permissions | Independently substantiated license for a stated scope; artist declaration; rights unknown | A creator identity or signed media manifest grants copyright or permission to train/use media |
| Artistic experience and value | Opt-in, artist-approved evidence-linked process narrative and contributor credits | Subjective artistic merit, effort, originality or ranking can be objectively certified by AI |

**Identity trust model:** C2PA application/product signing identifies the responsible claim-generating tool. Optional **CAWG identity assertions** bind an individual or organization with their own credential and scoped roles; the product's signature must not silently impersonate the artist or claim that the app verified a human-only workflow. Support per-field consent before any future signing of an opt-in identity assertion, including pseudonymous participation, refusal and removal prior to publication. See https://cawg.io/specs/ , https://cawg.io/about/identity-framework/ and https://cawg.io/ux-guidance/1.0/ . A subsequent C2PA signature validating a creator-supplied assertion must distinguish **created** app assertions from **gathered** user/external assertions. Attribution, identity credential validation and media hard-binding must be independently reported.

**P0 compatibility invariant:** If Optic Rift generates AI media, its signed public disclosure stays legible regardless of whether a creator opts into CAWG or encrypts a private workflow appendix. A creator may contribute meaningful manual actions to an AI-assisted output, and those actions should be visible as such. If a creator imports an unsigned asset then paints or trims it, show the operation as **locally observed manual editing of an unknown-origin input**, never "verified human-made." An absent CAWG identity says nothing about artistic value. Qualified production output must never say "C2PA verified" without validating the signature, content binding and appropriate issuer trust. The updated #58 **fail-closed signer policy is recommended but not accepted**; no unsigned draft fallback is authorized by recording this paragraph.

**P1 discovery boundary:** #64 owns consenting creator research, voluntary intermediate-artifact/process evidence, artist-approved descriptions with citations and correction, explicit private/public disclosure previews, anti-forgery and anti-coercion threat modeling, and eventual creator benefit measurement. Sonic Rift has its own audio format implementation and [provenance issue #169](https://github.com/Knapp-Kevin/sonic-rift/issues/169); shared concepts must not create a mandatory shared cloud account. No scoring, automatic AI-use accusation, scraped portfolio matching, commercial artist directory or publishing of raw project files is justified without a separate owner-reviewed product/ethics decision.

**Adversarial acceptance scenarios:** forged artist name on a locally signed AI generation; user clicks "human created" on imported unsigned media; valid CAWG identity for someone claiming another person's work; absent/stripped credentials; valid product signature and untrusted CAWG provider; verified camera capture later altered by AI; multi-person collaboration with revocable consent; genuine manual edits to AI-generated footage; creator process evidence selectively omitted; fabricated screenshots/creation histories; misrepresented rights statements; hidden unapproved AI text in an artist story; artist refuses public identity but allows a pseudonym. Every scenario must retain **separate** origin, assertion source, signature/trust, identity and rights states with evidence gaps, without an all-purpose "human verified" result.

## Public media credential (verifiable by anyone)

For a qualified export, create a **new** C2PA manifest with a hard binding to the final media file and a signing identity:
- accurate creation/open actions and `digitalSourceType` for directly observed generation; include `c2pa.ai-disclosure` only when supported by the chosen SDK/spec version and independently validated;
- `c2pa.ingredient.v3` for each relevant source, with parent/component/input relationship and **preserved validated predecessor manifests** (do not launder invalid imported claims);
- ordered edits and model stages for known contributions; optional still-image region or video stream/time scope; mark missing scope as unknown, not as global coverage;
- model product name and immutable revision **if disclosable and actually known**, and a privacy-preserving application/publisher identity;
- safe opaque identifiers/hashes for private provenance correlation, without publicly exposing raw prompt text or full local source paths;
- final publish receipt with credential version, signing/issuer status, signed media identity and independent validation outcome.

Embedded credentials are preferred for supported formats (start PNG and MP4; expand to JPEG/WebP only after exact-format qualification). The signer operates **after** final media encoding and **before** atomic non-overwriting publication. Verify both the claim and media decode; adding a manifest must not change pixels, audio samples, video frames or color/HDR/rotation semantics by re-encoding. File byte size/hash **will** change when metadata is embedded, so record digests against the *correct signed final artifact* and content/bitstream as appropriate; never compare an unsigned file hash to its signed counterpart as though they were identical.

Use a bounded staging + journaling transaction. If signing/validation fails, do not publish a file claimed to have verifiable provenance. Any proposed **explicit unsigned fallback** needs owner acceptance and a conspicuous unverified status; it cannot be labeled C2PA verified. PNG and MP4 must each have a separate real on-disk, third-party validation path.

## Confidential private forensic trace

Public provenance is minimal and readable. Private details belong in **at-rest encrypted** workflow records, with optional recipient-encrypted payloads when an owner intentionally shares them.

Protect: raw/negative prompts, user identity, local filesystem paths, device fingerprints, private reference media metadata and detailed tool/runtime context. The existing generated-image `generation.rs::Provenance` contains raw prompts in a **plaintext, unsigned** JSON file; this is **not** a C2PA claim or encrypted forensic vault. The existing `photo_export.rs` JSON export receipt is also unsigned. Do not silently copy either record wholesale into public media credentials or publish them as verified evidence.

Candidate cryptographic boundary (subject to security review):
- audited versioned AEAD such as AES-256-GCM with fresh random nonce and per-record associated data binding project/asset/revision/record schema; random entropy, no nonce reuse, bounded payloads;
- keys isolated in native Windows-protected storage, inaccessible to WebView and never packaged globally; explicit key backup, rotation, recovery, revocation, portability and deletion semantics;
- **Embedded encrypted capsule, if the selected SDK/format and owner consent policy permit:** include a *bounded*, namespaced **custom encrypted C2PA assertion** (ciphertext plus algorithm/key-envelope version and digests), covered by the public C2PA signature. A recipient with the independently supplied key can recover richer model-stage details directly from the final media; a verifier without the key still sees the public AI disclosure, and must not claim to have inspected the sealed content. Large, sensitive or unsupported payloads stay in an encrypted private store with a signed reference; a sidecar alone is **not** equivalent to embedded forensic detail. Do not emit custom encryption unless conforming validators can preserve the unknown assertion without treating it as standard cleartext AI disclosure.
- optional recipient key envelopes for exportable encrypted details; signed public ciphertext commitment/opaque reference cannot enable decryption on its own;
- least-privilege local storage, prompt minimization, retention control, explicit disclosure, no hidden network service and no automatic server escrow.

This proposal **does not** commit to DPAPI key portability, a master key lifecycle, an encrypted project vault or a production certificate. Those require owner choice under [#58](https://github.com/Knapp-Kevin/optic-rift/issues/58), [#60](https://github.com/Knapp-Kevin/optic-rift/issues/60), and existing private-output storage [#45](https://github.com/Knapp-Kevin/optic-rift/issues/45).

## Source integration map: observed on main 2026-11-10

| Existing source | Verified current function | Planned credential seam |
| --- | --- | --- |
| `src-tauri/src/generation.rs` | Stores locally generated PNG plus unsigned JSON with prompt, negative prompt, model repo/revision, adapter/version and output SHA-256; optional source-job reference | Emit typed native model/ingredient events and encrypted private trace; preserve existing evidence while migrating safely. Do not export plaintext prompts. |
| `src-tauri/src/photo_export.rs` | Validates WebView-rendered PNG and emits a non-overwriting Pictures PNG plus JSON receipt; WebView-reported source digest not proof of original input | Join to trusted project/source asset when available, sign output and verify content binding before publish; unknown origin otherwise. |
| `src-tauri/src/project_photos.rs` (draft #56) | Managed original PNG plus immutable edit-recipe revisions, source hash and project scope | Preserve original and attach edit events to each exported derived revision; never claim draft branch merged. |
| Native video trim (draft #37) | Frame-accurate trim pipeline with staged output, FFmpeg codec packaging and release gates outstanding | Sign final MP4 container after encode/remux, include verified source and frame/time range; do not rewrite media streams just for metadata. |
| `docs/architecture/project-asset-lineage.md` | Proposed asset identities, immutable inputs and provenance edges | Remains the **single** source for project asset identity, DAG and storage; this document owns external credential trust, signing, disclosure and privacy seam only. |

## Implementation sequence / non-goals

1. **Decision & threat model (#58):** classify release signing strategy, exact C2PA 2.5 SDK capability, key custody, certificate/trust list, timestamp and offline claims, public/private data boundary, explicit fail-closed policy, non-AI attestation authority.
2. **Pure internal events & migration (#59/#60):** derive typed, bounded, independently verifiable multi-model graph from native job/asset evidence; preserve legacy records without claiming upgraded trust; add tests for AI, human edits, mixed, unknown imports, untrusted/contradictory inputs.
3. **Signed PNG vertical slice (#59):** after native export, embed/verifiably sign final PNG, preserve pixels and source bytes, independent validator round trip; don't ship placeholder certs or a mock sign flag.
4. **Encrypted private trace (#60):** native key isolation, encryption and explicit disclose/recipient flow; test crash, wrong key, replay, tampering and portability.
5. **Video MP4 and import validation (#59/#61):** integrate separate qualified video lane, ancestor credentials, model/time/region inputs; verify remux quality and true Windows installer.
6. **Review UX (#61):** user-readable contribution chain, issuer trust status, privacy preview, independent Inspect Credential, unknown origin fallback.
7. **Release gates:** complete exact-head Rust/Node/IPC and clean installed Windows tests, C2PA third-party verification, legal/transitive license, cryptographic threat review, Qortara independent source/security authorization and post-merge tests.

Out of scope now: universal AI pixel detection; retrospective claims on unknown imported media; steganographic robustness guarantees; centralized telemetry; watermarking as a substitute for cryptographic credentials; falsely marking signature-valid but untrusted self-issued certificates as authenticated identity; mandatory raw-prompt publication.

## Required negative and forensic test fixtures

- One model-generated PNG; single human-adjusted imported PNG; AI inpaint/edit on imported source; two-model image → video → frame → image lineage; no-model native video trim of an unknown-origin source; C2PA-bearing device capture; missing credential import.
- Tamper hard-bound bytes, swap manifest, corrupt CBOR, counterfeit issuer, expired/revoked cert, wrong timestamp, inflated assertion, oversized/recursive ingredients, stripped metadata and incompatible format; ensure error states are explicit and no 'human-created' inference.
- Confirm public manifest and any public sidecar **never contain** raw/negative prompts, local paths, user tokens or device identifiers; independently decrypt private trace with authorized key only; deny a different project.
- Destroy staged process at every commit step, simulate disk-full/permission denial, prove original unchanged, no ghost completed output and reliable restart. Use synthetic fixtures free of user secrets.
- Pixel-exact output for lossless image path; no frame/sample loss and no additional encode for video credential embedding; preserve duration, color tags, orientation, audio presence and rational frame-range identity.
- Third-party verifier on installed Windows with producer certificate and trusted timestamp policy; developer certificates are untrusted and qualify only the code path, not identity claims.

**Decision ledger:** Pending owner acceptance on #58. The repo has no C2PA signed export merely because this proposal exists.
