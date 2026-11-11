// Actual UI modules through controlled fetch/storage. This is not native
// PostgreSQL/issuer/Runtime/Chromium acceptance.
import assert from "node:assert/strict";
import test, { afterEach } from "node:test";
import { randomUUID } from "node:crypto";
import { storeToken, readOriginalAuthContext } from "../../../lib/api.js";
import { codexLoginSelection, findCodexLoginOriginal, runCodexLogin } from "./provider-login.js";
import { subscribeProviderBootstrap } from "./provider-mutations.js";

const originalTimer = globalThis.setTimeout;
const originalStorage = globalThis.sessionStorage, originalWindow = globalThis.window, originalFetch = globalThis.fetch;
afterEach(() => {
  globalThis.setTimeout = originalTimer;
  globalThis.sessionStorage = originalStorage;
  globalThis.window = originalWindow;
  globalThis.fetch = originalFetch;
});
const copy = value => JSON.parse(JSON.stringify(value));
function storage() {
  const values = new Map();
  return { values,getItem:key=>values.get(key)??null,setItem:(key,value)=>values.set(key,value),removeItem:key=>values.delete(key) };
}
const provider = {id:"clicked-codex",adapter:"openai_codex",revision:7,default_model:"clicked-model",
  selection_refs:{kohai:"a".repeat(64),sempai:"b".repeat(64)}};
const expiry = "2000-01-01T00:00:00Z";
function receipt(request, changes={}) {
  const prefix = request.role === "kohai" ? "llm.default" : "llm.sempai";
  return {mutation_id:request.mutation_id,operation:"upsert",provider_id:request.provider_id,provider_revision:7,
    role_selections:{[`${prefix}.provider_id`]:request.provider_id,[`${prefix}.model`]:request.model},...changes};
}
function reply(request,status="completed",changes={}) {
  return {mutation_id:request.mutation_id,status,disposition:status==="completed"?"published":"outcome_unconfirmed",expires_at:expiry,error:null,poll_interval_millis:5001,
    mutation_receipt:status==="completed"?receipt(request):null,...changes};
}
function fixture() {
  const saved = storage(), calls = [], waits = [], events = [];
  globalThis.sessionStorage = saved;
  globalThis.window = {location:{origin:"http://operator.example"}};
  storeToken("controlled-operator-token");
  const context = readOriginalAuthContext(), selection = codexLoginSelection(provider,"sempai");
  let request, rejectPost = false, status = null, failStatus = null, pendingPost = null, foreignReceipt = null, noChallenge = false;
  globalThis.setTimeout = (fn, ms) => { waits.push(ms); return originalTimer(fn,0); };
  globalThis.fetch = async (path,options) => {
    calls.push({path,options});
    if (path.endsWith("/codex/login")) {
      request = JSON.parse(options.body);
      const original = findCodexLoginOriginal(context,selection);
      assert.equal(original.bytes,options.body,"exact original must exist and read back before POST");
      events.push("post");
      if (rejectPost) { rejectPost=false; throw new TypeError("controlled lost acknowledgement"); }
      if (pendingPost) await pendingPost;
      return Response.json({mutation_id:request.mutation_id,expires_at:expiry,poll_interval_millis:5001,
        user_code:noChallenge?null:"USERCODE_NEVER_STORE",verification_uri:noChallenge?null:"https://issuer.example/device"});
    }
    if (path.includes("/codex/login/")) {
      events.push("read");
      if (failStatus) return new Response("controlled read failure",{status:failStatus});
      if (!request) return new Response("not found",{status:404});
      const value = Array.isArray(status)?status.shift():status;
      return Response.json(typeof value==="function"?value(request):value??reply(request));
    }
    if (path.endsWith("/reconcile")) {
      events.push("reconcile");
      return Response.json({mutation_receipt:receipt(request,foreignReceipt??{})});
    }
    throw new Error(`unexpected route ${path}`);
  };
  return {saved,calls,waits,events,context,selection,
    run:options=>runCodexLogin(context,selection,options),
    losePost:()=>{rejectPost=true;}, noChallenge:()=>{noChallenge=true;}, statuses:value=>{status=value;}, failReads:value=>{failStatus=value;},
    deferPost:promise=>{pendingPost=promise;}, foreignReconcile:value=>{foreignReceipt=value;},
    original:()=>findCodexLoginOriginal(context,selection),request:()=>request};
}

