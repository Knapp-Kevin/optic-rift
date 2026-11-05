import assert from "node:assert/strict";
import test from "node:test";
import { formatSize } from "../src/mediaFormat.ts";

test("sizes never round small files down to zero", () => {
  assert.equal(formatSize(13), "13 B");
  assert.equal(formatSize(27526), "26.9 KB");
  assert.equal(formatSize(4104714), "3.9 MB");
  assert.equal(formatSize(3 * 1024 ** 3), "3.00 GB");
  assert.equal(formatSize(-1), "unknown size");
});
