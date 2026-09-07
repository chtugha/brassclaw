// Phase M.6 — live template authoring feedback for a single intent expression.
//
// Renders the §0.17.5 authoring surface: the expression with `%` slot markers
// shown as styled chips, an anchor-classification line (green / yellow / red),
// and inline §0.17.4 Q1 errors / warnings. Pure presentational — the feedback
// is computed client-side by `templateFeedback` (no round-trip), so it updates
// live as the operator types.
import { html } from "../lib/html.js";
import { templateFeedback } from "../lib/template-feedback.js";

const CLASSIFICATION_TONE = {
  plain: { label: "Exact-match intent", className: "text-[var(--v2-text-muted)]" },
  anchored: { label: "Anchored", className: "text-[var(--v2-accent-text)]" },
  "suffix-only": { label: "Suffix anchor only", className: "text-amber-300" },
  "no-anchor": { label: "No anchor — blocked", className: "text-[var(--v2-danger-text)]" },
};

// Split the expression into literal segments and `%` chips so the author can
// see at a glance which parts are slots.
function renderWithChips(expr) {
  const segments = String(expr).split("%");
  const nodes = [];
  for (let i = 0; i < segments.length; i += 1) {
    if (segments[i] !== "") {
      nodes.push(
        html`<span className="text-[var(--v2-text)]">${segments[i]}</span>`
      );
    }
    if (i < segments.length - 1) {
      nodes.push(
        html`<span
          className="mx-0.5 inline-flex items-center rounded bg-[var(--v2-accent-bg)] px-1.5 py-0.5 font-mono text-[11px] font-semibold text-[var(--v2-accent-text)]"
          title="Slot marker — captures a value from the user text"
        >
          %
        </span>`
      );
    }
  }
  return nodes;
}

export function IntentTemplateFeedback({ expr }) {
  const fb = templateFeedback(expr);
  const tone = CLASSIFICATION_TONE[fb.classification] ?? CLASSIFICATION_TONE.plain;

  // Suppress the empty-expression render — nothing to feedback on yet.
  if (expr === "" || expr == null) {
    return null;
  }

  return html`
    <div className="mt-2 space-y-1.5">
      <div
        className="flex flex-wrap items-center gap-1.5 rounded border border-[var(--v2-panel-border)] bg-[var(--v2-surface-muted)] px-2.5 py-1.5 font-mono text-xs"
      >
        ${renderWithChips(expr)}
      </div>
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px]">
        <span className=${`font-medium ${tone.className}`}>
          ${tone.label}
        </span>
        ${fb.slotCount > 0 &&
        html`<span className="text-[var(--v2-text-faint)]">
          ${fb.slotCount} slot${fb.slotCount === 1 ? "" : "s"}
        </span>`}
        ${fb.prefix !== "" &&
        html`<span className="text-[var(--v2-text-muted)]">
          prefix: <code className="text-[var(--v2-text)]">${fb.prefix}</code>
        </span>`}
        ${fb.suffix !== "" &&
        html`<span className="text-[var(--v2-text-muted)]">
          suffix: <code className="text-[var(--v2-text)]">${fb.suffix}</code>
        </span>`}
      </div>
      ${fb.errors.map(
        (err) =>
          html`<p
            key=${err}
            className="text-[11px] leading-5 text-[var(--v2-danger-text)]"
          >
            ${err}
          </p>`
      )}
      ${fb.warnings.map(
        (warn) =>
          html`<p key=${warn} className="text-[11px] leading-5 text-amber-300">
            ${warn}
          </p>`
      )}
    </div>
  `;
}
