import { html } from "../../../lib/html.js";

// The global Tokens tab has been removed. Token budget settings are now
// configured per-provider in the provider dialog (Settings → Inference →
// select a provider → Token Budget tab).
// This file is kept as a tombstone to avoid 404s from any stale cached route
// navigations; it renders nothing.
export function TokensTab() {
  return html`<div />`;
}
