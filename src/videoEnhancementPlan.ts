/** Video enhancement is a user-intent planning surface, not a renderer.
 * Every option below is future scope until the native media pipeline is
 * qualified. Never transmit a plan to an unverified model or start a job.
 */
export type EnhanceOperation =
  | "denoise" | "upscale" | "stabilize" | "frame-interpolation"
  | "deinterlace" | "color-repair" | "sharpen" | "scratch-repair";
export type EnhancePriority = "preserve-detail" | "balanced" | "maximum-improvement";

export type VideoEnhancementPlan = {
  videoId: string | null;
  operations: EnhanceOperation[];
  target: "original" | "1080p" | "2160p";
  priority: EnhancePriority;
  notes: string;
};

export const videoEnhanceOperations: ReadonlyArray<{
  id: EnhanceOperation;
  name: string;
  description: string;
  risk: string;
}> = [
  { id: "denoise", name: "Reduce noise", description: "Remove distracting grain or sensor noise.",
    risk: "Can erase fine textures if overused." },
  { id: "upscale", name: "Increase resolution", description: "Plan a larger delivery size.",
    risk: "A larger image does not automatically contain more genuine detail." },
  { id: "stabilize", name: "Stabilize footage", description: "Smooth unwanted camera shake.",
    risk: "Can crop frame edges or distort motion." },
  { id: "frame-interpolation", name: "Smoother motion", description: "Plan interpolation between source frames.",
    risk: "Synthesized motion may create artifacts." },
  { id: "deinterlace", name: "Repair interlaced footage", description: "Convert field-based footage into progressive frames.",
    risk: "Field-order errors can harm clarity." },
  { id: "color-repair", name: "Correct color and exposure", description: "Repair color casts, contrast and exposure.",
    risk: "Incorrect assumptions can shift skin tones or clip highlights." },
  { id: "sharpen", name: "Recover apparent sharpness", description: "Plan conservative detail enhancement.",
    risk: "Over-sharpening causes halos or invented-looking detail." },
  { id: "scratch-repair", name: "Restore damaged footage", description: "Plan dust and scratch repair.",
    risk: "Repair can remove intended scene details." },
];

export const VIDEO_ENHANCEMENT_EXECUTION_QUALIFIED = false as const;
export const MAX_VIDEO_ENHANCE_NOTES = 500;

export function normalizeVideoEnhancementPlan(
  plan: VideoEnhancementPlan,
  availableVideoIds: readonly string[],
): VideoEnhancementPlan {
  if (!plan.videoId || !availableVideoIds.includes(plan.videoId)) {
    throw new Error("Choose a local video in your Media Library first.");
  }
  if (!["original", "1080p", "2160p"].includes(plan.target) ||
      !["preserve-detail", "balanced", "maximum-improvement"].includes(plan.priority)) {
    throw new Error("Unsupported video enhancement preferences.");
  }
  const valid = new Set(videoEnhanceOperations.map((x) => x.id));
  const operations = [...new Set(plan.operations)];
  if (operations.length === 0 || operations.some((op) => !valid.has(op))) {
    throw new Error("Choose at least one supported planning operation.");
  }
  if (plan.notes.length > MAX_VIDEO_ENHANCE_NOTES || /[\u0000-\u0008\u000B\u000C\u000E-\u001F]/.test(plan.notes)) {
    throw new Error("Keep enhancement notes under 500 characters without control characters.");
  }
  return { ...plan, operations, notes: plan.notes.trim() };
}
