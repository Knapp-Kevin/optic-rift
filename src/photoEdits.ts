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

/** Validate source pixel dimensions BEFORE asking the browser image decoder to allocate.
 * We intentionally reject unfamiliar, truncated or unusually padded headers.
 * Only these three supported formats can pass the offline photo-export path.
 */
export function probePhotoDimensions(bytes: Uint8Array, mime: string): { width: number; height: number } {
  const u16be = (i: number) => ((bytes[i] ?? 0) << 8) | (bytes[i + 1] ?? 0);
  const u16le = (i: number) => (bytes[i] ?? 0) | ((bytes[i + 1] ?? 0) << 8);
  const u24le = (i: number) =>
    (bytes[i] ?? 0) | ((bytes[i + 1] ?? 0) << 8) | ((bytes[i + 2] ?? 0) << 16);
  const u32be = (i: number) => (((bytes[i] ?? 0) * 0x1000000)
    + ((bytes[i + 1] ?? 0) << 16) + ((bytes[i + 2] ?? 0) << 8) + (bytes[i + 3] ?? 0)) >>> 0;
  let width = 0;
  let height = 0;

  if (mime === "image/png") {
    const sig = [137, 80, 78, 71, 13, 10, 26, 10];
    if (bytes.length < 24 || sig.some((v, i) => bytes[i] !== v) ||
        u32be(8) !== 13 || String.fromCharCode(...bytes.slice(12, 16)) !== "IHDR") {
      throw new Error("Unrecognized PNG header.");
    }
    width = u32be(16);
    height = u32be(20);
  } else if (mime === "image/jpeg") {
    if (bytes.length < 12 || bytes[0] !== 255 || bytes[1] !== 216) {
      throw new Error("Unrecognized JPEG header.");
    }
    let p = 2;
    const limit = Math.min(bytes.length, 64 * 1024);
    while (p + 8 <= limit) {
      if (bytes[p++] !== 255) break;
      while (bytes[p] === 255) p++;
      const marker = bytes[p++];
      if (marker === 218 || marker === 217 || marker === undefined) break;
      if (marker === 1 || (marker >= 208 && marker <= 215)) continue;
      if (p + 2 > limit) break;
      const length = u16be(p);
      if (length < 2 || p + length > limit) break;
      if ([192, 193, 194, 195, 197, 198, 199, 201, 202, 203, 205, 206, 207].includes(marker)) {
        if (length < 7) break;
        height = u16be(p + 3);
        width = u16be(p + 5);
        break;
      }
      p += length;
    }
  } else if (mime === "image/webp") {
    if (bytes.length < 25 || String.fromCharCode(...bytes.slice(0, 4)) !== "RIFF" ||
        String.fromCharCode(...bytes.slice(8, 12)) !== "WEBP") {
      throw new Error("Unrecognized WebP header.");
    }
    const kind = String.fromCharCode(...bytes.slice(12, 16));
    if (kind === "VP8X" && bytes.length >= 30) {
      if (bytes[20] & 2) throw new Error("Animated WebP is not a still photo. Extract a still frame first.");
      width = 1 + u24le(24);
      height = 1 + u24le(27);
    } else if (kind === "VP8 " && bytes.length >= 30 && bytes[23] === 157 &&
               bytes[24] === 1 && bytes[25] === 42) {
      width = u16le(26) & 0x3fff;
      height = u16le(28) & 0x3fff;
    } else if (kind === "VP8L" && bytes.length >= 25 && bytes[20] === 47) {
      width = 1 + (bytes[21] | ((bytes[22] & 63) << 8));
      height = 1 + ((bytes[22] >> 6) | (bytes[23] << 2) | ((bytes[24] & 15) << 10));
    }
  } else {
    throw new Error("This source image type is not supported for PNG export.");
  }
  // Bounds are checked before createImageBitmap, not afterwards.
  photoExportDimensions(width, height, 0);
  return { width, height };
}

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

/**
 * Shared raster pipeline for display and export. The only difference is the
 * preview's bounded output resolution; canvas color/filter order is identical.
 * The source ImageBitmap is owned by the caller and must be closed by it.
 */
