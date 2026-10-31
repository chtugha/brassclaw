import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

const source = readFileSync(new URL("./oauth-original.js", import.meta.url), "utf8")
  .replace(/^import .*;$/gm, "").replace(/^export /gm, "");
const sandbox = vm.createContext({ clientActionId: randomUUID, URL });
vm.runInContext(`${source}\nglobalThis.api={findOauthOriginal,retainOauthOriginal,recordOauthOriginal,renewOauthOriginal,oauthOriginalBody};`, sandbox);
const { findOauthOriginal: find, retainOauthOriginal: rawRetain, recordOauthOriginal: record,
  renewOauthOriginal: rawRenew, oauthOriginalBody: body } = sandbox.api;
// Every new setup in this fixture explicitly chooses New unless overridden.
const retain = (context, options = {}) => rawRetain(context, { target: { kind: "new" }, ...options });
const renew = (operation, context, options = {}) => rawRenew(operation, context, { target: { kind: "new" }, ...options });
const copy = value => JSON.parse(JSON.stringify(value));
const context = { packageId: "notion", requirement: "oauth-account", provider: "notion",
  accountLabel: "notion account", scopes: ["read"], invocationId: randomUUID() };
const expiresAt = "2099-01-01T00:00:00.000Z";
function storage() {
  const values = new Map();
  return { values, getItem: key => values.get(key) ?? null, setItem: (key, value) => values.set(key, value) };
}
function ack(operation, overrides = {}) {
  return { flow_id: operation.flowId, provider: context.provider, account_label: context.accountLabel,
    scopes: [...context.scopes], callback_scope: { invocation_id: context.invocationId },
    expires_at: expiresAt, continuation: { type: "setup_only" }, status: "awaiting_user", terminal: false,
    expired: false, error: null, cleanup: null, authorization_url: "https://issuer.example/authorize?state=RAW_STATE_NEVER_STORE", ...overrides };
}

test("cleanup stays redacted and cannot claim local completion without remote proof", () => {
  const saved=storage(), original=retain(context,{storage:saved,expiresAt});
  const cleanup={remote:"unconfirmed",local:"retained",issue:null,http_status:null,error:"backend_unavailable",can_reconcile:true};
  const closed=record(original,context,ack(original,{status:"canceled",terminal:true,authorization_url:null,cleanup}),saved);
  assert.equal(closed.receipt.status,"canceled");
  assert.equal(JSON.stringify([...saved.values]).includes("cleanup"),false,"projection does not become another persisted cleanup owner");
  for (const wrong of [{...cleanup,raw_token:"PRIVATE"},{...cleanup,local:"confirmed",can_reconcile:false},
    {...cleanup,error:"RAW_PROVIDER_REPLY"},{...cleanup,remote:"confirmed",http_status:200,error:null},
    {...cleanup,issue:"management_unavailable"}, {...cleanup,error:null}, {...cleanup,http_status:401}]) {
    assert.throws(()=>record(closed,context,ack(original,{status:"canceled",terminal:true,authorization_url:null,cleanup:wrong}),saved));
  }
  record(closed,context,ack(original,{status:"canceled",terminal:true,authorization_url:null,
    cleanup:{remote:"confirmed",local:"confirmed",issue:null,http_status:204,error:null,can_reconcile:false}}),saved);
});

test("reload and retries keep the exact independent original and first expiry", () => {
  const saved = storage(); let identities = 0;
  const original = retain(context, { storage: saved, expiresAt, actionId: () => { identities++; return randomUUID(); } });
  const first = record(original, context, ack(original), saved);
  assert.deepEqual(copy(retain(context, { storage: saved, expiresAt: "2099-02-01T00:00:00Z" })), copy(first));
  assert.deepEqual(copy(find(context, saved)), copy(first));
  assert.equal(identities, 1);
  assert.equal(JSON.stringify([...saved.values]).includes("RAW_STATE_NEVER_STORE"), false);
  assert.equal(JSON.stringify([...saved.values]).includes("authorization_url"), false);
});

test("a missing original is not manufactured by a read or failed persistence", () => {
  const saved = storage();
  assert.equal(find(context, saved), null);
  assert.throws(() => retain(context, { storage: saved, create: false }));
  assert.throws(() => retain(context, { expiresAt, storage: { getItem: () => null, setItem() {} } }));
  const throwing = { getItem: () => null, setItem() { throw new Error("storage unavailable"); } };
  assert.throws(() => retain(context, { storage: throwing, expiresAt }));
  assert.equal(saved.values.size, 0);
});

test("changed definition, identity, invocation or expiry cannot replace original metadata", () => {
  const saved = storage(), original = retain(context, { storage: saved, expiresAt });
  for (const change of [{ flow_id: randomUUID() }, { provider: "other" }, { account_label: "other" },
    { scopes: ["write"] }, { callback_scope: { invocation_id: randomUUID() } },
    { expires_at: "2099-01-02T00:00:00Z" }, { raw_token: "PRIVATE_TOKEN" }]) {
    assert.throws(() => record(original, context, ack(original, change), saved));
    assert.deepEqual(copy(find(context, saved)), copy(original));
  }
  assert.throws(() => retain({ ...context, scopes: ["write"] }, { storage: saved, expiresAt }));
  assert.throws(() => retain({ ...context, provider: "changed" }, { storage: saved, expiresAt }));
});

