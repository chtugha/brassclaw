import { readProviderMutation, reconcileProviderMutation } from "./provider-mutations.js";
import { clientActionId, readStoredToken, originalAuthContextIsCurrent } from "../../../lib/api.js";
import { readOriginalJson, writeOriginalJson } from "../../../lib/original-custody.js";
import { startCodexLogin, codexLoginStatus } from "./settings-api.js";
import { modelSelectionRef } from "./llm-providers.js";

// Read and reconcile only this original login. Provider-name polling can report
// success for an older login and must never confirm the current request.
export async function waitForProviderLogin(id, providerId, deadlineMs, intervalMs, isCurrent = () => true) {
  if (typeof id !== "string" || !id) throw new Error("Original login identity is unavailable.");
  const deadline = Date.now() + deadlineMs;
  const validateReceipt = receipt => {
    if (receipt?.mutation_id !== id || receipt.operation !== "upsert" || receipt.provider_id !== providerId) throw new Error("Original login acknowledgement differs.");
  };
  while (isCurrent() && Date.now() < deadline) {
    let originalKnown = false;
    try {
      const receipt = await readProviderMutation(id, { isCurrent, validateReceipt });
      if (!isCurrent()) return false;
      if (receipt?.mutation_id !== id || receipt.operation !== "upsert" || receipt.provider_id !== providerId) throw new Error("Original login acknowledgement differs.");
      originalKnown = true;
      const snapshot = await reconcileProviderMutation(id, { isCurrent, validateReceipt });
      if (!isCurrent()) return false;
      if (snapshot?.mutation_receipt?.mutation_id !== id) throw new Error("Original login reconciliation differs.");
      return true;
    } catch (error) {
      // An absent original before authorization is pending. Transport and
      // reconciliation failures remain errors, never success or another write.
      if (originalKnown || error?.status !== 404) throw error;
    }
    await new Promise(resolve => setTimeout(resolve, intervalMs));
  }
  return false;
}

const CODEX_UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const CODEX_TERMINAL = new Set(["completed", "failed", "expired", "canceled"]);
const CODEX_STATES = new Set(["pending", "awaiting_user", "callback_received", "completing", "completed", "failed", "expired", "canceled"]);
const CODEX_ERRORS = new Set(["unknown_or_expired_flow", "cross_scope_denied", "provider_denied",
  "token_exchange_failed", "refresh_failed", "credential_missing", "account_selection_required",
  "backend_unavailable", "malformed_config", "malformed_callback", "canceled", "flow_already_terminal", "invalid_request"]);
const codexInvalid = () => new Error("Original Codex login could not be verified.");
const codexObject = (value, fields) => value && typeof value === "object" && !Array.isArray(value) &&
  Object.keys(value).length === fields.length && fields.every(key => Object.hasOwn(value, key));
const codexText = value => typeof value === "string" && value.length > 0 && value.trim() === value && !/[\x00-\x1f\x7f]/.test(value);
const codexId = value => typeof value === "string" && CODEX_UUID.test(value) && value !== "00000000-0000-0000-0000-000000000000";
// This is the existing acknowledged Monty polling field's signed browser-timer
// range, not a new polling cap. There is no 2000ms fallback or local deadline.
const codexInterval = value => Number.isInteger(value) && value > 0 && value <= 2147483647;

