import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";
function fixture({ inactive = false, records = {}, revision = 3 } = {}) {
  const state = [], captures = [], imports = [], pairImports = [], effects = [], listeners = new Map(), fetches = []; let cursor = 0, deferred = false;
  const target = { provider_id: "registered", provider_revision: revision, model: "active-model", consumer: "kohai", configured_context_window: null };
  let catalogue = { targets: [target, { ...target, consumer: "sempai" }], specifications: [], proposals: [], design_generation_available: false, blocked_reason: "prefix.modelDesignPrerequisites" };
  catalogue = { ...catalogue, ...records };
  if (inactive) catalogue.targets.push({ ...target, model: "configured-model", consumer: "configured" });
  const context = { PrefixModelSetupDraft: "PrefixModelSetupDraft", TextDecoder, PrefixPairCases: "PrefixPairCases", addEventListener: (name, listener) => listeners.set(name, listener), removeEventListener: name => listeners.delete(name), React: {
    useState(initial) { const i=cursor++; if (!(i in state)) state[i]=typeof initial === "function" ? initial() : initial; return [state[i], value => { state[i]=typeof value === "function" ? value(state[i]) : value; }]; },
    useRef(initial) { const i=cursor++; if (!(i in state)) state[i]={current:initial}; return state[i]; },
    useEffect(effect) { if (!effects.length) effects.push(effect); },
  }, html:(strings,...values)=>({strings:[...strings],values}), Button:"Button", PrefixModelConfiguration:"PrefixModelConfiguration", ModelTokenLimits:"ModelTokenLimits",
    fetchPrefixModelDesigns: () => deferred ? new Promise((resolve,reject)=>fetches.push({resolve,reject,snapshot:catalogue})) : Promise.resolve(catalogue),
    prepareModelArtifactImport: async files => { await Promise.resolve(); return JSON.stringify({files:files.map(file=>file.name)}); },
    retainPrefixPairConstruction: async request => { pairImports.push(request); await Promise.resolve(); catalogue = {...catalogue,pair_proposals:[{id:"pair-draft",specification:request.specification,proposal_bytes:request.proposal_bytes,qualification:"unaccepted-pair-construction-proposal"}]}; },
    retainPrefixModelArtifacts: async request => { imports.push(request); await Promise.resolve(); catalogue = {...catalogue,artifact_sets:[{id:"asset-set",specification:request.specification,files:[],binding_bytes:"operator provenance",qualification:"unverified-model-artifacts"}]}; },
    capturePrefixModel: async request => { captures.push(request); await Promise.resolve(); catalogue = { ...catalogue, specifications: [{id:"retained",provider_id:request.provider_id,provider_revision:request.expected_provider_revision,model:request.model,specification_bytes:"<script>untrusted</script>",qualification:"unknown-model-artifacts"}] }; },
  };
  const source=readFileSync(process.env.PREFIX_MODEL_DESIGNS_SOURCE || new URL("./prefix-model-designs.js",import.meta.url),"utf8").split("\n").filter(line=>!line.startsWith("import ")).join("\n").replace("export function","function");
  vm.createContext(context);vm.runInContext(source+"\nglobalThis.render=PrefixModelDesigns;",context);
  const render=()=>{cursor=0;return context.render({t:key=>key});}; render(); effects[0]();
  return {captures,imports,pairImports,render,fetches,deferReloads(){deferred=true;},resumeReloads(){deferred=false;}, advanceRevision: async revision => { catalogue = {...catalogue, targets: catalogue.targets.map(item => ({...item, provider_revision: revision}))}; await listeners.get("brassclaw-provider-settings-changed")(); }};
}
function nodes(root) { const out=[];function visit(v){if(Array.isArray(v)){v.forEach(visit);return;}if(!v?.strings)return;out.push(v);v.values.forEach(visit);}visit(root);return out; }
function value(node,key,after="") { const start=after ? node.strings.findIndex(s=>s.includes(after)) : 0;assert.notEqual(start,-1,after);const i=node.strings.findIndex((s,index)=>index>=start && s.endsWith(`${key}=`));assert.notEqual(i,-1,key);return node.values[i]; }
function button(root,label) { return nodes(root).find(node=>node.values.includes(label)); }
test("selecting an active target is read-only; capture pins revision and deduplicates clicks",async()=>{
  const f=fixture({revision:"9007199254740993"}); await new Promise(setImmediate);
  const select=nodes(f.render()).find(node=>node.strings.some(s=>s.includes("<select")));
  const options=nodes(f.render()).filter(node=>node.strings[0].startsWith("<option key="));
  assert.equal(options.length,1,"Kohai/Sempai same model must not duplicate target choices");
  value(select,"onChange")({target:{value:JSON.stringify(["registered","active-model"])}});
  assert.equal(f.captures.length,0);
  assert.equal(value(button(f.render(),"prefix.generateModelDesign"),"disabled"),true);
  const capture=value(button(f.render(),"prefix.captureModel"),"onClick");
  await Promise.all([capture(),capture()]);
  assert.equal(f.captures.length,1);
  assert.equal(JSON.stringify(f.captures[0]),JSON.stringify({provider_id:"registered",expected_provider_revision:"9007199254740993",model:"active-model"}));
  assert.ok(nodes(f.render()).some(node=>node.values.includes("prefix.unknownModelArtifacts")));
  assert.ok(nodes(f.render()).some(node=>node.values.includes("<script>untrusted</script>")),"record is rendered as escaped text, not executable HTML");
});

