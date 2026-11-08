# Optic Rift: whole-product creative UX qualification (November 8, 2026)

**Status:** development branch only, not Windows-qualified/merged/approved. Parent [PR #33](https://github.com/Knapp-Kevin/optic-rift/pull/33) is frozen at `17368391aaac52624041c191559f9c09ce129a91`; this branch must not rewrite PRs #1–#34.

## Why this exists

The owner found that even the expanded 7-destination app lacked basic user journeys: actual **Create Videos** planning, video quality improvement/restoration selection, and an intelligible map of what the photo/video creative studio does and does **not** do. One-off controls in Studio do not satisfy the whole product vision.

The approved destination remains a first-class professional Photo Studio, Video Studio and cross-media project with optional locally qualified generation and visual enhancement; **no audio creation/mixing**.

## Exact features now present in code

- Ten actual navigation destinations grouped Your Work, Generate, Edit & Enhance and Tools: Projects, Media Library, Workflows, Create Images, Create Videos, Photo Studio, Video Studio, Enhance Video, Models, Settings.
- **Create Videos** is an in-memory **shot planner** for text-to-video, image-to-video and video-to-video, with source selected only from opened session media. User can draft prompt, select aspect/length/camera motion and general/mature/adult-focused *planning intent*, add/edit/reorder/remove up to 24 shots, see total duration and source references, and navigate to the relevant video model discovery capability. **No Generate/Render output exists**, because there is no verified video executor.
- **Enhance Video** is an in-memory treatment planner for video source, denoise, upscale, stabilize, frame interpolation, deinterlace, color repair, sharpening and scratch/dust repair, with risk warnings, output target preferences and a reviewed plan. **No processing/export runs**; 4K is not promised.
- **Workflows** is a searchable/filterable capability/acceptance directory of **25** real product journeys grouped Photo, Video, Production, Enhancement, Privacy & projects. Each shows current implementation state and E2E acceptance before a destination is considered finished. Missing features have no fake click action. No unsolicited media thumbnails.
- Shot plans and enhancement preferences **survive navigation during the session** (mounted/hidden), but are explicitly **not saved into projects** and disappear on restart. Neither creates provenance, downloads, cloud requests, model jobs or user files.
- Video-focused model discovery can preselect `text-to-video`, `image-to-video` or `video-to-video`. Catalog results still do **not** imply supported execution or licensing permission.
- Model adult discovery/preview choice remains separate from a shot's content intent, which is **not** age verification or rated-content authorization.

## Required Windows execution

1. Fetch current branch head and parent PR #33. Use isolated Windows worktree/QA profile and non-sensitive local photo/video fixtures. Do **not** launch GPU generation, download model weights, or use owner's private assets. Run:
```powershell
npm ci
npm run test:content
npm run build
cargo fmt --all --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml --locked
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
```
If a test fails, repair and re-run on a recorded exact final commit. Existing CI success on ancestors is not evidence for this branch.
2. Navigation at 1280×720, 1440×900, 1920×1080; short 720px rail, keyboard focus/tab, accessible labels and scrolling. Ensure ten destinations are usable and none visually overlaps app-lifetime download progress/Cancel from PR #28. At 125% Windows scaling only if owner explicitly authorizes the settings change.
3. **Video shot planner:** create text-to-video shot, image-to-video shot with explicit photo reference, and video-to-video shot with clip reference. Set prompt/camera/ratio/duration/intensity. Test error handling for no source, deleted reference, blank/oversize prompt, 24-shot cap, order changes and multiple shot editing. Verify navigating Models/Video/Library and back retains the session board but restarting drops it with clear copy. Verify **no GPU job, network, media file or native `video_generate` IPC call**.
4. **Enhancement:** import local video, choose multiple operations/target resolution/quality goal, review risks. Test missing source/invalid operation/notes and clearing state. Confirm it is plan-only: no denoise/upscale video created, no source modified and no accidental model/FFmpeg invocation.
5. **Model search:** the "Explore video models" action preselects the corresponding catalog capability, and displays unqualified results as found/not runnable, not an active Generate button. Adult-focused shot intent does **not** turn on adult model discovery/reveal. Changing discovery does not delete owned media or automatically render explicit previews.
6. **Workflows:** verify all 25 cards, filters, search and status/acceptance labels. A future-only story has **no fake active execution action**. Click available related routes; no blank pages. Verify content is readable at a professional grade 10 reading level.
7. Regression: original create-image→photo-edit→explicit export→reopen, photo/video selection retention, frame capture PR #33, native model download status/Cancel, imported media privacy, project metadata reopen and unsupported video rendering. Can use existing approved test media/fixtures without another GPU run; differentiate synthetic IPC success from real native generation evidence.
8. Accessibility/privacy/security: focus visibility, no hidden-workspace focus traps, long filenames and prompts, 125% scale if authorized, no extra cloud/OneDrive exports, no audio creative tools. Confirm hidden planners do not leak private prompt text across other destinations.
9. Attach clean screenshots, final exact SHA, commands, individual test pass/fail results, negative cases, and any reproduction to this PR. Never claim native video generation/quality improvement from a successful planning UI.

## Gate and owner sign-off

Design review and user-journey mapping [Issue #29](https://github.com/Knapp-Kevin/optic-rift/issues/29) remains **open** until proper screen wireframes, novice/advanced usability tests and accepted full Photo/Video/Production requirements. The development train still depends on #33 → #32 → #31 → #30 → #25 → #20 → #18 → #16 → #13 → #1 plus #26/#28 and the independent human reviewer/Qortara gate #2. Do **not** bypass review or publish an installer just to claim progress.

**Done = real Windows exact-head quality + independent source/security review + authentic Qortara approval + authorized merge to main + post-merge verification.** Only then close accurately scoped issues; do not close #29 for a planning-only increment.
