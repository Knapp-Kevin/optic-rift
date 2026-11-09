import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  captureFrameTime, capturedFrameName, captureVideoFrame, validateFrameDimensions,
  validateFrameSource, MAX_CAPTURE_PNG_BYTES,
} from "../src/videoFrameCapture.ts";

const source = (path) => readFileSync(new URL(path, import.meta.url), "utf8");

function sampleVideo(overrides = {}) {
  const video = {
    currentSrc: "blob:http://127.0.0.1:1420/sample-video",
    seeking: false,
    readyState: 4,
    error: null,
    currentTime: 12.345,
    duration: 90,
    videoWidth: 640,
    videoHeight: 360,
    paused: false,
    pause() { this.paused = true; },
  };
  return Object.assign(video, overrides);
}

function fakeCanvas({ blob = new Blob(["test-png"], { type: "image/png" }), failDraw = false } = {}) {
  const calls = [];
  const canvas = {
    width: 0, height: 0,
    getContext(kind, options) {
      calls.push(["context", kind, options]);
      return { drawImage(...args) {
        if (failDraw) throw new Error("Protected source");
        calls.push(["draw", ...args]);
      } };
    },
    toBlob(callback, mime) {
      calls.push(["encode", mime]);
      callback(blob);
    },
  };
  return { canvas, calls };
}

async function withCanvas(mock, task) {
  const previous = globalThis.document;
  globalThis.document = { createElement(tag) {
    assert.equal(tag, "canvas");
    return mock.canvas;
  } };
  try {
    return await task();
  } finally {
    if (previous === undefined) delete globalThis.document;
    else globalThis.document = previous;
  }
}

test("frame dimensions and timestamps reject unbounded and non-finite input", () => {
  assert.deepEqual(validateFrameDimensions(1920, 1080), { width: 1920, height: 1080 });
  assert.deepEqual(validateFrameDimensions(4000, 5000), { width: 4000, height: 5000 });
  for (const [w, h] of [
    [0, 100], [100, -1], [8193, 100], [5000, 5000],
    [1.5, 400], [Infinity, 720], [NaN, 2],
  ]) assert.throws(() => validateFrameDimensions(w, h));
  assert.equal(captureFrameTime(12.3454), 12.345);
  assert.equal(captureFrameTime(12.3456), 12.346);
  for (const value of [NaN, Infinity, -1]) assert.throws(() => captureFrameTime(value));
  assert.equal(capturedFrameName(12.345), "video-frame-00-00-12-345.png");
  assert.equal(capturedFrameName(3671.2), "video-frame-01-01-11-200.png");
});

test("capture demands the actual local selected frame, not a stale, remote or seeking video", () => {
  const url = "blob:http://127.0.0.1:1420/sample-video";
  assert.deepEqual(validateFrameSource(sampleVideo(), url),
    { width: 640, height: 360, timeSeconds: 12.345 });
  for (const [video, requested] of [
    [sampleVideo(), "https://example.invalid/video.mp4"],
    [sampleVideo({ currentSrc: "https://example.invalid/movie.mp4" }), url],
    [sampleVideo({ currentSrc: "blob:other" }), url],
    [sampleVideo({ seeking: true }), url],
    [sampleVideo({ readyState: 1 }), url],
    [sampleVideo({ error: { code: 3 } }), url],
    [sampleVideo({ currentTime: NaN }), url],
    [sampleVideo({ videoWidth: 9000, videoHeight: 100 }), url],
    [sampleVideo({ currentTime: 100, duration: 90 }), url],
  ]) assert.throws(() => validateFrameSource(video, requested));
});

test("real capture renders the selected paused frame to a bounded in-memory PNG", async () => {
  const video = sampleVideo();
  const mock = fakeCanvas();
  const result = await withCanvas(mock, () => captureVideoFrame(video, video.currentSrc));
  assert.equal(video.paused, true);
  assert.equal(result.width, 640);
  assert.equal(result.height, 360);
  assert.equal(result.timeSeconds, 12.345);
  assert.equal(result.blob.type, "image/png");
  assert.equal(mock.canvas.width, 640);
  assert.equal(mock.canvas.height, 360);
  assert.deepEqual(mock.calls.map((c) => c[0]), ["context", "draw", "encode"]);
  assert.equal(mock.calls[1][1], video);
  assert.deepEqual(mock.calls[1].slice(2), [0, 0, 640, 360]);
});

test("canvas failures, invalid encodings and oversized PNGs do not yield an asset", async () => {
  const video = sampleVideo();
  for (const [options, message] of [
    [{ failDraw: true }, /could not be read/],
    [{ blob: null }, /Could not encode this frame as a PNG/],
    [{ blob: new Blob(["jpeg"], { type: "image/jpeg" }) }, /valid PNG/],
    [{ blob: new Blob([], { type: "image/png" }) }, /valid PNG/],
    [{ blob: { type: "image/png", size: MAX_CAPTURE_PNG_BYTES + 1 } }, /exceeds the supported 32 MB limit\.$/],
  ]) {
    const mock = fakeCanvas(options);
    await assert.rejects(withCanvas(mock, () => captureVideoFrame(video, video.currentSrc)), message);
  }
});

test("Video → Photo UI requires explicit selection, carries session lineage, and never writes automatically", () => {
  const app = source("../src/App.tsx");
  const nav = source("../src/creativeNavigation.ts");
  const library = source("../src/SessionMediaLibrary.tsx");
  const renderer = source("../src/videoFrameCapture.ts");
  assert.match(app, /async function captureCurrentVideoFrame\(\)/);
  assert.match(app, /captureVideoFrame\(video, source\.url\)/);
  assert.match(app, /if \(videoRef\.current !== video\) return/);
  assert.match(app, /frameCaptureBusyRef\.current/);
  assert.match(app, /setSelectedPhotoId\(item\.id\);\s*setView\("photo"\)/);
  assert.match(app, /Capture frame to Photo Studio/);
  assert.match(app, /disabled=\{!isVideo \|\| !videoFrameReady \|\| frameCaptureBusy\}/);
  assert.match(app, /onSeeking=\{\(\) => setVideoFrameReady\(false\)\}/);
  assert.match(app, /derivedFrom: \{\s*kind: "video-frame"/);
  assert.match(nav, /derivedFrom\?: \{/);
  assert.match(library, /Not saved as project provenance/);
  assert.doesNotMatch(renderer, /fetch\(|invoke\(|fs\.write|localStorage|sessionStorage|createObjectURL\(/);
  assert.doesNotMatch(app.slice(app.indexOf("async function captureCurrentVideoFrame()"),
    app.indexOf("function seek(to: number)")), /photo_export_copy|generation_generate|model_install/);
});
