/** Human-readable file size; small files must not round down to "0.0 MB". */
export function formatSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "unknown size";
  if (bytes < 1024) return bytes + " B";
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + " KB";
  if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + " MB";
  return (bytes / (1024 * 1024 * 1024)).toFixed(2) + " GB";
}

export const IMAGE_PREVIEW_FAILED = "This image can't be previewed. The file may be damaged or not really an image.";
// The webview reports one error code for both cases, so the message must not guess which.
export const VIDEO_PREVIEW_FAILED = "This video can't be previewed here. Its codec may be unsupported by the preview, or the file may be damaged.";
