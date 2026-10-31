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

function actualProfileKeys(entries) {
  const context = { html: (strings,...values)=>({strings:[...strings],values}),
    Card:"Card",Button:"Button",Badge:"Badge",PrefixModelDesigns:"Designs",
    PrefixGenerationHistory:"History",PrefixSourcesPanel:"Sources",Date,
    useT:()=>key=>key,
    usePrefixes:()=>({entries,isLoading:false,loadError:null,regenerating:new Set(),
      regenerateError:null,handleRegenerate(){},reload(){}}),
  };
  vm.createContext(context);
  vm.runInContext(source+"\nglobalThis.tab=PrefixTab;globalThis.row=PrefixEntryRow;",context);
  const rows=[];
  function visit(node){
    if(Array.isArray(node)){node.forEach(visit);return;}
    if(!node?.strings)return;
    if(node.values.includes(context.row)){
      const key=node.strings.findIndex(s=>s.endsWith("key="));
      const entry=node.strings.findIndex(s=>s.endsWith("entry="));
      rows.push({key:node.values[key],entry:node.values[entry]});
    }
    node.values.forEach(visit);
  }
  visit(context.tab());return rows;
}

test("actual parent keys retain profile ownership through changed equal and swapped labels",()=>{
  const a={name:"base-prompt",label:"Operator A",profile_revision:1};
  const b={name:"prefix-maker",label:"Operator B",profile_revision:1};
  for(const entries of [
    [a,b],
    [{...a,label:"Renamed A",profile_revision:2},b],
    [{...a,label:"Shared label"},{...b,label:"Shared label"}],
    [{...a,label:b.label},{...b,label:a.label}],
    [{...b,label:a.label},{...a,label:b.label}],
  ]){
    const rows=actualProfileKeys(entries);
    assert.equal(rows.length,2);
    assert.deepEqual(rows.map(row=>row.key),entries.map(entry=>entry.name));
    assert.equal(new Set(rows.map(row=>row.key)).size,2);
    for(const row of rows)assert.equal(row.entry,entries.find(entry=>entry.name===row.key));
  }
});
