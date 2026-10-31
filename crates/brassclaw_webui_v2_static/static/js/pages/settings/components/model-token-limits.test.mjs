import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";
import { modelTokenLimitsPayload } from "../lib/model-token-limits.js";
const source=readFileSync(new URL("./model-token-limits.js",import.meta.url),"utf8").split("\n").filter(line=>!line.startsWith("import ")).join("\n").replace("export function","function");
const initial=()=>({providers:[{id:"local",revision:7,adapter:"open_ai_completions",default_model:"ornith",description:"Local",context_window_tokens:131072,
  model_token_limits:{ornith:{max_prefix_tokens:10,max_prompt_tokens:20},other:{max_prefix_tokens:2,max_prompt_tokens:3}}}]});
const receipt=revision=>({mutation_id:`00000000-0000-4000-8000-${String(revision).padStart(12,"0")}`,operation:"upsert",provider_id:"local",provider_revision:revision,role_selections:{}});
function fixture(shared={data:initial()}) {
  const state=[],effects=[],requests=[];let cursor=0,effectCursor=0;
  const context={React:{
    useState(initial){const i=cursor++;if(!(i in state))state[i]=initial;return[state[i],v=>state[i]=typeof v==="function"?v(state[i]):v];},
    useRef(initial){const i=cursor++;if(!(i in state))state[i]={current:initial};return state[i];},
    useEffect(effect,deps){const i=effectCursor++,old=effects[i];if(!old||deps.some((v,j)=>v!==old.deps[j]))effects[i]={effect,deps,cleanup:old?.cleanup};},
  },useQuery:opts=>{assert.deepEqual([...opts.queryKey],["llm-providers"]);return shared;},
  useQueryClient:()=>({async invalidateQueries(filters,options){assert.deepEqual([...filters.queryKey],["llm-providers"]);assert.equal(filters.exact,true);assert.equal(options.throwOnError,true);shared.invalidations=(shared.invalidations||0)+1;if(shared.refetchError){shared.error=shared.refetchError;throw shared.refetchError;}if(shared.refetchSnapshot)shared.data=shared.refetchSnapshot;},getQueryData(key){assert.deepEqual([...key],["llm-providers"]);return shared.data;},setQueryData(key,value){assert.deepEqual([...key],["llm-providers"]);shared.publications=(shared.publications||0)+1;shared.data=value;}}),
  fetchLlmProviders:()=>{},upsertLlmProvider:payload=>new Promise((resolve,reject)=>requests.push({payload,resolve,reject})),
  modelTokenLimitsPayload,ProviderMutationStatus:"ProviderMutationStatus",Button:"Button",html:(strings,...values)=>({strings:[...strings],values})};
  vm.createContext(context);vm.runInContext(source+"\nglobalThis.render=ModelTokenLimits",context);
  const render=()=>{cursor=0;effectCursor=0;return context.render({providerId:"local",model:"ornith",t:k=>k});};
  function flush(){for(const e of effects)if(e.effect){const fn=e.effect;e.effect=null;e.cleanup=fn();}}
  render();flush();
  return {shared,requests,render(){const n=render();flush();return render();},unmount(){effects.forEach(e=>e.cleanup?.());}};
}
function nodes(node,out=[]){if(Array.isArray(node))node.forEach(v=>nodes(v,out));else if(node&&typeof node==="object"&&node.values){out.push(node);node.values.forEach(v=>nodes(v,out));}return out;}
function change(f,label,value){const node=nodes(f.render()).find(n=>n.values.includes(label));const start=node.values.indexOf(label);const index=node.strings.findIndex((s,i)=>i>=start&&s.includes("onChange="));node.values[index]({target:{value}});}
function save(f){const node=nodes(f.render()).find(n=>n.values.includes("common.save"));assert.ok(node);const label=node.values.indexOf("common.save");const index=node.strings.reduce((found,s,i)=>i<label&&s.endsWith("onClick=")?i:found,-1);assert.ok(index>=0);return node.values[index]();}
function shown(f,key){return nodes(f.render()).some(n=>n.values.includes(key));}
test("provider and prefix editors observe one saved server snapshot without resetting other model limits",async()=>{
  const shared={data:initial()},provider=fixture(shared),prefix=fixture(shared);
  change(provider,"llm.maxPrefixTokens","100000");change(provider,"llm.maxPromptTokens","14688");
  const pending=save(provider);assert.equal(provider.requests.length,1);
  const payload=provider.requests[0].payload;assert.equal(payload.expected_revision,7);
  assert.deepEqual(payload.model_token_limits.other,{max_prefix_tokens:2,max_prompt_tokens:3});
  shared.refetchSnapshot={providers:[{...shared.data.providers[0],revision:8,model_token_limits:payload.model_token_limits}]};
  provider.requests[0].resolve(shared.refetchSnapshot);await pending;
  assert.ok(shown(prefix,100000));assert.ok(shown(prefix,14688));assert.ok(shown(prefix,114688));
  assert.equal(shared.data.providers[0].revision,8);
});
test("a stale draft cannot overwrite another editor and pending writes coalesce",async()=>{
  const f=fixture();change(f,"llm.maxPrefixTokens","100000");change(f,"llm.maxPromptTokens","14688");
  f.shared.data={providers:[{...f.shared.data.providers[0],revision:8}]};await save(f);
  assert.equal(f.requests.length,0);assert.ok(shown(f,"llm.providerChanged"));
  const fresh=fixture();change(fresh,"llm.maxPrefixTokens","100000");change(fresh,"llm.maxPromptTokens","14688");
  const pending=save(fresh);await save(fresh);assert.equal(fresh.requests.length,1);
  const survivor=fixture(fresh.shared);
  fresh.shared.refetchSnapshot={providers:[{...fresh.shared.data.providers[0],revision:8,model_token_limits:fresh.requests[0].payload.model_token_limits}]};
  fresh.unmount();fresh.requests[0].resolve(fresh.shared.refetchSnapshot);await pending;
  assert.ok(shown(survivor,100000));
  assert.equal(fresh.shared.data.providers[0].model_token_limits.ornith.max_prefix_tokens,100000);
});
test("an acknowledged old response cannot replace a newer provider snapshot",async()=>{
  const f=fixture();change(f,"llm.maxPrefixTokens","100000");change(f,"llm.maxPromptTokens","14688");
  const original={...f.shared.data.providers[0]};const pending=save(f);
  f.shared.data={providers:[{...original,revision:9,description:"Later edit"}]};
  f.requests[0].resolve({mutation_receipt:receipt(8),providers:[{...original,revision:8,model_token_limits:f.requests[0].payload.model_token_limits}]});
  await pending;assert.equal(f.shared.data.providers[0].revision,9);assert.ok(shown(f,"llm.providerChangedAfterCommit"));
});
test("a confirmed no-op may keep the same provider definition revision",async()=>{
  const f=fixture();change(f,"llm.maxPrefixTokens","10");change(f,"llm.maxPromptTokens","20");
  const pending=save(f);f.requests[0].resolve({mutation_receipt:receipt(7),providers:[{...f.shared.data.providers[0]}]});
  await pending;assert.equal(f.shared.data.providers[0].revision,7);assert.ok(!shown(f,"llm.modelTokenLimitsUnsupported"));
});

