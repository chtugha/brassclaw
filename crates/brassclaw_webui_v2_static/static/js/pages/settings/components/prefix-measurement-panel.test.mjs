import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

function fixture({ fail = false, status = "Completed", saved = "" } = {}) {
  const state = [], effects = [], timers = new Map(), storage = new Map(), requests = [];
  if (saved) storage.set("brassclaw.prefix.measurement.base-prompt.generation", saved);
  let cursor = 0, effectCursor = 0, timerId = 0, identities = 0;
  const context = { React: {
    useState(initial) { const i = cursor++; if (!(i in state)) state[i] = typeof initial === "function" ? initial() : initial; return [state[i], value => { state[i] = typeof value === "function" ? value(state[i]) : value; }]; },
    useRef(initial) { const i = cursor++; if (!(i in state)) state[i] = { current: initial }; return state[i]; },
    useEffect(effect, deps) { const i = effectCursor++, old = effects[i]; if (!old || deps.some((value, i) => value !== old.deps[i])) { old?.cleanup?.(); effects[i] = { deps, effect }; } },
  }, html: (strings, ...values) => ({ strings: [...strings], values }), Button: "Button",
  clientActionId: () => `00000000-0000-4000-8000-${String(++identities).padStart(12, "0")}`,
  sessionStorage: { getItem: key => storage.get(key), setItem: (key, value) => storage.set(key, value) },
  setTimeout: callback => { const id = ++timerId; timers.set(id, callback); return id; }, clearTimeout: id => timers.delete(id),
  fetchPrefixModelDesigns: async () => ({ proposals: [{ id: "proposal", profile: "base-prompt", specification: "spec" }, { id: "other-profile", profile: "sempai", specification: "spec" }], artifact_sets: [{ id: "files", specification: "spec" }, { id: "wrong-files", specification: "other-spec" }], specifications: [{ id: "spec", model: "model", provider_id: "provider", specification_bytes: '{"configured_context_window":131072}' }] }),
  fetchPrefixJob: async (_profile, id) => ({ job_id: id, run_status: status, answered_tool_calls: 2 }),
  measurePrefix: async (profile, id, request) => { requests.push({ profile, id, request }); await Promise.resolve(); if (fail) { fail = false; throw Error("lost response"); } return { job_id: id, run_status: status, answered_tool_calls: 2, internal_receipt: status === "Completed" ? JSON.stringify({ format: "brassclaw-prefix-capacity-comparison/1", fits: false, required_tokens: 150000, context_window_tokens: 131072 }) : null }; },
  };
  const source = readFileSync(new URL("./prefix-measurement-panel.js", import.meta.url), "utf8").split("\n").filter(line => !line.startsWith("import ")).join("\n").replace("export function", "function");
  vm.createContext(context); vm.runInContext(source + "\nglobalThis.render = PrefixMeasurementPanel;", context);
  function render(revision = 1) {
    cursor = 0; effectCursor = 0;
    const result = context.render({ entry: { name: "base-prompt", profile_revision: revision }, generation: { generation: "generation" }, t: key => key });
    for (const record of effects) if (record.effect) { const effect = record.effect; record.effect = null; record.cleanup = effect(); }
    return result;
  }
  return { render, requests, storage, timers, identities: () => identities, flush: () => new Promise(resolve => setImmediate(resolve)) };
}
function nodes(root) { const result = []; function visit(value) { if (Array.isArray(value)) value.forEach(visit); else if (value?.strings) { result.push(value); value.values.forEach(visit); } } visit(root); return result; }
function attribute(node, name) { const i = node.strings.findIndex(value => value.endsWith(`${name}=`)); assert.notEqual(i, -1, name); return node.values[i]; }
function button(root, label) { return nodes(root).find(value => value.values.includes(label)); }
function controls(root, tag) {
  const result = [];
  for (const node of nodes(root)) for (let i = 0; i < node.strings.length; i++) {
    const start = node.strings[i].indexOf(`<${tag} `);
    if (start < 0) continue;
    const end = node.strings.findIndex((value, j) => j >= i && value.includes(tag === "input" ? "/>" : `</${tag}>`));
    const strings = node.strings.slice(i, end < 0 ? undefined : end + 1);
    strings[0] = strings[0].slice(start);
    result.push({ strings, values: node.values.slice(i, end < 0 ? undefined : end + 1) });
  }
  return result;
}
async function choose(f) {
  attribute(button(f.render(), "prefix.measureOffline"), "onClick")(); f.render(); await f.flush();
  attribute(controls(f.render(), "select")[0], "onChange")({ target: { value: "proposal" } });
  attribute(controls(f.render(), "select")[1], "onChange")({ target: { value: "files" } });
}
test("lost-response retries preserve identity, inputs and revision; storage keeps only correlation", async () => {
  const f = fixture({ fail: true }); await choose(f);
  const submit = attribute(button(f.render(), "prefix.measureStart"), "onClick");
  await Promise.all([submit(), submit()]);
  assert.equal(f.requests.length, 1); assert.equal(f.identities(), 1);
  const retry = attribute(button(f.render(2), "prefix.measureRetry"), "onClick"); await retry();
  assert.equal(f.requests.length, 2); assert.equal(f.identities(), 1);
  assert.equal(f.requests[1].id, f.requests[0].id); assert.equal(f.requests[1].request, f.requests[0].request);
  assert.equal(f.requests[1].request.expected_profile_revision, 1);
  assert.equal(f.requests[1].request.context_window_tokens, 131072);
  assert.deepEqual([...f.storage.values()], [f.requests[0].id]);
  const rendered = f.render(2);
  assert.ok(button(rendered, "prefix.measureDoesNotFit")); assert.equal(f.timers.size, 0);
});
test("invalid numeric limits cannot be silently coerced or dispatch a measurement", async () => {
  const f = fixture(); await choose(f);
  const field = controls(f.render(), "input").find(node => node.values.includes("prefix.measureContext"));
  attribute(field, "onChange")({ target: { value: "1e6" } });
  await attribute(button(f.render(), "prefix.measureStart"), "onClick")();
  assert.equal(f.requests.length, 0); assert.ok(button(f.render(), "prefix.measureInvalidLimits"));
});
test("proposal and artifact choices are restricted to the selected profile and model binding", async () => {
  const f = fixture(); await choose(f);
  const selects = controls(f.render(), "select");
  assert.ok(!JSON.stringify(selects).includes("other-profile"));
  assert.ok(!JSON.stringify(selects).includes("wrong-files"));
});

