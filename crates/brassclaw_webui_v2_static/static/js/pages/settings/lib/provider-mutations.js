import { apiFetch, clientActionId } from "../../../lib/api.js";

// Transport state only. Never persist a provider payload or API key in browser
// storage. Keep the original bytes in memory while the outcome is uncertain.
const pending = new Map();
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
function receiptMatches(receipt, entry) {
  return receipt?.mutation_id === entry.id && receipt.operation === entry.operation;
}
function complete(snapshot, entry) {
  if (!receiptMatches(snapshot?.mutation_receipt, entry)) throw new Error(`Provider change ${entry.id}: original acknowledgement unavailable.`);
  // A reference-only reconcile may have settled this entry while its original
  // response was delayed. Never remove a subsequent request for the same target.
  if (pending.get(entry.key) === entry) pending.delete(entry.key);
  changed();
  settingsChanged();
  return snapshot;
}

export function pendingProviderMutations() {
  return Array.from(pending.values(), entry => ({ id: entry.id, operation: entry.operation, target: entry.target, committed: entry.committed === true }));
}
export function subscribeProviderMutations(listener) {
  globalThis.addEventListener(eventName, listener);
  return () => globalThis.removeEventListener(eventName, listener);
}
export async function readProviderMutation(id) {
  const receipt = await apiFetch(`/api/webchat/v2/llm/mutations/${encodeURIComponent(id)}`);
  if (receipt?.mutation_id !== id) throw new Error("Original provider acknowledgement differs.");
  const entry = Array.from(pending.values()).find(item => item.id === id);
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
  return receipt;
}
export async function reconcileProviderMutation(id) {
  const snapshot = await apiFetch(`/api/webchat/v2/llm/mutations/${encodeURIComponent(id)}/reconcile`, { method: "POST" });
  if (snapshot?.mutation_receipt?.mutation_id !== id) throw new Error("Original provider acknowledgement differs.");
  const entry = Array.from(pending.values()).find(item => item.id === id);
  return entry ? complete(snapshot, entry) : snapshot;
}

export async function submitProviderMutation(operation, target, path, payload) {
  const key = `${operation}:${target}`;
  const bytes = JSON.stringify(canonical(payload));
  let entry = pending.get(key);
  if (entry?.committed) {
    if (entry.bytes === bytes) {
      // This is still the same saved request: never rotate its key or create
      // a replacement revision just because its reload failed.
      return entry.inFlight || reconcileProviderMutation(entry.id);
    }
    // A different explicit edit is allowed after its predecessor is known to
    // have committed. The server still checks its supplied definition revision.
    entry = null;
  }
  if (entry && entry.bytes !== bytes) throw new Error(`Provider change ${entry.id} is unresolved. Read or reconcile its original outcome before changing the request.`);
  if (!entry) {
    entry = { id: clientActionId(), operation, target, key, bytes, path };
    pending.set(key, entry);
    changed();
  }
  // Concurrent clicks share this one actual request and original identity.
  if (entry.inFlight) return entry.inFlight;
  entry.inFlight = (async () => {
    try {
      return complete(await apiFetch(entry.path, { method: "POST", body: JSON.stringify({ ...JSON.parse(entry.bytes), mutation_id: entry.id }) }), entry);
    } catch (error) {
      // A transport failure can follow a successful commit. Read the original
      // before deciding whether another write is allowed. Reconcile has no body.
      try {
        const receipt = await readProviderMutation(entry.id);
        if (!receiptMatches(receipt, entry)) throw new Error("Original provider acknowledgement differs.");
        return await reconcileProviderMutation(entry.id);
      } catch (readError) {
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