test("a late target response cannot roll back another provider or same-revision role selection",async()=>{
  const f=fixture();change(f,"llm.maxPrefixTokens","100000");change(f,"llm.maxPromptTokens","14688");
  const pending=save(f);
  const target={...f.shared.data.providers[0],revision:8,model_token_limits:f.requests[0].payload.model_token_limits};
  const newest={providers:[target,{id:"other",revision:12}],sempai_active:{provider_id:"local",model:"new-model"}};
  f.shared.data=newest;f.shared.refetchSnapshot=newest;
  f.requests[0].resolve({mutation_receipt:receipt(8),providers:[target,{id:"other",revision:11}],sempai_active:{provider_id:"local",model:"old-model"}});
  await pending;
  assert.deepEqual(f.shared.data,newest);
  assert.equal(f.shared.publications||0,0);
  assert.equal(f.shared.invalidations,1);
});
test("a committed edit with failed canonical refresh keeps the draft and never injects the mutation snapshot",async()=>{
  const f=fixture();change(f,"llm.maxPrefixTokens","100000");change(f,"llm.maxPromptTokens","14688");
  const pending=save(f);f.shared.refetchError=Error("canonical query unavailable");
  f.requests[0].resolve({mutation_receipt:receipt(8),providers:[{...f.shared.data.providers[0],revision:8,model_token_limits:f.requests[0].payload.model_token_limits}]});
  await pending;
  assert.equal(f.shared.data.providers[0].revision,7);
  assert.equal(f.shared.publications||0,0);
  assert.equal(f.requests.length,1);
  assert.ok(shown(f,"100000"));assert.ok(shown(f,"canonical query unavailable"));
  assert.ok(shown(f,"llm.mutationLastCommitted"));assert.ok(shown(f,receipt(8).mutation_id));
});

test("a confirmed publication returned with reload failure remains visible without claiming a current snapshot",async()=>{
  const f=fixture();change(f,"llm.maxPrefixTokens","100000");change(f,"llm.maxPromptTokens","14688");
  const pending=save(f);
  const failure=Error("confirmed publication; runtime reload unavailable");
  failure.committedReceipt={mutation_id:"confirmed-provider-change",operation:"upsert",provider_id:"local",provider_revision:8,role_selections:{}};
  f.shared.refetchSnapshot={providers:[{...f.shared.data.providers[0],revision:8,model_token_limits:f.requests[0].payload.model_token_limits}]};
  f.requests[0].reject(failure);await pending;
  assert.ok(shown(f,"llm.mutationLastCommitted"));assert.ok(shown(f,"confirmed-provider-change"));
  assert.ok(shown(f,failure.message));assert.equal(f.shared.data.providers[0].revision,8);
  assert.equal(f.shared.invalidations,1);assert.equal(f.requests.length,1);
  assert.equal(f.shared.publications||0,0);
});
