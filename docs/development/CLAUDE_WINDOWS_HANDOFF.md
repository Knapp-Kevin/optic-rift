# Optic Rift: Claude local Windows development and qualification handoff

Status: Actionable **handoff**, not a Qortara acceptance, Windows PASS, signed release, or authority to merge. Created 2026-11-05. Refresh all live SHAs before acting.

## Mission

Move Optic Rift toward its first **genuinely functional offline Windows creative studio**. Begin with evidence-driven build and testing of the existing Tauri/React/Rust foundation and its narrow project metadata store. Then repair concrete failures and implement one small user-visible vertical slice at a time. Work in the repository, not in another planning loop.

**Canonical truth:** https://github.com/Knapp-Kevin/optic-rift/blob/main/docs/ARCHITECTURE_INDEX.md. Read it, root README, AGENTS.md, and governance/manifest.json before edits. The architectural index is an **explicit file**, NOT docs/README.md; docs/README.md is only a navigation page. Each architecture source owns its own claims. Accepted decisions, real source, tests and release receipts remain separate.

## Verified GitHub starting snapshot (not an assumed future state)

- Main: 2230f43459fe8e10a1c895a2189a7e13ab2969eb, with the explicit canonical index from merged PR #12.
- Desktop foundation: draft PR #1, feature/windows-desktop-foundation-20261105-no-ci, head d81eaf001f222b69fa3453f3506f539241d99911. Contains Tauri 2, React/TS, Rust shell, per-session media previews, draft ADR-0001 and runtime/model/content rules. **No confirmed Windows compile or native export.**
- Project metadata foundation: draft PR #13, implementation/f1-project-store-20261105-no-ci, initially head e90912e47c04e8aaeeae952fa1dff95671081dfb (read current live head). PR #13 is **stacked on PR #1**, not on main. Adds src-tauri/src/project_store.rs, native create/list/open commands and 3 declared Rust tests. No claim tests ran.
- Design alternatives/architecture owners: draft PR #11, architecture/creative-workflows-wayfinder-20261105-no-ci, initially head fc677d359521c9033f66fb4523c4485996731949. Eight **Proposed** architecture owners and Wayfinder decision map #4, not yet accepted.
- Governance path correction: draft PR #14, governance/protect-explicit-architecture-index-20261105-no-ci, initially head 9637e06d219bbf94ec1869a522c10d8edd433620. It adds canonical index and docs/architecture path protections; must be independently reviewed and Qortara-qualified, not merged just because GitHub can merge.
- Governance issue #2 remains open: main unprotected, no enforced rulesets, native index checker/ledger not established.
- The architecture and execution branches have different bases. **Never assume their files are all in main.** Compare the exact PR branches with main before merging or rebasing. Do not force push.

## Prerequisite: this MUST be a real local Windows host for Windows acceptance

This prompt is intended for Claude Code with a checkout and shell access on a Windows 11 development machine. A Claude cloud session without access to the Windows machine **cannot** qualify the Windows Tauri app. If on Linux/cloud, do source-level work there, record the host limitation, and hand real Windows smoke/codec/GPU gates to a Windows host. Never claim the user's RTX 5070 Ti or Ryzen CPU was observed without checking.

Record:
- git status, branch, HEAD and origin/main and PR head SHAs;
- OS/build, CPU, GPU, VRAM, NVIDIA driver and available hardware encoders;
- node/npm, rustc/cargo/rustup, C++ Build Tools, WebView2 and FFmpeg availability;
- current model/license/network constraints and test output artifacts.

Do not install GPU models, accept gated model licenses, retrieve Hugging Face tokens, run paid APIs, enable cloud inference, or upload media without owner consent. No new GitHub Actions/CI consumption. Install standard development dependencies only when required and record the changes. Do not write any files under .qortara/ by hand. Do not forge independent reviewers, gate verdicts, signatures or ledger events.

## Work sequence

### 1. Sync and establish a clean baseline

In local PowerShell from the repo root, inspect before any mutations:

~~~powershell
git status --short --branch
git remote -v
git fetch origin --prune
git rev-parse origin/main
git branch --all
node --version
npm --version
rustc --version
cargo --version
rustup show
nvidia-smi
Get-Command ffmpeg -ErrorAction SilentlyContinue
~~~

Missing tools are blockers to **that** test, not evidence of working software. Check build requirements for Tauri 2 on Windows: Rust stable MSVC target, Visual Studio C++ Build Tools, Windows SDK, WebView2. Avoid opaque installers that silently change the machine. Keep any preexisting uncommitted work intact.

First validate PR #1 on its **exact current head**. It is older than main, so do not overwrite the canonical index with an obsolete README/doc branch. Record the ancestry and reconcile branches carefully after tests.

### 2. Execute real gates on PR #1

~~~powershell
git switch feature/windows-desktop-foundation-20261105-no-ci
npm install
npm run test:content
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run tauri dev
~~~

