import { apiFetch } from "../../../lib/api.js";

// Settings endpoints depend on v1 `/api/settings/*`, `/api/llm/*`,
// `/api/tools/*`, `/api/skills/*`, etc. Extension reads use the v2
// registry/list endpoints; the remaining settings APIs are known tech-debt stubs.

// Agent / Networking config — v2 native endpoints (Phase 6).
// GET /api/settings/config returns { settings: { key: value, … } }.
export function fetchSettingsExport() {
  return apiFetch("/api/settings/config");
}
export function fetchSetting(key) {
  return apiFetch("/api/settings/config").then((r) => {
    const val = r?.settings?.[key];
    return val !== undefined ? val : null;
  });
}
export function updateSetting(key, value) {
  return apiFetch(`/api/settings/config/${encodeURIComponent(key)}`, {
    method: "PUT",
    body: JSON.stringify({ value }),
  });
}
export function importSettings(_payload) {
  return Promise.resolve({ success: false, message: "TODO: requires v2 settings endpoint" });
}
// LLM provider configuration — v2 native endpoints. The snapshot is the single
// source of truth: a unified provider list (built-in + operator-defined) plus
// the active selection. API-key values are write-only; the snapshot only ever
// reports `api_key_set`.
export function fetchLlmProviders() {
  return apiFetch("/api/webchat/v2/llm/providers");
}
export function upsertLlmProvider(payload) {
  return apiFetch("/api/webchat/v2/llm/providers", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}
export function deleteLlmProvider(providerId) {
  return apiFetch(`/api/webchat/v2/llm/providers/${encodeURIComponent(providerId)}/delete`, {
    method: "POST",
  });
}
export function setActiveLlm(payload) {
  return apiFetch("/api/webchat/v2/llm/active", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}
export function testLlmProviderConnection(payload) {
  return apiFetch("/api/webchat/v2/llm/test-connection", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}
export function listLlmProviderModels(payload) {
  return apiFetch("/api/webchat/v2/llm/list-models", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}
// Begin NEAR AI browser login. Returns { auth_url } to open; a background task
// stores the session token and makes NEAR AI active once the user authorizes.
export function startNearaiLogin(payload) {
  return apiFetch("/api/webchat/v2/llm/nearai/login", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}


// Begin an OpenAI Codex (ChatGPT subscription) device-code login. Returns
// { user_code, verification_uri } to display; a background task polls for
// authorization, stores the tokens, and makes Codex active once authorized.
export function startCodexLogin() {
  return apiFetch("/api/webchat/v2/llm/codex/login", {
    method: "POST",
  });
}
export function fetchExtensions() {
  return apiFetch("/api/webchat/v2/extensions");
}
export function fetchExtensionRegistry() {
  return apiFetch("/api/webchat/v2/extensions/registry");
}
// Tools/capabilities management — v2 native endpoints
export function fetchTools() {
  return apiFetch("/api/webchat/v2/tools");
}
export function updateToolPermission(toolId, mode) {
  return apiFetch(`/api/webchat/v2/tools/${encodeURIComponent(toolId)}/permission`, {
    method: "PUT",
    body: JSON.stringify({ capability_id: toolId, permission_mode: mode }),
  });
}
export function fetchUsers() {
  return Promise.resolve({ users: [], todo: true });
}
export function createUser(_payload) {
  return Promise.resolve({ success: false, message: "TODO: requires v2 users endpoint" });
}
export function updateUser(_id, _payload) {
  return Promise.resolve({ success: false, message: "TODO: requires v2 users endpoint" });
}

// Safety configuration — v2 native endpoints
export function fetchSafetySensitivePaths() {
  return apiFetch("/api/webchat/v2/safety/sensitive-paths");
}
export function updateSafetySensitivePaths(payload) {
  return apiFetch("/api/webchat/v2/safety/sensitive-paths", {
    method: "PUT",
    body: JSON.stringify(payload),
  });
}
export function fetchSafetyWorkspaceRules() {
  return apiFetch("/api/webchat/v2/safety/workspace-rules");
}
export function updateSafetyWorkspaceRules(payload) {
  return apiFetch("/api/webchat/v2/safety/workspace-rules", {
    method: "PUT",
    body: JSON.stringify(payload),
  });
}
export function fetchSafetyBlockedPaths() {
  return apiFetch("/api/webchat/v2/safety/blocked-paths");
}
export function updateSafetyBlockedPaths(payload) {
  return apiFetch("/api/webchat/v2/safety/blocked-paths", {
    method: "PUT",
    body: JSON.stringify(payload),
  });
}

// Token settings — per-provider only. The global /tokens endpoint has been
// removed; all token budget configuration is now per-provider.
export function fetchProviderTokenSettings(providerId) {
  return apiFetch(
    `/api/webchat/v2/providers/${encodeURIComponent(providerId)}/tokens`
  );
}
export function updateProviderTokenSettings(providerId, payload) {
  return apiFetch(
    `/api/webchat/v2/providers/${encodeURIComponent(providerId)}/tokens`,
    { method: "PUT", body: JSON.stringify(payload) }
  );
}

// Interceptor configuration — v2 native endpoints (Phase 5.5).
export function fetchInterceptorConfig() {
  return apiFetch("/api/webchat/v2/interceptor/config");
}
export function updateInterceptorConfig(payload) {
  return apiFetch("/api/webchat/v2/interceptor/config", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}
// Phase K.1 — Prefix cache routes.
export function fetchPrefixes() {
  return apiFetch("/api/webchat/v2/prefixes");
}
export function regeneratePrefix(name) {
  return apiFetch(`/api/webchat/v2/prefixes/${encodeURIComponent(name)}/regenerate`, {
    method: "POST",
  });
}

// Phase 6 — Settings UI: component library endpoints (10-tab editor).
// Note: `/api/settings/skills` returns every `reborn_skills` row — classes
// 1/2/3 plus 10 (Orchestrator) and 50 (Scaffold) — because `SPEC_SKILLS` has
// no class filter. Callers must narrow by `class_code` themselves.
export function fetchSettingsSkills() {
  return apiFetch("/api/settings/skills");
}
export function fetchSettingsExtensions() {
  return apiFetch("/api/settings/extensions");
}
export function fetchSettingsActions() {
  return apiFetch("/api/settings/actions");
}
export function fetchSettingsOrchestrators() {
  return apiFetch("/api/settings/orchestrators");
}
export function fetchSettingsScaffolds() {
  return apiFetch("/api/settings/scaffolds");
}
export function fetchSettingsRecipes() {
  return apiFetch("/api/settings/recipes");
}
export function fetchSettingsToolSkills() {
  return apiFetch("/api/settings/tool-skills");
}
export function fetchSettingsPythonCode() {
  return apiFetch("/api/settings/python-code");
}
export function fetchSettingsExtensionCatalogues() {
  return apiFetch("/api/settings/extension-catalogues");
}
// Class-0 Tool catalog (`reborn_tools`) — the registered capability
// descriptors, distinct from the runtime permission editor on
// `/api/webchat/v2/tools`.
export function fetchSettingsToolCatalog() {
  return apiFetch("/api/settings/tools");
}
// One catalog row in full: { id, class_code, component } where `component`
// is the whole DB row as opaque JSON. `componentType` is the endpoint
// segment (`recipes`, `tool-skills`, `orchestrators`, …), not the tab id.
export function fetchSettingsComponentDetail(componentType, id) {
  return apiFetch(
    `/api/settings/${encodeURIComponent(componentType)}/${encodeURIComponent(id)}`
  );
}
// The whole catalog's wiring: { nodes: [{id, name, class_code,
// validation_status}], edges: [{from, to, kind, channel, step_ref,
// step_label}] }. Fetched once and indexed client-side, because a reverse
// reference ("which Recipes include this PythonCode") is only answerable
// with every Recipe's step_descriptions in hand.
export function fetchSettingsComponentGraph() {
  return apiFetch("/api/settings/component-graph");
}

// Phase 6 — Monty VM settings + lifecycle.
export function fetchMontyVmSettings() {
  return apiFetch("/api/settings/monty-vm");
}
export function updateMontyVmSettings(payload) {
  return apiFetch("/api/settings/monty-vm", {
    method: "PUT",
    body: JSON.stringify(payload),
  });
}
export function restartMontyVm(payload = {}) {
  return apiFetch("/api/settings/monty-vm/restart", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}
export function fetchMontyVmStatus() {
  return apiFetch("/api/settings/monty-vm/status");
}

// Step C.4 — Operator-level mode-driven security posture (per-layer overrides).
// Wire type is the bare SecurityModeConfig (six `*_override` fields, each
// "auto"|"on"|"off"); GET returns it directly, PUT accepts + returns it.
export function fetchSecuritySettings() {
  return apiFetch("/api/settings/security");
}
export function updateSecuritySettings(payload) {
  return apiFetch("/api/settings/security", {
    method: "PUT",
    body: JSON.stringify(payload),
  });
}

// Phase 6 — Chat preferences (ai_before_user etc.).
export function updateChatPreference(key, value) {
  return apiFetch(`/api/chat/preferences/${encodeURIComponent(key)}`, {
    method: "PUT",
    body: JSON.stringify({ value }),
  });
}

// Phase 6 — Validation queue (operator review surface).
// Reuses the existing v2 recipe/tool-skill validation-queue endpoints.
// project_id defaults to "default" for the global settings scope.
export function fetchValidationQueue() {
  return apiFetch("/api/webchat/v2/validation-queue?project_id=default");
}
export function fetchValidationQueueCount() {
  return apiFetch("/api/webchat/v2/validation-queue/count?project_id=default&status=pending");
}
// Move a component from auto_passed → validated (Q2 manual approve).
// For class_code 10 (Orchestrator) and 50 (Scaffold) the backend enforces
// an LLM audit-clean guard; the frontend mirrors that with a disabled state.
export function validateComponent(classCode, componentId) {
  return apiFetch(
    `/api/webchat/v2/components/${encodeURIComponent(classCode)}/${encodeURIComponent(componentId)}/validate?project_id=default`,
    { method: "PUT", body: JSON.stringify({}) }
  );
}
// Move a component to rejected (Q3 / Q4 depending on review_attempts).
export function rejectComponent(classCode, componentId, feedback) {
  return apiFetch(
    `/api/webchat/v2/components/${encodeURIComponent(classCode)}/${encodeURIComponent(componentId)}/reject?project_id=default`,
    { method: "PUT", body: JSON.stringify({ feedback: feedback ?? null }) }
  );
}

// Phase M.6 — intent inputs CRUD (per-component intent expression surface).
// No caller yet: the mounted surface (intent-template-preview-panel.js) is the
// client-only live-feedback half of M.6, and the save/upsert half waits on the
// recipe/variant editor and project-id threading. Retained because the routes
// are live server-side and this is their only client binding.
// Backed by the v2 settings intent-inputs routes (handlers.rs). The upsert
// re-seeds via `seed_intent_input` server-side, which populates the V076
// `is_template` / `template_prefix` / `template_suffix` columns via
// `parse_template`; the list response carries `input_text` so the client
// recomputes live template feedback (template-feedback.js).
export function listIntentInputs({ projectId, componentId } = {}) {
  const params = new URLSearchParams();
  if (projectId) params.set("project_id", projectId);
  if (componentId) params.set("component_id", componentId);
  const qs = params.toString();
  return apiFetch(`/api/settings/intent-inputs${qs ? `?${qs}` : ""}`);
}
export function upsertIntentInput(payload) {
  return apiFetch("/api/settings/intent-inputs", {
    method: "PUT",
    body: JSON.stringify(payload),
  });
}
export function deleteIntentInputs({ projectId, classCode, componentId }) {
  const params = new URLSearchParams();
  if (projectId) params.set("project_id", projectId);
  const qs = params.toString();
  return apiFetch(
    `/api/settings/intent-inputs/${encodeURIComponent(classCode)}/${encodeURIComponent(componentId)}${qs ? `?${qs}` : ""}`,
    { method: "DELETE" }
  );
}

// Phase P Step 10 — Docs settings tab (reborn_docus).
export function fetchDocus() {
  return apiFetch("/api/webchat/v2/docus");
}
export function fetchDocusItem(id) {
  return apiFetch(`/api/webchat/v2/docus/${encodeURIComponent(id)}`);
}
export function updateDocusContent(id, content) {
  return apiFetch(`/api/webchat/v2/docus/${encodeURIComponent(id)}`, {
    method: "PUT",
    body: JSON.stringify({ content }),
  });
}

// Phase V — Orchestrator MCP Server settings + lifecycle.
export function fetchMcpServerSettings() {
  return apiFetch("/api/settings/mcp-server");
}
export function updateMcpServerSettings(payload) {
  return apiFetch("/api/settings/mcp-server", {
    method: "PUT",
    body: JSON.stringify(payload),
  });
}
export function fetchMcpServerStatus() {
  return apiFetch("/api/settings/mcp-server/status");
}
export function startMcpServer(payload = {}) {
  return apiFetch("/api/settings/mcp-server/start", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}
export function stopMcpServer(payload = {}) {
  return apiFetch("/api/settings/mcp-server/stop", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}