test("file import pins the displayed specification, coalesces concurrent events and cannot enable design generation",async()=>{
  const f=fixture(); await new Promise(setImmediate);
  const select=nodes(f.render()).find(node=>node.strings.some(s=>s.includes("<select")));
  value(select,"onChange")({target:{value:JSON.stringify(["registered","active-model"])}});
  await value(button(f.render(),"prefix.captureModel"),"onClick")();
  const input=nodes(f.render()).find(node=>node.strings.some(s=>s.includes('<input type="file"')));
  const change=value(input,"onChange");
  const event=()=>({target:{files:[{name:"config.json"}],value:"local file selection"}});
  change(event()); change(event());
  await new Promise(setImmediate);
  assert.equal(f.imports.length,1);
  assert.equal(f.imports[0].specification,"retained");
  assert.equal(f.captures.length,1,"import does not create a different provider/model capture");
  assert.equal(value(button(f.render(),"prefix.generateModelDesign"),"disabled"),true);
  assert.ok(nodes(f.render()).some(node=>node.values.includes("operator provenance")));
});


test("provider settings refresh retains the selected model and captures the new revision",async()=>{
  const f=fixture(); await new Promise(setImmediate);
  const select=nodes(f.render()).find(node=>node.strings.some(s=>s.includes("<select")));
  value(select,"onChange")({target:{value:JSON.stringify(["registered","active-model"])}});
  await f.advanceRevision(4);
  const root=f.render();
  assert.equal(value(button(root,"prefix.captureModel"),"disabled"),false);
  assert.ok(nodes(root).some(node=>node.values.includes("ModelTokenLimits")),"shared editor remains mounted after the settings update");
  await value(button(root,"prefix.captureModel"),"onClick")();
  assert.equal(f.captures[0].expected_provider_revision,4);
});


test("an older catalogue response cannot replace the latest model revision before capture",async()=>{
  const f=fixture();await new Promise(setImmediate);
  const select=nodes(f.render()).find(node=>node.strings.some(s=>s.includes("<select")));
  value(select,"onChange")({target:{value:JSON.stringify(["registered","active-model"])}});
  f.deferReloads();const old=f.advanceRevision(4),latest=f.advanceRevision(5);
  f.fetches[1].resolve(f.fetches[1].snapshot);await latest;
  f.fetches[0].resolve(f.fetches[0].snapshot);await old;f.resumeReloads();
  await value(button(f.render(),"prefix.captureModel"),"onClick")();
  assert.equal(f.captures[0].expected_provider_revision,5);
});
test("an obsolete read failure cannot hide fresh model-design information",async()=>{
  const f=fixture();await new Promise(setImmediate);
  f.deferReloads();const old=f.advanceRevision(4),latest=f.advanceRevision(5);
  f.fetches[1].resolve(f.fetches[1].snapshot);await latest;
  f.fetches[0].reject(Error("obsolete request failed"));await old;
  assert.ok(!nodes(f.render()).some(node=>node.values.includes("obsolete request failed")));
});


