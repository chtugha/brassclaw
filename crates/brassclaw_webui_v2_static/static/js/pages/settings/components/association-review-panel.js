import { React, html } from "../../../lib/html.js";
import { Card } from "../../../design-system/card.js";
import { useT } from "../../../lib/i18n.js";
import { clientActionId } from "../../../lib/api.js";
import { prepareAssociationReview, approveAssociation } from "../lib/settings-api.js";

// Q2 reviews an exact usage/evidence selection. Authoring and catalogue
// activation remain separate. Values render as text, never HTML or code.
export function AssociationReviewPanel() {
  const t = useT();
  const [skill, setSkill] = React.useState("");
  const [q1, setQ1] = React.useState("");
  const [behavior, setBehavior] = React.useState("");
  const [view, setView] = React.useState(null);
  const [selection, setSelection] = React.useState(null);
  const [note, setNote] = React.useState("");
  const [decision, setDecision] = React.useState(null);
  const [receipt, setReceipt] = React.useState(null);
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState("");
  const startAnother = () => {
    setDecision(null);
    setReceipt(null);
    setView(null);
    setSelection(null);
    setNote("");
    setError("");
  };

  const change = (setter, value) => {
    setter(value);
    setView(null);
    setSelection(null);
    setNote("");
    setReceipt(null);
    setError("");
  };
  const load = async () => {
    setBusy(true);
    setError("");
    setView(null);
    try {
      const selected = { q1_ref: q1.trim(), behavioral_refs: behavior.split(/\s+/).filter(Boolean) };
      const loaded = await prepareAssociationReview(skill.trim(), selected);
      setSelection(selected);
      setView(loaded);
      setNote("");
      setReceipt(null);
    } catch (err) {
      setError(err.message);
    } finally {
      setBusy(false);
    }
  };
  const approve = async () => {
    setBusy(true);
    setError("");
    try {
      const request = decision ?? {
        approval_id: clientActionId(), selection,
        review_checksum: view.review_checksum, semantic_review: note,
      };
      setDecision(request);
      setReceipt(await approveAssociation(skill.trim(), request));
    } catch (err) {
      setError(err.message);
    } finally {
      setBusy(false);
    }
  };
  const locked = busy || Boolean(decision);
  const noteTooLarge = new TextEncoder().encode(note).byteLength > 16384;
  return html`
    <${Card} padding="md">
      <h3 className="font-semibold">${t("associationReview.title")}</h3>
      <p className="mt-2 text-sm text-[var(--v2-text-muted)]">${t("associationReview.scope")}</p>
      <div className="mt-4 space-y-3">
        <label className="block text-sm">${t("associationReview.skill")}
          <input className="mt-1 w-full rounded border p-2 bg-transparent" value=${skill}
            disabled=${locked} onChange=${e => change(setSkill, e.target.value)} />
        </label>
        <label className="block text-sm">${t("associationReview.q1")}
          <input className="mt-1 w-full rounded border p-2 bg-transparent" value=${q1}
            disabled=${locked} onChange=${e => change(setQ1, e.target.value)} />
        </label>
        <label className="block text-sm">${t("associationReview.behavior")}
          <textarea className="mt-1 w-full rounded border p-2 bg-transparent" value=${behavior}
            disabled=${locked} onChange=${e => change(setBehavior, e.target.value)} />
        </label>
        <button type="button" className="rounded border px-3 py-2 text-sm"
          disabled=${locked || !skill.trim() || !q1.trim() || !behavior.trim()} onClick=${load}>
          ${t("associationReview.load")}
        </button>
        ${view && html`
          <p className="break-all font-mono text-xs">${view.review_checksum}</p>
          ${view.review.components.map(component => html`
            <details key=${component.uuid} className="rounded border p-2">
              <summary className="break-all text-sm">${component.uuid} · v${component.version} · ${component.checksum}</summary>
              <pre className="mt-2 max-h-96 overflow-auto whitespace-pre-wrap break-all text-xs">${component.revision_bytes}</pre>
            </details>
          `)}
          ${view.review.evidence.map(evidence => html`
            <details key=${evidence.evidence_id} className="rounded border p-2">
              <summary className="break-all text-sm">${evidence.evidence_id} · ${evidence.checksum}</summary>
              <pre className="mt-2 max-h-96 overflow-auto whitespace-pre-wrap break-all text-xs">${evidence.evidence_bytes}</pre>
            </details>
          `)}
          <label className="block text-sm">${t("associationReview.meaning")}
            <textarea className="mt-1 w-full rounded border p-2 bg-transparent" value=${note}
              maxLength=${16384} disabled=${locked} onChange=${e => setNote(e.target.value)} />
          </label>
          ${noteTooLarge && html`<p role="alert" className="text-sm text-[var(--v2-danger-text)]">${t("associationReview.noteTooLarge")}</p>`}
          <button type="button" className="rounded border px-3 py-2 text-sm"
            disabled=${busy || Boolean(receipt) || !note.trim() || noteTooLarge} onClick=${approve}>
            ${t(decision ? "associationReview.retry" : "associationReview.approve")}
          </button>
        `}
        ${error && html`<p role="alert" className="text-sm text-[var(--v2-danger-text)]">${error}</p>`}
        ${receipt && html`
          <p role="status" className="break-all text-sm">${t("associationReview.recorded", { id: receipt.approval_id })}</p>
          <button type="button" className="rounded border px-3 py-2 text-sm" onClick=${startAnother}>${t("associationReview.another")}</button>
        `}
      </div>
    <//>
  `;
}
