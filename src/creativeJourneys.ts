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
  { id: "P01", area: "Photo", title: "Import, correct and export a photo", outcome: "Fix a photo and save a new PNG. Your original stays the same.",
    current: "Brightness, contrast, rotation and crop are in the editor branch. Crop still needs Windows tests. Layers are not ready.", state: "working-slice", route: "photo",
    acceptance: "Edit a real photo, save it, restart, and reopen it. The saved pixels and file hashes must match." },
  { id: "P02", area: "Photo", title: "Full non-destructive photo editor", outcome: "Crop, fix perspective, use curves, layers and masks, heal, clone and keep versions.",
    current: "Crop controls are added but not yet tested on Windows. Layers, masks and color tools are still missing.", state: "not-implemented", route: null,
    acceptance: "A layered project with masks must reopen after a restart. Full-size export must keep correct color." },
  { id: "P03", area: "Photo", title: "Generate concept art and variants", outcome: "Make new images and refine them. Earlier results are kept.",
    current: "Local text-to-image and image-to-image work on development branches.", state: "working-slice", route: "create",
    acceptance: "Make an image on this PC, pick a version, edit it, save it, and reopen it." },
  { id: "P04", area: "Photo", title: "Repair, restore, inpaint and upscale photos", outcome: "Fix damaged or small photos with AI. The app shows what AI changed.",
    current: "You can find models, but no retouch, repair, fill-in or upscale tool has been tested to run.", state: "not-implemented", route: null,
    acceptance: "Repair real sample photos with masks you can edit. Show before and after. Keep the original." },
  { id: "P05", area: "Photo", title: "Batch photo adjustments", outcome: "Apply one tested edit to many photos at once.",
    current: "There is no safe queue for editing many files yet.", state: "not-implemented", route: null,
    acceptance: "A batch can be cancelled and reports each failed file. The same input gives the same output. Originals are never overwritten." },
  { id: "V01", area: "Video", title: "Play and inspect local footage", outcome: "Open a clip, play it, jump to a moment, and mark start and end points.",
    current: "Playback works. Start and end marks are for preview only and cannot be exported.", state: "partial", route: "video",
    acceptance: "Common video formats play, seeking is accurate, no AI model is needed, and file errors can be recovered from." },
  { id: "V02", area: "Video", title: "Capture a video frame for photo editing", outcome: "Send the frame you see to Photo Studio, then save it if you choose.",
    current: "Frame capture to Photo Studio works on development branches. The captured frame lasts only for this session.", state: "partial", route: "video",
    acceptance: "Capture a real frame, edit it in Photo Studio, and save it only when you choose. The video file does not change." },
  { id: "V03", area: "Video", title: "Edit and export video on a timeline", outcome: "Trim, arrange, layer, color and preview clips, then save a video.",
    current: "There is no tested multi-clip editor, timeline renderer or video export.", state: "not-implemented", route: null,
    acceptance: "A multi-clip project renders the same way every time, reopens after a restart, and passes quality checks." },
  { id: "V04", area: "Video", title: "Generate video from a prompt", outcome: "Describe a shot and make it with a tested model on this PC.",
    current: "You can plan shots. No video model has been tested to run, so nothing is generated.", state: "planning-only", route: "create-video",
    acceptance: "Check the model, its license and this PC. Then render with a working Cancel, keep the output, and record where it came from." },
  { id: "V05", area: "Video", title: "Animate a still image into video", outcome: "Use one of your photos as the starting frame for an animation.",
    current: "You can pick a photo in the shot planner. Making the video is not available yet.", state: "planning-only", route: "create-video",
    acceptance: "Record which photo was used, animate it with a tested model, and save a clip that links back to the photo." },
  { id: "V06", area: "Video", title: "Transform an existing video", outcome: "Change a clip's look or motion. The original clip is kept.",
    current: "You can pick an open clip and plan the change. Nothing is processed.", state: "planning-only", route: "create-video",
    acceptance: "Transform a real clip with a tested model, check the frames, and recover cleanly from cancel or failure." },
  { id: "V07", area: "Video", title: "Place photos in a video", outcome: "Add stills, overlays and transitions to a movie.",
    current: "Media Library opens photos and videos. There is no multi-track timeline to combine them.", state: "not-implemented", route: null,
    acceptance: "Put photos and overlays in a video sequence and save a video file that plays correctly." },
  { id: "V08", area: "Video", title: "Reframe for social formats", outcome: "Crop a video for tall, square or wide screens.",
    current: "You can choose a frame shape when planning a shot. Real video cropping and export do not exist.", state: "not-implemented", route: null,
    acceptance: "Crop a real video safely, with moving crop points, and save a clip with the right shape." },
  { id: "R01", area: "Enhancement", title: "Denoise and sharpen footage", outcome: "Clean up grainy or dark video without losing real detail.",
    current: "You can choose a clip and plan the fix. Nothing is processed.", state: "planning-only", route: "enhance-video",
    acceptance: "Compare before and after, measure the quality, run reliably on this PC, and keep the original." },
  { id: "R02", area: "Enhancement", title: "Upscale video to HD or 4K", outcome: "Make video larger, with honest notes on how much detail it really gains.",
    current: "You can plan a target size. No upscaling runs yet.", state: "planning-only", route: "enhance-video",
    acceptance: "A tested upscaler with measured detail and flaws, safe graphics-memory use, and a valid saved file." },
  { id: "R03", area: "Enhancement", title: "Stabilize shaky footage", outcome: "Smooth out camera shake and see how much the frame is cropped.",
    current: "Planning only. Nothing is processed.", state: "planning-only", route: "enhance-video",
    acceptance: "A steadier clip with a crop preview, a before-and-after check, and the same result each time." },
  { id: "R04", area: "Enhancement", title: "Restore old and damaged video", outcome: "Fix old video, color and scratches without harming the original.",
    current: "You can plan the repair steps. Nothing is processed.", state: "planning-only", route: "enhance-video",
    acceptance: "Check the source first, run a tested repair, compare side by side, and record what changed." },
  { id: "R05", area: "Enhancement", title: "Improve motion smoothness", outcome: "Add in-between frames where it helps, and show where it adds flaws.",
    current: "Planning only. Nothing is processed.", state: "planning-only", route: "enhance-video",
    acceptance: "A tested frame tool that keeps the clip's timing and reports its quality." },
  { id: "F01", area: "Production", title: "Storyboard a short film", outcome: "Plan shots, prompts, camera moves, references and lengths.",
    current: "You can build and reorder a shot list for this session. It is not saved, and it cannot render.", state: "planning-only", route: "create-video",
    acceptance: "Save the storyboard in a project, link its sources, compare shots, and pick up after a restart." },
  { id: "F02", area: "Production", title: "Keep characters and visuals consistent", outcome: "Reuse reference images and characters so scenes match.",
    current: "Each shot can use one reference. There is no saved set of characters or references.", state: "not-implemented", route: null,
    acceptance: "Use several references with clear rules, and keep a version history for everything made from them." },
  { id: "F03", area: "Production", title: "Assemble and finish a short film", outcome: "Put made and imported scenes together, add titles and color, and save the film.",
    current: "There is no scene timeline, real transitions, titles or full-movie export.", state: "not-implemented", route: null,
    acceptance: "Go from storyboard to footage to timeline to a finished movie that reopens after a restart." },
  { id: "S01", area: "Privacy & projects", title: "Private project media and version history", outcome: "Keep originals, edits, versions and their history safe across restarts.",
    current: "Projects store basic details. Imports, shot plans and captured frames last only for this session.", state: "partial", route: "library",
    acceptance: "Clear rules for where files go, no surprise OneDrive copies, a lasting history, and safe upgrades." },
  { id: "S02", area: "Privacy & projects", title: "Content intent and adult model consent", outcome: "Set general, mature or adult intent apart from model search and image reveal.",
    current: "Shots can carry an intent label, and adult model search has its own opt-in. Neither is an age check or permission to run.", state: "partial", route: "models",
    acceptance: "The app itself enforces rights, asks before revealing images, checks age where the law requires it, and stores files safely." },
  { id: "S03", area: "Privacy & projects", title: "Offline editing without AI models", outcome: "Import, edit and save without a Hugging Face account.",
    current: "Basic photo editing and saving work with no models. Full offline video editing is not built.", state: "partial", route: "photo",
    acceptance: "On a fresh install with no internet, photo and video work can be finished and saved." },
  { id: "S04", area: "Privacy & projects", title: "Resumable render and download jobs", outcome: "See lasting progress, cancel safely, and recover after a restart.",
    current: "Model downloads show progress and can be cancelled on development branches. Video render jobs are not connected.", state: "partial", route: "models",
    acceptance: "Each job keeps its exact identity, is picked up again after a restart, never leaves broken files, and handles failures." },
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
