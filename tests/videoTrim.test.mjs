import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = (path) => readFileSync(new URL(path, import.meta.url), "utf8");

test("The WebView never sends a file path or command line for a video export", () => {
  const trim = source("../src/videoTrim.ts");
  const panel = source("../src/VideoTrimPanel.tsx");
  // Only typed identifiers, marker hints and the confirmed frame indices cross IPC.
  assert.match(trim, /request: \{ assetId: asset\.id, name: asset\.name, size: asset\.size, lastModifiedMs: asset\.lastModifiedMs \}/);
  assert.match(trim, /invoke<JobStatus>\("video_trim_start", \{\s*request: \{ \.\.\.request, acceptPicturesExport: true \},/);
  assert.match(trim, /invoke\("video_export_reveal", \{ jobId: job\.jobId \}\)/);
  // No path field, argument list, shell or asset-protocol URL anywhere in the export UI.
  assert.doesNotMatch(trim + panel, /\bpath\b\s*:|\bargs\b\s*:|argv|shell|convertFileSrc/i);
});

test("Native IPC types reject unknown fields and keep the dialog seam out of release builds", () => {
  const rust = source("../src-tauri/src/video_trim.rs");
  const lib = source("../src-tauri/src/lib.rs");
  assert.equal((rust.match(/#\[serde\(deny_unknown_fields, rename_all = "camelCase"\)\]/g) ?? []).length, 3);
  assert.match(lib, /#\[cfg\(debug_assertions\)\]\s*fn qa_picked_video\(name: &str\)/);
  assert.match(lib, /#\[cfg\(not\(debug_assertions\)\)\]\s*fn qa_picked_video\(_name: &str\) -> Option<Option<std::path::PathBuf>> \{\s*None\s*\}/);
  for (const command of ["video_source_grant", "video_source_revoke", "video_trim_resolve", "video_trim_start",
    "video_trim_status", "video_trim_cancel", "video_trim_jobs", "video_export_reveal"]) {
    assert.match(lib, new RegExp(`^\\s+${command},\\r?$`, "m"), command);
  }
  assert.match(lib, /video_trim::reconcile\(&roots\)/);
  // Never stream copy for a trim, never a shell.
  assert.doesNotMatch(rust, /"-c", "copy"|"-c:v", "copy"|cmd \/C ffmpeg|Command::new\("cmd"\)\s*\.args\(\[\s*"\/C",\s*"ffmpeg/);
  assert.match(rust, /"-fps_mode",\s*"passthrough"/);
  assert.match(rust, /ProcessTree::adopt\(&child\)/);
  assert.match(rust, /fs::hard_link\(&partial_video, &video_path\)/);
});

test("Export asks for destination consent and explains audio before anything is written", () => {
  const panel = source("../src/VideoTrimPanel.tsx");
  assert.match(panel, /window\.confirm\(/);
  assert.match(panel, /Pictures\\\\Optic Rift, which may sync with OneDrive/);
  assert.match(panel, /Export declined\. Nothing was written\./);
  assert.match(panel, /No file was chosen\. Nothing was exported\./);
  assert.match(panel, /audioText\(plan\.audio\)/);
  assert.match(source("../src/videoTrim.ts"), /re-encoded to AAC/);
  assert.match(panel, /there are no audio editing tools/);
});

test("A running export stays visible and cancellable on every destination", () => {
  const app = source("../src/App.tsx");
  const trim = source("../src/videoTrim.ts");
  assert.match(app, /const videoExport = useVideoExport\(\);/);
  assert.match(app, /<div className="top-actions"><VideoExportChip exporter=\{videoExport\}/);
  assert.match(trim, /invoke<JobStatus \| null>\("video_trim_status", \{ jobId: id \}\)/);
  assert.match(source("../src/VideoTrimPanel.tsx"), /Cancel export/);
  // Distinct from model-download status.
  assert.match(source("../src/VideoTrimPanel.tsx"), /Video export · /);
});

test("A notice about one clip is cleared when another clip is selected", () => {
  assert.match(source("../src/VideoTrimPanel.tsx"), /useEffect\(\(\) => \{ setNotice\(""\); \}, \[clip\?\.id\]\);/);
});

test("Removing a clip from the session withdraws its export permission", () => {
  const app = source("../src/App.tsx");
  assert.match(app, /const grant = videoGrants\[id\];\s*if \(grant\) \{\s*void revokeSource\(grant\.grantId\);/);
  assert.match(app, /lastModifiedMs: file\.lastModified/);
});
