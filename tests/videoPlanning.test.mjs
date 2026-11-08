import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  MAX_SESSION_VIDEO_SHOTS, VIDEO_GENERATION_EXECUTION_QUALIFIED, normalizeVideoShot,
  moveVideoShot, videoShotDuration, videoIntentLabel, videoModeLabel,
} from "../src/videoShotPlan.ts";
import {
  MAX_VIDEO_ENHANCE_NOTES, VIDEO_ENHANCEMENT_EXECUTION_QUALIFIED,
  normalizeVideoEnhancementPlan, videoEnhanceOperations,
} from "../src/videoEnhancementPlan.ts";
import { creativeJourneys, journeyAreas, journeyStateText } from "../src/creativeJourneys.ts";
import { navigation } from "../src/creativeNavigation.ts";

const source = (name) => readFileSync(new URL(name, import.meta.url), "utf8");
const baseShot = () => ({
  mode: "text-to-video", prompt: "A boat gliding across a misty bay",
  referenceId: null, durationSeconds: 4, aspect: "16:9",
  intent: "general", camera: "static",
});

test("Video shot modes have explicit validated source contracts", () => {
  assert.equal(normalizeVideoShot(baseShot(), [], []).prompt, "A boat gliding across a misty bay");
  const photo = { ...baseShot(), mode: "image-to-video", referenceId: "photo1" };
  const clip = { ...baseShot(), mode: "video-to-video", referenceId: "clip1" };
  assert.deepEqual(normalizeVideoShot(photo, ["photo1"], []), photo);
  assert.deepEqual(normalizeVideoShot(clip, [], ["clip1"]), clip);
  for (const invalid of [
    { ...baseShot(), mode: "image-to-video" },
    { ...baseShot(), mode: "image-to-video", referenceId: "missing" },
    { ...baseShot(), mode: "video-to-video", referenceId: "missing" },
    { ...baseShot(), mode: "video-to-video", referenceId: "photo1" },
    { ...baseShot(), durationSeconds: 13 },
    { ...baseShot(), durationSeconds: 2.5 },
    { ...baseShot(), aspect: "2:1" },
    { ...baseShot(), camera: "orbit" },
    { ...baseShot(), prompt: " " },
    { ...baseShot(), prompt: "a".repeat(1201) },
    { ...baseShot(), prompt: "bad\u0000prompt" },
  ]) assert.throws(() => normalizeVideoShot(invalid, ["photo1"], ["clip1"]));
});

test("Content intent is a planning label, not an authorization bypass", () => {
  for (const intent of ["general", "mature-themes", "adult-focused"]) {
    const normalized = normalizeVideoShot({ ...baseShot(), intent }, [], []);
    assert.equal(normalized.intent, intent);
  }
  assert.match(videoIntentLabel["adult-focused"], /18\+/);
  assert.match(videoIntentLabel["mature-themes"], /PG-13-style/);
  assert.equal(VIDEO_GENERATION_EXECUTION_QUALIFIED, false);
  assert.deepEqual(Object.keys(videoModeLabel),
    ["text-to-video", "image-to-video", "video-to-video"]);
});

test("Storyboard keeps its order and duration safely without mutating inputs", () => {
  const shots = [
    { ...baseShot(), id: "a", durationSeconds: 4 },
    { ...baseShot(), id: "b", durationSeconds: 6 },
    { ...baseShot(), id: "c", durationSeconds: 2 },
  ];
  assert.equal(videoShotDuration(shots), 12);
  assert.deepEqual(moveVideoShot(shots, "b", -1).map((x) => x.id), ["b", "a", "c"]);
  assert.deepEqual(moveVideoShot(shots, "c", 1).map((x) => x.id), ["a", "b", "c"]);
  assert.deepEqual(moveVideoShot(shots, "absent", -1).map((x) => x.id), ["a", "b", "c"]);
  assert.deepEqual(shots.map((x) => x.id), ["a", "b", "c"]);
  assert.equal(MAX_SESSION_VIDEO_SHOTS, 24);
});

test("Enhancement plan requires genuine opened local video and supported operations", () => {
  const base = {
    videoId: "clip1", operations: ["denoise", "upscale"],
    target: "1080p", priority: "preserve-detail", notes: " Keep grain natural ",
  };
  assert.equal(VIDEO_ENHANCEMENT_EXECUTION_QUALIFIED, false);
  assert.deepEqual(normalizeVideoEnhancementPlan(base, ["clip1"]),
    { ...base, notes: "Keep grain natural" });
  assert.ok(videoEnhanceOperations.length >= 7);
  assert.throws(() => normalizeVideoEnhancementPlan({ ...base, videoId: "not-open" }, ["clip1"]));
  assert.throws(() => normalizeVideoEnhancementPlan({ ...base, operations: [] }, ["clip1"]));
  assert.throws(() => normalizeVideoEnhancementPlan({ ...base, operations: ["magic-repair"] }, ["clip1"]));
  assert.throws(() => normalizeVideoEnhancementPlan({ ...base, target: "16K" }, ["clip1"]));
  assert.throws(() => normalizeVideoEnhancementPlan({ ...base, notes: "x".repeat(MAX_VIDEO_ENHANCE_NOTES + 1) }, ["clip1"]));
});

