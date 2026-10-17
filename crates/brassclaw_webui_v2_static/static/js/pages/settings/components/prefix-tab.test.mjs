import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";
const source = readFileSync(new URL("./prefix-tab.js", import.meta.url), "utf8")
  .split("\n").filter(line => !line.startsWith("import ")).join("\n").replaceAll("export function", "function");
function row(entry) {
  const calls = [];
  const context = { html: (strings, ...values) => ({ strings: [...strings], values }), Badge: "Badge", Button: "Button", PrefixGenerationHistory: "History", PrefixSourcesPanel: "Sources", Date };
  vm.createContext(context); vm.runInContext(source + "\nglobalThis.row = PrefixEntryRow;", context);
  const rendered = context.row({ entry, isRegenerating: false, onRegenerate: name => calls.push(name), t: key => key });
  const index = rendered.strings.findIndex(s => s.endsWith("disabled="));
  const click = rendered.strings.findIndex(s => s.endsWith("onClick="));
  return { rendered, calls, disabled: rendered.values[index], click: rendered.values[click] };
}
function values(tree) {
  if (!tree?.values) return [tree];
  return tree.values.flatMap(value => Array.isArray(value) ? value.flatMap(values) : values(value));
}
test("unqualified profiles disable generation and explain prerequisites", () => {
  const r = row({ name: "sempai", label: "Sempai", description: "Component authoring", generation_available: false, generation_status: "prerequisites-required", blocked_reason: "prefix.generationPrerequisites" });
  assert.equal(r.disabled, true); r.click(); assert.deepEqual(r.calls, []);
  assert.ok(values(r.rendered).includes("prefix.blocked"));
  assert.ok(values(r.rendered).includes("prefix.generationPrerequisites"));
  assert.ok(values(r.rendered).includes("Component authoring"));
});
test("legacy base metadata cannot imply verified readiness", () => {
  const r = row({ name: "base-prompt", generation_available: true, generation_status: "legacy-unverified", fingerprint: "existing", assembled_at: "2026-10-11T00:00:00Z", is_stale: false });
  assert.equal(r.disabled, false); r.click(); assert.deepEqual(r.calls, ["base-prompt"]);
  const text = values(r.rendered);
  assert.ok(text.includes("prefix.legacyUnverified")); assert.ok(text.includes("prefix.legacyNotice"));
  assert.ok(!text.includes("prefix.fresh"));
});
test("older incomplete API responses fail closed", () => {
  const r = row({ name: "homeassistant" });
  assert.equal(r.disabled, true); r.click(); assert.deepEqual(r.calls, []);
});