test("configured inactive models can be captured without selecting a runtime role", async () => {
  const f = fixture({ inactive: true }); await new Promise(setImmediate);
  const root = f.render();
  const options = nodes(root).filter(node => node.strings[0].startsWith("<option key="));
  assert.equal(options.length, 2);
  const select = nodes(root).find(node => node.strings.some(s => s.includes("<select")));
  value(select, "onChange")({ target: { value: JSON.stringify(["registered", "configured-model"]) } });
  assert.equal(f.captures.length, 0, "selection alone is read-only");
  await value(button(f.render(), "prefix.captureModel"), "onClick")();
  assert.equal(JSON.stringify(f.captures), JSON.stringify([{
    provider_id: "registered", expected_provider_revision: 3, model: "configured-model"
  }]));
  assert.equal(value(button(f.render(), "prefix.generateModelDesign"), "disabled"), true);
});

test("pair draft import is exact, scoped and coalesced without role generation or activation", async () => {
  const f=fixture();await new Promise(setImmediate);
  const select=nodes(f.render()).find(node=>node.strings.some(s=>s.includes("<select")));
  value(select,"onChange")({target:{value:JSON.stringify(["registered","active-model"])}});
  await value(button(f.render(),"prefix.captureModel"),"onClick")();
  const raw=' {"format":"brassclaw-prefix-pair-construction-proposal/1","specification":"retained","data":"<script>literal</script>"}\n';
  const file={size:raw.length,arrayBuffer:async()=>new TextEncoder().encode(raw).buffer};
  const input=nodes(f.render()).find(node=>node.strings.some(s=>s.includes('accept=".json,application/json"')));
  const change=value(input,"onChange",'accept=".json,application/json"');const event=()=>({target:{files:[file],value:"picked"}});
  change(event());change(event());await new Promise(setImmediate);
  assert.equal(f.pairImports.length,1);assert.equal(f.pairImports[0].specification,"retained");
  assert.equal(f.pairImports[0].proposal_bytes,raw,"do not reserialize or normalize the draft");
  assert.ok(nodes(f.render()).some(node=>node.values.includes(raw)),"raw proposal is escaped text");
  const panel = nodes(f.render()).find(node=>node.values.includes("PrefixPairCases"));
  assert.ok(panel, "the retained pair proposal exposes its comparison panel");
  assert.equal(value(panel,"specification"), "retained");
  assert.equal(value(panel,"proposal"), "pair-draft");
  assert.equal(value(button(f.render(),"prefix.generateModelDesign"),"disabled"),true);
  assert.equal(f.captures.length,1,"no role/provider recapture from importing a pair draft");
});
test("invalid or oversized pair files are rejected before dispatch and can be retried", async () => {
  const f=fixture();await new Promise(setImmediate);
  const select=nodes(f.render()).find(node=>node.strings.some(s=>s.includes("<select")));
  value(select,"onChange")({target:{value:JSON.stringify(["registered","active-model"])}});
  await value(button(f.render(),"prefix.captureModel"),"onClick")();
  for (const file of [{size:1048577,arrayBuffer:async()=>{throw Error("must not read oversized source");}},
    {size:1,arrayBuffer:async()=>new Uint8Array([255]).buffer},
    {size:5,arrayBuffer:async()=>new Uint8Array([239,187,191,123,125]).buffer},
    {size:2,arrayBuffer:async()=>new TextEncoder().encode("{}").buffer}]) {
    const input=nodes(f.render()).find(node=>node.strings.some(s=>s.includes('accept=".json,application/json"')));
    value(input,"onChange",'accept=".json,application/json"')({target:{files:[file],value:"picked"}});await new Promise(setImmediate);
  }
  assert.equal(f.pairImports.length,0);assert.equal(f.captures.length,1);
  assert.ok(nodes(f.render()).some(node=>node.values.includes("prefix.pairProposalInvalid")));
});