export function codexLoginSelection(provider, role = "kohai", model = provider?.default_model) {
  if (!provider || !["openai_codex", "open_ai_codex"].includes(provider.adapter) ||
      !["kohai", "sempai"].includes(role) || !codexText(provider.id) || !codexText(model) ||
      !Number.isSafeInteger(provider.revision) || provider.revision <= 0) throw codexInvalid();
  return { provider_id: provider.id, expected_revision: provider.revision,
    expected_selection_ref: modelSelectionRef(provider.selection_refs, role), model, role };
}
function codexRequest(value) {
  if (!codexObject(value, ["provider_id", "expected_revision", "expected_selection_ref", "model", "role", "mutation_id"]) ||
      !codexId(value.mutation_id) || !codexText(value.provider_id) || !codexText(value.model) ||
      !["kohai", "sempai"].includes(value.role) || !Number.isSafeInteger(value.expected_revision) || value.expected_revision <= 0 ||
      typeof value.expected_selection_ref !== "string" || !/^[0-9a-f]{64}$/.test(value.expected_selection_ref)) throw codexInvalid();
  return value;
}
function codexContext(context, selection) {
  if (!codexObject(context, ["origin", "nonce"]) || !codexText(context.origin) || !codexId(context.nonce) ||
      !codexText(selection.provider_id) || !["kohai", "sempai"].includes(selection.role)) throw codexInvalid();
  return { ...context, provider_id: selection.provider_id, role: selection.role };
}
function codexKey(context) {
  return "brassclaw:codex-login-original:" + JSON.stringify([context.origin, context.nonce, context.provider_id, context.role]);
}
export function findCodexLoginOriginal(context, selection, storage = globalThis.sessionStorage) {
  const expected = codexContext(context, selection), key = codexKey(expected);
  const stored = readOriginalJson(storage, key, codexInvalid);
  if (!stored.present) return null;
  const value = stored.value;
  if (!codexObject(value, ["version", "context", "bytes", "observation"]) || value.version !== 1 ||
      !codexObject(value.context, Object.keys(expected)) || Object.keys(expected).some(k => value.context[k] !== expected[k]) ||
      typeof value.bytes !== "string") throw codexInvalid();
  let request;
  try { request = codexRequest(JSON.parse(value.bytes)); } catch { throw codexInvalid(); }
  if (JSON.stringify(request) !== value.bytes || request.provider_id !== expected.provider_id || request.role !== expected.role) throw codexInvalid();
  if (value.observation !== null && (!codexObject(value.observation, ["status", "expires_at"]) ||
      (value.observation.status !== null && !CODEX_STATES.has(value.observation.status)) ||
      typeof value.observation.expires_at !== "string" || !Number.isFinite(Date.parse(value.observation.expires_at)))) throw codexInvalid();
  return value;
}
function saveCodexOriginal(original, storage) {
  return writeOriginalJson(storage, codexKey(original.context), original, codexInvalid);
}
function newCodexOriginal(context, selection, storage, actionId) {
  const request = codexRequest({ ...selection, mutation_id: actionId() });
  return saveCodexOriginal({ version: 1, context: codexContext(context, request),
    bytes: JSON.stringify(request), observation: null }, storage);
}
function currentCodexOriginal(original, storage) {
  const current = findCodexLoginOriginal({ origin: original.context.origin, nonce: original.context.nonce },
    original.context, storage);
  if (!current || current.bytes !== original.bytes) throw codexInvalid();
  return current;
}
function codexReceipt(receipt, request) {
  const prefix = request.role === "kohai" ? "llm.default" : "llm.sempai";
  if (!receipt || receipt.mutation_id !== request.mutation_id || receipt.operation !== "upsert" ||
      receipt.provider_id !== request.provider_id ||
      !(Number.isSafeInteger(receipt.provider_revision) && receipt.provider_revision > 0 ||
        typeof receipt.provider_revision === "string" && /^[1-9][0-9]*$/.test(receipt.provider_revision)) ||
      (receipt.selected_role != null && receipt.selected_role !== request.role) ||
      receipt.role_selections?.[`${prefix}.provider_id`] !== request.provider_id ||
      receipt.role_selections?.[`${prefix}.model`] !== request.model) throw codexInvalid();
  return receipt;
}
export function checkedCodexLoginStatus(reply, original) {
  const request = codexRequest(JSON.parse(original.bytes));
  if (!codexObject(reply, ["mutation_id", "status", "expires_at", "error", "mutation_receipt", "poll_interval_millis"]) ||
      reply.mutation_id !== request.mutation_id || !CODEX_STATES.has(reply.status) ||
      typeof reply.expires_at !== "string" || !Number.isFinite(Date.parse(reply.expires_at)) || !codexInterval(reply.poll_interval_millis) ||
      (reply.error !== null && !CODEX_ERRORS.has(reply.error)) ||
      (CODEX_TERMINAL.has(original.observation?.status) && original.observation.status !== reply.status)) throw codexInvalid();
  if (reply.mutation_receipt !== null) codexReceipt(reply.mutation_receipt, request);
  if (reply.status === "completed" && (reply.mutation_receipt === null || reply.error !== null)) throw codexInvalid();
  return reply;
}
// Expiry is a current native observation (Host intent, then legitimate issuer
// shortening), never a request field or browser-owned/recomputed deadline.
function observeCodexStatus(reply, original, storage) {
  const current = currentCodexOriginal(original, storage);
  checkedCodexLoginStatus(reply, current);
  return saveCodexOriginal({ ...current, observation: { status: reply.status, expires_at: reply.expires_at } }, storage);
}
export function codexLoginIsTerminal(original) { return CODEX_TERMINAL.has(original?.observation?.status); }

