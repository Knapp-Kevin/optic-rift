import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { validatePhotoAdjustments, paintAdjustedPhoto } from "../src/photoEdits.ts";
import { createPhotoHistory, applyPhotoEdit, undoPhotoEdit, redoPhotoEdit, finishPhotoGesture } from "../src/photoHistory.ts";

const source = (path) => readFileSync(new URL(path, import.meta.url), "utf8");
const base = { brightness: 100, contrast: 100, rotation: 0 };

test("Saturation validates bounded integer values and neutral optional compatibility", () => {
  assert.deepEqual(validatePhotoAdjustments(base), base);
  for (const percent of [0, 100, 200]) {
    assert.equal(validatePhotoAdjustments({ ...base, saturation: percent }).saturation, percent);
  }
  for (const invalid of [-1, 201, 44.5, NaN, Infinity, "100", null]) {
    assert.throws(() => validatePhotoAdjustments({ ...base, saturation: invalid }));
  }
});

test("Neutral legacy filter, desaturated and vivid filters use one export/preview renderer", () => {
  const operations = [];
  const ctx = {
    filter: "",
    save() { operations.push("save"); },
    restore() { operations.push("restore"); },
    clearRect() {},
    scale() {},
    translate() {},
    rotate() {},
    drawImage() { operations.push(["draw", this.filter]); },
  };
  const canvas = { width: 0, height: 0, getContext: () => ctx };
  const bitmap = { width: 200, height: 100 };
  const draw = (saturation) => {
    operations.length = 0;
    const edits = saturation === undefined ? base : { ...base, saturation };
    const dimensions = paintAdjustedPhoto(bitmap, edits, canvas);
    return { filter: ctx.filter, dimensions, draws: operations.filter(x => Array.isArray(x)).length };
  };
  assert.deepEqual(draw(undefined), { filter: "brightness(100%) contrast(100%)", dimensions: { width: 200, height: 100 }, draws: 1 });
  assert.equal(draw(100).filter, "brightness(100%) contrast(100%)");
  assert.equal(draw(0).filter, "brightness(100%) contrast(100%) saturate(0%)");
  assert.equal(draw(200).filter, "brightness(100%) contrast(100%) saturate(200%)");
  assert.equal(draw(50).draws, 1);
});

test("Saturation has one undo step per drag and remains independent of other photo edits", () => {
  const first = applyPhotoEdit(createPhotoHistory(), { saturation: 40 }, "saturation");
  const next = applyPhotoEdit(first, { saturation: 75 }, "saturation");
  assert.equal(next.past.length, 1);
  assert.equal(undoPhotoEdit(next).present.saturation, 100);
  assert.equal(redoPhotoEdit(undoPhotoEdit(next)).present.saturation, 75);
  const afterRelease = applyPhotoEdit(finishPhotoGesture(next), { saturation: 110 }, "saturation");
  assert.equal(afterRelease.past.length, 2);
  const rotation = applyPhotoEdit(afterRelease, { rotation: 90 });
  assert.equal(undoPhotoEdit(rotation).present.saturation, 110);
});

test("Studio preview, original comparison, export and native receipt agree on saturation", () => {
  const app = source("../src/App.tsx");
  const preview = source("../src/PhotoCanvasPreview.tsx");
  const raster = source("../src/photoEdits.ts");
  const native = source("../src-tauri/src/photo_export.rs");
  assert.match(app, /aria-label="Photo saturation percent"/);
  assert.match(app, /updatePhotoAdjustments\(\{ saturation: Number\(event\.target\.value\) \}, "saturation"\)/);
  assert.match(app, /saturation_percent: saturation/);
  assert.match(app, /saturation=\{saturation\}/);
  assert.match(preview, /showOriginal\s*\? paintAdjustedPhoto\(decoded\.bitmap, \{ brightness: 100, contrast: 100, rotation: 0 \}, canvas, 1400\)/);
  assert.match(preview, /: paintAdjustedPhoto\(decoded\.bitmap, \{ brightness, contrast, saturation, rotation, crop, flipHorizontal, flipVertical \}, canvas, 1400\)/);
  assert.match(raster, /saturation === 100 \? "" : ` saturate\(\$\{saturation\}%\)`/);
  assert.match(native, /r\.saturation_percent > 200/);
  assert.match(native, /#\[serde\(default = "neutral_saturation_percent"\)\]/);
  assert.match(native, /"saturation_percent": request\.saturation_percent/);
  assert.doesNotMatch(native.split("#[cfg(test)]")[0], /std::process::Command/);
});
