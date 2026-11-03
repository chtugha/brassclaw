// Real shipped QueryClient/QueryObserver; only operator transport replies and
// React hook scheduling are controlled. This is no PostgreSQL/uptake receipt.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";
import { register } from "node:module";
import { resolve, sep } from "node:path";
import { pathToFileURL } from "node:url";
const staticRoot = process.env.PREFIX_STATIC_ROOT
  ? pathToFileURL(resolve(process.env.PREFIX_STATIC_ROOT) + sep)
  : new URL("../../../../", import.meta.url);
// Use the browser's actual shipped import-map targets; no copied cache or React.
const imports = {react:new URL("vendor/esm/react.js",staticRoot).href,
  "react/jsx-runtime":new URL("vendor/esm/react-jsx-runtime.js",staticRoot).href};
register("data:text/javascript," + encodeURIComponent(
  `const imports=${JSON.stringify(imports)};export async function resolve(specifier,context,nextResolve){return imports[specifier]?{url:imports[specifier],shortCircuit:true}:nextResolve(specifier,context);}`), import.meta.url);
const authApi = await import(new URL("js/lib/api.js", staticRoot).href);
const mutationApi = await import(new URL("js/pages/settings/lib/provider-mutations.js", staticRoot).href);
const { QueryClient, QueryObserver } = await import(new URL("vendor/esm/react-query.js",staticRoot).href);
const sourcePath = process.env.PREFIX_STATUS_SOURCE || new URL("js/pages/settings/components/provider-mutation-status.js",staticRoot);
const source = readFileSync(sourcePath, "utf8").split("\n").filter(line => !line.startsWith("import ")).join("\n").replace("export function", "function");
const current = () => ({providers:[{id:"local",revision:10,model_token_limits:{ornith:{max_prefix_tokens:100000,max_prompt_tokens:14688}}}],sempai_active:{provider_id:"local",model:"new-model"}});
const receipt = {mutation_id:"original",provider_id:"local",provider_revision:8};
function fixture({ failure = false, initial = current(), realTransport = false } = {}) {
  const client = new QueryClient({defaultOptions:{queries:{retry:false,gcTime:Infinity,staleTime:Infinity}}});
  let reads = 0, cursor = 0, finish, reject, cleanup, effectStarted = false, authCurrent = true, contextUnavailable = false;
  const calls = [], updates = [], originalContext = { origin: "http://operator.example", nonce: "original-context" };
  const observer = new QueryObserver(client, {queryKey:["llm-providers"],initialData:initial,queryFn:async()=>{reads++;if(failure)throw Error("canonical query unavailable");return current();}});
  const unsubscribe = observer.subscribe(()=>{});
  const transport = new Promise((resolve, fail)=>{finish=resolve;reject=fail;});
  const state = [];
  const context = {
    React:{useState(initial){const i=cursor++;if(!(i in state))state[i]=typeof initial==="function"?initial():initial;return[state[i],value=>{updates.push([i,value]);state[i]=typeof value==="function"?value(state[i]):value;}];},useRef(initial){const i=cursor++;if(!(i in state))state[i]={current:initial};return state[i];},useEffect(effect){if(!effectStarted){effectStarted=true;cleanup=effect();}}},
    useQueryClient:()=>client,Button:"Button",html:(strings,...values)=>({strings:[...strings],values}),
    pendingProviderMutations:()=>[{id:"original",target:"local",committed:true}],subscribeProviderMutations:()=>()=>{},
    readStoredToken:realTransport?authApi.readStoredToken:()=>"controlled-status-token",
    readOriginalAuthContext:realTransport?authApi.readOriginalAuthContext:()=>{if(contextUnavailable)throw Error("original context unavailable");return originalContext;},
    originalAuthContextIsCurrent:realTransport?authApi.originalAuthContextIsCurrent:(context,token)=>authCurrent&&context===originalContext&&token==="controlled-status-token",
    AbortController,
    readProviderMutation:(id,options)=>{calls.push({id,options,reconcile:false});return realTransport?mutationApi.readProviderMutation(id,options):transport;},
    reconcileProviderMutation:(id,options)=>{calls.push({id,options,reconcile:true});return realTransport?mutationApi.reconcileProviderMutation(id,options):transport;},
  };
  vm.createContext(context);vm.runInContext(source+"\nglobalThis.render=ProviderMutationStatus;",context);
  function render(){cursor=0;return context.render({t:key=>key});}
  return {client,render,finish,reject,calls,updates,reads:()=>reads,
    changeAuth(){authCurrent=false;if(realTransport)authApi.storeToken("changed-status-token");},denyContext(){contextUnavailable=true;},unmount(){cleanup?.();},
    close(){cleanup?.();unsubscribe();client.clear();}};
}
function nodes(value,out=[]){if(Array.isArray(value))value.forEach(v=>nodes(v,out));else if(value?.strings){out.push(value);value.values.forEach(v=>nodes(v,out));}return out;}
function click(f,label){const node=nodes(f.render()).find(n=>n.values.includes(label));assert.ok(node,label);const labelIndex=node.values.indexOf(label);const index=node.strings.reduce((found,s,i)=>i<labelIndex&&s.endsWith("onClick=")?i:found,-1);assert.ok(index>=0);return node.values[index]();}
for (const sameRevision of [false,true]) {
  test(`late reconcile cannot overwrite canonical limits or roles (same revision=${sameRevision})`,async()=>{
    const f=fixture();try{
      const pending=click(f,"llm.mutationReconcile");
      f.finish({mutation_receipt:receipt,providers:[{id:"local",revision:sameRevision?10:9,model_token_limits:{ornith:{max_prefix_tokens:1,max_prompt_tokens:2}}}],sempai_active:{provider_id:"local",model:"old-model"}});
      await pending;
      assert.deepEqual(f.client.getQueryData(["llm-providers"]),current());
      assert.equal(f.reads(),1,"canonical query owns publication");
    }finally{f.close();}
  });
}
test("reading a committed original refreshes canonical settings without requiring runtime reconciliation",async()=>{
  const f=fixture({initial:{providers:[{id:"local",revision:7,model_token_limits:{ornith:{max_prefix_tokens:10,max_prompt_tokens:20}}}]}});try{
    const pending=click(f,"llm.mutationRead");f.finish(receipt);await pending;
    assert.equal(f.reads(),1);
    assert.deepEqual(f.client.getQueryData(["llm-providers"]),current());
  }finally{f.close();}
});
test("failed canonical refresh preserves both known commit and visible read error",async()=>{
  const f=fixture({failure:true});try{
    const pending=click(f,"llm.mutationReconcile");f.finish({mutation_receipt:receipt,providers:[]});await pending;
    assert.deepEqual(f.client.getQueryData(["llm-providers"]),current());
    const view=nodes(f.render());
    assert.ok(view.some(node=>node.values.includes("llm.mutationCommitted")));
    assert.ok(view.some(node=>node.values.includes("canonical query unavailable")));
  }finally{f.close();}
});


