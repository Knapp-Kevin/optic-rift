import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { hasQualifiedLocalImageModel } from "../src/models.ts";

const VALID_REV = "60987f37e94cd59c36b1cba832b9f97b57395a10";
const library = () => ({
  runtime: { installed: { runtime: "diffusers-v1" } },
  gpu: { vram_total_mb: 16384 },
  installed: [{ files_present: true, manifest: {
    repo: "segmind/SSD-1B", revision: VALID_REV, adapter: "diffusers-sdxl-text-to-image",
  } }],
});

test("Create default requires qualified local model and executable runtime", () => {
  assert.equal(hasQualifiedLocalImageModel(library()), true);
  const unsupported = library();
  unsupported.installed[0].manifest.repo = "other/video-only";
  assert.equal(hasQualifiedLocalImageModel(unsupported), false);
  const incomplete = library();
  incomplete.installed[0].files_present = false;
  assert.equal(hasQualifiedLocalImageModel(incomplete), false);
  const unqualified = library();
  unqualified.installed[0].manifest.revision = "f".repeat(40);
  assert.equal(hasQualifiedLocalImageModel(unqualified), false);
  const noGpu = library();
  noGpu.gpu.vram_total_mb = null;
  assert.equal(hasQualifiedLocalImageModel(noGpu), false);
  const noRuntime = library();
  noRuntime.runtime.installed = null;
  assert.equal(hasQualifiedLocalImageModel(noRuntime), false);
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
