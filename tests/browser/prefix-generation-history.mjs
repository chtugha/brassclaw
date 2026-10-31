// Native operator middleware/facade/PostgreSQL plus the actual React component.
// The relay delays or cuts real acknowledgements; it invents no API results.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import http from "node:http";
import { resolve, sep, extname } from "node:path";
import { spawn } from "node:child_process";
import { EventEmitter, once } from "node:events";
import { fileURLToPath } from "node:url";

const [upstream, source, generationFile, revisionContext] = process.argv.slice(2);
const precision = revisionContext ? JSON.parse(revisionContext) : null;
const designView = generationFile === "--designs";
const target = new URL(upstream);
assert.equal(target.hostname, "127.0.0.1");
const root = resolve(source);
const route = "/api/webchat/v2/prefixes/base-prompt/generations";
const headers = { authorization: "Bearer catalogue-acceptance" };
async function native(path) {
  const response = await fetch(upstream + path, { headers });
  assert.equal(response.status, 200);
  return response.json();
}
const profiles = await native("/api/webchat/v2/prefixes");
const entry = profiles.prefixes.find(value => value.name === "base-prompt");
assert(entry);
const originalBytes = designView ? null : await readFile(generationFile, "utf8");
const initial = designView || precision ? null : await native(route);
if (!designView && !precision) assert(initial.length >= 2, "existing native fixture must retain distinct real generations");
// Generation identity is over the complete package, not a caller-supplied field.
const imported = designView || precision ? [] : await Promise.all(initial.map(value => native(`${route}/${value.generation}`)));
const a = imported.find(value => value.generation_bytes === originalBytes);
if (!designView && !precision) assert(a, "actual exported package must already be retained");
const b = imported.find(value => value.summary.generation !== a.summary.generation);
if (!designView && !precision) { assert(b); assert.equal(a.summary.qualification, "quarantined"); }
const aId = a?.summary.generation, bId = b?.summary.generation;
const index = await readFile(resolve(root, "index.html"), "utf8");
const importMap = index.match(/<script type="importmap"[^>]*>([\s\S]*?)<\/script>/)[1];
const page = `<!doctype html><html><head><meta charset="utf-8"><title>Native prefix history acceptance</title>
<script type="importmap">${importMap}</script><link rel="stylesheet" href="/styles/app.css"></head>
<body><div id="root"></div><script>
sessionStorage.setItem('brassclaw_token','catalogue-acceptance');localStorage.setItem('brassclaw_language','en');
const readFileText=File.prototype.text;
window.nativeHistoryHoldFile=false;window.nativeHistoryFileStarted=false;
File.prototype.text=async function(){const bytes=await readFileText.call(this);if(window.nativeHistoryHoldFile){window.nativeHistoryFileStarted=true;await new Promise(resolve=>window.nativeHistoryReleaseFile=resolve);}return bytes;};
</script><script type="module">
import {React,html} from '/js/lib/html.js';
import {createRoot} from 'react-dom/client';
import {I18nProvider,useT} from '/js/lib/i18n.js';
import '/js/i18n/en.js';
import {PrefixGenerationHistory} from '/js/pages/settings/components/prefix-generation-history.js';
import {PrefixModelDesigns} from '/js/pages/settings/components/prefix-model-designs.js';
const entry=${JSON.stringify(entry).replaceAll("<", "\\u003c")};
const root=createRoot(document.getElementById('root'));
function App(){const t=useT();return ${designView || precision ? "html`<${PrefixModelDesigns} t=${t}/>`" : "html`<${PrefixGenerationHistory} entry=${entry} t=${t}/>`"};}
window.nativeHistoryMount=show=>root.render(show?html\`<\${I18nProvider}><\${App}/><//>\`:null);
window.nativeHistoryMount(true);
</script></body></html>`;
const requests = [], held = [], relayEvents = new EventEmitter();
let stage = "initial browser load";
let holdDetail = null, holdWrite = false, cutWrites = false;
const mime = { ".js": "text/javascript", ".css": "text/css", ".svg": "image/svg+xml", ".png": "image/png", ".mjs": "text/javascript" };
const server = http.createServer(async (req, res) => {
  try {
    const path = new URL(req.url, "http://127.0.0.1").pathname;
    if (path.startsWith("/api/")) {
      const record = { method: req.method, path, body: "", complete: false };
      requests.push(record);
      const requestChunks = [];
      req.on("data", bytes => { requestChunks.push(bytes); });
      req.on("end", () => { record.body = Buffer.concat(requestChunks).toString("utf8"); });
      const delay = (req.method === "GET" && path === `${route}/${holdDetail}`)
        || (req.method === "POST" && holdWrite);
      const cut = req.method === "POST" && cutWrites;
      const forward = http.request(new URL(req.url, target), {
        method: req.method, headers: { ...req.headers, host: target.host },
      }, response => {
        const chunks = [];
        response.on("data", bytes => chunks.push(bytes));
        response.on("end", () => {
          record.complete = true; record.status = response.statusCode;
          const send = () => {
            if (cut) { res.destroy(); return; }
            res.writeHead(response.statusCode, response.headers); res.end(Buffer.concat(chunks));
          };
          if (delay) { held.push({ record, send }); relayEvents.emit("held"); } else send();
        });
      });
      forward.on("error", error => { res.writeHead(502); res.end(error.message); });
      req.pipe(forward); return;
    }
    if (precision && path === "/precision-package") { res.writeHead(200, {"content-type":"text/plain; charset=utf-8"}); res.end(originalBytes); return; }
    if (path === "/") { res.writeHead(200, { "content-type": "text/html; charset=utf-8" }); res.end(page); return; }
    const file = resolve(root, "." + decodeURIComponent(path));
    if (!file.startsWith(root + sep)) { res.writeHead(403); res.end(); return; }
    const bytes = await readFile(file);
    res.writeHead(200, { "content-type": mime[extname(file)] || "application/octet-stream" }); res.end(bytes);
  } catch (error) { res.writeHead(error.code === "ENOENT" ? 404 : 500); res.end(); }
});
server.listen(0, "127.0.0.1"); await once(server, "listening");
const session = `brassclaw-prefix-history-${process.pid}`;
const platform = process.platform === "linux" && !process.report.getReport().header.glibcVersionRuntime
  ? "linux-musl" : process.platform;
