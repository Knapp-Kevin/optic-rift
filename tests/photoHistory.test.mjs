import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  MAX_PHOTO_UNDO, createPhotoHistory, applyPhotoEdit,
  finishPhotoGesture, undoPhotoEdit, redoPhotoEdit,
} from "../src/photoHistory.ts";

test("undo and redo restore exact non-destructive photo edit values", () => {
  const initial = createPhotoHistory();
  const tone = applyPhotoEdit(initial, { brightness: 126 });
  const rotated = applyPhotoEdit(tone, { rotation: 90 });
  const cropped = applyPhotoEdit(rotated, { crop: { left: 1250, top: 0, right: 0, bottom: 0 } });
  assert.deepEqual(undoPhotoEdit(cropped).present, rotated.present);
  assert.deepEqual(undoPhotoEdit(undoPhotoEdit(cropped)).present, tone.present);
  assert.deepEqual(redoPhotoEdit(undoPhotoEdit(cropped)).present, cropped.present);
  assert.deepEqual(initial.present, { brightness: 100, contrast: 100, saturation: 100, rotation: 0 });
});

test("multiple slider updates in one drag create exactly one undo step", () => {
  const initial = createPhotoHistory();
  const a = applyPhotoEdit(initial, { brightness: 110 }, "brightness");
  const b = applyPhotoEdit(a, { brightness: 125 }, "brightness");
  const c = applyPhotoEdit(b, { brightness: 140 }, "brightness");
  assert.equal(c.past.length, 1);
  assert.equal(undoPhotoEdit(c).present.brightness, 100);
  const nextDrag = applyPhotoEdit(finishPhotoGesture(c), { brightness: 115 }, "brightness");
  assert.equal(nextDrag.past.length, 2);
  assert.equal(undoPhotoEdit(nextDrag).present.brightness, 140);
});

test("redo is invalidated when editing after undo", () => {
  const a = applyPhotoEdit(createPhotoHistory(), { contrast: 120 });
  const b = applyPhotoEdit(a, { rotation: 180 });
  const back = undoPhotoEdit(b);
  assert.equal(back.future.length, 1);
  const branch = applyPhotoEdit(back, { brightness: 115 });
  assert.equal(branch.future.length, 0);
  assert.strictEqual(redoPhotoEdit(branch), branch);
});

test("crop snapshots never share mutable nested state", () => {
  const crop = { left: 500, right: 250, top: 100, bottom: 0 };
  const updated = applyPhotoEdit(createPhotoHistory(), { crop });
  crop.left = 999;
  assert.equal(updated.present.crop.left, 500);
  const latest = applyPhotoEdit(updated, { contrast: 130 });
  assert.notStrictEqual(latest.past[1].crop, updated.present.crop);
  assert.equal(undoPhotoEdit(latest).present.crop.left, 500);
});

test("unchanged edits do not push history and invalid edits do not corrupt it", () => {
  const initial = createPhotoHistory();
  assert.strictEqual(applyPhotoEdit(initial, { brightness: 100 }), initial);
  assert.strictEqual(applyPhotoEdit(initial, { crop: { left: 0, right: 0, top: 0, bottom: 0 } }), initial);
  assert.throws(() => applyPhotoEdit(initial, { rotation: 45 }));
  assert.throws(() => applyPhotoEdit(initial, { contrast: Infinity }));
  assert.deepEqual(initial.past, []);
  assert.strictEqual(undoPhotoEdit(initial), initial);
  assert.strictEqual(redoPhotoEdit(initial), initial);
});

test("history is bounded and advances after the oldest record expires", () => {
  let state = createPhotoHistory();
  for (let n = 0; n < MAX_PHOTO_UNDO + 12; n++) {
    state = applyPhotoEdit(state, { brightness: n % 2 ? 100 : 101 });
  }
  assert.equal(state.past.length, MAX_PHOTO_UNDO);
  for (let n = 0; n < MAX_PHOTO_UNDO; n++) state = undoPhotoEdit(state);
  assert.equal(state.past.length, 0);
  assert.equal(state.future.length, MAX_PHOTO_UNDO);
});

test("Photo Studio exposes functional undo redo reset and explicit session-only status", () => {
  const app = readFileSync(new URL("../src/App.tsx", import.meta.url), "utf8");
  assert.match(app, /photoHistories\[selected\.id\]/);
  assert.match(app, /stepPhotoHistory\("undo"\)/);
  assert.match(app, /stepPhotoHistory\("redo"\)/);
  assert.match(app, /onPointerUp=\{finishPhotoSlider\}/);
  assert.match(app, /window\.addEventListener\("keydown", onKeyDown\)/);
  assert.match(app, /These edits are not saved in your project/);
  assert.match(app, /delete next\[id\]/);
  assert.match(app, /await renderAdjustedPhoto\(selected\.url, \{ brightness, contrast, saturation, rotation, crop, flipHorizontal, flipVertical \}\)/);
  assert.doesNotMatch(app, /localStorage\.setItem|sessionStorage\.setItem/);
});

test("Photo Studio history controls have disabled states and keyboard focus safeguards", () => {
  const app = readFileSync(new URL("../src/App.tsx", import.meta.url), "utf8");
  assert.match(app, /!editHistory\?\.past\.length/);
  assert.match(app, /!editHistory\?\.future\.length/);
  assert.match(app, /event\.isComposing/);
  assert.match(app, /element\.isContentEditable/);
  assert.match(app, /photoExportBusy\) return/);
});


test("Crop gestures coalesce by edge, but not across different edges or pointer releases", () => {
  const first = applyPhotoEdit(createPhotoHistory(), {
    crop: { left: 250, right: 0, top: 0, bottom: 0 },
  }, "crop-left");
  const second = applyPhotoEdit(first, {
    crop: { left: 1500, right: 0, top: 0, bottom: 0 },
  }, "crop-left");
  assert.equal(second.past.length, 1);
  assert.deepEqual(undoPhotoEdit(second).present.crop, undefined);
  const differentEdge = applyPhotoEdit(second, {
    crop: { left: 1500, right: 500, top: 0, bottom: 0 },
  }, "crop-right");
  assert.equal(differentEdge.past.length, 2);
  assert.equal(undoPhotoEdit(differentEdge).present.crop.left, 1500);
  assert.equal(undoPhotoEdit(differentEdge).present.crop.right, 0);
  const released = applyPhotoEdit(finishPhotoGesture(differentEdge), {
    crop: { left: 1500, right: 750, top: 0, bottom: 0 },
  }, "crop-right");
  assert.equal(released.past.length, 3);
});


test("Flips are independent, undoable, and reset invalidates redo", () => {
  const source = createPhotoHistory();
  const horizontal = applyPhotoEdit(source, {flipHorizontal: true});
  const both = applyPhotoEdit(horizontal, {flipVertical: true});
  assert.equal(both.present.flipHorizontal, true);
  assert.equal(both.present.flipVertical, true);
  assert.equal(undoPhotoEdit(both).present.flipVertical, undefined);
  assert.equal(undoPhotoEdit(both).present.flipHorizontal, true);
  assert.equal(redoPhotoEdit(undoPhotoEdit(both)).present.flipVertical, true);
  const restored = applyPhotoEdit(both, {flipHorizontal: false, flipVertical: false});
  assert.equal(restored.present.flipHorizontal, false);
  assert.equal(restored.present.flipVertical, false);
  assert.equal(undoPhotoEdit(restored).present.flipHorizontal, true);
  assert.throws(() => applyPhotoEdit(source, {flipHorizontal: "yes"}));
});