test("Codex uses exact clicked request/CAS/role and only its own status plus receipt",async()=>{
  const f=fixture(); let code;
  const result=await f.run({onCode:value=>{code=value;}});
  assert.equal(result.completed,true);
  assert.deepEqual(f.events,["post","read","reconcile"]);
  assert.deepEqual({...f.request(),mutation_id:"original"},{...f.selection,mutation_id:"original"});
  assert.equal(code.userCode,"USERCODE_NEVER_STORE");
  for(const [key,value] of f.saved.values) if(key.startsWith("brassclaw:codex-login-original:")) {
    assert(!value.includes("USERCODE_NEVER_STORE"));
    assert(!value.includes("issuer.example"));
    assert(!value.includes("controlled-operator-token"));
  }
});

test("lost start ACK retains exact bytes across retry and never adopts a new model/revision",async()=>{
  const f=fixture(); f.losePost();
  await assert.rejects(f.run());
  const original=f.original(),first=f.calls[0].options.body;
  const changed=codexLoginSelection({...provider,revision:99,default_model:"different-model"},"sempai");
  const result=await runCodexLogin(f.context,changed);
  assert.equal(result.completed,true);
  const starts=f.calls.filter(call=>call.options.method==="POST"&&call.path.endsWith("/codex/login"));
  assert.equal(starts.length,1);assert.equal(starts[0].options.body,first);
  assert.equal(f.original().bytes,original.bytes);
  assert.deepEqual(f.events,["post","read","reconcile"],"known completed status needs no new issuer work");
});

test("404 retry posts only original bytes/ID; transport error permits no replacement",async()=>{
  const f=fixture(); f.losePost(); await assert.rejects(f.run()); const original=f.original();
  f.failReads(503); await assert.rejects(f.run());
  assert.equal(f.calls.filter(c=>c.path.endsWith("/codex/login")).length,1);
  f.failReads(404); // Original may be absent after the first request never arrived.
  let reads=0;const fetch=globalThis.fetch;
  globalThis.fetch=(path,opts)=>{ if(path.includes("/codex/login/")&&++reads>1) f.failReads(null);return fetch(path,opts); };
  const result=await f.run();assert.equal(result.completed,true);
  const starts=f.calls.filter(c=>c.path.endsWith("/codex/login"));
  assert.equal(starts.length,2);assert.equal(starts[0].options.body,starts[1].options.body);
  assert.equal(f.original().bytes,original.bytes);
});

test("explicit Completing resume after expiry reattaches same POST; polling stays GET at server interval",async()=>{
  const f=fixture(); f.losePost();await assert.rejects(f.run());const original=f.original();let codes=0;
  f.noChallenge();f.statuses([r=>reply(r,"completing"),r=>reply(r,"completing"),r=>reply(r)]);
  const result=await f.run({retryOnly:true,onCode:code=>{if(code)codes++;}});
  assert.equal(result.completed,true);assert.equal(codes,0);
  assert.equal(f.original().bytes,original.bytes);
  const starts=f.calls.filter(c=>c.path.endsWith("/codex/login"));assert.equal(starts.length,2);
  assert.equal(starts[0].options.body,starts[1].options.body);
  assert(f.waits.length>0);assert(f.waits.every(ms=>ms===5001));
  assert.equal(f.events.at(-1),"reconcile");
});

test("terminal failure is visible; only an explicitly requested successor gets a new original",async()=>{
  const f=fixture(); f.statuses(r=>reply(r,"failed",{error:"provider_denied"}));
  const failed=await f.run();assert.equal(failed.completed,false);assert.equal(failed.status.error,"provider_denied");
  const old=f.original().bytes;const same=await f.run();assert.equal(same.completed,false);assert.equal(f.original().bytes,old);
  assert.equal(f.calls.filter(c=>c.path.endsWith("/codex/login")).length,1);
  f.statuses([r=>reply(r,"failed",{error:"provider_denied"}),r=>reply(r)]);
  const done=await f.run({newOriginal:true});assert.equal(done.completed,true);
  assert.notEqual(f.original().bytes,old);
});

test("nonterminal state, unknown storage shape and embedding never authorize another original",async()=>{
  const f=fixture();f.losePost();await assert.rejects(f.run());const old=f.original().bytes;
  f.statuses(r=>reply(r,"completing"));await assert.rejects(f.run({newOriginal:true}));assert.equal(f.original().bytes,old);
  assert.throws(()=>codexLoginSelection(provider,"embedding"));
  const key=[...f.saved.values.keys()].find(k=>k.startsWith("brassclaw:codex-login-original:"));
  f.saved.values.set(key,"null");await assert.rejects(f.run());
  assert.equal(f.calls.filter(c=>c.path.endsWith("/codex/login")).length,1);
});

