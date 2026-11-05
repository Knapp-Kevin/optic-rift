import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

// Measured on Windows 11 26300 with WebView2 156.0.4314.58: without these switches the WebView2
// browser process loads OneAuth/WAM and connects to Microsoft sign-in endpoints at launch, and
// its network service makes a background connection later. With them, a 120 s soak showed no
// TCP or UDP endpoints from the app's process tree. Setting additionalBrowserArgs replaces
// Tauri's defaults, so those defaults must be kept explicitly.
const config = JSON.parse(readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"));
const args = config.app.windows.map((w) => w.additionalBrowserArgs ?? "");

function switchValues(line, name) {
  const match = line.split(/\s+/).find((part) => part.startsWith(`--${name}=`));
  return match ? match.slice(name.length + 3).split(",") : [];
}

test("every window keeps Tauri's default WebView2 feature switches", () => {
  for (const line of args) {
    for (const feature of ["msWebOOUI", "msPdfOOUI", "msSmartScreenProtection"]) {
      assert.ok(switchValues(line, "disable-features").includes(feature), feature);
    }
  }
});

test("every window suppresses WebView2 implicit sign-in and background networking", () => {
  assert.ok(args.length > 0);
  for (const line of args) {
    const disabled = switchValues(line, "disable-features");
    for (const feature of ["msLoadOneAuthInBackground", "msOneAuthWAM", "msImplicitSignin", "msEnableProfileAADAccountSSO"]) {
      assert.ok(disabled.includes(feature), feature);
    }
    assert.ok(switchValues(line, "enable-features").includes("msLazyLoadOneAuthInWebView"));
    for (const flag of ["--disable-background-networking", "--disable-component-update", "--no-pings"]) {
      assert.ok(line.split(/\s+/).includes(flag), flag);
    }
  }
});
