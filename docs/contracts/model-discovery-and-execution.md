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

## Hugging Face access

Optional read tokens are entered, tested and stored through the [dedicated Hugging Face access contract](hugging-face-access.md). Credential acceptance is distinct from gating, model licensing, format support and runtime readiness.

## Execution boundary

Model workers execute in separate, minimally privileged Python processes or environments. No arbitrary model repository code runs in the desktop process. Never default to trust_remote_code, pickle weights, arbitrary install scripts, or privileged shell activation. The user explicitly approves downloads, terms, compute use, and any network inference. No built-in model is mandatory to view/edit/export ordinary media.

## Third-party runtime installer disclosure and identity

- Before installing a Python runtime, show the absolute resolved path and SHA-256 of the **third-party** `uv.exe` selected by the host. A separate explicit acknowledgment authorizes running that particular executable and downloading the disclosed Python packages.
- Require the same checksum over native IPC and rehash the selected binary before **each** installer process launch. If it changes, refuse instead of silently invoking another executable. The checksum identifies the selected bytes, **not** their publisher or integrity against any official release; the local executable can also change between the check and process creation.
- Native installer qualification still requires authentic uv publisher provenance and hash-locked wheel resolution. Version pinning alone is insufficient. Nothing here establishes a general supply-chain GO.

## Installed weight integrity at use time

- Downloaded model files receive verified SHA-256 values during installation. The cheap `Installed` listing checks file presence and size only; it is **not** a full integrity verification.
- Before every fresh GPU **model load**, Optic Rift re-hashes every recorded model file and compares the digest to the installation manifest. Same-size modifications, missing files, unexpected links/junctions within a revision, and invalid manifest digests fail closed before any weights enter the inference worker.
- Warm generations reuse the already loaded worker without re-hashing gigabytes on each image; after an idle unload, the next load verifies again. The GPU-sharing availability snapshot is repeated after disk verification because other apps can become active during the hash pass.
- This detects changes present when verification runs, **not** replacements by a separate local process after the hash check, and it does not authenticate who authored or signed the model weights in Hugging Face. Real Windows verification and performance timing at the new exact commit remain required.

## Generated-output export privacy

- In-app outputs remain under the app-owned local directory until the user explicitly chooses an export destination.
- Windows Pictures can be redirected or synced by OneDrive or another provider. **Before** a Save to Pictures action, disclose that the generated image and its provenance record (including the prompt and model/settings details) may be uploaded through the user’s folder-sync settings. Require confirmation or do not copy.
- This confirmation is a user notice, not a guarantee that local or synced folders will remain private. Never call an untested save path cloud-free, and do not infer sync state only from a path string.
- Save/Copy success and project asset-lineage registration are different states; project persistence remains pending the owner decision in issue #5.

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

See [adult content and preview contract](adult-content-and-preview.md).

## Implementation status (observed, not accepted design)

Branch `feature/local-image-generation-20261105-no-ci` implements a first slice of this contract. Its source is the authority for behaviour; this section only records what exists and how it was checked.

- **Discovery** (`src-tauri/src/hf_catalog.rs`): anonymous Hugging Face search for all eight capabilities above, using the source's `pipeline_tag`/tag filters. License, gated status, revision SHA and file listing come from structured metadata. Model-card prose and preview images are never fetched. Listings tagged `not-for-all-audiences` by the source are withheld unless adult discovery is opted into, then badged 18+. Every other listing is "content classification unverified".
- **Supported** (`src-tauri/src/adapters.rs`): decided per pinned revision from `model_index.json` and the actual file listing. The rules are an exact pipeline class, components loaded only from `diffusers`/`transformers`, safetensors weights only, published hashes required, and no code or pickles. One adapter exists: `diffusers-sdxl-text-to-image` (StableDiffusionXLPipeline). Gated repositories whose files cannot be read are reported as "compatibility can't be checked yet", not as unsupported.
- **Verified**: the registry entry `VERIFIED` holds only receipts from executed runs on a named hardware class.
- **Installed** (`src-tauri/src/model_store.rs`): explicit consent bound to the license string shown for that exact revision. A disk check runs first. Downloads are resumable and restricted to HTTPS `huggingface.co`/`*.hf.co`. LFS files are checked against SHA-256 and other files against their git-blob SHA-1. The manifest (repo, revision, adapter, files with SHA-256, license, consent timestamp) is written only after every file verifies.
- **Ready**: the model is installed, the pinned runtime (`src-tauri/src/runtime.rs`) passed a CUDA health check, and an NVIDIA GPU is detected. A VRAM preflight runs before load.
- **Execution** (`src-tauri/src/generation.rs`, `src-tauri/workers/diffusers_worker.py`): a separate Python process using `python -I`, offline (`HF_HUB_OFFLINE`, no tokens in its environment), loading with `use_safetensors=True, local_files_only=True`. It supports progress, cancel, OOM recovery and rejection of blank or NaN output. The host re-hashes the PNG and checks its header, then writes a provenance JSON per output: prompt, parameters, seed, repo, revision, adapter, weight hashes, runtime versions, output hash and metrics.
- **GPU sharing** (`src-tauri/src/gpu_guard.rs`, `process_tree.rs`): before every load and every job, Optic Rift reads Windows GPU performance counters. These give dedicated VRAM and 3D-engine utilisation per process on the NVIDIA adapter. Optic Rift then proceeds within a budget that keeps at least max(1.5 GB, 10%) free for other software. If another app holds 768 MB or more, or uses 10% or more of the 3D engine (for example a game), the user is asked first and the dialog names the app. If the job wouldn't fit without squeezing other apps, it is refused. The worker applies `torch.cuda.set_per_process_memory_fraction` as a **PyTorch caching-allocator limit**, not an absolute bound on native CUDA allocations or system GPU use. This snapshot-based sharing check can become stale if another application starts a GPU job; **do not promise that Optic Rift cannot affect games or will always preserve that reserve**. A real GPU OOM stress test must not be repeated on a machine being used for other tasks. The worker releases cached working memory after each job. The model unloads after 5 idle minutes. The worker tree runs in a kill-on-close Job Object, so it ends with the app even if the app crashes. This was added after a stress test crashed the owner's PC while a game was running.
- **Measured** (release app, RTX 5070 Ti 16 GB, driver 621.01, `segmind/SSD-1B@60987f37`, fp16): 1024×1024, 25 steps: 14.5 s on a cold start (model load 9.1 s), 3.9–5.4 s warm. Peak PyTorch allocation 8,349 MB. About 6.25 GB in use system-wide between jobs, down from 14.5 GB before cache release. The same seed gives bit-identical PNGs. Cancel takes 0.8 s and leaves no output. No network connections from the app or worker while generating or for 60 s afterwards.
- **Not yet:** project asset lineage (outputs live in Optic Rift's output folder; decision #5 is pending), hash-pinned Python wheels, a bundled runtime (it currently requires `uv`), non-NVIDIA GPUs, and adapters for any capability other than SDXL-architecture text-to-image.
