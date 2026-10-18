import assert from "node:assert/strict";
import test from "node:test";
import { fetchInstanceToolSettings, updateInstanceToolSetting } from "./instance-tool-policy.js";

const id = "12345678-1234-1234-1234-123456789abc";
const unlimited = { max_output_bytes: null };
const row = (patch = {}) => ({ tool_id: id, enabled: true, limits: unlimited, revision: "9007199254740993", capability_ids: ["retained.old", "current.alias"], ...patch });
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
  const updated = await updateInstanceToolSetting(id, "9007199254740993", false, unlimited);
  assert.equal(updated.revision, "9007199254740994");
  assert.equal(calls.length, 1);
  assert.equal(calls[0].path, `/api/settings/tool-policies/${id}`);
  assert.deepEqual(JSON.parse(calls[0].options.body), { expected_revision: "9007199254740993", enabled: false, limits: unlimited });
  assert.equal(calls[0].options.headers.get("Authorization"), "Bearer unit-test-token");
});

test("invalid identity/revision/mode is rejected before any mutation", async () => {
  let calls = 0;
  globalThis.fetch = async () => { calls++; throw Error("unexpected dispatch"); };
  for (const [tool, revision, enabled] of [
    ["../other", "1", true], ["00000000-0000-0000-0000-000000000000", "1", true],
    [id, 1, true], [id, "01", true], [id, "0", true],
    [id, "18446744073709551616", true], [id, "1", "ask"],
  ]) await assert.rejects(updateInstanceToolSetting(tool, revision, enabled, unlimited), /Invalid/);
  assert.equal(calls, 0);
});

test("stale policy errors are surfaced without retry or revision rebasing", async () => {
  let calls = 0;
  globalThis.fetch = async () => { calls++; return reply({ error: "conflict" }, 409); };
  await assert.rejects(updateInstanceToolSetting(id, "1", false, unlimited), error => error.status === 409);
  assert.equal(calls, 1);
});

test("malformed lists and inconsistent successful acknowledgements cannot become editable policy", async () => {
  for (const value of [{ capabilities: [] }, [row(), row()], [row({ revision: 1 })], [row({ enabled: "true" })]]) {
    globalThis.fetch = async () => reply(value);
    await assert.rejects(fetchInstanceToolSettings(), /Invalid|Duplicate/);
  }
  for (const value of [row({ tool_id: "87654321-1234-1234-1234-123456789abc" }), row(), row({ enabled: false, revision: "9007199254740995" })]) {
    globalThis.fetch = async () => reply(value);
    await assert.rejects(updateInstanceToolSetting(id, "9007199254740993", false, unlimited), /Unexpected/);
  }
  globalThis.fetch = async () => reply([row({ capability_ids: [] })]);
  assert.equal((await fetchInstanceToolSettings())[0].tool_id, id);
});


test("complete output limits preserve wide integers and explicit clearing", async () => {
  for (const maximum of ["1", "9007199254740993", "18446744073709551615", null]) {
    const limits = { max_output_bytes: maximum };
    let body;
    globalThis.fetch = async (_path, options) => {
      body = JSON.parse(options.body);
      return reply(row({ enabled: false, revision: "9007199254740994", limits }));
    };
    const updated = await updateInstanceToolSetting(id, "9007199254740993", false, limits);
    assert.deepEqual(body.limits, limits);
    assert.deepEqual(updated.limits, limits);
  }
  let calls = 0;
  globalThis.fetch = async () => { calls++; throw Error("unexpected mutation"); };
  for (const limits of [undefined, null, {}, {max_output_bytes:0}, {max_output_bytes:1}, {max_output_bytes:true}, {max_output_bytes:"0"}, {max_output_bytes:"01"}, {max_output_bytes:"1.0"}, {max_output_bytes:"18446744073709551616"}, {max_output_bytes:null,max_wall_clock_ms:"1"}]) {
    await assert.rejects(updateInstanceToolSetting(id,"1",true,limits),/Invalid/);
  }
  assert.equal(calls,0);
  for (const limits of [undefined, {}, {max_output_bytes:1}, {max_output_bytes:"0"}]) {
    globalThis.fetch = async () => reply([row({ limits })]);
    await assert.rejects(fetchInstanceToolSettings(),/Invalid/);
  }
  globalThis.fetch = async () => reply(row({enabled:false,revision:"9007199254740994",limits:{max_output_bytes:"2"}}));
  await assert.rejects(updateInstanceToolSetting(id,"9007199254740993",false,{max_output_bytes:"1"}),/Unexpected/);
});
