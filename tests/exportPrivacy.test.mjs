import assert from "node:assert/strict";
import test from "node:test";
import { PICTURES_EXPORT_WARNING, saveWithPicturesConsent } from "../src/exportPrivacy.ts";

test("Pictures export discloses OneDrive/cloud sync and provenance before copying", () => {
  for (const expected of ["Pictures", "OneDrive", "cloud", "prompt", "provenance", "Cancel"]) {
    assert.ok(PICTURES_EXPORT_WARNING.includes(expected), expected);
  }
});

test("declining Pictures save causes no native write", async () => {
  let writes = 0;
  const result = await saveWithPicturesConsent("job-id", () => false, async () => {
    writes++; return "unreachable";
  });
  assert.equal(result, null);
  assert.equal(writes, 0);
});

test("approval copies exactly once and propagates native failures", async () => {
  const calls = [];
  const consented = await saveWithPicturesConsent("job-id", (notice) => {
    assert.equal(notice, PICTURES_EXPORT_WARNING);
    return true;
  }, async (id) => { calls.push(id); return "C:\\Users\\Example\\Pictures\\Optic Rift\\out.png"; });
  assert.match(consented, /out\.png$/);
  assert.deepEqual(calls, ["job-id"]);
  await assert.rejects(
    saveWithPicturesConsent("job-id", () => true, async () => { throw new Error("disk full"); }),
    /disk full/,
  );
});