const executable = process.env.AGENT_BROWSER_EXECUTABLE || fileURLToPath(new URL(
  `./node_modules/agent-browser/bin/agent-browser-${platform}-${process.arch}${process.platform === "win32" ? ".exe" : ""}`, import.meta.url));
const commandTimeoutMs = Number(process.env.BRASSCLAW_BROWSER_COMMAND_TIMEOUT_MS || 30000);
assert(Number.isSafeInteger(commandTimeoutMs) && commandTimeoutMs > 0 && commandTimeoutMs <= 2147483647);
function browser(...args) {
  return new Promise((accept, reject) => {
    const child = spawn(executable, ["--session", session, ...args], {
      stdio: ["ignore", "pipe", "pipe"], timeout: commandTimeoutMs,
    });
    let stdout = "", stderr = "";
    child.stdout.on("data", bytes => { stdout += bytes; });
    child.stderr.on("data", bytes => { stderr += bytes; });
    child.on("error", reject);
    child.on("close", (code, signal) => code === 0 ? accept(stdout)
      : reject(Error(`${args.join(" ")} failed (${signal ?? code}): ${stderr} ${stdout}`)));
  });
}
const writes = () => requests.filter(value => value.method === "POST");
async function click(label) {
  await browser("find", "role", "button", "click", "--name", label, "--exact");
}
async function inspect(id) {
  // Focus the actual row control before genuine keyboard activation. A pointer
  // action during the first large-list layout does not prove its handler ran.
  await browser("eval", `(()=>{const row=Array.from(document.querySelectorAll('code')).find(n=>n.textContent===${JSON.stringify(id)})?.parentElement;const button=row?.querySelector('button');if(!button)throw Error('missing Inspect control');button.focus();if(document.activeElement!==button)throw Error('Inspect focus failed');})()`);
  await browser("press", "Enter");
}
async function detail(id) {
  await browser("wait", "--fn", `(()=>{try{return JSON.parse(document.querySelector('details pre')?.textContent).summary.generation===${JSON.stringify(id)};}catch{return false;}})()`);
}
async function settled() {
  // A real request in the same connection path establishes event-loop progress
  // after a released response; no timing-based sleep chooses the result.
  await browser("eval", "fetch('/api/webchat/v2/prefixes',{headers:{authorization:'Bearer catalogue-acceptance'}}).then(r=>r.json()).then(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))))");
}
function release() { for (const value of held.splice(0)) value.send(); }
async function waitHeld(method, path) {
  const signal = AbortSignal.timeout(commandTimeoutMs);
  while (!held.some(value => value.record.method === method && value.record.path === path)) {
    await once(relayEvents, "held", { signal });
  }
}
let primaryFailure = null;
try {
  await browser("open", `http://127.0.0.1:${server.address().port}`);
  if (precision) {
    stage = "large revision from real browser JSON and settings-api";
    const context = JSON.stringify(precision);
    await browser("eval", `window.nativePrecisionContext=${context}`);
    await browser("eval", `window.nativePrecisionReady=(async()=>{
      const api=await import('/js/pages/settings/lib/settings-api.js');
      const catalogue=await api.fetchPrefixModelDesigns();
      const profiles=await api.fetchPrefixes();
      const cfg=window.nativePrecisionContext;
      const target=catalogue.targets.find(t=>t.provider_id===cfg.provider_id);
      const entry=profiles.prefixes.find(p=>p.name==='base-prompt');
      if(typeof target?.provider_revision!=='string'||target.provider_revision!==cfg.revision||typeof entry?.profile_revision!=='string'||entry.profile_revision!==cfg.revision)throw Error('lossless original revisions missing');
      const bytes=await fetch('/precision-package').then(r=>{if(!r.ok)throw Error(r.status);return r.text();});
      const capture={provider_id:target.provider_id,model:target.model,expected_provider_revision:target.provider_revision};
      const imported={expected_profile_revision:entry.profile_revision,generation_bytes:bytes};
      const first=await api.capturePrefixModel(capture); const retry=await api.capturePrefixModel(capture);
      if(JSON.stringify(first)!==JSON.stringify(retry)||first.provider_revision!==cfg.revision)throw Error('capture original mismatch');
      try{await api.capturePrefixModel({...capture,expected_provider_revision:(BigInt(cfg.revision)-1n).toString()});throw Error('stale capture accepted');}catch(e){if(e.status!==409)throw e;}
      window.nativePrecisionState={api,capture,imported};
      window.nativePrecisionLoaded=true;
    })()`);
    await browser("wait", "--fn", "window.nativePrecisionLoaded===true");
    await browser("wait", "--fn", `Array.from(document.querySelectorAll('option')).some(n=>n.textContent.includes(${JSON.stringify(precision.revision)}))`);
    const count = writes().length;
    await browser("eval", `(()=>{
      const {api}=window.nativePrecisionState;const unsafe=Number(window.nativePrecisionContext.revision);
      for(const call of [()=>api.capturePrefixModel({expected_provider_revision:unsafe}),()=>api.importPrefixGeneration('base-prompt',{expected_profile_revision:unsafe}),()=>api.measurePrefix('base-prompt',crypto.randomUUID(),{expected_profile_revision:unsafe})]){
        let rejected=false;try{call();}catch(e){rejected=/Invalid prefix revision/.test(e.message);}if(!rejected)throw Error('unsafe Number did not fail before Fetch');
      }
    })()`);
    assert.equal(writes().length,count,"unsafe numeric identities dispatch no write");
    stage="large revision original import with lost ACK";
    const beforeImport=writes().length;cutWrites=true;
    await browser("eval", `window.nativePrecisionUnknown=(async()=>{const s=window.nativePrecisionState;try{await s.api.importPrefixGeneration('base-prompt',s.imported);throw Error('cut ACK unexpectedly arrived');}catch(e){if(e.message==='cut ACK unexpectedly arrived')throw e;}window.nativePrecisionImportUnknown=true;})()`);
    await browser("wait", "--fn", "window.nativePrecisionImportUnknown===true");
    const originalImports=writes().slice(beforeImport).filter(r=>r.path===route);
    assert(originalImports.length>0);assert(originalImports.every(r=>r.complete&&r.status===200));
    const original=originalImports[0].body;
    assert.deepEqual(JSON.parse(original),{expected_profile_revision:precision.revision,generation_bytes:originalBytes});
    assert(originalImports.every(r=>r.body===original));
    cutWrites=false;
    await browser("eval", `window.nativePrecisionFinished=(async()=>{
      const s=window.nativePrecisionState,cfg=window.nativePrecisionContext;
      const receipt=await s.api.importPrefixGeneration('base-prompt',s.imported);
      if(receipt.summary.profile_revision!==cfg.revision||receipt.generation_bytes!==s.imported.generation_bytes||receipt.summary.qualification!=='quarantined')throw Error('import original mismatch');
      try{await s.api.importPrefixGeneration('base-prompt',{...s.imported,expected_profile_revision:(BigInt(cfg.revision)-1n).toString()});throw Error('stale import accepted');}catch(e){if(e.status!==409)throw e;}
      const request={expected_profile_revision:cfg.revision,generation:receipt.summary.generation,proposal:'0'.repeat(64),artifact_set:'0'.repeat(64),probes_json:'[]',context_window_tokens:1,reserved_additional_input_tokens:0};
      const job=crypto.randomUUID();
      for(let i=0;i<2;i++){try{await s.api.measurePrefix('base-prompt',job,request);throw Error('incomplete measurement accepted');}catch(e){if(e.status!==400)throw e;}}
      try{await s.api.fetchPrefixJob('base-prompt',job);throw Error('rejected measurement retained job');}catch(e){if(e.status!==404)throw e;}
      window.nativePrecisionDone=true;
    })()`);
    await browser("wait", "--fn", "window.nativePrecisionDone===true");
    const imports=writes().slice(beforeImport).filter(r=>r.path===route&&r.status===200);
    assert(imports.length>=2&&imports.every(r=>r.body===original),"successful original retry is byte-identical");
    const measures=writes().filter(r=>r.path.includes('/jobs/'));
    assert.equal(measures.length,2);assert.equal(measures[0].body,measures[1].body);assert(measures.every(r=>r.status===400&&JSON.parse(r.body).expected_profile_revision===precision.revision));
    console.log("native-prefix-revision-browser: passed; measurement rejected/no-job");
  } else if (designView) {
    const catalogue = await native("/api/webchat/v2/prefix-model-designs");
    assert(catalogue.specifications.length > 0 && catalogue.proposals.length > 0);
    const spec = catalogue.specifications[0];
    await browser("eval", "fetch('/api/webchat/v2/prefix-model-designs',{headers:{authorization:'Bearer catalogue-acceptance'}}).then(r=>{if(!r.ok)throw Error(r.status);return r.json();}).then(v=>{window.nativeOriginalCatalogue=v;})");
    stage = "model design identity and original details";
    await browser("wait", "--fn", `Array.from(document.querySelectorAll('code')).some(n=>n.textContent===${JSON.stringify(spec.id)})`);
    // Focus the existing native summary and use real keyboard activation.
    await browser("eval", `Array.from(document.querySelectorAll('details')).find(d=>Array.from(d.querySelectorAll(':scope > dl dd')).some(n=>n.textContent===${JSON.stringify(spec.provider_id)})).querySelector(':scope > summary').focus()`);
    await browser("press", "Enter");
    await browser("wait", "--text", "Original specification");
    await browser("eval", "Array.from(document.querySelectorAll('summary')).find(n=>n.textContent==='Original specification').focus()");
    await browser("press", "Enter");
    await browser("wait", "--fn", "Array.from(document.querySelectorAll('pre')).some(p=>p.textContent===window.nativeOriginalCatalogue.specifications[0].specification_bytes&&p.children.length===0&&p.closest('details').open)");
    await browser("wait", "--fn", "(()=>{const c=window.nativeOriginalCatalogue;const originals=[...c.specifications.map(v=>v.specification_bytes),...c.proposals.map(v=>v.proposal_bytes),...c.pair_proposals.map(v=>v.proposal_bytes),...c.artifact_sets.map(v=>v.binding_bytes)];return originals.every(text=>Array.from(document.querySelectorAll('pre')).some(p=>p.textContent===text&&p.children.length===0));})()");
    assert(catalogue.proposals.some(value => value.proposal_bytes.includes("<img src=x onerror=")));
    await browser("wait", "--fn", "!window.prefixOriginalExecuted&&!document.querySelector('pre img')");
    await browser("wait", "--fn", "Array.from(document.querySelectorAll('dt')).some(n=>n.textContent==='Provider configuration revision')");
    assert.equal(writes().length, 0, "viewing originals cannot capture/import/generate or activate");
    assert(requests.every(value => value.method === "GET"));
    assert.deepEqual(await native("/api/webchat/v2/prefix-model-designs"), catalogue);
    console.log(await browser("snapshot", "-i"));
    console.log("native-prefix-design-view-browser: passed");
  } else {
  stage = "out-of-order generation details";
  await click("Generation history");
  await browser("wait", "--text", aId); await browser("wait", "--text", bId);
  holdDetail = aId;
  await inspect(aId); await waitHeld("GET", `${route}/${aId}`);
  await inspect(bId); await detail(bId);
  holdDetail = null; release(); await settled(); await detail(bId);

  // Closing or unmounting while the actual File.text result is pending must
  // fence the old selection before it can send any import to the native API.
  for (const unmount of [false, true]) {
    stage = unmount ? "unmount during file read" : "close during file read";
    const count = writes().length;
    await browser("eval", "window.nativeHistoryHoldFile=true;window.nativeHistoryFileStarted=false");
    await browser("upload", 'input[type="file"]', resolve(generationFile));
    await browser("wait", "--fn", "window.nativeHistoryFileStarted");
    if (unmount) {
      await browser("eval", "window.nativeHistoryMount(false)");
      await browser("wait", "--fn", "document.querySelector('button')===null");
    } else {
      await click("Generation history");
      await browser("wait", "--fn", "document.querySelector('input[type=file]')===null");
    }
    await browser("eval", "window.nativeHistoryHoldFile=false;window.nativeHistoryReleaseFile()");
    await settled(); assert.equal(writes().length, count);
    if (unmount) await browser("eval", "window.nativeHistoryMount(true)");
    await click("Generation history"); await browser("wait", "--text", bId);
  }

  stage = "confirmed import after newer detail selection";
  holdWrite = true;
  await browser("upload", 'input[type="file"]', resolve(generationFile));
  await browser("wait", "--fn", "document.querySelector('input[type=file]').disabled");
  await inspect(bId); await detail(bId);
  await waitHeld("POST", route);
  holdWrite = false; release();
  await browser("wait", "--fn", "!document.querySelector('input[type=file]').disabled");
  await detail(bId);

  // Remount gives the file control a new selection. Cut every real POST ACK
  // until explicit retry, including any Chromium transport-level retry.
  await browser("eval", "window.nativeHistoryMount(false)");
  await browser("wait", "--fn", "document.querySelector('button')===null");
  await browser("eval", "window.nativeHistoryMount(true)"); await click("Generation history");
  await browser("wait", 'input[type="file"]');
  stage = "lost import acknowledgement and original retry";
  const beforeUnknown = writes().length; cutWrites = true;
  await browser("upload", 'input[type="file"]', resolve(generationFile));
  await browser("wait", "--fn", "Array.from(document.querySelectorAll('button')).some(b=>b.textContent.trim()==='Retry the same import'&&!b.disabled)");
  const unknown = writes().slice(beforeUnknown); assert(unknown.length > 0);
  const original = unknown[0].body;
  assert.deepEqual(JSON.parse(original), { expected_profile_revision: entry.profile_revision, generation_bytes: originalBytes });
  assert(unknown.every(value => value.body === original));
  cutWrites = false; await click("Retry the same import");
  await detail(aId);
  await browser("wait", "--fn", "!Array.from(document.querySelectorAll('button')).some(b=>b.textContent.trim()==='Retry the same import')");
  assert(writes().slice(beforeUnknown).every(value => value.body === original));
  assert.deepEqual(await native(route), initial, "idempotent retries cannot add or change generations");
  assert.deepEqual(await native(`${route}/${aId}`), a, "native original bytes and quarantine remain exact");
  assert.deepEqual(await native("/api/webchat/v2/prefixes"), profiles, "history cannot activate or mutate profiles");
  assert(requests.every(value => value.method === "GET" || (value.method === "POST" && value.path === route)));
  console.log(await browser("snapshot", "-i"));
  console.log("native-prefix-history-browser: passed");
  }
} catch (error) {
  // Transport/timing diagnostics contain identities/statuses only, no original
  // source bytes, request bodies, bearer values or fabricated success data.
  console.error(JSON.stringify({ stage, requests: requests.map(({ method, path, complete, status }) => ({ method, path, complete, status })), held: held.map(value => ({ method: value.record.method, path: value.record.path })) }));
  primaryFailure = error;
} finally {
  const cleanupFailures=[];
  try { release(); } catch(error) { cleanupFailures.push(error); }
  try { await browser("close"); } catch(error) { cleanupFailures.push(error); }
  try {
    server.closeAllConnections();
    await new Promise((accept,reject)=>server.close(error=>error?reject(error):accept()));
  } catch(error) { cleanupFailures.push(error); }
  if(primaryFailure&&cleanupFailures.length) throw new AggregateError([primaryFailure,...cleanupFailures],"prefix browser check and cleanup failed");
  if(primaryFailure) throw primaryFailure;
  if(cleanupFailures.length) throw new AggregateError(cleanupFailures,"prefix browser cleanup failed");
}
