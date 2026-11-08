/** Session-only shot plans. A planned video is NOT an execution request,
 * a media file, a project asset or evidence that a model is runnable.
 *
 * Native execution must be added behind a separately qualified, authenticated
 * capability contract. Catalog metadata and local model files do not suffice.
 */
export type VideoShotMode = "text-to-video" | "image-to-video" | "video-to-video";
export type VideoAspect = "16:9" | "9:16" | "1:1" | "4:3";
export type VisualIntent = "general" | "mature-themes" | "adult-focused";
export type CameraMotion = "static" | "pan" | "tracking" | "handheld";

export type VideoShotInput = {
  mode: VideoShotMode;
  prompt: string;
  referenceId: string | null;
  aspect: VideoAspect;
  durationSeconds: number;
  intent: VisualIntent;
  camera: CameraMotion;
};

export type VideoShot = VideoShotInput & {
  id: string;
};

export const MAX_VIDEO_SHOT_PROMPT = 1200;
export const MAX_SESSION_VIDEO_SHOTS = 24;
export const VIDEO_GENERATION_EXECUTION_QUALIFIED = false as const;

const shotModes: VideoShotMode[] = ["text-to-video", "image-to-video", "video-to-video"];
const shotAspects: VideoAspect[] = ["16:9", "9:16", "1:1", "4:3"];
const shotIntents: VisualIntent[] = ["general", "mature-themes", "adult-focused"];
const cameraMotions: CameraMotion[] = ["static", "pan", "tracking", "handheld"];

export function normalizeVideoShot(
  input: VideoShotInput, availableImageIds: readonly string[], availableVideoIds: readonly string[],
): VideoShotInput {
  const prompt = input.prompt.trim();
  if (!shotModes.includes(input.mode) || !shotAspects.includes(input.aspect) ||
      !shotIntents.includes(input.intent) || !cameraMotions.includes(input.camera) ||
      !Number.isInteger(input.durationSeconds) ||
      input.durationSeconds < 2 || input.durationSeconds > 12) {
    throw new Error("This shot has unsupported planning settings.");
  }
  if (!prompt || prompt.length > MAX_VIDEO_SHOT_PROMPT ||
      /[\u0000-\u0008\u000B\u000C\u000E-\u001F]/.test(prompt)) {
    throw new Error("Describe the shot in 1 to 1200 characters without control characters.");
  }
  const referenceId = input.referenceId || null;
  if (input.mode === "image-to-video" &&
      (!referenceId || !availableImageIds.includes(referenceId))) {
    throw new Error("Choose a photo currently in your Media Library as the shot reference.");
  }
  const availableIds = input.mode === "video-to-video" ? availableVideoIds : availableImageIds;
  if (input.mode === "video-to-video" &&
      (!referenceId || !availableVideoIds.includes(referenceId))) {
    throw new Error("Choose a local video currently open in the Media Library.");
  }
  if (referenceId && !availableIds.includes(referenceId)) {
    throw new Error("This reference file is no longer open in the Media Library.");
  }
  return { ...input, prompt, referenceId };
}

export function moveVideoShot(
  shots: readonly VideoShot[], id: string, offset: -1 | 1,
): VideoShot[] {
  const current = shots.findIndex((shot) => shot.id === id);
  const destination = current + offset;
  if (current < 0 || destination < 0 || destination >= shots.length) return [...shots];
  const next = [...shots];
  [next[current], next[destination]] = [next[destination], next[current]];
  return next;
}

export function videoShotDuration(shots: readonly VideoShot[]): number {
  return shots.reduce((total, shot) => total + shot.durationSeconds, 0);
}

/** This label is intentionally not a Motion Picture Association classification.
 * The user's visual-intent note NEVER authorizes adult model discovery or execution.
 */
export const videoIntentLabel: Record<VisualIntent, string> = {
  general: "General themes",
  "mature-themes": "Mature / PG-13-style themes",
  "adult-focused": "Adult-focused (18+) intent",
};

export const cameraMotionLabel: Record<CameraMotion, string> = {
  static: "Locked-off camera",
  pan: "Pan / tilt",
  tracking: "Tracking movement",
  handheld: "Handheld look",
};

export const videoModeLabel: Record<VideoShotMode, string> = {
  "text-to-video": "Text to video",
  "image-to-video": "Photo to video",
  "video-to-video": "Video to video",
};
