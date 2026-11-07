# PR #32 navigation qualification: Windows evidence

Head under test: `39fd7ca03bc43a091965ef7056ec95c0ef62c84a` (`feature/creative-workspace-navigation-20261106-no-ci`), which includes PR #28 at `100f2ce`.

Environment: Windows 11 Pro 10.0.26300, WebView2 157, debug build of the app in an isolated QA profile (`studio.mythologiq.opticrift.qa`), driven over a loopback-only DevTools port. Screenshots are `Page.captureScreenshot` captures of the app's WebView at 100% scaling.

## Gates at `39fd7ca`

| Gate | Result |
|---|---|
| `npm ci` | exit 0 |
| `npm run test:content` | 66 / 66 pass |
| `npm run build` | exit 0 (tsc + vite) |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo check --locked` | exit 0 |
| `cargo test --locked` | 99 passed, 4 ignored; doc/integration 1 passed |
| `cargo clippy --all-targets --locked -- -D warnings` | exit 0 |

## Folders

- `navigation/`: each of the seven destinations at 1280×720, 1440×900, 1920×1080 and 1100×700 (icon-only rail), captured on the final build.
- `downloads/`: one synthetic held model install (IPC fixtures only; nothing searched or downloaded) visited across every destination at 1100×700, 1280×720 and 1920×1080, with adult discovery on, then off (identity concealed), Cancel, and adoption after a WebView reload. Captured on the merge build `57dfad3`; `39fd7ca` changes only the Create result label and the runbook.
- `journeys/`: Media Library video filter, photo B edits retained, Video Studio marks in tenths, Create held job surviving route changes, a simulated Create result opened in Photo Studio, and a stored QA-profile output opened from history into Photo Studio.

Simulated IPC is limited to `models_library`, `gpu_check`, `generation_generate`, `generation_cancel`, `generation_output_image` for the fixture job, and the model catalog/install commands in the download run. History listing, stored-output reading and photo export ran natively. No GPU inference, model download or non-loopback WebView request occurred.
