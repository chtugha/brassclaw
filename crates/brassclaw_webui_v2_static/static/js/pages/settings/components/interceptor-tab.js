import { React, html } from "../../../lib/html.js";
import { Badge } from "../../../design-system/badge.js";
import { Button } from "../../../design-system/button.js";
import { Card } from "../../../design-system/card.js";
import { useT } from "../../../lib/i18n.js";
import { useInterceptor } from "../hooks/useInterceptor.js";

// ---------------------------------------------------------------------------
// InterceptorTab — top-level component.
// ---------------------------------------------------------------------------

export function InterceptorTab({ searchQuery = "" }) {
  const t = useT();
  const {
    config,
    isLoading,
    loadError,
    isMutating,
    mutationError,
    actionStatus,
    handleUpdate,
    handleReassemble,
    handlePrewarm,
  } = useInterceptor();

  if (isLoading) {
    return html`<${InterceptorSkeleton} />`;
  }

  if (loadError) {
    return html`
      <div className="rounded-xl border border-red-400/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
        ${t("interceptor.failedLoad", { message: loadError.message || String(loadError) })}
      </div>
    `;
  }

  return html`
    <div className="space-y-5">
      ${mutationError &&
        html`
          <div className="rounded-xl border border-red-400/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
            ${mutationError}
          </div>
        `}

      <${StatusCard} config=${config} t=${t} />

      <${PersonaCard}
        config=${config}
        isMutating=${isMutating}
        onUpdate=${handleUpdate}
        t=${t}
      />

      <${ControlCard}
        config=${config}
        isMutating=${isMutating}
        actionStatus=${actionStatus}
        onReassemble=${handleReassemble}
        onPrewarm=${handlePrewarm}
        t=${t}
      />
    </div>
  `;
}

// ---------------------------------------------------------------------------
// StatusCard — mode, connection, last-assembled, pre-warm timestamp.
// ---------------------------------------------------------------------------

function StatusCard({ config, t }) {
  if (!config) return null;
  const modeLabel =
    config.mode === "rerouting"
      ? t("interceptor.mode.rerouting")
      : t("interceptor.mode.routing");
  const modeTone = config.mode === "rerouting" ? "positive" : "neutral";

  return html`
    <${Card} padding="none" className="p-4 sm:p-5">
      <h3 className="mb-4 font-mono text-[11px] uppercase tracking-[0.14em] text-[var(--v2-accent-text)]">
        ${t("interceptor.status")}
      </h3>
      <div className="grid gap-4 sm:grid-cols-2">
        <div className="rounded-md border border-[var(--v2-panel-border)] bg-[var(--v2-surface-soft)] px-4 py-3">
          <div className="text-xs text-[var(--v2-text-muted)]">${t("interceptor.modeLabel")}</div>
          <div className="mt-1 flex items-center gap-2">
            <span className="font-mono text-lg font-semibold text-[var(--v2-text-strong)]">
              ${modeLabel}
            </span>
            <${Badge} tone=${modeTone} label=${config.sempai_connected ? t("interceptor.connected") : t("interceptor.disconnected")} size="sm" />
          </div>
        </div>
        <div className="rounded-md border border-[var(--v2-panel-border)] bg-[var(--v2-surface-soft)] px-4 py-3">
          <div className="text-xs text-[var(--v2-text-muted)]">${t("interceptor.basePromptLabel")}</div>
          <div className="mt-1 text-sm text-[var(--v2-text-strong)]">
            ${config.base_prompt_assembled_at
              ? html`
                  <span>${new Date(config.base_prompt_assembled_at).toLocaleString()}</span>
                  ${config.base_prompt_size_chars != null &&
                    html`<span className="ml-2 text-xs text-[var(--v2-text-muted)]">(${config.base_prompt_size_chars} ${t("interceptor.chars")})</span>`}
                `
              : html`<span className="text-[var(--v2-text-muted)]">${t("interceptor.neverAssembled")}</span>`}
          </div>
          ${config.components_since_rebuild != null && config.components_since_rebuild > 0 &&
            html`
              <div className="mt-2 flex items-center gap-1 text-xs text-amber-400">
                <${Badge} tone="warning" label=${t("interceptor.componentsBadge", { count: config.components_since_rebuild })} size="sm" />
                <span>${t("interceptor.componentsBadgeHint")}</span>
              </div>
            `}
        </div>
        ${config.prewarm_last_at &&
          html`
            <div className="rounded-md border border-[var(--v2-panel-border)] bg-[var(--v2-surface-soft)] px-4 py-3">
              <div className="text-xs text-[var(--v2-text-muted)]">${t("interceptor.prewarmLastAt")}</div>
              <div className="mt-1 text-sm text-[var(--v2-text-strong)]">
                ${new Date(config.prewarm_last_at).toLocaleString()}
              </div>
            </div>
          `}
      </div>
    <//>
  `;
}

// ---------------------------------------------------------------------------
// PersonaCard — textarea for editing the Sempai persona (Part B).
// ---------------------------------------------------------------------------

