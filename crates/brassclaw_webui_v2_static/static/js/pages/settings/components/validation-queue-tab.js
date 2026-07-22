import { React, html } from "../../../lib/html.js";
import { Card } from "../../../design-system/card.js";
import { Badge } from "../../../design-system/badge.js";
import { useT } from "../../../lib/i18n.js";
import { useQuery } from "@tanstack/react-query";
import { fetchValidationQueue, fetchValidationQueueCount } from "../lib/settings-api.js";
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

function QueueRow({ item }) {
  const statusTone =
    item.validation_status === "validated"
      ? "positive"
      : item.validation_status === "rejected"
      ? "negative"
      : "neutral";

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
      </div>
      <${Badge}
        tone=${statusTone}
        label=${item.validation_status ?? "unknown"}
        size="sm"
      />
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
