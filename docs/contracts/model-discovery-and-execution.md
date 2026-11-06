# Model discovery, compatibility and license contract

## User-facing states

- **Discovered:** Hugging Face returns a repository with a possible capability. It has not been executed.
- **Supported:** an Optic Rift adapter recognizes an explicit model family and published revision/format with documented compatibility requirements.
- **Verified:** an actual qualification receipt for this model revision and this class of hardware/driver exists.
- **Installed:** the approved local weights and isolated environment exist and pass integrity checks.
- **Ready:** installed, healthy, enough current disk/VRAM, terms accepted for the intended use, and worker test passed.

Discovery never implies the later states. Do not silently promote an unverified model based on names, downloads, likes, model-card tags or an LLM guess.

## Capability examples

Text-to-image, image-to-image, image-to-video, text-to-video, inpainting, outpainting, segmentation, video-to-video, upscaling, restoration, interpolation. A single model can carry multiple *advertised* capabilities; adapter evidence is separately scoped to each capability.

## Provenance

Persist model repo ID, immutable revision SHA, file hashes, inference adapter and version, license **for code and weights**, inherited base-model restrictions, gated access status, user acceptance record, seed, parameters and generation output hash. Mark missing information unknown and do not infer commercial rights.

## Execution boundary

Model workers execute in separate, minimally privileged Python processes or environments. No arbitrary model repository code runs in the desktop process. Never default to trust_remote_code, pickle weights, arbitrary install scripts, or privileged shell activation. The user explicitly approves downloads, terms, compute use, and any network inference. No built-in model is mandatory to view/edit/export ordinary media.

## Failure UX

Explain plainly: "We found this model, but Optic Rift doesn't yet know how to run it"; "This model requires more graphics memory than currently available"; "This model allows personal use only"; or "Sign in to Hugging Face and accept the model's access terms."

Unknown rights, unavailable GPUs and driver issues are blockers, not low-confidence green indicators.

## Audience/content labels are distinct from capabilities and license rights

A model's task capabilities (text-to-video, inpainting, etc.) and intended audience/content classification are independent fields. An adult-focused model is **not** a new technical capability or a reason to prevent discovery of lawful tools. Do not conflate adult content, noncommercial restrictions, or gated access.

- Use **Adult-focused (18+) · Explicit** for a model specifically intended for generating pornographic/sexually explicit material, backed by review of an appropriate source. Include model-card evidence and review status.
- Use **Mature themes** for non-pornographic but potentially sensitive adult themes where evidence supports that label.
- Use **General purpose** only with reviewed evidence; otherwise use **Content classification unverified**, not an unsupported "safe" claim.
- A general-purpose model may still generate explicit outputs. **Output labels are assessed separately from model audience labels.** Do not infer every output is explicit or nonexplicit based solely on the originating model.
- Adult discovery is off by default, opt-in, and scoped independently from an explicit-preview reveal. Opting in does not autoplay explicit previews or silently change the selected inference engine.
- Show content classification, evidence/source, model capability, license, support state and gating status separately.
- Show clear terms again at explicit download, model activation and when necessary for sharing/export.
- **Never silently suppress lawful content from a user-owned local project** merely because adult discovery is disabled; the discovery preference controls browse listings, not the user's existing files.

See [adult content and preview contract](adult-content-and-preview.md). Model discovery currently does not run in the pre-alpha shell; these are forward-compatibility requirements.