test("foreign status/receipt or missing receipt never becomes success/reconciliation",async()=>{
  for(const bad of [r=>reply(r,"completed",{mutation_id:randomUUID()}),r=>reply(r,"completed",{mutation_receipt:null}),
    r=>reply(r,"completed",{mutation_receipt:receipt(r,{provider_id:"foreign"})}),
    r=>reply(r,"completed",{mutation_receipt:receipt(r,{role_selections:{"llm.sempai.provider_id":r.provider_id,"llm.sempai.model":"foreign"}})}),
    r=>reply(r,"completed",{mutation_receipt:receipt(r,{provider_revision:{bad:"revision"}})}),
    r=>reply(r,"completed",{mutation_receipt:receipt(r,{provider_revision:0})}),
    r=>reply(r,"completing",{poll_interval_millis:0}),
    r=>reply(r,"completed",{disposition:"outcome_unconfirmed"}),
    r=>reply(r,"failed",{disposition:"published"}),
    r=>reply(r,"failed",{disposition:"raw-issuer-body"})]) {
    const f=fixture();f.statuses(bad);await assert.rejects(f.run());assert(!f.events.includes("reconcile"));
  }
  const f=fixture();f.foreignReconcile({mutation_id:randomUUID()});await assert.rejects(f.run());
});

test("late response after context/unmount fencing cannot display code or reconcile",async()=>{
  const f=fixture();let resolve,current=true,codes=0;
  f.deferPost(new Promise(done=>{resolve=done;}));
  const pending=f.run({isCurrent:()=>current,onCode:code=>{if(code)codes++;}});
  await new Promise(done=>setImmediate(done));current=false;resolve();
  const result=await pending;assert.equal(result.completed,false);assert.equal(codes,0);
  assert.deepEqual(f.events,["post"]);assert.ok(f.original());
});

test("verified auth context persists across identical token/reload and isolates token/instance changes",async()=>{
  const f=fixture(); const old=copy(f.context);storeToken("controlled-operator-token");
  assert.deepEqual(readOriginalAuthContext(),old);
  storeToken("different-controlled-token");const next=readOriginalAuthContext();assert.notEqual(next.nonce,old.nonce);
  globalThis.window.location.origin="http://different-instance.example";
  assert.notDeepEqual(readOriginalAuthContext(),next);
});

test("unavailable storage and inconsistent writeback block a new POST",async()=>{
  for(const mode of ["unavailable","dropped-write"]) {
    const f=fixture();const real=f.saved.setItem;
    f.saved.setItem=(key,value)=>{if(key.startsWith("brassclaw:codex-login-original:")) {if(mode==="unavailable")throw Error("denied");return;}real(key,value);};
    await assert.rejects(f.run());assert.equal(f.calls.length,0);
  }
});


test("the same original may observe issuer-shortened expiry without mutating request bytes",async()=>{
  const f=fixture();f.statuses([r=>reply(r,"pending",{expires_at:"2099-01-01T00:00:00Z"}),
    r=>reply(r,"completing",{expires_at:expiry}),r=>reply(r)]);
  const result=await f.run();assert.equal(result.completed,true);
  assert(!Object.hasOwn(f.request(),"expires_at"));
  assert.equal(f.original().observation.expires_at,expiry);
  assert.equal(f.calls.filter(c=>c.path.endsWith("/codex/login")).length,1);
});


test("code-free Pending/Completing/AwaitingUser are separate observations, not a fake challenge or automatic POST",async()=>{
  const f=fixture();f.noChallenge();let codes=0;
  f.statuses([r=>reply(r,"pending"),r=>reply(r,"completing"),r=>reply(r,"awaiting_user"),r=>reply(r)]);
  const result=await f.run({onCode:code=>{if(code)codes++;}});
  assert.equal(result.completed,true);assert.equal(codes,0);
  assert.equal(f.calls.filter(c=>c.path.endsWith("/codex/login")).length,1);
});

test("auth-context write failure remains invalid and blocks any new original POST",async()=>{
  const f=fixture(),write=f.saved.setItem;let writes=0;
  f.saved.setItem=(key,value)=>{
    if(key==="brassclaw_auth_original_context"&&++writes===2)return;
    write(key,value);
  };
  assert.throws(()=>storeToken("changed-after-context-marker"));
  assert.throws(()=>readOriginalAuthContext());
  const result=await f.run();assert.equal(result.completed,false);assert.equal(f.calls.length,0);
});

