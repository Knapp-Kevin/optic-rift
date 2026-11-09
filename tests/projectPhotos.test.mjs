import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  PROJECT_PHOTO_RECIPE_VERSION, recipeFromEdits, editsFromRecipe,
  restoredPngBlob, projectPhotosClient,
} from "../src/projectPhotos.ts";

const source = path => readFileSync(new URL(path, import.meta.url), "utf8");
const edits = {
  brightness: 116, contrast: 91, saturation: 133, rotation: 90,
  straightenTenths: -75, resizeLongEdge: 512,
  crop: { left: 50, right: 25, top: 75, bottom: 100 },
  flipHorizontal: true, flipVertical: false,
};

test("An independently specified v1 recipe round-trips all implemented editing operations", () => {
  const recipe = recipeFromEdits(edits);
  assert.equal(recipe.schema_version, PROJECT_PHOTO_RECIPE_VERSION);
  assert.equal(recipe.straighten_tenths, -75);
  assert.equal(recipe.resize_long_edge, 512);
  assert.deepEqual(editsFromRecipe(recipe), edits);
  assert.notStrictEqual(recipe.crop, edits.crop);
  const neutral = recipeFromEdits({ brightness: 100, contrast: 100, rotation: 0 });
  assert.equal(neutral.saturation, 100);
  assert.equal(neutral.straighten_tenths, 0);
  assert.equal(neutral.resize_long_edge, null);
  assert.deepEqual(editsFromRecipe(neutral), {
    brightness: 100, contrast: 100, saturation: 100, rotation: 0,
    straightenTenths: 0, flipHorizontal: false, flipVertical: false,
  });
});

test("Unsupported recipe schemas, invalid values and forged flags are refused", () => {
  const good = recipeFromEdits(edits);
  assert.throws(() => editsFromRecipe({ ...good, schema_version: 9 }), /unsupported/);
  assert.throws(() => editsFromRecipe({ ...good, brightness: 300 }));
  assert.throws(() => editsFromRecipe({ ...good, flip_horizontal: "true" }), /invalid/);
  assert.throws(() => editsFromRecipe({ ...good, resize_long_edge: -1 }), /invalid|range|unsupported|settings/i);
  assert.throws(() => recipeFromEdits({ ...edits, straightenTenths: 250 }));
});

test("Restoring a project PNG is bounded and returns a fresh in-memory PNG blob", async () => {
  const tiny = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScL/nwAAAABJRU5ErkJggg==";
  const blob = restoredPngBlob(tiny);
  assert.equal(blob.type, "image/png");
  assert.ok(blob.size > 16);
  assert.deepEqual([...new Uint8Array(await blob.arrayBuffer())].slice(0,8),
    [137,80,78,71,13,10,26,10]);
  assert.throws(() => restoredPngBlob("!!!"));
  assert.throws(() => restoredPngBlob("A".repeat(45 * 1024 * 1024)));
});

test("Project photo API passes project IDs, asset IDs and errors without inventing success", async () => {
  const calls = [];
  const invoke = async (name,args) => {
    calls.push({ name,args });
    if (name === "project_photo_list") return { photos: [], problems: [] };
    if (name === "project_photo_open") throw "The project original has changed";
    throw new Error("Unexpected command");
  };
  const client = projectPhotosClient(invoke);
  assert.deepEqual(await client.list("project-1"), {
    ok: true, value: { photos: [], problems: [] },
  });
  assert.deepEqual(calls[0], { name: "project_photo_list", args: { projectId:"project-1" } });
  assert.deepEqual(await client.open("project-1","asset-1"), {
    ok: false, error: "The project original has changed",
  });
  assert.deepEqual(calls[1], {
    name: "project_photo_open", args: { projectId: "project-1", assetId: "asset-1" },
  });
});

test("Native project photo implementation owns file authority and refuses source tampering", () => {
  const native = source("../src-tauri/src/project_photos.rs");
  const ipc = source("../src-tauri/src/lib.rs");
  const app = source("../src/App.tsx");
  const library = source("../src/SessionMediaLibrary.tsx");
  const production = native.split("#[cfg(test)]")[0];
  assert.match(native, /static SAVE_LOCK: Mutex<\(\)>/);
  assert.match(native, /fn checked_png\(/);
  assert.match(native, /source_digest\(&bytes\) != doc\.source_sha256/);
  assert.match(native, /expected_revision/);
  assert.match(native, /create_new\(true\)/);
  assert.match(native, /fs::rename\(&staging, &final_dir\)/);
  assert.match(native, /if existing != bytes/);
  assert.match(native, /Project photo file is not regular/);
  assert.match(native, /pub fn open\(/);
  assert.doesNotMatch(production, /std::process::Command|std::process::Stdio|http::/);
  assert.match(ipc, /project_photo_save,/);
  assert.match(ipc, /project_photo_list,/);
  assert.match(ipc, /project_photo_open,/);
  assert.match(app, /savePhotoToProject\(/);
  assert.match(app, /openProjectPhoto\(/);
  assert.match(library, /Reopen editable photo/);
  assert.match(app, /activeProjectRef\.current !== targetProject\.id/);
  assert.match(app, /restoredPngBlob\(/);
  assert.doesNotMatch(app, /localStorage\.setItem/);
});
