import assert from "node:assert/strict";
import test from "node:test";
import { imageToImageStepError, localImageSourceChoices } from "../src/imageToImage.ts";

const id = "b4f80767-4d71-44a7-a1cf-3bd189ee06d1";
const valid = (overrides = {}) => ({
  job_id: id,
  prompt: "  A quiet blue lake at dawn  ",
  output: { width: 512, height: 768, sha256: "a".repeat(64) },
  ...overrides,
});

test("only well-formed local generation records become source choices", () => {
  assert.deepEqual(localImageSourceChoices([valid()]), [{
    id, label: "A quiet blue lake at dawn", width: 512, height: 768,
  }]);
  assert.deepEqual(localImageSourceChoices(null), []);
  assert.deepEqual(localImageSourceChoices({ records: [valid()] }), []);
});

test("malformed image metadata cannot crash the source picker", () => {
  const rejected = [
    null, "not-an-object", { job_id: id },
    valid({ job_id: "../../outside" }),
    valid({ job_id: id.toUpperCase() }),
    valid({ output: { width: 512, height: 768, sha256: "invalid" } }),
    valid({ output: { width: Infinity, height: 512, sha256: "a".repeat(64) } }),
    valid({ output: { width: 1027, height: 512, sha256: "a".repeat(64) } }),
    valid({ output: { width: 1536, height: 2048, sha256: "a".repeat(64) } }),
  ];
  assert.deepEqual(localImageSourceChoices(rejected), []);
  assert.equal(localImageSourceChoices([valid({ prompt: { injected: "not string" } })])[0].label, "Untitled");
});

test("source choices are bounded to 20 without mutating history", () => {
  const records = Array.from({ length: 50 }, (_, i) =>
    valid({ job_id: `b4f80767-4d71-44a7-a1cf-${i.toString(16).padStart(12, "0")}` }));
  const choices = localImageSourceChoices(records);
  assert.equal(choices.length, 20);
  assert.equal(records.length, 50);
  assert.equal(choices[0].id, records[0].job_id);
});

test("SDXL image-to-image needs at least one actual denoising step", () => {
  assert.match(imageToImageStepError("image-to-image", 1, 0.65), /denoising step/);
  assert.match(imageToImageStepError("image-to-image", 10, 0.05), /denoising step/);
  assert.equal(imageToImageStepError("image-to-image", 20, 0.05), null);
  assert.equal(imageToImageStepError("image-to-image", 25, 0.65), null);
  assert.equal(imageToImageStepError("text-to-image", 1, 0.05), null);
});
