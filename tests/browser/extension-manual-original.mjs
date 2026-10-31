// Real protected Extension/native acceptance; fixture-only token sentinels.
import assert from "node:assert/strict";
import {spawn} from "node:child_process";
import {fileURLToPath} from "node:url";
const [origin]=process.argv.slice(2),target=new URL(origin);
assert.equal(target.protocol,"http:");assert.equal(target.hostname,"127.0.0.1");
assert.equal(target.pathname,"/");assert.equal(target.username,"");assert.equal(target.password,"");
const page=new URL("/__native_extension_original",target).href;
const executable=fileURLToPath(new URL("./node_modules/agent-browser/bin/agent-browser.js",import.meta.url));
const session=`brassclaw-extension-original-${process.pid}`;
const firstToken="native-extension-original-token-ü",changedToken="native-extension-changed-token",canceledToken="native-extension-canceled-token";
const input='input[type="password"]';
const unknown="The outcome could not be confirmed. Retry with the same original setup or use Continue saved setup. No replacement was created.";
function browser(...args) {
  return new Promise((accept, reject) => {
    const child = spawn(process.execPath, [executable, "--session", session, ...args], {
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "", stderr = "";
    child.stdout.on("data", bytes => { stdout += bytes; });
    child.stderr.on("data", bytes => { stderr += bytes; });
    child.on("error", reject);
    const action = args[0] === "find" ? args.slice(0, 4).join(" ") : args[0];
    child.on("close", code => code === 0
      ? accept(stdout) : reject(new Error(`Browser ${action} failed (${code}): ${stderr} ${stdout}`)));
  });
}

async function evaluate(expression) {
  const response = JSON.parse(await browser("--json", "eval", "-b", Buffer.from(expression).toString("base64")));
  assert.equal(response.success, true);
  return response.data.result;
}


async function inspect(){return evaluate(`({audit:window.extensionOriginalAudit,
  originals:Object.entries(sessionStorage).filter(([key])=>key.startsWith("brassclaw:extension-manual-token-original:")).map(([,value])=>JSON.parse(value)),
  storage:JSON.stringify(Object.entries(sessionStorage)),visible:document.body.innerText,
  token_empty:document.querySelector(${JSON.stringify(input)})?.value===""})`);}
function privateOriginal(state,original,originalCalls=state.audit.length){
  assert.equal(state.originals.length,1);
  assert.deepEqual(state.originals[0].identity,original.identity);
  assert.equal(state.originals[0].invocationId,original.invocationId);
  assert.deepEqual(state.originals[0].definition,original.definition);
  for(const token of[firstToken,changedToken,canceledToken]){
    assert.equal(state.storage.includes(token),false);assert.equal(state.visible.includes(token),false);
  }
  for(const call of state.audit.slice(0,originalCalls)){
    assert.deepEqual(call.identity,original.identity);assert.equal(call.invocation_id,original.invocationId);
    assert.equal(call.retained_before_transport,true);if(call.token_present)assert.equal(call.token_matches_input,true);
  }
}
async function saveToken(value,calls,status){
  await browser("fill",input,value);
  await browser("find","role","button","click","--name","Save token","--exact");
  await browser("wait","--fn",`window.extensionOriginalAudit.length===${calls}&&window.extensionOriginalAudit.at(-1).http_status===${status}`);
  await browser("wait","--text",unknown);
  await browser("wait","--fn",`!document.querySelector(${JSON.stringify(input)}).disabled`);
}
try{
  await browser("open",page);await browser("wait",input);
  await saveToken(firstToken,1,503);
  let state=await inspect(),original=state.originals[0];
  assert.equal(original.setup,null);assert.equal(original.closed,false);
  assert.equal(original.definition.kind,"extension");assert.equal(original.definition.packageId,"nearai");
  assert.equal(original.definition.target.kind,"existing");privateOriginal(state,original);
  await browser("open",page);await browser("wait",input);state=await inspect();
  assert.equal(state.token_empty,true);privateOriginal(state,original);
  await saveToken(firstToken,3,503);state=await inspect();privateOriginal(state,original);
  const expiry=state.originals[0].setup.expiresAt;assert.equal(Number.isFinite(Date.parse(expiry)),true);
  await saveToken(changedToken,5,409);state=await inspect();privateOriginal(state,original);
  assert.equal(state.originals[0].closed,false);assert.equal(state.originals[0].setup.status,"completed");
  assert.equal(state.originals[0].setup.expiresAt,expiry);
  await browser("find","role","button","click","--name","Continue saved setup","--exact");
  await browser("wait","--fn","window.extensionOriginalAudit.length===7&&window.extensionOriginalAudit.at(-1).http_status===200");
  await browser("wait","--text","The original credential setup is confirmed.");
  state=await inspect();privateOriginal(state,original);assert.equal(state.originals[0].closed,true);
  assert.equal(state.originals[0].setup.expiresAt,expiry);assert.equal(state.token_empty,true);
  const reference=state.audit.filter(call=>call.path.endsWith("/continue"));
  assert.equal(reference.length,1);assert.equal(reference[0].token_present,false);
  assert.deepEqual(reference[0].keys,["identity","invocation_id","requirement"]);
  for(const call of state.audit.filter(call=>call.reply&&call.path.endsWith("/setup")))assert.equal(call.reply.expires_at,expiry);
  await browser("find","role","button","click","--name","Start a new setup","--exact");
  await browser("wait","--fn",`!document.querySelector(${JSON.stringify(input)}).disabled`);
  state=await inspect();const next=state.originals[0];
  assert.notEqual(next.identity.flow_id,original.identity.flow_id);assert.notEqual(next.identity.interaction_id,original.identity.interaction_id);
  assert.equal(next.invocationId,original.invocationId);assert.equal(next.definition.target.kind,"existing");
  await saveToken(canceledToken,8,503);
  await browser("find","role","button","click","--name","Cancel original setup","--exact");
  await browser("wait","--fn","window.extensionOriginalAudit.length===9&&window.extensionOriginalAudit.at(-1).http_status===200");
  await browser("wait","--text","This original setup is closed. Start a new setup to make another change.");
  state=await inspect();assert.equal(state.originals[0].closed,true);assert.equal(state.originals[0].setup.status,"canceled");
  assert.deepEqual(state.originals[0].identity,next.identity);assert.equal(state.token_empty,true);
  assert.deepEqual(state.audit.map(call=>call.http_status),[503,200,503,200,409,200,200,503,200]);
  for(const call of state.audit.slice(7)){
    assert.deepEqual(call.identity,next.identity);assert.equal(call.invocation_id,next.invocationId);
    assert.equal(call.token_present,false);assert.equal(call.retained_before_transport,true);
  }
  for(const token of[firstToken,changedToken,canceledToken]){
    assert.equal(state.storage.includes(token),false);assert.equal(state.visible.includes(token),false);
  }
  assert.equal(state.audit.filter(call=>call.token_present).length,2);
  assert.deepEqual(state.audit.at(-1).keys,["identity","invocation_id","requirement"]);
  console.log(JSON.stringify({kind:"native-extension-original-browser",http_statuses:state.audit.map(call=>call.http_status),setup_calls:5,raw_calls:2,reference_calls:1,cancel_calls:1,original_identity_and_expiry_retained:true,raw_privacy_checked:true}));
  console.log("native-extension-original-browser: passed");
}catch(error){
  try{console.error(await browser("doctor","--offline","--quick"));}catch(diagnostic){console.error(diagnostic.message);}
  try{console.error(JSON.stringify(await evaluate(`({inputs:Array.from(document.querySelectorAll('input')).map(element=>({type:element.type,label:element.getAttribute('aria-label'),disabled:element.disabled})),buttons:Array.from(document.querySelectorAll('button')).map(element=>({text:element.textContent.trim(),disabled:element.disabled})),audit:window.extensionOriginalAudit})`)));}catch(diagnostic){console.error(diagnostic.message);}
  throw error;
}finally{await browser("close");}