for (const status of ["Completed", "Failed", "Cancelled", "RecoveryRequired"]) {
  test(`polling stops for the actual ${status} wire status`, async () => {
    const f = fixture({ status }); await choose(f);
    await attribute(button(f.render(), "prefix.measureStart"), "onClick")();
    f.render(); assert.equal(f.timers.size, 0);
  });
}
test("restoring a remembered job reads its result without resubmitting probes", async () => {
  const f = fixture({ saved: "retained-job" });
  attribute(button(f.render(), "prefix.measureOffline"), "onClick")(); f.render(); await f.flush();
  assert.equal(f.timers.size, 1);
  const [id, poll] = [...f.timers][0]; f.timers.delete(id); await poll(); f.render();
  assert.equal(f.requests.length, 0); assert.equal(f.identities(), 0); assert.equal(f.timers.size, 0);
});
test("an explicit new measurement gets a new identity without modifying the prior request", async () => {
  const f = fixture({ fail: true }); await choose(f);
  await attribute(button(f.render(), "prefix.measureStart"), "onClick")();
  attribute(button(f.render(2), "prefix.measureEditNew"), "onClick")();
  await attribute(button(f.render(2), "prefix.measureStart"), "onClick")();
  assert.equal(f.requests.length, 2); assert.notEqual(f.requests[0].id, f.requests[1].id);
  assert.equal(f.requests[0].request.expected_profile_revision, 1);
  assert.equal(f.requests[1].request.expected_profile_revision, 2);
});
