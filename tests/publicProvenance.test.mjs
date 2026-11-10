import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = (file) => readFileSync(new URL(file, import.meta.url), "utf8");

test("native public disclosure omits private generation fields by construction", () => {
  const rust = source("../src-tauri/src/content_provenance.rs");
  const production = rust.split("#[cfg(test)]")[0];
  assert.match(production, /fn public_generation_receipt/);
  assert.match(production, /"ai_involvement": "ai_generated"/);
  assert.match(production, /"credential_status": "unsigned_unverified_not_c2pa"/);
  assert.match(production, /"model_stages": stages/);
  assert.match(production, /"lineage_status"/);
  assert.match(production, /fn public_model_field/);
  assert.match(production, /MAX_PUBLIC_RECORD_BYTES/);
  assert.match(production, /MAX_STAGES/);
  assert.doesNotMatch(production, /"prompt"\s*:/);
  assert.doesNotMatch(production, /"negative_prompt"\s*:/);
  assert.doesNotMatch(production, /"runtime"\s*:/);
  assert.doesNotMatch(production, /"parameters"\s*:/);
  assert.doesNotMatch(production, /"project_id"\s*:/);
  assert.doesNotMatch(production, /"path"\s*:/);
  assert.doesNotMatch(production, /std::fs|std::process::Command|unsafe\s*\{/);
});

test("public model identifiers reject path-shaped and URL-shaped values", () => {
  const rust = source("../src-tauri/src/content_provenance.rs");
  const production = rust.split("#[cfg(test)]")[0];
  assert.match(production, /let valid_segment =/);
  assert.match(production, /!segment\.starts_with\('\.'\)/);
  assert.match(production, /!segment\.contains\("\.\."\)/);
  assert.match(production, /let mut parts = value\.split\('\/'\)/);
  assert.match(production, /"revision" \| "adapter" \| "adapter_version"/);
  assert.match(rust, /relative_paths_and_unrecognized_identifiers_are_omitted/);
  assert.match(rust, /"Users\/alice\/private"/);
});

test("native Pictures export uses sanitized disclosure not the private source JSON", () => {
  const lib = source("../src-tauri/src/lib.rs");
  const generation = source("../src-tauri/src/generation.rs");
  assert.match(lib, /mod content_provenance;/);
  const save = generation.slice(generation.indexOf("pub fn save_copy("),
    generation.indexOf("pub fn pictures_dir("));
  assert.match(save, /verified_output\(app_local, job_id\)/);
  assert.match(save, /content_provenance::public_generation_receipt/);
  assert.match(save, /serde_json::from_slice::<Provenance>\(&parent_record\)/);
  assert.match(save, /verified_output\(app_local, parent_id\)/);
  assert.ok(save.indexOf("public_generation_receipt") < save.indexOf("fs::create_dir_all"),
    "disclosure preflight must finish before any export files are created");
  assert.match(save, /sidecar\.write_all\(&record\)/);
  assert.doesNotMatch(save, /sidecar\.write_all\(&source_record\)/);
});

test("unsigned and incomplete provenance is never represented as trusted C2PA", () => {
  const rust = source("../src-tauri/src/content_provenance.rs");
  assert.match(rust, /complete = false/);
  assert.match(rust, /visited\.contains\(&parent_id\)/);
  assert.match(rust, /parent\.output\["sha256"\]\.as_str\(\) == Some\(parent_digest\.as_str\(\)\)/);
  assert.match(rust, /"incomplete"/);
  assert.match(rust, /"unsigned_unverified_not_c2pa"/);
  assert.doesNotMatch(rust.split("#[cfg(test)]")[0], /"c2pa_valid": true/);
});

test("Pictures consent discloses redaction limits and cloud sync", () => {
  const notice = source("../src/exportPrivacy.ts");
  assert.match(notice, /Raw prompts are not copied/);
  assert.match(notice, /not signed, encrypted, or a C2PA Content Credential/);
  assert.match(notice, /OneDrive/);
  assert.match(notice, /model identities/);
  assert.match(notice, /local storage/);
});
