// Actual shipped QueryClient/QueryObserver and provider GET. Only login result,
// HTTP replies and React hook scheduling are controlled; no native acceptance.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test, { afterEach } from "node:test";
import vm from "node:vm";
import { register } from "node:module";
import { resolve, sep } from "node:path";
import { pathToFileURL } from "node:url";
const staticRoot = process.env.PREFIX_STATIC_ROOT
  ? pathToFileURL(resolve(process.env.PREFIX_STATIC_ROOT) + sep)
  : new URL("../../../../", import.meta.url);
const imports = { react: new URL("vendor/esm/react.js", staticRoot).href,
  "react/jsx-runtime": new URL("vendor/esm/react-jsx-runtime.js", staticRoot).href };
register("data:text/javascript," + encodeURIComponent(
  `const imports=${JSON.stringify(imports)};export async function resolve(specifier,context,nextResolve){return imports[specifier]?{url:imports[specifier],shortCircuit:true}:nextResolve(specifier,context);}`), import.meta.url);
const { QueryClient, QueryObserver } = await import(new URL("vendor/esm/react-query.js", staticRoot).href);
const auth = await import(new URL("js/lib/api.js", staticRoot).href);
const { fetchLlmProviders } = await import(new URL("js/pages/settings/lib/settings-api.js", staticRoot).href);
const login = await import(new URL("js/pages/settings/lib/provider-login.js", staticRoot).href);
const source = readFileSync(new URL("js/pages/settings/hooks/useProviderLogin.js", staticRoot), "utf8")
  .replace(/import[\s\S]*?from "[^"]+";\n/g, "").replaceAll("export ", "");
const saved = { window: globalThis.window, sessionStorage: globalThis.sessionStorage, fetch: globalThis.fetch };
afterEach(() => Object.assign(globalThis, saved));
const tick = () => new Promise(resolve => setImmediate(resolve));

test("completed login cancels the pre-commit GET and keeps real active observers on the fresh canonical query", async () => {
  const values = new Map();
  globalThis.sessionStorage = {getItem:key=>values.get(key)??null,setItem:(key,value)=>values.set(key,value),removeItem:key=>values.delete(key)};
  const window = {location:{origin:"http://operator.example"},open:()=>null,confirm:()=>true};
  globalThis.window = window; auth.storeToken("controlled-query-token");
  const id = "9e82b42f-990c-4faf-8848-ddf5e48bc18d";
  const requests = [];
  globalThis.fetch = (path, options) => new Promise(resolve => {
    assert.equal(path, "/api/webchat/v2/llm/providers");
    requests.push({signal:options.signal,reply:snapshot=>resolve(Response.json(snapshot))});
  });
  const client = new QueryClient({defaultOptions:{queries:{retry:false,gcTime:Infinity}}});
  const observer = new QueryObserver(client,{queryKey:["llm-providers"],queryFn:fetchLlmProviders});
  const updates = [], unsubscribe = observer.subscribe(value => updates.push(value.data));
  let navigations = 0, cleanup;
  const context = {
    React:{useRef:value=>({current:value}),useState:value=>[value,()=>{}],useCallback:fn=>fn,useEffect:fn=>{cleanup=fn();}},
    useQueryClient:()=>client,useT:()=>key=>key,window,AbortController,
    readStoredToken:auth.readStoredToken,readOriginalAuthContext:auth.readOriginalAuthContext,
    originalAuthContextIsCurrent:auth.originalAuthContextIsCurrent,
    codexLoginSelection:login.codexLoginSelection,findCodexLoginOriginal:login.findCodexLoginOriginal,
    codexLoginIsTerminal:login.codexLoginIsTerminal,fetchLlmProviders,
    runCodexLogin:async()=>({completed:true,status:{status:"completed",mutation_id:id}}),
  };
  vm.createContext(context);vm.runInContext(source+"\nglobalThis.create=useProviderLogin;",context);
  const hook = context.create({onSuccess:()=>{navigations++;}});
  const provider = {id:"clicked-codex",adapter:"openai_codex",revision:7,default_model:"clicked-model",selection_refs:{kohai:"a".repeat(64)}};
  const snapshot = mutationId => ({kohai_active:{provider_id:provider.id,model:provider.default_model},providers:[{id:provider.id,native_auth:{registration:"bound",access:"current",login_mutation_id:mutationId}}]});
  try {
    await tick();assert.equal(requests.length,1);
    const pending = hook.startCodex(provider);
    await tick();assert.equal(requests.length,2);
    assert.equal(requests[0].signal.aborted,true);
    assert.equal(requests[1].signal.aborted,false);
    requests[0].reply(snapshot("old-head"));await tick();
    assert.equal(navigations,0);
    assert.equal(client.getQueryData(["llm-providers"]),undefined);
    const fresh = snapshot(id);requests[1].reply(fresh);await pending;
    assert.equal(navigations,1);
    assert.deepEqual(client.getQueryData(["llm-providers"]),fresh);
    assert.deepEqual(observer.getCurrentResult().data,fresh);
    assert(updates.some(value=>value?.providers?.[0].native_auth.login_mutation_id===id));
  } finally { cleanup?.();unsubscribe();client.clear(); }
});
