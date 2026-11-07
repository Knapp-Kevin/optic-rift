# Optic Rift: First-class Creative Navigation Windows Acceptance

**Status:** source committed on a separate branch based on [PR #31](https://github.com/Knapp-Kevin/optic-rift/pull/31) exact head `71642e337037a94a3a82d89d73b4065b682c6b50`. **Not Windows-qualified, approved, or merged.** Parent #31 remains untouched. Scope: implement real app navigation matching user journeys, not dummy sidebar links.

## User-visible navigation contract

Primary left sidebar, grouped by intention, with keyboard navigation and `aria-current="page"`:
- **Your Work:** Projects; Media Library.
- **Create & Edit:** Create Images; Photo Studio; Video Studio.
- **Tools:** Models; Settings.

**Media Library** contains opened session photos/videos with filename search and All/Photos/Videos filters. No unsolicited thumbnails or video playback. An explicit Open Photo Studio/Open Video Studio button selects that file and navigates to the respective workspace. Native generated-history listings can be opened in Photo Studio; that existing output is not automatically an asset owned by a project. Imports remain session-only, which is explained in UI.

**Create Images** hosts the existing qualified-local-model-based generation UI. The component remains mounted but hidden after first visit, keeping a running job, its Cancel action and result state alive when navigating. Genuine generation and GPU behavior are **not** newly qualified here.

**Photo Studio** is the bounded existing non-destructive-in-memory adjustment stage and explicit no-overwrite Pictures PNG export. No video timeline below its canvas. Switching Photo→Video→Photo preserves the selected photo and per-photo adjustment values; generated outputs route to Photo. Pre-existing read-only oversized/unsupported image fallback remains.

**Video Studio** has only genuinely implemented local clip playback, seek in 0.1s increments, and non-exportable in/out markers. Clip assembly, transitions, frame extraction, video-to-image, mature-scene generation and exported trim/timeline are **NOT IMPLEMENTED**, and should be labeled rather than represented as working buttons. Switching between photo and video does not display the wrong media type or reset the photo adjustment state. **No audio workspace or sound production**.

**Models and Settings** retain the existing functionality. PR #28's app-lifetime download provider (with its PR #26 and PR #18 bases, `100f2ce`) is merged into this branch: the compact download status and Cancel sit under the navigation on every destination except Models, which shows its own. Below 1200 px the rail is icon-only, so the card floats beside it above the page.

## CPU-only Windows acceptance, record exact tested head

1. Fetch live PR heads and use an isolated Windows worktree. Preserve PR #31's qualified head and PR #28's app-lifetime progress work. Run:
```powershell
npm ci
npm run test:content
npm run build
cargo fmt --all --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml --locked
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
```
No unnecessary GPU generation, model download or cloud/network operation.
2. In the real app at **1280×720, 1440×900, 1920×1080** (100% scaling), verify each of seven sidebar buttons works and its selected/keyboard focus state is accurate. Confirm icons remain visible below 1200px and the navigation scrolls at short height. At 125% Windows display scaling only if owner permits changing OS scale; otherwise document as untested.
3. Open Project → Media Library; create/import 1 photo and 1 video. Search/filter by filename and category, open each in correct editor. Ensure no implicit thumbnails/reveal, no project media persistence falsely claimed; restart proves session imports do not persist, and saved generated history is separately handled.
4. In Photo: set brightness/contrast/rotate and switch to Video, Models, Library, then back. **Verify same photo and values are retained**; adjust and export after explicitly accepting Pictures/OneDrive sync warning. Refusal writes nothing; original file never overwritten. Open a second photo and return to first, verify separate edit values.
5. In Video: select clip; play, seek, set in/out; switch back to Photo then Video and ensure selected clip doesn't switch to a photo. Timeline must display preview-only markers, no fake Export Video button. If the clip codec is unsupported, show the honest error.
6. In Create: enter prompt/source and commence a **synthetic held IPC job only**; switch to Media Library / Photo / Models, return to Create and verify state, progress and Cancel survive. Trigger synthetic generated result and verify it routes to Photo Studio; then try verified local Generation History → Photo. Clearly separate simulated and genuine native generation acceptance. Do not run a new GPU inference job without owner authorization.
7. Model installation: after integrating #28, start one synthetic held install and navigate through all seven destinations. The global status/Cancel and adult-name concealment must survive. The new sidebar must never obscure the global status. Test real cancellation separately only if necessary and owner-authorized.
8. Check accessibility and privacy: tab order, focus ring, narrator labels, text truncation, long filenames, no lingering previews across tab changes, no unauthorized external network calls, and no audio production surfaces.
9. Fix any TypeScript/React or native regressions, rerun exact-head gates, publish real Windows screenshots of the seven-navigation shell, photo and video stage, session library and resize states with pass/fail receipts. Require distinct independent human review and real Qortara gate before integration.

## Merge and closure

This branch is stacked on #31 → #30 → #25 → #20 → #18 → #16 → #13 → #1. Integrate carefully with #26/#28 and their real Windows progress tests. Do not recreate their download progress state in this branch, duplicate UI links to nonworking modules, or bypass high-risk authorization in [issue #2](https://github.com/Knapp-Kevin/optic-rift/issues/2).

[Issue #29](https://github.com/Knapp-Kevin/optic-rift/issues/29) remains open for the full professional photo/video/cross-media UX. The navigation fix is a small but useful functional increment, **not** the completed professional studio.

**Done:** exact-head Windows qualified, independently reviewed, Qortara-authorized, merged into `main`, post-merge Windows-verified. Close only fully satisfied implementation tasks; keep #29 open.
