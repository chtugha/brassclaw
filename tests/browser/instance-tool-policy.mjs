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
const page = `<!doctype html><html><head><meta charset="utf-8"><title>Native Tool policy acceptance</title>
<script type="importmap">${importMap}</script><link rel="stylesheet" href="/styles/app.css"></head>
<body><div id="root"></div><script>
localStorage.setItem("brassclaw_language","en");
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
import {ToolsTab} from '/js/pages/settings/components/tools-tab.js';
const client=new QueryClient({defaultOptions:{queries:{retry:false}}});
createRoot(document.getElementById('root')).render(html\`<\${QueryClientProvider} client=\${client}><\${I18nProvider}><\${ToolsTab}/><//><//>\`);
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
const session = `brassclaw-tool-policy-${process.pid}`;
function browser(...args) {
  return new Promise((accept, reject) => {
    const process = spawn("agent-browser", ["--session", session, ...args], { stdio: ["ignore", "pipe", "pipe"] });
    let stdout = "", stderr = "";
    process.stdout.on("data", bytes => { stdout += bytes; });
    process.stderr.on("data", bytes => { stderr += bytes; });
    process.on("error", reject);
    process.on("close", code => code === 0 ? accept(stdout) : reject(Error(`${args.join(" ")} failed: ${stderr} ${stdout}`)));
  });
}
async function policies() {
  const response = await fetch(upstream + "/api/settings/tool-policies");
  assert.equal(response.status, 200); return response.json();
}
let toolId, initial;
try {
  initial = (await policies()).find(value => value.capability_ids.includes("host.post_reply"));
  assert.equal(initial.enabled, true); toolId = initial.tool_id;
  const selector = `select[aria-label="Policy for Tool ${toolId}"]`;
  await browser("open", origin);
  await browser("wait", "--load", "networkidle");
  console.log(await browser("snapshot", "-i"));
  console.log(await browser("errors"));
  await browser("wait", selector);
  await browser("select", selector, "deny");
  await browser("wait", "--fn", `document.querySelector(${JSON.stringify(selector)}).value === 'deny' && !document.querySelector(${JSON.stringify(selector)}).disabled`);
  const blocked = (await policies()).find(value => value.tool_id === toolId);
  assert.equal(blocked.enabled, false); assert.equal(BigInt(blocked.revision), BigInt(initial.revision) + 1n);
  // Real concurrent operator write. The old browser revision must receive 409;
  // refreshing cannot automatically repeat its stale edit.
  const response = await fetch(`${upstream}/api/settings/tool-policies/${toolId}`, {
    method: "PUT", headers: { "content-type": "application/json" },
    body: JSON.stringify({ expected_revision: blocked.revision, enabled: true }),
  });
  assert.equal(response.status, 200); const concurrent = await response.json();
  await browser("select", selector, "allow");
  await browser("wait", "--text", "This policy changed since it was loaded.");
  await browser("wait", "--fn", `document.querySelector(${JSON.stringify(selector)}).disabled`);
  assert.deepEqual((await policies()).find(value => value.tool_id === toolId), concurrent);
  await browser("find", "role", "button", "click", "--name", "Refresh policies");
  await browser("wait", "--fn", `document.querySelector(${JSON.stringify(selector)}).value === 'allow' && !document.querySelector(${JSON.stringify(selector)}).disabled`);
  assert.deepEqual((await policies()).find(value => value.tool_id === toolId), concurrent);
  assert.equal(requests.filter(value => value.method === "PUT").length, 2);
  loseNextWriteResponse = true;
  await browser("select", selector, "deny");
  await browser("wait", "--text", "The previous edit may already have been saved.");
  await browser("wait", "--fn", `document.querySelector(${JSON.stringify(selector)}).disabled`);
  const uncertain = (await policies()).find(value => value.tool_id === toolId);
  assert.equal(uncertain.enabled, false);
  assert.equal(BigInt(uncertain.revision), BigInt(concurrent.revision) + 1n);
  await browser("wait", "--fn", "window.nativePolicyFetches.length === 3");
  const faultWrites = requests.filter(value => value.method === "PUT").slice(2);
  assert(faultWrites.length >= 1);
  for (const attempt of faultWrites) assert.deepEqual(JSON.parse(attempt.body), {
    expected_revision: concurrent.revision, enabled: false,
  });
  const writeCount = requests.filter(value => value.method === "PUT").length;
  await browser("find", "role", "button", "click", "--name", "Refresh policies");
  await browser("wait", "--fn", `document.querySelector(${JSON.stringify(selector)}).value === 'deny' && !document.querySelector(${JSON.stringify(selector)}).disabled`);
  assert.deepEqual((await policies()).find(value => value.tool_id === toolId), uncertain);
  assert.equal(requests.filter(value => value.method === "PUT").length, writeCount);
  await browser("wait", "--fn", "window.nativePolicyFetches.length === 3");
  await browser("select", selector, "allow");
  await browser("wait", "--fn", `document.querySelector(${JSON.stringify(selector)}).value === 'allow' && !document.querySelector(${JSON.stringify(selector)}).disabled`);
  const restored = (await policies()).find(value => value.tool_id === toolId);
  assert.equal(restored.enabled, true);
  assert.equal(BigInt(restored.revision), BigInt(uncertain.revision) + 1n);
  assert.equal(requests.filter(value => value.method === "PUT").length, writeCount + 1);
  await browser("wait", "--fn", "window.nativePolicyFetches.length === 4");
  assert(requests.every(value => value.path === "/api/settings/tool-policies" || value.path === `/api/settings/tool-policies/${toolId}`));
  console.log("Native browser policy allow/block, concurrent-edit fencing and lost committed acknowledgement passed without replay.");
} finally {
  try {
    // Preserve the fixture's original value even when an assertion fails.
    if (toolId) {
      const current = (await policies()).find(value => value.tool_id === toolId);
      if (current.enabled !== initial.enabled) {
        const restored = await fetch(`${upstream}/api/settings/tool-policies/${toolId}`, {
          method: "PUT", headers: { "content-type": "application/json" },
          body: JSON.stringify({ expected_revision: current.revision, enabled: initial.enabled }),
        });
        assert.equal(restored.status, 200);
      }
    }
  } finally {
    try { await browser("close"); }
    finally {
      const closed = once(server, "close");
      server.close(); server.closeAllConnections(); await closed;
    }
  }
}
