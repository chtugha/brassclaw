import { React, html } from "../../../lib/html.js";
import { Card } from "../../../design-system/card.js";
import { useT } from "../../../lib/i18n.js";

/**
 * Reliability tab — Phase 6.
 *
 * Surfaces the runtime reliability configuration: failure-rollback threshold,
 * Q4 retention windows, forensic packet retention, and prior-knowledge token
 * budget. These values are derived from the MontyVM settings payload
 * (max_duration_secs, failure_rollback_threshold, q4_retention_days,
 * forensic_packet_retention_days, prior_knowledge_token_budget) and are
 * edited via the monty-vm PUT endpoint exposed by the Monty VM tab.
 *
 * This tab is read-only and links the operator to the Monty VM tab for
 * mutations — that keeps the write surface unified in one place.
 */
export function ReliabilityTab() {
  const t = useT();

  return html`
    <div className="space-y-4">
      <${Card} padding="md">
        <h3 className="mb-2 font-mono text-[11px] uppercase tracking-[0.14em] text-[var(--v2-accent-text)]">
          ${t("reliability.title")}
        </h3>
        <p className="text-sm text-[var(--v2-text-muted)]">
          ${t("reliability.desc")}
        </p>
      <//>

      <${Card} padding="md">
        <h4 className="mb-3 text-sm font-semibold text-[var(--v2-text-strong)]">
          ${t("reliability.retentionTitle")}
        </h4>
        <dl className="space-y-3">
          <${FieldRow}
            label=${t("reliability.q4RetentionDays")}
            desc=${t("reliability.q4RetentionDaysDesc")}
          />
          <${FieldRow}
            label=${t("reliability.forensicRetentionDays")}
            desc=${t("reliability.forensicRetentionDaysDesc")}
          />
        </dl>
      <//>

      <${Card} padding="md">
        <h4 className="mb-3 text-sm font-semibold text-[var(--v2-text-strong)]">
          ${t("reliability.rollbackTitle")}
        </h4>
        <dl className="space-y-3">
          <${FieldRow}
            label=${t("reliability.failureRollbackThreshold")}
            desc=${t("reliability.failureRollbackThresholdDesc")}
          />
          <${FieldRow}
            label=${t("reliability.priorKnowledgeTokenBudget")}
            desc=${t("reliability.priorKnowledgeTokenBudgetDesc")}
          />
        </dl>
        <p className="mt-4 text-xs text-[var(--v2-text-faint)]">
          ${t("reliability.editHint")}
        </p>
      <//>
    </div>
  `;
}

function FieldRow({ label, desc }) {
  return html`
    <div className="flex items-start justify-between border-t border-[var(--v2-panel-border)] pt-3 first:border-0 first:pt-0">
      <div>
        <dt className="text-sm font-medium text-[var(--v2-text-strong)]">${label}</dt>
        <dd className="mt-0.5 text-xs text-[var(--v2-text-muted)]">${desc}</dd>
      </div>
    </div>
  `;
}
