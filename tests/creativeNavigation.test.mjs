import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { editorFor, mediaForEditor, navigation, routeTitle } from "../src/creativeNavigation.ts";

const source = (name) => readFileSync(new URL(name, import.meta.url), "utf8");

test("The expanded creative destinations are unique and accessible without audio scope", () => {
  const routes = navigation.flatMap((section) => section.items);
  assert.deepEqual(routes.map((route) => route.view),
    ["projects", "library", "workflows", "create", "create-video", "photo", "video", "enhance-video", "models", "settings"]);
  assert.equal(new Set(routes.map((route) => route.view)).size, routes.length);
  for (const route of routes) {
    assert.ok(route.label && route.description && route.symbol);
    assert.equal(typeof routeTitle[route.view], "string");
  }
  assert.equal(routes.find((route) => route.view === "photo")?.label, "Photo Studio");
  assert.equal(routes.find((route) => route.view === "video")?.label, "Video Studio");
  assert.equal(routes.find((route) => route.view === "create-video")?.label, "Create Videos");
  assert.equal(routes.find((route) => route.view === "enhance-video")?.label, "Enhance Video");
  assert.equal(routes.find((route) => route.view === "workflows")?.label, "Workflows");
  assert.doesNotMatch(routes.map((route) => route.label).join(" "), /audio|music|soundtrack/i);
});

test("Opening a file chooses the correct editor without mixing photo and video state", () => {
  const photo = { id: "p", name: "safe-photo.png", kind: "image", mimeType: "image/png", url: "blob:p", size: 4 };
  const video = { id: "v", name: "clip.mp4", kind: "video", mimeType: "video/mp4", url: "blob:v", size: 10 };
  assert.equal(editorFor("image"), "photo");
  assert.equal(editorFor("video"), "video");
  assert.deepEqual(mediaForEditor([photo, video], "photo"), [photo]);
  assert.deepEqual(mediaForEditor([photo, video], "video"), [video]);
  assert.deepEqual(mediaForEditor([photo, video], "library"), []);
  assert.deepEqual(mediaForEditor([photo, video], "create"), []);
});

test("Sidebar navigation renders each destination as a real route, not a dead link", () => {
  const app = source("../src/App.tsx");
  const css = source("../src/styles.css");
  assert.match(app, /<nav className="primary-navigation" aria-label="Primary navigation"/);
  assert.match(app, /navigation\.map\(\(section\)/);
  assert.match(app, /onClick=\{\(\) => setView\(item\.view\)\}/);
  assert.match(app, /aria-current=\{view === item\.view \? "page" : undefined\}/);
  // Below 1200 px the visible label is hidden, so the accessible name must not fall back to the tooltip.
  assert.match(app, /aria-label=\{item\.label\} title=\{item\.description\}/);
  assert.match(app, /\{routeTitle\[view\]\}/);
  assert.match(css, /\.primary-navigation\{flex:1;min-height:0;overflow-y:auto/);
  assert.match(css, /\.navigation \.nav \.nav-symbol\{display:inline-flex\}/);
});

test("Download status and Cancel stay on every destination, including the icon-only rail", () => {
  const app = source("../src/App.tsx");
  const css = source("../src/styles.css");
  assert.match(app, /<DownloadProvider onInstalled=/);
  assert.match(app, /\{view !== "models" && <DownloadStatus includeAdult=\{includeAdultModels\} compact \/>\}/);
  // Below 1200 px the card floats beside the rail; the sticky rail must stack above the page or the card is hidden.
  assert.match(css, /@media \(max-width:1200px\)\{\s*\.navigation\{z-index:20\}\s*\.navigation \.download-status\.compact\{position:fixed;/);
});

test("Media Library works without claiming persisted project assets or silently rendering previews", () => {
  const app = source("../src/App.tsx");
  const library = source("../src/SessionMediaLibrary.tsx");
  assert.match(app, /view === "library" && <SessionMediaLibrary/);
  assert.match(library, /Project media persistence is not implemented|project does not yet\s*store imported media/);
  assert.match(library, /Opened media matches|No opened media matches/);
  assert.match(library, /onImport\}/);
  assert.match(app, /notification=\{message\} onDismissNotification=\{\(\) => setMessage\(""\)\}/);
  assert.match(library, /role="status"/);
  assert.match(library, /aria-label="Dismiss media notification"/);
  assert.match(library, /onOpen\(item\)/);
  assert.match(library, /onRemove\(item\.id\)/);
  assert.match(library, /<GenerationHistory refreshVersion=\{historyVersion\}/);
  assert.doesNotMatch(library, /<img|<video|autoplay|\.thumbnail\(/i);
});

test("Create stays alive while changing pages; generation and history open Photo Studio", () => {
  const app = source("../src/App.tsx");
  assert.match(app, /createOpened && <main className="create-workspace" hidden=\{view !== "create"\}/);
  assert.match(app, /visible=\{view === "create"\}/);
  assert.match(app, /onResult=\{addGenerated\}/);
  assert.match(app, /function addGenerated\([\s\S]*?setSelectedPhotoId\(item\.id\);\s*setView\("photo"\)/);
  assert.match(app, /setView\("library"\);\s*\}\} \/>/);
  // The result action names the destination that exists in this navigation.
  assert.match(source("../src/GeneratePanel.tsx"), />Open in Photo Studio<\/button>/);
  assert.doesNotMatch(source("../src/GeneratePanel.tsx"), /Open in Edit media/);
  assert.doesNotMatch(app, /setStudioMode|studioModeChosen/);
});

test("Editor navigation preserves per-asset photo edits and separate video selection", () => {
  const app = source("../src/App.tsx");
  assert.match(app, /const \[selectedPhotoId, setSelectedPhotoId\]/);
  assert.match(app, /const \[selectedVideoId, setSelectedVideoId\]/);
  assert.match(app, /const \[photoAdjustments, setPhotoAdjustments\]/);
  // Since the crop work, updatePhotoAdjustments captures the photo id in a local `id`.
  assert.match(app, /\[id\]: \{ \.\.\.\(previous\[id\] \?\? \{ brightness: 100, contrast: 100, rotation: 0 \}\), \.\.\.changes \}/);
  assert.match(app, /editorItems\.map\(\(item\)/);
  assert.match(app, /mediaForEditor\(items, view\)/);
});

test("Video Studio saves single-range trims but never promises timeline editing or audio work", () => {
  const app = source("../src/App.tsx");
  assert.match(app, /view === "video" && <div className="timeline-panel"/);
  assert.match(app, /Markers set the range for Save trimmed video/);
  assert.match(app, /<VideoTrimPanel /);
  // Marks set in 0.1 s steps must be distinguishable: labels show tenths after rounding away float drift.
  assert.match(app, /const tenths = Math\.round\(/);
  assert.match(app, /s\.toFixed\(1\)\.padStart\(4, "0"\)/);
  assert.match(app, /Video timeline editing is not yet available/);
  assert.match(app, /Capture frame to Photo Studio/);
  assert.match(app, /Clip assembly, transitions and multi-clip export still need qualified native implementation/);
  assert.match(app, /view === "photo" && editablePhoto && !previewFailed/);
  assert.match(app, /Save adjusted PNG copy/);
  assert.doesNotMatch(app, /exportVideo\(|generateAudio\(|mixSoundtrack\(/);
});
