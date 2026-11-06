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
});
