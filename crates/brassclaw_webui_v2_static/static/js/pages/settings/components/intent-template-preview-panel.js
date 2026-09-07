// Phase M.6 — self-contained template expression preview tool.
//
// An operator reviewing a component can type or paste an intent expression and
// see the live §0.17.5 authoring feedback: `%` slot chips, anchor
// classification (green / yellow / red), and inline §0.17.4 Q1 errors /
// warnings. Pure client-side (templateFeedback) — no backend round-trip, no
// project context required. The full authoring field with save/upsert awaits
// the recipe/variant editor + project-id threading (future phase); this panel
// delivers the live-feedback half of M.6 against the existing validation-queue
// surface.
import { React, html } from "../../../lib/html.js";
import { useT } from "../../../lib/i18n.js";
import { IntentTemplateFeedback } from "../../../components/intent-template-feedback.js";

export function IntentTemplatePreviewPanel() {
  const t = useT();
  const [expr, setExpr] = React.useState("");

  return html`
    <div className="rounded-lg border border-[var(--v2-panel-border)] bg-[var(--v2-surface)] p-3">
      <div className="mb-1.5 flex items-center gap-2">
        <span className="font-mono text-[11px] uppercase tracking-[0.14em] text-[var(--v2-accent-text)]">
          ${t("validationQueue.templatePreviewTitle", {
            defaultValue: "Intent template preview",
          })}
        </span>
        <span className="text-[11px] text-[var(--v2-text-faint)]">
          ${t("validationQueue.templatePreviewHint", {
            defaultValue: "Type an intent expression — `%` marks a slot.",
          })}
        </span>
      </div>
      <input
        type="text"
        value=${expr}
        onChange=${(e) => setExpr(e.target.value)}
        placeholder=${t("validationQueue.templatePreviewPlaceholder", {
          defaultValue: "e.g. show me files in the % directory",
        })}
        className="w-full rounded border border-[var(--v2-panel-border)] bg-[var(--v2-canvas)] px-2.5 py-1.5 font-mono text-sm text-[var(--v2-text)] outline-none focus:border-[var(--v2-accent-text)]"
      />
      <${IntentTemplateFeedback} expr=${expr} />
    </div>
  `;
}
