import type { View } from "./creativeNavigation";

export type JourneyArea = "Photo" | "Video" | "Production" | "Enhancement" | "Privacy & projects";
export type JourneyState = "working-slice" | "partial" | "planning-only" | "not-implemented";
export type CreativeJourney = {
  id: string;
  area: JourneyArea;
  title: string;
  outcome: string;
  current: string;
  state: JourneyState;
  route: View | null;
  acceptance: string;
};

/** The capabilities on these branches, not a release feature checklist.
 * Real acceptance requires a specific Windows end-to-end receipt on the
 * combined exact head. Do not infer runtime execution from metadata discovery.
 */
export const creativeJourneys: readonly CreativeJourney[] = [
  { id: "P01", area: "Photo", title: "Import, correct and export a photo", outcome: "Adjust a photo and save a new PNG without altering its source.",
    current: "Basic brightness, contrast and rotation export exist; no layers or ICC workflow.", state: "working-slice", route: "photo",
    acceptance: "Real original → edits → export → restart → reopen, pixel/hashes verified." },
  { id: "P02", area: "Photo", title: "Full non-destructive photo editor", outcome: "Crop, perspective, curves, layers, masks, heal, clone and versions.",
    current: "Only basic adjustments and raster PNG export.", state: "not-implemented", route: null,
    acceptance: "Editable layer/mask project survives restart; full-resolution color-managed export." },
  { id: "P03", area: "Photo", title: "Generate concept art and variants", outcome: "Create and refine images without losing original results.",
    current: "Qualified local text-to-image and image-to-image paths on development branches.", state: "working-slice", route: "create",
    acceptance: "Real local generation → select variant → edit → export → reopen." },
  { id: "P04", area: "Photo", title: "Repair, restore, inpaint and upscale photos", outcome: "Improve damaged or low-resolution photos with transparent AI provenance.",
    current: "Model discovery exists, but no validated retouch/inpaint/upscale execution path.", state: "not-implemented", route: null,
    acceptance: "Restore real samples with editable masks, quality comparison and source preservation." },
  { id: "P05", area: "Photo", title: "Batch photo adjustments", outcome: "Apply a validated edit recipe to many photos safely.",
    current: "No bounded job queue for multi-file photo edits.", state: "not-implemented", route: null,
    acceptance: "Batch with cancel, per-item failures, deterministic outputs and no overwritten originals." },
  { id: "V01", area: "Video", title: "Play and inspect local footage", outcome: "Import a clip, play, seek and mark in/out points.",
    current: "Playback and non-exportable in/out preview markers are present.", state: "partial", route: "video",
    acceptance: "Supported codecs, accurate seeking, no-model operation and recoverable media errors." },
  { id: "V02", area: "Video", title: "Capture a video frame for photo editing", outcome: "Take a visible frame into Photo Studio and optionally export it.",
    current: "Explicit session-only PNG frame capture is in draft development.", state: "partial", route: "video",
    acceptance: "Real decoded frame → Photo editing → explicit export, original video unchanged." },
  { id: "V03", area: "Video", title: "Edit and export video on a timeline", outcome: "Trim, arrange, overlay, grade, preview and render clips.",
    current: "No qualified multi-clip editing, native timeline renderer or video export.", state: "not-implemented", route: null,
    acceptance: "Multi-clip project → deterministic render → restart/reopen and quality verification." },
  { id: "V04", area: "Video", title: "Generate video from a prompt", outcome: "Compose and generate a locally qualified video shot.",
    current: "Shot planning UI; no verified video-generation adapter or native job.", state: "planning-only", route: "create-video",
    acceptance: "Qualified model/rights/device check → render/cancel → retained output and provenance." },
  { id: "V05", area: "Video", title: "Animate a still image into video", outcome: "Use a project photo as a reference frame for motion.",
    current: "Choose a local photo in the session shot planner; generation unavailable.", state: "planning-only", route: "create-video",
    acceptance: "Image source/hash → qualified animation → exported clip with traceable lineage." },
  { id: "V06", area: "Video", title: "Transform an existing video", outcome: "Change visual style or motion while preserving source footage.",
    current: "Choose an opened clip and plan the transformation only.", state: "planning-only", route: "create-video",
    acceptance: "Video reference → qualified transform → verified frames and cancel/failure recovery." },
  { id: "V07", area: "Video", title: "Place photos in a video", outcome: "Insert stills, overlays and transitions into a movie.",
    current: "Media Library can open photos/videos, but no multi-track composition timeline exists.", state: "not-implemented", route: null,
    acceptance: "Photo clips/overlays → video sequence → validated rendered file." },
  { id: "V08", area: "Video", title: "Reframe for social formats", outcome: "Crop/reframe a video for portrait, square or landscape delivery.",
    current: "Shot aspect planning exists, but no native video reframe/export.", state: "not-implemented", route: null,
    acceptance: "Source video → safe crop/keyframes → aspect-correct exported clip." },
  { id: "R01", area: "Enhancement", title: "Denoise and sharpen footage", outcome: "Improve low-light or noisy video without losing genuine details.",
    current: "Source and treatment planning only; no processing adapter.", state: "planning-only", route: "enhance-video",
    acceptance: "Before/after review, measurable quality, stable native processing and reversible source." },
  { id: "R02", area: "Enhancement", title: "Upscale video to HD or 4K", outcome: "Increase delivery resolution with honest quality expectations.",
    current: "Resolution targets can be planned; no actual upscale renderer.", state: "planning-only", route: "enhance-video",
    acceptance: "Qualified upscale with measured detail/artifacts, VRAM bounds and export integrity." },
  { id: "R03", area: "Enhancement", title: "Stabilize shaky footage", outcome: "Smooth camera movement and inspect cropping.",
    current: "Treatment planning only.", state: "planning-only", route: "enhance-video",
    acceptance: "Stabilized output with framing preview, before/after check and reproducible export." },
  { id: "R04", area: "Enhancement", title: "Restore old and damaged video", outcome: "Deinterlace, correct color and repair scratches without damaging original.",
    current: "Restoration recipe planning only.", state: "planning-only", route: "enhance-video",
    acceptance: "Source-field/proxy analysis → validated repair → side-by-side export and provenance." },
  { id: "R05", area: "Enhancement", title: "Improve motion smoothness", outcome: "Interpolate frames where suitable and identify created motion artifacts.",
    current: "Treatment planning only.", state: "planning-only", route: "enhance-video",
    acceptance: "Qualified interpolation, original timing preserved/reconciled and quality receipts." },
  { id: "F01", area: "Production", title: "Storyboard a short film", outcome: "Plan shots, prompts, camera movements, references and durations.",
    current: "Session-only shot list with ordering/editing; not a durable project or executable render queue.", state: "planning-only", route: "create-video",
    acceptance: "Persist storyboard by project, link sources, compare shots and resume after restart." },
  { id: "F02", area: "Production", title: "Keep characters and visuals consistent", outcome: "Reuse reference images, character identities and scene continuity.",
    current: "Single-shot references can be selected, no persistent character/reference graph.", state: "not-implemented", route: null,
    acceptance: "Multiple reference assets + clear scopes + consistent derived media with versioned lineage." },
  { id: "F03", area: "Production", title: "Assemble and finish a short film", outcome: "Arrange generated and imported scenes with titles, grade and export.",
    current: "No scene timeline, real transitions, caption/title layers or master movie renderer.", state: "not-implemented", route: null,
    acceptance: "Storyboard → footage → timeline → full movie export → restart/reopen." },
  { id: "S01", area: "Privacy & projects", title: "Private project media and version history", outcome: "Keep linked original/derived files, versions and provenance across restarts.",
    current: "Project metadata exists; imports/shot plans and frame grabs remain session-only.", state: "partial", route: "library",
    acceptance: "Encrypted or restricted output policy specified, no OneDrive surprise, durable graph and recoverable migration." },
  { id: "S02", area: "Privacy & projects", title: "Content intent and adult model consent", outcome: "Set general/mature/adult intent separately from model discovery and image reveal.",
    current: "Session shot intent and separately opted-in model discovery exist; native adult authorization remains unqualified.", state: "partial", route: "models",
    acceptance: "Native rights/admission enforcement, explicit reveal, age/legal gate where required, safe storage." },
  { id: "S03", area: "Privacy & projects", title: "Offline editing without AI models", outcome: "Import, edit and export without creating a Hugging Face account.",
    current: "Basic photo editing/export works without models; full offline video editing is not built.", state: "partial", route: "photo",
    acceptance: "Air-gapped fresh install: photo and video workflows complete with verified output." },
  { id: "S04", area: "Privacy & projects", title: "Resumable render and download jobs", outcome: "See durable progress, cancel safely and recover after restart.",
    current: "Model-download lifecycle is qualified in development; video render jobs not connected.", state: "partial", route: "models",
    acceptance: "Exact job identity, restart reconciliation, partial-file integrity and negative-case coverage." },
];

export const journeyStateText: Record<JourneyState, string> = {
  "working-slice": "Working slice on development branches",
  partial: "Partial tools",
  "planning-only": "Planning only",
  "not-implemented": "Not implemented",
};

export const journeyAreas: readonly JourneyArea[] = [
  "Photo", "Video", "Production", "Enhancement", "Privacy & projects",
];