test("a foreign auth context cannot readopt an old original under a different token",async()=>{
  const f=fixture();f.losePost();await assert.rejects(f.run());const old=f.original().bytes;
  storeToken("other-operator-context");const result=await f.run();assert.equal(result.completed,false);
  assert.equal(f.calls.length,1);assert.equal(f.original().bytes,old);
});

test("half-present, empty or dangerous challenge pairs are rejected before code display",async()=>{
  for(const challenge of [{user_code:null,verification_uri:"https://issuer.example/device"},
    {user_code:"CODE",verification_uri:null},{user_code:"",verification_uri:"https://issuer.example/device"},
    {user_code:"CODE",verification_uri:"javascript:alert(1)"}]) {
    const f=fixture(),fetch=globalThis.fetch;let codes=0;
    globalThis.fetch=async(path,options)=>{
      const response=await fetch(path,options);
      if(!path.endsWith("/codex/login"))return response;
      return Response.json({...await response.json(),...challenge});
    };
    await assert.rejects(f.run({onCode:()=>{codes++;}}));assert.equal(codes,0);assert(!f.events.includes("reconcile"));
  }
});

test("legacy authenticated sessions initialize one verified nonsecret context without changing the token",()=>{
  const saved=storage();globalThis.sessionStorage=saved;globalThis.window={location:{origin:"http://operator.example"}};
  saved.values.set("brassclaw_token","legacy-controlled-token");
  const first=readOriginalAuthContext(),second=readOriginalAuthContext();
  assert.deepEqual(first,second);assert.equal(saved.values.get("brassclaw_token"),"legacy-controlled-token");
  assert(!saved.values.get("brassclaw_auth_original_context").includes("legacy-controlled-token"));
  saved.values.delete("brassclaw_auth_original_context");saved.setItem=()=>{};
  assert.throws(()=>readOriginalAuthContext(),/verified/);
});

test("Codex rejects a foreign reconciliation receipt before bootstrap notification", async () => {
  const f = fixture(), notices = [], unsubscribe = subscribeProviderBootstrap(value => notices.push(value));
  try {
    f.foreignReconcile({ provider_id: "foreign-provider" });
    await assert.rejects(f.run(), /could not be verified/);
    assert.deepEqual(notices, []);
    assert.equal(f.calls.filter(call => call.path.endsWith("/codex/login")).length, 1);
  } finally { unsubscribe(); }
});

test("late Codex reconciliation after unmount, auth change or abort cannot publish bootstrap", async () => {
  for (const change of ["unmount", "auth", "abort"]) {
    const f = fixture(), notices = [], abort = new AbortController(); let current = true, release, arrived;
    const waiting = new Promise(resolve => { release = resolve; });
    const started = new Promise(resolve => { arrived = resolve; });
    const fetch = globalThis.fetch;
    globalThis.fetch = async (path, options) => {
      const response = await fetch(path, options);
      if (path.endsWith("/reconcile")) { arrived(); await waiting; }
      return response;
    };
    const unsubscribe = subscribeProviderBootstrap(value => notices.push(value));
    try {
      const pending = f.run({ isCurrent: () => current, signal: abort.signal });
      await started;
      if (change === "auth") storeToken("changed-during-reconciliation");
      else if (change === "abort") abort.abort();
      else current = false;
      release();
      assert.equal((await pending).completed, false);
      assert.deepEqual(notices, []);
      const reconcile = f.calls.find(call => call.path.endsWith("/reconcile"));
      assert.equal(reconcile.options.signal, abort.signal);
      assert.equal(f.calls.filter(call => call.path.endsWith("/codex/login")).length, 1);
    } finally { release(); unsubscribe(); }
  }
});

test("a Codex observer superseded by its terminal-status callback starts no reconciliation", async () => {
  const f = fixture(); let current = true;
  const result = await f.run({ isCurrent: () => current, onStatus: value => {
    if (value.status === "completed") current = false;
  } });
  assert.equal(result.completed, false);
  assert.deepEqual(f.events, ["post", "read"]);
});

test("an exact current Codex completion publishes its original bootstrap receipt once", async () => {
  const f = fixture(), notices = [], unsubscribe = subscribeProviderBootstrap(value => notices.push(value));
  try {
    const result = await f.run();
    assert.equal(result.completed, true);
    assert.equal(notices.length, 1);
    assert.equal(notices[0].mutation_id, f.request().mutation_id);
    assert.equal(notices[0].provider_id, f.selection.provider_id);
  } finally { unsubscribe(); }
});
