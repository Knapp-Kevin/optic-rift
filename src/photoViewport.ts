/** Photo inspection only: viewport transforms never modify edit recipes or export pixels. */
export type PhotoPoint = { x: number; y: number };
export type PhotoSize = { width: number; height: number };

export const PHOTO_ZOOM_LEVELS = [1, 1.25, 1.5, 2, 3, 4] as const;

function validSize(size: PhotoSize): boolean {
  return Number.isFinite(size.width) && Number.isFinite(size.height) &&
    size.width > 0 && size.height > 0;
}

/** Scale the bounded proxy to the viewport without enlarging its bitmap. */
export function photoViewportFit(image: PhotoSize, viewport: PhotoSize): PhotoSize {
  if (!validSize(image) || !validSize(viewport)) return { width: 0, height: 0 };
  const scale = Math.min(1, viewport.width / image.width, viewport.height / image.height);
  return { width: image.width * scale, height: image.height * scale };
}

/** Pan only when the zoomed displayed bitmap exceeds its viewport on an axis. */
export function photoPanBounds(image: PhotoSize, viewport: PhotoSize, zoom: number): PhotoPoint {
  const fitted = photoViewportFit(image, viewport);
  if (!fitted.width || !Number.isFinite(zoom) || zoom < 1 || zoom > 4) return { x: 0, y: 0 };
  return {
    x: Math.max(0, (fitted.width * zoom - viewport.width) / 2),
    y: Math.max(0, (fitted.height * zoom - viewport.height) / 2),
  };
}

export function photoClampPan(pan: PhotoPoint, image: PhotoSize, viewport: PhotoSize, zoom: number): PhotoPoint {
  const limit = photoPanBounds(image, viewport, zoom);
  const clamp = (value: number, maximum: number) =>
    Number.isFinite(value) ? Math.min(maximum, Math.max(-maximum, value)) : 0;
  return { x: clamp(pan.x, limit.x), y: clamp(pan.y, limit.y) };
}

export function photoZoomStep(zoom: number, direction: -1 | 1): number {
  if (!Number.isFinite(zoom)) return 1;
  if (direction === 1) return PHOTO_ZOOM_LEVELS.find(level => level > zoom + 0.001) ?? 4;
  return [...PHOTO_ZOOM_LEVELS].reverse().find(level => level < zoom - 0.001) ?? 1;
}
