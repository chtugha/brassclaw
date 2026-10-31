import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

function fixture({ saved = "", fail = false, status = "Completed", phase = "settled", wrong = false } = {}) {
  const state = [], effects = [], storage = new Map(), submissions = [], reads = [], deferred = [];
  const key = "brassclaw.prefix.pair.spec.proposal";
  if (saved) storage.set(key, saved);
  let cursor = 0, effectCursor = 0, identities = 0, deferReads = false;
  function detail(id) {
    return { target_kind: "pair", operation: "check_cases", mode: "target", specification: wrong ? "other" : "spec", proposal: "proposal", job_id: id, run_status: status, admission_phase: phase,
      internal_receipt: status === "Completed" ? JSON.stringify({ format: "brassclaw-prefix-pair-case-comparison/1", specification: "spec", proposal: "proposal", all_cases_match: false }) : null };
  }
  const context = {
    React: {
      useState(initial) { const i = cursor++; if (!(i in state)) state[i] = typeof initial === "function" ? initial() : initial; return [state[i], value => { state[i] = typeof value === "function" ? value(state[i]) : value; }]; },
      useRef(initial) { const i = cursor++; if (!(i in state)) state[i] = { current: initial }; return state[i]; },
      useEffect(effect, deps) { const i = effectCursor++, old = effects[i]; if (!old || deps.some((value, i) => value !== old.deps[i])) { old?.cleanup?.(); effects[i] = { deps, effect }; } },
    },
    html: (strings, ...values) => ({ strings: [...strings], values }), Button: "Button",
    clientActionId: () => `job-${++identities}`,
    sessionStorage: { getItem: key => storage.get(key), setItem: (key, value) => storage.set(key, value) },
    checkPrefixPairCases: async (specification, proposal, id) => {
      submissions.push({ specification, proposal, id }); await Promise.resolve();
      if (fail) { fail = false; throw Error("lost submission response"); }
      return detail(id);
    },
    fetchPrefixPairJob: async (specification, proposal, id) => {
      reads.push({ specification, proposal, id });
      if (deferReads) return new Promise((resolve, reject) => deferred.push({ resolve, reject }));
      return detail(id);
    },
  };
  const source = readFileSync(new URL("./prefix-pair-cases.js", import.meta.url), "utf8").split("\n").filter(line => !line.startsWith("import ")).join("\n").replace("export function", "function");
  vm.createContext(context); vm.runInContext(source + "\nglobalThis.render = PrefixPairCases;", context);
  function render() {
    cursor = 0; effectCursor = 0;
    const result = context.render({ specification: "spec", proposal: "proposal", t: key => key });
    for (const record of effects) if (record.effect) { const effect = record.effect; record.effect = null; record.cleanup = effect(); }
    return result;
  }
  return { render, storage, submissions, reads, deferred, detail,
    deferReads() { deferReads = true; },
    flush: () => new Promise(setImmediate),
    unmount() { for (const record of effects) record.cleanup?.(); },
  };
}
function nodes(root) { const out = []; function visit(v) { if (Array.isArray(v)) v.forEach(visit); else if (v?.strings) { out.push(v); v.values.forEach(visit); } } visit(root); return out; }
function button(root, label) { return nodes(root).find(node => node.values.includes(label)); }
function click(root, label) { const node = button(root, label); assert.ok(node, label); const i = node.strings.findIndex(value => value.endsWith("onClick=")); assert.notEqual(i, -1); return node.values[i](); }
function open(f) { click(f.render(), "prefix.pairCases"); f.render(); }

test("lost response and repeated clicks retain the original pair and job ID", async () => {
  const f = fixture({ fail: true }); open(f);
  const first = f.render(); await Promise.all([click(first, "prefix.pairCheck"), click(first, "prefix.pairCheck")]);
  assert.equal(f.submissions.length, 1);
  assert.ok(button(f.render(), "lost submission response"));
  await click(f.render(), "prefix.pairRetrySame");
  assert.equal(f.submissions.length, 2);
  assert.deepEqual(f.submissions[1], f.submissions[0]);
  assert.deepEqual([...f.storage.values()], ["job-1"]);
  assert.ok(button(f.render(), "prefix.pairMismatch"));
  await click(f.render(), "prefix.pairNewCheck");
  assert.equal(f.submissions[2].id, "job-2", "only the explicit new-job action changes identity");
});

test("remembered jobs and refresh perform only reads, never a replacement submission", async () => {
  const f = fixture({ saved: "original-job" }); open(f); await f.flush();
  assert.equal(f.reads.length, 1); assert.equal(f.submissions.length, 0);
  await click(f.render(), "common.refresh");
  assert.equal(f.reads.length, 2); assert.equal(f.submissions.length, 0);
  assert.ok(f.reads.every(value => value.id === "original-job"));
});

test("a failed or unfinished job cannot produce a comparison verdict or new-job action", async () => {
  for (const [status, phase] of [["Failed", "started"], ["Running", "started"]]) {
    const f = fixture({ status, phase }); open(f); await click(f.render(), "prefix.pairCheck");
    const root = f.render();
    assert.ok(!button(root, "prefix.pairNewCheck"));
    assert.ok(!button(root, "prefix.pairMatches"));
    assert.ok(!button(root, "prefix.pairMismatch"));
  }
});

test("foreign target responses remain errors rather than displayed proof", async () => {
  const f = fixture({ wrong: true }); open(f); await click(f.render(), "prefix.pairCheck");
  assert.ok(button(f.render(), "prefix.pairWrongJob"));
  assert.ok(!button(f.render(), "prefix.pairMismatch"));
});

test("an obsolete read cannot replace the current submission or publish a stale error", async () => {
  const f = fixture({ saved: "original-job" }); f.deferReads(); open(f);
  assert.equal(f.deferred.length, 1);
  await click(f.render(), "prefix.pairRetrySame");
  f.deferred[0].reject(Error("obsolete failure")); await f.flush();
  assert.ok(!button(f.render(), "obsolete failure"));
  assert.ok(button(f.render(), "prefix.pairMismatch"));
  const pending = click(f.render(), "common.refresh");
  f.unmount(); f.deferred[1].resolve(f.detail("original-job")); await pending;
  assert.equal(f.submissions.length, 1);
});
