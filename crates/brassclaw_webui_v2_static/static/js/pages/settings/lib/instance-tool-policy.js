import { apiFetch } from "../../../lib/api.js";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const NIL = "00000000-0000-0000-0000-000000000000";
function validId(id) { return typeof id === "string" && UUID.test(id) && id !== NIL; }
function validRevision(revision) {
  return typeof revision === "string" && /^[1-9][0-9]*$/.test(revision)
    && revision.length <= 20 && BigInt(revision) <= 18446744073709551615n;
}
export function validToolOutputLimit(value) {
  return value === null || validRevision(value);
}
function validLimits(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    && Object.keys(value).length === 1 && Object.hasOwn(value, "max_output_bytes")
    && validToolOutputLimit(value.max_output_bytes);
}
function setting(value) {
  if (!value || !validId(value.tool_id) || !validRevision(value.revision)
      || typeof value.enabled !== "boolean" || !Array.isArray(value.capability_ids)
      || value.capability_ids.some(id => typeof id !== "string" || id.length === 0)
      || !validLimits(value.limits)) {
    throw new Error("Invalid instance Tool policy response; reload before editing.");
  }
  return value;
}

export async function fetchInstanceToolSettings() {
  const values = await apiFetch("/api/settings/tool-policies");
  if (!Array.isArray(values)) throw new Error("Invalid instance Tool policy list.");
  const seen = new Set();
  for (const value of values) {
    setting(value);
    if (seen.has(value.tool_id)) throw new Error("Duplicate instance Tool policy identity.");
    seen.add(value.tool_id);
  }
  return values;
}

export async function updateInstanceToolSetting(toolId, expectedRevision, enabled, limits) {
  if (!validId(toolId) || !validRevision(expectedRevision) || typeof enabled !== "boolean"
      || !validLimits(limits)) {
    throw new Error("Invalid instance Tool policy edit.");
  }
  // Capture a complete primitive value before any await; callers cannot mutate
  // the expected acknowledgement while a request is in flight.
  const expectedLimits = { max_output_bytes: limits.max_output_bytes };
  const value = setting(await apiFetch(`/api/settings/tool-policies/${toolId}`, {
    method: "PUT",
    body: JSON.stringify({ expected_revision: expectedRevision, enabled, limits: expectedLimits }),
  }));
  if (value.tool_id !== toolId || value.enabled !== enabled
      || BigInt(value.revision) !== BigInt(expectedRevision) + 1n
      || value.limits.max_output_bytes !== expectedLimits.max_output_bytes) {
    throw new Error("Unexpected instance Tool policy acknowledgement; reload before editing.");
  }
  return value;
}
