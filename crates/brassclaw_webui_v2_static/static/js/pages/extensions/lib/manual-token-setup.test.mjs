import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

function load(relative, bindings, exports) {
  const source=readFileSync(new URL(relative,import.meta.url),"utf8");
  let importing=false;
  const lines=[];
  for (const line of source.split("\n")) {
    if (!importing && line.startsWith("import ")) importing=!line.trimEnd().endsWith(";");
    else if (importing) importing=!line.trimEnd().endsWith(";");
    else lines.push(line.replace(/^export /,""));
  }
  const context=vm.createContext({...bindings});
  vm.runInContext(`${lines.join("\n")}\nglobalThis.exports={${exports.join(",")}};`,context);
  return context.exports;
}
const helpers=load("../../chat/lib/manual-token-operation.js",{clientActionId:randomUUID},[
  "retainExtensionManualTokenOperation","recordExtensionManualTokenSetup","completeExtensionManualTokenOperation",
]);
const {retainExtensionManualTokenOperation:retain,recordExtensionManualTokenSetup:setup,completeExtensionManualTokenOperation:complete}=helpers;
const context={packageId:"github",requirement:"api_token",provider:"github",accountLabel:"github github api_token",invocationId:randomUUID()};
const expiry="2099-01-01T00:00:00Z";
const copy=value=>JSON.parse(JSON.stringify(value));
function storage() {
  const values=new Map();
  return {values,getItem:key=>values.get(key)??null,setItem:(key,value)=>values.set(key,value)};
}
function ack(operation,overrides={}) {
  return {identity:copy(operation.identity),invocation_id:operation.invocationId,provider:context.provider,
    label:context.accountLabel,status:"awaiting_user",expires_at:expiry,...overrides};
}
function completed(operation,overrides={}) {
  return {identity:copy(operation.identity),invocation_id:operation.invocationId,credential_ref:randomUUID(),status:"configured",continuation:{type:"setup_only"},...overrides};
}

test("extension reload/retry retains pair, canonical invocation and original target despite today's account",()=>{
  const saved=storage(),target={kind:"existing",account_id:randomUUID()};
  const original=retain(context,{target,storage:saved});
  const retry=retain(context,{target:{kind:"existing",account_id:randomUUID()},storage:saved});
  assert.deepEqual(copy(retry),copy(original));
  assert.equal(retry.invocationId,context.invocationId);
  assert.deepEqual(copy(retry.definition.target),target);
  assert.equal(saved.values.size,1);
  assert.equal(JSON.stringify([...saved.values]).includes("token_value"),false);
});

test("extension metadata detects changed definition/expiry, malformed storage and no-write retention",()=>{
  const saved=storage(),original=retain(context,{target:{kind:"new"},storage:saved});
  assert.throws(()=>retain({...context,accountLabel:"changed"},{storage:saved}));
  const first=setup(original,context,ack(original),saved);
  assert.throws(()=>setup(first,context,ack(original,{expires_at:"2099-01-02T00:00:00Z"}),saved));
  assert.throws(()=>retain(context,{storage:{getItem:()=>null,setItem:()=>{}},target:{kind:"new"}}));
  const poisoned=copy(first);poisoned.definition.token="token_value";
  saved.setItem(first.key,JSON.stringify(poisoned));
  assert.throws(()=>retain(context,{storage:saved,target:{kind:"new"}}));
});

test("extension cannot mint from reference, replace pending, accept wrong continuation or apply late completion",()=>{
  const saved=storage();
  assert.throws(()=>retain(context,{storage:saved,create:false}));
  const original=retain(context,{storage:saved,target:{kind:"new"}});
  assert.throws(()=>retain(context,{storage:saved,startNew:true,target:{kind:"new"}}));
  const prepared=setup(original,context,ack(original),saved);
  assert.throws(()=>complete(prepared,context,completed(prepared,{continuation:{type:"turn_gate_resume",turn_run_ref:"fake",gate_ref:"fake"}}),saved));
  const closed=complete(prepared,context,completed(prepared),saved);
  assert.equal(closed.closed,true);
  assert.equal(closed.setup.status,"completed");
  const next=retain(context,{storage:saved,startNew:true,target:{kind:"new"}});
  assert.notEqual(next.identity.flow_id,original.identity.flow_id);
  assert.throws(()=>complete(prepared,context,completed(prepared),saved));
});

