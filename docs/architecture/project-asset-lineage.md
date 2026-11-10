## Experimental managed-PNG photo slice (branch pending qualification, 2026-11-09)

This section describes an **implementation candidate**, not a final owner-accepted general asset policy. The first photo-project save path is an **explicit opt-in managed copy**, not a default for imported media. It is separate from user exports to Pictures, external linked media, and generated-output caches.

- The existing project record v1 remains an immutable name/ID JSON in `app_data/projects/<uuid>.json`.
- A specifically saved original PNG is staged under `app_data/project-media/<project-uuid>/.<asset-uuid>.partial/`, validated with a bounded full PNG decode, SHA-256 hashed by Rust, and atomically published as `project-media/<project-uuid>/<asset-uuid>/original.png`. Image source bytes are never overwritten by edit operations.
- Versioned photo edit records live as immutable `r000001.json`, `r000002.json`, etc. A native `SAVE_LOCK` and caller-supplied `expected_revision` avoid silent stale writes; older revisions remain available on disk but the initial interface opens the most recent. Revision count is capped at 1000.
- The saved record v1 holds project/asset UUIDs, original PNG name, **native-calculated** SHA-256, dimensions, record version, sequence and bounded edit values: brightness, contrast, saturation, quarter-turn rotation, straighten angle, crop, flips and longest-edge resize. Viewport zoom/pan and original-comparison toggle are explicitly excluded.
- The native open command checks project identity, canonical asset UUID, regular non-reparse directories/files, bounds, original SHA-256, and PNG decode/dimensions **before** returning source bytes for a new WebView blob URL. The WebView repeats its normal image preflight. Corrupt documents are visible as problems; partial staging directories are never advertised as committed media.
- Current scope: **PNG only**, explicit save in the selected project, app-data-local storage, single-device; no JPEG/WebP video persistence, portable project bundle, relink workflow, autosave, source deletion, cross-device transfer, vault or cloud sync guarantees.
- **Interrupted-save inventory (draft mitigation, not recovery acceptance):** potential initial-save `.uuid.partial` entries and edit-revision `.revision.uuid.partial` entries do not consume committed photo/revision limits. Native `project_photo_list` returns `possible_incomplete_saves`, a count of **names matching** those two patterns within readable project media; it does not establish provenance, count unknown filenames, guarantee a complete disk inventory, or authorize cleanup. The Media Library displays an explicit non-destructive warning when the count is nonzero. Enumeration permits at most 128 unrecognized entries in each relevant directory; beyond that, it fails with an explicit recovery-required error. The implementation does **not** delete staged bytes, assume their provenance, or promise disk-space reclamation. [Recovery issue #57](https://github.com/Knapp-Kevin/optic-rift/issues/57) owns the pending retention and safe-cleanup decision; actual Windows interruption qualification remains pending.
- Security and usability acceptance remains open until full Rust + IPC tests, actual Windows save/kill/restart/reopen, undo after reopening, disk-full/staging fault injection, native file-permission/symlink tests, and owner-approved storage choice are complete. This experimental slice does not close [Wayfinder decision #5](https://github.com/Knapp-Kevin/optic-rift/issues/5), [photo document #40](https://github.com/Knapp-Kevin/optic-rift/issues/40), or [Photo Studio delivery #42](https://github.com/Knapp-Kevin/optic-rift/issues/42).

---

# Project, asset lineage and non-destructive revisions

Status: Proposed. Domain sketch and candidate storage paths; not a published schema.
- Claim owner: asset identity, project persistence, reference/revision semantics and recovery.
- Wayfinder decision: [Durable project and asset-lineage contract](https://github.com/Knapp-Kevin/optic-rift/issues/5).
- Inheritance: draft desktop ADR-0001 suggests SQLite plus on-disk assets; this proposal does not turn it into an accepted commitment.

## Domain vocabulary

| Term | Proposed meaning | Must not be confused with |
| --- | --- | --- |
| Project | Durable named creative workspace with an identity, media inventory, sequence revisions and preferences | An export folder or a transient tab |
| Source asset | Imported or generated original bytes with a stable identity and verified location/content hash | A mutable thumbnail |
| Derived asset | A new image/video created by an operation and linked to its source inputs | An in-place overwrite |
| Reference edge | The purpose for using an asset in another operation (subject, style, framing, structure, start/end frame) | Claim of perfect identity consistency |
| Operation recipe | Versioned intent and parameters, including crop coordinates, source range, requested capability and model settings | Evidence that an executor ran |
| Execution attempt | Environment/worker/revision, time, logs summary, success/failure, exact output hashes | Accepted review or license grant |
| Timeline/selection | A reversible arrangement or view of assets and non-destructive edit recipes | The original video |
| Export | Delivered external file with codec/preset, provenance and result verification | The authoritative editable project |

## Relationship sketch

~~~mermaid
flowchart LR
 A[Original photograph] -->|reference: composition| C[Concept art variant]
 C -->|image-to-video| V1[Shot 1]
 V1 -->|extract exact decoded frame @ time PTS| F[Still frame]
 F -->|crop and upscale| H[Enhanced still]
 H -->|image-to-video| V2[Shot 2]
 V1 --> T[Timeline revision]
 V2 --> T
 T --> E[Export MP4]
 A -->|unmodified source| SRC[(Original bytes)]
~~~

Each edge records the **role** of the input. Multiple reference assets may serve different roles. A derivation edge never points to a mutable editor preview. Cyclic user histories are permissible only as separate new asset versions, so provenance remains a directed acyclic history even when a user goes image -> video -> image.

## Minimum project contract to decide

- Stable opaque identifiers for Project, Asset, Operation, Attempt, TimelineRevision and Export.
- Original file locator and an observed content digest; original media always read-only by default. A missing or relocated original produces a recoverable link state rather than deletion of its history.
- Source video frame identity includes source asset revision, stream identifier, presentation timestamp in **rational time base**, decode/selection policy and transform rectangle. Display timecode and frame number are not necessarily a unique identity for variable-frame-rate video.
- Generated assets carry model repo ID + immutable revision + adapter version when known, actual output hash, settings/seed and rights status. Unknown model metadata is labeled unknown.
- Output files commit **only after** staging integrity, filesystem sync/atomicity where supported, and validation. Incomplete outputs never appear as completed assets.
- User adjustments make new reversible project recipes. Original bytes never mutate by dragging a slider. An export records exactly which project/timeline revision produced it.
- Cache paths for proxies/thumbnails are derived/rebuildable, size-bounded and not the sole copy of valuable user assets.

## Storage alternatives

| Design | Advantage | Risk | Recommended disposition |
| --- | --- | --- | --- |
| Single versioned project JSON + assets | Portable and readable | Concurrent autosave, large graph, crash consistency | **Viable prototype**, if atomic writes and bounded size are proven |
| SQLite project index + versioned media/recipe files | Transactional graph, queries, migrations and recovery | More migration and packaging complexity | **Preferred hypothesis** |
| Content-addressed store exclusively | Deduplication and traceable bytes | Relocation, GC, accidental destruction of user-owned material | Consider for managed derived output only, **not** external originals |
| In-memory editor state only | Fast initial UI | Loses work on restart; violates key journeys | Not acceptable past shell mockup |

## User-visible behaviors

1. A project opens to its last usable state, with originals, derived versions, references and selections recoverable.
2. An extracted video frame becomes a standard image asset, traceable to exact source video and time, and can be cropped/used as a fresh reference.
3. A user can compare original, enhanced, and generated versions without losing any.
4. Source files may be **linked** from existing directories or explicitly **copied into a portable project**; external media are not silently moved.
5. A missing source prompts relink; an unavailable model affects re-generation, not opening an existing finished image or export.

## Failure conditions and acceptance probes

- Simulate abrupt process kill during project save, media export and model output commit; reopen with coherent previous saved state and no half-accepted asset.
- Simulate disk full, permission error, renamed external media, corrupt DB, duplicate filenames and unsupported metadata.
- Verify output references and source hashes before trust; refuse opening untrusted paths outside approved project root through a forged command.
- Compare two branches of the same original; changing one recipe must not alter the other's output or the original.

**Unresolved owner choices:** linked vs managed media default, portable bundle format, source media deletion/relocation policy, history retention/pruning, migration ownership and whether all output variants are stored indefinitely. These belong in decision ticket #5, not in an invented final schema.

