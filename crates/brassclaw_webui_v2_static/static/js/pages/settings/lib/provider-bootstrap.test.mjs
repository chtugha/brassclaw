// Pure UI parsing and controlled hook/transport scheduling only. This is no
// PostgreSQL, Monty, credential, semantic qualification or browser acceptance.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";
const sourceBase = process.env.PROVIDER_TEST_SOURCE_BASE || import.meta.url;
const load = (path, names, bindings = {}) => vm.runInNewContext("(function(){" + readFileSync(path === "./provider-mutations.js" && process.env.PROVIDER_MUTATION_SOURCE ? process.env.PROVIDER_MUTATION_SOURCE : new URL(path, sourceBase), "utf8").split("\n").filter(line => !line.startsWith("import ")).join("\n").replaceAll("export ", "") + ";return {" + names.join(",") + "};})()", bindings);
const api = load("./settings-api.js", ["validatePrefixRevision"]);
const setup = load("./model-setup-draft.js", ["parseSetupDraft"]);
const parser = load("./provider-bootstrap.js", ["providerBootstrapTarget", "providerBootstrapStatus"], { ...api, ...setup });
const id = n => `00000000-0000-4000-8000-${String(n).padStart(12,"0")}`;
function receipt(n = 1, revision = "0") { return { mutation_id:id(n), bootstrap_job_id:id(n+10), bootstrap_target:{target_kind:"model_setup",specification:(n===1?"a":"b").repeat(64),setup_revision:revision} }; }
function status(target) {
  const fields=Object.fromEntries(["max_prefix_tokens","max_prompt_tokens","reserved_output_tokens","additional_framing_tokens"].map(name=>[name,{value:null,origin:"unset"}]));
  fields.max_prefix_tokens={value:0,origin:"manual"};fields.max_prompt_tokens={value:null,origin:"manual"};
  return {job_id:target.job,target_kind:"model_setup",specification:target.specification,setup_revision:target.revision,operation:"bootstrap_setup",mode:"target",admission_phase:"settled",run_status:"completed",
    internal_receipt:JSON.stringify({format:"brassclaw-bootstrap-setup-result/1",job_id:target.job,origin_mutation:target.mutation,target:{specification:target.specification},executing_pair:{specification:target.specification},setup:{specification:target.specification,revision:Number(target.revision),operation_id:target.revision==="0"?null:id(99),fields},readiness:{scope:"configuration-prefill-only",generate:"blocked",design_acceptance:"unknown",serving:"unmeasured"}})};
}
test("original target preserves zero, safe upper boundary and historical NULL",()=>{
  for (const revision of [0,"0",Number.MAX_SAFE_INTEGER,String(Number.MAX_SAFE_INTEGER)]) assert.equal(parser.providerBootstrapTarget(receipt(1,revision)).revision,String(revision));
  assert.equal(parser.providerBootstrapTarget({mutation_id:id(1)}),null);
  assert.equal(parser.providerBootstrapTarget({bootstrap_target:null,bootstrap_job_id:null}),null);
});
test("partial, foreign and inexact bootstrap targets are rejected",()=>{
  for (const revision of [null,"00",-1,1.5,Number.MAX_SAFE_INTEGER+1,"9007199254740992","1e1",true]) assert.throws(()=>parser.providerBootstrapTarget(receipt(1,revision)));
  for (const change of [{bootstrap_job_id:null},{bootstrap_target:null},{bootstrap_job_id:id(0).replace("4000","0000").replace("8000","0000")}]) assert.throws(()=>parser.providerBootstrapTarget({...receipt(),...change}));
});
test("result is bound to original job/setup and preserves manual null/zero",()=>{
  const target=parser.providerBootstrapTarget(receipt(1,"1")), parsed=parser.providerBootstrapStatus(target,status(target));
  assert.equal(parsed.setup.fields.max_prefix_tokens.value,0);assert.equal(parsed.setup.fields.max_prompt_tokens.value,null);assert.equal(parsed.setup.fields.max_prompt_tokens.origin,"manual");
  for (const change of [{job_id:id(40)},{setup_revision:"2"},{specification:"b".repeat(64)},{operation:"generate"}]) assert.throws(()=>parser.providerBootstrapStatus(target,{...status(target),...change}));
  assert.equal(parser.providerBootstrapStatus(target,{...status(target),internal_receipt:null}),null);
});
function ui() {
  const states=[],pending=[];let cursor=0,listener,cleanup,mounted=false,writes=0;
  const context={...parser,Button:"Button",PrefixModelSetupDraft:"Setup",html:(strings,...values)=>({strings:[...strings],values}),
    subscribeProviderBootstrap:cb=>{listener=cb;return()=>{listener=null;};},
    fetchPrefixModelSetupJob:(...args)=>new Promise((resolve,reject)=>pending.push({args,resolve,reject})),
    React:{useState(initial){const i=cursor++;if(!(i in states))states[i]=typeof initial==="function"?initial():initial;return[states[i],value=>{writes++;states[i]=typeof value==="function"?value(states[i]):value;}];},useRef(initial){const i=cursor++;if(!(i in states))states[i]={current:initial};return states[i];},useEffect(effect){if(!mounted){cleanup=effect();mounted=true;}}}
  };
  const {ProviderBootstrapStatus}=load("../components/provider-bootstrap-status.js",["ProviderBootstrapStatus"],context);
  const render=()=>{cursor=0;return ProviderBootstrapStatus({t:key=>key});};render();
  return {render,pending,notify:(r,n)=>listener(r,n),close:()=>cleanup(),writes:()=>writes};
}
function nodes(value,out=[]){if(Array.isArray(value))value.forEach(v=>nodes(v,out));else if(value?.strings){out.push(value);value.values.forEach(v=>nodes(v,out));}return out;}
function click(f,label){const node=nodes(f.render()).find(n=>n.values.includes(label));assert.ok(node,label);const index=node.strings.findIndex(s=>s.endsWith("onClick="));return node.values[index]();}
test("late original replies cannot retarget newer selection or its inspector",async()=>{
  const f=ui();f.notify(receipt(1),1);const old=click(f,"prefix.bootstrapInspect");assert.deepEqual([...f.pending[0].args],["a".repeat(64),"0",id(11)]);
  f.notify(receipt(2,"1"),2);f.notify(receipt(1),1);f.pending[0].resolve(status(parser.providerBootstrapTarget(receipt(1))));await old;
  const current=click(f,"prefix.bootstrapInspect");assert.deepEqual([...f.pending[1].args],["b".repeat(64),"1",id(12)]);
  f.pending[1].resolve(status(parser.providerBootstrapTarget(receipt(2,"1"))));await current;
  assert.ok(nodes(f.render()).some(n=>n.values.includes("prefix.bootstrapScopedResult")));f.close();
});
test("unmount invalidates an in-flight original inspection",async()=>{
  const f=ui();f.notify(receipt(1),1);const work=click(f,"prefix.bootstrapInspect");f.close();const before=f.writes();f.pending[0].resolve(status(parser.providerBootstrapTarget(receipt(1))));await work;assert.equal(f.writes(),before);
});
test("provider transport retains action order across out-of-order original acknowledgements",async()=>{
  const calls=[],observed=[];let next=0;
  const transport=load("./provider-mutations.js",["submitProviderMutation","subscribeProviderBootstrap"],{readStoredToken:()=>"controlled-bootstrap-token",readOriginalAuthContext:()=>({origin:"http://operator.example",nonce:"original"}),originalAuthContextIsCurrent:()=>true,clientActionId:()=>id(++next),apiFetch:(path,options)=>new Promise(resolve=>calls.push({path,options,resolve}))});
  const unsubscribe=transport.subscribeProviderBootstrap((r,order)=>observed.push([r.mutation_id,order]));
  const a=transport.submitProviderMutation("select","a","/active",{provider_id:"a"}),b=transport.submitProviderMutation("select","b","/active",{provider_id:"b"});
  calls[1].resolve({mutation_receipt:{...receipt(2),operation:"select"}});await b;
  calls[0].resolve({mutation_receipt:{...receipt(1),operation:"select"}});await a;
  assert.deepEqual(observed,[[id(2),2],[id(1),1]]);unsubscribe();
});
