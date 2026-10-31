// Native operator middleware/facade/PostgreSQL plus the actual React component.
// The relay delays or cuts real acknowledgements; it invents no API results.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import http from "node:http";
import { resolve, sep, extname } from "node:path";
import { spawn } from "node:child_process";
import { EventEmitter, once } from "node:events";
import { fileURLToPath } from "node:url";

const [upstream, source, generationFile] = process.argv.slice(2);
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
const initial = designView ? null : await native(route);
if (!designView) assert(initial.length >= 2, "existing native fixture must retain distinct real generations");
// Generation identity is over the complete package, not a caller-supplied field.
const imported = designView ? [] : await Promise.all(initial.map(value => native(`${route}/${value.generation}`)));
const a = imported.find(value => value.generation_bytes === originalBytes);
if (!designView) assert(a, "actual exported package must already be retained");
const b = imported.find(value => value.summary.generation !== a.summary.generation);
if (!designView) { assert(b); assert.equal(a.summary.qualification, "quarantined"); }
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
function App(){const t=useT();return ${designView ? "html`<${PrefixModelDesigns} t=${t}/>`" : "html`<${PrefixGenerationHistory} entry=${entry} t=${t}/>`"};}
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
try {
  await browser("open", `http://127.0.0.1:${server.address().port}`);
  if (designView) {
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
  throw error;
} finally {
  release();
  await browser("close").catch(error => console.error("browser cleanup failed:", error.message));
  server.closeAllConnections(); await new Promise(resolve => server.close(resolve));
}
