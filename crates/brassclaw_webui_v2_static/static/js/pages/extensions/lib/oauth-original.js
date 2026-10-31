// Redacted original correlation only; no OAuth URL/state, token or raw reply is stored.
import { clientActionId } from "../../../lib/api.js";

const PREFIX = "brassclaw:extension-oauth-original:";
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const TERMINAL = new Set(["completed", "failed", "expired", "canceled"]);
const RANK = { pending: 0, awaiting_user: 1, callback_received: 2, completing: 3,
  completed: 4, failed: 4, expired: 4, canceled: 4 };
function invalid() { return new Error("OAuth original could not be verified."); }
function exact(value, keys) {
  return value && typeof value === "object" && !Array.isArray(value) &&
    Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));
}
function nonempty(value) { return typeof value === "string" && value.length > 0; }
export function checkedOauthCleanup(value) {
  if (value === null) return null;
  const remote = ["retained", "pending", "not_registered", "unavailable", "unconfirmed", "confirmed"];
  const local = ["retained", "pending", "unresolved", "confirmed"];
  const issues = ["registration_unconfirmed", "registration_response_unusable", "management_unavailable", "invalid_management_endpoint"];
  const errors = ["unknown_or_expired_flow", "cross_scope_denied", "provider_denied", "token_exchange_failed",
    "refresh_failed", "credential_missing", "account_selection_required", "backend_unavailable", "malformed_config",
    "malformed_callback", "canceled", "flow_already_terminal", "invalid_request"];
  if (!exact(value, ["remote", "local", "issue", "http_status", "error", "can_reconcile"]) ||
      !remote.includes(value.remote) || !local.includes(value.local) || typeof value.can_reconcile !== "boolean" ||
      (value.issue !== null && !issues.includes(value.issue)) ||
      (value.error !== null && !errors.includes(value.error)) ||
      (value.http_status !== null && (!Number.isInteger(value.http_status) || value.http_status < 200 || value.http_status > 599)) ||
      (value.remote === "unavailable") !== (value.issue !== null) ||
      (value.remote === "confirmed" && (value.http_status !== 204 || value.error !== null)) ||
      (value.remote === "unconfirmed" && value.http_status === null && value.error === null) ||
      (value.http_status !== null && value.error !== null) ||
      (!["confirmed", "unconfirmed"].includes(value.remote) && (value.http_status !== null || value.error !== null)) ||
      (value.local !== "retained" && !["confirmed", "not_registered"].includes(value.remote)) ||
      (value.local === "confirmed" && value.can_reconcile)) throw invalid();
  return { ...value };
}
function checkedTarget(target) {
  if (exact(target, ["kind"]) && target.kind === "new") return { kind: "new" };
  if (exact(target, ["kind", "account_id"]) && target.kind === "existing" && UUID.test(target.account_id)) {
    return { kind: "existing", account_id: target.account_id };
  }
  throw invalid();
}
function definition(context) {
  const value = { packageId: context.packageId, requirement: context.requirement,
    provider: context.provider, accountLabel: context.accountLabel, scopes: context.scopes };
  if (!Object.values(value).slice(0, 4).every(nonempty) ||
      !Array.isArray(value.scopes) || !value.scopes.every(nonempty) || !UUID.test(context.invocationId)) throw invalid();
  return { ...value, scopes: [...value.scopes] };
}
export function oauthOriginalKey(context) {
  const checked = definition(context);
  return PREFIX + JSON.stringify([checked.packageId, checked.requirement, context.invocationId]);
}
function read(context, storage) {
  const key = oauthOriginalKey(context), text = storage.getItem(key);
  if (text === null) return null;
  let value;
  try { value = JSON.parse(text); } catch { throw invalid(); }
  const keys = ["version", "key", "definition", "flowId", "invocationId", "expiresAt", "receipt"];
  if (value?.version === 2) keys.push("target");
  if (!exact(value, keys) || ![1, 2].includes(value.version) || value.key !== key || !UUID.test(value.flowId) || value.invocationId !== context.invocationId ||
      !exact(value.definition, ["packageId", "requirement", "provider", "accountLabel", "scopes"]) ||
      JSON.stringify(value.definition) !== JSON.stringify(definition(context)) ||
      typeof value.expiresAt !== "string" || !Number.isFinite(Date.parse(value.expiresAt)) ||
      (value.receipt !== null && (!exact(value.receipt, ["status", "terminal", "expired", "hasError"]) ||
        !Object.hasOwn(RANK, value.receipt.status) || typeof value.receipt.terminal !== "boolean" ||
        value.receipt.terminal !== TERMINAL.has(value.receipt.status) ||
        typeof value.receipt.expired !== "boolean" || typeof value.receipt.hasError !== "boolean" ||
        (value.receipt.terminal && value.receipt.expired)))) throw invalid();
  if (value.version === 2) checkedTarget(value.target);
  return value;
}
function write(value, storage) {
  const bytes = JSON.stringify(value);
  storage.setItem(value.key, bytes);
  if (storage.getItem(value.key) !== bytes) throw invalid();
  return value;
}
export function findOauthOriginal(context, storage = globalThis.sessionStorage) {
  return read(context, storage);
}
export function retainOauthOriginal(context, {
  storage = globalThis.sessionStorage, actionId = clientActionId, create = true, expiresAt, target,
} = {}) {
  const value = read(context, storage);
  if (value) return value;
  if (!create || typeof expiresAt !== "string" || !Number.isFinite(Date.parse(expiresAt))) throw invalid();
  const original = { version: 2, key: oauthOriginalKey(context), definition: definition(context),
    flowId: actionId(), invocationId: context.invocationId, expiresAt, receipt: null, target: checkedTarget(target) };
  if (!UUID.test(original.flowId)) throw invalid();
  return write(original, storage);
}
function current(operation, context, storage) {
  const value = read(context, storage);
  if (!value || value.flowId !== operation.flowId || value.expiresAt !== operation.expiresAt) throw invalid();
  return value;
}
export function recordOauthOriginal(operation, context, reply, storage = globalThis.sessionStorage) {
  const value = current(operation, context, storage);
  if (!exact(reply, ["flow_id", "status", "provider", "account_label", "scopes", "authorization_url",
    "expires_at", "continuation", "callback_scope", "error", "terminal", "expired", "cleanup"]) ||
      reply.flow_id !== value.flowId || reply.provider !== value.definition.provider ||
      reply.account_label !== value.definition.accountLabel || JSON.stringify(reply.scopes) !== JSON.stringify(value.definition.scopes) ||
      reply.callback_scope?.invocation_id !== value.invocationId || reply.continuation?.type !== "setup_only" ||
      typeof reply.expires_at !== "string" || Date.parse(reply.expires_at) !== Date.parse(value.expiresAt) ||
      !Object.hasOwn(RANK, reply.status) || typeof reply.terminal !== "boolean" ||
      reply.terminal !== TERMINAL.has(reply.status) || typeof reply.expired !== "boolean" ||
      (reply.terminal && reply.expired) ||
      (reply.error !== null && (!reply.error || typeof reply.error.code !== "string" ||
        typeof reply.error.retryable !== "boolean" ||
        !Object.keys(reply.error).every(key => ["code", "retryable", "conflict"].includes(key)) ||
        (Object.hasOwn(reply.error, "conflict") && typeof reply.error.conflict !== "boolean")))) throw invalid();
  checkedOauthCleanup(reply.cleanup);
  if (reply.authorization_url !== null) {
    let url;
    try { url = new URL(reply.authorization_url); } catch { throw invalid(); }
    if (typeof reply.authorization_url !== "string" || url.protocol !== "https:" ||
        reply.status !== "awaiting_user" || reply.terminal || reply.expired || reply.error !== null) throw invalid();
  }
  if (reply.status === "awaiting_user" && !reply.expired && reply.error === null && reply.authorization_url === null) throw invalid();
  if (value.receipt?.terminal && value.receipt.status !== reply.status) throw invalid();
  if (value.receipt && RANK[value.receipt.status] > RANK[reply.status]) return value;
  return write({ ...value, receipt: { status: reply.status, terminal: reply.terminal,
    expired: reply.expired, hasError: reply.error !== null } }, storage);
}
export function renewOauthOriginal(operation, context, {
  storage = globalThis.sessionStorage, actionId = clientActionId, expiresAt, target,
} = {}) {
  const value = current(operation, context, storage);
  if (!value.receipt?.terminal || typeof expiresAt !== "string" || !Number.isFinite(Date.parse(expiresAt))) throw invalid();
  const successor = { ...value, version: 2, flowId: actionId(), expiresAt, receipt: null, target: checkedTarget(target) };
  if (!UUID.test(successor.flowId) || successor.flowId === value.flowId) throw invalid();
  return write(successor, storage);
}
export function oauthOriginalBody(operation, action) {
  if (!["start", "recover", "cancel"].includes(action)) throw invalid();
  return { action, flow_id: operation.flowId, provider: operation.definition.provider,
    ...(operation.version === 2 ? { requirement: operation.definition.requirement, target: checkedTarget(operation.target) } : {}),
    account_label: operation.definition.accountLabel, scopes: [...operation.definition.scopes],
    invocation_id: operation.invocationId, expires_at: operation.expiresAt };
}
