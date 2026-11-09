import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { photoStraightenCoverScale, paintAdjustedPhoto, validatePhotoAdjustments } from "../src/photoEdits.ts";
import { createPhotoHistory, applyPhotoEdit, finishPhotoGesture, undoPhotoEdit, redoPhotoEdit } from "../src/photoHistory.ts";

const source = (path) => readFileSync(new URL(path, import.meta.url), "utf8");
const base = { brightness: 100, contrast: 100, saturation: 100, rotation: 0 };

test("Fine straighten rejects forged/out-of-range values and is optional for older editors", () => {
  assert.deepEqual(validatePhotoAdjustments(base), base);
  for (const angle of [-150, -75, 0, 75, 150]) {
    assert.equal(validatePhotoAdjustments({ ...base, straightenTenths: angle }).straightenTenths, angle);
  }
  for (const bad of [-151, 151, 2.5, NaN, Infinity, "50", null]) {
    assert.throws(() => validatePhotoAdjustments({ ...base, straightenTenths: bad }));
  }
  assert.throws(() => photoStraightenCoverScale(0, 10, 10));
  assert.throws(() => photoStraightenCoverScale(10, Infinity, 10));
  assert.throws(() => photoStraightenCoverScale(400, 200, 151));
});

test("Auto-cover geometry leaves no blank corners for either straighten direction", () => {
  assert.equal(photoStraightenCoverScale(400, 200, 0), 1);
  for (const [w, h] of [[400, 200], [200, 400], [400, 400], [1920, 1080]]) {
    for (const tenths of [-150, -50, 50, 150]) {
      const radians = tenths * Math.PI / 1800;
      const cover = photoStraightenCoverScale(w, h, tenths);
      assert.ok(cover > 1 && Number.isFinite(cover));
      for (const x of [-w / 2, w / 2]) for (const y of [-h / 2, h / 2]) {
        const sourceX = (Math.cos(radians) * x + Math.sin(radians) * y) / cover;
        const sourceY = (-Math.sin(radians) * x + Math.cos(radians) * y) / cover;
        assert.ok(Math.abs(sourceX) <= w / 2 + 1e-8, "image fills output X");
        assert.ok(Math.abs(sourceY) <= h / 2 + 1e-8, "image fills output Y");
      }
    }
  }
});

test("Neutral legacy raster path unchanged; straighten rotates and zooms before display flip", () => {
  const operations = [];
  const ctx = {
    filter: "",
    save() { operations.push(["save"]); },
    restore() { operations.push(["restore"]); },
    clearRect() { operations.push(["clear"]); },
    scale(...args) { operations.push(["scale", ...args]); },
    translate(...args) { operations.push(["translate", ...args]); },
    rotate(...args) { operations.push(["rotate", ...args]); },
    drawImage(...args) { operations.push(["draw", ...args]); },
  };
  const bitmap = { width: 400, height: 200 };
  const canvas = { width: 0, height: 0, getContext: () => ctx };
  const neutral = paintAdjustedPhoto(bitmap, base, canvas);
  assert.deepEqual(neutral, { width: 400, height: 200 });
  assert.deepEqual(operations.map(op => op[0]),
    ["save", "clear", "scale", "translate", "translate", "rotate", "draw", "restore"]);
  operations.length = 0;
  const leveled = paintAdjustedPhoto(bitmap, {
    ...base, rotation: 90, straightenTenths: 75, flipHorizontal: true,
  }, canvas);
  assert.deepEqual(leveled, { width: 200, height: 400 });
  assert.deepEqual(operations.map(op => op[0]),
    ["save", "clear", "scale", "translate", "translate", "rotate", "scale", "scale", "rotate", "draw", "restore"]);
  assert.ok(Math.abs(operations[5][1] - 7.5 * Math.PI / 180) < 1e-12);
  const cover = photoStraightenCoverScale(200, 400, 75);
  assert.deepEqual(operations[6], ["scale", cover, cover]);
  assert.deepEqual(operations[7], ["scale", -1, 1]);
  assert.equal(operations[8][1], Math.PI / 2);
});

test("Straighten slider drags are grouped, reversible, and reset without altering the source", () => {
  const original = createPhotoHistory();
  assert.equal(original.present.straightenTenths, 0);
  const first = applyPhotoEdit(original, { straightenTenths: 25 }, "straighten");
  const drag = applyPhotoEdit(first, { straightenTenths: 80 }, "straighten");
  assert.equal(drag.past.length, 1);
  assert.equal(undoPhotoEdit(drag).present.straightenTenths, 0);
  assert.equal(redoPhotoEdit(undoPhotoEdit(drag)).present.straightenTenths, 80);
  const secondDrag = applyPhotoEdit(finishPhotoGesture(drag), { straightenTenths: -50 }, "straighten");
  assert.equal(secondDrag.past.length, 2);
  assert.equal(undoPhotoEdit(secondDrag).present.straightenTenths, 80);
  const reset = applyPhotoEdit(secondDrag, { straightenTenths: 0 });
  assert.equal(reset.present.straightenTenths, 0);
  assert.equal(undoPhotoEdit(reset).present.straightenTenths, -50);
  assert.equal(original.present.straightenTenths, 0);
});

test("Studio controls, native request, preview and export use the same angle", () => {
  const app = source("../src/App.tsx");
  const preview = source("../src/PhotoCanvasPreview.tsx");
  const native = source("../src-tauri/src/photo_export.rs");
  const renderer = source("../src/photoEdits.ts");
  assert.match(app, /Fine straighten/);
  assert.match(app, /aria-label="Fine straighten degrees clockwise"/);
  assert.match(app, /updatePhotoAdjustments\(\{ straightenTenths: Number\(event\.target\.value\) \}, "straighten"\)/);
  assert.match(app, /straighten_tenths: straightenTenths/);
  assert.match(app, /renderAdjustedPhoto\(selected\.url, \{ brightness, contrast, saturation, rotation, straightenTenths, resizeLongEdge, crop, flipHorizontal, flipVertical \}\)/);
  assert.match(preview, /paintAdjustedPhoto\(decoded\.bitmap, \{ brightness, contrast, saturation, rotation, straightenTenths, resizeLongEdge, crop, flipHorizontal, flipVertical \}, canvas, 1400\)/);
  assert.match(preview, /showOriginal\s*\? paintAdjustedPhoto\(decoded\.bitmap, \{ brightness: 100, contrast: 100, rotation: 0 \}, canvas, 1400\)/);
  assert.match(renderer, /const cover = photoStraightenCoverScale\(rotated\.width, rotated\.height, straightenTenths\)/);
  assert.match(native, /pub straighten_tenths: i16/);
  assert.match(native, /!\(-150\.\.=150\)\.contains\(&r\.straighten_tenths\)/);
  assert.match(native, /"straighten_tenths_degrees_clockwise": request\.straighten_tenths/);
  assert.doesNotMatch(native.split("#[cfg(test)]")[0], /std::process::Command/);
});

test("Crop presets remain disabled if dimensions are absent or an export is active", () => {
  const app = source("../src/App.tsx");
  const buttons = [...app.matchAll(/<button type="button" className="secondary" disabled=\{!selectedPhotoDimensions \|\| photoExportBusy\}/g)];
  assert.equal(buttons.length, 3);
  assert.doesNotMatch(app, /disabled=\{!selectedPhotoDimensions\}\s*disabled=\{photoExportBusy\}/);
});
