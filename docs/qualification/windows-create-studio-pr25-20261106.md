# PR #25: Windows qualification and merge checklist

**Status: NOT QUALIFIED / NOT MERGED.** This is not a completion receipt.
**Repository:** Knapp-Kevin/optic-rift
**Target:** PR #25, `feature/create-studio-workspace-20261106-no-ci`, stacked on PR #20's previously Windows-qualified `3af04facc4f86ebbf7fb187228d7b8a0982614d5`.
**Owner acceptance:** Create defaults only when a qualified local image engine is available; Edit media remains usable; add a native read-only bounded thumbnail path with privacy safeguards. See issue #21.

## 0. Preconditions and safety

1. Start with a clean worktree and fetch the **actual current** PR #25 head; record its complete SHA before and after each fix. Do not overwrite concurrent work or rebase under an unobserved branch move.
2. No GitHub Actions on this task; use local Windows. Commits should include `[skip ci]`.
3. Run static/CPU/source checks first. Only run real T2I/I2I generation after explicit owner approval and an idle-GPU check; do not run intentional OOM/stress tests or compete with games.
4. Use non-sensitive sample prompts, no personal/explicit images or secrets in screenshots. Do not expose HF tokens, home paths, prompts in logs/PR comments. Preserve WebView2 OneAuth suppression introduced on PR #20 and mirrored into PR #1.
5. `main` merge is **not** authorized by merely passing local tests. Check all upstream PRs #1, #16, #18, #20, independent approving review, authentic Qortara gate and owner acceptance in order. Do not fabricate a receipt.

## 1. Build/source gates (commands on Windows PowerShell)

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
python -m unittest discover -s src-tauri/workers -p "test_*.py" -v
```

Run Python suite with the installed pinned image runtime interpreter too. Record full command, versions, pass/fail counts and environment skips. If any command fails, **fix it in PR #25, re-run and bind evidence to final SHA**. Distinguish a TypeScript build from native Windows execution.

## 2. Native preview trust tests

- Exercise `generation_output_thumbnail` directly with the exact generated job UUID. Confirm the returned `data:image/png` decodes, longest edge <=256 pixels and encoded bytes <=512 KiB. Confirm no new thumbnail file, unprotected sidecar or cache appears in app-local outputs or WebView2 profile.
- Challenge with bad UUID, missing PNG, tampered PNG/hash, corrupt/missing sidecar, wrong source dimensions, non-PNG bytes, a linked/junction output directory and a linked PNG file where safe and possible. Each must reject without silently repairing or modifying source/provenance.
- Test representative generated source PNGs in RGB, RGBA, gray/alpha when available, portrait/square/landscape, plus oversized image-header fixtures. Watch CPU/runtime and memory. Ensure thumbnail data URI is never used as a full-resolution source for generation; native source must still be verified original.
- Native code creates **no persistent derivative preview files**. If private/encrypted vault mode is implemented later, reassess all webview memory and cache paths separately.

## 3. Studio UX: real Windows app screenshots and checks

Capture **before/after** against pinned issue #21 baseline at:
- 1280x720, 1440x900, 1920x1080 actual windows; 100% display scale and real 125% if feasible (if emulated, clearly label).
- Empty library/no qualified model; Create initial; text-to-image in progress; text-to-image result with action buttons; image-to-image chosen source; image-to-image side-by-side result; recent history; model settings; Edit media square/portrait image. Use a long prompt/model name and keyboard-only navigation.
- Check preview image is always aspect-contained and never cropped; Generate/Transform and prompt visible without page scrolling at 1440x900 and within reachable view at 1280x720; only settings/filmstrip scroll internally on desktop; mode switches/labels correctly announce; GPU consent and cancel are visible and keyboard-reachable; page has no horizontal overflow.
- **Privacy negative:** reopening Studio or choosing Image-to-image must not automatically expose the image contents of any prior, potentially explicit generation. All history previews start concealed; only a deliberate **Show previews** action displays thumbnails. Choosing a specific history record intentionally opens full preview. Disabling thumbnail visibility removes data from rendered gallery.
- Legacy Edit media preview, media import, timeline, and local history reopen must remain usable and correctly labeled. A downloaded/unsupported video model does not make Create “ready” or imply working video generation.
- Confirm Save copy still asks permission for possible OneDrive sync, including prompt/provenance JSON. Ordinary local storage is *not* advertised as encrypted.

## 4. Small real GPU smoke test, after owner consent

On a confirmed idle GPU only, run one moderate local T2I image, then one I2I from it at strength 0.65, 25 steps. Confirm:
- correct preview and visible stage/progress/cancel;
- model and source selected, original bytes/hash unchanged, lineage and settings correctly saved;
- no unexpected non-loopback network egress; WebView2 OneAuth WAM flag present;
- after restart, the images reopen and thumbnail show/hide behaves, with no model load required for preview;
- the process terminates cleanly and VRAM returns to idle.
Do not repeat expensive GPU runs simply for screenshot aesthetic.

## 5. Reporting and merge handoff

Post one comment to PR #25: exact source/tested SHAs, native versions and GPU conditions, source/build tests, UI screenshots with raw GitHub URLs, thumbnail security allow/deny matrix, known limitations, and independent review + authentic Qortara receipts (or explicit missing gate). If a code defect is fixed, preserve that exact-head evidence.

If all mandatory gates are met, merge **in dependency order**, verify the exact commit landed in main, and rerun post-merge essential smoke checks. If independent review, Qortara receipts or upstream supply-chain gates are missing, leave PR #25 open and state **BLOCKED; not done**. Do not close issue #21 early.
