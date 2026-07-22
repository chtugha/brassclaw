import { React, html } from "../../../lib/html.js";
import { Card } from "../../../design-system/card.js";
import { Badge } from "../../../design-system/badge.js";
import { useT } from "../../../lib/i18n.js";
import { useQuery } from "@tanstack/react-query";
import { fetchSettingsScaffolds } from "../lib/settings-api.js";
import { matchesSearch } from "../lib/settings-search.js";
import { SettingsSearchEmpty } from "./settings-search-empty.js";

export function ScaffoldTab({ searchQuery = "" }) {
  const t = useT();
  const query = useQuery({
    queryKey: ["settings", "scaffolds"],
    queryFn: fetchSettingsScaffolds,
  });

  if (query.isLoading) {
    return html`
      <div className="space-y-4">
        ${[1, 2].map(
          (i) => html`
            <div key=${i} className="h-16 animate-pulse rounded-lg bg-[var(--v2-surface-muted)]" />
          `
        )}
      </div>
    `;
  }

  if (query.isError) {
    return html`
      <${Card} padding="md">
        <p className="text-sm text-[var(--v2-danger-text)]">
          ${t("scaffold.failedLoad", { message: query.error?.message ?? String(query.error) })}
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
          ${t("scaffold.none")}
        </h3>
        <p className="mt-2 max-w-md text-sm leading-6 text-[var(--v2-text-muted)]">
          ${t("scaffold.noneDesc")}
        </p>
      <//>
    `;
  }

  if (filtered.length === 0) {
    return html`<${SettingsSearchEmpty} query=${searchQuery} />`;
  }

  return html`
    <${Card} padding="md">
      <h3 className="mb-4 font-mono text-[11px] uppercase tracking-[0.14em] text-[var(--v2-accent-text)]">
        ${t("scaffold.library")}
      </h3>
      ${filtered.map(
        (item) => html`
          <div
            key=${item.id}
            className="flex items-start justify-between border-t border-[var(--v2-panel-border)] py-4 first:border-0"
          >
            <div className="flex-1 min-w-0 pr-4">
              <span className="font-mono text-sm font-semibold text-[var(--v2-text-strong)]">
                ${item.name}
              </span>
              ${item.description &&
                html`<p className="mt-0.5 text-xs text-[var(--v2-text-muted)] truncate">
                  ${item.description}
                </p>`}
            </div>
            <${Badge}
              tone=${item.validation_status === "validated" ? "positive" : "neutral"}
              label=${item.validation_status ?? "unknown"}
              size="sm"
            />
          </div>
        `
      )}
    <//>
  `;
}
