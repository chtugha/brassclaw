// UI transport state only. PostgreSQL/secret/commit claims are covered by the
// actual native provider service tests, not by these controlled responses.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

const source = readFileSync(new URL("./provider-mutations.js", import.meta.url), "utf8")
  .split("\n").filter(line => !line.startsWith("import ")).join("\n").replaceAll("export ", "");
function fixture() {
  const calls = [], events = []; let next = 0;
  let nonce = "original", authenticated = true;
  const auth = {
    readStoredToken: () => authenticated ? "controlled-token" : "",
    readOriginalAuthContext: () => { if (!authenticated) throw Error("Authentication required"); return { origin: "http://operator.example", nonce }; },
    originalAuthContextIsCurrent: (value, token) => authenticated && token === "controlled-token" && value.nonce === nonce,
  };
  const context = { ...auth, clientActionId: () => `00000000-0000-4000-8000-${String(++next).padStart(12, "0")}`,
    apiFetch: (path, options) => new Promise((resolve, reject) => calls.push({ path, options, resolve, reject })),
    Event: class { constructor(type) { this.type = type; } }, dispatchEvent: event => events.push(event.type) };
  vm.createContext(context);
  vm.runInContext(source + "\nglobalThis.api = { submitProviderMutation, pendingProviderMutations, readProviderMutation, reconcileProviderMutation, subscribeProviderBootstrap };", context);
  return { calls, events, ...context.api, changeAuth() { nonce = "successor"; }, logout() { authenticated = false; }, restoreAuth() { nonce = "original"; authenticated = true; } };
}
const payload = () => ({ id: "local", adapter: "open_ai_completions", expected_revision: 4, api_key: "fixture-private-key" });
const submit = (f, p = payload()) => f.submitProviderMutation("upsert", "local", "/providers", p);
const request = call => JSON.parse(call.options.body);
function receipt(id, operation = "upsert") { return { mutation_id: id, operation, provider_id: "local", provider_revision: 5, role_selections: {} }; }
function reject(status) { const error = new Error("controlled transport failure"); error.status = status; return error; }
const tick = () => new Promise(resolve => setImmediate(resolve));

test("concurrent clicks share exact bytes and one original identity without exposing the key in status", async () => {
  const f = fixture(), a = submit(f), b = submit(f, { ...payload() });
  assert.equal(f.calls.length, 1);
  const id = request(f.calls[0]).mutation_id;
  assert.equal(request(f.calls[0]).api_key, payload().api_key);
  assert.equal(JSON.stringify(f.pendingProviderMutations()).includes(payload().api_key), false);
  f.calls[0].resolve({ mutation_receipt: receipt(id) });
  await Promise.all([a, b]); assert.equal(f.pendingProviderMutations().length, 0);
});

test("lost reply reads the original then reconciles without resending an old provider payload", async () => {
  const f = fixture(), work = submit(f); const id = request(f.calls[0]).mutation_id;
  f.calls[0].reject(reject(503)); await tick();
  assert.equal(f.calls[1].path, `/api/webchat/v2/llm/mutations/${id}`);
  f.calls[1].resolve(receipt(id)); await tick();
  assert.equal(f.calls[2].path, `/api/webchat/v2/llm/mutations/${id}/reconcile`);
  assert.equal(f.calls[2].options.method, "POST"); assert.equal(f.calls[2].options.body, undefined);
  f.calls[2].resolve({ mutation_receipt: receipt(id), providers: [{ id: "local", revision: 6 }] });
  const response = await work; assert.equal(response.providers[0].revision, 6);
  assert.equal(response.mutation_receipt.provider_revision, 5); assert.equal(f.calls.length, 3);
});