test("retained identities and optional originals preserve exact text and existing child bindings", async () => {
  const specId = "a".repeat(64), pairId = "b".repeat(64), profileId = "c".repeat(64), setId = "d".repeat(64), fileHash = "e".repeat(64);
  const raw = kind => ` {"kind":"${kind}","integer":18446744073709551615,"data":"<script>literal</script>"}\r\n `;
  const spec = { id: specId, provider_id: "<provider>", provider_revision: 7, model: "<model>", specification_bytes: raw("specification"), qualification: "unknown-model-artifacts" };
  const pair = { id: pairId, specification: specId, proposal_bytes: raw("pair"), qualification: "unaccepted-pair-construction-proposal" };
  // Historical content remains literal data; the presentation must not parse it.
  const profile = { id: profileId, profile: "<profile>", specification: specId, proposal_bytes: "historical raw text\r\n<template>not HTML</template>", qualification: "unaccepted-proposal" };
  const set = { id: setId, specification: specId, files: [{name: "config.json", sha256: fileHash, bytes: 123}], binding_bytes: raw("binding"), qualification: "unverified-model-artifacts" };
  const records = { specifications: [spec], pair_proposals: [pair], proposals: [profile], artifact_sets: [set] };
  const originals = JSON.stringify(records);
  const f = fixture({ records }); await new Promise(setImmediate);
  let root = f.render();
  const specDetails = nodes(root).find(node => node.strings[0].startsWith("<details key=") && node.values[0] === specId);
  assert.ok(specDetails);
  value(specDetails, "onToggle")({ currentTarget: { open: true } }); root = f.render();
  const all = nodes(root);
  const hasText = (node, text) => nodes(node).some(part => part.values.includes(text));
  // The existing fixture records htm templates, not parsed DOM elements.
  // Inspect their actual label/value slots without claiming browser rendering.
  const fields = label => all.flatMap(node => node.values.flatMap((item, i) => item === label && node.strings[i].endsWith("<dt>") ? [node.values[i + 1]] : []));
  assert.deepEqual(fields("llm.providerId"), ["<provider>"]);
  assert.deepEqual(fields("llm.model"), ["<model>"]);
  assert.deepEqual(fields("prefix.providerConfigurationRevision"), [7]);
  assert.deepEqual(fields("prefix.specificationId"), [specId, specId, specId, specId]);
  assert.deepEqual(fields("prefix.proposalId"), [pairId, profileId]);
  assert.deepEqual(fields("prefix.artifactSetId"), [setId]);
  for (const [label, text] of [["prefix.originalSpecification", spec.specification_bytes], ["prefix.originalProposal", pair.proposal_bytes], ["prefix.originalProposal", profile.proposal_bytes], ["prefix.originalArtifactBinding", set.binding_bytes]]) {
    const node = all.find(part => part.values.some((item, i) => item === label && part.values[i + 1] === text));
    assert.ok(node, label);
    const i = node.values.indexOf(label, node.values.indexOf(text) - 1);
    assert.match(node.strings[i], /<details>\s*<summary>$/);
    assert.match(node.strings[i + 1], /^<\/summary>\s*<pre [^>]*>$/);
    assert.match(node.strings[i + 2], /^<\/pre>\s*<\/details>/);
    assert.equal(node.values[i + 1], text, "complete original string is a text child, without reserialization");
  }
  const file = all.find(node => node.strings[0].startsWith("<li key="));
  assert.ok(file.values.includes(fileHash)); assert.ok(file.values.includes(123)); assert.ok(file.values.includes("config.json"));
  const setup = all.find(node => node.values.includes("PrefixModelSetupDraft"));
  assert.equal(value(setup, "key"), specId); assert.equal(value(setup, "specification"), spec);
  const cases = all.find(node => node.values.includes("PrefixPairCases"));
  assert.equal(value(cases, "key"), pairId); assert.equal(value(cases, "specification"), specId); assert.equal(value(cases, "proposal"), pairId);
  const inspector = all.find(node => node.values.includes("PrefixModelConfiguration"));
  assert.equal(value(inspector, "specification"), specId); assert.equal(value(inspector, "artifactSet"), set);
  assert.equal(value(button(root, "prefix.generateModelDesign"), "disabled"), true);
  assert.ok(hasText(root, "prefix.unacceptedPairProposal")); assert.ok(hasText(root, "prefix.unacceptedProposal"));
  assert.ok(hasText(root, "prefix.unknownModelArtifacts")); assert.ok(hasText(root, "<profile>"));
  assert.equal(JSON.stringify(records), originals);
  assert.deepEqual([f.captures.length, f.imports.length, f.pairImports.length], [0, 0, 0], "read-only presentation introduces no mutations");
});
