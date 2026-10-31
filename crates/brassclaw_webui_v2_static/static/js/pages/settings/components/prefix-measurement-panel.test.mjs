import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

function fixture({ fail = false, status = "Completed", saved = "", revision = 1, read, measure } = {}) {
  const state = [], effects = [], layouts = [], timers = new Map(), storage = new Map(), requests = [], reads = [];
  if (saved) storage.set("brassclaw.prefix.measurement.base-prompt.generation", saved);
  let cursor = 0, effectCursor = 0, layoutCursor = 0, timerId = 0, identities = 0;
  let selection = { profile: "base-prompt", generation: "generation", revision };
  const context = { React: {
    useState(initial) { const i = cursor++; if (!(i in state)) state[i] = typeof initial === "function" ? initial() : initial; return [state[i], value => { state[i] = typeof value === "function" ? value(state[i]) : value; }]; },
    useRef(initial) { const i = cursor++; if (!(i in state)) state[i] = { current: initial }; return state[i]; },
    useEffect(effect, deps) { const i = effectCursor++, old = effects[i]; if (!old || deps.some((value, i) => value !== old.deps[i])) { old?.cleanup?.(); effects[i] = { deps, effect }; } },
    useLayoutEffect(effect, deps) { const i = layoutCursor++, old = layouts[i]; if (!old || deps.some((value, i) => value !== old.deps[i])) { old?.cleanup?.(); layouts[i] = { deps, effect }; } },
  }, html: (strings, ...values) => ({ strings: [...strings], values }), Button: "Button",
  clientActionId: () => `00000000-0000-4000-8000-${String(++identities).padStart(12, "0")}`,
  sessionStorage: { getItem: key => storage.get(key), setItem: (key, value) => storage.set(key, value) },
  setTimeout: callback => { const id = ++timerId; timers.set(id, callback); return id; }, clearTimeout: id => timers.delete(id),
  fetchPrefixModelDesigns: async () => ({ proposals: [{ id: "proposal", profile: "base-prompt", specification: "spec", proposal_bytes: '{"format":"brassclaw-prefix-design-proposal/1"}' }, { id: "other-profile", profile: "sempai", specification: "spec", proposal_bytes: '{"format":"brassclaw-prefix-design-proposal/1"}' }, { id: "rule-proposal", profile: "base-prompt", specification: "spec", proposal_bytes: '{"format":"brassclaw-prefix-design-proposal/2"}' }, { id: "invalid-proposal", profile: "base-prompt", specification: "spec", proposal_bytes: 'null' }], artifact_sets: [{ id: "files", specification: "spec" }, { id: "wrong-files", specification: "other-spec" }], specifications: [{ id: "spec", model: "model", provider_id: "provider", specification_bytes: '{"configured_context_window":131072}' }] }),
  fetchPrefixJob: async (profile, id) => { reads.push({ profile, id }); return read ? read(profile, id) : { job_id: id, run_status: status, answered_tool_calls: 2 }; },
  measurePrefix: async (profile, id, request) => { requests.push({ profile, id, request }); await Promise.resolve(); if (measure) return measure(profile, id, request); if (fail) { fail = false; throw Error("lost response"); } return { job_id: id, run_status: status, answered_tool_calls: 2, internal_receipt: status === "Completed" ? JSON.stringify({ format: "brassclaw-prefix-capacity-comparison/1", fits: false, required_tokens: 150000, context_window_tokens: 131072 }) : null }; },
  };
  const source = readFileSync(new URL("./prefix-measurement-panel.js", import.meta.url), "utf8").split("\n").filter(line => !line.startsWith("import ")).join("\n").replace("export function", "function");
  vm.createContext(context); vm.runInContext(source + "\nglobalThis.render = PrefixMeasurementPanel;", context);
  function render(selected) {
    if (selected !== undefined) selection = typeof selected === "object" ? { ...selection, ...selected } : { ...selection, revision: selected };
    cursor = 0; effectCursor = 0; layoutCursor = 0;
    const result = context.render({ entry: { name: selection.profile, profile_revision: selection.revision }, generation: { generation: selection.generation }, t: key => key });
    for (const record of layouts) if (record.effect) { const effect = record.effect; record.effect = null; record.cleanup = effect(); }
    for (const record of effects) if (record.effect) { const effect = record.effect; record.effect = null; record.cleanup = effect(); }
    return result;
  }
  return { render, requests, reads, storage, timers, identities: () => identities, flush: () => new Promise(resolve => setImmediate(resolve)) };
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
  const f = fixture({ fail: true, revision: "9007199254740993" }); await choose(f);
  const submit = attribute(button(f.render(), "prefix.measureStart"), "onClick");
  await Promise.all([submit(), submit()]);
  assert.equal(f.requests.length, 1); assert.equal(f.identities(), 1);
  const retry = attribute(button(f.render(2), "prefix.measureRetry"), "onClick"); await retry();
  assert.equal(f.requests.length, 2); assert.equal(f.identities(), 1);
  assert.equal(f.requests[1].id, f.requests[0].id); assert.equal(f.requests[1].request, f.requests[0].request);
  assert.equal(f.requests[1].request.expected_profile_revision, "9007199254740993");
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

test("reference-rule proposals stay outside the legacy diagnostic and cannot dispatch", async () => {
  const f = fixture(); await choose(f);
  const rendered = f.render();
  assert.ok(button(rendered, "prefix.measureRuleDesignNotice"));
  const select = controls(rendered, "select")[0];
  assert.ok(!JSON.stringify(select).includes("rule-proposal"));
  assert.ok(!JSON.stringify(select).includes("invalid-proposal"));
  attribute(select, "onChange")({ target: { value: "rule-proposal" } });
  await attribute(button(f.render(), "prefix.measureStart"), "onClick")();
  assert.equal(f.requests.length, 0);
  assert.equal(f.identities(), 0);
  assert.ok(button(f.render(), "prefix.measureSelectInputs"));
});

function deferred() { let resolve, reject; const promise = new Promise((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; }
function completed(id, fits = true) { return { job_id: id, run_status: "Completed", answered_tool_calls: 2,
  internal_receipt: JSON.stringify({ format: "brassclaw-prefix-capacity-comparison/1", fits, required_tokens: 100, context_window_tokens: 1000 }) }; }
function shown(f, value) { return nodes(f.render()).some(node => node.values.includes(value)); }
for (const via of ["refresh", "poll"]) {
  test(`late old-job ${via} cannot settle a new uncertain request`, async () => {
    const old = deferred();
    const f = fixture({ saved: "old-job", read: () => old.promise, measure: async () => { throw Error("uncertain write"); } });
    await choose(f);
    let waiting;
    if (via === "refresh") waiting = attribute(button(f.render(), "common.refresh"), "onClick")();
    else { const [id, poll] = [...f.timers][0]; f.timers.delete(id); waiting = poll(); }
    await attribute(button(f.render(), "prefix.measureStart"), "onClick")();
    const original = f.requests[0], originalBytes = JSON.stringify(original.request);
    old.resolve(completed("old-job")); await waiting;
    assert.ok(button(f.render(2), "prefix.measureRetry"));
    assert.ok(!button(f.render(), "prefix.measureFits"));
    assert.ok(shown(f, original.id));
    await attribute(button(f.render(2), "prefix.measureRetry"), "onClick")();
    assert.equal(f.requests[1].profile, original.profile);
    assert.equal(f.requests[1].id, original.id);
    assert.equal(f.requests[1].request, original.request);
    assert.equal(JSON.stringify(f.requests[1].request), originalBytes);
    assert.equal(f.identities(), 1);
  });
}
test("older reads of the same job cannot replace its newer completed observation", async () => {
  const old = deferred(); let calls = 0;
  const f = fixture({ saved: "retained-job", read: (_profile, id) => ++calls === 1 ? old.promise : completed(id, false) });
  attribute(button(f.render(), "prefix.measureOffline"), "onClick")(); f.render(); await f.flush();
  const refresh = attribute(button(f.render(), "common.refresh"), "onClick");
  const first = refresh(); await refresh(); f.render();
  old.resolve({ job_id: "retained-job", run_status: "Running", answered_tool_calls: 0 }); await first;
  assert.ok(shown(f, "Completed"));
  assert.ok(!shown(f, "Running"));
  assert.ok(button(f.render(), "prefix.measureDoesNotFit"));
  assert.equal(f.timers.size, 0);
});
test("profile and generation changes restore only their own remembered job", async () => {
  const old = deferred(); let calls = 0;
  const f = fixture({ saved: "old-job", read: (_profile, id) => ++calls === 1 ? old.promise : completed(id, false) });
  f.storage.set("brassclaw.prefix.measurement.sempai.other-generation", "own-job");
  attribute(button(f.render(), "prefix.measureOffline"), "onClick")(); f.render(); await f.flush();
  const pending = attribute(button(f.render(), "common.refresh"), "onClick")();
  f.render({ profile: "sempai", generation: "other-generation" }); f.render(); await f.flush();
  old.resolve(completed("old-job")); await pending;
  assert.ok(!shown(f, "old-job"));
  assert.ok(!button(f.render(), "prefix.measureFits"));
  assert.ok(shown(f, "own-job"));
  await attribute(button(f.render(), "common.refresh"), "onClick")();
  assert.deepEqual(f.reads.at(-1), { profile: "sempai", id: "own-job" });
  assert.ok(button(f.render(), "prefix.measureDoesNotFit"));
  assert.equal(f.requests.length, 0);
  assert.equal(f.identities(), 0);
});
test("late submit success cannot appear under another generation", async () => {
  const waiting = deferred(); const f = fixture({ measure: () => waiting.promise }); await choose(f);
  const submitted = attribute(button(f.render(), "prefix.measureStart"), "onClick")(); await f.flush();
  const original = f.requests[0], bytes = JSON.stringify(original.request);
  f.render({ generation: "different-generation" }); f.render();
  waiting.resolve(completed(original.id)); await submitted;
  assert.ok(!shown(f, original.id));
  assert.ok(!button(f.render(), "prefix.measureFits"));
  assert.equal(JSON.stringify(original.request), bytes);
  assert.equal(f.storage.get("brassclaw.prefix.measurement.base-prompt.generation"), original.id);
  assert.equal(f.storage.has("brassclaw.prefix.measurement.base-prompt.different-generation"), false);
  assert.equal(f.requests.length, 1);
});
test("a foreign job response cannot confirm or discard the original uncertain request", async () => {
  const f = fixture({ measure: () => completed("foreign-job") }); await choose(f);
  await attribute(button(f.render(), "prefix.measureStart"), "onClick")();
  assert.ok(button(f.render(), "prefix.measureRetry"));
  assert.ok(button(f.render(), "prefix.measureUnknown"));
  assert.ok(!button(f.render(), "prefix.measureFits"));
  const original = f.requests[0];
  await attribute(button(f.render(2), "prefix.measureRetry"), "onClick")();
  assert.equal(f.requests[1].id, original.id);
  assert.equal(f.requests[1].request, original.request);
});
test("superseding a running poll with refresh keeps polling that same active job", async () => {
  const old = deferred(); let reads = 0;
  const f = fixture({ status: "Running", read: (_profile, id) => ++reads === 1 ? old.promise : { job_id: id, run_status: "Running", answered_tool_calls: 2 } });
  await choose(f); await attribute(button(f.render(), "prefix.measureStart"), "onClick")(); f.render();
  const [timer, poll] = [...f.timers][0]; f.timers.delete(timer); const waiting = poll();
  await attribute(button(f.render(), "common.refresh"), "onClick")(); f.render();
  old.resolve({ job_id: f.requests[0].id, run_status: "Running", answered_tool_calls: 1 }); await waiting; f.render();
  assert.equal(f.timers.size, 1);
  assert.ok(shown(f, "Running"));
  assert.equal(f.requests.length, 1);
});
