import { useQuery, useQueryClient } from "@tanstack/react-query";
import { React, html } from "../../../lib/html.js";
import { Button } from "../../../design-system/button.js";
import { fetchLlmProviders, upsertLlmProvider } from "../lib/settings-api.js";
import { modelTokenLimitsPayload } from "../lib/model-token-limits.js";
import { ProviderMutationStatus } from "./provider-mutation-status.js";

// Used by both Inference and Prefix settings, with the same query and API.
export function ModelTokenLimits({ providerId, model, t }) {
  const cache = useQueryClient();
  const query = useQuery({ queryKey: ["llm-providers"], queryFn: fetchLlmProviders });
  const provider = query.data?.providers?.find(item => item.id === providerId);
  const limits = provider?.model_token_limits?.[model];
  const [prefix, setPrefix] = React.useState("");
  const [prompt, setPrompt] = React.useState("");
  const [revision, setRevision] = React.useState(null);
  const [dirty, setDirty] = React.useState(false);
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState(null);
  const [lastReceipt, setLastReceipt] = React.useState(null);
  const inFlight = React.useRef(false);
  const mounted = React.useRef(true);
  React.useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  React.useEffect(() => {
    if (dirty) return;
    setPrefix(limits ? String(limits.max_prefix_tokens) : "");
    setPrompt(limits ? String(limits.max_prompt_tokens) : "");
    setRevision(provider?.revision ?? null);
  }, [provider?.revision, providerId, model, dirty]);
  const supported = provider && Object.hasOwn(provider, "model_token_limits") && provider.model_token_limits && typeof provider.model_token_limits === "object" && !Array.isArray(provider.model_token_limits);
  const stale = dirty && revision !== provider?.revision;
  async function save() {
    if (inFlight.current || !supported) return;
    inFlight.current = true; setBusy(true); setError(null);
    try {
      const payload = modelTokenLimitsPayload(provider, model, prefix, prompt, revision);
      const snapshot = await upsertLlmProvider(payload);
      if (mounted.current && snapshot.mutation_receipt) setLastReceipt(snapshot.mutation_receipt);
      // A delayed mutation response must not replace newer roles, other
      // providers or model limits. The existing canonical query owns publication.
      await cache.invalidateQueries({ queryKey: ["llm-providers"], exact: true }, { throwOnError: true });
      const retained = snapshot?.providers?.find(item => item.id === providerId);
      const actual = retained?.model_token_limits?.[model];
      const cached = cache.getQueryData?.(["llm-providers"]);
      const newer = [...(cached?.providers || []), ...(cached?.inactive_providers || [])].find(item => item.id === providerId);
      if (newer && newer.revision > (retained?.revision ?? -1)) throw new Error("llm.providerChangedAfterCommit");
      if (snapshot.mutation_receipt && retained?.revision !== snapshot.mutation_receipt.provider_revision) {
        throw new Error("llm.providerChangedAfterCommit");
      }
      if (!actual || actual.max_prefix_tokens !== payload.model_token_limits[model].max_prefix_tokens || actual.max_prompt_tokens !== payload.model_token_limits[model].max_prompt_tokens || retained.revision < revision) throw new Error("llm.modelTokenLimitsUnsupported");
      if (mounted.current) setDirty(false);
    } catch (err) {
      let message = err.message || String(err);
      if (err.committedReceipt) {
        if (mounted.current) setLastReceipt(err.committedReceipt);
        try {
          await cache.invalidateQueries({ queryKey: ["llm-providers"], exact: true }, { throwOnError: true });
        } catch (readError) {
          message += `; ${readError.message || String(readError)}`;
        }
      }
      if (mounted.current) setError(message);
    }
    finally { inFlight.current = false; if (mounted.current) setBusy(false); }
  }
  return html`<fieldset className="my-3 space-y-2 rounded-md border border-[var(--v2-panel-border)] p-3">
    <legend>${t("llm.modelTokenLimits")} · ${model}</legend>
    <p className="text-xs">${t("llm.modelTokenLimitsHelp")}</p>
    <${ProviderMutationStatus} t=${t} />
    ${lastReceipt && html`<p role="status">${t("llm.mutationLastCommitted")}: ${lastReceipt.mutation_id} · ${lastReceipt.provider_revision ?? "—"}</p>`}
    ${query.error && html`<p role="alert">${query.error.message}</p>`}
    ${!supported && html`<p role="status">${t(query.isPending ? "common.loading" : "llm.modelTokenLimitsUnsupported")}</p>`}
    <label className="block">${t("llm.maxPrefixTokens")}<input type="number" min="0" step="1" value=${prefix} disabled=${busy || !supported} onChange=${e => { setPrefix(e.target.value); setDirty(true); }} /></label>
    <label className="block">${t("llm.maxPromptTokens")}<input type="number" min="1" step="1" value=${prompt} disabled=${busy || !supported} onChange=${e => { setPrompt(e.target.value); setDirty(true); }} /></label>
    ${limits && html`<p role="status">${t("llm.currentModelInputLimit")}: ${limits.max_prefix_tokens} + ${limits.max_prompt_tokens} = ${limits.max_prefix_tokens + limits.max_prompt_tokens}</p>`}
    ${stale && html`<p role="alert">${t("llm.providerChanged")}</p>`}
    ${error && html`<p role="alert">${error.startsWith("llm.") ? t(error) : error}</p>`}
    <${Button} size="sm" disabled=${busy || !dirty || stale || !supported} onClick=${save}>${t("common.save")}<//>
    <${Button} size="sm" variant="ghost" disabled=${busy} onClick=${() => { setDirty(false); setError(null); query.refetch(); }}>${t("common.refresh")}<//>
  </fieldset>`;
}
