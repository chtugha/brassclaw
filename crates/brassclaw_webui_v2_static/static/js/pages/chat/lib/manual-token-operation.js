// Redacted correlation only. Raw credentials and fingerprints never enter storage.
import { clientActionId } from "../../../lib/api.js";

const VERSION = 1;
const PREFIX = "brassclaw:manual-token-original:";
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const STATUSES = new Set(["awaiting_user", "completed", "canceled", "expired", "failed"]);
function invalid() {
  return new Error("Manual credential operation could not be verified.");
}
function exactKeys(value, names) {
  return value && typeof value === "object" && !Array.isArray(value) &&
    Object.keys(value).length === names.length && names.every(name => Object.hasOwn(value, name));
}
function identityEqual(a, b) {
  return a?.interaction_id === b?.interaction_id && a?.flow_id === b?.flow_id;
}
function validIdentity(value) {
  return exactKeys(value, ["interaction_id", "flow_id"]) &&
    UUID.test(value.interaction_id) && UUID.test(value.flow_id);
}
export function manualTokenOperationKey({ threadId, runId, gateRef, provider, accountLabel }) {
  const parts = [threadId, runId, gateRef];
  if (![...parts, provider, accountLabel].every(part => typeof part === "string" && part.length > 0)) throw invalid();
  return PREFIX + JSON.stringify(parts);
}
function validTarget(value) {
  return (exactKeys(value, ["kind"]) && value.kind === "new") ||
    (exactKeys(value, ["kind", "account_id"]) && value.kind === "existing" && UUID.test(value.account_id));
}
function validDefinition(value) {
  if (exactKeys(value, ["provider", "accountLabel"])) return true;
  return exactKeys(value, ["kind", "packageId", "requirement", "provider", "accountLabel", "target"]) &&
    value.kind === "extension" && [value.packageId, value.requirement].every(part => typeof part === "string" && part.length > 0) &&
    validTarget(value.target);
}
function read(key, storage) {
  const text = storage.getItem(key);
  if (text === null) return null;
  let operation;
  try { operation = JSON.parse(text); } catch { throw invalid(); }
  if (!exactKeys(operation, ["version", "key", "definition", "identity", "invocationId", "setup", "closed"]) ||
      operation.version !== VERSION || operation.key !== key ||
      !validDefinition(operation.definition) ||
      ![operation.definition.provider, operation.definition.accountLabel].every(value => typeof value === "string" && value.length > 0) ||
      !validIdentity(operation.identity) || !UUID.test(operation.invocationId) ||
      typeof operation.closed !== "boolean" ||
      (operation.setup !== null &&
        (!exactKeys(operation.setup, ["expiresAt", "status"]) ||
         typeof operation.setup.expiresAt !== "string" || !Number.isFinite(Date.parse(operation.setup.expiresAt)) ||
         !STATUSES.has(operation.setup.status)))) throw invalid();
  return operation;
}
function write(operation, storage) {
  const bytes = JSON.stringify(operation);
  storage.setItem(operation.key, bytes);
  // No transport may start if the browser cannot retain the exact original.
  if (storage.getItem(operation.key) !== bytes) throw invalid();
  return operation;
}
function current(operation, storage) {
  const stored = read(operation.key, storage);
  if (!stored || !identityEqual(stored.identity, operation.identity) ||
      stored.invocationId !== operation.invocationId || stored.closed) throw invalid();
  return stored;
}
export function retainManualTokenOperation(context, {
  storage = globalThis.sessionStorage, actionId = clientActionId, create = true,
} = {}) {
  const key = manualTokenOperationKey(context);
  const existing = read(key, storage);
  if (existing) {
    if (existing.closed || existing.definition.provider !== context.provider ||
        existing.definition.accountLabel !== context.accountLabel) throw invalid();
    return existing;
  }
  if (!create) throw invalid();
  const operation = {
    version: VERSION, key, definition: { provider: context.provider, accountLabel: context.accountLabel },
    identity: { interaction_id: actionId(), flow_id: actionId() },
    invocationId: actionId(), setup: null, closed: false,
  };
  if (!validIdentity(operation.identity) || !UUID.test(operation.invocationId)) throw invalid();
  return write(operation, storage);
}
export function recordManualTokenSetup(operation, context, receipt, storage = globalThis.sessionStorage) {
  const retained = current(operation, storage);
  if (manualTokenOperationKey(context) !== retained.key ||
      retained.definition.provider !== context.provider || retained.definition.accountLabel !== context.accountLabel ||
      !identityEqual(receipt?.identity, retained.identity) ||
      receipt?.invocation_id !== retained.invocationId ||
      receipt?.interaction_id !== retained.identity.interaction_id ||
      receipt?.provider !== context.provider || receipt?.label !== context.accountLabel ||
      !STATUSES.has(receipt?.status) || typeof receipt?.expires_at !== "string" ||
      !Number.isFinite(Date.parse(receipt.expires_at)) ||
      (retained.setup && retained.setup.expiresAt !== receipt.expires_at)) throw invalid();
  return write({ ...retained, setup: { expiresAt: receipt.expires_at, status: receipt.status } }, storage);
}
export function validateManualTokenCompletion(operation, context, receipt, storage = globalThis.sessionStorage) {
  const retained = current(operation, storage);
  if (manualTokenOperationKey(context) !== retained.key ||
      retained.definition.provider !== context.provider || retained.definition.accountLabel !== context.accountLabel || !retained.setup ||
      !identityEqual(receipt?.identity, retained.identity) ||
      receipt?.invocation_id !== retained.invocationId || receipt?.status !== "configured" ||
      typeof receipt?.credential_ref !== "string" || !UUID.test(receipt.credential_ref) ||
      receipt?.continuation?.type !== "turn_gate_resume" ||
      receipt.continuation.turn_run_ref !== context.runId || receipt.continuation.gate_ref !== context.gateRef) throw invalid();
  return receipt;
}
export function closeManualTokenOperation(operation, storage = globalThis.sessionStorage) {
  const stored = read(operation.key, storage);
  if (!stored || !identityEqual(stored.identity, operation.identity) ||
      stored.invocationId !== operation.invocationId) throw invalid();
  return write({ ...stored, closed: true }, storage);
}

