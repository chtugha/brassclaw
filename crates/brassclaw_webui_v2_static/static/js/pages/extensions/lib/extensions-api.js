// Extensions surface:
// - The browser talks only to `/api/webchat/v2/extensions/*` endpoints.
// - The v2 backend owns the registry/list/install/activate/remove/setup
//   projection and maps those operations to the extension registry.

import { apiFetch, setupExtension } from "../../../lib/api.js";
import { oauthOriginalBody } from "./oauth-original.js";
import { redeemPairingCode } from "./pairing-api.js";
import { fetchMontyVmStatus } from "../../settings/lib/settings-api.js";

export function fetchExtensions() {
  return apiFetch("/api/webchat/v2/extensions");
}
export function fetchExtensionRegistry() {
  return apiFetch("/api/webchat/v2/extensions/registry");
}
export function installExtension(packageRef) {
  return apiFetch("/api/webchat/v2/extensions/install", {
    method: "POST",
    body: JSON.stringify({ package_ref: packageRef }),
  });
}
export function activateExtension(packageRef) {
  return apiFetch(`/api/webchat/v2/extensions/${encodeURIComponent(packageId(packageRef))}/activate`, {
    method: "POST",
  });
}
export function removeExtension(packageRef) {
  return apiFetch(`/api/webchat/v2/extensions/${encodeURIComponent(packageId(packageRef))}/remove`, {
    method: "POST",
  });
}
export function fetchExtensionSetup(packageRef) {
  return apiFetch(`/api/webchat/v2/extensions/${encodeURIComponent(packageId(packageRef))}/setup`);
}
export function submitExtensionSetup(packageRef, fields) {
  return setupExtension(packageId(packageRef), {
    action: "submit",
    payload: { fields },
  });
}
function manualBody(operation) {
  return { requirement: operation.definition.requirement, identity: { ...operation.identity },
    invocation_id: operation.invocationId };
}
function manualPath(packageRef, action) {
  return `/api/webchat/v2/extensions/${encodeURIComponent(packageId(packageRef))}/setup/manual-token/${action}`;
}
export function prepareExtensionManualToken(packageRef, operation) {
  return apiFetch(manualPath(packageRef, "setup"), { method: "POST",
    body: JSON.stringify({ ...manualBody(operation), target: { ...operation.definition.target } }) });
}
export function submitExtensionManualToken(packageRef, operation, token) {
  return apiFetch(manualPath(packageRef, "secret-submit"), { method: "POST",
    body: JSON.stringify({ ...manualBody(operation), token }) });
}
export function continueExtensionManualToken(packageRef, operation) {
  return apiFetch(manualPath(packageRef, "continue"), { method: "POST", body: JSON.stringify(manualBody(operation)) });
}
export function cancelExtensionManualToken(packageRef, operation) {
  return apiFetch(manualPath(packageRef, "cancel"), { method: "POST", body: JSON.stringify(manualBody(operation)) });
}
// Called only for an absent original or a confirmed explicit successor.
// A configured unavailable owner never falls back to a browser constant.
export async function extensionOauthExpiry() {
  const status = await fetchMontyVmStatus();
  const policy = status?.execution_limits;
  const ttl = policy?.limits?.oauth_setup_default_ttl_secs;
  const maximum = policy?.limits?.oauth_setup_max_ttl_secs;
  if (!Number.isSafeInteger(policy?.effective_revision) || policy.effective_revision <= 0 ||
      !Number.isInteger(ttl) || ttl <= 0 || ttl > 4294967295 ||
      !Number.isInteger(maximum) || maximum <= 0 || maximum > 4294967295 || ttl > maximum) {
    throw new Error("OAuth setup policy is unavailable.");
  }
  return new Date(Date.now() + ttl * 1000).toISOString();
}
export function sendExtensionOauthOriginal(packageRef, operation, action) {
  const id=packageId(packageRef);
  if (id!==operation.definition.packageId) throw new Error("OAuth original package does not match.");
  return apiFetch(`/api/webchat/v2/extensions/${encodeURIComponent(id)}/setup/oauth/start`, {
    method:"POST",body:JSON.stringify(oauthOriginalBody(operation,action)),
  });
}
export function fetchPairingRequests() {
  return Promise.resolve({ requests: [] });
}
export function approvePairingCode(channel, code) {
  return redeemPairingCode(channel, code);
}

function packageId(packageRef) {
  const id = typeof packageRef === "string" ? packageRef : packageRef?.id;
  if (!id) {
    throw new Error("Extension package_ref is required");
  }
  return id;
}