function PersonaCard({ config, isMutating, onUpdate, t }) {
  const [draft, setDraft] = React.useState(config ? config.persona : "");
  const [saved, setSaved] = React.useState(false);

  // Sync draft if config changes from the outside (e.g., after reassemble).
  React.useEffect(() => {
    if (config) setDraft(config.persona);
  }, [config]);

  const isDirty = config && draft !== config.persona;

  const handleSave = React.useCallback(async () => {
    await onUpdate(draft);
    setSaved(true);
    setTimeout(() => setSaved(false), 2500);
  }, [draft, onUpdate]);

  return html`
    <${Card} padding="none" className="p-4 sm:p-5">
      <h3 className="mb-1 font-mono text-[11px] uppercase tracking-[0.14em] text-[var(--v2-accent-text)]">
        ${t("interceptor.personaTitle")}
      </h3>
      <p className="mb-3 text-xs text-[var(--v2-text-muted)]">${t("interceptor.personaDesc")}</p>
      <textarea
        className="w-full min-h-[160px] resize-y rounded-md border border-[var(--v2-panel-border)] bg-[var(--v2-surface-soft)] px-3 py-2 font-mono text-xs text-[var(--v2-text-strong)] focus:outline-none focus:ring-1 focus:ring-[var(--v2-accent)]"
        value=${draft}
        disabled=${isMutating}
        onInput=${(e) => setDraft(e.target.value)}
        placeholder=${t("interceptor.personaPlaceholder")}
      />
      <div className="mt-2 flex items-center gap-2">
        <${Button}
          variant="primary"
          size="sm"
          disabled=${!isDirty || isMutating}
          onClick=${handleSave}
        >
          ${isMutating ? t("common.saving") : t("common.save")}
        <//>
        ${saved && !isDirty &&
          html`<span className="text-xs text-emerald-400">${t("interceptor.personaSaved")}</span>`}
      </div>
    <//>
  `;
}

// ---------------------------------------------------------------------------
// ControlCard — Reassemble and Pre-warm action buttons.
// ---------------------------------------------------------------------------

function ControlCard({ config, isMutating, actionStatus, onReassemble, onPrewarm, t }) {
  return html`
    <${Card} padding="none" className="p-4 sm:p-5">
      <h3 className="mb-1 font-mono text-[11px] uppercase tracking-[0.14em] text-[var(--v2-accent-text)]">
        ${t("interceptor.actionsTitle")}
      </h3>
      <p className="mb-4 text-xs text-[var(--v2-text-muted)]">${t("interceptor.actionsDesc")}</p>
      <div className="flex flex-wrap gap-3">

        <div className="flex flex-col gap-1">
          <${Button}
            variant="secondary"
            size="sm"
            disabled=${isMutating}
            onClick=${onReassemble}
          >
            ${isMutating && actionStatus.reassemble === ""
              ? t("interceptor.reassembling")
              : t("interceptor.reassemble")}
          <//>
          <p className="max-w-xs text-xs text-[var(--v2-text-muted)]">
            ${t("interceptor.reassembleDesc")}
          </p>
          ${actionStatus.reassemble === "ok" &&
            html`<span className="text-xs text-emerald-400">${t("interceptor.reassembleOk")}</span>`}
          ${actionStatus.reassemble === "error" &&
            html`<span className="text-xs text-red-400">${t("interceptor.reassembleError")}</span>`}
        </div>

        <div className="flex flex-col gap-1">
          <${Button}
            variant="secondary"
            size="sm"
            disabled=${isMutating || !config?.base_prompt_assembled_at}
            onClick=${onPrewarm}
          >
            ${isMutating && actionStatus.prewarm === ""
              ? t("interceptor.prewarming")
              : t("interceptor.prewarm")}
          <//>
          <p className="max-w-xs text-xs text-[var(--v2-text-muted)]">
            ${t("interceptor.prewarmDesc")}
          </p>
          ${actionStatus.prewarm === "ok" &&
            html`<span className="text-xs text-emerald-400">${t("interceptor.prewarmOk")}</span>`}
          ${actionStatus.prewarm === "error" &&
            html`<span className="text-xs text-red-400">${t("interceptor.prewarmError")}</span>`}
        </div>

      </div>
    <//>
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

function InterceptorSkeleton() {
  return html`
    <div className="space-y-5">
      <${Card} padding="none" className="p-4 sm:p-5">
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
      <${Card} padding="none" className="p-4 sm:p-5">
        <${Skeleton} className="mb-4 h-3 w-20" />
        <${Skeleton} className="h-32 w-full" />
        <${Skeleton} className="mt-2 h-8 w-16" />
      <//>
      <${Card} padding="none" className="p-4 sm:p-5">
        <${Skeleton} className="mb-4 h-3 w-20" />
        <div className="flex gap-3">
          <${Skeleton} className="h-9 w-28" />
          <${Skeleton} className="h-9 w-24" />
        </div>
      <//>
    </div>
  `;
}
