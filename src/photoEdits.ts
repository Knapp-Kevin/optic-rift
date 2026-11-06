/**
 * Bounded, CPU-only export of the three adjustments that Photo Studio already
 * previews. Originals and browser object URLs are read-only. The editor is not
 * yet a layer/mask editor, and exporting does not save the project.
 */
export type PhotoAdjustments = {
  brightness: number;
  contrast: number;
  rotation: number;
};

export const MAX_PHOTO_SOURCE_BYTES = 32 * 1024 * 1024;
export const MAX_PHOTO_PNG_BYTES = 32 * 1024 * 1024;
export const MAX_PHOTO_PIXELS = 20_000_000;
export const MAX_PHOTO_EDGE = 8192;

export function validatePhotoAdjustments(value: PhotoAdjustments): PhotoAdjustments {
  const { brightness, contrast, rotation } = value;
  if (!Number.isInteger(brightness) || brightness < 25 || brightness > 175 ||
      !Number.isInteger(contrast) || contrast < 25 || contrast > 175 ||
      !Number.isInteger(rotation) || ![0, 90, 180, 270].includes(rotation)) {
    throw new Error("Photo adjustment values are outside the supported export range.");
  }
  return { brightness, contrast, rotation };
}

export function photoExportDimensions(width: number, height: number, rotation: number) {
  if (!Number.isInteger(width) || !Number.isInteger(height) ||
      width < 1 || height < 1 || width > MAX_PHOTO_EDGE || height > MAX_PHOTO_EDGE ||
      width * height > MAX_PHOTO_PIXELS || ![0, 90, 180, 270].includes(rotation)) {
    throw new Error("This photo is too large or its dimensions cannot be exported safely.");
  }
  return rotation % 180 === 0 ? { width, height } : { width: height, height: width };
}

function hex(bytes: Uint8Array): string {
  return [...bytes].map((v) => v.toString(16).padStart(2, "0")).join("");
}

function pngBase64(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(new Error("Cannot read the rendered PNG."));
    reader.onload = () => {
      const value = reader.result;
      if (typeof value !== "string" || !value.startsWith("data:image/png;base64,")) {
        reject(new Error("The browser did not produce a PNG export."));
      } else {
        resolve(value.slice("data:image/png;base64,".length));
      }
    };
    reader.readAsDataURL(blob);
  });
}

export async function renderAdjustedPhoto(
  sourceUrl: string, edits: PhotoAdjustments,
): Promise<{ png_base64: string; source_sha256: string; width: number; height: number }> {
  const settings = validatePhotoAdjustments(edits);
  // Only browser-owned imported/generated media can be exported. Never fetch a
  // remote URL, local filesystem URL, or a model-card preview.
  if (!sourceUrl.startsWith("blob:")) {
    throw new Error("Only a photo opened in this Studio session can be exported.");
  }
  const response = await fetch(sourceUrl);
  if (!response.ok) throw new Error("The source photo is no longer available.");
  const source = await response.blob();
  if (!["image/png", "image/jpeg", "image/webp"].includes(source.type) || !source.size || source.size > MAX_PHOTO_SOURCE_BYTES) {
    throw new Error("The source photo exceeds the supported 32 MB export limit.");
  }
  // Source hash is a browser-observed lineage hint, not a native-authenticated
  // signature. Never claim this export is a project-owned revision.
  const bytes = await source.arrayBuffer();
  const source_sha256 = hex(new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)));
  const bitmap = await createImageBitmap(source);
  try {
    const dims = photoExportDimensions(bitmap.width, bitmap.height, settings.rotation);
    const canvas = document.createElement("canvas");
    canvas.width = dims.width;
    canvas.height = dims.height;
    const ctx = canvas.getContext("2d", { alpha: true });
    if (!ctx) throw new Error("The photo export canvas is unavailable.");
    ctx.save();
    ctx.translate(dims.width / 2, dims.height / 2);
    ctx.rotate(settings.rotation * Math.PI / 180);
    ctx.filter = `brightness(${settings.brightness}%) contrast(${settings.contrast}%)`;
    ctx.drawImage(bitmap, -bitmap.width / 2, -bitmap.height / 2);
    ctx.restore();
    const output = await new Promise<Blob>((resolve, reject) =>
      canvas.toBlob((blob) => blob ? resolve(blob) : reject(new Error("Photo encoding failed.")), "image/png"),
    );
    // Keep the UI and native byte limits consistent; no partial export.
    if (output.type !== "image/png" || !output.size || output.size > MAX_PHOTO_PNG_BYTES) {
      throw new Error("The adjusted PNG exceeds the supported 32 MB export limit.");
    }
    return { png_base64: await pngBase64(output), source_sha256, ...dims };
  } finally {
    bitmap.close();
  }
}
