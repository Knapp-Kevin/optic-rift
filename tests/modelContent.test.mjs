import assert from "node:assert/strict";
import test from "node:test";
import { allowAutomaticPreview, displayAudience, listInDiscovery } from "../src/modelContent.ts";

const adult = { audience: "adult-focused", evidence: "reviewed-model-card" };
const mature = { audience: "mature", evidence: "developer-review" };
const general = { audience: "general", evidence: "developer-review" };
const unknown = { audience: "unverified", evidence: "unknown" };
const suspicious = { audience: "general", evidence: "unknown" };

test("adult-focused models are hidden by default and included on deliberate opt-in", () => {
  assert.equal(listInDiscovery(adult, false), false);
  assert.equal(listInDiscovery(adult, true), true);
  assert.equal(listInDiscovery(general, false), true);
  assert.equal(listInDiscovery(unknown, false), true);
});
test("adult discovery never implicitly authorizes automatic explicit previews", () => {
  assert.equal(allowAutomaticPreview(adult), false);
  assert.equal(allowAutomaticPreview(mature), false);
  assert.equal(allowAutomaticPreview(unknown), false);
  assert.equal(allowAutomaticPreview(suspicious), false);
  assert.equal(allowAutomaticPreview(general), true);
});
test("audience labels distinguish adult, mature, and unsupported safety claims", () => {
  assert.equal(displayAudience(adult), "Adult-focused (18+) · Explicit");
  assert.equal(displayAudience(mature), "Mature themes");
  assert.equal(displayAudience(unknown), "Content classification unverified");
  assert.equal(displayAudience(suspicious), "Content classification unverified");
});
