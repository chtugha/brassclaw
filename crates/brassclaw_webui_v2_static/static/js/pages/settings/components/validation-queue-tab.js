import { React, html } from "../../../lib/html.js";
import { Card } from "../../../design-system/card.js";
import { Badge } from "../../../design-system/badge.js";
import { useT } from "../../../lib/i18n.js";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  fetchValidationQueue,
  fetchValidationQueueCount,
  validateComponent,
  rejectComponent,
} from "../lib/settings-api.js";
import { matchesSearch } from "../lib/settings-search.js";
import { SettingsSearchEmpty } from "./settings-search-empty.js";

export function ValidationQueueTab({ searchQuery = "" }) {
  const t = useT();
  const countQuery = useQuery({
    queryKey: ["settings", "validation-queue", "count"],
    queryFn: fetchValidationQueueCount,
  });
  const query = useQuery({
    queryKey: ["settings", "validation-queue"],
    queryFn: fetchValidationQueue,
  });

  if (query.isLoading) {
    return html`<${ValidationQueueSkeleton} />`;
  }

  if (query.isError) {
    return html`
      <${Card} padding="md">
        <p className="text-sm text-[var(--v2-danger-text)]">
          ${t("validationQueue.failedLoad", { message: query.error?.message })}
        </p>
      <//>
    `;
  }

  const items = query.data?.items ?? [];
  const filtered = items.filter((item) =>
    matchesSearch(searchQuery, [item.name, item.description, item.class_label, item.validation_status])
  );

  const pendingCount =
    countQuery.data?.count ?? items.filter((i) => i.validation_status === "pending").length;

  if (items.length === 0) {
    return html`
      <${Card} padding="lg">
        <h3 className="text-lg font-semibold text-[var(--v2-text-strong)]">
          ${t("validationQueue.empty")}
        </h3>
        <p className="mt-2 max-w-md text-sm leading-6 text-[var(--v2-text-muted)]">
          ${t("validationQueue.emptyDesc")}
        </p>
      <//>
    `;
  }

  if (filtered.length === 0) {
    return html`<${SettingsSearchEmpty} query=${searchQuery} />`;
  }

  return html`
    <div className="space-y-4">
      <${Card} padding="md">
        <div className="mb-4 flex items-center justify-between">
          <h3 className="font-mono text-[11px] uppercase tracking-[0.14em] text-[var(--v2-accent-text)]">
            ${t("validationQueue.title")}
          </h3>
          ${pendingCount > 0 &&
            html`<span className="inline-flex items-center rounded-full bg-amber-500/15 px-2 py-0.5 text-xs font-medium text-amber-300">
              ${t("validationQueue.pendingCount", { count: pendingCount })}
            </span>`}
        </div>
        ${filtered.map(
          (item) => html`<${QueueRow} key=${item.id} item=${item} />`
        )}
      <//>
    </div>
  `;
}

// LLM-auditable class codes. For these classes the Validate button is
// disabled until the backend audit returns "clean" (spec §3.5 / §3.4).
const LLM_AUDIT_CLASS_CODES = new Set([10, 50]);

function QueueRow({ item }) {
  const t = useT();
  const queryClient = useQueryClient();
  const [actionError, setActionError] = React.useState("");

  const validateMutation = useMutation({
    mutationFn: () => validateComponent(item.class_code, item.id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings", "validation-queue"] });
      queryClient.invalidateQueries({ queryKey: ["settings", "validation-queue", "count"] });
    },
    onError: (err) => {
      setActionError(t("validationQueue.validateError", { message: err.message }));
    },
  });

  const rejectMutation = useMutation({
    mutationFn: () => rejectComponent(item.class_code, item.id, null),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings", "validation-queue"] });
      queryClient.invalidateQueries({ queryKey: ["settings", "validation-queue", "count"] });
    },
    onError: (err) => {
      setActionError(t("validationQueue.rejectError", { message: err.message }));
    },
  });

  const statusTone =
    item.validation_status === "validated"
      ? "positive"
      : item.validation_status === "rejected"
      ? "negative"
      : "neutral";

  // The Validate button is only shown for Q2 (manual review) items.
  // For class 10 (Orchestrator) and 50 (Scaffold) it is additionally
  // disabled when the LLM audit is pending or has flagged issues — the
  // backend enforces the same rule and returns 403 if bypassed.
  const isQ2 = item.queue_code === "q2_manual";
  const auditBlocked =
    LLM_AUDIT_CLASS_CODES.has(item.class_code) &&
    item.llm_audit_status !== "clean" &&
    item.llm_audit_status !== "not_applicable" &&
    item.llm_audit_status !== "error";
  const validateTooltip = auditBlocked
    ? item.llm_audit_status === "pending"
      ? t("validationQueue.auditPending")
      : t("validationQueue.auditFlagged")
    : null;
  const isBusy = validateMutation.isPending || rejectMutation.isPending;

  return html`
    <div
      className="flex items-start justify-between border-t border-[var(--v2-panel-border)] py-4 first:border-0"
    >
      <div className="flex-1 min-w-0 pr-4">
        <div className="flex items-center gap-2 flex-wrap">
          <span className="font-mono text-sm font-semibold text-[var(--v2-text-strong)]">
            ${item.name}
          </span>
          ${item.class_label &&
            html`<span className="text-xs text-[var(--v2-text-faint)]">${item.class_label}</span>`}
        </div>
        ${item.description &&
          html`<p className="mt-0.5 text-xs text-[var(--v2-text-muted)] truncate">
            ${item.description}
          </p>`}
        ${actionError &&
          html`<p className="mt-1 text-xs text-[var(--v2-danger-text)]">${actionError}</p>`}
      </div>
      <div className="flex items-center gap-2 shrink-0">
        <${Badge}
          tone=${statusTone}
          label=${item.validation_status ?? "unknown"}
          size="sm"
        />
        ${isQ2 && html`
          <button
            onClick=${() => { setActionError(""); rejectMutation.mutate(); }}
            disabled=${isBusy}
            className="rounded px-2 py-1 text-xs font-medium text-[var(--v2-danger-text)] hover:bg-[var(--v2-danger-bg)] disabled:opacity-40 disabled:cursor-not-allowed"
          >
            ${t("validationQueue.reject")}
          </button>
          <button
            onClick=${() => { setActionError(""); validateMutation.mutate(); }}
            disabled=${isBusy || auditBlocked}
            title=${validateTooltip ?? ""}
            className="rounded px-2 py-1 text-xs font-medium text-[var(--v2-accent-text)] hover:bg-[var(--v2-accent-bg)] disabled:opacity-40 disabled:cursor-not-allowed"
          >
            ${t("validationQueue.validate")}
          </button>
        `}
      </div>
    </div>
  `;
}

function ValidationQueueSkeleton() {
  return html`
    <div className="space-y-4">
      <div className="h-4 w-40 animate-pulse rounded bg-[var(--v2-surface-muted)]" />
      ${[1, 2, 3, 4].map(
        (i) => html`
          <div key=${i} className="flex items-center justify-between border-t border-[var(--v2-panel-border)] py-4 first:border-0">
            <div>
              <div className="h-4 w-36 animate-pulse rounded bg-[var(--v2-surface-muted)]" />
              <div className="mt-1 h-3 w-52 animate-pulse rounded bg-[var(--v2-surface-muted)]" />
            </div>
            <div className="h-6 w-20 animate-pulse rounded-full bg-[var(--v2-surface-muted)]" />
          </div>
        `
      )}
    </div>
  `;
}