Note if no npm lockfile exists. Create and review a package-lock.json for reproducibility if dependencies resolve correctly; do not treat a new lockfile as a harmless cosmetic change. Run the Tauri binary, not only Vite in the browser. Record real command outcomes, version info and stderr. If some commands cannot run, mark **BLOCKED**, not PASSED. Fix build errors on a scoped branch with tests, commit exact changes and update the appropriate PR.

Windows smoke checks (real users' media, not AI generation): launch, import a still and a short H.264/MP4, preview both, seek repeatedly, test unsupported codec handling, switch media, close/relaunch, verify no false claim of persistence, inspect memory/CPU responsiveness and lack of network/model side effects. 0.1-second seek buttons are not frame-accurate extraction; do not represent them as such. Do not claim ordinary UI preview filters produce edited export.

### 3. Qualify PR #13 as a separate stacked layer

From the **actual current** implementation/f1-project-store-20261105-no-ci head, run the same build/Rust gates above. Specifically test the three Rust native project commands:
- create a named project under the app-specific per-user data path;
- list and reopen it after process/app restart;
- reject empty/oversize/control-character names and traversal/malformed IDs;
- reject or accurately report corrupt, symlinked, oversize or mismatched project records;
- test permission failure, disk full, and interrupted temp file conditions where safe;
- check that project metadata cannot delete/move/overwrite original user media;
- verify temporary failures do not create false successful projects.

Review crash consistency and concurrency. In particular, consider whether a pre-check followed by rename is race-safe on Windows, directory symlinks or reparse points are handled, and whether temp files can remain after a kill. Do not assume parent directory sync works identically on all platforms. Add focused regression tests and fix actual defects, with exact receipts.

The runtime flag project_storage_ready deliberately remains false until the **UI and durable media linking actually work**. Do not flip it to green just because an empty project JSON can be written.

### 4. First user-facing vertical slice after hard gates

Within the accepted scope, implement a small, genuinely functioning project chooser in the desktop UI that calls the native create/list/open commands, clearly shows errors, and survives app restart. Do not claim this manages video/source-media persistence until that subsystem exists. Prefer a minimal useful journey over a sprawling editor.

Document what actually works at the exact head and what does not. Add tests for the UI interaction where possible, plus a real Windows manual acceptance receipt. Re-evaluate the design against Wayfinder project lineage decision #5 before adding linked media, new persistent asset schemas, or a separate project store.

### 5. Remaining architecture and governance

Review draft PR #11 against the **explicit** docs/ARCHITECTURE_INDEX.md. Preserve its eight proposed owner documents and the outlier register; do not accept architecture merely because the documentation PR merges. Resolve the decision tickets with real owner judgment when a decision is HITL, and record implementation/evidence separately.

Review draft governance PR #14 independently and validate against the current Qortara Logic policy schema and native lifecycle; preserve actor identity and required distinct reviews. Native Qortara's architecture-catalog checker defaults to docs/README.md and can be configured for docs/ARCHITECTURE_INDEX.md only through the authorized runtime/configuration surface. **Do not hand-edit .qortara/ or invent a native PASS.** GitHub main branch remains unprotected in the last observation, so ruleset configuration requires authorized GitHub administrative action and readback.

### 6. Follow-on after a qualified F1 project selector

Next architectural implementation target is **deterministic native media processing**: trustworthy probe/preview/proxies, timebase-accurate video frame extraction and image crop/zoom/export, without any model installed. Start with the smallest actual frame-to-photo journey, then add video trim and native export. Measure actual Windows seek latency, dropped frames, audio synchronization and resource use before deciding whether Tauri/WebView2 works or a Qt/native preview fallback is justified.

Do not download video models to compensate for missing FFmpeg/media functionality. Do not implement the expensive multi-shot AI chain before its lower-level asset and media operations exist.

## Tests, decision evidence and finish criteria

- **Branch discipline:** use a separate task branch or existing scoped draft PR. Commit and push only authored changes, with source/test/document links. No force push, auto-merge, release/tag or main mutation without owner authority and required Qortara gates.
- **Real checks:** command/executable, git SHA, device/OS, stdout or summarized result, exit code, and any failures. Distinguish source static checks from real Windows UI/media execution.
- **No false green:** tests passing do not equal image/video editing UX success; a compile is not frame accuracy; a UI screenshot is not a successful export; a mergeable PR is not governed approval.
- **Privacy:** use synthetic or owner-authorized footage; never upload user originals, private model metadata, or tokens into PRs/logs.
- **Performance:** do not introduce a UI-main-thread decoder/transcoder. Keep editor responsive under GPU work; record p50/p95 and actual load where measured, not invented thresholds.
- **Handoff output:** publish a concise end-of-session report and PR comment or sanitized evidence file that shows: exact branch/commit, changes committed/pushed, Windows/Rust/TS results, observed user journeys, failures, governance gaps, open PRs, and next concrete action. Do not close issue/PR just for low counts.

**Working priority:** get PR #1 compiling and behaving on a real Windows host; fix failures; test/repair PR #13; deliver an honestly labeled project-selector slice; only then expand into cross-media generation and higher-order workflows.
