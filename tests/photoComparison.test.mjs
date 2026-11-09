import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = (path) => readFileSync(new URL(path, import.meta.url), "utf8");

test("Original comparison draws immutable source with neutral edits through the bounded renderer", () => {
  const preview = source("../src/PhotoCanvasPreview.tsx");
  assert.match(preview, /showOriginal\s*\? paintAdjustedPhoto\(decoded\.bitmap, \{ brightness: 100, contrast: 100, rotation: 0 \}, canvas, 1400\)/);
  assert.match(preview, /: paintAdjustedPhoto\(decoded\.bitmap, \{ brightness, contrast, saturation, rotation, straightenTenths, crop, flipHorizontal, flipVertical \}, canvas, 1400\)/);
  assert.match(preview, /setShowOriginal\(false\)/);
  assert.match(preview, /flipHorizontal, flipVertical, showOriginal\]/);
  assert.doesNotMatch(preview, /canvas\.toBlob|createObjectURL|localStorage|sessionStorage|invoke\(/);
});

test("Comparison is a transient view, not photo history, project state or export", () => {
  const preview = source("../src/PhotoCanvasPreview.tsx");
  const app = source("../src/App.tsx");
  assert.match(preview, /const \[showOriginal, setShowOriginal\] = useState\(false\)/);
  assert.match(preview, /const canCompare = decoded\?\.sourceUrl === sourceUrl/);
  assert.match(preview, /\{canCompare && <div className="photo-comparison-toolbar">/);
  assert.match(preview, /aria-pressed=\{showOriginal\}/);
  assert.match(preview, /aria-label="Show unedited original"/);
  assert.match(preview, /setReady\(false\); setShowOriginal\(\(current\) => !current\)/);
  assert.match(app, /renderAdjustedPhoto\(selected\.url, \{ brightness, contrast, saturation, rotation, straightenTenths, crop, flipHorizontal, flipVertical \}\)/);
  assert.doesNotMatch(app, /showOriginal/);
});

test("Compare remains keyboard-focusable during redraw and does not sacrifice viewport pan", () => {
  const component = source("../src/PhotoCanvasPreview.tsx");
  const css = source("../src/styles.css");
  assert.match(component, /const shown = ready && canCompare/);
  assert.match(component, /onPointerDown=\{beginDrag\}/);
  assert.match(component, /new ResizeObserver\(update\)/);
  assert.match(component, /aria-live="polite"/);
  assert.match(css, /\.photo-compare-toggle:focus-visible/);
  assert.match(css, /\.photo-compare-toggle\[aria-pressed="true"\]/);
});
