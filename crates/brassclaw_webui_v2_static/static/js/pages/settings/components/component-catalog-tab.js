import { React, html } from "../../../lib/html.js";
import { Card } from "../../../design-system/card.js";
import { Badge } from "../../../design-system/badge.js";
import { useT } from "../../../lib/i18n.js";
import { useQuery } from "@tanstack/react-query";
import { matchesSearch } from "../lib/settings-search.js";
import { SettingsSearchEmpty } from "./settings-search-empty.js";
import { ComponentDetailPane } from "./component-detail-pane.js";
import { ConsumerTags, statusTone } from "./component-detail-primitives.js";

/**
 * Shared list shell for every Component Catalog tab.
 *
 * A tab supplies its endpoint (`queryKey`/`queryFn`), the detail path segment
 * (`componentType`) and its i18n namespace; the shell owns loading, error,
 * empty, search and row expansion, and delegates the expanded body to the
 * class-aware `ComponentDetailPane`.
 */
export function ComponentCatalogTab({
  searchQuery = "",
  ns,
  queryKey,
  queryFn,
  componentType,
  classCode,
  filterItem,
  renderRowBadges,
  renderSummary,
  skeletonRows = 3,
}) {
  const t = useT();
  const query = useQuery({ queryKey, queryFn });

  if (query.isLoading) {
    return html`<${CatalogSkeleton} rows=${skeletonRows} />`;
  }

  if (query.isError) {
    return html`
      <${Card} padding="md">
        <p className="text-sm text-[var(--v2-danger-text)]">
          ${t(`${ns}.failedLoad`, {
            message: query.error?.message ?? String(query.error),
          })}
        </p>
      <//>
    `;
  }

  const all = query.data?.items ?? [];
  const items = filterItem ? all.filter(filterItem) : all;
  const filtered = items.filter((item) =>
    matchesSearch(searchQuery, [
      item.name,
      item.description,
      item.validation_status,
      item.consumer_tags,
    ])
  );

  if (items.length === 0) {
    return html`
      <${Card} padding="lg">
        <h3 className="text-lg font-semibold text-[var(--v2-text-strong)]">${t(`${ns}.none`)}</h3>
        <p className="mt-2 max-w-md text-sm leading-6 text-[var(--v2-text-muted)]">
          ${t(`${ns}.noneDesc`)}
        </p>
      <//>
    `;
  }

  if (filtered.length === 0) {
    return html`<${SettingsSearchEmpty} query=${searchQuery} />`;
  }

  return html`
    <div className="space-y-4">
      ${renderSummary ? renderSummary(items) : null}
      <${Card} padding="md">
        <h3 className="mb-4 font-mono text-[11px] uppercase tracking-[0.14em] text-[var(--v2-accent-text)]">
          ${t(`${ns}.library`)}
        </h3>
        ${filtered.map(
          (item) => html`
            <${CatalogRow}
              key=${item.id}
              item=${item}
              componentType=${componentType}
              classCode=${item.class_code ?? classCode}
              renderRowBadges=${renderRowBadges}
            />
          `
        )}
      <//>
    </div>
  `;
}

function CatalogRow({ item, componentType, classCode, renderRowBadges }) {
  const t = useT();
  const [expanded, setExpanded] = React.useState(false);
  const toggle = () => setExpanded((value) => !value);

  return html`
    <div className="border-t border-[var(--v2-panel-border)] py-4 first:border-0">
      <div
        role="button"
        tabIndex=${0}
        onClick=${toggle}
        onKeyDown=${(event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            toggle();
          }
        }}
        className="flex w-full cursor-pointer items-start justify-between gap-4 text-left"
      >
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="font-mono text-sm font-semibold text-[var(--v2-text-strong)]">
              ${item.name}
            </span>
            ${item.version &&
            html`<span className="text-xs text-[var(--v2-text-muted)]">v${item.version}</span>`}
            ${renderRowBadges ? renderRowBadges(item) : null}
          </div>
          ${item.description &&
          html`<p className="mt-0.5 truncate text-xs text-[var(--v2-text-muted)]">
            ${item.description}
          </p>`}
          ${!expanded &&
          html`<div className="mt-1.5"><${ConsumerTags} tags=${item.consumer_tags} /></div>`}
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <${Badge}
            tone=${statusTone(item.validation_status)}
            label=${item.validation_status ?? t("common.unknown")}
            size="sm"
          />
          <span className="rounded px-2 py-1 text-xs font-medium text-[var(--v2-accent-text)]">
            ${expanded ? "▲" : "▼"}
          </span>
        </div>
      </div>

      ${expanded &&
      html`<div className="mt-3">
        <${ComponentDetailPane}
          componentType=${componentType}
          id=${item.id}
          classCode=${classCode}
        />
      </div>`}
    </div>
  `;
}

function CatalogSkeleton({ rows }) {
  return html`
    <div className="space-y-4">
      <div className="h-4 w-32 animate-pulse rounded bg-[var(--v2-surface-muted)]" />
      ${Array.from({ length: rows }, (_, i) => i).map(
        (i) => html`
          <div
            key=${i}
            className="flex items-center justify-between border-t border-[var(--v2-panel-border)] py-4 first:border-0"
          >
            <div>
              <div className="h-4 w-40 animate-pulse rounded bg-[var(--v2-surface-muted)]" />
              <div className="mt-1 h-3 w-56 animate-pulse rounded bg-[var(--v2-surface-muted)]" />
            </div>
            <div className="h-6 w-20 animate-pulse rounded-full bg-[var(--v2-surface-muted)]" />
          </div>
        `
      )}
    </div>
  `;
}
