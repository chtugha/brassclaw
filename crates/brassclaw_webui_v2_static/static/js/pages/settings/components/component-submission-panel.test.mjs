// Controlled UI hooks/transport only; no backend approval or execution evidence.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";
function fixture({ head = 2, loseFirst = false, readback = null } = {}) {
  const state = [], calls = [], reads = [], loads = []; let cursor = 0;
  const context = {
    React: { useState(initial) { const i = cursor++; if (!(i in state)) state[i] = initial; return [state[i], value => { state[i] = value; }]; }, useRef(initial) { const i = cursor++; state[i] ??= { current: initial }; return state[i]; } },
    html: (strings, ...values) => ({ strings: [...strings], values }), Card: "Card", useT: () => key => key,
    TextEncoder, clientActionId: () => "submission",
    fetchComponentRevisionHead: async id => { reads.push(id); return { last_allocated_version: head }; },
    fetchComponentReviewSubmission: async id => { loads.push(id); return readback ?? { receipt: {}, subject_bytes: "retained", component_bytes: [] }; },
    submitComponentReview: async request => { calls.push(request); if (loseFirst && calls.length === 1) throw Error("unknown commit"); return { submission_id: request.submission_id }; },
  };
  const source = readFileSync(new URL("./component-submission-panel.js", import.meta.url), "utf8").split("\n").filter(line => !line.startsWith("import ")).join("\n").replace("export function", "function");
  vm.createContext(context); vm.runInContext(source + "\nglobalThis.render = ComponentSubmissionPanel;", context);
  return { render: () => { cursor = 0; return context.render(); }, calls, reads, loads };
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

test("a joint function request retains both original edits after an uncertain commit", async () => {
  const f = fixture({ loseFirst: true }); fill(f);
  const partner = { role: "changed", candidate_bytes: '{"uuid":"implementation","content":"ä猫"}', base: null, expected_previous_version: 0 };
  field(f, "componentSubmission.partner", JSON.stringify(partner));
  await prop(button(f.render(), "componentSubmission.readHead"), "onClick")();
  await prop(button(f.render(), "componentSubmission.submit"), "onClick")();
  assert.equal(JSON.stringify(f.calls[0].function_partner), JSON.stringify(partner));
  const first = JSON.stringify(f.calls[0]);
  assert.equal(prop(button(f.render(), "componentSubmission.readHead"), "disabled"), true);
  await prop(button(f.render(), "componentSubmission.retry"), "onClick")();
  assert.equal(JSON.stringify(f.calls[1]), first);
});

test("context is sent as an exact partner, while null preserves the old request", async () => {
  const f = fixture(); fill(f);
  const partner = { role: "context", reference: { uuid: "skill", version: 1, checksum: "exact" } };
  field(f, "componentSubmission.partner", JSON.stringify(partner));
  await prop(button(f.render(), "componentSubmission.readHead"), "onClick")();
  await prop(button(f.render(), "componentSubmission.submit"), "onClick")();
  assert.equal(JSON.stringify(f.calls[0].function_partner), JSON.stringify(partner));
  const old = fixture(); fill(old);
  await prop(button(old.render(), "componentSubmission.readHead"), "onClick")();
  await prop(button(old.render(), "componentSubmission.submit"), "onClick")();
  assert.equal(Object.hasOwn(old.calls[0], "function_partner"), false);
});

test("invalid or rounded partner input makes no request", async () => {
  for (const partner of ["{", JSON.stringify({ role: "changed", expected_previous_version: 9007199254740992 }), JSON.stringify({ role: "changed", expected_previous_version: -1 })]) {
    const f = fixture(); fill(f); field(f, "componentSubmission.partner", partner);
    await prop(button(f.render(), "componentSubmission.readHead"), "onClick")();
    await prop(button(f.render(), "componentSubmission.submit"), "onClick")();
    assert.equal(f.calls.length, 0);
  }
});


function fieldReference(f, label) {
  for (const node of nodes(f.render())) {
    const index = node.values.indexOf(label);
    if (index >= 0) return { node, start: index + 1 };
  }
  assert.fail(`missing field ${label}`);
}
async function click(f, label) { await prop(button(f.render(), label), "onClick")(); }
const queueIds = ["22222222-2222-4222-8222-222222222222", "11111111-1111-4111-8111-111111111111"];
for (const target of [
  { mode: "existing", label: "componentSubmission.queueIds", value: JSON.stringify(queueIds), key: "queue_ids", expected: queueIds, changed: '{"incomplete"' },
  { mode: "create", label: "componentSubmission.queueProject", value: "project-ä猫", key: "create_queue_project", expected: "project-ä猫", changed: "other-project" },
]) {
  test(`${target.mode} queue target preserves the exact selectors and original after an unknown commit`, async () => {
    const f = fixture({ loseFirst: true }); fill(f);
    field(f, "componentSubmission.queueTarget", target.mode);
    field(f, target.label, target.value);
    // A change event already queued before the disabled render must not retarget
    // the retained request on retry. No backend receipt is fabricated here.
    const staleSelectorChange = prop(fieldReference(f, target.label), "onChange");
    await click(f, "componentSubmission.readHead");
    await click(f, "componentSubmission.submit");
    const first = JSON.stringify(f.calls[0]);
    assert.deepEqual(JSON.parse(first)[target.key], target.expected);
    const absent = target.mode === "existing" ? "create_queue_project" : "queue_ids";
    assert.equal(Object.hasOwn(f.calls[0], absent), false);
    assert.equal(prop(fieldReference(f, "componentSubmission.queueTarget"), "disabled"), true);
    assert.equal(prop(fieldReference(f, target.label), "disabled"), true);
    assert.equal(prop(button(f.render(), "componentSubmission.new"), "disabled"), true);
    staleSelectorChange({ target: { value: target.changed } });
    await click(f, "componentSubmission.retry");
    assert.equal(f.calls.length, 2);
    assert.strictEqual(f.calls[1], f.calls[0]);
    assert.equal(JSON.stringify(f.calls[1]), first);
  });

  test(`${target.mode} readback cannot acknowledge or replace an uncertain original`, async () => {
    const inspected = {
      receipt: { submission_id: "submission", queue_ids: ["foreign-queue"], queue_binding_status: "bound", review_status: "unreviewed", catalogue_activated: false },
      subject_bytes: "different retained subject — inspection only", component_bytes: [],
    };
    const f = fixture({ loseFirst: true, readback: inspected }); fill(f);
    field(f, "componentSubmission.queueTarget", target.mode); field(f, target.label, target.value);
    await click(f, "componentSubmission.readHead"); await click(f, "componentSubmission.submit");
    const first = JSON.stringify(f.calls[0]);
    await click(f, "componentSubmission.load");
    assert.deepEqual(f.loads, ["submission"]);
    assert.equal(f.calls.length, 1, "inspection must not dispatch a new submission");
    const tree = f.render();
    assert.ok(nodes(tree).some(node => node.values.includes(inspected.subject_bytes)), "inspected bytes remain visible as text");
    assert.equal(nodes(tree).some(node => node.values.includes("componentSubmission.retained")), false, "readback is not the pending POST acknowledgement");
    assert.equal(prop(button(tree, "componentSubmission.retry"), "disabled"), false);
    assert.equal(prop(button(tree, "componentSubmission.new"), "disabled"), true);
    assert.equal(prop(fieldReference(f, target.label), "disabled"), true);
    await click(f, "componentSubmission.retry");
    assert.strictEqual(f.calls[1], f.calls[0]);
    assert.equal(JSON.stringify(f.calls[1]), first);
  });
}

test("standalone retains the old field absence even after editing both hidden queue selectors", async () => {
  const f = fixture(); fill(f);
  field(f, "componentSubmission.queueTarget", "create"); field(f, "componentSubmission.queueProject", "unused-project");
  field(f, "componentSubmission.queueTarget", "existing"); field(f, "componentSubmission.queueIds", JSON.stringify(queueIds));
  field(f, "componentSubmission.queueTarget", "standalone");
  await click(f, "componentSubmission.readHead"); await click(f, "componentSubmission.submit");
  assert.equal(f.calls.length, 1);
  assert.deepEqual(Object.keys(f.calls[0]).sort(), ["base", "candidate_bytes", "dependencies", "expected_previous_version", "submission_id"]);
  assert.equal(Object.hasOwn(f.calls[0], "queue_ids"), false);
  assert.equal(Object.hasOwn(f.calls[0], "create_queue_project"), false);
});
