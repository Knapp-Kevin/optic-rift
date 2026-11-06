# Optic Rift PR #20 — Windows native/GPU qualification handoff

Status: **DRAFT / NO-GO for merge and release**. This document records a local Windows execution plan, **not** qualifying evidence or reviewer approval.

Repository: `Knapp-Kevin/optic-rift`  
PR: https://github.com/Knapp-Kevin/optic-rift/pull/20  
PR branch: `feature/sdxl-img2img-worker-20261106-no-ci`  
Code head before this handoff document: `03cce45c3727a369c474492fd3a9131d1d798ae0`  
Base/dependency: PR #18 at `77fb8700489aaa59584429b88125ffa12d9b2064`.

**Executor:** an explicitly authorized Windows-local agent (Claude) on the owner's Windows 11 machine. **Authority boundary:** test and fix within the current PR branch; do not merge, publish, sign, accept governance on behalf of a reviewer, change protected-branch settings, access personal credentials, or claim a public-alpha GO. Do not launch GitHub Actions; use `[skip ci]` for test-fix commits.

## Already observed outside Windows

- Text-to-image and img2img source code exists, but this exact PR head has **no qualified end-to-end Windows image-to-image GPU execution**.
- Direct Node 22.16 CPU test `node --experimental-strip-types --test tests/imageToImage.test.mjs`: **4/4 PASS** on the exact GitHub blobs `9ca3c4ef47ae6992230fd1207e9f104b84516299` and `1098460a11bf1553d62b8a867d9c7b4b95feec72`.
- Three new CPU-only tests for atomic image publication exist in `test_diffusers_worker.py`, alongside a hardening of `diffusers_worker.py:save_png`. The *exact repository Python test module* has **not** yet been run on this head. Separately, an isolated Linux/Pillow 12.3 verification of the identical function logic passed 3/3 (output digest, existing destination, pre-existing partial). That is not Windows or end-to-end evidence.
- Rust `gpu_check` nested `Result` handling was repaired; an img2img zero-effective-denoising-step guard was added in Studio, Rust GPU preflight/generate, and Python. None of the newly changed native Rust code has been compiled here.
- A prior Windows SSD-1B **text-to-image** result on an earlier PR #18 ancestor is **not transferable** to this PR #20 head.
- Project assets and portable lineage remain owner decision #5; app-local generation history and optional `project_id` alone do not qualify this.

## Prerequisites and safe checkout

1. First confirm the current remote PR #20 head and PR #18 base using GitHub. Identify the HEAD SHA in the receipt. If either has moved, assess new diffs before executing; do not imply the pinned source still applies.
2. Work in a clean clone or preserve any existing local edits without destructive reset/clean. For a new checkout in PowerShell:

   ```powershell
   git clone https://github.com/Knapp-Kevin/optic-rift.git
   cd optic-rift
   git fetch origin feature/sdxl-img2img-worker-20261106-no-ci
   git switch --track origin/feature/sdxl-img2img-worker-20261106-no-ci
   git rev-parse HEAD
   git status --short
   ```

3. Verify toolchain versions and Windows GPU state without consuming GPU memory: `node --version`, `npm --version`, `python --version`, `cargo --version`, `rustc --version`, `nvidia-smi` if installed. Identify the Python interpreter with Pillow and the installed pinned image runtime; do not silently switch interpreters. Keep private model/token paths and prompt contents out of public receipts.
4. Run all checks locally; **do not deliberately cause OOM, run high-pressure benchmarks, or test concurrently with gaming**. In a previous stress run the occupied host crashed. Use the idle GPU only with owner approval. Do not download model weights or install untrusted executables as part of this qualification.

## Gate A — source, CPU, build, native compile

Run and capture each exit code, count and failure output. On Windows PowerShell (within repository):

```powershell
npm ci
npm run test:content
npm run build
python -m unittest discover -s src-tauri/workers -p "test_*.py" -v
cargo fmt --all --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml --locked
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
```

