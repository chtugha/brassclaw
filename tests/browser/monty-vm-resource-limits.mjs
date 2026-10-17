// Run via BRASSCLAW_TOOL_POLICY_BROWSER_ACCEPTANCE from the native global
// runtime regression. Uses its real router/facade/PostgreSQL, original static
// modules and real React/Query in Chromium. No network responses are mocked.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import http from "node:http";
import { resolve, sep, extname } from "node:path";
import { spawn } from "node:child_process";
import { once } from "node:events";

const [upstream, source] = process.argv.slice(2);
const target = new URL(upstream);
assert.equal(target.hostname, "127.0.0.1");
const root = resolve(source);
const index = await readFile(resolve(root, "index.html"), "utf8");
const importMap = index.match(/<script type="importmap"[^>]*>([\s\S]*?)<\/script>/)[1];
const page = `<!doctype html><html><head><meta charset="utf-8"><title>Native Monty resource acceptance</title>
<script type="importmap">${importMap}</script><link rel="stylesheet" href="/styles/app.css"></head>
<body><div id="root"></div><script>
localStorage.setItem("brassclaw_language",new URLSearchParams(location.search).get("lang") || "en");
window.nativePolicyFetches=[];
const originalFetch=window.fetch.bind(window);
window.fetch=(path,options)=>{
  if(options?.method==='PUT') window.nativePolicyFetches.push({path,body:options.body});
  return originalFetch(path,options);
};
</script><script type="module">
import {React,html} from '/js/lib/html.js';
import {createRoot} from 'react-dom/client';
import {QueryClient,QueryClientProvider} from '@tanstack/react-query';
import {I18nProvider} from '/js/lib/i18n.js';
import '/js/i18n/en.js';
import '/js/i18n/de.js';
import {MontyVmTab} from '/js/pages/settings/components/monty-vm-tab.js';
const client=new QueryClient({defaultOptions:{queries:{retry:false}}});
createRoot(document.getElementById('root')).render(html\`<\${QueryClientProvider} client=\${client}><\${I18nProvider}><\${MontyVmTab}/><//><//>\`);
</script></body></html>`;
const requests = [];
let loseNextWriteResponse = false;
const mime = { ".js": "text/javascript", ".css": "text/css", ".svg": "image/svg+xml", ".png": "image/png", ".mjs": "text/javascript" };
const server = http.createServer(async (req, res) => {
  try {
    const path = new URL(req.url, "http://127.0.0.1").pathname;
    if (path.startsWith("/api/")) {
      const record = { method: req.method, path, body: "" };
      requests.push(record);
      req.on("data", bytes => { record.body += bytes; });
      const loseResponse = req.method === "PUT" && loseNextWriteResponse;
      if (loseResponse) loseNextWriteResponse = false;
      const forward = http.request(new URL(req.url, target), {
        method: req.method, headers: { ...req.headers, host: target.host },
      }, response => {
        // Fault injection after the real upstream committed: close the browser
        // transport instead of inventing an HTTP response or saved result.
        if (loseResponse) { response.resume(); res.destroy(); return; }
        res.writeHead(response.statusCode, response.headers); response.pipe(res);
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
const origin = `http://127.0.0.1:${server.address().port}`;
const session = `brassclaw-monty-limits-${process.pid}`;
// Windows runners can point at the installed native .exe, avoiding a shell shim.
const browserExecutable = process.env.AGENT_BROWSER_EXECUTABLE || "agent-browser";
function browser(...args) {
  return new Promise((accept, reject) => {
    const process = spawn(browserExecutable, ["--session", session, ...args], { stdio: ["ignore", "pipe", "pipe"] });
    let stdout = "", stderr = "";
    process.stdout.on("data", bytes => { stdout += bytes; });
    process.stderr.on("data", bytes => { stderr += bytes; });
    process.on("error", reject);
    process.on("close", code => code === 0 ? accept(stdout) : reject(Error(`${args.join(" ")} failed: ${stderr} ${stdout}`)));
  });
}
async function settings() {
  const response = await fetch(upstream + "/api/settings/monty-vm");
  assert.equal(response.status, 200); return (await response.json()).settings;
}
async function status() {
  const response = await fetch(upstream + "/api/settings/monty-vm/status");
  assert.equal(response.status, 200); return response.json();
}
const request = "#monty-max_mcp_request_bytes";
const response = "#monty-max_mcp_response_bytes";
let initial;
try {
  initial = await settings();
  await browser("open", origin);
  await browser("wait", "--load", "networkidle");
  console.log(await browser("snapshot", "-i"));
  await browser("scrollintoview", "summary");
  await browser("click", "summary");
  await browser("wait", request);
  assert((await browser("get", "text", request+"-help")).includes("acknowledged limit"));
  assert((await browser("get", "text", response+"-help")).includes("without truncation"));
  console.log("Details after open:",await browser("eval","JSON.stringify(Array.from(document.querySelectorAll('details')).map(el=>el.open))"));
  async function fillObserved(label,value) {
    const snapshot=await browser("snapshot","-i");
    console.log(snapshot);
    const row=snapshot.split("\n").find(line=>line.includes('spinbutton "'+label+'"'));
    const ref=row?.match(/\[ref=(e[0-9]+)\]/)?.[1];
    assert(ref,"visible field missing: "+label);
    await browser("scrollintoview","@"+ref);
    await browser("fill","@"+ref,value);
  }
  await fillObserved("Default Memory tree depth","2");
  await fillObserved("Maximum requested Memory tree depth","12");
  assert((await browser("get", "text", "#monty-default_memory_tree_depth-help")).includes("omits depth"));
  assert((await browser("get", "text", "#monty-max_memory_tree_depth-help")).includes("silently clamped"));
  await fillObserved("MCP request bytes","2097153");
  await fillObserved("MCP response bytes","3145729");
  const actualRequest=await browser("get","value",request);
  const actualResponse=await browser("get","value",response);
  console.log("Filled DOM values:",actualRequest,actualResponse);
  assert(actualRequest.includes("2097153"));
  assert(actualResponse.includes("3145729"));
  {
    const snapshot=await browser("snapshot","-i");
    console.log(snapshot);
    const ref=snapshot.match(/button "Save"[^\n]*\[ref=(e[0-9]+)\]/)?.[1];
    assert(ref,"visible Save button missing");
    await browser("scrollintoview","@"+ref);
    await browser("click","@"+ref);
  }
  const view = await browser("get","text","body");
  assert(requests.some(value=>value.method==="PUT"),"The form did not submit: "+view);
  await browser("wait", "--text", "Saved");
  const changed = await settings();
  assert.equal(changed.execution_limits.default_memory_tree_depth,2);
  assert.equal(changed.execution_limits.max_memory_tree_depth,12);
  assert.equal(changed.execution_limits.max_mcp_request_bytes,2097153);
  assert.equal(changed.execution_limits.max_mcp_response_bytes,3145729);
  assert.equal(changed.revision,initial.revision+1);
  const effective = (await status()).execution_limits;
  assert.equal(effective.effective_revision,changed.revision);
  assert.equal(effective.limits.default_memory_tree_depth,2);
  assert.equal(effective.limits.max_memory_tree_depth,12);
  assert.equal(effective.limits.max_mcp_request_bytes,2097153);
  assert.equal(effective.limits.max_mcp_response_bytes,3145729);
  await browser("open", origin+"/?lang=de");
  await browser("wait", "--load", "networkidle");
  console.log(await browser("snapshot", "-i"));
  await browser("scrollintoview", "summary");
  await browser("click", "summary");
  // Read localized help directly; no browser/API response is fabricated.
  assert((await browser("get", "text", "#monty-default_memory_tree_depth-help")).includes("keine Tiefe"));
  assert((await browser("get", "text", "#monty-max_memory_tree_depth-help")).includes("stiller Begrenzung"));
  assert((await browser("get", "text", request+"-help")).includes("bestätigte Grenze"));
  assert((await browser("get", "text", response+"-help")).includes("automatische Wiederholung"));
  await browser("open", origin);
  await browser("wait", "--load", "networkidle");
  await browser("scrollintoview", "summary");
  await browser("click", "summary");
  await browser("scrollintoview", "#monty-default_memory_tree_depth");
  await browser("fill", "#monty-default_memory_tree_depth", String(initial.execution_limits.default_memory_tree_depth));
  await browser("scrollintoview", "#monty-max_memory_tree_depth");
  await browser("fill", "#monty-max_memory_tree_depth", String(initial.execution_limits.max_memory_tree_depth));
  await browser("scrollintoview", request);
  await browser("fill", request, String(initial.execution_limits.max_mcp_request_bytes));
  await browser("scrollintoview", response);
  await browser("fill", response, String(initial.execution_limits.max_mcp_response_bytes));
  {
    const snapshot=await browser("snapshot","-i");
    console.log(snapshot);
    const ref=snapshot.match(/button "Save"[^\n]*\[ref=(e[0-9]+)\]/)?.[1];
    assert(ref,"visible Save button missing");
    await browser("scrollintoview","@"+ref);
    await browser("click","@"+ref);
  }
  await browser("wait", "--text", "Saved");
  const restored = await settings();
  assert.deepEqual(restored.execution_limits,initial.execution_limits);
  assert.equal(requests.filter(value=>value.method==="PUT").length,2);
  console.log("Real rendered Monty byte controls, EN/DE help, persisted save, effective revision and restoration passed.");
} finally {
  try {
    if (initial) {
      const current = await settings();
      if (current.execution_limits.max_mcp_request_bytes!==initial.execution_limits.max_mcp_request_bytes || current.execution_limits.max_mcp_response_bytes!==initial.execution_limits.max_mcp_response_bytes) {
        const limits={...current.execution_limits,max_mcp_request_bytes:initial.execution_limits.max_mcp_request_bytes,max_mcp_response_bytes:initial.execution_limits.max_mcp_response_bytes};
        const response=await fetch(upstream+"/api/settings/monty-vm",{method:"PUT",headers:{"content-type":"application/json"},body:JSON.stringify({expected_revision:current.revision,execution_limits:limits})});
        assert.equal(response.status,200);
      }
    }
  } finally {
    try { await browser("close"); }
    finally {const closed=once(server,"close");server.close();server.closeAllConnections();await closed;}
  }
}
