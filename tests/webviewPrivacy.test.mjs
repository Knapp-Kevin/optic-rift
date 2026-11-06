import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

// Observed on Windows 11 26300 with WebView2 156.0.4314.58: native release
// app showed no TCP/UDP endpoints in a 120-second monitored soak.
// Undocumented WebView2 feature names can change with runtime upgrades.
// This static guard does NOT replace release-build runtime egress qualification.
// WebView2 157.0.4341.42 (auto-updated 2026-11-06) also needs msForceOneAuthWAM disabled:
// without it oneauth.dll loaded and a 443 connection to Microsoft sign-in occurred at launch.
const config = JSON.parse(readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"));
const args = config.app.windows.map((w) => w.additionalBrowserArgs ?? "");

function switchValues(line, name) {
  const match = line.split(/\s+/).find((part) => part.startsWith(`--${name}=`));
  return match ? match.slice(name.length + 3).split(",") : [];
}


const REQUIRED_DISABLED = ["msWebOOUI","msPdfOOUI","msSmartScreenProtection","msLoadOneAuthInBackground","msOneAuthWAM","msImplicitSignin","msEnableProfileAADAccountSSO","msM365LinksImplicitSignin","msSSOBrowserHint","msSSOBrowserHintPhase2","msSendSSODiagnostics","msForceOneAuthWAM"];
const REQUIRED_ENABLED = ["msLazyLoadOneAuthInWebView"];
const REQUIRED_FLAGS = ["--disable-background-networking","--disable-component-update","--no-pings"];

function checkPrivacyArgs(line) {
  const parts = line.split(/\s+/);
  const disabled = switchValues(line, "disable-features");
  const enabled = switchValues(line, "enable-features");
  for (const feature of REQUIRED_DISABLED) {
    assert.ok(disabled.includes(feature), `Missing disabled WebView2 feature: ${feature}`);
  }
  for (const feature of REQUIRED_ENABLED) {
    assert.ok(enabled.includes(feature), `Missing enabled WebView2 feature: ${feature}`);
  }
  for (const flag of REQUIRED_FLAGS) {
    assert.ok(parts.includes(flag), `Missing WebView2 privacy flag: ${flag}`);
  }
}

test("every native window retains all observed WebView2 privacy controls", () => {
  assert.ok(args.length > 0);
  for (const line of args) {
    assert.ok(line.length > 0, "Native window must declare additionalBrowserArgs");
    checkPrivacyArgs(line);
  }
});

test("each required WebView2 privacy switch is mutation-guarded", () => {
  assert.ok(args.length > 0);
  for (const line of args) {
    checkPrivacyArgs(line);
    for (const feature of [...REQUIRED_DISABLED, ...REQUIRED_ENABLED]) {
      const changed = line.replace(feature, "removed_for_test");
      assert.notEqual(changed, line, `Missing test fixture for ${feature}`);
      assert.throws(() => checkPrivacyArgs(changed), /Missing/, `Uncaught feature removal: ${feature}`);
    }
    for (const flag of REQUIRED_FLAGS) {
      const changed = line.replace(flag, "--removed-for-test");
      assert.notEqual(changed, line, `Missing test fixture for ${flag}`);
      assert.throws(() => checkPrivacyArgs(changed), /Missing/, `Uncaught switch removal: ${flag}`);
    }
  }
});
