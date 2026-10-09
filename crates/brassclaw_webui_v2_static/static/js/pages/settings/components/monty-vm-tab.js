/**
 * MontyVmTab — Settings tab for Monty VM resource limits, orchestrator
 * selection, and lifecycle controls (spec §3.10, Phase 6 Step 6.2).
 *
 * Duration and token-mode edits apply live. Saved desired revisions and actual
 * worker acknowledgements are shown separately; restart is an optional service
 * capability, never a prerequisite for applying budgets.
 */
import { React, html } from "../../../lib/html.js";
import { Card } from "../../../design-system/card.js";
import { Button } from "../../../design-system/button.js";
import { Badge } from "../../../design-system/badge.js";
import { useT } from "../../../lib/i18n.js";
import {
  fetchMontyVmSettings,
  updateMontyVmSettings,
  restartMontyVm,
  fetchMontyVmStatus,
} from "../lib/settings-api.js";

// ── Poll interval for live status while restarting ────────────────────────────
const STATUS_POLL_MS = 3000;
const EXECUTION_FIELDS = [
  "max_source_bytes", "max_compiled_source_bytes", "max_feeds", "max_stdout_bytes",
  "execution_slice_millis", "max_value_depth", "max_value_nodes", "max_value_bytes",
];
const pendingSettings = (status) => status.state === "running" &&
  (status.task_budget?.uptake !== "applied" || status.execution_limits?.uptake !== "applied");

