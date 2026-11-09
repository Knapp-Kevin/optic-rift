import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  EMPTY_PHOTO_CROP, photoResizedOutputDimensions, photoPaintDimensions,
  paintAdjustedPhoto, validatePhotoAdjustments,
} from "../src/photoEdits.ts";
import {
  createPhotoHistory, applyPhotoEdit, finishPhotoGesture, undoPhotoEdit, redoPhotoEdit,
} from "../src/photoHistory.ts";

const source = (path) => readFileSync(new URL(path, import.meta.url), "utf8");
const base = { brightness: 100, contrast: 100, saturation: 100, rotation: 0 };

test("Resize preserves aspect ratio and never upscales original pixels", () => {
  assert.deepEqual(photoResizedOutputDimensions(4000, 3000, 0), { width: 4000, height: 3000 });
  assert.deepEqual(photoResizedOutputDimensions(4000, 3000, 0, EMPTY_PHOTO_CROP, 2048),
    { width: 2048, height: 1536 });
  assert.deepEqual(photoResizedOutputDimensions(200, 100, 0, EMPTY_PHOTO_CROP, 2048),
    { width: 200, height: 100 });
  assert.deepEqual(photoResizedOutputDimensions(4000, 3000, 90, EMPTY_PHOTO_CROP, 1000),
    { width: 750, height: 1000 });
  const crop = { left: 1250, right: 1250, top: 0, bottom: 0 };
  assert.deepEqual(photoResizedOutputDimensions(4000, 3000, 0, crop, 1000),
    { width: 1000, height: 1000 });
});

test("Resize is optional and rejects malformed or out-of-range pixel targets", () => {
  assert.deepEqual(validatePhotoAdjustments(base), base);
  assert.equal(validatePhotoAdjustments({ ...base, resizeLongEdge: 1024 }).resizeLongEdge, 1024);
  for (const bad of [-1, 0, 31, 8193, 512.25, Infinity, NaN, null, "1024", true]) {
    assert.throws(() => validatePhotoAdjustments({ ...base, resizeLongEdge: bad }));
    assert.throws(() => photoResizedOutputDimensions(2000, 1000, 0, EMPTY_PHOTO_CROP, bad));
  }
  assert.throws(() => photoResizedOutputDimensions(9000, 1000, 0, EMPTY_PHOTO_CROP, 512));
  assert.throws(() => photoResizedOutputDimensions(5000, 5000, 0, EMPTY_PHOTO_CROP, 512));
});

test("Shared paint dimensions use the exact resized output and bounded preview", () => {
  assert.deepEqual(photoPaintDimensions(4000, 3000, 0, undefined, EMPTY_PHOTO_CROP, 2048),
    { width: 2048, height: 1536, drawWidth: 2048, drawHeight: 1536 });
  assert.deepEqual(photoPaintDimensions(4000, 3000, 0, 1400, EMPTY_PHOTO_CROP, 2048),
    { width: 1400, height: 1050, drawWidth: 1400, drawHeight: 1050 });
  assert.deepEqual(photoPaintDimensions(4000, 3000, 0, 1400, EMPTY_PHOTO_CROP, 900),
    { width: 900, height: 675, drawWidth: 900, drawHeight: 675 });
  assert.deepEqual(photoPaintDimensions(200, 100, 0, 1400, EMPTY_PHOTO_CROP, 2048),
    { width: 200, height: 100, drawWidth: 200, drawHeight: 100 });
  const operations = [];
  const ctx = {
    filter: "", save() {}, restore() {}, clearRect() {},
    scale(...args) { operations.push(["scale", ...args]); },
    translate() {}, rotate() {}, drawImage() { operations.push(["draw"]); },
  };
  const canvas = { width: 0, height: 0, getContext: () => ctx };
  const dims = paintAdjustedPhoto({ width: 4000, height: 3000 }, {
    ...base, resizeLongEdge: 1000,
  }, canvas);
  assert.deepEqual(dims, { width: 1000, height: 750 });
  assert.deepEqual(operations[0], ["scale", 0.25, 0.25]);
  assert.equal(operations.filter(op => op[0] === "draw").length, 1);
});

test("Resize changes form bounded independent undo steps and reset returns original size", () => {
  const initial = createPhotoHistory();
  const a = applyPhotoEdit(initial, { resizeLongEdge: 2000 }, "resize");
  const b = applyPhotoEdit(a, { resizeLongEdge: 1000 }, "resize");
  assert.equal(b.past.length, 1);
  assert.equal(undoPhotoEdit(b).present.resizeLongEdge, undefined);
  assert.equal(redoPhotoEdit(undoPhotoEdit(b)).present.resizeLongEdge, 1000);
  const c = applyPhotoEdit(finishPhotoGesture(b), { resizeLongEdge: 700 }, "resize");
  assert.equal(c.past.length, 2);
  const d = applyPhotoEdit(c, { resizeLongEdge: undefined });
  assert.equal(d.present.resizeLongEdge, undefined);
  assert.equal(undoPhotoEdit(d).present.resizeLongEdge, 700);
  assert.equal(initial.present.resizeLongEdge, undefined);
});

test("UI, preview, export and native receipt share resize while original comparison stays untouched", () => {
  const app = source("../src/App.tsx");
  const preview = source("../src/PhotoCanvasPreview.tsx");
  const native = source("../src-tauri/src/photo_export.rs");
  const raster = source("../src/photoEdits.ts");
  assert.match(app, /Resize exported photo/);
  assert.match(app, /aria-label="Output longest edge pixels"/);
  assert.match(app, /photoResizedOutputDimensions\(/);
  assert.match(app, /resize_long_edge: resizeLongEdge \?\? null/);
  assert.match(app, /renderAdjustedPhoto\(selected\.url, \{ brightness, contrast, saturation, rotation, straightenTenths, resizeLongEdge, crop, flipHorizontal, flipVertical \}\)/);
  assert.match(preview, /paintAdjustedPhoto\(decoded\.bitmap, \{ brightness, contrast, saturation, rotation, straightenTenths, resizeLongEdge, crop, flipHorizontal, flipVertical \}, canvas, 1400\)/);
  assert.match(preview, /showOriginal\s*\? paintAdjustedPhoto\(decoded\.bitmap, \{ brightness: 100, contrast: 100, rotation: 0 \}, canvas, 1400\)/);
  assert.match(raster, /photoPaintDimensions\(bitmap\.width, bitmap\.height, rotation, maxPreviewEdge, crop, resizeLongEdge\)/);
  assert.match(native, /pub resize_long_edge: Option<u32>/);
  assert.match(native, /r\.width\.max\(r\.height\) > longest/);
  assert.match(native, /"resize_long_edge_requested_webview_reported": request\.resize_long_edge/);
  assert.doesNotMatch(native.split("#[cfg(test)]")[0], /std::process::Command/);
});
