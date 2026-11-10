import assert from "node:assert/strict";
import test from "node:test";
import { parsePublicProvenance } from "../src/publicProvenance.ts";

const idA = "1a1a1a1a-aaaa-4aaa-8aaa-111111111111";
const idB = "2b2b2b2b-bbbb-4bbb-8bbb-222222222222";
const hashA = "a".repeat(64);
const hashB = "b".repeat(64);
const stage = (job_id, output_sha256, model_repo) => ({
  job_id, output_sha256, model_repo, model_revision: "rev-1",
  adapter: "local", adapter_version: "1.0",
});
const receipt = () => ({
  record_version: 2, kind: "optic-rift-ai-generation-disclosure",
  ai_involvement: "ai_generated", assertion_origin: "optic_rift_local_unsigned_record",
  credential_status: "unsigned_unverified_not_c2pa",
  lineage_status: "complete_within_available_local_history",
  stage_order: "earliest_to_latest",
  model_stages: [stage(idA, hashA, "studio/ModelA"), stage(idB, hashB, "studio/ModelB")],
  output: { sha256: hashB, width: 512, height: 512, bytes: 1024 },
});

test("two valid model stages remain readable and explicitly unsigned", () => {
  const parsed = parsePublicProvenance(receipt());
  assert.equal(parsed.status, "unsigned_unverified_not_c2pa");
  assert.equal(parsed.involvement, "ai_generated");
  assert.equal(parsed.assertionOrigin, "optic_rift_local_unsigned_record");
  assert.equal(parsed.stages.length, 2);
  assert.equal(parsed.stages[0].modelRepo, "studio/ModelA");
  assert.equal(parsed.stages[1].modelRevision, "rev-1");
  assert.equal(parsed.outputSha256, hashB);
});

test("missing stages and incomplete ancestry are shown without inventing an issuer", () => {
  const incomplete = { ...receipt(), lineage_status: "incomplete" };
  const parsed = parsePublicProvenance(incomplete);
  assert.equal(parsed.lineage, "incomplete");
  for (const bad of [
    { ...receipt(), credential_status: "trusted_c2pa" },
    { ...receipt(), ai_involvement: "human_created" },
    { ...receipt(), assertion_origin: "verified_human_creator" },
    { ...receipt(), assertion_origin: "external_cawg_identity" },
    { ...receipt(), assertion_origin: undefined },
    { ...receipt(), model_stages: [] },
    { ...receipt(), model_stages: Array.from({ length: 17 }, () => stage(idA, hashA, "studio/X")) },
    { ...receipt(), output: { sha256: hashA } },
  ]) {
    assert.throws(() => parsePublicProvenance(bad));
  }
});

test("reject forged stage paths and malformed identifiers", () => {
  for (const badName of ["Users/alice/private", "../model", "studio//model",
    "https://secret@example.com/model", "studio/model/version", "studio/.hidden"]) {
    const value = receipt();
    value.model_stages[0].model_repo = badName;
    assert.throws(() => parsePublicProvenance(value), /model disclosure/i);
  }
  const value = receipt();
  value.model_stages[0].adapter = "C:/private";
  assert.throws(() => parsePublicProvenance(value), /path-shaped/);
});

test("artist, identity and rights assertions in a raw record never become verified metadata", () => {
  const value = receipt();
  value.artist_name = "Person claiming credit";
  value.creator_verified = true;
  value.rights_verified = true;
  value.artistic_integrity_score = 100;
  value.model_stages[0].identity = { type: "CAWG", verified: true };
  const parsed = parsePublicProvenance(value);
  for (const unsubstantiated of ["Person claiming credit", "creator_verified", "rights_verified",
    "artistic_integrity_score", "CAWG"]) {
    assert.ok(!JSON.stringify(parsed).includes(unsubstantiated));
  }
  assert.equal(parsed.assertionOrigin, "optic_rift_local_unsigned_record");
  assert.equal(parsed.status, "unsigned_unverified_not_c2pa");
});

test("optional model IDs may be null, while private fields never survive parsing", () => {
  const value = receipt();
  value.model_stages[0].model_repo = null;
  value.model_stages[0].model_revision = null;
  value.prompt = "PRIVATE PROMPT";
  value.model_stages[1].token = "PRIVATE TOKEN";
  const parsed = parsePublicProvenance(value);
  assert.equal(parsed.stages[0].modelRepo, null);
  assert.ok(!JSON.stringify(parsed).includes("PRIVATE PROMPT"));
  assert.ok(!JSON.stringify(parsed).includes("PRIVATE TOKEN"));
});
