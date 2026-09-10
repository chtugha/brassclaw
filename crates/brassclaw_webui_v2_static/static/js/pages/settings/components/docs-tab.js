import { React, html } from "../../../lib/html.js";
import { Card } from "../../../design-system/card.js";
import { Badge } from "../../../design-system/badge.js";
import { useT } from "../../../lib/i18n.js";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { fetchDocus, updateDocusContent } from "../lib/settings-api.js";
import { matchesSearch } from "../lib/settings-search.js";
import { SettingsSearchEmpty } from "./settings-search-empty.js";

export function DocsTab({ searchQuery = "" }) {
  const t = useT();
  const query = useQuery({
    queryKey: ["settings", "docus"],
    queryFn: fetchDocus,
  });

  if (query.isLoading) {
    return html`<${DocsTabSkeleton} />`;
  }

  if (query.isError) {
    return html`
      <${Card} padding="md">
        <p className="text-sm text-[var(--v2-danger-text)]">
          ${t("settings.docs.failedLoad", { message: query.error?.message })}
        </p>
      <//>
    `;
  }

  const items = query.data?.items ?? [];
  const filtered = items.filter((item) =>
    matchesSearch(searchQuery, [item.name, item.description, item.source, item.validation_status])
  );

  if (items.length === 0) {
    return html`
      <${Card} padding="lg">
        <h3 className="text-lg font-semibold text-[var(--v2-text-strong)]">
          ${t("settings.docs.empty")}
        </h3>
      <//>
    `;
  }

  if (filtered.length === 0) {
    return html`<${SettingsSearchEmpty} query=${searchQuery} />`;
  }

  return html`
    <div className="space-y-4">
      <${Card} padding="md">
        <div className="mb-4">
          <h3 className="font-mono text-[11px] uppercase tracking-[0.14em] text-[var(--v2-accent-text)]">
            ${t("settings.docs.title")}
          </h3>
        </div>
        ${filtered.map(
          (item) => html`<${DocRow} key=${item.id} item=${item} />`
        )}
      <//>
    </div>
  `;
}

function DocRow({ item }) {
  const t = useT();
  const queryClient = useQueryClient();
  const [expanded, setExpanded] = React.useState(false);
  const [draft, setDraft] = React.useState(item.content ?? "");
  const [saveError, setSaveError] = React.useState("");

  // Reset draft when item changes externally (e.g. after refetch).
  React.useEffect(() => {
    setDraft(item.content ?? "");
  }, [item.id, item.updated_at]);

  const saveMutation = useMutation({
    mutationFn: () => updateDocusContent(item.id, draft),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings", "docus"] });
      setSaveError("");
      setExpanded(false);
    },
    onError: (err) => {
      setSaveError(
        t("settings.docs.saveError", { message: err?.message ?? "Unknown error" })
      );
    },
  });

  const statusTone =
    item.validation_status === "validated"
      ? "positive"
      : item.validation_status === "rejected"
      ? "negative"
      : "neutral";

  return html`
    <div
      className="border-t border-[var(--v2-panel-border)] py-4 first:border-0"
    >
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 flex-wrap">
            <span className="font-mono text-sm font-semibold text-[var(--v2-text-strong)]">
              ${item.name}
            </span>
            <span
              className="text-xs text-[var(--v2-text-faint)]"
              title=${t("settings.docs.source_label")}
            >
              ${item.source}
            </span>
          </div>
          ${item.description &&
            html`<p className="mt-0.5 text-xs text-[var(--v2-text-muted)] truncate">
              ${item.description}
            </p>`}
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <${Badge}
            tone=${statusTone}
            label=${item.validation_status ?? "unknown"}
            size="sm"
          />
          <button
            onClick=${() => setExpanded((v) => !v)}
            className="rounded px-2 py-1 text-xs font-medium text-[var(--v2-accent-text)] hover:bg-[var(--v2-accent-bg)]"
          >
            ${expanded ? "▲" : "▼"}
          </button>
        </div>
      </div>

      ${expanded && html`
        <div className="mt-3 space-y-2">
          <textarea
            className="w-full rounded border border-[var(--v2-panel-border)] bg-[var(--v2-surface)] px-3 py-2 font-mono text-xs text-[var(--v2-text-strong)] focus:outline-none focus:ring-1 focus:ring-[var(--v2-accent-border)] resize-y"
            rows="12"
            value=${draft}
            onInput=${(e) => setDraft(e.target.value)}
          />
          ${saveError && html`<p className="text-xs text-[var(--v2-danger-text)]">${saveError}</p>`}
          <div className="flex gap-2">
            <button
              onClick=${() => saveMutation.mutate()}
              disabled=${saveMutation.isPending}
              className="rounded px-3 py-1.5 text-xs font-medium bg-[var(--v2-accent-bg)] text-[var(--v2-accent-text)] hover:opacity-90 disabled:opacity-40 disabled:cursor-not-allowed"
            >
              ${saveMutation.isPending
                ? t("common.saving")
                : t("settings.docs.save")}
            </button>
            <button
              onClick=${() => { setExpanded(false); setDraft(item.content ?? ""); setSaveError(""); }}
              disabled=${saveMutation.isPending}
              className="rounded px-3 py-1.5 text-xs font-medium text-[var(--v2-text-muted)] hover:bg-[var(--v2-surface-muted)] disabled:opacity-40"
            >
              ${t("common.cancel")}
            </button>
          </div>
        </div>
      `}
    </div>
  `;
}

function DocsTabSkeleton() {
  return html`
    <div className="space-y-4">
      <div className="h-4 w-32 animate-pulse rounded bg-[var(--v2-surface-muted)]" />
      ${[1, 2, 3].map(
        (i) => html`
          <div key=${i} className="flex items-center justify-between border-t border-[var(--v2-panel-border)] py-4 first:border-0">
            <div>
              <div className="h-4 w-48 animate-pulse rounded bg-[var(--v2-surface-muted)]" />
              <div className="mt-1 h-3 w-64 animate-pulse rounded bg-[var(--v2-surface-muted)]" />
            </div>
            <div className="h-6 w-20 animate-pulse rounded-full bg-[var(--v2-surface-muted)]" />
          </div>
        `
      )}
    </div>
  `;
}
