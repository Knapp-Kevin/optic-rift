import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const root = new URL("../", import.meta.url);
const source = (p) => readFileSync(new URL(p, root), "utf8");

test("Video task families remain discoverable without advertising execution", () => {
  const adapters = source("src-tauri/src/adapters.rs");
  for (const tag of ["text-to-video", "image-to-video", "video-to-video"]) {
    assert.match(adapters, new RegExp('pipeline_tag", "' + tag + '"'));
  }
  assert.doesNotMatch(adapters.slice(adapters.indexOf("pub const REGISTRY"), adapters.indexOf("pub fn adapter(")), /Capability::TextToVideo|Capability::ImageToVideo|Capability::VideoToVideo/);
});

test("Video cache native admission is explicit, hash-bound and anonymous", () => {
  const native = source("src-tauri/src/video_metadata_cache.rs");
  const catalog = source("src-tauri/src/hf_catalog.rs");
  const ipc = source("src-tauri/src/model_commands.rs");
  const commands = source("src-tauri/src/lib.rs");
  assert.match(native, /if !request\.accept/);
  assert.match(native, /if detail\.gated != "no"/);
  assert.match(native, /!request\.adult_acknowledged/);
  assert.match(native, /request\.license_acknowledged != license/);
  assert.match(native, /hf_catalog::inspect_public/);
  assert.match(native, /reqwest::redirect::Policy::none\(\)/);
  assert.match(native, /verify_download\(&data, file\)/);
  assert.match(native, /fs::hard_link\(&temp, &destination\)/);
  assert.match(native, /existing_cache_folder\(app_local, &detail\.id, &detail\.revision\)/);
  assert.match(catalog, /fn token_retry_allowed/);
  assert.match(catalog, /inspect_with_token_policy\(id, revision, capability, false\)/);
  assert.match(ipc, /pub async fn video_cache_metadata/);
  assert.match(ipc, /pub async fn video_cache_clear/);
  assert.match(commands, /model_commands::video_cache_metadata/);
  assert.match(commands, /model_commands::video_cache_clear/);
  const prod = native.slice(0, native.indexOf("#[cfg(test)]"));
  assert.doesNotMatch(prod, /fs::remove_dir_all/);
  assert.doesNotMatch(prod, /trust_remote_code|\.safetensors|Command::new/);
});

test("Video metadata controls never claim model weights or video inference", () => {
  const browser = source("src/ModelBrowser.tsx");
  const types = source("src/models.ts");
  assert.match(browser, /function VideoMetadataCache/);
  assert.match(browser, /Cache configuration metadata only/);
  assert.match(browser, /No weights were downloaded/);
  assert.match(browser, /adult_checked|adultChecked/);
  assert.match(browser, /video_cache_clear/);
  assert.match(types, /cache_metadata_file: RepoFile \| null/);
  assert.match(types, /cached_metadata: boolean/);
});

test("Video cache UI does not offer caching when the source's declared task differs", () => {
  const browser = readFileSync(new URL("../src/ModelBrowser.tsx", import.meta.url), "utf8");
  assert.match(browser, /const taskMatches = detail\.pipeline_tag === detail\.capability;/);
  assert.match(browser, /const canCache = Boolean\(file && ungated && taskMatches && !cached\);/);
});

test("Config download follows only the Hub's own resolve-cache redirect", () => {
  const native = readFileSync(new URL("../src-tauri/src/video_metadata_cache.rs", import.meta.url), "utf8");
  assert.match(native, /redirect\(reqwest::redirect::Policy::none\(\)\)/);
  assert.match(native, /let expected = format!\("\/api\/resolve-cache\/models\/\{repo\}\/\{revision\}\/\{file\}"\);/);
  assert.match(native, /target\.host_str\(\) == base\.host_str\(\)/);
});

test("Switching adult discovery opt-in invalidates the prior model browser", () => {
  const app = source("src/App.tsx");
  // A keyed remount discards stale catalog results and in-flight selection state
  // before React can paint an opted-out view with previously flagged listings.
  assert.match(app, /<ModelBrowser key=\{includeAdultModels \? "adult-discovery-on" : "adult-discovery-off"\}/);
  assert.match(app, /setIncludeAdultModels\(false\)/);
  assert.match(app, /setIncludeAdultModels\(allowed\)/);
});
