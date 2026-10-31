// Runs the actual login hook control flow with controlled transport, hooks and
// query refresh. React rendering/backend publication are separate native scopes.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";
import { afterEach } from "node:test";
import { storeToken, readStoredToken, readOriginalAuthContext, originalAuthContextIsCurrent } from "../../../lib/api.js";
import { codexLoginSelection, findCodexLoginOriginal, codexLoginIsTerminal, runCodexLogin } from "../lib/provider-login.js";
const source = readFileSync(new URL("./useProviderLogin.js", import.meta.url), "utf8")
  .replace(/import[\s\S]*?from "[^"]+";\n/g, "").replaceAll("export ", "");
const tick = () => new Promise(resolve => setImmediate(resolve));
function fixture(overrides = {}) {
  const updates = [], opened = [], waits = [], refresh = []; let cleanup; let success = 0, next = 0;
  const context = { React: { useCallback: fn => fn, useRef: initial => ({ current: initial }),
    useEffect: effect => { cleanup = effect(); }, useState: initial => [initial, value => updates.push(value)] },
    useQueryClient: () => ({ invalidateQueries: () => new Promise(resolve => refresh.push(resolve)) }),
    useT: () => value => value, window: { location: { origin: "http://operator.local" }, open: (...args) => opened.push(args) },
    startNearaiLogin: async () => ({ auth_url: "http://issuer.local/authorize", mutation_id: `original-${++next}` }),
    waitForProviderLogin: async (...args) => { waits.push(args); return true; },
    fetchLlmProviders: () => { throw new Error("NEAR login must not poll the active provider name"); },
    startCodexLogin: () => { throw new Error("not this flow"); },
    setTimeout, Date };
  Object.assign(context,overrides);
  vm.createContext(context);
  vm.runInContext(source + "\nglobalThis.create = useProviderLogin;", context);
  const hook = context.create({ onSuccess: () => { success += 1; } });
  return { hook, updates, opened, waits, refresh, unmount: () => cleanup(), successes: () => success };
}
test("current NEAR login waits for its original receipt and completed query refresh", async () => {
  const f = fixture(), pending = f.hook.startNearai("github");
  await tick();
  assert.equal(f.waits[0][0], "original-1");
  assert.equal(f.waits[0][1], "nearai");
  assert.equal(f.successes(), 0);
  f.refresh[0](); await pending;
  assert.equal(f.successes(), 1);
  assert.equal(f.updates.at(-1), false);
});
test("unmount during query refresh cannot trigger stale success navigation", async () => {
  const f = fixture(), pending = f.hook.startNearai("github");
  await tick(); f.unmount(); f.refresh[0](); await pending;
  assert.equal(f.successes(), 0);
});
test("a newer login during query refresh fences the older completion", async () => {
  const f = fixture(), first = f.hook.startNearai("github");
  await tick(); const second = f.hook.startNearai("google");
  await tick();
  f.refresh[0](); await first;
  assert.equal(f.successes(), 0);
  f.refresh[1](); await second;
  assert.equal(f.successes(), 1);
  assert.deepEqual(f.waits.map(args => args[0]), ["original-1", "original-2"]);
});

