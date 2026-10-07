import { React, html } from "../../../lib/html.js";
import { Card } from "../../../design-system/card.js";
import { useT } from "../../../lib/i18n.js";
import { clientActionId } from "../../../lib/api.js";
import { submitComponentReview, fetchComponentReviewSubmission } from "../lib/settings-api.js";

// Source is submitted/rendered as text. A pending request retains its exact ID
// and bytes until acknowledged; loading by ID recovers a committed subject.
export function ComponentSubmissionPanel() {
  const t = useT();
  const [id, setId] = React.useState("");
  const [candidate, setCandidate] = React.useState("");
  const [base, setBase] = React.useState("null");
  const [dependencies, setDependencies] = React.useState("[]");
  const [pending, setPending] = React.useState(null);
  const [view, setView] = React.useState(null);
  const [receipt, setReceipt] = React.useState(null);
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState("");
  const inFlight = React.useRef(false);
  const uncertain = React.useRef(false);
  const submit = async () => {
    if (inFlight.current) return;
    inFlight.current = true;
    setBusy(true); setError("");
    let sent = false;
    try {
      const request = pending ?? {
        submission_id: id.trim(), candidate_bytes: candidate,
        base: JSON.parse(base), dependencies: JSON.parse(dependencies),
      };
      // Serialize before locking: syntax/transport errors make no request.
      const bytes = JSON.stringify(request);
      if (new TextEncoder().encode(bytes).byteLength > 14 * 1024 * 1024) {
        throw new Error(t("componentSubmission.capacity"));
      }
      setPending(request);
      sent = true;
      setReceipt(await submitComponentReview(request));
    } catch (err) {
      // A first explicit rejection permits correction. Once a response was
      // lost/uncertain, keep the original even if a later retry is rejected.
      if (sent && !uncertain.current && [400, 401, 403, 404, 409, 413, 422, 429].includes(err.status)) {
        setPending(null);
      } else if (sent) { uncertain.current = true; }
      setError(err.message);
    }
    finally { inFlight.current = false; setBusy(false); }
  };
  const load = async () => {
    if (inFlight.current) return;
    inFlight.current = true;
    setBusy(true); setError(""); setView(null);
    try {
      const loaded = await fetchComponentReviewSubmission(id.trim());
      // Reading proves retention, not which actor/request committed it. A
      // pending mutation must still receive its own exact POST acknowledgement.
      setView(loaded);
      if (!pending) setReceipt(loaded.receipt);
    } catch (err) { setError(err.message); }
    finally { inFlight.current = false; setBusy(false); }
  };
  const another = () => {
    try { setId(clientActionId()); }
    catch (err) { setError(err.message); return; }
    setPending(null); setReceipt(null); setView(null); setError("");
    uncertain.current = false;
  };
  const locked = busy || Boolean(pending) || Boolean(receipt);
  return html`
    <${Card} padding="md">
      <h3 className="font-semibold">${t("componentSubmission.title")}</h3>
      <p className="mt-2 text-sm text-[var(--v2-text-muted)]">${t("componentSubmission.scope")}</p>
      <details className="mt-4">
        <summary className="text-sm">${t("componentSubmission.editor")}</summary>
        <div className="mt-3 space-y-3">
          <label className="block text-sm">${t("componentSubmission.id")}
            <input className="mt-1 w-full rounded border p-2 bg-transparent" value=${id}
              disabled=${locked} onChange=${e => { setId(e.target.value); setView(null); }} />
          </label>
          <button type="button" className="rounded border px-3 py-2 text-sm" disabled=${busy || (Boolean(pending) && !receipt)} onClick=${another}>
            ${t("componentSubmission.new")}
          </button>
          <label className="block text-sm">${t("componentSubmission.candidate")}
            <textarea className="mt-1 w-full rounded border p-2 bg-transparent font-mono text-xs" rows="10"
              value=${candidate} disabled=${locked} onChange=${e => setCandidate(e.target.value)} />
          </label>
          <label className="block text-sm">${t("componentSubmission.base")}
            <textarea className="mt-1 w-full rounded border p-2 bg-transparent font-mono text-xs" value=${base}
              disabled=${locked} onChange=${e => setBase(e.target.value)} />
          </label>
          <label className="block text-sm">${t("componentSubmission.dependencies")}
            <textarea className="mt-1 w-full rounded border p-2 bg-transparent font-mono text-xs" value=${dependencies}
              disabled=${locked} onChange=${e => setDependencies(e.target.value)} />
          </label>
          <button type="button" className="rounded border px-3 py-2 text-sm" disabled=${busy || Boolean(receipt) || !id.trim() || !candidate.trim()} onClick=${submit}>
            ${t(pending ? "componentSubmission.retry" : "componentSubmission.submit")}
          </button>
          <button type="button" className="rounded border px-3 py-2 text-sm" disabled=${busy || !id.trim()} onClick=${load}>
            ${t("componentSubmission.load")}
          </button>
          ${error && html`<p role="alert" className="text-sm text-[var(--v2-danger-text)]">${error}</p>`}
          ${receipt && html`<p role="status" className="break-all text-sm">${t("componentSubmission.retained", { id: receipt.submission_id })}</p>`}
          ${view && html`
            <pre className="max-h-96 overflow-auto whitespace-pre-wrap break-all text-xs">${view.subject_bytes}</pre>
            ${view.component_bytes.map((bytes, index) => html`
              <details key=${index} className="rounded border p-2">
                <summary>${t("componentSubmission.component", { index: index + 1 })}</summary>
                <pre className="mt-2 max-h-96 overflow-auto whitespace-pre-wrap break-all text-xs">${bytes}</pre>
              </details>
            `)}
          `}
        </div>
      </details>
    <//>
  `;
}
