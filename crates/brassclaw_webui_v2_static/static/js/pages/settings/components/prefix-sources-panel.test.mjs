import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";
function fixture(options = {}) {
  const state=[],effects=[],calls=[],saved=[];let cursor=0,dependencies=null,cleanup=null;
  let entry={name:"sempai",description:"Sempai sources"};
  const context={React:{Fragment:"Fragment",
    useState(initial){const i=cursor++;if(!(i in state))state[i]=typeof initial==="function"?initial():initial;return[state[i],value=>{state[i]=typeof value==="function"?value(state[i]):value;}];},
    useRef(initial){const i=cursor++;if(!(i in state))state[i]={current:initial};return state[i];},
    useEffect(effect,deps){if(JSON.stringify(deps)!==dependencies){cleanup?.();dependencies=JSON.stringify(deps);effects.push(()=>{cleanup=effect();});}},
  },html:(strings,...values)=>({strings:[...strings],values}),Button:"Button",Modal:"Modal",
    fetchPrefixSources:async(profile,generation)=>{calls.push({profile,generation});if(options.sources)return options.sources(profile,generation);return{used:generation?[{reference:"archived.md",disposition:"included",reason:"original",checksum:"exact",archived:true}]:[],registered:[{id:"source",title:"Pending source",reference:"https://example.org/v3",availability:"not-checked"}],checks_available:false,check_blocked_reason:"prefix.sourceCheckPrerequisites"};},
    fetchPrefixGenerations:async()=>[{generation:"retained-generation",qualification:"quarantined"}],
    registerPrefixSource:async(profile,request)=>{saved.push({profile,request});if(options.save)return options.save(profile,request);await Promise.resolve();return{};},
  };
  const source=readFileSync(new URL("./prefix-sources-panel.js",import.meta.url),"utf8").split("\n").filter(line=>!line.startsWith("import ")).join("\n").replace("export function","function");
  vm.createContext(context);vm.runInContext(source+"\nglobalThis.render=PrefixSourcesPanel;",context);
  const render=(next)=>{if(next)entry=next;cursor=0;const result=context.render({entry,t:key=>key});while(effects.length)effects.shift()();return result;};
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

function dialog(f) { return nodes(f.render()).find(node=>node.values.includes("Modal")); }
function editReference(f, value) {
  const d=dialog(f);const index=d.strings.findIndex(s=>s.includes('input required value='));
  attribute(d,"onChange",index)({target:{value}});
}
function referenceValue(f) {
  const d=dialog(f);return d.values[d.strings.findIndex(s=>s.includes('input required value='))];
}
test("profile switch never requests a generation owned by the previous profile",async()=>{
  const f=fixture();attribute(f.render(),"onClick")();f.render();await new Promise(setImmediate);
  attribute(dialog(f),"onChange")({target:{value:"retained-generation"}});f.render();await new Promise(setImmediate);
  editReference(f,"https://example.org/sempai-draft");
  const switched=f.render({name:"homeassistant",description:"Home Assistant sources"});
  assert.ok(!nodes(switched).some(node=>node.values.includes("archived.md")),"old inventory must be hidden on the first render");
  await new Promise(setImmediate);
  assert.equal(f.calls.at(-1).profile,"homeassistant");
  assert.equal(f.calls.at(-1).generation,"");
  assert.equal(referenceValue(f),"");
  f.render({name:"sempai",description:"Sempai sources"});
  assert.equal(referenceValue(f),"https://example.org/sempai-draft");
});
test("late source-save success preserves edits made after submission",async()=>{
  let finish;const f=fixture({save:()=>new Promise(resolve=>{finish=resolve;})});
  attribute(f.render(),"onClick")();f.render();await new Promise(setImmediate);
  editReference(f,"https://example.org/submitted");
  const pending=attribute(dialog(f),"onSubmit")({preventDefault(){}});
  editReference(f,"https://example.org/new-draft");finish({});await pending;
  assert.equal(referenceValue(f),"https://example.org/new-draft");
  assert.equal(f.saved[0].request.reference,"https://example.org/submitted");
});
test("late old-profile save failure cannot replace the current inventory error",async()=>{
  let reject;const f=fixture({save:()=>new Promise((resolve,no)=>{reject=no;}),sources:async profile=>{
    if(profile==="homeassistant")throw new Error("HA inventory unavailable");
    return{used:[],registered:[],checks_available:false};
  }});
  attribute(f.render(),"onClick")();f.render();await new Promise(setImmediate);
  editReference(f,"https://example.org/sempai");
  const pending=attribute(dialog(f),"onSubmit")({preventDefault(){}});
  f.render({name:"homeassistant",description:"HA"});await new Promise(setImmediate);
  editReference(f,"https://example.org/ha-draft");
  reject(new Error("old Sempai save failed"));await pending;
  assert.equal(referenceValue(f),"https://example.org/ha-draft");
  const tree=nodes(f.render());assert.ok(tree.some(node=>node.values.includes("HA inventory unavailable")));
  assert.ok(!tree.some(node=>node.values.includes("old Sempai save failed")));
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
