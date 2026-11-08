import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  EMPTY_PHOTO_CROP, centeredPhotoCrop, photoCropBounds,
  photoPaintDimensions, paintAdjustedPhoto, validatePhotoCrop,
  validatePhotoAdjustments,
} from "../src/photoEdits.ts";

const source = (file) => readFileSync(new URL(file, import.meta.url), "utf8");

test("Cropping is bounded, uses exact post-rotation pixel insets, and cannot erase a photo", () => {
  assert.deepEqual(photoCropBounds(4000, 3000), { x: 0, y: 0, width: 4000, height: 3000 });
  assert.deepEqual(photoCropBounds(4000, 3000, { left: 1250, top: 1000, right: 1250, bottom: 2000 }),
    { x: 500, y: 300, width: 3000, height: 2100 });
  assert.deepEqual(photoCropBounds(2, 2, { left: 4900, top: 4900, right: 4900, bottom: 4900 }),
    { x: 0, y: 0, width: 2, height: 2 });
  for (const invalid of [
    { left: -1, top: 0, right: 0, bottom: 0 },
    { left: 9501, top: 0, right: 0, bottom: 0 },
    { left: 9500, top: 0, right: 500, bottom: 0 },
    { left: 0, top: 8000, right: 0, bottom: 2000 },
    { left: NaN, top: 0, right: 0, bottom: 0 },
    { left: 0.5, top: 0, right: 0, bottom: 0 },
  ]) assert.throws(() => validatePhotoCrop(invalid));
  assert.throws(() => photoCropBounds(5000, 5000));
});

test("Centered aspect crop presets follow rotation, never mutate the original", () => {
  const square = centeredPhotoCrop(4000, 3000, 0, 1);
  assert.deepEqual(square, { left: 1250, right: 1250, top: 0, bottom: 0 });
  assert.deepEqual(photoCropBounds(4000, 3000, square), { x: 500, y: 0, width: 3000, height: 3000 });
  const rotated = centeredPhotoCrop(4000, 3000, 90, 1);
  assert.deepEqual(rotated, { left: 0, right: 0, top: 1250, bottom: 1250 });
  assert.deepEqual(photoCropBounds(3000, 4000, rotated), { x: 0, y: 500, width: 3000, height: 3000 });
  assert.deepEqual(centeredPhotoCrop(4000, 3000, 0, 4 / 5), { left: 2000, right: 2000, top: 0, bottom: 0 });
  assert.throws(() => centeredPhotoCrop(4000, 3000, 0, NaN));
  assert.deepEqual(EMPTY_PHOTO_CROP, { left: 0, top: 0, right: 0, bottom: 0 });
});

test("Crop and rotation have matching full-resolution and bounded preview geometry", () => {
  const c = { left: 2500, top: 0, right: 0, bottom: 5000 };
  assert.deepEqual(photoPaintDimensions(400, 200, 90, undefined, c),
    { width: 150, height: 200, drawWidth: 400, drawHeight: 200 });
  assert.deepEqual(photoPaintDimensions(4000, 3000, 90, 1400, c),
    { width: 1400, height: 1244, drawWidth: 2489, drawHeight: 1867 });
  const square = { left: 1250, top: 0, right: 1250, bottom: 0 };
  assert.deepEqual(photoPaintDimensions(4000, 3000, 0, 1400, square),
    { width: 1400, height: 1400, drawWidth: 1867, drawHeight: 1400 });
  assert.deepEqual(validatePhotoAdjustments({ brightness: 120, contrast: 110, rotation: 90, crop: c }),
    { brightness: 120, contrast: 110, rotation: 90, crop: c });
});

test("Preview and exported pixels use the same crop-after-rotate canvas transform", () => {
  const ops = [];
  const ctx = {
    filter: "",
    save() { ops.push(["save"]); },
    restore() { ops.push(["restore"]); },
    clearRect(...p) { ops.push(["clear", ...p]); },
    scale(...p) { ops.push(["scale", ...p]); },
    translate(...p) { ops.push(["translate", ...p]); },
    rotate(...p) { ops.push(["rotate", ...p]); },
    drawImage(...p) { ops.push(["draw", ...p]); },
  };
  const canvas = { width: 0, height: 0, getContext: () => ctx };
  const bitmap = { width: 400, height: 200 };
  const edits = {
    brightness: 120, contrast: 110, rotation: 90,
    crop: { left: 2500, top: 0, right: 0, bottom: 5000 },
  };
  const dims = paintAdjustedPhoto(bitmap, edits, canvas);
  assert.deepEqual(dims, { width: 150, height: 200 });
  assert.deepEqual(ops.map((x) => x[0]), ["save", "clear", "scale", "translate", "translate", "rotate", "draw", "restore"]);
  assert.deepEqual(ops[3], ["translate", -50, 0]);
  assert.deepEqual(ops[4], ["translate", 100, 200]);
  assert.equal(ops[5][1], Math.PI / 2);
  assert.deepEqual(ops[6], ["draw", bitmap, -200, -100]);
  assert.equal(ctx.filter, "brightness(120%) contrast(110%)");
  assert.deepEqual(canvas.width, 150);
  assert.deepEqual(canvas.height, 200);
});

test("Inspector, preview, and native export all retain crop details without a hidden save", () => {
  const app = source("../src/App.tsx");
  const preview = source("../src/PhotoCanvasPreview.tsx");
  const native = source("../src-tauri/src/photo_export.rs").split("#[cfg(test)]")[0];
  assert.match(app, /Crop photo/);
  assert.match(app, /Square 1:1/);
  assert.match(app, /Wide 16:9/);
  assert.match(app, /Portrait 4:5/);
  assert.match(app, /Reset crop/);
  assert.match(app, /updatePhotoCrop\(\{ \[edge\]: Number\(event\.target\.value\) \}\)/);
  assert.match(app, /renderAdjustedPhoto\(selected\.url, \{ brightness, contrast, rotation, crop \}\)/);
  assert.match(app, /crop: \{ \.\.\.crop \},/);
  assert.match(preview, /paintAdjustedPhoto\(decoded\.bitmap, \{ brightness, contrast, rotation, crop \}, canvas, 1400\)/);
  assert.match(native, /pub crop: Option<PhotoCrop>/);
  assert.match(native, /fn validate_crop\(/);
  assert.match(native, /crop_insets_basis_points_of_rotated_image/);
  assert.match(app, /window\.confirm\(/);
  assert.doesNotMatch(app.slice(app.indexOf("function updatePhotoCrop("), app.indexOf("function addMedia(")),
    /photo_export_copy|invoke\(/);
});
