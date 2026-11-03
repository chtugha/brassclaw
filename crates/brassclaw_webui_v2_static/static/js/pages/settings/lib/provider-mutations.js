import { apiFetch, clientActionId, readStoredToken, readOriginalAuthContext, originalAuthContextIsCurrent } from "../../../lib/api.js";

// Transport state only. Never persist a provider payload or API key in browser
// storage. Original bytes stay in memory only while their auth context remains
// current and the outcome is uncertain; abandoned originals keep references only.
const pending = new Map();
// Ephemeral navigation notifications carry only the immutable original receipt.
// Listener failures cannot change a committed mutation into a transport retry.
const bootstrapListeners = new Set();
let navigationOrder = 0;
export function subscribeProviderBootstrap(listener) { bootstrapListeners.add(listener); return () => bootstrapListeners.delete(listener); }
function bootstrapAvailable(receipt, order = ++navigationOrder) {
  for (const listener of bootstrapListeners) { try { listener(receipt, order); } catch {} }
}
const eventName = "brassclaw-provider-mutation-pending";
function changed() {
  if (typeof globalThis.dispatchEvent === "function" && typeof globalThis.Event === "function") globalThis.dispatchEvent(new Event(eventName));
}
function settingsChanged() {
  if (typeof globalThis.dispatchEvent === "function" && typeof globalThis.Event === "function") globalThis.dispatchEvent(new Event("brassclaw-provider-settings-changed"));
}
function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === "object") return Object.fromEntries(Object.keys(value).sort().filter(key => value[key] !== undefined).map(key => [key, canonical(value[key])]));
  return value;
}
// Reuse the existing original-auth namespace. These are in-memory transport
// fences, not credentials, Tool grants or a second authentication store.
function captureContext() {
  const token = readStoredToken(), context = readOriginalAuthContext();
  return { context, isCurrent: () => originalAuthContextIsCurrent(context, token) };
}
function contextChanged(entry) {
  const error = new Error(`Authentication changed while provider change ${entry.id} was pending. Its original outcome requires inspection; the request was not restarted.`);
  error.mutationId = entry.id;
  return error;
}
function requireCurrent(entry) {
  if (!currentOriginal(entry)) throw contextChanged(entry);
}
function currentOriginal(entry) {
  if (entry.isCurrent()) return true;
  // Keep the original nonsecret ID/outcome metadata for inspection, but an
  // abandoned context must not retain a reusable secret-bearing POST body.
  // Recovery of this original is reference-only after this point.
  entry.bytes = null;
  return false;
}
function pendingOriginal(id) {
  const entry = Array.from(pending.values()).find(item => item.id === id);
  // An explicit reference read uses the current observer's authentication.
  // Retire old POST bytes without turning their author context into a new
  // permission gate for the authenticated instance operator.
  if (entry) currentOriginal(entry);
  return entry;
}
async function reconcileOriginal(entry) {
  requireCurrent(entry);
  const snapshot = await reconcileProviderMutation(entry.id, { isCurrent: entry.isCurrent });
  requireCurrent(entry);
  return snapshot;
}
function receiptMatches(receipt, entry) {
  return receipt?.mutation_id === entry.id && receipt.operation === entry.operation;
}
function complete(snapshot, entry, isCurrent = () => currentOriginal(entry)) {
  if (!isCurrent()) throw contextChanged(entry);
  if (!receiptMatches(snapshot?.mutation_receipt, entry)) throw new Error(`Provider change ${entry.id}: original acknowledgement unavailable.`);
  // A reference-only reconcile may have settled this entry while its original
  // response was delayed. Never remove a subsequent request for the same target.
  if (pending.get(entry.key) === entry) pending.delete(entry.key);
  changed();
  settingsChanged();
  bootstrapAvailable(snapshot.mutation_receipt, entry.navigationOrder);
  return snapshot;
}