test("inspection retains original identity and forwards its observer/abort fence", async()=>{
  for(const action of ["llm.mutationRead","llm.mutationReconcile"]){
    const f=fixture();try{
      const pending=click(f,action);
      assert.equal(f.calls.length,1);assert.equal(f.calls[0].id,"original");
      assert.equal(f.calls[0].options.isCurrent(),true);
      assert.equal(f.calls[0].options.signal.aborted,false);
      f.finish(action==="llm.mutationRead"?receipt:{mutation_receipt:receipt});await pending;
      assert.equal(f.reads(),1);
    }finally{f.close();}
  }
});

test("unmount aborts observation and delayed responses cannot publish or refresh", async()=>{
  for(const action of ["llm.mutationRead","llm.mutationReconcile"]){
    const f=fixture();try{
      const pending=click(f,action),before=f.updates.length;
      f.unmount();assert.equal(f.calls[0].options.signal.aborted,true);
      assert.equal(f.calls[0].options.isCurrent(),false);
      f.finish(action==="llm.mutationRead"?receipt:{mutation_receipt:receipt});await pending;
      assert.equal(f.reads(),0);assert.equal(f.updates.length,before);
    }finally{f.close();}
  }
});

test("auth changes fence old receipts and errors without leaving the observer busy", async()=>{
  for(const failure of [false,true]){
    const f=fixture();try{
      const pending=click(f,"llm.mutationReconcile");f.changeAuth();
      assert.equal(f.calls[0].options.isCurrent(),false);
      if(failure)f.reject(Error("old-context-error"));else f.finish({mutation_receipt:receipt});
      await pending;assert.equal(f.reads(),0);
      const view=nodes(f.render());
      assert(!view.some(node=>node.values.includes("llm.mutationCommitted")));
      assert(!view.some(node=>node.values.includes("old-context-error")));
      assert.equal(f.updates.at(-1)[1],null);
    }finally{f.close();}
  }
});

test("missing original auth context stops inspection before transport", async()=>{
  const f=fixture();try{
    f.denyContext();await click(f,"llm.mutationRead");
    assert.equal(f.calls.length,0);assert.equal(f.reads(),0);
    assert(nodes(f.render()).some(node=>node.values.includes("original context unavailable")));
  }finally{f.close();}
});

test("duplicate inspection clicks share one original request", async()=>{
  const f=fixture();try{
    const pending=click(f,"llm.mutationReconcile");await click(f,"llm.mutationRead");
    assert.equal(f.calls.length,1);f.finish({mutation_receipt:receipt});await pending;
    assert.equal(f.reads(),1);
  }finally{f.close();}
});


test("actual status caller and mutation transport fence late bootstrap notifications", async()=>{
  const saved={window:globalThis.window,sessionStorage:globalThis.sessionStorage,fetch:globalThis.fetch};
  try{
    for(const change of ["current","unmount","auth"]){
      const values=new Map(),notices=[];
      globalThis.window={location:{origin:"http://operator.example"}};
      globalThis.sessionStorage={getItem:key=>values.get(key)??null,setItem:(key,value)=>values.set(key,value),removeItem:key=>values.delete(key)};
      authApi.storeToken("controlled-status-token");
      let respond;const requests=[];
      globalThis.fetch=(path,options)=>new Promise(resolve=>{requests.push({path,options});respond=resolve;});
      const unsubscribe=mutationApi.subscribeProviderBootstrap(value=>notices.push(value));
      const f=fixture({realTransport:true});
      try{
        const pending=click(f,"llm.mutationReconcile");
        assert.equal(requests.length,1);assert.equal(requests[0].options.method,"POST");
        assert.equal(requests[0].options.body,undefined);
        assert.equal(requests[0].options.headers.get("Authorization"),"Bearer controlled-status-token");
        if(change==="unmount")f.unmount();else if(change==="auth")f.changeAuth();
        respond(Response.json({mutation_receipt:receipt}));await pending;
        assert.equal(notices.length,change==="current"?1:0);
        assert.equal(f.reads(),change==="current"?1:0);
        assert.equal(requests.length,1,"observation never resubmits the provider change");
      }finally{unsubscribe();f.close();}
    }
  }finally{Object.assign(globalThis,saved);}
});
