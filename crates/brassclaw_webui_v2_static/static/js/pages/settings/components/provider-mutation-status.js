import { React, html } from "../../../lib/html.js";
import { useQueryClient } from "@tanstack/react-query";
import { Button } from "../../../design-system/button.js";
import { pendingProviderMutations, subscribeProviderMutations, readProviderMutation, reconcileProviderMutation } from "../lib/provider-mutations.js";

export function ProviderMutationStatus({ t }) {
  const cache = useQueryClient();
  const [pending, setPending] = React.useState(pendingProviderMutations);
  const [busy, setBusy] = React.useState(null);
  const [result, setResult] = React.useState(null);
  const [error, setError] = React.useState(null);
  const inFlight = React.useRef(false);
  React.useEffect(() => subscribeProviderMutations(() => setPending(pendingProviderMutations())), []);
  async function inspect(entry, reconcile) {
    if (inFlight.current) return;
    inFlight.current = true;
    setBusy(entry.id); setError(null);
    try {
      if (reconcile) {
        const snapshot = await reconcileProviderMutation(entry.id);
        setResult(snapshot.mutation_receipt);
      } else setResult(await readProviderMutation(entry.id));
      // Mutation snapshots have no global ordering for roles or other providers.
      // Publish only through the shared canonical query, also after read-only
      // confirmation of a committed request whose runtime reload is pending.
      await cache.invalidateQueries({ queryKey: ["llm-providers"], exact: true }, { throwOnError: true });
    } catch (failure) { setError(failure.message || String(failure)); }
    finally { inFlight.current = false; setBusy(null); }
  }
  if (!pending.length && !result && !error) return null;
  return html`<section className="my-3 space-y-2 rounded-md border p-3" aria-label=${t("llm.mutationTitle")}>
    <p>${t("llm.mutationHelp")}</p>
    ${pending.map(entry => html`<div key=${entry.id} className="space-x-2">
      <span>${entry.target} · ${entry.id} · ${t(entry.committed ? "llm.mutationStoredPending" : "llm.mutationUnknown")}</span>
      <${Button} size="sm" disabled=${Boolean(busy)} onClick=${() => inspect(entry, false)}>${t("llm.mutationRead")}<//>
      <${Button} size="sm" disabled=${Boolean(busy)} onClick=${() => inspect(entry, true)}>${t("llm.mutationReconcile")}<//>
    </div>`)}
    ${result && html`<p role="status">${t("llm.mutationCommitted")}: ${result.mutation_id} · ${result.provider_id || "—"} · ${result.provider_revision ?? "—"}</p>`}
    ${error && html`<p role="alert">${error}</p>`}
  </section>`;
}
