# Photo Studio: deterministic PNG export qualification

**Status: source committed on a branch; NOT Windows-qualified, NOT merged.**
**Purpose:** implement a small, real, no-AI offline photo user journey: open PNG/JPEG/WebP; preview brightness, contrast and 90-degree rotation; explicitly export adjusted PNG to Pictures/Optic Rift; verify the original bytes remain unchanged.

This deliberately does **not** implement layers, masks, selective retouch, project-asset persistence, private-folder/vault storage, video editing, or audio functionality. See P0 [cross-media design issue #29](https://github.com/Knapp-Kevin/optic-rift/issues/29). The changes are based on PR #25's separate image-generation UI branch and must not disrupt the qualified PR #20 generation path or Claude's PR #28 lifecycle work.

## Source contracts

- Photo rendering in `src/photoEdits.ts`: only app-owned `blob:` images, PNG/JPEG/WebP, source <=32 MiB, <=8192px each side and <=20MP, encoded PNG <=32 MiB; no arbitrary remote URL and no model dependency. Source digest is browser-computed and **explicitly not native-authenticated provenance**.
- The canvas applies the same `brightness(%), contrast(%)` filter values and clockwise quarter-turn the Photo Studio preview displays. Export rasterizes the edit into a new image and strips original EXIF/ICC profiles. Color-managed photo accuracy beyond this simple operation is **not qualified**.
- Native `photo_export_copy`: Tauri Rust independently requires a consent bool, valid bounds/settings/digest, strict base64, fully decodable PNG with matching dimensions and <=80MiB decoded buffer. The client cannot nominate an export path.
- Output destination: the OS Pictures directory, `Optic Rift` subfolder. This **may sync with OneDrive**, so explicit disclosure occurs before the first render and again in native validation. Refuse symlink/junction Pictures/Optic Rift paths. Unique UUID-named PNG plus minimal `*.provenance.json` are published with no-overwrite same-directory hard links, receipt last. Staging is removed on success/failure. Metadata describes the reported source hash honestly; it is **not** a project asset.
- The original file is never passed as an arbitrary native path or overwritten. No GPU/model, download, audio or network action is intended.

## Mandatory local Windows execution

1. Reconcile current GitHub heads, check out the exact photo-export branch into a separate clean worktree. Never rewrite PR #25, #20, #26 or #28 concurrently. Record the tested full SHA.
2. CPU-only build/regression gates:

```powershell
npm ci
npm run test:content
npm run build
cargo fmt --all --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml --locked
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
```

3. Fix actual TypeScript/Rust errors and rerun the **final SHA**. PR #18 may introduce inherited fmt errors; report whether they are from unmodified ancestor files and correct at the appropriate integration boundary. Do not claim all-gates-pass on a single-file static assertion.
4. Real Windows app smoke with **synthetic, non-sensitive photos**:
   - Import a source JPEG, PNG and WebP separately; verify image dimensions/pixel colors; set 25%/175% brightness/contrast, quarter-turns 0/90/180/270 and compare preview vs native output dimensions and visible pixels.
   - One unedited copy (100%, 100%, 0) must retain expected visual pixels; the exported bytes need not match source compressed bytes because PNG is re-encoded.
   - Accept Pictures/OneDrive disclosure -> verify `Pictures/Optic Rift` has a new PNG, correct `*.provenance.json`, source original SHA-256 and mtime unchanged, and no previous export replaced.
   - Decline the cloud disclosure -> **zero disk writes**. User must be able to edit/revert/compare without needing an AI model or network/token.
   - Source >32 MiB, >8192 edge, >20 MP, malicious SVG, wrong MIME, broken image, and image that disappears while editing must report a clear failure without creating exports.
   - Call `photo_export_copy` directly with consent=false, forged dimensions, corrupt/truncated PNG, huge base64 input, invalid settings/hash and a redirected Pictures folder. All must refuse safely, no unexpected files.
   - Inspect temporary/partial files after success, rejection and forced write failures; verify no overwrite or partial assets are advertised as completed.
   - Confirm privacy: no unexpected network request, no original filename in sidecar, no unapproved cloud sync, no files written to project or private vault. Explicitly state that this export to Pictures **can** be cloud-synced.
   - Keyboard, focus, disabled state, progress/error/success status at 1280x720, 1440x900 and 1920x1080, 100% Windows scaling; test portrait and square at 90 degrees.
5. Take sanitized Windows screenshots and attach exact SHA, executed commands/results, file hashes, output proof, failure matrix and any exceptions to the PR.
6. Require distinct independent security/source/UI review and authentic Qortara approval. Merge through the established upstream dependency chain only when authorized, then verify functionality from `main`. Keep #29 OPEN for full Photo+Video studio design acceptance.

**Completion criterion:** this narrow feature's real export passes Windows acceptance and is merged+verified in main. Passing tests on an unmerged draft does not mean done.