test("uncertain absence retains the original payload and prevents a changed revision or key", async () => {
  const f = fixture(), work = submit(f); const id = request(f.calls[0]).mutation_id;
  f.calls[0].reject(reject(503)); await tick(); f.calls[1].reject(reject(404));
  await assert.rejects(work, error => error.mutationId === id);
  assert.equal(f.events.includes("brassclaw-provider-settings-changed"), false);
  await assert.rejects(submit(f, { ...payload(), expected_revision: 5 }), /unresolved/);
  await assert.rejects(submit(f, { ...payload(), api_key: "changed-fixture-key" }), /unresolved/);
  assert.equal(f.calls.length, 2);
  const retry = submit(f); assert.equal(request(f.calls[2]).mutation_id, id);
  assert.deepEqual(request(f.calls[2]), request(f.calls[0]));
  f.calls[2].resolve({ mutation_receipt: receipt(id) }); await retry;
});

test("definitive precommit rejection and absent original allow an explicitly corrected new request", async () => {
  const f = fixture(), work = submit(f); const id = request(f.calls[0]).mutation_id;
  f.calls[0].reject(reject(409)); await tick(); f.calls[1].reject(reject(404));
  await assert.rejects(work, error => error.status === 409);
  assert.equal(f.pendingProviderMutations().length, 0);
  const corrected = submit(f, { ...payload(), expected_revision: 5 });
  const newId = request(f.calls[2]).mutation_id; assert.notEqual(newId, id);
  f.calls[2].resolve({ mutation_receipt: receipt(newId) }); await corrected;
});

test("an unavailable original cannot turn even a classified rejection into permission for a new payload", async () => {
  const f = fixture(), work = submit(f); const id = request(f.calls[0]).mutation_id;
  f.calls[0].reject(reject(409)); await tick(); f.calls[1].reject(reject(503));
  await assert.rejects(work, error => error.mutationId === id);
  await assert.rejects(submit(f, { ...payload(), name: "new name" }), /unresolved/);
});

test("a mismatching reference-only acknowledgement cannot settle the pending original", async () => {
  const f = fixture(), work = submit(f); const id = request(f.calls[0]).mutation_id;
  f.calls[0].reject(reject(503)); await tick(); f.calls[1].reject(reject(404)); await assert.rejects(work);
  const reconcile = f.reconcileProviderMutation(id);
  f.calls[2].resolve({ mutation_receipt: receipt("wrong-id") });
  await assert.rejects(reconcile, /differs/); assert.equal(f.pendingProviderMutations().length, 1);
  const read = f.readProviderMutation(id); f.calls[3].resolve(receipt("wrong-id")); await assert.rejects(read, /differs/);
});

test("independent roles have separate original identities", async () => {
  const f = fixture();
  const kohai = f.submitProviderMutation("select", "kohai", "/active", { provider_id: "local", role: "kohai" });
  const sempai = f.submitProviderMutation("select", "sempai", "/active", { provider_id: "local", role: "sempai" });
  const first = request(f.calls[0]).mutation_id, second = request(f.calls[1]).mutation_id;
  assert.notEqual(first, second);
  f.calls[0].resolve({ mutation_receipt: receipt(first, "select") });
  f.calls[1].resolve({ mutation_receipt: receipt(second, "select") }); await Promise.all([kohai, sempai]);
});

test("a delayed original reply cannot clear a newer pending change after reference reconciliation", async () => {
  const f = fixture(), first = submit(f); const id = request(f.calls[0]).mutation_id;
  const reconciled = f.reconcileProviderMutation(id);
  f.calls[1].resolve({ mutation_receipt: receipt(id) }); await reconciled;
  const next = submit(f, { ...payload(), expected_revision: 5 });
  const nextId = request(f.calls[2]).mutation_id; assert.notEqual(nextId, id);
  f.calls[0].resolve({ mutation_receipt: receipt(id) }); await first;
  assert.equal(f.pendingProviderMutations().length, 1);
  assert.equal(f.pendingProviderMutations()[0].id, nextId);
  f.calls[2].resolve({ mutation_receipt: receipt(nextId) }); await next;
});