test("extension API sends raw only to dedicated raw ingress with exact text and no generic secrets",async()=>{
  const calls=[];
  const api=load("./extensions-api.js",{
    apiFetch:async(path,options)=>{calls.push({path,body:JSON.parse(options.body)});return {};},
    setupExtension:async(id,body)=>{calls.push({id,body});return {};},redeemPairingCode:()=>{},
  },["prepareExtensionManualToken","submitExtensionManualToken","continueExtensionManualToken","cancelExtensionManualToken","submitExtensionSetup"]);
  const original=retain(context,{storage:storage(),target:{kind:"new"}});
  await api.prepareExtensionManualToken({id:"github"},original);
  await api.submitExtensionManualToken({id:"github"},original,"  exact ü  ");
  await api.continueExtensionManualToken({id:"github"},{...original,token:"must not leave"});
  await api.cancelExtensionManualToken({id:"github"},original);
  await api.submitExtensionSetup({id:"github"},{workspace:"one"});
  assert.equal(calls[1].body.token,"  exact ü  ");
  assert.equal(calls.filter(call=>Object.hasOwn(call.body,"token")).length,1);
  for(const call of calls.slice(0,4)) assert.deepEqual(copy(call.body.identity),copy(original.identity));
  assert.deepEqual(copy(calls[2].body),{requirement:context.requirement,identity:copy(original.identity),invocation_id:context.invocationId});
  assert.deepEqual(copy(calls[4].body),{action:"submit",payload:{fields:{workspace:"one"}}});
});

function deferred(){let resolve;const promise=new Promise(done=>{resolve=done;});return {promise,resolve};}
function controller({saved=storage(),ports={}}={}) {
  const states=[],refs=[],effects=[];let stateIndex,refIndex,effectIndex,pending=[],tree;
  let props={packageRef:{id:context.packageId},secret:{name:context.requirement,provider:context.provider,
    prompt:"Token",setup:{account_label:context.accountLabel,invocation_id:context.invocationId}}};
  const calls=[],invalidations=[];
  const React={
    useState(initial){const index=stateIndex++;if(!(index in states))states[index]=initial;return[states[index],next=>{states[index]=typeof next==="function"?next(states[index]):next;}];},
    useRef(initial){return refs[refIndex++]??=( {current:initial} );},
    useEffect(fn,deps){const index=effectIndex++,before=effects[index];if(!before||deps.some((value,i)=>value!==before.deps[i]))pending.push(()=>{before?.cleanup?.();effects[index]={deps,cleanup:fn()};});},
  };
  const api={};
  for(const [name,kind] of [["prepareExtensionManualToken","setup"],["submitExtensionManualToken","raw"],["continueExtensionManualToken","reference"],["cancelExtensionManualToken","cancel"]]){
    api[name]=async(...args)=>{calls.push({kind,args});if(ports[kind])return ports[kind](...args);
      if(kind==="setup")return ack(args[1]);
      if(kind==="cancel")return ack(args[1],{status:"canceled"});
      return completed(args[1]);};
  }
  const wrapped={};for(const[name,fn]of Object.entries(helpers))wrapped[name]=(...args)=>{
    if(name==="retainExtensionManualTokenOperation")return fn(args[0],{...args[1],storage:saved});
    return fn(...args,saved);
  };
  const module=load("../components/manual-token-setup.js",{React,html:(strings,...values)=>({strings,values}),
    useT:()=>key=>key,Button:"Button",useQueryClient:()=>({invalidateQueries:value=>invalidations.push(value)}),...wrapped,...api},["ManualTokenSetup"]);
  function render(){stateIndex=0;refIndex=0;effectIndex=0;tree=module.ManualTokenSetup(props);const ready=pending;pending=[];for(const effect of ready)effect();return tree;}
  function click(label){render();const index=tree.values.indexOf(label);assert.ok(index>=0,label);for(let i=index-1;i>=0;i--)if(tree.strings[i].endsWith("onClick="))return tree.values[i]();throw new Error("missing action");}
  render();render();
  return {states,calls,invalidations,saved,click,
    enter(value){states[0]=value;render();},
    switchRequirement(){props={...props,secret:{...props.secret,name:"other"}};render();},
    unmount(){for(const effect of effects)effect?.cleanup?.();},
  };
}

