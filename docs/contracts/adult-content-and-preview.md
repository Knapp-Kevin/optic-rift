# Adult-oriented models and explicit content: discovery and UX contract

**Status:** Adopted product requirement (2026-11-05), not a claim that adult model discovery, classification, generation or preview gating is currently implemented.

## Intent

Optic Rift is a model-independent local creative studio for adults to generate and edit lawful visual media, including sexually explicit adult media. We do not impose a blanket pornography ban or pretend an adult-capable model must be excluded. We also do not expose unsuspecting users to explicit model thumbnails, autoplay, or surprising output. **Choice and labeling, not surprise, are the governing UX principles.**

## Separate concepts

| Dimension | Example | Meaning |
| --- | --- | --- |
| Technical capability | image-to-video, text-to-image | What the engine can execute |
| Intended audience/content | **Adult-focused (18+) · Explicit**, Mature themes, General purpose, Unverified | The model's stated or reviewed purpose, not a judgment of every output |
| License / terms | noncommercial, community license, unknown, gated | Legal usage conditions, evaluated separately |
| Runtime readiness | discovered, supported, verified, installed, ready | Whether this checkpoint works here |
| Generated-asset content | user-marked explicit, unreviewed, user-marked nonexplicit | What an individual output contains (not guaranteed by a model badge) |

These fields must never be collapsed into a single "safe" or "approved" indicator. No model is silently called safe solely because its metadata omits an explicit-content tag.

## Discovery rules

1. Default model-browser experience excludes **known adult-focused models** and shows no explicit model-card thumbnails, GIFs or autoplay, including search recommendations and related-model cards.
2. A clearly named **Include adult-focused models (18+)** preference can be enabled deliberately. First enablement requests explicit adult-age acknowledgment. This acknowledgment is not formal age verification, so any real age-assurance obligations must be evaluated before distribution.
3. Once enabled, adult-focused model cards are discoverable and visibly labeled **Adult-focused (18+) · Explicit**, with a source/evidence indicator and a separate license indicator. No de-prioritization solely because a model is adult-focused.
4. Treat model cards, keyword tags and upstream metadata as **claims requiring interpretation**, not reliable classification. A missing tag produces **Content classification unverified**. Review and user-report correction paths are needed; no silent auto-promotion to General purpose.
5. A general-purpose model may produce explicit imagery; this does not automatically reclassify every output as explicit. Do not over-promise that labels guarantee all behavior.
6. Mature but non-pornographic classifications should carry their own distinct labels; do not mislabel them pornographic.
7. An adult discovery preference must not delete, hide, reclassify or rewrite user-imported local media. A separate user-controlled privacy/visibility feature may govern library preview presentation.

## Previews and consent

- Adult-model discovery opt-in **never** implies permission to render explicit thumbnails automatically. Keep thumbnail grids blurred/covered or show a neutral placeholder until the user explicitly opens an individual preview.
- Do not include explicit images in first-run tours, notification toasts, desktop taskbar thumbnails, crash reports, recommendation banners or hover-preview autoplay.
- Generated files inherit a **content status separate from model identity**. If not classified by the user or a qualified, disclosed mechanism, mark them unreviewed rather than silently ordinary.
- When the user intentionally opens an explicit file or project, enable normal playback/editing without unwanted repeated warnings. User-controlled privacy mode remains independent.
- Model install/activate and relevant export/share interfaces show license and intended-use notices without conflating license permissions with audience labels.

## Non-negotiable boundaries

- No sexual content involving minors, including fictionalized or AI-generated depictions.
- No nonconsensual intimate imagery or sexual deepfakes of identifiable real people. Consent and authorization must be meaningful; a checkbox alone is not evidence.
- Follow applicable law, required age restrictions, and upstream model-service/license conditions. Avoid claiming all discovered models are legally distributable or executable.
- Do not assert that an automated classifier can reliably determine age, consent, or legality. Implement reporting/escalation, source provenance and targeted safeguards in the generation/download flows that actually exist.

This is not a blanket local-media censor or a justification to scan unrelated private files. Keep safeguards scoped, transparent and proportional.

## Interface language (10th-grade reading level)

- Setting: **Include adult-focused models (18+)**. Default off.
- Catalog label: **Adult-focused (18+) · Explicit**.
- Ambiguous source label: **Content classification unverified**.
- First-time consent: "Adult-focused models can produce sexually explicit material. Confirm you are 18 or older and want to include their listings. Explicit previews stay hidden until you open them."
- Preview: **Show explicit preview** (separate per-preview action).
- Download page: show model name, checkpoint revision, license, permissions, known restrictions and explicit-content notice.

## Privacy and persistence

The current UI preference is **session-only**. The future stable application should persist preferences locally in protected per-user settings, not upload them or include them in telemetry. Consider shared machines and OS-user separation. No adult preferences or generated prompt text should leak into logs or crash reports.

## Qualification criteria

1. Default search excludes verified adult-focused entries in every recommendation surface.
2. Opt-in shows those entries with the 18+ Explicit label and allows adult models to be selected when supported and lawful.
3. Opt-in alone never exposes explicit thumbnails, animations or hover previews.
4. Unreviewed model records are visibly unreviewed, rather than falsely generalized or hidden as adult.
5. A general model's explicit output can be flagged separately without relabeling all model outputs.
6. Disabling discovery leaves user-owned local projects and imported media intact.
7. Age confirmation alone is **not** claimed to satisfy statutory age-verification requirements.
8. Automated and manual tests confirm minors/nonconsensual intimate-image protections before any production generation release.

The pre-alpha UI presently only meets the **session preference copy and interaction** portion of this contract. Catalog enforcement, previews, output-level labels, safeguarding and legal review remain unimplemented.
