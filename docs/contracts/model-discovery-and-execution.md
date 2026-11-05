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
