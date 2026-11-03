import { React, html } from "../../../lib/html.js";
import { readStoredToken, readOriginalAuthContext, originalAuthContextIsCurrent } from "../../../lib/api.js";
import { useQueryClient } from "@tanstack/react-query";
import { Button } from "../../../design-system/button.js";
import { pendingProviderMutations, subscribeProviderMutations, readProviderMutation, reconcileProviderMutation } from "../lib/provider-mutations.js";

export function ProviderMutationStatus({ t }) {
  const cache = useQueryClient();
  const [pending, setPending] = React.useState(pendingProviderMutations);
  const [busy, setBusy] = React.useState(null);
  const [result, setResult] = React.useState(null);
  const [error, setError] = React.useState(null);
  const inFlight = React.useRef(null), mounted = React.useRef(false);
  React.useEffect(() => {
    mounted.current = true;
    const unsubscribe = subscribeProviderMutations(() => setPending(pendingProviderMutations()));
    return () => { mounted.current = false; inFlight.current?.abort(); unsubscribe(); };
  }, []);
  async function inspect(entry, reconcile) {
    if (!mounted.current || inFlight.current) return;
    let token, context;
    try {
      token = readStoredToken();
      context = readOriginalAuthContext();
    } catch (failure) { setError(failure.message || String(failure)); return; }
    const abort = new AbortController();
    inFlight.current = abort;
    const isCurrent = () => mounted.current && inFlight.current === abort &&
      !abort.signal.aborted && originalAuthContextIsCurrent(context, token);
    setBusy(entry.id); setError(null);
    try {
      const options = { signal: abort.signal, isCurrent };
      const value = reconcile
        ? await reconcileProviderMutation(entry.id, options)
        : await readProviderMutation(entry.id, options);
      if (!isCurrent()) return;
      setResult(reconcile ? value.mutation_receipt : value);
      // Mutation snapshots have no global ordering for roles or other providers.
      // Publish only through the shared canonical query, also after read-only
      // confirmation of a committed request whose runtime reload is pending.
      await cache.invalidateQueries({ queryKey: ["llm-providers"], exact: true }, { throwOnError: true });
    } catch (failure) { if (isCurrent()) setError(failure.message || String(failure)); }
    finally {
      if (inFlight.current === abort) {
        inFlight.current = null;
        if (mounted.current) setBusy(null);
      }
    }
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
