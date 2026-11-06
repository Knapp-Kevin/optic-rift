import assert from "node:assert/strict";
import test from "node:test";
import { compatibilityText, licenseAcknowledgement, licenseText, modelStates, settingsError } from "../src/models.ts";

const plan = { plan: { adapter: "diffusers-sdxl-text-to-image", files: [], total_bytes: 4468829801, variant: "fp16" }, adapter_label: "SDXL", verified: null };
const detail = (over = {}) => ({
  id: "segmind/SSD-1B", revision: "60987f37e94cd59c36b1cba832b9f97b57395a10", pipeline_tag: "text-to-image", capability: "text-to-image",
  license: "apache-2.0", license_name: null, license_link: null, base_models: null, gated: "no", access: "public", audience: "unverified",
  format: "diffusers", repo_files: 27, repo_bytes: 1, plans: [plan], rejections: [], page_url: "https://huggingface.co/segmind/SSD-1B", ...over,
});
const gpu = { name: "RTX", driver: "1", vram_total_mb: 16303, vram_used_mb: 1, vram_free_mb: 10000 };
const installed = { manifest: { repo: "segmind/SSD-1B", revision: "60987f37e94cd59c36b1cba832b9f97b57395a10" }, dir: "x", files_present: true };

test("states are independent and earned", () => {
  const discoveredOnly = modelStates({ detail: detail({ plans: [], rejections: [{ adapter: "a", reason: "Pipeline is FluxPipeline" }] }), installed: null, gpu, runtime_installed: true });
  assert.equal(discoveredOnly.discovered.reached, true);
  assert.equal(discoveredOnly.supported.reached, false);
  assert.match(discoveredOnly.supported.detail, /FluxPipeline/);
  assert.equal(discoveredOnly.ready.reached, false);

  const supported = modelStates({ detail: detail(), installed: null, gpu, runtime_installed: true });
  assert.deepEqual([supported.supported.reached, supported.verified.reached, supported.installed.reached, supported.ready.reached], [true, false, false, false]);

  const noRuntime = modelStates({ detail: detail(), installed, gpu, runtime_installed: false });
  assert.equal(noRuntime.installed.reached, true);
  assert.equal(noRuntime.ready.reached, false);
  assert.match(noRuntime.ready.detail, /runtime/);

  const noGpu = modelStates({ detail: detail(), installed, gpu: { ...gpu, vram_total_mb: null }, runtime_installed: true });
  assert.equal(noGpu.ready.reached, false);

  const ready = modelStates({ detail: detail(), installed, gpu, runtime_installed: true }, ["segmind/SSD-1B", "60987f37e94cd59c36b1cba832b9f97b57395a10"]);
  assert.equal(ready.ready.reached, true);
  assert.match(ready.ready.detail, /Loaded/);
  assert.equal(ready.verified.reached, false, "installed and ready never imply verified");

  const incomplete = modelStates({ detail: detail(), installed: { ...installed, files_present: false }, gpu, runtime_installed: true });
  assert.equal(incomplete.installed.reached, false);
});

test("licenses are shown verbatim and unknown is never permissive", () => {
  assert.equal(licenseText({ license: "other", license_name: "flux-1-dev-non-commercial-license" }), "other (flux-1-dev-non-commercial-license)");
  assert.equal(licenseText({ license: null, license_name: null }), "unknown");
  assert.equal(licenseAcknowledgement({ license: null }), "unknown");
});

test("compatibility explanations", () => {
  assert.match(compatibilityText({ kind: "no-adapter" }), /No Optic Rift adapter/);
  assert.equal(compatibilityText({ kind: "unsupported-format", detail: "GGUF" }), "GGUF");
});

test("generation settings mirror worker limits", () => {
  const ok = { width: 1024, height: 1024, steps: 25, guidance: 7, seed: null };
  assert.equal(settingsError("a lighthouse", ok), null);
  assert.match(settingsError("  ", ok), /Describe/);
  assert.match(settingsError("x", { ...ok, width: 1000 + 4 }), /multiple of 8/);
  assert.match(settingsError("x", { ...ok, steps: 0 }), /Steps/);
  assert.match(settingsError("x", { ...ok, guidance: 30 }), /Guidance/);
  assert.match(settingsError("x", { ...ok, seed: -1 }), /Seed/);
  assert.equal(settingsError("x", { ...ok, seed: 4294967295 }), null);
});
