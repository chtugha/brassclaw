// Actual native router/PG backend. Only browser acknowledgements are cut;
// no draft, validation or storage response is invented by the relay.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import http from "node:http";
import { resolve, sep, extname } from "node:path";
import { spawn } from "node:child_process";
import { once } from "node:events";
const [upstream, source, specificationJson] = process.argv.slice(2);
const specification = JSON.parse(specificationJson);
const target = new URL(upstream); assert.equal(target.hostname, "127.0.0.1");
const path = `/api/webchat/v2/prefix-model-specifications/${specification.id}/setup-draft`;
const root = resolve(source);
const index = await readFile(resolve(root, "index.html"), "utf8");
const importMap = index.match(/<script type="importmap"[^>]*>([\s\S]*?)<\/script>/)[1];
const page = `<!doctype html><html><head><meta charset="utf-8"><title>Native model draft acceptance</title>
<script type="importmap">${importMap}</script><link rel="stylesheet" href="/styles/app.css"></head>
<body><div id="root"></div><script>sessionStorage.setItem('brassclaw_token','catalogue-acceptance');localStorage.setItem('brassclaw_language','en');</script>
<script type="module">
import {React,html} from '/js/lib/html.js';
import {createRoot} from 'react-dom/client';
import '/js/i18n/en.js';
import {I18nProvider,useT} from '/js/lib/i18n.js';
import {PrefixModelSetupDraft} from '/js/pages/settings/components/prefix-model-setup-draft.js';
const specification=${JSON.stringify(specification).replaceAll("<", "\\u003c")};
const root=createRoot(document.getElementById('root'));
function App(){const t=useT();return html\`<\${PrefixModelSetupDraft} specification=\${specification} t=\${t}/>\`;}
window.nativeSetupMount=show=>root.render(show?html\`<\${I18nProvider}><\${App}/><//>\`:null);
window.nativeSetupMount(true);
</script></body></html>`;
const requests = [];
// Hold faults until the explicit test retry. Chromium may transparently
// repeat an idempotent PUT/GET after a dropped socket; native operations and
// their original IDs remain real on every such request.
let cutWriteResponses = false;
let cutOriginalRead = false;
let cutCurrentReads = false;
const mime = { ".js": "text/javascript", ".css": "text/css", ".svg": "image/svg+xml", ".png": "image/png", ".mjs": "text/javascript" };
const server = http.createServer(async (req, res) => {
  try {
    const path = new URL(req.url, "http://127.0.0.1").pathname;
    if (path.startsWith("/api/")) {
      if (req.method === "GET" && ((cutOriginalRead && req.url.includes("?revision=")) || (cutCurrentReads && !req.url.includes("?")))) {
        req.resume(); res.destroy(); return;
      }
      const record = { method: req.method, path, body: "" };
      requests.push(record);
      req.on("data", bytes => { record.body += bytes; });
      const loseResponse = req.method === "PUT" && cutWriteResponses;
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
const session = `brassclaw-model-setup-${process.pid}`;
// Windows runners can point at the installed native .exe, avoiding a shell shim.
const browserExecutable = process.env.AGENT_BROWSER_EXECUTABLE || "agent-browser";
function browser(...args) {
  return new Promise((accept, reject) => {
    // Use the CLI's documented action-timeout default in this fixture even
    // when another desktop task configured longer waits in its environment.
    const command = spawn(browserExecutable, ["--session", session, ...args], { stdio: ["ignore", "pipe", "pipe"], env: { ...process.env, AGENT_BROWSER_DEFAULT_TIMEOUT: "25000" } });
    let stdout = "", stderr = "";
    command.stdout.on("data", bytes => { stdout += bytes; });
    command.stderr.on("data", bytes => { stderr += bytes; });
    command.on("error", reject);
    command.on("close", code => code === 0 ? accept(stdout) : reject(Error(`${args.join(" ")} failed: ${stderr} ${stdout}`)));
  });
}
async function read() {
  const response = await fetch(upstream + path, { headers: { authorization: "Bearer catalogue-acceptance" } });
  assert.equal(response.status, 200); return response.json();
}
async function externalSave(expected, edits) {
  const response = await fetch(upstream + path, { method: "PUT", headers: { authorization: "Bearer catalogue-acceptance", "content-type": "application/json" }, body: JSON.stringify({ operation_id: crypto.randomUUID(), expected_revision: expected, edits }) });
  assert.equal(response.status, 200); return response.json();
}
async function waitSaveDisabled() {
  await browser("wait", "--fn", "Array.from(document.querySelectorAll('button')).some(button => button.textContent.trim() === 'Save draft inputs' && button.disabled)");
}
async function waitRetryReady() {
  await browser("wait", "--fn", "Array.from(document.querySelectorAll('button')).some(button => button.textContent.trim() === 'Check / retry original save' && !button.disabled)");
}
async function waitRevision(revision) { await browser("wait", `[data-setup-revision="${revision}"]`); }
async function click(label) {
  await browser("wait", "--fn", `Array.from(document.querySelectorAll('button')).some(button => button.textContent.trim() === ${JSON.stringify(label)} && !button.disabled)`);
  const snapshot = await browser("snapshot", "-i"); console.log(snapshot);
  const row = snapshot.split("\n").find(line => line.includes(`button "${label}"`));
  assert(row && !/\[[^\]]*\bdisabled\b/.test(row), `missing enabled ${label}: ${snapshot}`);
  const ref = row.match(/\bref=(e[0-9]+)\b/); assert(ref, `missing button ref: ${row}`);
  await browser("click", "@" + ref[1]);
}
async function fill(field, value) {
  await browser("fill", `[data-setup-field="${field}"]`, value);
}
const writes = () => requests.filter(record => record.method === "PUT");
try {
  const initial = await read(); assert.equal(initial.revision, 2);
  await browser("open", origin); await waitRevision(2);
  console.log(await browser("snapshot", "-i"));
  assert((await browser("get", "text", "body")).includes("Saving does not change provider settings"));
  await fill("max_prefix_tokens", "0"); await fill("max_prompt_tokens", "999");
  await click("Save draft inputs"); await waitRevision(3);
  const saved = await read(); assert.equal(saved.revision, 3);
  assert.deepEqual(saved.fields.max_prefix_tokens, { value: 0, origin: "manual" });
  assert.deepEqual(saved.fields.max_prompt_tokens, { value: 999, origin: "manual" });
  assert.deepEqual(saved.fields.reserved_output_tokens, { value: null, origin: "manual" });
  await fill("max_prompt_tokens", "1000");
  const newer = await externalSave(3, [{ field: "reserved_output_tokens", value: 10 }]);
  assert.equal(newer.revision, 4);
  await click("Save draft inputs"); await browser("wait", "--text", "A newer draft excluded this edit");
  await browser("wait", "--fn", "document.querySelector('[data-setup-field=max_prompt_tokens]').value === '1000'");
  await waitSaveDisabled();
  await click("Discard local edits and reload"); await waitRevision(4);
  assert.equal((await read()).fields.max_prompt_tokens.value, 999);
  await fill("max_prefix_tokens", "5");
  cutWriteResponses = true; cutOriginalRead = true;
  await click("Save draft inputs"); await browser("wait", "--text", "The original save is unresolved"); await waitRetryReady();
  assert.equal((await read()).revision, 5);
  const originalBody = writes().at(-1).body;
  await browser("eval", "window.nativeSetupMount(false)");
  await browser("wait", "--fn", "document.querySelector('[data-setup-specification]') === null");
  await browser("eval", "window.nativeSetupMount(true)");
  await browser("wait", "--text", "The original save is unresolved"); await waitRetryReady();
  await browser("wait", "--fn", "Array.from(document.querySelectorAll('[data-setup-field]')).length === 4 && Array.from(document.querySelectorAll('[data-setup-field]')).every(input => input.disabled)");
  await waitSaveDisabled();
  cutOriginalRead = false; cutWriteResponses = false;
  await click("Check / retry original save"); await waitRevision(5);
  assert.equal(writes().at(-1).body, originalBody, "retry must preserve exact original UUID, CAS and edits");
  assert.equal((await read()).revision, 5, "native original retry must not append");
  // A single-key replacement with the same value is still an explicit input;
  // retain that edit together with the clear, rather than relying on onChange.
  await browser("click", '[data-setup-field="max_prefix_tokens"]');
  await browser("eval", "document.querySelector('[data-setup-field=max_prefix_tokens]').select()");
  await browser("press", "5");
  await browser("wait", "--fn", "document.querySelector('[data-setup-field=max_prefix_tokens]').value === '5'");
  await fill("reserved_output_tokens", "");
  cutCurrentReads = true;
  await click("Save draft inputs"); await browser("wait", "--text", "The original save is unresolved"); await waitRetryReady();
  const count = writes().length; cutCurrentReads = false;
  const clearRequest = JSON.parse(writes().at(-1).body);
  assert.deepEqual(clearRequest.edits, [{ field: "max_prefix_tokens", value: 5 }, { field: "reserved_output_tokens", value: null }], "explicit same-value input and clear must both retain Manual intent");
  await click("Check / retry original save"); await waitRevision(6);
  assert.equal(writes().length, count, "acknowledged original retries read head without writing again");
  assert.deepEqual((await read()).fields.reserved_output_tokens, { value: null, origin: "manual" });
  await fill("max_prompt_tokens", "0");
  await browser("wait", "--text", "Enter exact whole token counts");
  await waitSaveDisabled();
  await click("Discard local edits and reload"); await waitRevision(6);
  assert.equal((await read()).revision, 6);
  assert(requests.every(record => record.path === path), "draft editor must never mutate provider settings, generate or activate");
  console.log("native-model-setup-draft-browser: passed");
} finally {
  await browser("close").catch(() => {}); await new Promise(resolve => server.close(resolve));
}
