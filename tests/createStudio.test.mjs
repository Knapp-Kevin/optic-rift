import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { hasQualifiedLocalImageModel } from "../src/models.ts";

test("Create default follows native qualified-execution decision", () => {
  assert.equal(hasQualifiedLocalImageModel({ qualified_image_ready: true }), true);
  for (const value of [false, null, undefined, "true"]) {
    assert.equal(hasQualifiedLocalImageModel({ qualified_image_ready: value }), false);
  }
});

test("Create and Edit remain independently navigable with a bounded preview", () => {
  const app = readFileSync(new URL("../src/App.tsx", import.meta.url), "utf8");
  const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
  assert.match(app, /className="studio-tabs"/);
  assert.match(app, /setStudioMode\("create"\)/);
  assert.match(app, /setStudioMode\("edit"\)/);
  assert.match(app, /layout="create"/);
  assert.match(css, /\.create-preview img\{[^}]*object-fit:contain/);
  assert.match(css, /\.canvas video,\.canvas img\{[^}]*max-height:100%/);
  assert.match(css, /\.create-settings\{[^}]*overflow-y:auto/);
});

test("Native image thumbnails never skip output/provenance validation or persist copies", () => {
  const native = readFileSync(new URL("../src-tauri/src/generation.rs", import.meta.url), "utf8");
  const commands = readFileSync(new URL("../src-tauri/src/model_commands.rs", import.meta.url), "utf8");
  const registry = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");
  assert.match(native, /pub fn output_thumbnail_png\([\s\S]*?output_png\(app_local, job_id\)/);
  assert.match(native, /const EDGE: u32 = 256/);
  assert.match(native, /const MAX_DIM: u32 = 1536/);
  assert.match(commands, /pub async fn generation_output_thumbnail/);
  assert.match(registry, /model_commands::generation_output_thumbnail/);
  assert.doesNotMatch(native.slice(native.indexOf("fn thumbnail_png("), native.indexOf("/// Export a validated image")), /fs::write|fs::File::create|OpenOptions/);
});

test("History does not render sensitive thumbnails or select source by default", () => {
  const history = readFileSync(new URL("../src/GenerationFilmstrip.tsx", import.meta.url), "utf8");
  const engine = readFileSync(new URL("../src/GeneratePanel.tsx", import.meta.url), "utf8");
  assert.match(history, /previewsRevealed\s*\?\s*<GenerationThumbnail/);
  assert.match(history, /useState\(false\)/);
  assert.match(history, /Show previews/);
  assert.doesNotMatch(history, /item\.prompt/);
  assert.match(engine, /item\.id\.slice\(0, 8\)\} · \{item\.width\}/);
  assert.match(engine, /layout === "create" \? "" : valid\[0\]/);
  assert.match(engine, /className="create-progress"/);
  assert.match(engine, /aria-live="polite"/);
});

test("Create survives Edit media switches and re-conceals previews while hidden", () => {
  const app = readFileSync(new URL("../src/App.tsx", import.meta.url), "utf8");
  const panel = readFileSync(new URL("../src/GeneratePanel.tsx", import.meta.url), "utf8");
  const strip = readFileSync(new URL("../src/GenerationFilmstrip.tsx", import.meta.url), "utf8");
  // Hidden rather than unmounted, so a running job keeps its progress, Cancel and result.
  assert.match(app, /createOpened && <main className="create-workspace" hidden=\{!\(view === "studio" && studioMode === "create"\)\}/);
  assert.match(app, /visible=\{view === "studio" && studioMode === "create"\}/);
  assert.match(panel, /active=\{visible\}/);
  assert.match(strip, /if \(!active\) setPreviewsRevealed\(false\)/);
});

test("GPU consent takes keyboard focus and an empty prompt is not shown as an error", () => {
  const panel = readFileSync(new URL("../src/GeneratePanel.tsx", import.meta.url), "utf8");
  assert.match(panel, /role="alertdialog" aria-modal="true"/);
  assert.match(panel, /autoFocus onClick=\{\(\) => setConfirm\(null\)\}>Not now/);
  assert.match(panel, /e\.key === "Escape"\) setConfirm\(null\)/);
  assert.match(panel, /\{problem && prompt\.trim\(\) \? <small className="project-error"/);
});

test("Filmstrip sizes to its tiles and letterboxes thumbnails instead of cropping", () => {
  const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
  assert.match(css, /\.create-panel\{grid-template-rows:minmax\(160px,1fr\) auto auto\}/);
  assert.match(css, /\.create-filmstrip-heading \.model-actions\{flex-wrap:nowrap;flex:0 0 auto;flex-direction:row/);
  assert.match(css, /\.generation-thumbnail\{display:flex;align-items:center;justify-content:center\}/);
  assert.match(css, /\.generation-thumbnail img\{width:auto;height:auto;max-width:100%;max-height:100%;object-fit:contain\}/);
});

test("Image-to-image never displays an unchosen history item as the selected source", () => {
  const panel = readFileSync(new URL("../src/GeneratePanel.tsx", import.meta.url), "utf8");
  const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
  assert.match(panel, /<option value="">\{sources\.length \? "Choose a starting image…" : "No saved generations yet"\}<\/option>/);
  assert.match(css, /\.create-settings input\[type=range\]\{width:100%;margin:0;/);
});

test("Edit media accounts for the Studio tab bar instead of scrolling the page", () => {
  const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
  assert.match(css, /\.studio-tabs~\.workspace\{min-height:calc\(100vh - 120px\)\}/);
});

test("GPU consent dialog scrolls inside a short stage instead of being clipped", () => {
  const css = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
  assert.match(css, /\.create-gpu-consent\{max-height:calc\(100% - 16px\);overflow-y:auto\}/);
});

test("Cancel is tracked natively before the worker knows the job", () => {
  const native = readFileSync(new URL("../src-tauri/src/generation.rs", import.meta.url), "utf8");
  const run = native.slice(native.indexOf("fn run_job<"), native.indexOf("pub fn cancel("));
  // Checked before verification, after the multi-GB hash check, and right before the job is sent.
  assert.ok((run.match(/check_not_cancelled\(&req\.job_id\)\?/g) ?? []).length >= 3);
  assert.match(native, /forget_cancel\(&req\.job_id\)/);
});

test("The progress card is not drawn over the empty-stage welcome copy", () => {
  const panel = readFileSync(new URL("../src/GeneratePanel.tsx", import.meta.url), "utf8");
  assert.match(panel, /\{active \? <img src=\{active\.uri\} alt="Generated image" \/> : jobId \? null : <div className="create-welcome">/);
});