const savedGlobals = { window:globalThis.window,sessionStorage:globalThis.sessionStorage,fetch:globalThis.fetch };
afterEach(()=>Object.assign(globalThis,savedGlobals));
function codexFixture() {
  const values = new Map(), events = [], popups = [];
  globalThis.sessionStorage = {getItem:key=>values.get(key)??null,setItem:(key,value)=>values.set(key,value),removeItem:key=>values.delete(key)};
  const window = {location:{origin:"http://operator.example"},confirm:()=>true,
    open:()=>{events.push("open");const p={closed:false,location:{href:"about:blank"},close(){this.closed=true;}};popups.push(p);return p;}};
  globalThis.window=window;storeToken("controlled-hook-token");
  let pendingPost = null, request, phases = null, starts = 0;
  globalThis.fetch=async(path,options)=>{
    if(path.endsWith("/codex/login")) {
      events.push("post");request=JSON.parse(options.body);starts++;
      const retained=[...values.entries()].filter(([key])=>key.startsWith("brassclaw:codex-login-original:"));
      assert(retained.some(([,value])=>JSON.parse(value).bytes===options.body));
      if(pendingPost) { const waiting=pendingPost; pendingPost=null; await waiting; }
      const codeFree=phases!==null&&starts>1;
      return Response.json({mutation_id:request.mutation_id,user_code:codeFree?null:"CODE",verification_uri:codeFree?null:"https://issuer.example/device",
        expires_at:"2000-01-01T00:00:00Z",poll_interval_millis:17});
    }
    const receipt={mutation_id:request.mutation_id,operation:"upsert",provider_id:request.provider_id,provider_revision:7,
      role_selections:{"llm.default.provider_id":request.provider_id,"llm.default.model":request.model}};
    if(path.includes("/codex/login/")) {
      const status=phases?.shift()??"completed";
      return Response.json({mutation_id:request.mutation_id,status,expires_at:"2000-01-01T00:00:00Z",
        error:null,mutation_receipt:status==="completed"?receipt:null,poll_interval_millis:17});
    }
    if(path.endsWith("/reconcile"))return Response.json({mutation_receipt:receipt});
    throw Error("unexpected route");
  };
  const f=fixture({window,AbortController,readStoredToken,readOriginalAuthContext,originalAuthContextIsCurrent,
    codexLoginSelection,findCodexLoginOriginal,codexLoginIsTerminal,runCodexLogin});
  const provider={id:"clicked-codex",adapter:"openai_codex",revision:7,default_model:"clicked-model",selection_refs:{kohai:"a".repeat(64),sempai:"b".repeat(64)}};
  return {...f,events,popups,provider,request:()=>request,hold:promise=>{pendingPost=promise;},phases:value=>{phases=value;}};
}
test("Codex opens synchronously, persists before POST and freezes clicked data across awaits",async()=>{
  const f=codexFixture();let release;f.hold(new Promise(resolve=>{release=resolve;}));
  const first=f.hook.startCodex(f.provider);const duplicate=f.hook.startCodex(f.provider);
  assert.equal(first,duplicate);assert.deepEqual(f.events,["open","post"]);assert.equal(f.popups.length,1);
  f.provider.default_model="changed-after-click";f.provider.revision=99;f.provider.selection_refs.kohai="b".repeat(64);
  release();await tick();assert.equal(f.request().model,"clicked-model");assert.equal(f.request().expected_revision,7);
  assert.equal(f.request().expected_selection_ref,"a".repeat(64));
  f.refresh[0]();await first;assert.equal(f.successes(),1);
});
test("Codex late response after unmount or auth change never navigates or succeeds",async()=>{
  for(const change of ["unmount","auth"]) {
    const f=codexFixture();let release;f.hold(new Promise(resolve=>{release=resolve;}));
    const pending=f.hook.startCodex(f.provider);
    if(change==="unmount")f.unmount();else storeToken("different-hook-token");
    release();await pending;
    assert.equal(f.popups[0].location.href,"about:blank");assert.equal(f.popups[0].closed,true);
    assert.equal(f.successes(),0);assert.equal(f.refresh.length,0);
    assert(!f.updates.some(value=>value?.userCode==="CODE"));
  }
});
test("Codex unmount during query refresh cannot trigger stale success",async()=>{
  const f=codexFixture(),pending=f.hook.startCodex(f.provider);
  await tick();assert.equal(f.refresh.length,1);f.unmount();f.refresh[0]();await pending;
  assert.equal(f.successes(),0);
});
test("Codex embedding rejection occurs before popup, storage original or POST",async()=>{
  const f=codexFixture();await f.hook.startCodex(f.provider,"embedding");
  assert.equal(f.popups.length,0);assert.deepEqual(f.events,[]);assert.equal(f.successes(),0);
});

test("Codex terminal status alone never starts a successor and a canceled confirmation preserves the original",async()=>{
  const f=codexFixture(),first=f.hook.startCodex(f.provider);
  await tick();f.refresh[0]();await first;
  const id=f.request().mutation_id,events=[...f.events];
  globalThis.window.confirm=()=>false;
  await f.hook.startCodex(f.provider);
  assert.deepEqual(f.events,events);assert.equal(f.request().mutation_id,id);
  globalThis.window.confirm=()=>true;
  const next=f.hook.startCodex(f.provider);
  await tick();f.refresh[1]();await next;
  assert.notEqual(f.request().mutation_id,id);
});


test("explicit resume supersedes only observation and reattaches the identical original POST",async()=>{
  const f=codexFixture();let release;f.hold(new Promise(resolve=>{release=resolve;}));
  const first=f.hook.startCodex(f.provider);const original=f.request().mutation_id;
  f.phases(["completing","completed"]);
  const resumed=f.hook.retryCodex();
  await tick();assert.equal(f.request().mutation_id,original);assert.deepEqual(f.events,["open","post","open","post"]);
  f.refresh[0]();await resumed;release();await first;
  assert.equal(f.successes(),1);assert.equal(f.popups[1].location.href,"about:blank");assert.equal(f.popups[1].closed,true);
});
