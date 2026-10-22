import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";
const helper = readFileSync(new URL("../lib/prefix-model-configuration.js", import.meta.url), "utf8").replace("export function", "function");
const component = readFileSync(new URL("./prefix-model-configuration.js", import.meta.url), "utf8").split("\n").filter(line => !line.startsWith("import ")).join("\n").replace("export function", "function");
const artifact = { id:"assets", files:[{name:"config.json",sha256:"a".repeat(64),bytes:42}] };
function detail(spec="spec", set=artifact) {
  return { summary:{...set,specification:spec}, configuration_observation:JSON.stringify({
    format:"brassclaw-prefix-model-config-observation/1",specification:spec,artifact_set:set.id,
    config_sha256:set.files[0].sha256,qualification:"declared-config-only",status:"observed-declarations",
    fields:{"/model_type":"<script>data</script>","/quantization_config/bits":4},
    runtime_cache:null,weights_identity:null,serving_context_limit:null,attention_kv_cache:null,recurrent_state:null,
  }) };
}
function fixture() {
  const state=[],effects=[],requests=[]; let cursor=0,effectCursor=0;
  const context={ React:{
    useState(initial){const i=cursor++;if(!(i in state))state[i]=initial;return[state[i],v=>{state[i]=typeof v==="function"?v(state[i]):v;}];},
    useRef(initial){const i=cursor++;if(!(i in state))state[i]={current:initial};return state[i];},
    useEffect(effect,deps){const i=effectCursor++,old=effects[i];if(!old||deps.some((v,j)=>v!==old.deps[j])){old?.cleanup?.();effects[i]={effect,deps};}},
  },html:(strings,...values)=>({strings:[...strings],values}),Button:"Button",
  fetchPrefixModelArtifacts:(specification,id)=>new Promise((resolve,reject)=>requests.push({specification,id,resolve,reject})), };
  vm.createContext(context);vm.runInContext(helper+"\n"+component+"\nglobalThis.render=PrefixModelConfiguration;globalThis.parse=parseModelConfigurationObservation;",context);
  function render(specification="spec",artifactSet=artifact){cursor=0;effectCursor=0;const value=context.render({specification,artifactSet,t:key=>key});for(const record of effects)if(record.effect){const effect=record.effect;record.effect=null;record.cleanup=effect();}return value;}
  render();
  return {render,requests,parse:context.parse,unmount:()=>effects.forEach(record=>record.cleanup?.())};
}
function nodes(root){const out=[];function visit(v){if(Array.isArray(v))v.forEach(visit);else if(v?.strings){out.push(v);v.values.forEach(visit);}}visit(root);return out;}
function click(root){const node=nodes(root).find(node=>node.values.includes("prefix.inspectModelConfig"));const i=node.strings.findIndex(s=>s.endsWith("onClick="));return node.values[i]();}
function contains(root,text){return nodes(root).some(node=>node.values.some(value=>typeof value==="string"&&value.includes(text)));}

test("inspection is explicit, pinned, deduplicated and displays declarations as text",async()=>{
  const f=fixture();assert.equal(f.requests.length,0);const first=click(f.render());const duplicate=click(f.render());assert.equal(f.requests.length,1);
  assert.equal(f.requests[0].specification,"spec");assert.equal(f.requests[0].id,"assets");
  const response=detail();response.configuration_observation=response.configuration_observation.replace('"/quantization_config/bits":4','"/quantization_config/bits":18446744073709551615');
  f.requests[0].resolve(response);await Promise.all([first,duplicate]);
  assert.ok(contains(f.render(),"declared-config-only"));assert.ok(contains(f.render(),"<script>data</script>"));
  assert.ok(contains(f.render(),'"runtime_cache":null'));assert.ok(contains(f.render(),"18446744073709551615"),"observation JSON is displayed without lossy browser number reserialization");await click(f.render());assert.ok(!contains(f.render(),"declared-config-only"));
  await click(f.render());assert.equal(f.requests.length,1);assert.ok(contains(f.render(),"declared-config-only"));
});
test("late responses cannot replace another model's inspection",async()=>{
  const f=fixture();const old=click(f.render());f.render("other");const current=click(f.render("other"));assert.equal(f.requests.length,2);
  f.requests[0].resolve(detail());await old;assert.ok(!contains(f.render("other"),"declared-config-only"));
  f.requests[1].resolve(detail("other"));await current;assert.ok(contains(f.render("other"),'"specification":"other"'));
});
test("mismatched identities and invented runtime claims fail closed; a failed read can be retried",async()=>{
  const f=fixture();for(const mutate of [v=>v.summary.id="wrong",v=>v.summary.files=[{...artifact.files[0],sha256:"b".repeat(64)}],v=>{const r=JSON.parse(v.configuration_observation);r.runtime_cache={dtype:"int4"};v.configuration_observation=JSON.stringify(r);},v=>v.configuration_observation="bad JSON"]){const value=detail();mutate(value);assert.throws(()=>f.parse(value,"spec",artifact));}
  const pending=click(f.render());f.requests[0].resolve({...detail(),configuration_observation:"bad JSON"});await pending;assert.ok(!contains(f.render(),"declared-config-only"));
  const retry=click(f.render());assert.equal(f.requests.length,2);f.requests[1].resolve(detail());await retry;assert.ok(contains(f.render(),"declared-config-only"));
});
test("closing the owning component ignores the in-flight read",async()=>{
  const f=fixture();const pending=click(f.render());f.unmount();f.requests[0].resolve(detail());await pending;assert.ok(!contains(f.render(),"declared-config-only"));
});
