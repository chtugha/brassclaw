// Actual settings API -> mutation helper -> apiFetch, with controlled browser
// storage and HTTP responses. This does not certify native PG/issuer behavior.
import assert from "node:assert/strict";
import test, { afterEach } from "node:test";
import { storeToken } from "../../../lib/api.js";
import { upsertLlmProvider } from "./settings-api.js";
import { pendingProviderMutations, readProviderMutation, reconcileProviderMutation, subscribeProviderBootstrap } from "./provider-mutations.js";

const saved = { sessionStorage:globalThis.sessionStorage, window:globalThis.window, fetch:globalThis.fetch };
afterEach(() => Object.assign(globalThis,saved));
const tick = () => new Promise(resolve => setImmediate(resolve));
const body = { id:"auth-context-provider",adapter:"open_ai_completions",expected_revision:4,api_key:"first-context-private-key" };
function fixture() {
  const values = new Map(), calls = [];
  globalThis.sessionStorage = {getItem:key=>values.get(key)??null,setItem:(key,value)=>values.set(key,value),removeItem:key=>values.delete(key)};
  globalThis.window = {location:{origin:"http://operator.example"}};
  storeToken("first-context-bearer");
  globalThis.fetch = (path,options) => new Promise((resolve,reject) => calls.push({path,options,resolve,reject}));
  return {calls,values};
}
const request = call => JSON.parse(call.options.body);
const receipt = id => ({mutation_id:id,operation:"upsert",provider_id:body.id,provider_revision:5,role_selections:{}});

test("actual settings caller rejects a late old-context commit before bootstrap or recovery", async () => {
  const f=fixture(), notices=[], unsubscribe=subscribeProviderBootstrap(value=>notices.push(value));
  try {
    const pending=upsertLlmProvider(body), id=request(f.calls[0]).mutation_id;
    assert.equal(f.calls[0].options.headers.get("Authorization"),"Bearer first-context-bearer");
    storeToken("next-context-bearer");
    f.calls[0].resolve(Response.json({mutation_receipt:receipt(id)}));
    await assert.rejects(pending,error=>error.mutationId===id&&/Authentication changed/.test(error.message));
    assert.equal(f.calls.length,1); assert.deepEqual(notices,[]);
  } finally {unsubscribe();}
});

test("actual settings recovery never automatically borrows a successor bearer; explicit reference inspection stays available", async () => {
  for (const where of ["post","read","reconcile"]) {
    const f=fixture(), pending=upsertLlmProvider(body), id=request(f.calls[0]).mutation_id;
    if(where!=="post") {
      f.calls[0].reject(new TypeError("controlled lost ACK")); await tick();
      if(where==="reconcile") {f.calls[1].resolve(Response.json(receipt(id))); await tick();}
    }
    const last=f.calls.at(-1), count=f.calls.length;
    storeToken("next-context-bearer");
    if(where==="post")last.reject(new TypeError("controlled lost ACK"));
    else last.resolve(Response.json(where==="read"?receipt(id):{mutation_receipt:receipt(id)}));
    await assert.rejects(pending,/Authentication changed/);
    assert.equal(f.calls.length,count);
    assert(f.calls.every(call=>call.options.headers.get("Authorization")==="Bearer first-context-bearer"));
    const read=readProviderMutation(id);
    assert.equal(f.calls[count].options.headers.get("Authorization"),"Bearer next-context-bearer");
    assert.equal(f.calls[count].options.body,undefined);
    f.calls[count].resolve(Response.json(receipt(id)));await read;
    const reconcile=reconcileProviderMutation(id);
    assert.equal(f.calls[count+1].options.headers.get("Authorization"),"Bearer next-context-bearer");
    assert.equal(f.calls[count+1].options.body,undefined);
    f.calls[count+1].resolve(Response.json({mutation_receipt:receipt(id)}));await reconcile;
    assert.equal(f.calls.length,count+2);
  }
});

test("an explicit new-context edit cannot reuse the old original identity or API-key bytes", async () => {
  const f=fixture(), old=upsertLlmProvider(body), oldId=request(f.calls[0]).mutation_id;
  storeToken("next-context-bearer");
  const next=upsertLlmProvider({...body,api_key:"next-context-private-key"});
  const nextId=request(f.calls[1]).mutation_id;
  assert.notEqual(nextId,oldId);
  assert.equal(f.calls[1].options.headers.get("Authorization"),"Bearer next-context-bearer");
  assert.equal(request(f.calls[1]).api_key,"next-context-private-key");
  assert(!f.calls[1].options.body.includes(body.api_key));
  assert.equal(pendingProviderMutations().find(item=>item.id===nextId).committed,false);
  f.calls[0].resolve(Response.json({mutation_receipt:receipt(oldId)}));
  await assert.rejects(old,/Authentication changed/);
  f.calls[1].resolve(Response.json({mutation_receipt:receipt(nextId)}));await next;
  for (const value of f.values.values()) {
    assert(!value.includes(body.api_key));assert(!value.includes("next-context-private-key"));
  }
});

test("logout blocks a new actual settings write before HTTP", async () => {
  const f=fixture();storeToken("");await assert.rejects(upsertLlmProvider(body),/Authentication is required/);
  assert.equal(f.calls.length,0);
});
