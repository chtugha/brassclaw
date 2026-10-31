import { html } from "../../../lib/html.js";
import { Badge } from "../../../design-system/badge.js";
import { Card } from "../../../design-system/card.js";
import { useT } from "../../../lib/i18n.js";
import { INFERENCE_FIELDS } from "../lib/settings-schema.js";
import { filterSettingsSections, matchesSearch } from "../lib/settings-search.js";
import { ProviderManagement } from "./provider-management.js";
import { SettingsGroup } from "./settings-field.js";
import { SettingsSearchEmpty } from "./settings-search-empty.js";
import { useLlmProviders } from "../hooks/useLlmProviders.js";

export function InferenceTab({
  settings,
  gatewayStatus,
  onSave,
  savedKeys,
  isLoading,
  searchQuery = "",
}) {
  const t = useT();
  const llm = useLlmProviders();
  if (isLoading || llm.isLoading) {
    return html`<${SettingsSkeleton} />`;
  }

  const backend = llm.activeProviderId;
  const model = llm.selectedModel;
  const sections = filterSettingsSections(INFERENCE_FIELDS, settings, searchQuery, t);
  const showProviderSummary = matchesSearch(searchQuery, [
    t("inference.provider"),
    t("inference.backend"),
    backend,
    t("inference.model"),
    model,
  ]);
  const showProviderManagement = matchesSearch(searchQuery, [
    t("llm.providers"),
    t("llm.providersDesc"),
    t("llm.addProvider"),
    "llm",
    "provider",
    "openai",
    "anthropic",
    "ollama",
    "near",
  ]);

  if (!showProviderSummary && !showProviderManagement && sections.length === 0) {
    return html`<${SettingsSearchEmpty} query=${searchQuery} />`;
  }

  return html`
    <div className="space-y-5">
      ${showProviderSummary &&
      html`
      <${Card} padding="none" className="p-4 sm:p-5">
        ${llm.error && html`<p role="alert" className="mb-3 text-sm text-red-200">${t("error.loadFailed", { what: t("llm.providers"), message: llm.error.message })}</p>`}
        <h3 className="mb-4 font-mono text-[11px] uppercase tracking-[0.14em] text-[var(--v2-accent-text)]">${t("inference.provider")}</h3>
        <div className="grid gap-4 sm:grid-cols-2">
          <div className="rounded-md border border-[var(--v2-panel-border)] bg-[var(--v2-surface-soft)] px-4 py-3">
            <div className="text-xs text-[var(--v2-text-muted)]">${t("inference.backend")}</div>
            <div className="mt-1 flex items-center gap-2">
              <span className="font-mono text-lg font-semibold text-[var(--v2-text-strong)]">${backend || "—"}</span>
              ${backend && html`<${Badge} tone="positive" label=${t("inference.selected")} size="sm" />`}
            </div>
          </div>
          <div className="rounded-md border border-[var(--v2-panel-border)] bg-[var(--v2-surface-soft)] px-4 py-3">
            <div className="text-xs text-[var(--v2-text-muted)]">${t("inference.model")}</div>
            <div className="mt-1 font-mono text-lg font-semibold text-[var(--v2-text-strong)]">
              ${model || t("inference.none")}
            </div>
          </div>
        </div>
        <p className="mt-4 text-xs text-[var(--v2-text-muted)]">${t("inference.publicationHelp")}</p>
        <div className="mt-2 space-y-1 text-xs text-[var(--v2-text-muted)]">
          ${["kohai", "sempai"].map((role) => {
            const publication = llm.runtimePublications[role];
            const selection = publication?.state === "configured" ? publication.selection : null;
            return html`<div key=${role}><span className="font-semibold">${role === "kohai" ? "Kohai" : "Sempai"}: </span>${selection
              ? `${selection.provider_id} / ${selection.model} · ${t("inference.definitionRevision")} ${selection.definition_revision}`
              : t(publication?.state === "unconfigured" ? "inference.none" : "inference.publicationUnknown")}</div>`;
          })}
        </div>
      <//>
      `}

      ${showProviderManagement &&
      html`
        <${ProviderManagement}
          settings=${settings}
          gatewayStatus=${gatewayStatus}
          searchQuery=${searchQuery}
        />
      `}

      ${sections.map(
        (section) =>
          html`
            <${SettingsGroup}
              key=${section.groupKey}
              groupKey=${section.groupKey}
              fields=${section.fields}
              settings=${settings}
              onSave=${onSave}
              savedKeys=${savedKeys}
            />
          `
      )}
    </div>
  `;
}

function Skeleton({ className = "" }) {
  return html`
    <div
      className=${"rounded animate-pulse bg-[var(--v2-surface-muted)] " + className}
    />
  `;
}

function SettingsSkeleton() {
  return html`
    <div className="space-y-5">
      <${Card} padding="md">
        <${Skeleton} className="mb-4 h-3 w-24" />
        <div className="grid gap-4 sm:grid-cols-2">
          <div className="rounded-md border border-[var(--v2-panel-border)] bg-[var(--v2-surface-soft)] p-4">
            <${Skeleton} className="h-3 w-16" />
            <${Skeleton} className="mt-2 h-6 w-28" />
          </div>
          <div className="rounded-md border border-[var(--v2-panel-border)] bg-[var(--v2-surface-soft)] p-4">
            <${Skeleton} className="h-3 w-16" />
            <${Skeleton} className="mt-2 h-6 w-40" />
          </div>
        </div>
      <//>
      ${[1, 2].map(
        (i) =>
          html`
            <${Card} key=${i} padding="md">
              <${Skeleton} className="mb-4 h-3 w-20" />
              ${[1, 2, 3].map(
                (j) =>
                  html`
                    <div key=${j} className="flex items-center justify-between border-t border-[var(--v2-panel-border)] py-4 first:border-0">
                      <${Skeleton} className="h-4 w-32" />
                      <${Skeleton} className="h-9 w-36" />
                    </div>
                  `
              )}
            <//>
          `
      )}
    </div>
  `;
}