export function pendingProviderMutations() {
  return Array.from(pending.values()).filter(currentOriginal).map(entry => ({ id: entry.id, operation: entry.operation, target: entry.target, committed: entry.committed === true }));
}
export function subscribeProviderMutations(listener) {
  globalThis.addEventListener(eventName, listener);
  return () => globalThis.removeEventListener(eventName, listener);
}
// Login observers carry their original context/abort fence and exact receipt
// validator through this shared transport. Check both before sending and before
// publishing any pending/settings/bootstrap projection; checking in the caller
// after this function returns would already be too late.
export async function readProviderMutation(id, { signal, isCurrent = () => true, validateReceipt = () => {} } = {}) {
  if (signal?.aborted || !isCurrent()) return null;
  const captured = captureContext(), entry = pendingOriginal(id);
  const receipt = await apiFetch(`/api/webchat/v2/llm/mutations/${encodeURIComponent(id)}`, { signal });
  if (signal?.aborted || !isCurrent() || !captured.isCurrent()) return null;
  if (receipt?.mutation_id !== id) throw new Error("Original provider acknowledgement differs.");
  validateReceipt(receipt);
  if (entry) {
    if (!receiptMatches(receipt, entry)) throw new Error("Original provider acknowledgement differs.");
    // Persistence is now known, even if runtime reconciliation fails. Retain
    // only in-memory retry state and allow an explicit corrective new payload.
    entry.committed = true;
    entry.receipt = receipt;
    changed();
  }
  // A persisted change invalidates provider-dependent views even when the
  // following runtime reload fails. This is not an uptake acknowledgement.
  settingsChanged();
  bootstrapAvailable(receipt, entry?.navigationOrder);
  return receipt;
}
export async function reconcileProviderMutation(id, { signal, isCurrent = () => true, validateReceipt = () => {} } = {}) {
  if (signal?.aborted || !isCurrent()) return null;
  const captured = captureContext(), entry = pendingOriginal(id);
  const snapshot = await apiFetch(`/api/webchat/v2/llm/mutations/${encodeURIComponent(id)}/reconcile`, { method: "POST", signal });
  if (signal?.aborted || !isCurrent() || !captured.isCurrent()) return null;
  if (snapshot?.mutation_receipt?.mutation_id !== id) throw new Error("Original provider acknowledgement differs.");
  validateReceipt(snapshot.mutation_receipt);
  if (entry) return complete(snapshot, entry, () => !signal?.aborted && isCurrent() && captured.isCurrent());
  bootstrapAvailable(snapshot.mutation_receipt);
  return snapshot;
}

export async function submitProviderMutation(operation, target, path, payload) {
  const captured = captureContext();
  for (const original of pending.values()) currentOriginal(original);
  const key = JSON.stringify([captured.context.origin, captured.context.nonce, operation, target]);
  const bytes = JSON.stringify(canonical(payload));
  let entry = pending.get(key);
  if (entry?.committed) {
    if (entry.bytes === bytes) {
      // This is still the same saved request: never rotate its key or create
      // a replacement revision just because its reload failed.
      return entry.inFlight || reconcileOriginal(entry);
    }
    // A different explicit edit is allowed after its predecessor is known to
    // have committed. The server still checks its supplied definition revision.
    entry = null;
  }
  if (entry && entry.bytes !== bytes) throw new Error(`Provider change ${entry.id} is unresolved. Read or reconcile its original outcome before changing the request.`);
  if (!entry) {
    const id = clientActionId();
    if (Array.from(pending.values()).some(original => original.id === id)) throw new Error("Provider change identity is already in use.");
    entry = { id, operation, target, key, bytes, path, isCurrent: captured.isCurrent, navigationOrder: ++navigationOrder };
    pending.set(key, entry);
    changed();
  }
  // Concurrent clicks share this one actual request and original identity.
  if (entry.inFlight) return entry.inFlight;
  entry.inFlight = (async () => {
    try {
      requireCurrent(entry);
      return complete(await apiFetch(entry.path, { method: "POST", body: JSON.stringify({ ...JSON.parse(entry.bytes), mutation_id: entry.id }) }), entry);
    } catch (error) {
      // Never use new authentication to inspect/reconcile or replay an older
      // request. Its immutable native outcome remains owned by the server.
      requireCurrent(entry);
      // A transport failure can follow a successful commit. Read the original
      // before deciding whether another write is allowed. Reconcile has no body.
      try {
        const receipt = await readProviderMutation(entry.id, { isCurrent: entry.isCurrent });
        requireCurrent(entry);
        if (!receiptMatches(receipt, entry)) throw new Error("Original provider acknowledgement differs.");
        return await reconcileOriginal(entry);
      } catch (readError) {
        requireCurrent(entry);
        if (entry.committed) {
          const failure = new Error(`Provider change ${entry.id} committed; runtime reconciliation is unavailable. Read its receipt or submit an explicit corrective edit.`);
          failure.committedReceipt = entry.receipt;
          throw failure;
        }
        // A classified precommit rejection plus an absent original is final.
        // Absence after a timeout/5xx is not proof of rollback.
        if ([400, 404, 409, 422].includes(error.status) && readError.status === 404) {
          if (pending.get(key) === entry) pending.delete(key);
          changed();
          throw error;
        }
        const unresolved = new Error(`Provider change ${entry.id} remains unresolved. Read or reconcile its original outcome; retry keeps the same request.`);
        unresolved.mutationId = entry.id;
        throw unresolved;
      }
    } finally {
      entry.inFlight = null;
    }
  })();
  return entry.inFlight;
}
