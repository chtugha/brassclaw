import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";
function fixture(options = {}) {
  const state = [], requests = []; let cursor = 0, fail = true;
  let entry = { name: "base-prompt", profile_revision: 1 };
  const effects = new Map(), pendingPassive = [], committed = new Map();
  let planned = [];
  function effectHook(effect, deps, layout) {
    const i=cursor++, previous=effects.get(i);
    if (!previous || JSON.stringify(previous.deps)!==JSON.stringify(deps)) {
      const selected={deps,cleanup:null,layout};
      planned.push({layout,commit:()=>{
        effects.set(i,selected);
        return ()=>{previous?.cleanup?.();selected.cleanup=effect();};
      }});
    }
  }
  const context = { React: {
    useState(initial) { const i=cursor++; if (!(i in state)) state[i]=initial; return [state[i], value => { state[i]=typeof value === "function" ? value(state[i]) : value; }]; },
    useRef(initial) { const i=cursor++; if (!(i in state)) state[i]={current:initial}; return state[i]; },
    useEffect(effect, deps) { effectHook(effect, deps, false); },
    useLayoutEffect(effect, deps) { effectHook(effect, deps, true); },
  }, html:(strings,...values)=>({strings:[...strings],values}), Button:"Button", PrefixMeasurementPanel:"Measurement",
    fetchPrefixGenerations:async name=>options.generations?options.generations(name):(committed.get(name)||[]),
    fetchPrefixGeneration:async(name,generation)=>options.detail?options.detail(name,generation):detail(name,generation),
    importPrefixGeneration:async(name,request)=> {
      requests.push({name,request});
      if(options.import)return options.import(name,request);
      await Promise.resolve();if(fail){fail=false;throw Error("unknown commit");}
      const retained=detail(name,"retained-id",request.generation_bytes);
      committed.set(name,[retained.summary]);return retained;
    },
  };
  const source=readFileSync(new URL("./prefix-generation-history.js",import.meta.url),"utf8").split("\n").filter(s=>!s.startsWith("import ")).join("\n").replace("export function","function");
  vm.createContext(context);vm.runInContext(source+"\nglobalThis.render=PrefixGenerationHistory;",context);
  function flushPassive(){while(pendingPassive.length)pendingPassive.shift()();}
  return {requests,render:(next, phase = {})=>{
    const selected=typeof next==="number"?{...entry,profile_revision:next}:next||entry;
    cursor=0;planned=[];const tree=context.render({entry:selected,t:k=>k});
    // An interrupted render commits neither effect dependencies nor ref fences.
    if(phase.abort){planned=[];return tree;}
    entry=selected;
    for(const effect of planned){const run=effect.commit();if(effect.layout)run();else pendingPassive.push(run);}
    if(!phase.deferPassive)flushPassive();return tree;
  },flushPassive,unmount:(phase={})=>{
    flushPassive();
    for(const effect of effects.values()){
      if(phase.deferPassive&&!effect.layout)pendingPassive.push(()=>effect.cleanup?.());else effect.cleanup?.();
    }
    effects.clear();
  }};
}
function detail(profile,generation,bytes="original bytes") {
  return {summary:{profile,generation,profile_revision:1,kind:"compiler-package",qualification:"quarantined",created_at:"2026-10-31T00:00:00Z"},generation_bytes:bytes};
}
function deferred(){let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no;});return{promise,resolve,reject};}
async function flush(){await new Promise(setImmediate);}
function nodes(root) { const out=[];function visit(v){if(Array.isArray(v)){v.forEach(visit);return;}if(!v?.strings)return;out.push(v);v.values.forEach(visit);}visit(root);return out; }
function value(node,key) { const i=node.strings.findIndex(s=>s.endsWith(`${key}=`));assert.notEqual(i,-1,key);return node.values[i]; }
function button(root,label) { return nodes(root).find(n=>n.values.includes(label)); }
function toggle(f){value(button(f.render(),"prefix.history"),"onClick")();return f.render();}
function choose(f,file){const input=nodes(f.render()).find(n=>n.strings.some(s=>s.includes('type="file"')));return value(input,"onChange")({target:{files:[file]}});}
function inspect(f,generation){const row=nodes(f.render()).find(n=>n.values.includes(generation));assert.ok(row,generation);return value(row,"onClick")();}
function displayed(f,text){return nodes(f.render()).some(n=>n.values.some(v=>typeof v==="string"&&v.includes(text)));}
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