// Extension setup uses the same redacted persistence/identity algorithm, with
// its actual canonical Web invocation and SetupOnly continuation (no fake gate).
function extensionKey(context) {
  const parts = [context.packageId, context.requirement, context.invocationId];
  if (![...parts, context.provider, context.accountLabel].every(value => typeof value === "string" && value.length > 0) ||
      !UUID.test(context.invocationId)) throw invalid();
  return "brassclaw:extension-manual-token-original:" + JSON.stringify(parts);
}
function extensionCurrent(operation, context, storage, allowClosed = false) {
  const saved = read(extensionKey(context), storage);
  if (!saved || !identityEqual(saved.identity, operation.identity) ||
      saved.invocationId !== context.invocationId || (!allowClosed && saved.closed) ||
      saved.definition.kind !== "extension" || saved.definition.packageId !== context.packageId ||
      saved.definition.requirement !== context.requirement || saved.definition.provider !== context.provider ||
      saved.definition.accountLabel !== context.accountLabel) throw invalid();
  return saved;
}
export function retainExtensionManualTokenOperation(context, {
  target, storage = globalThis.sessionStorage, actionId = clientActionId, create = true, startNew = false,
} = {}) {
  const key = extensionKey(context), saved = read(key, storage);
  if (saved) {
    extensionCurrent(saved, context, storage, true);
    if (!startNew) return saved;
    // Only an explicit operator action replaces a terminal original pointer.
    if (!saved.closed || !saved.setup || !["completed", "canceled", "expired", "failed"].includes(saved.setup.status)) throw invalid();
  } else if (startNew || !create) throw invalid();
  if (!validTarget(target)) throw invalid();
  const operation = { version: VERSION, key,
    definition: { kind: "extension", packageId: context.packageId, requirement: context.requirement,
      provider: context.provider, accountLabel: context.accountLabel, target: { ...target } },
    identity: { interaction_id: actionId(), flow_id: actionId() },
    invocationId: context.invocationId, setup: null, closed: false };
  if (!validIdentity(operation.identity)) throw invalid();
  return write(operation, storage);
}
export function recordExtensionManualTokenSetup(operation, context, receipt, storage = globalThis.sessionStorage) {
  const saved = extensionCurrent(operation, context, storage);
  if (!identityEqual(receipt?.identity, saved.identity) || receipt?.invocation_id !== saved.invocationId ||
      receipt?.provider !== context.provider || receipt?.label !== context.accountLabel ||
      !STATUSES.has(receipt?.status) || typeof receipt?.expires_at !== "string" ||
      !Number.isFinite(Date.parse(receipt.expires_at)) ||
      (saved.setup && saved.setup.expiresAt !== receipt.expires_at)) throw invalid();
  const terminal = ["canceled", "expired", "failed"].includes(receipt.status);
  return write({ ...saved, setup: { expiresAt: receipt.expires_at, status: receipt.status }, closed: terminal }, storage);
}
export function completeExtensionManualTokenOperation(operation, context, receipt, storage = globalThis.sessionStorage) {
  const saved = extensionCurrent(operation, context, storage);
  if (!saved.setup || !identityEqual(receipt?.identity, saved.identity) || receipt?.invocation_id !== saved.invocationId ||
      receipt?.status !== "configured" || typeof receipt?.credential_ref !== "string" || !UUID.test(receipt.credential_ref) ||
      !exactKeys(receipt?.continuation, ["type"]) || receipt.continuation.type !== "setup_only") throw invalid();
  return write({ ...saved, setup: { ...saved.setup, status: "completed" }, closed: true }, storage);
}
