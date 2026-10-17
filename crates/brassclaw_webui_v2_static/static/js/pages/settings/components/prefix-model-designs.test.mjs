import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";
function fixture() {
  const state = [], captures = [], imports = [], effects = []; let cursor = 0;
  const target = { provider_id: "registered", provider_revision: 3, model: "active-model", consumer: "kohai", configured_context_window: null };
  let catalogue = { targets: [target, { ...target, consumer: "sempai" }], specifications: [], proposals: [], design_generation_available: false, blocked_reason: "prefix.modelDesignPrerequisites" };
  const context = { React: {
    useState(initial) { const i=cursor++; if (!(i in state)) state[i]=initial; return [state[i], value => { state[i]=typeof value === "function" ? value(state[i]) : value; }]; },
    useRef(initial) { const i=cursor++; if (!(i in state)) state[i]={current:initial}; return state[i]; },
    useEffect(effect) { if (!effects.length) effects.push(effect); },
  }, html:(strings,...values)=>({strings:[...strings],values}), Button:"Button",
    fetchPrefixModelDesigns: async () => catalogue,
    prepareModelArtifactImport: async files => { await Promise.resolve(); return JSON.stringify({files:files.map(file=>file.name)}); },
    retainPrefixModelArtifacts: async request => { imports.push(request); await Promise.resolve(); catalogue = {...catalogue,artifact_sets:[{id:"asset-set",specification:request.specification,files:[],binding_bytes:"operator provenance",qualification:"unverified-model-artifacts"}]}; },
    capturePrefixModel: async request => { captures.push(request); await Promise.resolve(); catalogue = { ...catalogue, specifications: [{id:"retained",provider_id:request.provider_id,provider_revision:request.expected_provider_revision,model:request.model,specification_bytes:"<script>untrusted</script>",qualification:"unknown-model-artifacts"}] }; },
  };
  const source=readFileSync(new URL("./prefix-model-designs.js",import.meta.url),"utf8").split("\n").filter(line=>!line.startsWith("import ")).join("\n").replace("export function","function");
  vm.createContext(context);vm.runInContext(source+"\nglobalThis.render=PrefixModelDesigns;",context);
  const render=()=>{cursor=0;return context.render({t:key=>key});}; render(); effects[0]();
  return {captures,imports,render};
}
function nodes(root) { const out=[];function visit(v){if(Array.isArray(v)){v.forEach(visit);return;}if(!v?.strings)return;out.push(v);v.values.forEach(visit);}visit(root);return out; }
function value(node,key) { const i=node.strings.findIndex(s=>s.endsWith(`${key}=`));assert.notEqual(i,-1,key);return node.values[i]; }
function button(root,label) { return nodes(root).find(node=>node.values.includes(label)); }
test("selecting an active target is read-only; capture pins revision and deduplicates clicks",async()=>{
  const f=fixture(); await new Promise(setImmediate);
  const select=nodes(f.render()).find(node=>node.strings.some(s=>s.includes("<select")));
  const options=nodes(f.render()).filter(node=>node.strings[0].startsWith("<option key="));
  assert.equal(options.length,1,"Kohai/Sempai same model must not duplicate target choices");
  value(select,"onChange")({target:{value:JSON.stringify(["registered",3,"active-model"])}});
  assert.equal(f.captures.length,0);
  assert.equal(value(button(f.render(),"prefix.generateModelDesign"),"disabled"),true);
  const capture=value(button(f.render(),"prefix.captureModel"),"onClick");
  await Promise.all([capture(),capture()]);
  assert.equal(f.captures.length,1);
  assert.equal(JSON.stringify(f.captures[0]),JSON.stringify({provider_id:"registered",expected_provider_revision:3,model:"active-model"}));
  assert.ok(nodes(f.render()).some(node=>node.values.includes("prefix.unknownModelArtifacts")));
  assert.ok(nodes(f.render()).some(node=>node.values.includes("<script>untrusted</script>")),"record is rendered as escaped text, not executable HTML");
});

test("file import pins the displayed specification, coalesces concurrent events and cannot enable design generation",async()=>{
  const f=fixture(); await new Promise(setImmediate);
  const select=nodes(f.render()).find(node=>node.strings.some(s=>s.includes("<select")));
  value(select,"onChange")({target:{value:JSON.stringify(["registered",3,"active-model"])}});
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