test("closing or unmounting during file reading never starts a late import",async()=>{
  for(const action of ["close","unmount"]){
    const file=deferred(),f=fixture();toggle(f);await flush();
    const pending=choose(f,{size:20,text:()=>file.promise});
    if(action==="close")toggle(f);else f.unmount();
    file.resolve("previous selection bytes");await pending;
    assert.equal(f.requests.length,0,action);
  }
});

test("a confirmed import cannot replace a newer inspected generation",async()=>{
  const writing=deferred(),rows=[detail("base-prompt","A").summary,detail("base-prompt","B").summary];
  const f=fixture({generations:async()=>rows,import:()=>writing.promise});
  toggle(f);await flush();
  const pending=choose(f,{size:20,text:async()=>"A bytes"});await flush();
  await inspect(f,"B");assert.ok(displayed(f,'"generation": "B"'));
  writing.resolve(detail("base-prompt","A","A bytes"));await pending;await flush();
  assert.equal(f.requests.length,1);
  assert.ok(displayed(f,'"generation": "B"'));
  assert.ok(!button(f.render(),"prefix.retryImport"));
});

test("an earlier list snapshot cannot remove a confirmed imported generation",async()=>{
  const older=deferred();let reads=0;
  const retained=detail("base-prompt","retained-id");
  const f=fixture({generations:()=>++reads===1?older.promise:Promise.resolve([retained.summary]),import:async()=>retained});
  toggle(f);
  await choose(f,{size:20,text:async()=>"original bytes"});
  assert.ok(nodes(f.render()).some(n=>n.values.includes("retained-id")));
  older.resolve([]);await flush();
  assert.ok(nodes(f.render()).some(n=>n.values.includes("retained-id")));
  assert.equal(f.requests.length,1);
});

test("a new detail selection clears old data and rejects older errors",async()=>{
  const waiting=deferred(),rows=[detail("base-prompt","A").summary,detail("base-prompt","B").summary];
  const f=fixture({generations:async()=>rows,detail:async(_profile,generation)=>generation==="B"?waiting.promise:detail("base-prompt",generation)});
  toggle(f);await flush();await inspect(f,"A");
  assert.ok(displayed(f,'"generation": "A"'));
  const pending=inspect(f,"B");assert.ok(!displayed(f,'"generation": "A"'));
  toggle(f);toggle(f);await flush();await inspect(f,"A");
  waiting.reject(Error("obsolete B detail error"));await pending;
  assert.ok(displayed(f,'"generation": "A"'));
  assert.ok(!displayed(f,"obsolete B detail error"));
});

test("an old-profile retry is hidden and retains its original target when revisited",async()=>{
  const f=fixture();toggle(f);await flush();
  await choose(f,{size:20,text:async()=>"original bytes"});
  assert.ok(button(f.render(),"prefix.retryImport"));
  const tree=f.render({name:"prefix-maker",profile_revision:2});
  assert.ok(!button(tree,"prefix.retryImport"));
  assert.ok(!nodes(tree).some(n=>n.values.includes("unknown commit")));
  f.render({name:"base-prompt",profile_revision:3});await flush();
  await value(button(f.render(),"prefix.retryImport"),"onClick")();
  assert.equal(f.requests.length,2);
  assert.equal(f.requests[1].name,"base-prompt");
  assert.equal(f.requests[1].request,f.requests[0].request);
  assert.equal(f.requests[1].request.expected_profile_revision,1);
});

test("closing and reopening does not let a previous import select current detail",async()=>{
  const writing=deferred(),rows=[detail("base-prompt","A").summary,detail("base-prompt","B").summary];
  const f=fixture({generations:async()=>rows,import:()=>writing.promise});
  toggle(f);await flush();
  const pending=choose(f,{size:20,text:async()=>"A bytes"});await flush();
  toggle(f);toggle(f);await flush();await inspect(f,"B");
  writing.resolve(detail("base-prompt","A","A bytes"));await pending;await flush();
  assert.ok(displayed(f,'"generation": "B"'));
  assert.ok(!button(f.render(),"prefix.retryImport"));
  assert.equal(f.requests.length,1);
});

