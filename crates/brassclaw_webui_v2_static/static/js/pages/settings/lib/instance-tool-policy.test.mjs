import assert from "node:assert/strict";
import test from "node:test";
import { fetchInstanceToolSettings, updateInstanceToolSetting } from "./instance-tool-policy.js";

const id = "12345678-1234-1234-1234-123456789abc";
const row = (patch = {}) => ({ tool_id: id, enabled: true, revision: "9007199254740993", capability_ids: ["retained.old", "current.alias"], ...patch });
globalThis.sessionStorage = { getItem: () => "unit-test-token" };
function reply(value, status = 200) {
  return new Response(JSON.stringify(value), { status, headers: { "content-type": "application/json" } });
}

test("client preserves a revision beyond JS integer precision and sends only global policy fields", async () => {
  const calls = [];
  globalThis.fetch = async (path, options) => {
    calls.push({ path, options });
    return reply(row({ revision: "9007199254740994", enabled: false }));
  };
  const updated = await updateInstanceToolSetting(id, "9007199254740993", false);
  assert.equal(updated.revision, "9007199254740994");
  assert.equal(calls.length, 1);
  assert.equal(calls[0].path, `/api/settings/tool-policies/${id}`);
  assert.deepEqual(JSON.parse(calls[0].options.body), { expected_revision: "9007199254740993", enabled: false });
  assert.equal(calls[0].options.headers.get("Authorization"), "Bearer unit-test-token");
});

test("invalid identity/revision/mode is rejected before any mutation", async () => {
  let calls = 0;
  globalThis.fetch = async () => { calls++; throw Error("unexpected dispatch"); };
  for (const [tool, revision, enabled] of [
    ["../other", "1", true], ["00000000-0000-0000-0000-000000000000", "1", true],
    [id, 1, true], [id, "01", true], [id, "0", true],
    [id, "18446744073709551616", true], [id, "1", "ask"],
  ]) await assert.rejects(updateInstanceToolSetting(tool, revision, enabled), /Invalid/);
  assert.equal(calls, 0);
});

test("stale policy errors are surfaced without retry or revision rebasing", async () => {
  let calls = 0;
  globalThis.fetch = async () => { calls++; return reply({ error: "conflict" }, 409); };
  await assert.rejects(updateInstanceToolSetting(id, "1", false), error => error.status === 409);
  assert.equal(calls, 1);
});

test("malformed lists and inconsistent successful acknowledgements cannot become editable policy", async () => {
  for (const value of [{ capabilities: [] }, [row(), row()], [row({ revision: 1 })], [row({ enabled: "true" })]]) {
    globalThis.fetch = async () => reply(value);
    await assert.rejects(fetchInstanceToolSettings(), /Invalid|Duplicate/);
  }
  for (const value of [row({ tool_id: "87654321-1234-1234-1234-123456789abc" }), row(), row({ enabled: false, revision: "9007199254740995" })]) {
    globalThis.fetch = async () => reply(value);
    await assert.rejects(updateInstanceToolSetting(id, "9007199254740993", false), /Unexpected/);
  }
  globalThis.fetch = async () => reply([row({ capability_ids: [] })]);
  assert.equal((await fetchInstanceToolSettings())[0].tool_id, id);
});
