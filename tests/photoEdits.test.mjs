import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  MAX_PHOTO_EDGE, MAX_PHOTO_PIXELS, photoExportDimensions, photoPaintDimensions, probePhotoDimensions, validatePhotoAdjustments,
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

test("PNG JPEG and WebP header preflight rejects decompression bombs and animation", () => {
  const png = new Uint8Array(24);
  png.set([137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82]);
  const set32 = (bytes, at, n) => {
    bytes[at] = (n >>> 24) & 255; bytes[at + 1] = (n >>> 16) & 255;
    bytes[at + 2] = (n >>> 8) & 255; bytes[at + 3] = n & 255;
  };
  set32(png, 16, 4000); set32(png, 20, 3000);
  assert.deepEqual(probePhotoDimensions(png, "image/png"), { width: 4000, height: 3000 });
  set32(png, 16, 5000); set32(png, 20, 5000);
  assert.throws(() => probePhotoDimensions(png, "image/png"));
  assert.throws(() => probePhotoDimensions(png.subarray(0, 18), "image/png"));

  const jpeg = Uint8Array.from([
    255, 216, 255, 192, 0, 17, 8, 1, 44, 2, 88, 3,
    1, 17, 0, 2, 17, 0, 3, 17, 0,
  ]);
  assert.deepEqual(probePhotoDimensions(jpeg, "image/jpeg"), { width: 600, height: 300 });
  assert.throws(() => probePhotoDimensions(new Uint8Array([255, 216, 255, 218]), "image/jpeg"));

  const webp = new Uint8Array(30);
  webp.set([82, 73, 70, 70], 0); webp.set([87, 69, 66, 80, 86, 80, 56, 88], 8);
  webp[16] = 10;
  webp[24] = 231; webp[25] = 3; // 1000 px after adding 1
  webp[27] = 243; webp[28] = 1; // 500 px after adding 1
  assert.deepEqual(probePhotoDimensions(webp, "image/webp"), { width: 1000, height: 500 });
  webp[20] = 2;
  assert.throws(() => probePhotoDimensions(webp, "image/webp"));
  assert.throws(() => probePhotoDimensions(png, "image/svg+xml"));
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
  // Production code only: the Windows junction test legitimately runs `mklink`.
  const native = here("../src-tauri/src/photo_export.rs").split("#[cfg(test)]")[0];
  const ipc = here("../src-tauri/src/lib.rs");
  assert.match(edit, /sourceUrl\.startsWith\("blob:"\)/);
  assert.match(edit, /canvas\.toBlob/);
  assert.match(edit, /ctx\.filter =/);
  assert.match(edit, /crypto\.subtle\.digest\("SHA-256"/);
  assert.match(native, /if !r\.accept_pictures_export/);
  assert.match(native, /fn decode_verified_png/);
  assert.match(native, /reader\s*\.next_frame/);
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

test("Photo preview and export share the same renderer, not separate CSS filters", () => {
  const app = readFileSync(new URL("../src/App.tsx", import.meta.url), "utf8");
  const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
  const edits = readFileSync(new URL("../src/photoEdits.ts", import.meta.url), "utf8");
  const component = readFileSync(new URL("../src/PhotoCanvasPreview.tsx", import.meta.url), "utf8");
  assert.match(app, /<PhotoCanvasPreview/);
  assert.match(component, /paintAdjustedPhoto\(decoded\.bitmap/);
  assert.match(edits, /paintAdjustedPhoto\(bitmap, edits, canvas\)/);
  assert.match(css, /\.canvas canvas\.photo-preview-canvas\{[^}]*object-fit:contain/);
  assert.doesNotMatch(app, /style=\{\{ filter: "brightness\("/);
  assert.match(edits, /Only PNG, JPEG and WebP photos can be exported\. This file type is not supported\./);
});

test("Photo preview proxy is bounded, preserves the correct orientation and never upscales", () => {
  assert.deepEqual(photoPaintDimensions(4000, 3000, 90, 1400),
    { width: 1050, height: 1400, drawWidth: 1400, drawHeight: 1050 });
  assert.deepEqual(photoPaintDimensions(4000, 3000, 270, 1400),
    { width: 1050, height: 1400, drawWidth: 1400, drawHeight: 1050 });
  assert.deepEqual(photoPaintDimensions(800, 600, 180, 1400),
    { width: 800, height: 600, drawWidth: 800, drawHeight: 600 });
  assert.deepEqual(photoPaintDimensions(3000, 4000, 0),
    { width: 3000, height: 4000, drawWidth: 3000, drawHeight: 4000 });
  assert.throws(() => photoPaintDimensions(4000, 3000, 0, 3000));
  assert.throws(() => photoPaintDimensions(4000, 3000, 0, 50));
  assert.throws(() => photoPaintDimensions(5000, 5000, 0, 1400));
});

test("Edit media stays within the viewport on desktop; its columns scroll on their own", () => {
  const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
  assert.match(css, /@media\(min-width:1201px\)\{\s*\.studio-tabs~\.workspace\{flex:none;height:calc\(100vh - 120px\);min-height:0\}/);
  assert.match(css, /\.studio-tabs~\.workspace>\.editor-inspector\{min-height:0;overflow-y:auto;overscroll-behavior:contain\}/);
});

test("Photo preview decodes once per asset and never persists private derivatives", () => {
  const component = readFileSync(new URL("../src/PhotoCanvasPreview.tsx", import.meta.url), "utf8");
  const app = readFileSync(new URL("../src/App.tsx", import.meta.url), "utf8");
  assert.match(component, /loadPhotoBitmap\(sourceUrl\)/);
  assert.match(component, /\}, \[sourceUrl\]\)/);
  assert.match(component, /setTimeout\(\(\) => \{/);
  assert.match(component, /paintAdjustedPhoto\(decoded\.bitmap/);
  assert.match(component, /ownedBitmap\?\.close\(\)/);
  assert.match(component, /if \(!active\) \{\s*bitmap\.close\(\)/);
  assert.doesNotMatch(component, /toBlob|createObjectURL|localStorage|sessionStorage|invoke\(/);
  assert.match(app, /mimeType: file\.type/);
  assert.match(app, /mimeType: "image\/png"/);
  assert.match(app, /selected\?\.kind === "image" && !editablePhoto && !previewFailed/);
  assert.match(app, /Preview only\. PNG, JPEG and WebP are supported for editing and export/);
});

test("Photos the canvas editor refuses stay viewable read-only instead of becoming unpreviewable", () => {
  const app = readFileSync(new URL("../src/App.tsx", import.meta.url), "utf8");
  assert.match(app, /onError=\{\(reason\) => setReadOnlyPhotos\(\(previous\) => new Map\(previous\)\.set\(selected\.id, reason\)\)\}/);
  assert.match(app, /const editablePhoto = selected\?\.kind === "image" && !readOnlyReason &&/);
  assert.doesNotMatch(app, /<PhotoCanvasPreview[^>]*onError=\{\(reason\) => markFailed/);
  assert.match(app, /Preview only: \$\{readOnlyReason\}/);
});