export function MontyVmTab({ searchQuery = "" }) {
  const t = useT();

  // Settings state.
  const [settings, setSettings] = React.useState(null);
  const [isLoadingSettings, setIsLoadingSettings] = React.useState(true);
  const [settingsError, setSettingsError] = React.useState(null);
  const [isSaving, setIsSaving] = React.useState(false);
  const [saveError, setSaveError] = React.useState(null);
  const [savedOk, setSavedOk] = React.useState(false);

  // Status state.
  const [status, setStatus] = React.useState(null);
  const [statusError, setStatusError] = React.useState(null);
  const [isPolling, setIsPolling] = React.useState(false);

  // Restart state.
  const [showConfirm, setShowConfirm] = React.useState(false);
  const [isRestarting, setIsRestarting] = React.useState(false);
  const [restartError, setRestartError] = React.useState(null);

  // Load settings on mount.
  React.useEffect(() => {
    let cancelled = false;
    setIsLoadingSettings(true);
    Promise.allSettled([fetchMontyVmSettings(), fetchMontyVmStatus()])
      .then(([s, st]) => {
        if (cancelled) return;
        if (s.status === "fulfilled") {
          setSettings(s.value.settings);
          if (s.value.runtime) setStatus(s.value.runtime);
        }
        else setSettingsError(s.reason);
        if (st.status === "fulfilled") {
          setStatus(st.value);
          setIsPolling(pendingSettings(st.value));
        } else setStatusError(st.reason.message || String(st.reason));
      })
      .finally(() => {
        if (!cancelled) setIsLoadingSettings(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Serial polling while a saved revision awaits acknowledgement.
  React.useEffect(() => {
    if (!isPolling) return;
    let cancelled = false;
    let timer;
    const poll = async () => {
      try {
        const st = await fetchMontyVmStatus();
        if (!cancelled) {
          setStatus(st);
          setStatusError(null);
          if ((st.state === "running" && !pendingSettings(st)) || st.state === "stopped" || st.state === "error") {
            setIsPolling(false);
          } else timer = setTimeout(poll, STATUS_POLL_MS);
        }
      } catch (err) {
        if (!cancelled) {
          setIsPolling(false);
          setStatusError(err.message || String(err));
        }
      }
    };
    timer = setTimeout(poll, STATUS_POLL_MS);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [isPolling]);

  const handleSave = React.useCallback(async () => {
    if (!settings) return;
    setIsSaving(true);
    setSaveError(null);
    setSavedOk(false);
    try {
      const executionLimits = {};
      for (const key of EXECUTION_FIELDS) {
        const value = Number(settings.execution_limits?.[key]);
        if (!Number.isSafeInteger(value) || value <= 0) throw new Error(t("montyVm.executionLimitsInvalid"));
        executionLimits[key] = value;
      }
      const updated = await updateMontyVmSettings({
        execution_limits: executionLimits,
        expected_revision: settings.revision,
        token_budgets_enabled: settings.token_budgets_enabled,
        max_duration_secs: settings.max_duration_secs,
        failure_rollback_threshold: settings.failure_rollback_threshold,
        prior_knowledge_token_budget: settings.prior_knowledge_token_budget,
        q4_retention_days: settings.q4_retention_days,
        forensic_packet_retention_days: settings.forensic_packet_retention_days,
      });
      if (updated?.settings) setSettings(updated.settings);
      if (updated?.runtime) {
        setStatus(updated.runtime);
        setStatusError(null);
        setIsPolling(pendingSettings(updated.runtime));
      }
      setSavedOk(true);
      setTimeout(() => setSavedOk(false), 2500);
    } catch (err) {
      setSaveError(err.message || String(err));
    } finally {
      setIsSaving(false);
    }
  }, [settings, t]);

  const handleRestart = React.useCallback(async () => {
    setShowConfirm(false);
    setIsRestarting(true);
    setRestartError(null);
    try {
      const result = await restartMontyVm({ force: false });
      setStatus(result);
      if (result.state === "draining" || result.state === "restarting") {
        setIsPolling(true);
      }
    } catch (err) {
      setRestartError(err.message || String(err));
    } finally {
      setIsRestarting(false);
    }
  }, []);

  if (isLoadingSettings) {
    return html`<${MontyVmSkeleton} />`;
  }

  if (settingsError) {
    return html`
      <div className="rounded-xl border border-red-400/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
        ${t("montyVm.failedLoad", { message: settingsError.message || String(settingsError) })}
      </div>
    `;
  }

  return html`
    <div className="space-y-5">

      ${/* Status indicator */ ""}
      ${status && html`<${StatusCard} status=${status} isPolling=${isPolling} t=${t} />`}

      ${statusError && html`<div role="alert" className="text-sm text-red-200">${statusError}</div>`}
      ${/* Error banners */ ""}
      ${saveError && html`
        <div className="rounded-xl border border-red-400/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
          ${saveError}
        </div>
      `}
      ${restartError && html`
        <div className="rounded-xl border border-red-400/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
          ${restartError}
        </div>
      `}

      ${/* Settings form */ ""}
      ${settings && html`
        <${SettingsForm}
          settings=${settings}
          onChange=${setSettings}
          onSave=${handleSave}
          isSaving=${isSaving}
          savedOk=${savedOk}
          t=${t}
        />
      `}

      ${/* Restart section */ ""}
      <${RestartSection}
        status=${status}
        isRestarting=${isRestarting}
        isPolling=${isPolling}
        showConfirm=${showConfirm}
        onRequestRestart=${() => setShowConfirm(true)}
        onConfirmRestart=${handleRestart}
        onCancelRestart=${() => setShowConfirm(false)}
        t=${t}
      />

    </div>
  `;
}

// ── StatusCard ────────────────────────────────────────────────────────────────

function StatusCard({ status, isPolling, t }) {
  const tone =
    status.state === "running"
      ? "positive"
      : status.state === "error"
      ? "negative"
      : "neutral";

  return html`
    <${Card} padding="none" className="p-4 sm:p-5">
      <h3 className="mb-3 font-mono text-[11px] uppercase tracking-[0.14em] text-[var(--v2-accent-text)]">
        ${t("montyVm.status")}
      </h3>
      <div className="flex flex-wrap items-center gap-4">
        <div className="flex items-center gap-2">
          <span className="text-sm text-[var(--v2-text-muted)]">${t("montyVm.state")}</span>
          <${Badge} tone=${tone} label=${status.state} size="sm" />
          ${isPolling &&
            html`<span className="text-xs text-[var(--v2-text-muted)] animate-pulse">
              ${t("montyVm.polling")}
            </span>`}
        </div>
        ${status.task_budget && html`
          <div className="text-sm text-[var(--v2-text-muted)]">
            ${t("montyVm.desiredRevision")}: ${status.task_budget.desired_revision}
            · ${t("montyVm.effectiveRevision")}: ${status.task_budget.effective_revision}
            · ${t("montyVm.maxDuration")}: ${status.task_budget.max_duration_secs}
            · ${t(`montyVm.uptake.${status.task_budget.uptake}`)}
          </div>
        `}
        ${status.execution_limits && html`
          <div className="text-sm text-[var(--v2-text-muted)]">
            ${t("montyVm.executionLimitsTitle")}
            · ${t("montyVm.desiredRevision")}: ${status.execution_limits.desired_revision}
            · ${t("montyVm.effectiveRevision")}: ${status.execution_limits.effective_revision}
            · ${t(`montyVm.uptake.${status.execution_limits.uptake}`)}
            ${status.execution_limits.failure_reason && html`<span role="alert"> · ${status.execution_limits.failure_reason}</span>`}
          </div>
        `}
        ${status.orchestrator_version &&
          html`
            <div className="flex items-center gap-2">
              <span className="text-sm text-[var(--v2-text-muted)]">${t("montyVm.orchVersion")}</span>
              <span className="font-mono text-xs text-[var(--v2-text-strong)]">
                ${status.orchestrator_version}
              </span>
            </div>
          `}
        ${status.settings_hash &&
          html`
            <div className="flex items-center gap-2">
              <span className="text-sm text-[var(--v2-text-muted)]">${t("montyVm.settingsHash")}</span>
              <span className="font-mono text-xs text-[var(--v2-text-muted)] truncate max-w-[120px]">
                ${status.settings_hash.slice(0, 12)}…
              </span>
            </div>
          `}
      </div>
    <//>
  `;
}

// ── SettingsForm ──────────────────────────────────────────────────────────────

function SettingsForm({ settings, onChange, onSave, isSaving, savedOk, t }) {
  const field = (key, label, desc, type = "number") => html`
    <div className="grid grid-cols-1 sm:grid-cols-3 items-start gap-x-4 gap-y-1 py-3 border-t border-[var(--v2-panel-border)] first:border-0">
      <div>
        <label className="text-sm font-medium text-[var(--v2-text-strong)]">${label}</label>
        ${desc &&
          html`<p className="mt-0.5 text-xs text-[var(--v2-text-muted)]">${desc}</p>`}
      </div>
      <input
        type=${type}
        className="col-span-2 w-full rounded-md border border-[var(--v2-panel-border)] bg-[var(--v2-surface-soft)] px-3 py-1.5 font-mono text-sm text-[var(--v2-text-strong)] focus:outline-none focus:ring-1 focus:ring-[var(--v2-accent)]"
        value=${settings[key] ?? ""}
        disabled=${isSaving}
        onInput=${(e) =>
          onChange((prev) => ({
            ...prev,
            [key]: type === "number" ? Number(e.target.value) : e.target.value,
          }))}
      />
    </div>
  `;

  return html`
    <${Card} padding="none" className="p-4 sm:p-5">
      <h3 className="mb-4 font-mono text-[11px] uppercase tracking-[0.14em] text-[var(--v2-accent-text)]">
        ${t("montyVm.settingsTitle")}
      </h3>
      <p className="mb-3 text-xs text-[var(--v2-text-muted)]">${t("montyVm.allocationCountRetired")}</p>
      ${settings.retired_max_allocations != null && html`<p className="mb-3 text-xs text-[var(--v2-text-muted)]">${t("montyVm.retiredAllocationValue")}: ${settings.retired_max_allocations}</p>`}
      ${field("max_duration_secs", t("montyVm.maxDuration"), t("montyVm.maxDurationDesc"))}
      ${field("failure_rollback_threshold", t("montyVm.rollbackThreshold"), t("montyVm.rollbackThresholdDesc"))}
      <label className="flex items-center gap-3 py-3 border-t border-[var(--v2-panel-border)]">
        <input
          type="checkbox"
          checked=${settings.token_budgets_enabled === true}
          disabled=${isSaving}
          onChange=${(event) => {
            const enabled = event.target.checked;
            onChange((previous) => ({ ...previous, token_budgets_enabled: enabled }));
          }}
        />
        <span className="text-sm text-[var(--v2-text-strong)]">${t("montyVm.tokenBudgetsEnabled")}</span>
      </label>
      ${field("prior_knowledge_token_budget", t("montyVm.tokenBudget"), t("montyVm.tokenBudgetDesc"))}
      ${field("q4_retention_days", t("montyVm.q4RetentionDays"), t("montyVm.q4RetentionDaysDesc"))}
      ${field("forensic_packet_retention_days", t("montyVm.forensicRetentionDays"), t("montyVm.forensicRetentionDaysDesc"))}
      <details className="mt-4 border-t border-[var(--v2-panel-border)] pt-3">
        <summary className="cursor-pointer text-sm font-medium">${t("montyVm.executionLimitsTitle")}</summary>
        <p className="mt-2 text-xs text-[var(--v2-text-muted)]">${t("montyVm.executionLimitsDesc")}</p>
        ${EXECUTION_FIELDS.map((key) => html`
          <div key=${key} className="grid grid-cols-1 sm:grid-cols-3 gap-3 py-2">
            <label htmlFor=${`monty-${key}`} className="text-sm">${t(`montyVm.execution.${key}`)}</label>
            <input id=${`monty-${key}`} type="number" min="1" step="1"
              className="col-span-2 rounded-md border border-[var(--v2-panel-border)] bg-[var(--v2-surface-soft)] px-3 py-1.5 font-mono text-sm"
              value=${settings.execution_limits?.[key] ?? ""} disabled=${isSaving}
              onInput=${(event) => {
                const value = event.target.value;
                onChange((previous) => ({ ...previous,
                  execution_limits: { ...previous.execution_limits, [key]: value },
                }));
              }} />
          </div>
        `)}
      </details>
      <div className="mt-4 flex items-center gap-2">
        <${Button}
          variant="primary"
          size="sm"
          disabled=${isSaving}
          onClick=${onSave}
        >
          ${isSaving ? t("common.saving") : t("common.save")}
        <//>
        ${savedOk &&
          html`<span className="text-xs text-emerald-400">${t("montyVm.saved")}</span>`}
      </div>
    <//>
  `;
}

// ── RestartSection ────────────────────────────────────────────────────────────

function RestartSection({
  status,
  isRestarting,
  isPolling,
  showConfirm,
  onRequestRestart,
  onConfirmRestart,
  onCancelRestart,
  t,
}) {
  const canRestart = status?.restart_supported && (status.state === "running" || status.state === "stopped");

  return html`
    <${Card} padding="none" className="p-4 sm:p-5">
      <h3 className="mb-1 font-mono text-[11px] uppercase tracking-[0.14em] text-[var(--v2-accent-text)]">
        ${t("montyVm.lifecycle")}
      </h3>
      <p className="mb-4 text-xs text-[var(--v2-text-muted)]">
        ${t("montyVm.lifecycleDesc")}
      </p>
      ${!showConfirm && html`
        <${Button}
          variant="secondary"
          size="sm"
          disabled=${isRestarting || !canRestart}
          onClick=${onRequestRestart}
        >
          ${isRestarting ? t("montyVm.restarting") : t("montyVm.restart")}
        <//>
      `}
      ${showConfirm && html`
        <div className="rounded-lg border border-amber-400/30 bg-amber-500/10 p-4 space-y-3">
          <p className="text-sm text-amber-200 font-medium">${t("montyVm.confirmTitle")}</p>
          <p className="text-xs text-amber-200/80">${t("montyVm.confirmDesc")}</p>
          <div className="flex gap-2">
            <${Button} variant="primary" size="sm" onClick=${onConfirmRestart}>
              ${t("montyVm.confirmOk")}
            <//>
            <${Button} variant="secondary" size="sm" onClick=${onCancelRestart}>
              ${t("common.cancel")}
            <//>
          </div>
        </div>
      `}
    <//>
  `;
}

// ── Skeleton ──────────────────────────────────────────────────────────────────

function MontyVmSkeleton() {
  return html`
    <div className="space-y-5">
      ${[1, 2].map(
        (i) => html`
          <div key=${i} className="rounded-xl border border-[var(--v2-panel-border)] p-4 space-y-3">
            <div className="h-3 w-24 animate-pulse rounded bg-[var(--v2-surface-muted)]" />
            ${[1, 2, 3].map(
              (j) => html`
                <div key=${j} className="h-8 w-full animate-pulse rounded bg-[var(--v2-surface-muted)]" />
              `
            )}
          </div>
        `
      )}
    </div>
  `;
}
