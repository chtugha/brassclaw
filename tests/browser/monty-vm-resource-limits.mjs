// Run via BRASSCLAW_TOOL_POLICY_BROWSER_ACCEPTANCE from the native global
// runtime regression. Uses its real router/facade/PostgreSQL, original static
// modules and real React/Query in Chromium. No network responses are mocked.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import http from "node:http";
import { resolve, sep, extname } from "node:path";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { fileURLToPath } from "node:url";

// Bound the pinned native CLI itself; its Node wrapper does not forward signals.
// Keep the explicit native-executable override used by Windows runners.
const platform = process.platform === "linux" && !process.report.getReport().header.glibcVersionRuntime
  ? "linux-musl" : process.platform;
const browserExecutable = process.env.AGENT_BROWSER_EXECUTABLE || fileURLToPath(new URL(
  `./node_modules/agent-browser/bin/agent-browser-${platform}-${process.arch}${process.platform === "win32" ? ".exe" : ""}`, import.meta.url,
));
const commandTimeoutMs = Number(process.env.BRASSCLAW_BROWSER_COMMAND_TIMEOUT_MS || 30000);
assert(Number.isSafeInteger(commandTimeoutMs) && commandTimeoutMs > 0 && commandTimeoutMs <= 2147483647);

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
function browser(...args) {
  return new Promise((accept, reject) => {
    const process = spawn(browserExecutable, ["--session", session, ...args], {
      stdio: ["ignore", "pipe", "pipe"], timeout: commandTimeoutMs,
    });
    let stdout = "", stderr = "";
    process.stdout.on("data", bytes => { stdout += bytes; });
    process.stderr.on("data", bytes => { stderr += bytes; });
    process.on("error", reject);
    process.on("close", (code, signal) => code === 0 ? accept(stdout)
      : reject(Error(`${args.join(" ")} failed (${signal ?? code}): ${stderr} ${stdout}`)));
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
// Both normal restoration and failure cleanup cover the same complete edits.
const editedFields = {
  oauth_token_timeout_millis: "OAuth token timeout (ms)",
  oauth_token_response_body_bytes: "OAuth token response limit (bytes)",
  oauth_dcr_timeout_millis: "OAuth registration HTTP timeout (ms)",
  oauth_dcr_response_body_bytes: "OAuth registration HTTP body limit (bytes)",
  oauth_dcr_metadata_cache_ttl_secs: "OAuth metadata cache lifetime (s)",
  auth_backend_timeout_millis: "ProductAuth backend wait (ms)",
  manual_token_setup_ttl_secs: "Manual-token setup lifetime (s)",
  extension_manual_token_setup_ttl_secs: "Extension manual-token setup lifetime (s)",
  turn_gate_oauth_setup_ttl_secs: "Task-gate OAuth setup lifetime (s)",
  oauth_setup_default_ttl_secs: "Standalone OAuth setup default lifetime (s)",
  oauth_setup_max_ttl_secs: "Standalone OAuth setup maximum lifetime (s)",
  default_memory_tree_depth: "Default Memory tree depth",
  max_memory_tree_depth: "Maximum requested Memory tree depth",
  max_mcp_request_bytes: "MCP request bytes",
  max_mcp_response_bytes: "MCP response bytes",
};
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
    assert(Object.values(editedFields).includes(label), "edited field must be registered for restoration: "+label);
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
  await fillObserved("OAuth metadata cache lifetime (s)","0");
  assert((await browser("get", "text", "#monty-oauth_dcr_metadata_cache_ttl_secs-help")).includes("0 disables reuse"));
  await fillObserved("OAuth registration HTTP timeout (ms)","49000");
  await fillObserved("OAuth registration HTTP body limit (bytes)","131079");
  assert((await browser("get", "text", "#monty-oauth_dcr_timeout_millis-help")).includes("discovery"));
  assert((await browser("get", "text", "#monty-oauth_dcr_response_body_bytes-help")).includes("without truncation"));
  await fillObserved("OAuth token timeout (ms)","47000");
  await fillObserved("OAuth token response limit (bytes)","32769");
  await fillObserved("ProductAuth backend wait (ms)","61000");
  await fillObserved("Manual-token setup lifetime (s)","1777");
  await fillObserved("Extension manual-token setup lifetime (s)","1555");
  await fillObserved("Task-gate OAuth setup lifetime (s)","1779");
  await fillObserved("Standalone OAuth setup maximum lifetime (s)","1800");
  await fillObserved("Standalone OAuth setup default lifetime (s)","1700");
  assert((await browser("get", "text", "#monty-oauth_setup_default_ttl_secs-help")).includes("first expiry"));
  assert((await browser("get", "text", "#monty-oauth_setup_max_ttl_secs-help")).includes("never clipped"));
  assert((await browser("get", "text", "#monty-manual_token_setup_ttl_secs-help")).includes("first expiry"));
  assert((await browser("get", "text", "#monty-extension_manual_token_setup_ttl_secs-help")).includes("confirmed server expiry"));
  assert((await browser("get", "text", "#monty-turn_gate_oauth_setup_ttl_secs-help")).includes("first expiry"));
  assert((await browser("get", "text", "#monty-auth_backend_timeout_millis-help")).includes("committed work"));
  assert((await browser("get", "text", "#monty-oauth_token_timeout_millis-help")).includes("acknowledged revision"));
  assert((await browser("get", "text", "#monty-oauth_token_response_body_bytes-help")).includes("without truncation"));
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
  assert.equal(changed.execution_limits.oauth_dcr_metadata_cache_ttl_secs,0);
  assert.equal(changed.execution_limits.oauth_dcr_timeout_millis,49000);
  assert.equal(changed.execution_limits.oauth_dcr_response_body_bytes,131079);
  assert.equal(changed.execution_limits.oauth_token_timeout_millis,47000);
  assert.equal(changed.execution_limits.oauth_token_response_body_bytes,32769);
  assert.equal(changed.execution_limits.auth_backend_timeout_millis,61000);
  assert.equal(changed.execution_limits.manual_token_setup_ttl_secs,1777);
  assert.equal(changed.execution_limits.extension_manual_token_setup_ttl_secs,1555);
  assert.equal(changed.execution_limits.turn_gate_oauth_setup_ttl_secs,1779);
  assert.equal(changed.execution_limits.oauth_setup_default_ttl_secs,1700);
  assert.equal(changed.execution_limits.oauth_setup_max_ttl_secs,1800);
  assert.equal(changed.execution_limits.default_memory_tree_depth,2);
  assert.equal(changed.execution_limits.max_memory_tree_depth,12);
  assert.equal(changed.execution_limits.max_mcp_request_bytes,2097153);
  assert.equal(changed.execution_limits.max_mcp_response_bytes,3145729);
  assert.equal(changed.revision,initial.revision+1);
  const effective = (await status()).execution_limits;
  assert.equal(effective.effective_revision,changed.revision);
  assert.equal(effective.limits.oauth_dcr_metadata_cache_ttl_secs,0);
  assert.equal(effective.limits.oauth_dcr_timeout_millis,49000);
  assert.equal(effective.limits.oauth_dcr_response_body_bytes,131079);
  assert.equal(effective.limits.oauth_token_timeout_millis,47000);
  assert.equal(effective.limits.oauth_token_response_body_bytes,32769);
  assert.equal(effective.limits.auth_backend_timeout_millis,61000);
  assert.equal(effective.limits.manual_token_setup_ttl_secs,1777);
  assert.equal(effective.limits.extension_manual_token_setup_ttl_secs,1555);
  assert.equal(effective.limits.turn_gate_oauth_setup_ttl_secs,1779);
  assert.equal(effective.limits.oauth_setup_default_ttl_secs,1700);
  assert.equal(effective.limits.oauth_setup_max_ttl_secs,1800);
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
  assert((await browser("get", "text", "#monty-oauth_dcr_metadata_cache_ttl_secs-help")).includes("0 deaktiviert"));
  assert((await browser("get", "text", "#monty-oauth_dcr_timeout_millis-help")).includes("keine automatische Wiederholung"));
  assert((await browser("get", "text", "#monty-oauth_dcr_response_body_bytes-help")).includes("bestätigte Revision"));
  assert((await browser("get", "text", "#monty-oauth_token_timeout_millis-help")).includes("bestätigte Revision"));
  assert((await browser("get", "text", "#monty-oauth_token_response_body_bytes-help")).includes("erneutes Senden"));
  assert((await browser("get", "text", "#monty-auth_backend_timeout_millis-help")).includes("Original prüfen"));
  assert((await browser("get", "text", "#monty-manual_token_setup_ttl_secs-help")).includes("erste Ablaufzeit"));
  assert((await browser("get", "text", "#monty-extension_manual_token_setup_ttl_secs-help")).includes("Server-Ablaufzeit"));
  assert((await browser("get", "text", "#monty-turn_gate_oauth_setup_ttl_secs-help")).includes("erste Ablaufzeit"));
  assert((await browser("get", "text", "#monty-oauth_setup_default_ttl_secs-help")).includes("erste Ablaufzeit"));
  assert((await browser("get", "text", "#monty-oauth_setup_max_ttl_secs-help")).includes("weder gekürzt"));
  assert((await browser("get", "text", "#monty-default_memory_tree_depth-help")).includes("keine Tiefe"));
  assert((await browser("get", "text", "#monty-max_memory_tree_depth-help")).includes("stiller Begrenzung"));
  assert((await browser("get", "text", request+"-help")).includes("bestätigte Grenze"));
  assert((await browser("get", "text", response+"-help")).includes("automatische Wiederholung"));
  await browser("open", origin);
  await browser("wait", "--load", "networkidle");
  await browser("scrollintoview", "summary");
  await browser("click", "summary");
  for (const [field, label] of Object.entries(editedFields)) {
    const snapshot=await browser("snapshot","-i");
    const row=snapshot.split("\n").find(line=>line.includes('spinbutton "'+label+'"'));
    const ref=row?.match(/\[ref=(e[0-9]+)\]/)?.[1];
    assert(ref,"visible restore field missing: "+label);
    await browser("scrollintoview","@"+ref);
    await browser("fill","@"+ref,String(initial.execution_limits[field]));
  }
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
      const edited=Object.keys(editedFields);
      if (edited.some(field=>current.execution_limits[field]!==initial.execution_limits[field])) {
        const limits={...current.execution_limits};
        for (const field of edited) limits[field]=initial.execution_limits[field];
        const response=await fetch(upstream+"/api/settings/monty-vm",{method:"PUT",headers:{"content-type":"application/json"},body:JSON.stringify({expected_revision:current.revision,execution_limits:limits})});
        assert.equal(response.status,200);
      }
    }
  } finally {
    try { await browser("close"); }
    finally {const closed=once(server,"close");server.close();server.closeAllConnections();await closed;}
  }
}
