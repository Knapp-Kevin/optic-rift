import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { concealed, downloadLabel, mergeStatus } from "../src/downloadState.ts";

const ev = (id, file = "unet/secret-name.safetensors") => ({ install_id: id, repo: "src/repo", state: "downloading", file, file_index: 2, file_count: 9, bytes_done: 1, bytes_total: 4, message: null });
const session = (o = {}) => ({ installId: "A", repo: "src/repo", revision: "f".repeat(40), flagged: false, progress: null, cancel: "idle", adopted: false, ...o });
const status = (o = {}) => ({ install_id: "A", repo: "src/repo", revision: "f".repeat(40), source_flagged: false, cancel_requested: false, last: ev("A"), ...o });

test("Identity is concealed while discovery is off unless the source is known unflagged", () => {
  for (const [flagged, includeAdult, hidden] of [[true, false, true], [null, false, true], [false, false, false], [true, true, false], [null, true, false]]) {
    assert.equal(concealed({ flagged }, includeAdult), hidden, `${flagged}/${includeAdult}`);
    assert.equal(downloadLabel({ repo: "src/repo", flagged }, includeAdult), hidden ? "a model" : "src/repo");
  }
});

test("A download adopted before classification stays concealed (reload during pre-inspection)", () => {
  const adopted = mergeStatus(null, status({ source_flagged: null }), false);
  assert.equal(adopted.flagged, null);
  assert.equal(downloadLabel(adopted, false), "a model");
  // Classification arrives: unflagged reveals, flagged keeps concealing; flagged never relaxes.
  assert.equal(mergeStatus(adopted, status({ source_flagged: false }), false).flagged, false);
  const flagged = mergeStatus(adopted, status({ source_flagged: true }), false);
  assert.equal(flagged.flagged, true);
  assert.equal(mergeStatus(flagged, status({ source_flagged: false }), false).flagged, true);
  assert.equal(mergeStatus(flagged, status({ source_flagged: null }), false).flagged, true);
});

test("Native status settles cancellation; a rejected or failed cancel never sticks", () => {
  // Rejected or failed call: the provider returns the exact install to idle, and native (false) keeps it idle.
  assert.equal(mergeStatus(session({ cancel: "idle" }), status({ cancel_requested: false }), false).cancel, "idle");
  // An old sticky value is corrected by native truth when no newer cancel action happened.
  assert.equal(mergeStatus(session({ cancel: "accepted" }), status({ cancel_requested: false }), false).cancel, "idle");
  // Accepted stays disabled while native reports the request, until the download ends.
  assert.equal(mergeStatus(session({ cancel: "accepted" }), status({ cancel_requested: true }), false).cancel, "accepted");
  assert.equal(mergeStatus(session({ cancel: "idle" }), status({ cancel_requested: true }), false).cancel, "accepted");
  // While the call is in flight, a poll that hasn't seen it yet keeps "pending".
  assert.equal(mergeStatus(session({ cancel: "pending" }), status({ cancel_requested: false }), false).cancel, "pending");
  // Poll race: a status read issued before the latest cancel action cannot downgrade it.
  assert.equal(mergeStatus(session({ cancel: "accepted" }), status({ cancel_requested: false }), true).cancel, "accepted");
});

test("Stale or mismatched native ids adopt, keep or clear correctly", () => {
  // Our own in-flight install keeps ownership even if native reports a different id.
  const own = session({ installId: "A", adopted: false });
  assert.equal(mergeStatus(own, status({ install_id: "B" }), false), own);
  // An adopted session is replaced by the install native actually runs.
  const replaced = mergeStatus(session({ installId: "A", adopted: true }), status({ install_id: "B", source_flagged: null }), false);
  assert.equal(replaced.installId, "B");
  assert.equal(replaced.flagged, null);
  // Native reports nothing: an adopted session clears (no stuck bar); our own waits for its promise.
  assert.equal(mergeStatus(session({ adopted: true }), null, false), null);
  assert.equal(mergeStatus(own, null, false), own);
});

test("Progress and errors hide source file paths and messages while identity is concealed", () => {
  const ui = readFileSync(new URL("../src/downloads.tsx", import.meta.url), "utf8");
  assert.match(ui, /const hide = concealed\(session, includeAdult\);/);
  assert.match(ui, /!hide && p\.file \?/);
  assert.match(ui, /concealed\(error, includeAdult\) \? "Turn on adult discovery to see details\." : error\.message/);
  // Pending/accepted disable Cancel; settle() only resets the exact id's pending state.
  assert.match(ui, /prev\?\.installId === id && prev\.cancel === "pending" \? \{ \.\.\.prev, cancel: next \}/);
  assert.match(ui, /settle\(accepted \? "accepted" : "idle"\)/);
  assert.match(ui, /\.catch\(\(\) => \{ settle\("idle"\); void reconcile\(\); \}\)/);
  const native = readFileSync(new URL("../src-tauri/src/model_store.rs", import.meta.url), "utf8");
  assert.match(native, /source_flagged: None,/);
  assert.match(native, /a\.source_flagged = Some\(detail\.audience == "not-for-all-audiences"\)/);
});
