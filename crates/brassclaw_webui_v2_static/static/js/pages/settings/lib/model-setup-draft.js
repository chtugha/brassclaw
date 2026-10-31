import { clientActionId } from "../../../lib/api.js";
import { fetchPrefixModelSetupDraft, updatePrefixModelSetupDraft } from "./settings-api.js";

export const SETUP_FIELDS = Object.freeze(["max_prefix_tokens", "max_prompt_tokens", "reserved_output_tokens", "additional_framing_tokens"]);
const zeroAllowed = new Set(["max_prefix_tokens", "additional_framing_tokens"]);
const own = (value, key) => Object.hasOwn(value, key);
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const fail = () => { throw Error("prefix.setupInvalid"); };
function keys(value, expected) {
  if (!value || typeof value !== "object" || Array.isArray(value) || Object.keys(value).length !== expected.length || !expected.every(key => own(value, key))) fail();
}
function integer(value, minimum = 0) { return Number.isSafeInteger(value) && value >= minimum; }
function validValue(field, value) { return value === null || integer(value, zeroAllowed.has(field) ? 0 : 1); }
function uuid(value) { return typeof value === "string" && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value) && value !== "00000000-0000-0000-0000-000000000000"; }
export function parseSetupDraft(value, specification) {
  keys(value, ["specification", "revision", "operation_id", "fields"]);
  if (!/^[0-9a-f]{64}$/.test(specification) || value.specification !== specification || !integer(value.revision) || (value.revision === 0 ? value.operation_id !== null : !uuid(value.operation_id))) fail();
  keys(value.fields, SETUP_FIELDS);
  const fields = {};
  for (const field of SETUP_FIELDS) {
    const item = value.fields[field]; keys(item, ["value", "origin"]);
    if (!validValue(field, item.value) || !["unset", "manual", "provider_configuration"].includes(item.origin)) fail();
    if (item.origin === "unset" && item.value !== null) fail();
    if (item.origin === "provider_configuration" && (item.value === null || !["max_prefix_tokens", "max_prompt_tokens"].includes(field))) fail();
    fields[field] = { value: item.value, origin: item.origin };
  }
  return { specification, revision: value.revision, operation_id: value.operation_id, fields };
}
export function setupStrings(draft) {
  return Object.fromEntries(SETUP_FIELDS.map(field => [field, draft.fields[field].value === null ? "" : String(draft.fields[field].value)]));
}
export function setupEdits(strings, touched) {
  if (Object.keys(touched).some(field => !SETUP_FIELDS.includes(field))) fail();
  return SETUP_FIELDS.filter(field => touched[field]).map(field => {
    const raw = strings[field];
    if (typeof raw !== "string" || (raw !== "" && !/^(0|[1-9][0-9]*)$/.test(raw))) fail();
    const value = raw === "" ? null : Number(raw);
    if (!validValue(field, value)) fail();
    return { field, value };
  });
}
function receiptFor(original, value) {
  const draft = parseSetupDraft(value, original.basis.specification);
  const request = original.request;
  if (draft.operation_id !== request.operation_id || draft.revision !== request.expected_revision + 1) fail();
  for (const field of SETUP_FIELDS) {
    const edit = request.edits.find(edit => edit.field === field);
    const expected = edit ? { value: edit.value, origin: "manual" } : original.basis.fields[field];
    if (!same(draft.fields[field], expected)) fail();
  }
  return draft;
}

// Only unresolved original intents live here, shared across remounts of this
// page. This is neither a canonical draft cache nor a new persistence service.
// Never put request bodies in local/session storage or invent retry identities.
const pending = new Map();
const listeners = new Set();
const notify = event => { for (const listener of listeners) listener(event); };
export function subscribeSetupDraft(listener) { listeners.add(listener); return () => listeners.delete(listener); }
export function pendingSetupDraft(specification) {
  const original = pending.get(specification);
  return original ? { basis: structuredClone(original.basis), request: structuredClone(original.request), busy: !!original.promise } : null;
}
export async function loadSetupDraft(specification) {
  return parseSetupDraft(await fetchPrefixModelSetupDraft(specification), specification);
}
async function settle(original) {
  const specification = original.basis.specification;
  if (!original.receipt) {
    try {
      original.receipt = receiptFor(original, await updatePrefixModelSetupDraft(specification, original.request));
    } catch (error) {
      // A missing acknowledgement does not establish rollback. Inspect only
      // the exact successor slot; a different committed operation proves CAS
      // excluded this original intent. Otherwise retain it for explicit retry.
      let slot;
      try { slot = parseSetupDraft(await fetchPrefixModelSetupDraft(specification, original.request.expected_revision + 1), specification); }
      catch { throw error; }
      if (slot.operation_id !== original.request.operation_id) {
        pending.delete(specification); throw Error("prefix.setupConflict");
      }
      original.receipt = receiptFor(original, slot);
    }
  }
  const current = await loadSetupDraft(specification);
  if (current.revision < original.receipt.revision || (current.revision === original.receipt.revision && !same(current, original.receipt))) fail();
  pending.delete(specification);
  return current;
}
function dispatch(original) {
  if (original.promise) return original.promise;
  original.promise = settle(original).then(current => {
    notify({ specification: original.basis.specification, operation_id: original.request.operation_id, current });
    return current;
  }).finally(() => { original.promise = null; notify(); });
  notify(); return original.promise;
}
export function saveSetupDraft(basis, strings, touched) {
  const checked = parseSetupDraft(basis, basis.specification);
  if (pending.has(checked.specification)) throw Error("prefix.setupPending");
  const edits = setupEdits(strings, touched);
  if (!edits.length || checked.revision === Number.MAX_SAFE_INTEGER) fail();
  const operation_id = clientActionId();
  if (!uuid(operation_id)) fail();
  const original = { basis: checked, request: { operation_id, expected_revision: checked.revision, edits }, receipt: null, promise: null };
  pending.set(checked.specification, original);
  return dispatch(original);
}
export function retrySetupDraft(specification) {
  const original = pending.get(specification);
  if (!original) throw Error("prefix.setupInvalid");
  return dispatch(original);
}