test("a pending or merely time-expired setup cannot authorize a new original", () => {
  const saved = storage(), original = retain(context, { storage: saved, expiresAt });
  const pending = record(original, context, ack(original, { status: "pending", authorization_url: null,
    error: { code: "backend_unavailable", retryable: true } }), saved);
  assert.equal(pending.receipt.hasError, true);
  assert.throws(() => renew(pending, context, { storage: saved, expiresAt }));
  const expired = record(original, context, ack(original, { expired: true, authorization_url: null }), saved);
  assert.equal(expired.receipt.terminal, false);
  assert.throws(() => renew(expired, context, { storage: saved, expiresAt }));
});

test("only explicit renewal after a confirmed terminal receipt changes the original ID", () => {
  const saved = storage(), original = retain(context, { storage: saved, expiresAt });
  const closed = record(original, context, ack(original, { status: "canceled", terminal: true,
    authorization_url: null }), saved);
  assert.deepEqual(copy(retain(context, { storage: saved, expiresAt })), copy(closed));
  assert.throws(() => renew(closed, context, { storage: saved, expiresAt, actionId: () => original.flowId }));
  const next = renew(closed, context, { storage: saved, expiresAt: "2099-01-02T00:00:00Z" });
  assert.notEqual(next.flowId, original.flowId);
  assert.equal(next.invocationId, original.invocationId);
  assert.equal(next.receipt, null);
  assert.throws(() => record(original, context, ack(original), saved));
});

test("late acknowledgement cannot regress callback progress or change a terminal outcome", () => {
  const saved = storage(), original = retain(context, { storage: saved, expiresAt });
  const completing = record(original, context, ack(original, { status: "completing", authorization_url: null }), saved);
  assert.deepEqual(copy(record(original, context, ack(original), saved)), copy(completing));
  const completed = record(original, context, ack(original, { status: "completed", terminal: true, authorization_url: null }), saved);
  assert.throws(() => record(original, context, ack(original, { status: "canceled", terminal: true, authorization_url: null }), saved));
  assert.deepEqual(copy(find(context, saved)), copy(completed));
});

test("unexpected secrets, dangerous URLs and contradictory readiness are rejected before storage", () => {
  const saved = storage(), original = retain(context, { storage: saved, expiresAt });
  for (const change of [{ authorization_url: "javascript:alert(1)" }, { authorization_url: null },
    { terminal: true }, { expired: true },
    { error: { code: "backend_unavailable", retryable: true, raw_token: "PRIVATE_TOKEN" }, authorization_url: null }]) {
    assert.throws(() => record(original, context, ack(original, change), saved));
  }
  const poisoned = copy(original); poisoned.raw_secret = "PRIVATE_TOKEN";
  saved.setItem(original.key, JSON.stringify(poisoned));
  assert.throws(() => find(context, saved));
  assert.throws(() => retain(context, { storage: saved, expiresAt }));
  assert.equal(JSON.parse(saved.getItem(original.key)).raw_secret, "PRIVATE_TOKEN", "corrupt original was not silently overwritten");
});

test("wire actions keep one original and never include URLs, state or credential material", () => {
  const original = retain(context, { storage: storage(), expiresAt });
  for (const action of ["start", "recover", "cancel"]) {
    const payload = copy(body(original, action));
    assert.deepEqual(Object.keys(payload), ["action", "flow_id", "provider", "requirement", "target", "account_label", "scopes", "invocation_id", "expires_at"]);
    assert.equal(payload.flow_id, original.flowId);
    assert.equal(payload.invocation_id, context.invocationId);
    assert.equal(payload.expires_at, expiresAt);
  }
  assert.throws(() => body(original, "automatic_replay"));
});


test("new originals retain an explicit target while legacy originals never gain a binding", () => {
  assert.throws(() => rawRetain(context, { storage: storage(), expiresAt }));
  const saved = storage(), accountId = randomUUID();
  const original = retain(context, { storage: saved, expiresAt, target: { kind: "existing", account_id: accountId } });
  assert.equal(original.version, 2);
  assert.deepEqual(copy(body(original, "start").target), { kind: "existing", account_id: accountId });
  assert.equal(body(original, "recover").requirement, context.requirement);
  // A newly displayed account cannot reinterpret the already retained choice.
  const again = retain(context, { storage: saved, expiresAt, target: { kind: "existing", account_id: randomUUID() } });
  assert.equal(again.target.account_id, accountId);
  const legacy = copy(original); legacy.version = 1; delete legacy.target;
  const bytes = JSON.stringify(legacy); saved.setItem(legacy.key, bytes);
  assert.deepEqual(copy(find(context, saved)), legacy);
  assert.equal(saved.getItem(legacy.key), bytes);
  const oldWire = body(find(context, saved), "recover");
  assert.equal(Object.hasOwn(oldWire, "requirement"), false);
  assert.equal(Object.hasOwn(oldWire, "target"), false);
  for (const target of [null, {}, { kind: "new", account_id: accountId }, { kind: "existing" }, { kind: "existing", account_id: "wrong" }]) {
    assert.throws(() => rawRetain(context, { storage: storage(), expiresAt, target }));
  }
});
