# PR #35 whole-product creative UI: Windows evidence

Head under test: `49b589b39dd28e65a5aae4515b427c0e0d948314` (`feature/video-shot-composer-20261108-no-ci`). It is the starting head `8149bcc` plus two fix commits:

- `fbc2147`: plain-language journeys, stale enhancement notice, removed-reference copy.
- `49b589b`: search examples.

PR #35's frame-capture files are byte-identical to PR #33's qualified head `9e84a60`, and the branch merges into it cleanly. PR #33 and earlier branches were not changed.

Environment: Windows 11 Pro 10.0.26300, WebView2 157, debug build in the isolated QA profile (`studio.mythologiq.opticrift.qa`), driven over a loopback-only DevTools port at 100% scaling. All media is synthetic. No GPU inference, model-weight download, paid provider or cloud call was made. The harness blocks `catalog_search`, `catalog_inspect`, `model_install`, `models_load` and `generation_generate`, and none of them was ever attempted.

## Gates

| Gate | `8149bcc` (start) | `49b589b` (final) |
|---|---|---|
| `npm ci` | exit 0 | exit 0 |
| `npm run test:content` | 79/79 | **83/83** |
| `npm run build` | exit 0 | exit 0 |
| `cargo fmt --check` / `cargo check --locked` | exit 0 / exit 0 | exit 0 / exit 0 |
| `cargo test --locked` | 99 passed, 4 ignored; 1 passed | 99 passed, 4 ignored; 1 passed |
| `cargo clippy --all-targets --locked -D warnings` | exit 0 | exit 0 |

## Defects found and fixed

| # | Defect (seen in the real app) | Fix |
|---|---|---|
| 1 | Workflow journey text read at about **grade 12** (Flesch-Kincaid estimate; median 12.0, 12 of 25 cards above 12, max 16.7), with arrow shorthand in every acceptance line. The runbook asks for grade 10. | All 25 outcome, current-state and acceptance texts were rewritten in short sentences: **grade 5.4**, max 8.6. Titles, states and routes are unchanged. A test checks each card at grade 10 or below. |
| 2 | After a review in Enhance Video, changing any option cleared the review but left **"Enhancement plan ready for review"** on screen. | Every edit now clears the review and that notice together. |
| 3 | A shot whose reference photo was removed read **"Reference: Reference unavailable; reselect before execution"**. | It now reads "Reference no longer open. Edit this shot to choose another." |
| 4 | The search box suggests "Upscaling, animation, retouch, film", but **"Upscaling" found nothing**, a pre-existing gap. The rewrite in #1 also dropped the only "animation" and "retouch" matches. | The copy contains each term again, and a test runs every example through the page's own search rule. |

## Results at `49b589b` (`logs/final-run-49b589b.txt`)

- **Navigation (`navigation/`):** checked at 1280×720, 1440×900, 1920×1080 and 1100×700 (icon rail). All 40 page/size checks pass:
  - Each destination shows exactly one `aria-current`, the right title, and one visible page with content.
  - There is no horizontal scroll, and the rail fits at 700 px tall.
  - Tab runs Projects → Settings in order with a 2 px focus ring, and Enter opens the destination.
- **Create Videos, shot creation (`planning/plan-*`):**
  - All three modes work: text-to-video, photo-to-video with a photo reference, and video-to-video with a clip reference. Photo lists show only photos, and clip lists show only clips.
  - Every length, frame shape, camera movement and content intent can be set and is shown on the card.
- **Create Videos, errors and limits:**
  - These are refused with a clear message: a missing source, a blank prompt, a control character, and 1,201 characters (1,200 is accepted and wraps without overflow).
  - A 25th shot is refused at 24/24. Editing still works at the limit.
- **Create Videos, editing and order:**
  - Edit and Cancel edit work.
  - Move up/down reorders correctly, with the ends disabled. Remove works.
  - A removed photo reference is flagged on the card, and re-saving that shot asks for a new photo.
- **Enhance Video (`planning/enhance-*`):**
  - The real imported clip appears as a source.
  - Reviewing with no source, no operation, a control character, or 501-character notes is refused with a clear message.
  - With all eight operations, a 4K target and maximum priority, the review lists every risk and says no processing was started.
  - Changing an option clears the review and its notice. Removing the source clip clears the plan and says why.
  - There are no Process, Render or Export controls. The source file hash is unchanged.
- **Workflows (`planning/workflows-all.png`):**
  - All 25 journeys are listed. Area counts are 5/8/3/5/4 and state counts are 2/6/9/8. Search and combined filters work, and an empty result explains itself.
  - All 17 route buttons open a page with content.
  - The 8 not-implemented journeys have no action button, only "No working destination yet".
- **Session only:**
  - Shot lists, drafts and enhancement reviews survive moving between pages.
  - After a restart they are gone, and the empty state says plans are not saved after restart.
  - The hidden planner text never appears on the other nine destinations, and the hidden planners have no focusable controls.
- **Adult intent:**
  - An adult-focused shot leaves Models' adult discovery off.
  - "Explore Video to video models" preselects `video-to-video` without searching.
  - The Media Library shows no thumbnails.
- **Regressions (`regression/`):**
  - Video → Capture frame → Photo Studio still matches PR #33: frames 23 and 17, the rotated clip, lineage, and edits persisting.
  - A simulated Create Images job's result opens in Photo Studio at 1200×800.
  - Declined exports wrote nothing: Pictures stayed at 30 → 30 files.
- **Downloads (`downloads/`):** PR #28's synthetic held install was checked at 1100, 1280, 1440 and 1920 wide.
  - Status and Cancel are in view and topmost on all nine non-Models destinations, including Workflows, Create Videos and Enhance Video: **56/56 topmost**.
  - Concealment, exact-ID cancel and reload adoption still pass.
- **Network:** no non-IPC WebView request in any run.

## PR #34 favicon (`favicon-pr34/`)

- **Head:** `3a99e6c`. `npm ci`, content tests (6/6) and the build pass.
- **Artwork:** `public/branding/optic-rift-approved-256.webp` is the owner-approved lens artwork, committed 2026-11-05 as "user-approved". Its SHA is `dae5767d…`, matching `assets/branding/README.md`. It is the same file the sidebar logo uses, and it stays recognisable at 16, 32 and 48 px.
- **Real WebView2:** in the PR #34 build, the document's only icon link resolves to `http://tauri.localhost/branding/optic-rift-approved-256.webp`. It returns HTTP 200 as `image/webp`, decodes to 256×256, and its bytes hash to the approved SHA.
- **Not covered:** a Tauri window has no browser tab, so this favicon is not seen in normal use. The Windows taskbar, Start menu and installer icons come from `src-tauri/icons/icon.ico`. Those remain separate, unqualified work.

## Not covered

- 125% display scaling: not authorized.
- Live Narrator.
- Real video generation or enhancement: neither exists. These pages are planning only, and passing their controls does not mean video can be made or improved.
- A real (non-synthetic) model download on this branch.
