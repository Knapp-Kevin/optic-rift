# Studio interaction model and user-journey routes

- Status: **PROPOSED**; interaction design, not a functioning UI screenshot or finished product.
- Claim owner: creative workspace navigation, selection actions, progressive disclosure and user outcomes.
- Wayfinder: [Cross-media experience decision](https://github.com/Knapp-Kevin/optic-rift/issues/8).
- Source: owner's image, video, restoration, frame extraction, photo-to-concept, repeated reference and cross-media journeys.

## Default entry

Launch from Windows Start menu or desktop -> open the Studio with **Recent projects**, **New project**, **Import media**, **Create image**, **Create video** and an unobtrusive **Models** entry. The first screen asks what the user intends to make, not which model they want to configure.

Only expose an action as usable when the underlying operation actually works or when it clearly explains its unavailable prerequisites. An offline local edit never requires model download, Hugging Face sign-in or a network connection.

## Main surfaces (one project, not separate apps)

| Surface | Primary user intention | Owned interaction |
| --- | --- | --- |
| Home / project chooser | Begin or resume creative work | New/open/recent/restore and explicit portability choice |
| Studio workspace | Keep all material in one place | Media bin, player/canvas, timeline, selection, undo, export |
| Asset details / history | Understand source, versions and opportunities | Reference edges, related variants, source frame time, permissions |
| Creative action panel | Transform selected material | Context-sensitive actions and guided presets; expandable advanced controls |
| Job tray | Know what's running | Progress, estimated resource requirements when known, pause/cancel and detailed failures |
| Model library | Find optional tools by capability | Broad discovery, explicit rights/status/content flags and consent |
| Settings / diagnostics | Manage device, downloads and privacy | Tested hardware paths, storage/caches, logs scoped to app and model catalog preferences |

## Context actions instead of a workflow graph

~~~mermaid
flowchart TD
 P[Selected project asset] --> Q{Kind}
 Q -->|Image| I[Crop / correct / use as reference / animate / upscale / edit]
 Q -->|Video| V[Play / seek / extract frame / enhance / reframe / trim]
 Q -->|Timeline| T[Play whole sequence / rearrange / export]
 I --> R[Chosen operation and optional qualified model]
 V --> R
 T --> R
 R --> N[Preview new artifact + preserve source]
 N --> P
~~~

Actions are capability-driven and explain missing adapters. The underlying derivation graph is useful for history, but **not the main creation interface**. Users can move freely between an image, a clip, an extracted still and a new video without exporting/reimporting manually.

## End-to-end acceptance journeys

### Journey S1: no-AI editor

Open app -> create project -> import video -> play and select trim region -> preview edit -> export usable MP4 -> close/reopen with original and work state intact. An optional model never appears as a blocker.

### Journey S2: concept art to two-shot video

Generate or import a photograph -> mark as composition/structural reference -> create concept variations -> pick one -> make a short video -> navigate the generated shot frame by frame -> extract still with source PTS -> crop/enhance it -> use that still as another shot reference -> arrange shots and export. The full chain is visible in asset history. When no generation model is installed, the workspace preserves intent and offers an explicit setup path, not a fake generated result.

### Journey S3: restoration and high-quality still

Import old footage -> inspect/optionally make proxy -> select denoise/deinterlace/stabilize if supported -> render enhanced video revision -> compare original and enhanced -> inspect frames/contact sheet -> crop a selected exact source frame -> optionally upscale -> export photo with a clear notice that AI may synthesize missing detail.

### Journey S4: privacy-conscious adult model choice

Open model browser (explicit listings concealed by default) -> independently enable 18+ discovery -> select model marked with evidence and distinct licensing state -> explicitly reveal preview if desired -> confirm download/rights when supported. Existing owned media remains editable even with adult model discovery off.

### Journey S5: unsupported model

Search model -> inspect capability, source, license and device status -> user sees **Found, not runnable here** rather than a broken Generate button. Offer a verified compatible alternative only if one exists.

## Usability and accessibility

- Provide keyboard access to player, frame stepping, timeline selection, undo/redo, extraction and export; screen-reader labels for actions and dialogs; visible focus indicators.
- Use clear copy at roughly a tenth-grade reading level; advanced prompts/seed/guidance/steps are optional disclosures, not first-run tasks.
- Present input/source preservation, content reveal, external downloads, and model license acceptance as separate deliberate choices.
- Keep playback responsiveness and selection stable while worker jobs run; do not discard a user's selection on an unrelated job completion.
- If a step cannot execute, state precisely why (missing model, unsupported codec, license unknown, disk full, project source missing), with a safe route forward.

## Decisions not yet accepted

The default screen split between timeline and canvas, modal/panel treatment of creative actions, multi-reference selection UX, project relinking, and whether generation settings should share one task inspector are HITL design decisions. Validate with a clickable prototype and actual novice users before freezing layout.