Any `Pillow` skips mean reference/decode/publish tests remain **unqualified** in that interpreter. Run the actual compatible pinned runtime interpreter if needed and report it separately. Ensure `test_diffusers_worker.py::SavePngSafety` passes the real Windows NTFS `os.link` behavior. If an installed volume denies hardlink creation, fix fail-closed publication or explicitly block release; do **not** restore overwriting `os.replace`.

Investigate compiler/type errors, test assertion failures, Windows path behavior, isolated Python `-I` loading of `img2img_reference.py`, and any unexpected network calls. If fixes are required, make narrowly scoped commits on this PR branch with `[skip ci]` and rerun failed checks **at the new exact head**. Never fabricate a pass for an environmental skip.

## Gate B — non-destructive local behavior

With the owner's approval and GPU otherwise idle:

1. Launch the actual Windows Tauri desktop application, reusing a previously installed hash-verified and consented SDXL/SSD-1B runtime/model. Record application/Rust/Python/GPU driver versions. Confirm local generation is offline and no token is passed to the worker.
2. First generate a normal text-to-image PNG. Record a **non-sensitive** job UUID, dimensions, output SHA-256 and model revision. Do not publish the user's prompt.
3. Enter **Image to image** and select that previously generated PNG. Run 25 steps at strength 0.65 with a moderate image size and conservative idle-GPU setting. Verify a new job UUID/output PNG/digest, expected dimensions, source job+SHA lineage in the new provenance, and unchanged original image bytes/digest. Verify images open from **Recent generations** after app shutdown and restart without model re-execution.
4. Check Studio's validation for 1 step @ 0.65 and 10 steps @ 0.05; both must fail before source decode or GPU work. Check 20 @ 0.05 and 25 @ 0.65 are accepted for preflight, not presumed to succeed on GPU. Preserve the prior text-to-image behavior.
5. In disposable app-owned fixtures, exercise missing/corrupt PNG or provenance, mismatched hash/dimensions, linked/reparse references if the Windows test account permits, duplicate final destination and pre-existing `.partial`. Verify **no overwrite, no unexpected writes, fail-closed error, intact sources**, and temporary cleanup.
6. Check normal Cancel; honest GPU-busy/block/confirm messaging; idle unloading/recovery and GPU dedicated-VRAM baseline/peak where available. The additional provisional 2048 MiB img2img estimate is **not a hard total-device VRAM cap**, and allocator limits do not govern other applications. Do not create an unsafe competing load solely to test a race.
7. Confirm WebView2 privacy: no unintended exfiltration of token/prompt, no arbitrary path import, no network loads from a model card, and explicit disclosure before Pictures/possible OneDrive sync exports. Preserve opt-in, clearly labeled 18+ **discovery** behavior without enabling execution of unsupported models.

## Gate C — evidence, review and integration boundary

Attach a dated report to PR #20 with:

- exact starting and ending commit SHAs, PR base, dirty/clean status and what changed;
- Windows build/runtime/driver/GPU, Python interpreter, all command outcomes including skips;
- actual UI journey steps, expected-versus-observed results, output and source integrity evidence, memory observations without private prompt/token/media disclosure;
- defects fixed with files/commits, any remaining blockers, risk assessment and explicit **GO or NO-GO only for your local qualification**;
- explicit distinction between local tests, an independent security review and authentic Qortara governance approval.

**Do not merge or mark public-alpha ready.** PR #18 remains draft and blocked on publisher-trusted uv/wheel hash locking plus independent/Qortara review. PR #16 is ready for review but unmerged. Optic Rift issue #2 is still unresolved: main's GitHub branch policy does not itself enforce a distinct approval. Any release or merge needs independently authorized governance and validated dependency order. Preserve all currently approved architecture docs/artwork; do not add a speculative content filter or broader adapters.

## Completion criteria

Local qualification is complete only when every required source/build check is reported on the exact final head; an actual Windows image-to-image GPU run has been observed without damaging existing data; source/provenance/restart tests pass; unsafe workloads were avoided; and remaining security/governance/release blockers are itemized. **Passing tests alone is not authority to merge.**
