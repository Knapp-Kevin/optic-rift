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

test("Switching adult discovery clears flagged results without stopping native downloads", () => {
  const browser = source("src/ModelBrowser.tsx");
  const app = source("src/App.tsx");
  assert.match(browser, /useEffect\(\(\) => \{\s*searchVersion\.current\+\+;\s*inspectVersion\.current\+\+;/);
  assert.match(browser, /\}, \[includeAdult\]\)/);
  assert.match(browser, /const visibleModels = \(result\?\.models \?\? \[\]\)\.filter/);
  assert.match(browser, /includeAdult \|\| model\.audience !== "not-for-all-audiences"/);
  assert.match(browser, /inspectionFromLocalLibrary/);
  assert.match(browser, /version === searchVersion\.current/);
  assert.match(browser, /version === inspectVersion\.current/);
  assert.match(browser, /<InstalledPanel library=\{library\}/);
  assert.doesNotMatch(app, /<ModelBrowser key=/);
});

test("Model downloads are owned app-wide, not by the Models page (issue #27)", () => {
  const app = source("src/App.tsx");
  const browser = source("src/ModelBrowser.tsx");
  const downloads = source("src/downloads.tsx");
  const native = source("src-tauri/src/model_store.rs");
  // Ownership above the route-scoped browser; the browser only renders it.
  assert.match(app, /<DownloadProvider onInstalled=/);
  assert.ok(app.indexOf("<DownloadProvider") < app.indexOf('{view === "models"'), "provider wraps every page");
  assert.match(browser, /const \{ session: installSession, error: installError, start \} = useDownloads\(\);/);
  assert.doesNotMatch(browser, /listen<InstallEvent>|model_install"|useState<InstallSession/);
  // Exactly one native progress subscription, filtered to the live install id.
  assert.equal((downloads.match(/listen<InstallEvent>\("model-install"/g) ?? []).length, 1);
  assert.match(downloads, /e\.payload\.install_id !== s\.installId\) return s;/);
  // Cancel uses the exact live id from a ref, and an unaccepted cancel triggers native reconciliation.
  assert.match(downloads, /const id = s\.installId;/);
  assert.match(downloads, /invoke<boolean>\("model_install_cancel", \{ installId: id \}\)/);
  assert.match(downloads, /if \(!accepted\) void reconcile\(\);/);
  assert.match(downloads, /invoke<NativeStatus \| null>\("model_install_status"\)/);
  // One status display per page; a flagged name is hidden while discovery is off.
  assert.match(app, /\{view !== "models" && <DownloadStatus includeAdult=\{includeAdultModels\} compact \/>\}/);
  assert.match(browser, /\{!statusInDetail && <DownloadStatus includeAdult=\{includeAdult\} \/>\}/);
  assert.match(source("src/downloadState.ts"), /return !includeAdult && item\.flagged !== false;/);
  // No persisted download history.
  assert.doesNotMatch(downloads, /localStorage|sessionStorage|indexedDB/);
  // Native: one download at a time, claimed before inspection, exact-id cancel, released by a guard.
  assert.match(native, /let \(_guard, flag\) = claim\(&req\)\?;/);
  assert.ok(native.indexOf("claim(&req)?") < native.indexOf("hf_catalog::inspect(&req.repo"), "slot claimed before network work");
  assert.match(native, /\.filter\(\|a\| a\.install_id == install_id\)/);
});
