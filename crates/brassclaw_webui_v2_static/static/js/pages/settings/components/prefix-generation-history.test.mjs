import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";
function fixture() {
  const state = [], requests = []; let cursor = 0, fail = true;
  const context = { React: {
    useState(initial) { const i=cursor++; if (!(i in state)) state[i]=initial; return [state[i], value => { state[i]=typeof value === "function" ? value(state[i]) : value; }]; },
    useRef(initial) { const i=cursor++; if (!(i in state)) state[i]={current:initial}; return state[i]; }, useEffect() {},
  }, html:(strings,...values)=>({strings:[...strings],values}), Button:"Button",
    fetchPrefixGenerations:async()=>[], fetchPrefixGeneration:async()=>null,
    importPrefixGeneration:async(name,request)=> {requests.push({name,request}); await Promise.resolve(); if(fail){fail=false;throw Error("unknown commit");} return {summary:{generation:"retained-id",qualification:"quarantined"},generation_bytes:request.generation_bytes};},
  };
  const source=readFileSync(new URL("./prefix-generation-history.js",import.meta.url),"utf8").split("\n").filter(s=>!s.startsWith("import ")).join("\n").replace("export function","function");
  vm.createContext(context);vm.runInContext(source+"\nglobalThis.render=PrefixGenerationHistory;",context);
  return {requests,render:(revision=1)=>{cursor=0;return context.render({entry:{name:"base-prompt",profile_revision:revision},t:k=>k});}};
}
function nodes(root) { const out=[];function visit(v){if(Array.isArray(v)){v.forEach(visit);return;}if(!v?.strings)return;out.push(v);v.values.forEach(visit);}visit(root);return out; }
function value(node,key) { const i=node.strings.findIndex(s=>s.endsWith(`${key}=`));assert.notEqual(i,-1,key);return node.values[i]; }
function button(root,label) { return nodes(root).find(n=>n.values.includes(label)); }
test("import failure retains exact request across metadata changes and deduplicates clicks",async()=>{
  const f=fixture();value(button(f.render(),"prefix.history"),"onClick")();
  const input=nodes(f.render()).find(n=>n.strings.some(s=>s.includes('type="file"')));
  const choose=value(input,"onChange");
  await choose({target:{files:[{size:20,text:async()=>"exact compiler bytes"}]}});
  assert.equal(f.requests.length,1);
  const retry=value(button(f.render(2),"prefix.retryImport"),"onClick");
  await Promise.all([retry(),retry()]);
  assert.equal(f.requests.length,2);assert.equal(f.requests[1].request,f.requests[0].request);
  assert.equal(f.requests[1].request.expected_profile_revision,1);
  assert.ok(!button(f.render(2),"prefix.retryImport"));
});
test("oversized transfer fails before reading or posting",async()=>{
  const f=fixture();value(button(f.render(),"prefix.history"),"onClick")();
  const input=nodes(f.render()).find(n=>n.strings.some(s=>s.includes('type="file"')));
  await value(input,"onChange")({target:{files:[{size:65*1024*1024,text:async()=>{throw Error("must not read");}}]}});
  assert.equal(f.requests.length,0);
});
