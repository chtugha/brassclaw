// Invoked by the actual native Auth runtime fixture; no network responses or
// lifecycle state are mocked by this driver. Only test sentinels are entered.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

const [origin, thread, run, gate] = process.argv.slice(2);
const target = new URL(origin);
assert.equal(target.protocol, "http:");
assert.equal(target.hostname, "127.0.0.1");
assert.equal(target.pathname, "/");
assert.equal(target.username, "");
assert.equal(target.password, "");
assert.ok(thread && run && gate);
const page = new URL("/__native_manual_original", target).href;
// Use the locked package's cross-platform Node entry, without a shell/.cmd shim.
const executable = fileURLToPath(new URL("./node_modules/agent-browser/bin/agent-browser.js", import.meta.url));
const session = `brassclaw-manual-original-${process.pid}`;
const firstToken = "native-original-fixture-token-1";
const changedToken = "native-original-fixture-token-2";
const input = 'input[aria-label="Access token"]';
const unconfirmed = "The operation could not be confirmed. Retry with the same token or check the saved setup.";

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

async function inspect() {
  return evaluate(`({
    audit: window.manualOriginalAudit,
    originals: Object.entries(sessionStorage).filter(([key]) => key.startsWith("brassclaw:manual-token-original:")).map(([,value]) => JSON.parse(value)),
    storage: JSON.stringify(Object.entries(sessionStorage)),
    visible: document.body.innerText,
    token_empty: document.querySelector(${JSON.stringify(input)})?.value === ""
  })`);
}

function privateAndOriginal(state, original) {
  assert.equal(state.originals.length, 1);
  assert.deepEqual(state.originals[0].identity, original.identity);
  assert.equal(state.originals[0].invocationId, original.invocationId);
  for (const token of [firstToken, changedToken]) {
    assert.equal(state.storage.includes(token), false);
    assert.equal(state.visible.includes(token), false);
  }
  for (const call of state.audit) {
    assert.deepEqual(call.identity, original.identity);
    assert.equal(call.invocation_id, original.invocationId);
    assert.equal(call.retained_before_transport, true);
    if (call.token_present) assert.equal(call.token_matches_input, true);
  }
}

async function useToken(token, expectedCalls, expectedStatus) {
  await browser("snapshot", "-i");
  // This password field is named by aria-label. The pinned CLI's label finder
  // does not resolve it; use the exact live selector already verified by wait.
  await browser("fill", input, token);
  await browser("find", "role", "button", "click", "--name", "Use token", "--exact");
  await browser("wait", "--fn", `window.manualOriginalAudit.length === ${expectedCalls} && window.manualOriginalAudit.at(-1).http_status === ${expectedStatus}`);
  await browser("wait", "--text", unconfirmed);
  await browser("wait", "--fn", `!document.querySelector(${JSON.stringify(input)}).disabled`);
}

try {
  await browser("open", page);
  await browser("wait", input);
  await useToken(firstToken, 1, 503);
  let state = await inspect();
  assert.equal(state.originals.length, 1);
  const original = state.originals[0];
  assert.equal(original.setup, null);
  assert.equal(original.closed, false);
  assert.deepEqual(original.definition, {
    provider: "manual-browser-provider", accountLabel: "manual-browser-provider",
  });
  privateAndOriginal(state, original);
  assert.equal(state.audit[0].path.endsWith("/setup"), true);
  assert.equal(state.audit[0].token_present, false);

  // Full reload clears the controlled password input but keeps exact metadata.
  await browser("open", page);
  await browser("wait", input);
  state = await inspect();
  assert.equal(state.token_empty, true);
  privateAndOriginal(state, original);
  await useToken(firstToken, 3, 503);
  state = await inspect();
  privateAndOriginal(state, original);
  const expiry = state.originals[0].setup.expiresAt;
  assert.equal(Number.isFinite(Date.parse(expiry)), true);
  assert.deepEqual(state.audit.map(call => call.http_status), [503, 200, 503]);
  assert.equal(state.audit.at(-1).path.endsWith("/secret-submit"), true);
  assert.equal(state.audit.at(-1).token_present, true);

  // A different supplied token must be proved and rejected, not skipped in
  // favour of the earlier account. This must not resume the blocked gate.
  await useToken(changedToken, 5, 409);
  state = await inspect();
  privateAndOriginal(state, original);
  assert.equal(state.originals[0].closed, false);
  assert.equal(state.originals[0].setup.expiresAt, expiry);
  assert.equal(state.originals[0].setup.status, "completed");
  assert.equal(state.audit.filter(call => call.path.endsWith("/continue")).length, 0);
  assert.equal(state.audit.filter(call => call.token_present).length, 2);

  // Distinct explicit reference-only action: same identity, zero token fields.
  await browser("snapshot", "-i");
  await browser("find", "role", "button", "click", "--name", "Continue saved setup", "--exact");
  await browser("wait", "--fn", "window.manualOriginalAudit.length === 7 && window.manualOriginalAudit.at(-1).http_status === 200");
  await browser("wait", "--fn", `!document.querySelector(${JSON.stringify(input)})`);
  state = await inspect();
  privateAndOriginal(state, original);
  assert.equal(state.originals[0].closed, true);
  assert.equal(state.originals[0].setup.expiresAt, expiry);
  const references = state.audit.filter(call => call.path.endsWith("/continue"));
  assert.equal(references.length, 1);
  assert.equal(references[0].token_present, false);
  assert.deepEqual(references[0].keys, ["identity", "invocation_id", "thread_id"]);
  assert.equal(state.audit.filter(call => call.path.endsWith("/submit")).length, 0);
  for (const call of state.audit.filter(call => call.reply)) {
    assert.deepEqual(call.reply.identity, original.identity);
    assert.equal(call.reply.invocation_id, original.invocationId);
    if (call.path.endsWith("/setup")) assert.equal(call.reply.expires_at, expiry);
  }
  const privacy = await evaluate(`(async () => {
    const {fetchTimeline} = await import('/js/lib/api.js');
    const timeline = JSON.stringify(await fetchTimeline({threadId:${JSON.stringify(thread)}}));
    return {contains_fixture_token: [${JSON.stringify(firstToken)}, ${JSON.stringify(changedToken)}].some(value => timeline.includes(value))};
  })()`);
  assert.equal(privacy.contains_fixture_token, false);
  console.log(JSON.stringify({
    kind: "native-manual-original-browser", thread, run, gate,
    http_statuses: state.audit.map(call => call.http_status),
    setup_calls: 4, raw_calls: 2, explicit_reference_calls: references.length,
    original_retained: true, first_expiry_retained: true, raw_privacy_checked: true,
  }));
  console.log("native-manual-original-browser: passed");
} catch (error) {
  // Diagnose CLI/daemon/Chromium failures before issuing further browser actions.
  try { console.error(await browser("doctor", "--offline", "--quick")); }
  catch (diagnosticError) { console.error(diagnosticError.message); }
  try {
    console.error(JSON.stringify(await evaluate(`({
      inputs: Array.from(document.querySelectorAll('input')).map(element => ({type:element.type, label:element.getAttribute('aria-label'), disabled:element.disabled})),
      buttons: Array.from(document.querySelectorAll('button')).map(element => ({text:element.textContent.trim(), disabled:element.disabled})),
      audit: window.manualOriginalAudit
    })`)));
  } catch (diagnosticError) { console.error(diagnosticError.message); }
  throw error;
} finally {
  await browser("close");
}
