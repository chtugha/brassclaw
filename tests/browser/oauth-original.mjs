// Actual Chromium component + native Auth HTTP; issuer replies are controlled.
import assert from "node:assert/strict";
import {spawn} from "node:child_process";
import {fileURLToPath} from "node:url";
const [origin]=process.argv.slice(2),target=new URL(origin);
assert.equal(target.protocol,"http:");assert.equal(target.hostname,"127.0.0.1");
assert.equal(target.pathname,"/");assert.equal(target.username,"");assert.equal(target.password,"");
const page=new URL("/__native_dcr_original",target).href;
// Invoke the pinned native CLI directly: the package's Node wrapper does not
// forward termination to its child, so a command timeout could leave a waiter.
const platform=process.platform==="linux"&&!process.report.getReport().header.glibcVersionRuntime?"linux-musl":process.platform;
const executable=process.env.AGENT_BROWSER_EXECUTABLE||fileURLToPath(new URL(`./node_modules/agent-browser/bin/agent-browser-${platform}-${process.arch}${process.platform==="win32"?".exe":""}`,import.meta.url));
const session=`brassclaw-dcr-original-${process.pid}`;
const commandTimeoutMs=Number(process.env.BRASSCLAW_BROWSER_COMMAND_TIMEOUT_MS||30000);
assert.ok(Number.isSafeInteger(commandTimeoutMs)&&commandTimeoutMs>0&&commandTimeoutMs<=2147483647);
function browser(...args){return new Promise((resolve,reject)=>{
  const child=spawn(executable,["--session",session,...args],{stdio:["ignore","pipe","pipe"],timeout:commandTimeoutMs});
  let stdout="",stderr="";child.stdout.on("data",bytes=>stdout+=bytes);child.stderr.on("data",bytes=>stderr+=bytes);
  child.on("error",reject);child.on("close",(code,signal)=>code===0?resolve(stdout):reject(new Error(`Browser ${args[0]} failed (${signal??code}): ${stderr} ${stdout}`)));
});}
async function evaluate(expression){const reply=JSON.parse(await browser("--json","eval","-b",Buffer.from(expression).toString("base64")));assert.equal(reply.success,true);return reply.data.result;}
async function inspect(){return evaluate(`({audit:window.oauthAudit,popups:window.blockedPopupAttempts,observers:window.oauthObserverTimers,originals:Object.entries(sessionStorage).filter(([key])=>key.startsWith("brassclaw:extension-oauth-original:")).map(([,text])=>JSON.parse(text)),storage:JSON.stringify(Object.entries(sessionStorage)),text:document.body.innerText,links:Array.from(document.querySelectorAll('a')).map(a=>({href:a.href,rel:a.rel}))})`);}
async function click(name,calls,status){await browser("find","role","button","click","--name",name,"--exact");await browser("wait","--fn",`window.oauthAudit.length===${calls}&&window.oauthAudit.at(-1).status===${status}`);}
function sameOriginal(state,original){assert.equal(state.originals.length,1);assert.equal(state.originals[0].flowId,original.flowId);assert.equal(state.originals[0].invocationId,original.invocationId);assert.equal(state.originals[0].expiresAt,original.expiresAt);for(const call of state.audit){assert.equal(call.retained_before_transport,true);assert.equal(call.body.flow_id,original.flowId);assert.equal(call.body.invocation_id,original.invocationId);assert.equal(call.body.expires_at,original.expiresAt);}for(const forbidden of["authorization_url","registration-token","dcr-client","code_verifier","opaque_state"]){assert.equal(state.storage.includes(forbidden),false);}}
try {
  const unauthenticated=await fetch(new URL("/api/webchat/v2/extensions/notion/setup/oauth/start",target),{method:"POST",headers:{"Content-Type":"application/json"},body:"{}"});assert.equal(unauthenticated.status,401);
  await browser("open",page);await browser("wait","--text","Authorize / retry original");
  await click("Authorize / retry original",1,503);await browser("wait","--text","The result is not confirmed.");
  let state=await inspect(),original=state.originals[0];assert.ok(original);sameOriginal(state,original);
  await browser("open",page);await browser("wait","--text","Authorize / retry original");
  await click("Authorize / retry original",2,200);await browser("wait","--fn","document.querySelector('a')!==null");
  state=await inspect();sameOriginal(state,original);assert.ok(state.popups>0);assert.equal(state.links.length,1);
  await browser("wait","--fn","window.oauthObserverTimers.some(timer=>timer.active&&timer.ms===2000&&timer.ticks>0)");
  state=await inspect();assert.equal(state.observers.filter(timer=>timer.active&&timer.ms===2000).length,1,"a blocked popup observes its original without duplicating timers");
  const authorization=new URL(state.links[0].href);assert.equal(authorization.protocol,"https:");assert.equal(authorization.hostname,"oauth.notion.com");assert.ok(state.links[0].rel.includes("noopener"));assert.ok(state.links[0].rel.includes("noreferrer"));
  await click("Cancel original setup",3,202);await browser("wait","--text","Provider deletion is not confirmed.");
  state=await inspect();sameOriginal(state,original);assert.equal(state.originals[0].receipt.status,"canceled");assert.equal(state.links.length,0);assert.ok(state.text.includes("Local material is retained"));
  assert.equal(state.observers.filter(timer=>timer.active&&timer.ms===2000).length,0,"confirmed terminal original stops its observation");
  await click("Reconcile original cleanup",4,200);await browser("wait","--text","Provider deletion is not confirmed.");
  state=await inspect();sameOriginal(state,original);assert.deepEqual(state.audit.map(call=>call.status),[503,200,202,200]);assert.deepEqual(state.audit.map(call=>call.body.action),["start","start","cancel","cancel"]);
  await browser("open",page);await browser("wait","--text","Start new authorization");
  await click("Check original setup",5,200);await browser("wait","--text","Provider deletion is not confirmed.");
  state=await inspect();sameOriginal(state,original);assert.equal(state.originals[0].receipt.status,"canceled");
  console.log(JSON.stringify({kind:"native-dcr-original-browser",http_statuses:state.audit.map(call=>call.status),identity_and_expiry_retained:true,popup_blocked_link:true,unknown_cleanup_visible:true}));
  console.log("native-dcr-original-browser: passed");
}catch(error){try{console.error(await browser("doctor","--offline","--quick"));}catch(diagnostic){console.error(diagnostic.message);}throw error;}
finally{await browser("close");}