test("Journey map includes major photo video filmmaker restoration and privacy outcomes", () => {
  assert.equal(creativeJourneys.length, 25);
  assert.equal(new Set(creativeJourneys.map((x) => x.id)).size, creativeJourneys.length);
  assert.equal(journeyAreas.length, 5);
  for (const area of journeyAreas) assert.ok(creativeJourneys.some((journey) => journey.area === area));
  const titles = creativeJourneys.map((journey) => journey.title.toLowerCase()).join(" | ");
  for (const term of ["short film", "upscale", "stabilize", "restore", "from a prompt",
    "existing video", "photo", "adult", "private", "timeline"]) {
    assert.ok(titles.includes(term), `Missing journey: ${term}`);
  }
  const routes = navigation.flatMap((x) => x.items).map((x) => x.view);
  for (const journey of creativeJourneys) {
    assert.ok(journey.outcome && journey.current && journey.acceptance);
    assert.ok(journey.state in journeyStateText);
    if (journey.route) assert.ok(routes.includes(journey.route), journey.id);
    if (journey.state === "not-implemented") assert.equal(journey.route, null);
  }
});

test("All three product-wide workspaces are real routes, but execution is not invented", () => {
  const app = source("../src/App.tsx");
  const shot = source("../src/VideoShotComposer.tsx");
  const enhance = source("../src/VideoEnhancePlanner.tsx");
  const explorer = source("../src/CreativeWorkflowExplorer.tsx");
  const browser = source("../src/ModelBrowser.tsx");
  assert.match(app, /view === "workflows" && <CreativeWorkflowExplorer/);
  assert.match(app, /videoCreateOpened && <VideoShotComposer/);
  assert.match(app, /videoEnhanceOpened && <VideoEnhancePlanner/);
  assert.match(app, /initialCapability=\{modelBrowseCapability\}/);
  assert.match(browser, /initialCapability = "text-to-image"/);
  assert.match(shot, /hidden=\{!visible\}/);
  assert.match(enhance, /hidden=\{!visible\}/);
  assert.match(shot, /Nothing was generated/);
  assert.match(shot, /VIDEO_GENERATION_EXECUTION_QUALIFIED/);
  assert.match(enhance, /VIDEO_ENHANCEMENT_EXECUTION_QUALIFIED/);
  assert.match(enhance, /No processing or output was started/);
  assert.match(explorer, /End-to-end acceptance/);
  assert.doesNotMatch([shot, enhance, explorer].join("\n"),
    /invoke\(|generation_generate|video_generate\(|exportVideo\(|model_install\(/);
});

test("Video intent remains separate from adult-model discovery and preview reveal", () => {
  const shot = source("../src/VideoShotComposer.tsx");
  const app = source("../src/App.tsx");
  assert.match(shot, /not an age check, official film rating/);
  assert.match(shot, /Adult-model discovery has a separate opt-in/);
  assert.match(app, /includeAdultModels, setIncludeAdultModels/);
  assert.doesNotMatch(shot, /setIncludeAdultModels|includeAdultModels/);
  assert.doesNotMatch(shot, /<img|<video/);
});

test("Navigation and planning state persist across route changes without saving private media", () => {
  const app = source("../src/App.tsx");
  const css = source("../src/styles.css");
  assert.match(app, /setVideoCreateOpened\(true\)/);
  assert.match(app, /setVideoEnhanceOpened\(true\)/);
  assert.match(css, /\.video-compose-page\[hidden\]\{display:none!important\}/);
  assert.match(app, /visible=\{view === "create-video"\}/);
  assert.match(app, /visible=\{view === "enhance-video"\}/);
  assert.doesNotMatch(source("../src/videoShotPlan.ts"), /localStorage|sessionStorage|fetch\(/);
  assert.doesNotMatch(source("../src/videoEnhancementPlan.ts"), /localStorage|sessionStorage|fetch\(/);
});

test("A changed enhancement plan drops both the review and its 'ready' notice", () => {
  const enhance = source("../src/VideoEnhancePlanner.tsx");
  assert.match(enhance, /function invalidateReview\(\) \{\s*setReviewed\(null\);\s*setNotice\(""\);/);
  // Every user edit after a review goes through invalidateReview, never a bare setReviewed(null).
  const handlers = enhance.slice(enhance.indexOf("function toggle("));
  assert.doesNotMatch(handlers.slice(0, handlers.indexOf("function reviewPlan(")), /setReviewed\(null\)/);
  assert.doesNotMatch(enhance.slice(enhance.indexOf("return <main")), /setReviewed\(null\)/);
  assert.equal((enhance.match(/invalidateReview\(\)/g) ?? []).length, 6);
});

test("A shot whose reference was removed says so once, in plain words", () => {
  const shot = source("../src/VideoShotComposer.tsx");
  assert.match(shot, /Reference no longer open\. Edit this shot to choose another\./);
  assert.doesNotMatch(shot, /Reference unavailable; reselect/);
});

test("Journey cards are written in short, plain sentences", () => {
  // Rough Flesch-Kincaid estimate; the runbook asks for about a grade-10 reading level.
  const syllables = (word) => {
    let w = word.toLowerCase().replace(/[^a-z]/g, "");
    if (!w) return 0;
    if (w.length <= 3) return 1;
    w = w.replace(/(?:[^laeiouy]es|ed|[^laeiouy]e)$/, "").replace(/^y/, "");
    return Math.max(1, (w.match(/[aeiouy]{1,2}/g) ?? []).length);
  };
  for (const journey of creativeJourneys) {
    const text = [journey.outcome, journey.current, journey.acceptance].join(" ");
    const sentences = text.split(/[.!?]+/).filter((s) => s.trim().split(/\s+/).length > 1).length;
    const words = text.split(/\s+/).filter((x) => /[a-z]/i.test(x));
    const grade = 0.39 * (words.length / sentences) +
      11.8 * (words.reduce((n, w) => n + syllables(w), 0) / words.length) - 15.59;
    assert.ok(grade <= 10, `${journey.id} reads at about grade ${grade.toFixed(1)}`);
    assert.doesNotMatch(text, /→/, `${journey.id} uses arrow shorthand`);
  }
});
