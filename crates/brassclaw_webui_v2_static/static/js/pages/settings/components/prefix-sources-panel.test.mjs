import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";
function fixture() {
  const state=[],effects=[],calls=[],saved=[];let cursor=0,dependencies=null,cleanup=null;
  const context={React:{Fragment:"Fragment",
    useState(initial){const i=cursor++;if(!(i in state))state[i]=initial;return[state[i],value=>{state[i]=typeof value==="function"?value(state[i]):value;}];},
    useRef(initial){const i=cursor++;if(!(i in state))state[i]={current:initial};return state[i];},
    useEffect(effect,deps){if(JSON.stringify(deps)!==dependencies){cleanup?.();dependencies=JSON.stringify(deps);effects.push(()=>{cleanup=effect();});}},
  },html:(strings,...values)=>({strings:[...strings],values}),Button:"Button",Modal:"Modal",
    fetchPrefixSources:async(profile,generation)=>{calls.push({profile,generation});return{used:generation?[{reference:"archived.md",disposition:"included",reason:"original",checksum:"exact",archived:true}]:[],registered:[{id:"source",title:"Pending source",reference:"https://example.org/v3",availability:"not-checked"}],checks_available:false,check_blocked_reason:"prefix.sourceCheckPrerequisites"};},
    fetchPrefixGenerations:async()=>[{generation:"retained-generation",qualification:"quarantined"}],
    registerPrefixSource:async(profile,request)=>{saved.push({profile,request});await Promise.resolve();return{};},
  };
  const source=readFileSync(new URL("./prefix-sources-panel.js",import.meta.url),"utf8").split("\n").filter(line=>!line.startsWith("import ")).join("\n").replace("export function","function");
  vm.createContext(context);vm.runInContext(source+"\nglobalThis.render=PrefixSourcesPanel;",context);
  const render=()=>{cursor=0;const result=context.render({entry:{name:"sempai",description:"Sempai sources"},t:key=>key});while(effects.length)effects.shift()();return result;};
  return{render,calls,saved};
}
function nodes(root){const out=[];function visit(value){if(Array.isArray(value)){value.forEach(visit);return;}if(!value?.strings)return;out.push(value);value.values.forEach(visit);}visit(root);return out;}
function attribute(node,key,start=0){const i=node.strings.findIndex((s,index)=>index>=start&&s.endsWith(`${key}=`));assert.notEqual(i,-1,key);return node.values[i];}
test("opening Sources reads registered data and exact generation inventory without claiming availability",async()=>{
  const f=fixture();const initial=f.render();assert.equal(f.calls.length,0);
  attribute(initial,"onClick")();f.render();await new Promise(setImmediate);
  assert.deepEqual(f.calls,[{profile:"sempai",generation:""}]);
  const inventory=nodes(f.render()).find(node=>node.values.includes("prefix.usedSources"));
  assert.ok(nodes(f.render()).some(node=>node.values.includes("prefix.sourceCheckPrerequisites")));
  assert.equal(attribute(inventory,"disabled"),true);
  const dialog=nodes(f.render()).find(node=>node.values.includes("Modal"));
  attribute(dialog,"onChange")({target:{value:"retained-generation"}});f.render();await new Promise(setImmediate);
  assert.equal(f.calls.at(-1).generation,"retained-generation");
  assert.ok(nodes(f.render()).some(node=>node.values.includes("archived.md")));
  attribute(dialog,"onClose")();f.render();assert.equal(f.calls.length,2);
});
test("source save uses typed metadata and suppresses repeated submission",async()=>{
  const f=fixture();attribute(f.render(),"onClick")();f.render();await new Promise(setImmediate);
  const dialog=nodes(f.render()).find(node=>node.values.includes("Modal"));
  const reference=dialog.strings.findIndex(s=>s.includes('input required value='));
  attribute(dialog,"onChange",reference)({target:{value:"https://example.org/reference?v=3"}});
  const submit=attribute(nodes(f.render()).find(node=>node.values.includes("Modal")),"onSubmit");
  await Promise.all([submit({preventDefault(){}}),submit({preventDefault(){}})]);
  assert.equal(f.saved.length,1);
  assert.equal(f.saved[0].profile,"sempai");
  assert.equal(f.saved[0].request.reference,"https://example.org/reference?v=3");
  assert.equal(f.saved[0].request.required,false);
  assert.equal(f.saved[0].request.applicability,"Sempai sources");
});