export function paintAdjustedPhoto(
  bitmap: ImageBitmap, edits: PhotoAdjustments,
  canvas: HTMLCanvasElement, maxPreviewEdge?: number,
): { width: number; height: number } {
  const { brightness, contrast, rotation } = validatePhotoAdjustments(edits);
  photoExportDimensions(bitmap.width, bitmap.height, rotation);

  if (maxPreviewEdge !== undefined &&
      (!Number.isInteger(maxPreviewEdge) || maxPreviewEdge < 128 || maxPreviewEdge > 2048)) {
    throw new Error("The requested photo preview dimensions are invalid.");
  }
  const scale = maxPreviewEdge === undefined ? 1 :
    Math.min(1, maxPreviewEdge / Math.max(bitmap.width, bitmap.height));
  const drawWidth = Math.max(1, Math.round(bitmap.width * scale));
  const drawHeight = Math.max(1, Math.round(bitmap.height * scale));
  const dims = photoExportDimensions(drawWidth, drawHeight, rotation);
  canvas.width = dims.width;
  canvas.height = dims.height;
  const ctx = canvas.getContext("2d", { alpha: true });
  if (!ctx) throw new Error("The photo rendering canvas is unavailable.");
  ctx.save();
  try {
    ctx.clearRect(0, 0, dims.width, dims.height);
    ctx.translate(dims.width / 2, dims.height / 2);
    ctx.rotate(rotation * Math.PI / 180);
    ctx.filter = `brightness(${brightness}%) contrast(${contrast}%)`;
    ctx.drawImage(bitmap, -drawWidth / 2, -drawHeight / 2, drawWidth, drawHeight);
  } finally {
    ctx.restore();
  }
  return dims;
}

export async function loadPhotoBitmap(
  sourceUrl: string,
): Promise<{ bitmap: ImageBitmap; source_sha256: string }> {
  // Source fetch is limited to the app-owned, same-session blob URL.
  if (!sourceUrl.startsWith("blob:")) {
    throw new Error("Only photos opened in this Studio session can be edited.");
  }
  const response = await fetch(sourceUrl);
  if (!response.ok) throw new Error("The source photo is no longer available.");
  const source = await response.blob();
  if (!["image/png", "image/jpeg", "image/webp"].includes(source.type)) {
    throw new Error("Only PNG, JPEG and WebP photos can be exported. This file type is not supported.");
  }
  if (!source.size || source.size > MAX_PHOTO_SOURCE_BYTES) {
    throw new Error("The source photo exceeds the supported 32 MB export limit.");
  }
  const bytes = await source.arrayBuffer();
  const expected = probePhotoDimensions(new Uint8Array(bytes), source.type);
  const source_sha256 = hex(new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)));
  const bitmap = await createImageBitmap(source);
  // JPEG EXIF orientation can legitimately swap dimensions; reject any other
  // discrepancy. Never keep a decoded image above the bounded pixel budget.
  const expectedExact = bitmap.width === expected.width && bitmap.height === expected.height;
  const expectedSwapped = bitmap.width === expected.height && bitmap.height === expected.width;
  if (!(expectedExact || expectedSwapped)) {
    bitmap.close();
    throw new Error("Decoded photo dimensions do not match its checked source header.");
  }
  return { bitmap, source_sha256 };
}

export async function renderAdjustedPhoto(
  sourceUrl: string, edits: PhotoAdjustments,
): Promise<{ png_base64: string; source_sha256: string; width: number; height: number }> {
  const { bitmap, source_sha256 } = await loadPhotoBitmap(sourceUrl);
  try {
    const canvas = document.createElement("canvas");
    const dims = paintAdjustedPhoto(bitmap, edits, canvas);
    const output = await new Promise<Blob>((resolve, reject) =>
      canvas.toBlob((blob) => blob ? resolve(blob) : reject(new Error("Photo encoding failed.")), "image/png"),
    );
    if (output.type !== "image/png" || !output.size || output.size > MAX_PHOTO_PNG_BYTES) {
      throw new Error("The adjusted PNG exceeds the supported 32 MB export limit.");
    }
    return { png_base64: await pngBase64(output), source_sha256, ...dims };
  } finally {
    bitmap.close();
  }
}