test("an obsolete list failure cannot replace the current import error",async()=>{
  const older=deferred(),f=fixture({generations:()=>older.promise,import:async()=>{throw Error("current import uncertain");}});
  toggle(f);
  await choose(f,{size:20,text:async()=>"original bytes"});
  assert.ok(displayed(f,"current import uncertain"));
  older.reject(Error("obsolete inventory error"));await flush();
  assert.ok(displayed(f,"current import uncertain"));
  assert.ok(!displayed(f,"obsolete inventory error"));
  assert.ok(button(f.render(),"prefix.retryImport"));
});

test("an obsolete import error cannot replace the current inspected error",async()=>{
  const writing=deferred(),rows=[detail("base-prompt","B").summary];
  const f=fixture({generations:async()=>rows,import:()=>writing.promise,detail:async()=>{throw Error("current B detail error");}});
  toggle(f);await flush();
  const pending=choose(f,{size:20,text:async()=>"A bytes"});await flush();
  await inspect(f,"B");
  writing.reject(Error("obsolete A import error"));await pending;
  assert.ok(displayed(f,"current B detail error"));
  assert.ok(!displayed(f,"obsolete A import error"));
  assert.ok(button(f.render(),"prefix.retryImport"));
});

test("a slower earlier file read never replaces the latest selected import",async()=>{
  const earlier=deferred(),latest=deferred(),f=fixture();toggle(f);await flush();
  const first=choose(f,{size:20,text:()=>earlier.promise});
  const second=choose(f,{size:20,text:()=>latest.promise});
  latest.resolve("latest bytes");await second;
  earlier.resolve("superseded bytes");await first;
  assert.equal(f.requests.length,1);
  assert.equal(f.requests[0].request.generation_bytes,"latest bytes");
});

test("a committed profile change fences file completion before passive cleanup",async()=>{
  const reading=deferred(),f=fixture();toggle(f);await flush();
  const pending=choose(f,{size:20,text:()=>reading.promise});
  f.render({name:"prefix-maker",profile_revision:2},{deferPassive:true});
  reading.resolve("previous base-prompt bytes");await pending;
  assert.equal(f.requests.length,0,"old file continuation must be fenced at commit, not passive cleanup");
  f.flushPassive();
});

test("an abandoned render does not cancel a still-committed file selection",async()=>{
  const reading=deferred(),f=fixture();toggle(f);await flush();
  const pending=choose(f,{size:20,text:()=>reading.promise});
  f.render({name:"prefix-maker",profile_revision:2},{abort:true});
  reading.resolve("committed base-prompt bytes");await pending;
  assert.equal(f.requests.length,1);
  assert.equal(f.requests[0].name,"base-prompt");
  assert.equal(f.requests[0].request.expected_profile_revision,1);
});

test("unmount commit fences file completion before passive unmount cleanup",async()=>{
  const reading=deferred(),f=fixture();toggle(f);await flush();
  const pending=choose(f,{size:20,text:()=>reading.promise});
  f.unmount({deferPassive:true});
  reading.resolve("old mounted selection bytes");await pending;
  assert.equal(f.requests.length,0);
  f.flushPassive();
});

test("stable profile instances keep separate uncertain original requests",async()=>{
  const a=fixture(),b=fixture();
  toggle(a);b.render({name:"prefix-maker",profile_revision:7});toggle(b);await flush();
  await choose(a,{size:20,text:async()=>"original A bytes"});
  await choose(b,{size:20,text:async()=>"original B bytes"});
  assert.ok(button(a.render(),"prefix.retryImport"));
  assert.ok(button(b.render(),"prefix.retryImport"));
  await value(button(a.render(2),"prefix.retryImport"),"onClick")();
  assert.equal(a.requests.length,2);assert.equal(b.requests.length,1);
  assert.equal(a.requests[1].request,a.requests[0].request);
  assert.equal(a.requests[1].name,"base-prompt");
  assert.equal(a.requests[1].request.expected_profile_revision,1);
  assert.equal(b.requests[0].name,"prefix-maker");
  assert.equal(b.requests[0].request.expected_profile_revision,7);
  assert.ok(button(b.render(),"prefix.retryImport"));
});
