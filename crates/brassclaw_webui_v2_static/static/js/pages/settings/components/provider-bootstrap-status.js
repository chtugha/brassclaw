import { React, html } from "../../../lib/html.js";
import { Button } from "../../../design-system/button.js";
import { subscribeProviderBootstrap } from "../lib/provider-mutations.js";
import { providerBootstrapTarget, providerBootstrapStatus } from "../lib/provider-bootstrap.js";
import { fetchPrefixModelSetupJob } from "../lib/settings-api.js";
import { PrefixModelSetupDraft } from "./prefix-model-setup-draft.js";

// Navigation always retains the original receipt target. Neither role queries
// nor a later selection can retarget a delayed response or this open editor.
export function ProviderBootstrapStatus({ t }) {
  const [target, setTarget] = React.useState(null);
  const [open, setOpen] = React.useState(false);
  const [job, setJob] = React.useState(null);
  const [result, setResult] = React.useState(null);
  const [error, setError] = React.useState(null);
  const [busy, setBusy] = React.useState(false);
  const sequence = React.useRef(0);
  const acceptedOrder = React.useRef(0);
  const currentJob = React.useRef(null);
  React.useEffect(() => {
    const unsubscribe = subscribeProviderBootstrap((receipt, order) => {
      if (order < acceptedOrder.current) return;
      acceptedOrder.current = order;
      try {
        const next = providerBootstrapTarget(receipt);
        if (!next) return;
        if (currentJob.current === next.job) return;
        currentJob.current = next.job; sequence.current++;
        setTarget(next); setOpen(false); setJob(null); setResult(null); setBusy(false); setError(null);
      } catch (failure) { setError(failure.message); }
    });
    return () => { sequence.current++; unsubscribe(); };
  }, []);
  async function inspect() {
    const original = target, token = ++sequence.current;
    setBusy(true); setError(null);
    try {
      const value = await fetchPrefixModelSetupJob(original.specification, original.revision, original.job);
      const parsed = providerBootstrapStatus(original, value);
      if (sequence.current === token) { setJob(value); setResult(parsed); }
    } catch (failure) { if (sequence.current === token) setError(failure.message); }
    finally { if (sequence.current === token) setBusy(false); }
  }
  if (!target && !error) return null;
  return html`<section className="my-3 space-y-2 rounded-md border p-3" aria-label=${t("prefix.bootstrapTitle")}>
    ${target && html`<p>${t("prefix.bootstrapHelp")} <code>${target.specification}</code> · ${target.revision}</p>
      <${Button} size="sm" disabled=${busy} onClick=${inspect}>${t("prefix.bootstrapInspect")}<//>
      <${Button} size="sm" onClick=${() => setOpen(value => !value)}>${t("prefix.bootstrapOpen")}<//>
      ${job && html`<p role="status">${job.admission_phase || "—"} · ${job.run_status || "—"} · ${job.failure_kind || ""}</p>`}
      ${result && html`<p role="status">${t("prefix.bootstrapScopedResult")}</p><details><summary>${t("prefix.bootstrapOriginal")}</summary><pre className="max-h-96 overflow-auto whitespace-pre-wrap break-all">${result.text}</pre></details>`}
      ${open && html`<${PrefixModelSetupDraft} key=${target.job} specification=${{ id: target.specification }} initialRevision=${Number(target.revision)} t=${t} />`}`}
    ${error && html`<p role="alert">${t(error)}</p>`}
  </section>`;
}
