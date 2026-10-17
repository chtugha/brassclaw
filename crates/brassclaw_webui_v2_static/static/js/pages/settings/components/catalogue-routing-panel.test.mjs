import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";
function fixture({ conflict = false } = {}) {
  const state = []; let cursor = 0; let failed = true; let loads = 0; let ids = 0; const calls = [];
  const context = {
    React: { useState(initial) { const i = cursor++; if (!(i in state)) state[i] = initial; return [state[i], v => { state[i] = v; }]; },
      useRef(initial) { const i = cursor++; state[i] ??= { current: initial }; return state[i]; } },
    html: (strings, ...values) => ({ strings: [...strings], values }), Card: "Card", useT: () => key => key,
    clientActionId: () => { ids++; return "routing-action"; },
    fetchCatalogueActivationStatus: async () => { loads++; return { generation: "inspected", recipes: ["custom"], selections: [{ recipe_id: "custom", components: ["exact"] }], packaged_recipe_ids: ["builtin"], disabled_recipe_ids: ["withdrawn"] }; },
    activateComponentDeployment: async request => { calls.push(request); await Promise.resolve(); if (failed) { failed = false; const e = Error("publication error"); if (conflict) e.status = 409; throw e; } return { generation: "published", catalogue_activated: true }; },
  };
  const source = readFileSync(new URL("./catalogue-routing-panel.js", import.meta.url), "utf8").split("\n").filter(line => !line.startsWith("import ")).join("\n").replace("export function", "function");
  vm.createContext(context); vm.runInContext(source + "\nglobalThis.render = CatalogueRoutingPanel;", context);
  return { render: () => { cursor = 0; return context.render(); }, calls, loads: () => loads, ids: () => ids };
}
function nodes(root) { const out = []; const visit = v => { if (Array.isArray(v)) return v.forEach(visit); if (!v?.strings) return; out.push(v); v.values.forEach(visit); }; visit(root); return out; }
function prop(n, name) { const i = n.strings.findIndex(s => s.endsWith(`${name}=`)); assert.notEqual(i, -1); return n.values[i]; }
function button(root, label) { return nodes(root).find(n => n.values.includes(label)); }
async function load(f) { await prop(button(f.render(), "catalogueRouting.load"), "onClick")(); }
function toggle(f, id, checked) { const n = nodes(f.render()).find(n => n.values.includes(id)); prop(n, "onChange")({ target: { checked } }); }
test("routing edit preserves complete selections and withdrawn disables", async () => {
  const f = fixture(); await load(f); toggle(f, "builtin", false);
  await prop(button(f.render(), "catalogueRouting.save"), "onClick")();
  const request = JSON.parse(JSON.stringify(f.calls[0]));
  assert.deepEqual(request.recipes, [{ recipe_id: "custom", components: ["exact"] }]);
  assert.deepEqual(request.disabled_recipe_ids, ["builtin", "withdrawn"]);
  assert.equal(request.expected_generation, "inspected");
});
test("uncertain routing commit freezes exact publication and forbids rebase", async () => {
  const f = fixture(); await load(f); const save = prop(button(f.render(), "catalogueRouting.save"), "onClick");
  await Promise.all([save(), save()]); assert.equal(f.calls.length, 1);
  assert.equal(prop(button(f.render(), "catalogueRouting.load"), "disabled"), true);
  await load(f); assert.equal(f.loads(), 1);
  await prop(button(f.render(), "catalogueRouting.retry"), "onClick")();
  assert.strictEqual(f.calls[0], f.calls[1]); assert.equal(f.ids(), 1);
});
test("definite stale generation permits reload without auto-republication", async () => {
  const f = fixture({ conflict: true }); await load(f);
  await prop(button(f.render(), "catalogueRouting.save"), "onClick")();
  assert.equal(prop(button(f.render(), "catalogueRouting.load"), "disabled"), false);
  await load(f); assert.equal(f.loads(), 2); assert.equal(f.calls.length, 1);
});
