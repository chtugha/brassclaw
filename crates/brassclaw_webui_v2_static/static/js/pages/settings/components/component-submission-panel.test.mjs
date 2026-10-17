import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";
function fixture({ head = 2, loseFirst = false } = {}) {
  const state = [], calls = [], reads = []; let cursor = 0;
  const context = {
    React: { useState(initial) { const i = cursor++; if (!(i in state)) state[i] = initial; return [state[i], value => { state[i] = value; }]; }, useRef(initial) { const i = cursor++; state[i] ??= { current: initial }; return state[i]; } },
    html: (strings, ...values) => ({ strings: [...strings], values }), Card: "Card", useT: () => key => key,
    TextEncoder, clientActionId: () => "submission",
    fetchComponentRevisionHead: async id => { reads.push(id); return { last_allocated_version: head }; },
    fetchComponentReviewSubmission: async () => ({ receipt: {}, subject_bytes: "retained", component_bytes: [] }),
    submitComponentReview: async request => { calls.push(request); if (loseFirst && calls.length === 1) throw Error("unknown commit"); return { submission_id: request.submission_id }; },
  };
  const source = readFileSync(new URL("./component-submission-panel.js", import.meta.url), "utf8").split("\n").filter(line => !line.startsWith("import ")).join("\n").replace("export function", "function");
  vm.createContext(context); vm.runInContext(source + "\nglobalThis.render = ComponentSubmissionPanel;", context);
  return { render: () => { cursor = 0; return context.render(); }, calls, reads };
}
function nodes(root) { const out = []; const visit = value => { if (Array.isArray(value)) { value.forEach(visit); return; } if (!value?.strings) return; out.push(value); value.values.forEach(visit); }; visit(root); return out; }
function prop(reference, name) {
  const { node, start = 0, end = node.strings.length } = reference;
  const i = node.strings.findIndex((text, index) => index >= start && index <= end && text.includes(`${name}=`));
  assert.ok(i >= 0, `missing ${name}`); return node.values[i];
}
function button(tree, label) {
  for (const node of nodes(tree)) {
    const end = node.values.indexOf(label); if (end < 0) continue;
    let start = end; while (start >= 0 && !node.strings[start].includes("<button")) start--;
    if (start >= 0) return { node, start, end };
  }
  assert.fail(`missing button ${label}`);
}
function field(f, label, value) {
  for (const node of nodes(f.render())) {
    const index = node.values.indexOf(label); if (index < 0) continue;
    prop({ node, start: index + 1 }, "onChange")({ target: { value } }); return;
  }
  assert.fail(`missing field ${label}`);
}
function fill(f) { field(f, "componentSubmission.id", "submission"); field(f, "componentSubmission.candidate", '{"uuid":"component"}'); field(f, "componentSubmission.base", '{"uuid":"component","version":1}'); }
test("allocation counter differs from lineage and is retained in the request", async () => {
  const f = fixture(); fill(f); await prop(button(f.render(), "componentSubmission.readHead"), "onClick")(); await prop(button(f.render(), "componentSubmission.submit"), "onClick")();
  assert.deepEqual(f.reads, ["component"]); assert.equal(f.calls[0].expected_previous_version, 2); assert.equal(f.calls[0].base.version, 1);
});
test("uncertain retry keeps the exact counter, candidate and ID locked", async () => {
  const f = fixture({ loseFirst: true }); fill(f); await prop(button(f.render(), "componentSubmission.readHead"), "onClick")(); await prop(button(f.render(), "componentSubmission.submit"), "onClick")();
  assert.equal(prop(button(f.render(), "componentSubmission.readHead"), "disabled"), true);
  const first = JSON.stringify(f.calls[0]); await prop(button(f.render(), "componentSubmission.retry"), "onClick")(); assert.equal(JSON.stringify(f.calls[1]), first);
});
test("candidate changes require a new observed counter", async () => {
  const f = fixture(); fill(f); await prop(button(f.render(), "componentSubmission.readHead"), "onClick")(); field(f, "componentSubmission.candidate", '{"uuid":"other"}'); await prop(button(f.render(), "componentSubmission.submit"), "onClick")(); assert.equal(f.calls.length, 0);
});
test("unsafe numeric head cannot be rounded into an authoring request", async () => {
  const f = fixture({ head: 9007199254740992 }); fill(f); await prop(button(f.render(), "componentSubmission.readHead"), "onClick")(); await prop(button(f.render(), "componentSubmission.submit"), "onClick")(); assert.equal(f.calls.length, 0);
});
