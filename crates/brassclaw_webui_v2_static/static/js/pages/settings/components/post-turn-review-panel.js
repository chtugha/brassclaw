import { React, html } from "../../../lib/html.js";
import { Card } from "../../../design-system/card.js";
import { useT } from "../../../lib/i18n.js";
import { clientActionId } from "../../../lib/api.js";
import { fetchPostTurnReviews, inspectPostTurnReview, recordReviewDisposition } from "../lib/settings-api.js";

export function PostTurnReviewPanel() {
  const t = useT();
  const [page, setPage] = React.useState(null);
  const [attempt, setAttempt] = React.useState("");
  const [view, setView] = React.useState(null);
  const [note, setNote] = React.useState("");
  const [decision, setDecision] = React.useState(null);
  const requestRef = React.useRef(null);
  const [receipt, setReceipt] = React.useState(null);
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState("");
  const locked = busy || Boolean(decision);
  const list = async (after = null) => {
    setBusy(true); setError("");
    try { setPage(await fetchPostTurnReviews(after)); }
    catch (err) { setError(err.message); }
    finally { setBusy(false); }
  };
  const load = async (id) => {
    setBusy(true); setError(""); setView(null); setAttempt(id); setNote(""); setReceipt(null);
    try { setView(await inspectPostTurnReview(id)); }
    catch (err) { setError(err.message); }
    finally { setBusy(false); }
  };
  const save = async () => {
    setBusy(true); setError("");
    try {
      const request = requestRef.current ?? { disposition_id: clientActionId(), evidence_checksum: view.evidence_checksum, note };
      requestRef.current = request;
      setDecision(request);
      setReceipt(await recordReviewDisposition(attempt, request));
    }
    catch (err) { setError(err.message); }
    finally { setBusy(false); }
  };
  const reset = () => { requestRef.current = null; setDecision(null); setReceipt(null); setView(null); setNote(""); setError(""); };
  let evidence = null;
  if (view) {
    try { evidence = JSON.parse(view.evidence_bytes); }
    catch { /* Invalid retained data remains visible as exact text. */ }
  }
  const stopped = ["failed", "uncertain", "incomplete"].includes(evidence?.work?.phase);
  const tooLarge = new TextEncoder().encode(note).byteLength > 16384;
  return html`
    <${Card} padding="md">
      <h3 className="font-semibold">${t("postTurnReview.title")}</h3>
      <p className="mt-2 text-sm text-[var(--v2-text-muted)]">${t("postTurnReview.scope")}</p>
      <div className="mt-4 space-y-3">
        <button type="button" className="rounded border px-3 py-2 text-sm" disabled=${locked} onClick=${() => list()}>
          ${t("postTurnReview.list")}
        </button>
        ${page?.items.map(item => html`
          <button key=${item.attempt_id} type="button" className="block w-full rounded border p-2 text-left text-sm break-all"
            disabled=${locked} onClick=${() => load(item.attempt_id)}>
            ${item.attempt_id} · ${item.phase} · ${t("postTurnReview.dispatches", { count: item.model_dispatch_count })}
            · ${t("postTurnReview.notes", { count: item.disposition_count })}
          </button>
        `)}
        ${page && page.items.length === 0 && html`<p className="text-sm">${t("postTurnReview.empty")}</p>`}
        ${page?.next_after && html`<button type="button" disabled=${locked} className="rounded border px-3 py-2 text-sm"
          onClick=${() => list(page.next_after)}>${t("postTurnReview.next")}</button>`}
        ${view && html`
          <p className="break-all font-mono text-xs">${view.evidence_checksum}</p>
          <p className="text-sm">${t("postTurnReview.integrity", { status: evidence?.journal_integrity === true ? t("postTurnReview.valid") : t("postTurnReview.invalid") })}</p>
          <details className="rounded border p-2">
            <summary>${t("postTurnReview.evidence")}</summary>
            <pre className="mt-2 max-h-96 overflow-auto whitespace-pre-wrap break-all text-xs">${view.evidence_bytes}</pre>
          </details>
          <p className="text-sm">${t("postTurnReview.recentNotes", { count: view.disposition_count })}</p>
          ${view.dispositions.map(entry => html`
            <details key=${entry.receipt.disposition_id} className="rounded border p-2">
              <summary className="break-all text-sm">${entry.actor} · ${entry.receipt.disposition_id}</summary>
              <p className="break-all font-mono text-xs">${entry.receipt.evidence_checksum}</p>
              <pre className="whitespace-pre-wrap break-all text-sm">${entry.note}</pre>
            </details>
          `)}
          ${stopped && html`
            <label className="block text-sm">${t("postTurnReview.note")}
              <textarea className="mt-1 w-full rounded border p-2 bg-transparent" value=${note} maxLength=${16384}
                disabled=${locked} onChange=${e => setNote(e.target.value)} />
            </label>
            <button type="button" className="rounded border px-3 py-2 text-sm"
              disabled=${busy || Boolean(receipt) || !note.trim() || tooLarge} onClick=${save}>
              ${t(decision ? "postTurnReview.retry" : "postTurnReview.save")}
            </button>
          `}
        `}
        ${error && html`<p role="alert" className="text-sm text-[var(--v2-danger-text)]">${error}</p>`}
        ${receipt && html`<p role="status" className="break-all text-sm">${t("postTurnReview.recorded", { id: receipt.disposition_id })}</p>`}
        ${decision && html`<button type="button" disabled=${busy} className="rounded border px-3 py-2 text-sm"
          onClick=${reset}>${t("postTurnReview.reset")}</button>`}
      </div>
    <//>
  `;
}
