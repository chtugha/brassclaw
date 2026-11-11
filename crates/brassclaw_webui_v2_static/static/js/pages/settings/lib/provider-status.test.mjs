import assert from "node:assert/strict";
import test, { afterEach } from "node:test";
import { storeToken } from "../../../lib/api.js";
import { fetchLlmProviders } from "./settings-api.js";
const saved={window:globalThis.window,sessionStorage:globalThis.sessionStorage,fetch:globalThis.fetch};
afterEach(()=>Object.assign(globalThis,saved));
function fixture() {
  const values=new Map();
  globalThis.sessionStorage={getItem:key=>values.get(key)??null,setItem:(key,value)=>values.set(key,value),removeItem:key=>values.delete(key)};
  globalThis.window={location:{origin:"http://operator.example"}};
  storeToken("original-operator");
  let release, seen;
  globalThis.fetch=async(path,options)=>{seen={path,options}; await new Promise(resolve=>{release=resolve;});return Response.json({providers:[]});};
  return {release:()=>release(),seen:()=>seen};
}
test("canonical provider GET forwards cancellation and rejects a late old-auth snapshot",async()=>{
  for(const change of ["auth","abort"]) {
    const f=fixture(), abort=new AbortController(), pending=fetchLlmProviders({signal:abort.signal});
    assert.equal(f.seen().path,"/api/webchat/v2/llm/providers");
    assert.equal(f.seen().options.signal,abort.signal);
    if(change==="auth")storeToken("new-operator");else abort.abort();
    f.release();await assert.rejects(pending,{name:"AbortError"});
  }
});
test("canonical provider GET returns the current authenticated observation",async()=>{
  const f=fixture(), pending=fetchLlmProviders();f.release();
  assert.deepEqual(await pending,{providers:[]});
});
