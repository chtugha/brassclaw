import { React, html } from "../../../lib/html.js";
import { Badge } from "../../../design-system/badge.js";
import { Button } from "../../../design-system/button.js";
import { Card } from "../../../design-system/card.js";
import { useT } from "../../../lib/i18n.js";
import { usePrefixes } from "../hooks/usePrefixes.js";

// ---------------------------------------------------------------------------
// PrefixTab — operator surface for the V3 prefix cache.
// Shows the list of named prefix bundles, their fingerprint and staleness, and
// lets the operator trigger a bundle regeneration for any entry.
// ---------------------------------------------------------------------------

export function PrefixTab() {
  const t = useT();
  const {
    entries,
    isLoading,
    loadError,
    regenerating,
    regenerateError,
    handleRegenerate,
    reload,
  } = usePrefixes();

  if (isLoading) {
    return html`<${PrefixSkeleton} />`;
  }

  if (loadError) {
    return html`
      <div className="rounded-xl border border-red-400/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
        ${t("prefix.failedLoad", { message: loadError.message || String(loadError) })}
      </div>
    `;
  }

  return html`
    <div className="space-y-5">
      ${regenerateError &&
        html`
          <div className="rounded-xl border border-red-400/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
            ${regenerateError}
          </div>
        `}

      <${Card} padding="none" className="p-4 sm:p-5">
        <div className="mb-4 flex items-center justify-between">
          <h3 className="font-mono text-[11px] uppercase tracking-[0.14em] text-[var(--v2-accent-text)]">
            ${t("prefix.title")}
          </h3>
          <${Button} variant="ghost" size="sm" onClick=${reload}>
            ${t("common.refresh")}
          <//>
        </div>
        <p className="mb-4 text-xs text-[var(--v2-text-muted)]">
          ${t("prefix.desc")}
        </p>

        ${!entries || entries.length === 0
          ? html`
              <div className="rounded-md border border-[var(--v2-panel-border)] bg-[var(--v2-surface-soft)] px-4 py-6 text-center text-sm text-[var(--v2-text-muted)]">
                ${t("prefix.empty")}
              </div>
            `
          : html`
              <div className="space-y-3">
                ${entries.map(
                  (entry) =>
                    html`<${PrefixEntryRow}
                      key=${entry.name}
                      entry=${entry}
                      isRegenerating=${regenerating.has(entry.name)}
                      onRegenerate=${handleRegenerate}
                      t=${t}
                    />`
                )}
              </div>
            `}
      <//>
    </div>
  `;
}

// ---------------------------------------------------------------------------
// PrefixEntryRow — one row per named prefix bundle.
// ---------------------------------------------------------------------------

function PrefixEntryRow({ entry, isRegenerating, onRegenerate, t }) {
  const staleTone = entry.is_stale ? "warning" : "positive";
  const staleLabel = entry.is_stale ? t("prefix.stale") : t("prefix.fresh");

  return html`
    <div className="flex items-center justify-between gap-4 rounded-md border border-[var(--v2-panel-border)] bg-[var(--v2-surface-soft)] px-4 py-3">
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="font-mono text-sm font-semibold text-[var(--v2-text-strong)]">
            ${entry.name}
          </span>
          <${Badge} tone=${staleTone} label=${staleLabel} size="sm" />
        </div>
        ${entry.fingerprint &&
          html`
            <div className="mt-1 truncate font-mono text-[11px] text-[var(--v2-text-muted)]">
              ${t("prefix.fingerprint")}: ${entry.fingerprint}
            </div>
          `}
        ${entry.assembled_at &&
          html`
            <div className="mt-0.5 text-[11px] text-[var(--v2-text-muted)]">
              ${t("prefix.assembledAt")}: ${entry.assembled_at}
            </div>
          `}
      </div>
      <${Button}
        variant="secondary"
        size="sm"
        disabled=${isRegenerating}
        onClick=${() => onRegenerate(entry.name)}
      >
        ${isRegenerating ? t("prefix.regenerating") : t("prefix.regenerate")}
      <//>
    </div>
  `;
}

// ---------------------------------------------------------------------------
// Loading skeleton.
// ---------------------------------------------------------------------------

function Skeleton({ className = "" }) {
  return html`
    <div className=${"rounded animate-pulse bg-[var(--v2-surface-muted)] " + className} />
  `;
}

function PrefixSkeleton() {
  return html`
    <div className="space-y-5">
      <${Card} padding="none" className="p-4 sm:p-5">
        <${Skeleton} className="mb-4 h-3 w-24" />
        <${Skeleton} className="mb-4 h-3 w-64" />
        <div className="space-y-3">
          <${Skeleton} className="h-16 w-full" />
          <${Skeleton} className="h-16 w-full" />
        </div>
      <//>
    </div>
  `;
}
