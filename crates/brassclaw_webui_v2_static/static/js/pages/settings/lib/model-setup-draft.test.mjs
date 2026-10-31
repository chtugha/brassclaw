import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";
const specification = "a".repeat(64);
const fields = ["max_prefix_tokens", "max_prompt_tokens", "reserved_output_tokens", "additional_framing_tokens"];
const basis = () => ({ specification, revision: 0, operation_id: null, fields: Object.fromEntries(fields.map(field => [field, { value: null, origin: "unset" }])) });
const plain = value => JSON.parse(JSON.stringify(value));
function fixture() {
  const calls = []; let head = basis(), history = [head], next = 1, failWrite = false, failExact = false, failCurrent = false, corrupt = false;
  const context = { structuredClone, Number, Object, Map, Set, Promise,
    clientActionId: () => `00000000-0000-0000-0000-${String(next++).padStart(12,"0")}`,
    fetchPrefixModelSetupDraft: async (_id, revision = null) => {
      calls.push({ read: revision });
      if (revision !== null && failExact) throw Error("exact unavailable");
      if (revision === null && failCurrent) { failCurrent = false; throw Error("head unavailable"); }
      if (!history[revision ?? head.revision]) throw Error("missing slot");
      return structuredClone(history[revision ?? head.revision]);
    },
    updatePrefixModelSetupDraft: async (_id, request) => {
      calls.push({ write: plain(request) });
      const original = history.find(item => item.operation_id === request.operation_id);
      if (original) return structuredClone(original);
      if (request.expected_revision !== head.revision) throw Error("conflict");
      const result = structuredClone(head); result.revision++; result.operation_id = request.operation_id;
      for (const edit of request.edits) result.fields[edit.field] = { value: edit.value, origin: "manual" };
      head = result; history.push(result);
      if (failWrite) throw Error("ack lost");
      const response = structuredClone(result); if (corrupt) response.fields.max_prompt_tokens.value = 42;
      return response;
    },
  };
  const source = readFileSync(new URL("./model-setup-draft.js", import.meta.url), "utf8").split("\n").filter(line => !line.startsWith("import ")).join("\n").replaceAll("export ", "");
  vm.createContext(context); vm.runInContext(source + "\nglobalThis.api={parseSetupDraft,setupStrings,setupEdits,saveSetupDraft,retrySetupDraft,pendingSetupDraft,loadSetupDraft};", context);
  return { ...context.api, calls, advance: () => {
    const next = structuredClone(head); next.revision++; next.operation_id = "00000000-0000-0000-0000-000000000099";
    next.fields.max_prompt_tokens = { value: 19, origin: "manual" }; head = next; history.push(next);
  }, head: () => plain(head), flags: options => { ({ failWrite = false, failExact = false, failCurrent = false, corrupt = false } = options); } };
}
function save(f, draft = basis(), values = { max_prefix_tokens: "0" }) {
  return f.saveSetupDraft(draft, { ...f.setupStrings(draft), ...values }, Object.fromEntries(Object.keys(values).map(field => [field, true])));
}
test("draft parsing distinguishes explicit unknown and zero and rejects incomplete, unsafe and forged origins", () => {
  const f = fixture(), original = basis(); assert.deepEqual(plain(f.parseSetupDraft(original, specification)), original);
  for (const change of [v => delete v.fields.max_prompt_tokens.value, v => v.fields.max_prompt_tokens.value = 0, v => v.fields.max_prefix_tokens.value = true, v => v.fields.max_prefix_tokens.value = Number.MAX_SAFE_INTEGER + 1, v => v.extra = 1, v => v.fields.reserved_output_tokens.origin = "provider_configuration", v => v.operation_id = "00000000-0000-0000-0000-000000000001"]) {
    const invalid = basis(); change(invalid); assert.throws(() => f.parseSetupDraft(invalid, specification));
  }
  assert.throws(() => f.parseSetupDraft(original, "b".repeat(64)));
});
test("manual same-value edits and clearing unknown remain explicit; invalid numbers never dispatch", () => {
  const f = fixture(); assert.deepEqual(plain(f.setupEdits({ max_prefix_tokens: "0", reserved_output_tokens: "" }, { max_prefix_tokens: true, reserved_output_tokens: true })), [{ field: "max_prefix_tokens", value: 0 }, { field: "reserved_output_tokens", value: null }]);
  for (const value of ["-1", "1.0", "1e2", " 1", "01", "9007199254740992"]) assert.throws(() => f.setupEdits({ max_prefix_tokens: value }, { max_prefix_tokens: true }));
  assert.throws(() => f.setupEdits({ max_prompt_tokens: "0" }, { max_prompt_tokens: true }));
  assert.throws(() => save(f, basis(), {})); assert.equal(f.calls.length, 0);
});
test("a committed lost acknowledgement resolves through the exact native successor without another write", async () => {
  const f = fixture(); f.flags({ failWrite: true }); const result = await save(f);
  assert.equal(result.revision, 1); assert.equal(f.calls.filter(call => call.write).length, 1);
  assert.equal(f.pendingSetupDraft(specification), null);
});
test("unknown outcome locks identity across remount consumers and coalesces retries", async () => {
  const f = fixture(); f.flags({ failWrite: true, failExact: true }); await assert.rejects(save(f));
  const original = f.pendingSetupDraft(specification); original.request.edits[0].value = 99;
  assert.throws(() => save(f)); f.flags({});
  const a = f.retrySetupDraft(specification), b = f.retrySetupDraft(specification); assert.equal(a, b);
  await a;
  const writes = f.calls.filter(call => call.write); assert.equal(writes.length, 2);
  assert.deepEqual(writes[0], writes[1]); assert.equal(f.head().revision, 1);
});
test("an acknowledged write with failed head read retries only the read", async () => {
  const f = fixture(); f.flags({ failCurrent: true }); await assert.rejects(save(f));
  assert(f.pendingSetupDraft(specification)); await f.retrySetupDraft(specification);
  assert.equal(f.calls.filter(call => call.write).length, 1);
});
test("historical acknowledgement cannot replace a newer head", async () => {
  const f = fixture(); f.flags({ failCurrent: true }); await assert.rejects(save(f));
  f.advance();
  const current = await f.retrySetupDraft(specification);
  assert.equal(current.revision, 2);
  assert.equal(current.fields.max_prompt_tokens.value, 19);
  assert.equal(f.calls.filter(call => call.write).length, 1);
});
test("a conflicting committed successor releases only the excluded original, never silently rebases", async () => {
  const f = fixture(); await save(f);
  await assert.rejects(save(f, basis(), { max_prefix_tokens: "5" }), /prefix.setupConflict/);
  assert.equal(f.pendingSetupDraft(specification), null); assert.equal(f.head().revision, 1);
});
test("a malformed acknowledgement is recovered only from exact native storage, never accepted directly", async () => {
  const f = fixture(); f.flags({ corrupt: true }); const result = await save(f);
  assert.equal(result.fields.max_prompt_tokens.value, null);
  assert(f.calls.some(call => call.read === 1));
});
