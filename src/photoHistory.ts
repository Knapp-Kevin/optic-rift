import { EMPTY_PHOTO_CROP, validatePhotoAdjustments, type PhotoAdjustments } from "./photoEdits.ts";

/** Per-source session edit history. Never writes image bytes or persists a project asset. */
export type PhotoEditHistory = {
  past: PhotoAdjustments[];
  present: PhotoAdjustments;
  future: PhotoAdjustments[];
  /** Gesture identifier; repeated changes from one slider drag form one undo step. */
  gesture: string | null;
};

export const MAX_PHOTO_UNDO = 40;
export const DEFAULT_PHOTO_EDITS: Readonly<PhotoAdjustments> = Object.freeze({
  brightness: 100, contrast: 100, saturation: 100, rotation: 0,
});

function snapshot(value: PhotoAdjustments): PhotoAdjustments {
  const valid = validatePhotoAdjustments(value);
  return { ...valid, ...(valid.crop ? { crop: { ...valid.crop } } : {}) };
}

function equals(a: PhotoAdjustments, b: PhotoAdjustments): boolean {
  const ac = a.crop ?? EMPTY_PHOTO_CROP;
  const bc = b.crop ?? EMPTY_PHOTO_CROP;
  return a.brightness === b.brightness && a.contrast === b.contrast &&
    (a.saturation ?? 100) === (b.saturation ?? 100) &&
    a.rotation === b.rotation && (a.flipHorizontal ?? false) === (b.flipHorizontal ?? false) &&
    (a.flipVertical ?? false) === (b.flipVertical ?? false) &&
    ac.left === bc.left && ac.top === bc.top &&
    ac.right === bc.right && ac.bottom === bc.bottom;
}

export function createPhotoHistory(): PhotoEditHistory {
  return { past: [], present: snapshot(DEFAULT_PHOTO_EDITS), future: [], gesture: null };
}

/** Validate before mutation. A no-op retains the identical history object. */
export function applyPhotoEdit(
  history: PhotoEditHistory,
  changes: Partial<PhotoAdjustments>,
  gesture: string | null = null,
): PhotoEditHistory {
  const next = snapshot({ ...history.present, ...changes });
  if (equals(history.present, next)) return history;
  const continueGesture = !!gesture && history.gesture === gesture &&
    history.past.length > 0 && history.future.length === 0;
  const past = continueGesture ? history.past :
    [...history.past, snapshot(history.present)].slice(-MAX_PHOTO_UNDO);
  return { past, present: next, future: [], gesture };
}

export function finishPhotoGesture(history: PhotoEditHistory): PhotoEditHistory {
  return history.gesture === null ? history : { ...history, gesture: null };
}

export function undoPhotoEdit(history: PhotoEditHistory): PhotoEditHistory {
  if (!history.past.length) return history;
  const previous = history.past[history.past.length - 1];
  return {
    past: history.past.slice(0, -1),
    present: snapshot(previous),
    future: [snapshot(history.present), ...history.future].slice(0, MAX_PHOTO_UNDO),
    gesture: null,
  };
}

export function redoPhotoEdit(history: PhotoEditHistory): PhotoEditHistory {
  if (!history.future.length) return history;
  const next = history.future[0];
  return {
    past: [...history.past, snapshot(history.present)].slice(-MAX_PHOTO_UNDO),
    present: snapshot(next),
    future: history.future.slice(1),
    gesture: null,
  };
}
