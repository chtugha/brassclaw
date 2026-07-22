import { React, html } from "../../../lib/html.js";
import { Card } from "../../../design-system/card.js";
import { Button } from "../../../design-system/button.js";
import { Badge } from "../../../design-system/badge.js";
import { useT } from "../../../lib/i18n.js";
import { useQuery } from "@tanstack/react-query";
import { fetchSettingsActions } from "../lib/settings-api.js";
import { matchesSearch } from "../lib/settings-search.js";
import { SettingsSearchEmpty } from "./settings-search-empty.js";

export function ActionsTab({ searchQuery = "" }) {
  const t = useT();
  const query = useQuery({
    queryKey: ["settings", "actions"],
    queryFn: fetchSettingsActions,
  });

  if (query.isLoading) {
    return html`<${ActionsSkeleton} />`;
  }

  if (query.isError) {
    return html`
      <${Card} padding="md">
        <p className="text-sm text-[var(--v2-danger-text)]">
          ${t("actions.failedLoad", { message: query.error?.message })}
        </p>
      <//>
    `;
  }

  const items = query.data?.items ?? [];
  const filtered = items.filter((item) =>
    matchesSearch(searchQuery, [item.name, item.description, item.validation_status])
  );

  if (items.length === 0) {
    return html`
      <${Card} padding="lg">
        <h3 className="text-lg font-semibold text-[var(--v2-text-strong)]">
          ${t("actions.none")}
        </h3>
        <p className="mt-2 max-w-md text-sm leading-6 text-[var(--v2-text-muted)]">
          ${t("actions.noneDesc")}
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
        <h3 className="mb-4 font-mono text-[11px] uppercase tracking-[0.14em] text-[var(--v2-accent-text)]">
          ${t("actions.library")}
        </h3>
        ${filtered.map(
          (item) => html`
            <${ComponentRow} key=${item.id} item=${item} />
          `
        )}
      <//>
    </div>
  `;
}

function ComponentRow({ item }) {
  const t = useT();
  const isInQueue = item.consumer_tags?.includes("05:validator");
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
          ${item.version &&
            html`<span className="text-xs text-[var(--v2-text-muted)]">v${item.version}</span>`}
        </div>
        ${item.description &&
          html`<p className="mt-0.5 text-xs text-[var(--v2-text-muted)] truncate">
            ${item.description}
          </p>`}
        <div className="mt-1.5 flex flex-wrap gap-1">
          ${(item.consumer_tags ?? []).map(
            (tag) => html`
              <${TagChip} key=${tag} tag=${tag} greyed=${isInQueue && tag !== "05:validator"} />
            `
          )}
        </div>
      </div>
      <${Badge}
        tone=${statusTone}
        label=${item.validation_status ?? "unknown"}
        size="sm"
      />
    </div>
  `;
}

/**
 * Tag chip with greyed-out rendering per spec §3.9:
 * While a component carries `05:validator` (in queue), non-validator chips
 * render greyed but are shown so operators can see the target audience.
 */
function TagChip({ tag, greyed }) {
  const baseClass =
    "inline-flex items-center rounded px-1.5 py-0.5 font-mono text-[10px] border";
  const style = greyed
    ? `${baseClass} border-[var(--v2-panel-border)] bg-[var(--v2-surface-soft)] text-[var(--v2-text-faint)] opacity-50`
    : tag === "05:validator"
    ? `${baseClass} border-amber-400/40 bg-amber-500/10 text-amber-300`
    : `${baseClass} border-[var(--v2-panel-border)] bg-[var(--v2-surface-soft)] text-[var(--v2-text-muted)]`;
  return html`<span className=${style}>${tag}</span>`;
}

function ActionsSkeleton() {
  return html`
    <div className="space-y-4">
      <div className="h-4 w-32 animate-pulse rounded bg-[var(--v2-surface-muted)]" />
      ${[1, 2, 3].map(
        (i) => html`
          <div key=${i} className="flex items-center justify-between border-t border-[var(--v2-panel-border)] py-4 first:border-0">
            <div>
              <div className="h-4 w-32 animate-pulse rounded bg-[var(--v2-surface-muted)]" />
              <div className="mt-1 h-3 w-48 animate-pulse rounded bg-[var(--v2-surface-muted)]" />
            </div>
            <div className="h-6 w-20 animate-pulse rounded-full bg-[var(--v2-surface-muted)]" />
          </div>
        `
      )}
    </div>
  `;
}
