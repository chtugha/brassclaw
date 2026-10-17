import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";
function fixture({ failId = false, standalone = false, conflict = false } = {}) {
  const state = []; let cursor = 0; let ids = 0; let failed = true;
  const calls = [], preparations = [];
  const context = {
    React: { useRef(initial) { const i = cursor++; state[i] ??= { current: initial }; return state[i]; },
      useState(initial) { const i = cursor++; if (!(i in state)) state[i] = initial; return [state[i], v => { state[i] = v; }]; } },
    html: (strings, ...values) => ({ strings: [...strings], values }), Card: "Card", useT: () => key => key,
    TextEncoder, clientActionId: () => { ids++; if (failId) throw Error("randomness unavailable"); return "stable-action"; },
    fetchCatalogueActivationStatus: async () => ({ generation: "inspected-generation", recipes: ["existing"] }),
    fetchComponentReviewSubmission: async id => ({ receipt: { submission_id: id, subject_checksum: "exact-subject", candidate: { class_code: standalone ? 22 : 21 }, dependencies: [{ class_code: 1, uuid: "skill" }] } }),
    prepareComponentDeployment: async request => {
      preparations.push(request);
      return { activation: { activation_id: request.activation_id, expected_generation: request.expected_generation, recipes: [{ recipe_id: "existing" }, { recipe_id: "new" }], deployment_subjects: request.recipes.map(r => r.subject) }, preserved_recipe_ids: ["existing"], replaced_recipe_ids: [], catalogue_activated: false };
    },
    activateComponentDeployment: async request => {
      calls.push(request); await Promise.resolve();
      if (failed) { failed = false; const err = Error(conflict ? "stale generation" : "unknown commit"); if (conflict) err.status = 409; throw err; }
      return { generation: "activated", catalogue_activated: true };
    },
  };
  const source = readFileSync(new URL("./component-deployment-panel.js", import.meta.url), "utf8").split("\n").filter(line => !line.startsWith("import ")).join("\n").replace("export function", "function");
  vm.createContext(context); vm.runInContext(source + "\nglobalThis.render = ComponentDeploymentPanel;", context);
  return { render: () => { cursor = 0; return context.render(); }, calls, preparations, ids: () => ids };
}
function nodes(root) { const result = []; const visit = v => { if (Array.isArray(v)) { v.forEach(visit); return; } if (!v?.strings) return; result.push(v); v.values.forEach(visit); }; visit(root); return result; }
function prop(node, name) { const index = node.strings.findIndex(p => p.endsWith(`${name}=`)); assert.notEqual(index, -1, name); return node.values[index]; }
function button(root, label) { return nodes(root).find(n => n.strings.some(p => p.includes("<button")) && n.values.includes(label)); }
function field(f, label, value) {
  const node = nodes(f.render()).find(n => n.values.includes(label));
  const after = node.values.indexOf(label);
  const index = node.strings.findIndex((p, i) => i > after && p.endsWith("onChange="));
  assert.notEqual(index, -1); node.values[index]({ target: { value } });
}
async function prepare(f) { field(f, "componentDeployment.ids", "submission-a"); await prop(button(f.render(), "componentDeployment.prepare"), "onClick")(); }

test("preparation pins inspected subjects and generation without activation", async () => {
  const f = fixture(); field(f, "componentDeployment.approvals", '{"skill":"trusted-approval"}'); await prepare(f);
  assert.equal(f.calls.length, 0); assert.equal(f.preparations[0].expected_generation, "inspected-generation");
  assert.equal(f.preparations[0].recipes[0].subject.subject_checksum, "exact-subject");
  assert.equal(f.preparations[0].recipes[0].association_approvals.skill, "trusted-approval");
  assert.ok(button(f.render(), "componentDeployment.activate"));
});
test("unknown activation commit preserves exact request and blocks reprepare", async () => {
  const f = fixture(); await prepare(f);
  await prop(button(f.render(), "componentDeployment.activate"), "onClick")();
  assert.equal(prop(button(f.render(), "componentDeployment.prepare"), "disabled"), true);
  assert.equal(prop(button(f.render(), "componentDeployment.another"), "disabled"), true);
  await prop(button(f.render(), "componentDeployment.retry"), "onClick")();
  assert.strictEqual(f.calls[0], f.calls[1]); assert.equal(f.ids(), 1);
  assert.equal(prop(button(f.render(), "componentDeployment.retry"), "disabled"), true);
});
test("rapid activation clicks dispatch once", async () => {
  const f = fixture(); await prepare(f); const activate = prop(button(f.render(), "componentDeployment.activate"), "onClick");
  await Promise.all([activate(), activate()]); assert.equal(f.calls.length, 1);
});
test("explicit stale-generation rejection permits fresh preparation", async () => {
  const f = fixture({ conflict: true }); await prepare(f); await prop(button(f.render(), "componentDeployment.activate"), "onClick")();
  assert.equal(prop(button(f.render(), "componentDeployment.prepare"), "disabled"), false);
});
test("missing randomness and standalone candidates cannot activate", async () => {
  for (const options of [{ failId: true }, { standalone: true }]) {
    const f = fixture(options); await prepare(f); assert.equal(f.calls.length, 0); assert.equal(f.preparations.length, 0);
    assert.equal(prop(button(f.render(), "componentDeployment.prepare"), "disabled"), false);
  }
});
test("duplicate subjects and unused approval entries fail before preparation", async () => {
  const f = fixture(); field(f, "componentDeployment.ids", "same same"); await prop(button(f.render(), "componentDeployment.prepare"), "onClick")();
  assert.equal(f.preparations.length, 0);
  field(f, "componentDeployment.approvals", '{"unknown-skill":"approval"}'); await prepare(f); assert.equal(f.preparations.length, 0);
});
