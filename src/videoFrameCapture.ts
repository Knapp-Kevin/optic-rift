/**
 * Explicit, bounded cross-media still extraction from a local video already
 * opened by the user. The resulting PNG is in memory only until the user
 * separately chooses Photo Studio's consent-gated export.
 *
 * No FFmpeg, model, GPU inference, remote fetch, thumbnail cache, filesystem
 * write, audio operation, or automatic content reveal.
 */

export const MAX_CAPTURE_EDGE = 8192;
export const MAX_CAPTURE_PIXELS = 20_000_000;
export const MAX_CAPTURE_PNG_BYTES = 32 * 1024 * 1024;

export type CapturedVideoFrame = {
  blob: Blob;
  width: number;
  height: number;
  timeSeconds: number;
};

export function validateFrameDimensions(width: number, height: number): { width: number; height: number } {
  if (!Number.isInteger(width) || !Number.isInteger(height) ||
      width < 1 || height < 1 ||
      width > MAX_CAPTURE_EDGE || height > MAX_CAPTURE_EDGE ||
      width * height > MAX_CAPTURE_PIXELS) {
    throw new Error("This video frame is too large to edit safely (limit: 20 megapixels, 8192 px per edge).");
  }
  return { width, height };
}

export function captureFrameTime(seconds: number): number {
  if (!Number.isFinite(seconds) || seconds < 0) {
    throw new Error("The current video position is not available.");
  }
  return Math.round(seconds * 1000) / 1000;
}

export function validateFrameSource(
  video: HTMLVideoElement,
  expectedBlobUrl: string,
): { width: number; height: number; timeSeconds: number } {
  // Reject any unrelated media element, remote site, or stale clip after a
  // selection switch. This must stay an explicit action on the opened clip.
  if (!expectedBlobUrl.startsWith("blob:") || video.currentSrc !== expectedBlobUrl) {
    throw new Error("The selected local video is no longer available for frame capture.");
  }
  if (video.error || video.seeking || video.readyState < 2) {
    throw new Error("Wait until the selected video frame is ready, then capture again.");
  }
  const { width, height } = validateFrameDimensions(video.videoWidth, video.videoHeight);
  const timeSeconds = captureFrameTime(video.currentTime);
  if (Number.isFinite(video.duration) && timeSeconds > video.duration + 0.01) {
    throw new Error("The video position is outside the current clip.");
  }
  return { width, height, timeSeconds };
}

export async function captureVideoFrame(
  video: HTMLVideoElement,
  expectedBlobUrl: string,
): Promise<CapturedVideoFrame> {
  video.pause();
  const { width, height, timeSeconds } = validateFrameSource(video, expectedBlobUrl);

  // Draw the current decoded frame synchronously. The PNG encoding step can
  // be asynchronous; the video has already been paused and rasterized.
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext("2d", { alpha: false });
  if (!ctx) throw new Error("A photo capture canvas is not available.");
  try {
    ctx.drawImage(video, 0, 0, width, height);
  } catch {
    throw new Error("This video frame could not be read. Its format or protections may prevent capture.");
  }
  const blob = await new Promise<Blob>((resolve, reject) => {
    canvas.toBlob(
      (result) => result ? resolve(result) : reject(new Error("Could not encode this frame as a PNG.")),
      "image/png",
    );
  });
  if (blob.type !== "image/png" || !blob.size || blob.size > MAX_CAPTURE_PNG_BYTES) {
    throw new Error("The captured PNG exceeds the supported 32 MB limit or is invalid.");
  }
  return { blob, width, height, timeSeconds };
}

/** Session-only filename. No user video path is serialized to export metadata. */
export function capturedFrameName(timeSeconds: number): string {
  const millis = Math.round(captureFrameTime(timeSeconds) * 1000);
  const hours = Math.floor(millis / 3_600_000).toString().padStart(2, "0");
  const minutes = Math.floor(millis % 3_600_000 / 60_000).toString().padStart(2, "0");
  const seconds = Math.floor(millis % 60_000 / 1000).toString().padStart(2, "0");
  const ms = (millis % 1000).toString().padStart(3, "0");
  return `video-frame-${hours}-${minutes}-${seconds}-${ms}.png`;
}