test("confirmed publication with failed reload permits correction while identical retry only reconciles", async () => {
  const f = fixture(), work = submit(f); const id = request(f.calls[0]).mutation_id;
  f.calls[0].reject(reject(503)); await tick();
  f.calls[1].resolve(receipt(id)); await tick();
  assert.equal(f.pendingProviderMutations()[0].committed, true);
  assert.equal(f.events.filter(name => name === "brassclaw-provider-settings-changed").length, 1);
  f.calls[2].reject(reject(503));
  await assert.rejects(work, error => error.committedReceipt?.mutation_id === id);
  const retry = submit(f); assert.equal(f.calls[3].path, `/api/webchat/v2/llm/mutations/${id}/reconcile`);
  assert.equal(f.calls[3].options.body, undefined); f.calls[3].reject(reject(503)); await assert.rejects(retry);
  const correction = submit(f, { ...payload(), api_key: "corrected-fixture-key" });
  const nextId = request(f.calls[4]).mutation_id; assert.notEqual(nextId, id);
  f.calls[4].resolve({ mutation_receipt: receipt(nextId) }); await correction;
});

test("manual original read settles uncertainty without requiring a successful reload", async () => {
  const f = fixture(), work = submit(f); const id = request(f.calls[0]).mutation_id;
  f.calls[0].reject(reject(503)); await tick(); f.calls[1].reject(reject(404)); await assert.rejects(work);
  const read = f.readProviderMutation(id); f.calls[2].resolve(receipt(id)); await read;
  assert.equal(f.pendingProviderMutations()[0].committed, true);
  const correction = submit(f, { ...payload(), name: "Corrective definition" });
  const nextId = request(f.calls[3]).mutation_id; assert.notEqual(nextId, id);
  f.calls[3].resolve({ mutation_receipt: receipt(nextId) }); await correction;
});

test("inactive or aborted login observers send no read or reconciliation request", async () => {
  const f = fixture(), abort = new AbortController(); abort.abort();
  for (const operation of [f.readProviderMutation, f.reconcileProviderMutation]) {
    assert.equal(await operation("original", { isCurrent: () => false }), null);
    assert.equal(await operation("original", { signal: abort.signal }), null);
  }
  assert.equal(f.calls.length, 0); assert.deepEqual(f.events, []);
});

test("a stale or aborted response neither settles pending work nor publishes bootstrap/settings", async () => {
  for (const reconcile of [false, true]) for (const cancelled of [false, true]) {
    const f = fixture(), notices = [], work = submit(f), id = request(f.calls[0]).mutation_id;
    const originalEvents = [...f.events];
    f.subscribeProviderBootstrap(value => notices.push(value));
    let current = true; const abort = new AbortController();
    const operation = reconcile ? f.reconcileProviderMutation : f.readProviderMutation;
    const response = operation(id, { isCurrent: () => current, signal: abort.signal });
    assert.equal(f.calls[1].options.signal, abort.signal);
    if (cancelled) abort.abort(); else current = false;
    f.calls[1].resolve(reconcile ? { mutation_receipt: receipt(id) } : receipt(id));
    assert.equal(await response, null);
    assert.equal(f.pendingProviderMutations()[0].committed, false);
    assert.deepEqual(f.events, originalEvents); assert.deepEqual(notices, []);
    f.calls[0].resolve({ mutation_receipt: receipt(id) }); await work;
  }
});

test("the caller's exact receipt validator runs before any shared projection changes", async () => {
  for (const reconcile of [false, true]) {
    const f = fixture(), notices = [], work = submit(f), id = request(f.calls[0]).mutation_id;
    const originalEvents = [...f.events];
    f.subscribeProviderBootstrap(value => notices.push(value));
    const operation = reconcile ? f.reconcileProviderMutation : f.readProviderMutation;
    const response = operation(id, { validateReceipt: value => {
      if (value.provider_id !== "local") throw new Error("Wrong original provider");
    } });
    const foreign = { ...receipt(id), provider_id: "foreign" };
    f.calls[1].resolve(reconcile ? { mutation_receipt: foreign } : foreign);
    await assert.rejects(response, /Wrong original provider/);
    assert.equal(f.pendingProviderMutations()[0].committed, false);
    assert.deepEqual(f.events, originalEvents); assert.deepEqual(notices, []);
    f.calls[0].resolve({ mutation_receipt: receipt(id) }); await work;
  }
});


