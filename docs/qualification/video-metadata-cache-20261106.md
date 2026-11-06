# Optic Rift video-model metadata cache: exact-head Windows handoff

**Status:** implementation started, native/Windows qualification **NOT YET RUN**, **NOT MERGED**. This does **not** complete issue #22.

This branch is deliberately independent of Claude's active Studio PR #25. It is based on PR #18 head `77fb8700489aaa59584429b88125ffa12d9b2064` and touches `hf_catalog`, `video_metadata_cache`, model IPC, ModelBrowser, model types and tests. **Avoid replacing or rebasing Claude's live PR #25 work**. Integrate through reviewed dependency order later.

## User outcome and honest boundaries

Optic Rift is intended to discover text-to-video, image-to-video and video-to-video model families, including clearly labeled source-flagged adult listings. A missing execution adapter is **not** a policy ban. However, discovering or caching a configuration file **does not mean that model weights are installed or that video generation runs**.

This increment implements one cautious, non-executing **public JSON configuration cache**, not full model-file download. It accepts only an upstream hash-addressable, pinned, <=256 KiB `model_index.json` or root `config.json` from a declared video task. No `.safetensors`, model code, arbitrary repo files, video previews, CDN redirect, gated repo credentials, Python worker or GPU calls are involved. Download-only **weights** require a separate reviewed design and remain blocked under issue #22.

Model source audience, license, gated access, file format, hash integrity, technical support, and execution readiness are independent states. The UI explicitly says **Cache configuration metadata only**.

## Security architecture

1. Metadata-only plan from server-side `hf_catalog::video_cache_candidate` for three video capabilities only, exact known JSON file path, verified uppercase-free 40-char Git SHA-1 blob or 64-char LFS SHA-256, bounded source file size.
2. Client approval records exact repository ID, immutable 40-hex revision, capability, license acknowledgment, optional source-flagged 18+ acknowledgment and transfer consent. **This acknowledgment is not legal age verification.**
3. Native `video_cache_metadata` **re-inspects** the same pinned revision through `inspect_public` which never retries with a stored HF token, checks source-advertised pipeline task, gates, access, license and audience. No model file path or direct URL comes from the user.
4. A separate anonymous-only HTTP client fetches the exact `resolve/<sha>/<fixed-json>` path with HTTPS, redirects disabled, max 25s, bounded byte-stream (<=256KiB) and source-size match. Validates Git blob SHA-1 or LFS SHA-256 and parseable JSON.
5. Persist under app-local `video-model-metadata/<sha256(repo-id)>/<revision>/` with each directory checked against symlink/Windows reparse points, unique create-new staging and hard-link no-overwrite publication. Write a bounded JSON receipt last; only a hash-verified pair earns `cached_metadata=true`. Read-only inspection must not create cache directories.
6. Native `video_cache_clear` requires approval, checks exact repo/revision/receipt/status, verifies contents and refuses unexpected directory entries before deleting **only** the known config and receipt. No recursive delete.
7. No user prompt/token logs and no proactive image/GIF preview fetch. No install/ready promotion. Existing SDXL model installation path remains unchanged.

**Limitations to evaluate:** Path-based reparse checks cannot eliminate every hostile same-user rename race without handle-based Windows file operations; existing application folder may be subject to local privilege/ACL context; receipt creation is last but two-file publication is not fully atomic (a crash can leave an orphaned config that intentionally fails closed until manual cleanup); metadata is plain JSON in local app data, not an encrypted private vault. These are review items, not advertised guarantees.

## Required local Windows work

1. Record the **live** branch SHA and file-tree cleanliness; do not overwrite concurrent changes.
2. Run the following from the repository root using a properly configured Windows toolchain:

```powershell
git status --short
git rev-parse HEAD
npm ci
npm run test:content
npm run build
cargo fmt --all --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml --locked
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
```

3. Fix any compiler, clippy, TypeScript or regression failure on **this** branch, with `[skip ci]`, and rerun final exact head. Do not run GitHub Actions.
4. Offline fixtures: every video capability query, unknown/incorrect task, no license vs mismatched license, adult-flagged off/on, revoked/gated, unsupported or missing file hash, source hash mismatch, invalid JSON, oversized file and path, no overwrite, unknown cache entries, no recursive clear.
5. On the owner-approved **network** (GPU not needed), inspect pinned public metadata such as `Wan-AI/Wan2.1-T2V-1.3B` and `Lightricks/LTX-Video`. Record **actual** live `sha`, license, task tag, source `blobId` and file size. Do not automatically fetch model preview/GIF, model card media, weights or any giant downloads.
6. With owner approval, cache exactly **one small public JSON config file** from a verifiable task-tagged repo. Check content SHA, receipt, restart listing, duplicate-cache refusal, no-change-of-source, and explicit safe clear. Test a negative gated attempt **without** any real user credentials. Use fixture-only records for 18+ source labels; no explicit generated media required.
7. Verify network monitoring shows only expected public metadata requests during an explicitly approved cache action and no connection attempt on a denied consent, wrong task, gated, or unsupported download.
8. UX: show cacheable/not cacheable, cached/not cached, separate install/ready flags; permissions and explanatory copy are visible; accessible keyboard interactions; no adult thumbnails, autoplay or unreviewed content previews.
9. Publish one **real** pass/fail qualification receipt including exact SHA, native commands/test totals, fixture matrix, public model SHA/file identity, filesystem results, failed operations and known limitations.
10. Independent security review and authentic Qortara authority are still required for merge. Preserve upstream PR #18 dependencies, owner-specified merge ordering, and project governance. **A PR is not done until authorized merge into main and post-merge verification.**

## Integration and dependency notes

- Parent issue: [#22](https://github.com/Knapp-Kevin/optic-rift/issues/22); associated native source-flagged model admission [#23](https://github.com/Knapp-Kevin/optic-rift/issues/23), private outputs [#24](https://github.com/Knapp-Kevin/optic-rift/issues/24).
- Main merge chain currently includes PR #1, project store, PR #16, PR #18, PR #20, PR #25. **Do not collide with Claude's active changes.**
- This PR may need its base moved after parent merge; update base only after verifying the exact ancestry and conflict resolution, and rerun tests on the resulting integrated head.
