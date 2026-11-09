import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import test from "node:test";

test("WebView favicon references the single approved Optic Rift artwork", () => {
  const html = readFileSync(new URL("../index.html", import.meta.url), "utf8");
  const relativePath = "branding/optic-rift-approved-256.webp";
  assert.match(html, /<link rel="icon" type="image\/webp" href="\/branding\/optic-rift-approved-256\.webp" sizes="256x256"\s*\/>/);
  assert.doesNotMatch(html, /vite\.svg|favicon\.ico|shortcut icon/);
  const approved = readFileSync(new URL("../public/" + relativePath, import.meta.url));
  // Canonical SHA from assets/branding/README.md, not a newly generated logo.
  assert.equal(createHash("sha256").update(approved).digest("hex"),
    "dae5767d888bff06870957f975f562324dde5ed83467e6ed23bbb71f642d927c");
  // Format marker: actual WebP/RIFF, not just a renamed image.
  assert.equal(approved.subarray(0, 4).toString("ascii"), "RIFF");
  assert.equal(approved.subarray(8, 12).toString("ascii"), "WEBP");
});