test("auth changes prevent success projection and all automatic recovery under a new context", async () => {
  for (const rejected of [false, true]) {
    const f = fixture(), notices = [], work = submit(f), id = request(f.calls[0]).mutation_id;
    f.subscribeProviderBootstrap(value => notices.push(value));
    const before = [...f.events]; f.changeAuth();
    if (rejected) f.calls[0].reject(reject(503)); else f.calls[0].resolve({ mutation_receipt: receipt(id) });
    await assert.rejects(work, error => error.mutationId === id && /Authentication changed/.test(error.message));
    assert.equal(f.calls.length, 1); assert.deepEqual(f.events, before); assert.deepEqual(notices, []);
    assert.equal(f.pendingProviderMutations().length, 0);
    // A new explicit operator observation is allowed; automatic submit
    // recovery above sent nothing after the context change.
    const read = f.readProviderMutation(id);
    assert.equal(f.calls[1].options.body, undefined);
    f.calls[1].resolve(receipt(id)); await read;
    const reconcile = f.reconcileProviderMutation(id);
    assert.equal(f.calls[2].options.body, undefined);
    f.calls[2].resolve({ mutation_receipt: receipt(id) }); await reconcile;
    assert.equal(f.calls.length, 3);
  }
});

test("an explicit successor context gets its own bytes/ID instead of the old unresolved payload", async () => {
  const f = fixture(), first = submit(f), oldId = request(f.calls[0]).mutation_id;
  f.changeAuth();
  const next = submit(f, { ...payload(), api_key: "new-context-private-key" });
  assert.equal(f.calls.length, 2); const nextId = request(f.calls[1]).mutation_id;
  assert.notEqual(nextId, oldId); assert.equal(request(f.calls[1]).api_key, "new-context-private-key");
  f.calls[0].resolve({ mutation_receipt: receipt(oldId) }); await assert.rejects(first, /Authentication changed/);
  assert.equal(f.pendingProviderMutations()[0].id, nextId);
  f.calls[1].resolve({ mutation_receipt: receipt(nextId) }); await next;
});

test("auth changes during original read or reconcile stop the submit recovery chain", async () => {
  for (const duringReconcile of [false, true]) {
    const f = fixture(), work = submit(f), id = request(f.calls[0]).mutation_id;
    f.calls[0].reject(reject(503)); await tick();
    if (duringReconcile) { f.calls[1].resolve(receipt(id)); await tick(); }
    const count = f.calls.length; f.changeAuth();
    f.calls[count - 1].resolve(duringReconcile ? { mutation_receipt: receipt(id) } : receipt(id));
    await assert.rejects(work, /Authentication changed/); assert.equal(f.calls.length, count);
  }
});

test("missing authentication fails before retaining/sending a new secret payload", async () => {
  const f = fixture(); f.logout(); await assert.rejects(submit(f), /Authentication required/);
  assert.equal(f.calls.length, 0); assert.equal(f.pendingProviderMutations().length, 0);
});

test("a retired POST body cannot be reused even if the original auth observation recovers", async () => {
  const f = fixture(), work = submit(f), id = request(f.calls[0]).mutation_id;
  f.logout(); f.calls[0].reject(reject(503)); await assert.rejects(work, /Authentication changed/);
  f.restoreAuth();
  const retry = submit(f); assert.equal(f.calls.length, 1);
  await assert.rejects(retry, /unresolved/);
  const read = f.readProviderMutation(id);
  assert.equal(f.calls[1].options.body, undefined);
  f.calls[1].resolve(receipt(id)); await read;
  const reconcile = f.reconcileProviderMutation(id);
  assert.equal(f.calls[2].options.body, undefined);
  f.calls[2].resolve({ mutation_receipt: receipt(id) }); await reconcile;
  assert.equal(f.pendingProviderMutations().length, 0);
  assert.equal(f.calls.filter(call => call.path === "/providers").length, 1);
});