test("actual extension controller reloads lost setup with same IDs then sends exact raw once",async()=>{
  const saved=storage();
  const first=controller({saved,ports:{setup:async()=>{throw new Error("private backend detail");}}});
  first.enter("  exact ü  ");await first.click("extensions.manual.save");
  assert.equal(first.calls.length,1);assert.equal(first.calls[0].kind,"setup");
  assert.equal(first.states[0],"");assert.equal(first.states[2],"extensions.manual.unknown");
  first.unmount();
  const second=controller({saved});second.enter("  exact ü  ");await second.click("extensions.manual.save");
  assert.deepEqual(copy(second.calls[0].args[1].identity),copy(first.calls[0].args[1].identity));
  assert.equal(second.calls[1].kind,"raw");assert.equal(second.calls[1].args[2],"  exact ü  ");
  assert.equal(second.states[3],true);assert.equal(second.states[0],"");
  assert.equal(JSON.stringify([...saved.values]).includes("exact ü"),false);
});

test("actual controller uses reference without raw after lost raw acknowledgement; no auto-replay",async()=>{
  const saved=storage();
  const first=controller({saved,ports:{raw:async()=>{throw new Error("lost after publication");}}});
  first.enter("original");await first.click("extensions.manual.save");first.unmount();
  const second=controller({saved});second.enter("changed but unused");await second.click("extensions.manual.continue");
  assert.deepEqual(second.calls.map(call=>call.kind),["setup","reference"]);
  assert.equal(second.calls[1].args.length,2);
  assert.deepEqual(copy(second.calls[0].args[1].identity),copy(first.calls[0].args[1].identity));
  assert.equal(second.states[0],"");assert.equal(second.states[3],true);
});

test("actual controller fences switched/unmounted context before raw and has no original-creating cancel",async()=>{
  const pending=deferred(),view=controller({ports:{setup:(_package,operation)=>pending.promise.then(()=>ack(operation))}});
  view.enter("private");const work=view.click("extensions.manual.save");view.switchRequirement();pending.resolve();await work;
  assert.deepEqual(view.calls.map(call=>call.kind),["setup"]);assert.equal(view.invalidations.length,0);
  const missing=controller();await missing.click("extensions.manual.cancel");assert.equal(missing.calls.length,0);
  const pending2=deferred(),gone=controller({ports:{setup:(_package,operation)=>pending2.promise.then(()=>ack(operation))}});
  gone.enter("private");const work2=gone.click("extensions.manual.save");gone.unmount();pending2.resolve();await work2;
  assert.deepEqual(gone.calls.map(call=>call.kind),["setup"]);
});

test("actual controller reports completed cancel race honestly and preserves explicit reference",async()=>{
  const saved=storage();retain(context,{storage:saved,target:{kind:"new"}});
  const view=controller({saved,ports:{cancel:(_package,operation)=>ack(operation,{status:"completed"})}});
  await view.click("extensions.manual.cancel");
  assert.equal(view.states[3],false);assert.equal(view.states[2],"extensions.manual.confirmSaved");
  await view.click("extensions.manual.continue");assert.equal(view.states[3],true);
  assert.deepEqual(view.calls.map(call=>call.kind),["cancel","setup","reference"]);
});

test("actual controller blocks raw on authoritative expired original metadata and keeps its identity",async()=>{
  const saved=storage();
  const original=retain(context,{storage:saved,target:{kind:"new"}});
  const view=controller({saved,ports:{setup:(_package,operation)=>ack(operation,{status:"expired",expires_at:"2000-01-01T00:00:00Z"})}});
  view.enter("must never be sent");await view.click("extensions.manual.save");
  assert.deepEqual(view.calls.map(call=>call.kind),["setup"]);
  assert.equal(view.states[3],true);assert.equal(view.states[0],"");
  const retained=retain(context,{storage:saved,create:false});
  assert.deepEqual(copy(retained.identity),copy(original.identity));
  assert.equal(retained.setup.status,"expired");assert.equal(retained.closed,true);
  assert.equal(JSON.stringify([...saved.values]).includes("must never be sent"),false);
});