export async function runCodexLogin(context, selection, {
  storage = globalThis.sessionStorage, actionId = clientActionId,
  isCurrent = () => true, onCode = () => {}, onStatus = () => {}, onOriginal = () => {}, signal,
  newOriginal = false, retryOnly = false,
} = {}) {
  const observerCurrent = isCurrent, capturedToken = readStoredToken();
  isCurrent = () => observerCurrent() && !signal?.aborted && originalAuthContextIsCurrent(context, capturedToken);
  let original = findCodexLoginOriginal(context, selection, storage), status = null;
  if (!isCurrent()) return { completed: false };
  if (original) {
    onOriginal(original);
    const request = JSON.parse(original.bytes);
    try { status = await codexLoginStatus(request.mutation_id, { signal }); }
    catch (error) { if (error?.status !== 404 || newOriginal) throw error; }
    if (!isCurrent()) return { completed: false };
    if (status) {
      original = observeCodexStatus(status, original, storage);
      onStatus(status);
    }
    if (!isCurrent()) return { completed: false };
    if (newOriginal) {
      if (!status || !CODEX_TERMINAL.has(status.status)) throw codexInvalid();
      const previous = JSON.parse(original.bytes).mutation_id;
      // Explicit caller confirmation plus CURRENT authenticated terminal read.
      const next = codexRequest({ ...selection, mutation_id: actionId() });
      if (next.mutation_id === previous) throw codexInvalid();
      original = saveCodexOriginal({ version: 1, context: codexContext(context, next),
        bytes: JSON.stringify(next), observation: null }, storage);
      status = null;
    }
  } else {
    if (retryOnly || newOriginal) throw codexInvalid();
    original = newCodexOriginal(context, selection, storage, actionId);
  }
  onOriginal(original);
  const request = JSON.parse(original.bytes);
  if (!isCurrent()) return { completed: false };
  if (!status || !CODEX_TERMINAL.has(status.status)) {
    // An explicit resume can reattach the existing protocol driver, including
    // Completing after restart, using ONLY the same start bytes/ID. The native
    // claim/return owner forbids issuer replay and authenticates the original.
    // Nothing from the current role/provider/model can replace this request.
    const reply = await startCodexLogin(original.bytes, { signal });
    if (!isCurrent()) return { completed: false };
    currentCodexOriginal(original, storage);
    if (!codexObject(reply, ["mutation_id", "expires_at", "poll_interval_millis", "user_code", "verification_uri"]) ||
        reply.mutation_id !== request.mutation_id || !codexInterval(reply.poll_interval_millis) ||
        typeof reply.expires_at !== "string" || !Number.isFinite(Date.parse(reply.expires_at))) throw codexInvalid();
    const challenge = reply.user_code !== null || reply.verification_uri !== null;
    if (challenge) {
      if (!codexText(reply.user_code) || !codexText(reply.verification_uri)) throw codexInvalid();
      let uri;
      try { uri = new URL(reply.verification_uri); } catch { throw codexInvalid(); }
      if (!["http:", "https:"].includes(uri.protocol) || uri.username || uri.password) throw codexInvalid();
      original = saveCodexOriginal({ ...original, observation: {
        status: original.observation?.status ?? null, expires_at: reply.expires_at } }, storage);
      onCode({ userCode: reply.user_code, verificationUri: reply.verification_uri });
    } else {
      onCode(null);
      // This start and the next GET are different native observations. None
      // is honest even before a challenge exists or when a different driver
      // holds the original. Do not infer simultaneous phase or invent a code.
      // An AwaitingUser observation without a pair offers explicit resume;
      // automatic polling never turns it into another POST.
      status = await codexLoginStatus(request.mutation_id, { signal });
      if (!isCurrent()) return { completed: false };
      original = observeCodexStatus(status, original, storage);
    }
  }
  while (isCurrent()) {
    if (!status) status = await codexLoginStatus(request.mutation_id, { signal });
    if (!isCurrent()) return { completed: false };
    original = observeCodexStatus(status, original, storage);
    onStatus(status);
    if (status.status === "completed") {
      const snapshot = await reconcileProviderMutation(request.mutation_id, {
        signal, isCurrent, validateReceipt: receipt => codexReceipt(receipt, request),
      });
      if (!isCurrent()) return { completed: false };
      codexReceipt(snapshot?.mutation_receipt, request);
      return { completed: true, status, snapshot };
    }
    if (CODEX_TERMINAL.has(status.status)) return { completed: false, status };
    // A known returned result in Completing remains observable after expiry.
    // Only the native status owner can classify expiration or completion.
    await new Promise((resolve, reject) => {
      const abort = () => { clearTimeout(timer); reject(new Error("Login observation canceled.")); };
      const timer = setTimeout(() => { signal?.removeEventListener("abort", abort); resolve(); }, status.poll_interval_millis);
      signal?.addEventListener("abort", abort, { once: true });
      if (signal?.aborted) abort();
    });
    status = null;
  }
  return { completed: false };
}
