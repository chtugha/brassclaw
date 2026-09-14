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
            ${regenerateError === "rate_limited_cooldown"
              ? t("prefix.rateLimitError")
              : t("prefix.regenerateError", { message: regenerateError })}
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
// Helpers for human-readable time display.
// ---------------------------------------------------------------------------

/** Return whole days elapsed since an ISO-8601 timestamp string, or null. */
function daysAgo(isoString) {
  if (!isoString) return null;
  const then = new Date(isoString);
  if (isNaN(then.getTime())) return null;
  return Math.floor((Date.now() - then.getTime()) / 86_400_000);
}

/** Format generation_ms (integer ms) as a rounded-minute string, or null. */
function fmtGenerationTime(ms, t) {
  if (ms == null) return null;
  const minutes = Math.round(ms / 60_000);
  if (minutes < 1) return t("prefix.generationTimeSub1Min");
  return t("prefix.generationTime", { minutes });
}

/** Format days-ago label using the right singular/plural key. */
function fmtGeneratedAgo(days, t) {
  if (days === 0) return t("prefix.generatedToday");
  if (days === 1) return t("prefix.generatedAgo", { days });
  return t("prefix.generatedAgoDays", { days });
}

// ---------------------------------------------------------------------------
// PrefixEntryRow — one row per named prefix bundle.
// ---------------------------------------------------------------------------

function PrefixEntryRow({ entry, isRegenerating, onRegenerate, t }) {
  // An entry that has never been generated has no fingerprint or assembled_at.
  const neverGenerated = !entry.assembled_at && !entry.fingerprint;
  const staleTone = neverGenerated ? "muted" : entry.is_stale ? "warning" : "positive";
  const staleLabel = neverGenerated
    ? t("prefix.neverGenerated")
    : entry.is_stale
    ? t("prefix.stale")
    : t("prefix.fresh");

  // Label changes based on whether this is the first generation.
  const actionLabel = neverGenerated
    ? isRegenerating
      ? t("prefix.generating")
      : t("prefix.generate")
    : isRegenerating
    ? t("prefix.regenerating")
    : t("prefix.regenerate");

  const days = daysAgo(entry.assembled_at);
  const generatedAgoLabel = days != null ? fmtGeneratedAgo(days, t) : null;
  const generationTimeLabel = fmtGenerationTime(entry.generation_ms, t);

  return html`
    <div className="flex items-center justify-between gap-4 rounded-md border border-[var(--v2-panel-border)] bg-[var(--v2-surface-soft)] px-4 py-3">
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="font-mono text-sm font-semibold text-[var(--v2-text-strong)]">
            ${entry.name}
          </span>
          <${Badge} tone=${staleTone} label=${staleLabel} size="sm" />
          ${isRegenerating &&
            html`<span
              className="inline-block h-3 w-3 animate-spin rounded-full border-2 border-[var(--v2-accent-text)] border-t-transparent"
              aria-label="Generating"
            />`}
        </div>
        ${entry.fingerprint &&
          html`
            <div className="mt-1 truncate font-mono text-[11px] text-[var(--v2-text-muted)]">
              ${t("prefix.fingerprint")}: ${entry.fingerprint}
            </div>
          `}
        ${(generatedAgoLabel || generationTimeLabel) &&
          html`
            <div className="mt-1 flex gap-3 text-[11px] text-[var(--v2-text-muted)]">
              ${generatedAgoLabel && html`<span>${generatedAgoLabel}</span>`}
              ${generationTimeLabel && html`<span>${generationTimeLabel}</span>`}
            </div>
          `}
      </div>
      <${Button}
        variant="secondary"
        size="sm"
        disabled=${isRegenerating}
        onClick=${() => onRegenerate(entry.name)}
      >
        ${actionLabel}
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
