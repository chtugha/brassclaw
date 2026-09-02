import { apiFetch } from "../../../lib/api.js";

// Settings endpoints depend on v1 `/api/settings/*`, `/api/llm/*`,
// `/api/tools/*`, `/api/skills/*`, etc. Extension reads use the v2
// registry/list endpoints; the remaining settings APIs are known tech-debt stubs.

export function fetchSettingsExport() {
  return Promise.resolve({ settings: {}, todo: true });
}
export function fetchSetting(_key) {
  return Promise.resolve(null);
}
export function updateSetting(_key, _value) {
  return Promise.resolve({ success: false, message: "TODO: requires v2 settings endpoint" });
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

// Complete a NEAR AI wallet (NEP-413) login. `payload` carries the browser
// wallet's signed message; the backend relays it to NEAR AI, stores the session
// token, and makes NEAR AI active. Returns { active }.
export function completeNearaiWalletLogin(payload) {
  return apiFetch("/api/webchat/v2/llm/nearai/wallet", {
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
export function fetchSkills() {
  return apiFetch("/api/webchat/v2/skills");
}
export function installSkill(payload) {
  return apiFetch("/api/webchat/v2/skills/install", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}
export function removeSkill(name) {
  return apiFetch(`/api/webchat/v2/skills/${encodeURIComponent(name)}`, {
    method: "DELETE",
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
export function fetchSettingsSkills() {
  return apiFetch("/api/settings/skills");
}
export function fetchSettingsTools() {
  return apiFetch("/api/settings/tools");
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

// Phase 6 — Chat preferences (ai_before_user etc.).
export function updateChatPreference(key, value) {
  return apiFetch(`/api/chat/preferences/${encodeURIComponent(key)}`, {
    method: "PUT",
    body: JSON.stringify({ value }),
  });
}

// Phase 6 — Validation queue (operator review surface).
// Reuses the existing v2 recipe/tool-skill validation-queue endpoints.
export function fetchValidationQueue() {
  return apiFetch("/api/webchat/v2/validation-queue");
}
export function fetchValidationQueueCount() {
  return apiFetch("/api/webchat/v2/validation-queue/count");
}
// Move a component from auto_passed → validated (Q2 manual approve).
// For class_code 10 (Orchestrator) and 50 (Scaffold) the backend enforces
// an LLM audit-clean guard; the frontend mirrors that with a disabled state.
export function validateComponent(classCode, componentId) {
  return apiFetch(
    `/api/webchat/v2/components/${encodeURIComponent(classCode)}/${encodeURIComponent(componentId)}/validate`,
    { method: "PUT", body: JSON.stringify({}) }
  );
}
// Move a component to rejected (Q3 / Q4 depending on review_attempts).
export function rejectComponent(classCode, componentId, feedback) {
  return apiFetch(
    `/api/webchat/v2/components/${encodeURIComponent(classCode)}/${encodeURIComponent(componentId)}/reject`,
    { method: "PUT", body: JSON.stringify({ feedback: feedback ?? null }) }
  );
}
