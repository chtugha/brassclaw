import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

function fixture({ phase = "uncertain", failId = false } = {}) {
  const state = []; let cursor = 0; let calls = []; let ids = 0;
  let failed = true;
  const evidence = JSON.stringify({ work: { phase }, journal_integrity: true, user: "<script>evil()</script>" });
  const context = {
    React: { useState(initial) {
      const index = cursor++;
      if (!(index in state)) state[index] = initial;
      return [state[index], value => { state[index] = value; }];
    } },
    html: (strings, ...values) => ({ strings: [...strings], values }), Card: "Card", useT: () => key => key,
    TextEncoder, clientActionId: () => { ids++; if (failId) throw Error("randomness unavailable"); return "stable-id"; },
    fetchPostTurnReviews: async () => ({ items: [{ attempt_id: "attempt", phase, model_dispatch_count: 1 }], next_after: null }),
    inspectPostTurnReview: async () => ({ evidence_bytes: evidence, evidence_checksum: "exact-hash", dispositions: [], disposition_count: 0 }),
    recordReviewDisposition: async (attempt, request) => {
      calls.push({ attempt, request });
      if (failed) { failed = false; throw Error("unknown commit"); }
      return { disposition_id: request.disposition_id, retention_released: false, work_replayed: false };
    },
  };
  const source = readFileSync(new URL("./post-turn-review-panel.js", import.meta.url), "utf8")
    .split("\n").filter(line => !line.startsWith("import ")).join("\n")
    .replace("export function", "function");
  vm.createContext(context);
  vm.runInContext(source + "\nglobalThis.render = PostTurnReviewPanel;", context);
  return { render: () => { cursor = 0; return context.render(); }, calls, ids: () => ids, evidence };
}
function nodes(root) {
  const result = [];
  const visit = value => {
    if (Array.isArray(value)) { value.forEach(visit); return; }
    if (!value?.strings) return;
    result.push(value); value.values.forEach(visit);
  };
  visit(root); return result;
}
function prop(node, name) {
  const index = node.strings.findIndex(part => part.endsWith(`${name}=`));
  assert.notEqual(index, -1, name); return node.values[index];
}
function button(root, label) {
  return nodes(root).find(node => node.strings.some(part => part.includes("<button")) && node.values.includes(label));
}
async function inspect(f) {
  await prop(button(f.render(), "postTurnReview.list"), "onClick")();
  await prop(button(f.render(), "attempt"), "onClick")();
}

test("unknown observation commit keeps the same identity and exact inspected checksum on retry", async () => {
  const f = fixture(); await inspect(f);
  const noteNode = nodes(f.render()).find(node => node.values.includes("postTurnReview.note"));
  prop(noteNode, "onChange")({ target: { value: "unknown effects remain quarantined" } });
  await prop(button(f.render(), "postTurnReview.save"), "onClick")();
  assert.equal(prop(button(f.render(), "postTurnReview.list"), "disabled"), true);
  await prop(button(f.render(), "postTurnReview.retry"), "onClick")();
  assert.equal(f.ids(), 1);
  assert.strictEqual(f.calls[0].request, f.calls[1].request);
  assert.equal(f.calls[0].request.evidence_checksum, "exact-hash");
  assert.equal(prop(button(f.render(), "postTurnReview.retry"), "disabled"), true);
  assert.ok(nodes(f.render()).some(node => node.values.includes(f.evidence)), "evidence stays a text value");
});

test("running review never offers an observation write", async () => {
  const f = fixture({ phase: "model_dispatching" }); await inspect(f);
  assert.equal(button(f.render(), "postTurnReview.save"), undefined);
  assert.equal(f.calls.length, 0);
});

test("missing secure randomness reports failure without leaving the panel busy", async () => {
  const f = fixture({ failId: true }); await inspect(f);
  const noteNode = nodes(f.render()).find(node => node.values.includes("postTurnReview.note"));
  prop(noteNode, "onChange")({ target: { value: "observe" } });
  await prop(button(f.render(), "postTurnReview.save"), "onClick")();
  assert.equal(f.calls.length, 0);
  assert.equal(prop(button(f.render(), "postTurnReview.list"), "disabled"), false);
  assert.ok(nodes(f.render()).some(node => node.values.includes("randomness unavailable")));
});
