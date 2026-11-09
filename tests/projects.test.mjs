import assert from "node:assert/strict";
import test from "node:test";
import { errorText, projectClient, projectNameError } from "../src/projects.ts";

function fakeInvoke(handlers) {
  const calls = [];
  const invoke = async (command, args) => {
    calls.push({ command, args });
    const handler = handlers[command];
    if (!handler) throw new Error("unexpected command " + command);
    return handler(args);
  };
  return { invoke, calls };
}

test("project names follow the native rule", () => {
  assert.equal(projectNameError("  City Film  "), null);
  assert.match(projectNameError("   "), /Enter a project name/);
  assert.match(projectNameError("x".repeat(121)), /at most 120/);
  assert.equal(projectNameError("é".repeat(120)), null, "counts characters, not UTF-16 units or bytes");
  assert.equal(projectNameError("😀".repeat(120)), null, "astral characters count once");
  assert.match(projectNameError("line\nbreak"), /control characters/);
  assert.match(projectNameError("tab\there"), /control characters/);
});

test("create validates before reaching the native store and sends the trimmed name", async () => {
  const { invoke, calls } = fakeInvoke({
    project_create: ({ name }) => ({ schema_version: 1, id: "a", name, created_unix_ms: 1 })
  });
  const client = projectClient(invoke);
  const rejected = await client.create("  ");
  assert.equal(rejected.ok, false);
  assert.equal(calls.length, 0);
  const created = await client.create("  Trip  ");
  assert.equal(created.ok, true);
  assert.deepEqual(calls[0], { command: "project_create", args: { name: "Trip" } });
});

test("native errors surface as readable failures, never as success", async () => {
  const { invoke } = fakeInvoke({
    project_create: () => { throw "Could not save project: Access is denied. (os error 5)"; },
    project_list: () => { throw new Error("boom"); },
    project_open: () => { throw "Project record is corrupt: EOF"; }
  });
  const client = projectClient(invoke);
  assert.deepEqual(await client.create("X"), { ok: false, error: "Could not save project: Access is denied. (os error 5)" });
  assert.deepEqual(await client.list(), { ok: false, error: "boom" });
  assert.deepEqual(await client.open("id"), { ok: false, error: "Project record is corrupt: EOF" });
});

test("listing passes problems through so corrupt records stay visible", async () => {
  const listing = { projects: [{ id: "a", name: "A", created_unix_ms: 2 }], problems: [{ file_name: "b.json", error: "corrupt" }] };
  const { invoke } = fakeInvoke({ project_list: () => listing });
  assert.deepEqual(await projectClient(invoke).list(), { ok: true, value: listing });
});

test("unknown throwables get a desktop-only explanation", () => {
  assert.match(errorText(undefined), /desktop app/);
});
