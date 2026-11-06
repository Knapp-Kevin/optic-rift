import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  MAX_PHOTO_EDGE, MAX_PHOTO_PIXELS, photoExportDimensions, validatePhotoAdjustments,
} from "../src/photoEdits.ts";

const here = (file) => readFileSync(new URL(file, import.meta.url), "utf8");

test("Photo export preserves dimensions for 0/180 and swaps 90/270 degrees", () => {
  assert.deepEqual(photoExportDimensions(4000, 3000, 0), { width: 4000, height: 3000 });
  assert.deepEqual(photoExportDimensions(4000, 3000, 180), { width: 4000, height: 3000 });
  assert.deepEqual(photoExportDimensions(4000, 3000, 90), { width: 3000, height: 4000 });
  assert.deepEqual(photoExportDimensions(4000, 3000, 270), { width: 3000, height: 4000 });
  assert.deepEqual(photoExportDimensions(1024, 1024, 90), { width: 1024, height: 1024 });
});

test("Photo export enforces bounded memory, integer dimensions and quarter-turns", () => {
  const reject = [
    [0, 100, 0], [1, 0, 90], [-1, 2, 180], [MAX_PHOTO_EDGE + 1, 1, 0],
    [5000, 5000, 270], [30, 30, 45], [1.5, 20, 0], [1, 1, NaN],
  ];
  for (const [w, h, r] of reject) {
    assert.throws(() => photoExportDimensions(w, h, r));
  }
  assert.ok(4000 * 4000 <= MAX_PHOTO_PIXELS);
  assert.ok(photoExportDimensions(4000, 4000, 90).width === 4000);
});

test("Photo edit values match the displayed inspector range", () => {
  const base = { brightness: 25, contrast: 175, rotation: 270 };
  assert.deepEqual(validatePhotoAdjustments(base), base);
  for (const value of [
    { ...base, brightness: 24 }, { ...base, contrast: 176 },
    { ...base, rotation: 45 }, { ...base, brightness: 25.2 },
    { ...base, contrast: Infinity }, { ...base, rotation: -90 },
  ]) assert.throws(() => validatePhotoAdjustments(value));
});

test("Source photo cannot trigger remote fetch or an arbitrary filesystem destination", () => {
  const edit = here("../src/photoEdits.ts");
  const native = here("../src-tauri/src/photo_export.rs");
  const ipc = here("../src-tauri/src/lib.rs");
  assert.match(edit, /sourceUrl\.startsWith\("blob:"\)/);
  assert.match(edit, /canvas\.toBlob/);
  assert.match(edit, /ctx\.filter =/);
  assert.match(edit, /crypto\.subtle\.digest\("SHA-256"/);
  assert.match(native, /if !r\.accept_pictures_export/);
  assert.match(native, /fn decode_verified_png/);
  assert.match(native, /reader\.next_frame/);
  assert.match(native, /fs::hard_link\(&stage_png, &png_path\)/);
  assert.match(native, /fs::hard_link\(&stage_receipt, &receipt_path\)/);
  assert.match(native, /source_sha256_webview_reported/);
  assert.doesNotMatch(native, /std::process::Command|trust_remote_code|fs::rename/);
  assert.match(ipc, /photo_export_copy,/);
});

test("Photo Studio offers a real save action but honestly labels current limits", () => {
  const app = here("../src/App.tsx");
  assert.match(app, /await renderAdjustedPhoto\(selected\.url/);
  assert.match(app, /invoke<\{ path: string; sha256: string; width: number; height: number \}>\("photo_export_copy"/);
  assert.match(app, /Save adjusted PNG copy/);
  assert.match(app, /OneDrive/);
  assert.match(app, /not save an asset into this project/);
  assert.doesNotMatch(app, /Exporting adjusted images is not implemented yet/);
});
