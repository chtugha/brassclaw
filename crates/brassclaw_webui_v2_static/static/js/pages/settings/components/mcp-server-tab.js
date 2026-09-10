/**
 * McpServerTab — Settings tab for the Orchestrator MCP Server (Phase V).
 *
 * MCP server lifecycle:
 * 1. Operator changes port / auto_start and clicks Save — `PUT /api/settings/mcp-server`.
 * 2. Operator clicks "Start" / "Stop" — confirmation appears for Stop.
 * 3. On confirm, `POST /api/settings/mcp-server/start` or `.../stop` is called.
 * 4. Status indicator polls `GET /api/settings/mcp-server/status` every 3s
 *    while state is `starting`; stops once `running`/`stopped`/`error`.
 */
import { React, html } from "../../../lib/html.js";
import { Card } from "../../../design-system/card.js";
import { Button } from "../../../design-system/button.js";
import { Badge } from "../../../design-system/badge.js";
import { useT } from "../../../lib/i18n.js";
import {
  fetchMcpServerSettings,
  updateMcpServerSettings,
  fetchMcpServerStatus,
  startMcpServer,
  stopMcpServer,
} from "../lib/settings-api.js";

// ── Poll interval for live status while starting ──────────────────────────────
const STATUS_POLL_MS = 3000;

export function McpServerTab({ searchQuery = "" }) {
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
  const [isPolling, setIsPolling] = React.useState(false);

  // Action state.
  const [showStopConfirm, setShowStopConfirm] = React.useState(false);
  const [isActioning, setIsActioning] = React.useState(false);
  const [actionError, setActionError] = React.useState(null);

  // Load settings + status on mount.
  React.useEffect(() => {
    let cancelled = false;
    setIsLoadingSettings(true);
    Promise.all([fetchMcpServerSettings(), fetchMcpServerStatus()])
      .then(([s, st]) => {
        if (!cancelled) {
          setSettings(s.settings);
          setStatus(st);
        }
      })
      .catch((err) => {
        if (!cancelled) setSettingsError(err);
      })
      .finally(() => {
        if (!cancelled) setIsLoadingSettings(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Poll status while starting.
  React.useEffect(() => {
    if (!isPolling) return;
    let cancelled = false;
    const poll = async () => {
      try {
        const st = await fetchMcpServerStatus();
        if (!cancelled) {
          setStatus(st);
          if (st.state === "running" || st.state === "stopped" || st.state === "error") {
            setIsPolling(false);
          }
        }
      } catch (err) {
        if (!cancelled) {
          setIsPolling(false);
          setActionError(err.message || String(err));
        }
      }
    };
    const timer = setInterval(poll, STATUS_POLL_MS);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [isPolling]);

  const handleSave = React.useCallback(async () => {
    if (!settings) return;
    setIsSaving(true);
    setSaveError(null);
    setSavedOk(false);
    try {
      const updated = await updateMcpServerSettings({
        port: settings.port,
        auto_start: settings.auto_start,
      });
      if (updated?.settings) setSettings(updated.settings);
      setSavedOk(true);
      setTimeout(() => setSavedOk(false), 2500);
    } catch (err) {
      setSaveError(err.message || String(err));
    } finally {
      setIsSaving(false);
    }
  }, [settings]);

  const handleStart = React.useCallback(async () => {
    setIsActioning(true);
    setActionError(null);
    try {
      const result = await startMcpServer({});
      setStatus(result);
      if (result.state === "starting") {
        setIsPolling(true);
      }
    } catch (err) {
      setActionError(err.message || String(err));
    } finally {
      setIsActioning(false);
    }
  }, []);

  const handleStop = React.useCallback(async () => {
    setShowStopConfirm(false);
    setIsActioning(true);
    setActionError(null);
    try {
      const result = await stopMcpServer({});
      setStatus(result);
    } catch (err) {
      setActionError(err.message || String(err));
    } finally {
      setIsActioning(false);
    }
  }, []);

  if (isLoadingSettings) {
    return html`<${McpServerSkeleton} />`;
  }

  if (settingsError) {
    return html`
      <div className="rounded-xl border border-red-400/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
        ${t("mcpServer.failedLoad", { message: settingsError.message || String(settingsError) })}
      </div>
    `;
  }

  return html`
    <div className="space-y-5">

      ${/* Status indicator */ ""}
      ${status && html`<${StatusCard} status=${status} isPolling=${isPolling} t=${t} />`}

      ${/* Error banners */ ""}
      ${saveError && html`
        <div className="rounded-xl border border-red-400/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
          ${saveError}
        </div>
      `}
      ${actionError && html`
        <div className="rounded-xl border border-red-400/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
          ${actionError}
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

      ${/* Lifecycle section */ ""}
      <${LifecycleSection}
        status=${status}
        isActioning=${isActioning}
        isPolling=${isPolling}
        showStopConfirm=${showStopConfirm}
        onRequestStop=${() => setShowStopConfirm(true)}
        onConfirmStop=${handleStop}
        onCancelStop=${() => setShowStopConfirm(false)}
        onStart=${handleStart}
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
        ${t("mcpServer.status")}
      </h3>
      <div className="flex flex-wrap items-center gap-4">
        <div className="flex items-center gap-2">
          <span className="text-sm text-[var(--v2-text-muted)]">${t("mcpServer.state")}</span>
          <${Badge} tone=${tone} label=${status.state} size="sm" />
          ${isPolling &&
            html`<span className="text-xs text-[var(--v2-text-muted)] animate-pulse">
              ${t("mcpServer.polling")}
            </span>`}
        </div>
        ${status.bound_port != null &&
          html`
            <div className="flex items-center gap-2">
              <span className="text-sm text-[var(--v2-text-muted)]">${t("mcpServer.boundPort")}</span>
              <span className="font-mono text-xs text-[var(--v2-text-strong)]">
                ${status.bound_port}
              </span>
            </div>
          `}
        ${status.error_message &&
          html`
            <div className="flex items-center gap-2">
              <span className="text-sm text-red-300">${status.error_message}</span>
            </div>
          `}
      </div>
    <//>
  `;
}

// ── SettingsForm ──────────────────────────────────────────────────────────────

function SettingsForm({ settings, onChange, onSave, isSaving, savedOk, t }) {
  return html`
    <${Card} padding="none" className="p-4 sm:p-5">
      <h3 className="mb-3 font-mono text-[11px] uppercase tracking-[0.14em] text-[var(--v2-accent-text)]">
        ${t("mcpServer.settings")}
      </h3>
      <div>
        <div className="grid grid-cols-1 sm:grid-cols-3 items-start gap-x-4 gap-y-1 py-3">
          <div>
            <label className="text-sm font-medium text-[var(--v2-text-strong)]">${t("mcpServer.port")}</label>
            <p className="mt-0.5 text-xs text-[var(--v2-text-muted)]">${t("mcpServer.portDesc")}</p>
          </div>
          <input
            type="number"
            min="1024"
            max="65535"
            className="col-span-2 w-full rounded-md border border-[var(--v2-panel-border)] bg-[var(--v2-surface-soft)] px-3 py-1.5 font-mono text-sm text-[var(--v2-text-strong)] focus:outline-none focus:ring-1 focus:ring-[var(--v2-accent)]"
            value=${settings.port ?? ""}
            disabled=${isSaving}
            onInput=${(e) => onChange((prev) => ({ ...prev, port: Number(e.target.value) }))}
          />
        </div>
        <div className="grid grid-cols-1 sm:grid-cols-3 items-start gap-x-4 gap-y-1 py-3 border-t border-[var(--v2-panel-border)]">
          <div>
            <label className="text-sm font-medium text-[var(--v2-text-strong)]">${t("mcpServer.autoStart")}</label>
            <p className="mt-0.5 text-xs text-[var(--v2-text-muted)]">${t("mcpServer.autoStartDesc")}</p>
          </div>
          <div className="col-span-2 flex items-center">
            <input
              type="checkbox"
              className="h-4 w-4 rounded border-[var(--v2-panel-border)] accent-[var(--v2-accent)]"
              checked=${settings.auto_start ?? false}
              disabled=${isSaving}
              onChange=${(e) => onChange((prev) => ({ ...prev, auto_start: e.target.checked }))}
            />
          </div>
        </div>
      </div>
      <div className="mt-4 flex items-center gap-3">
        <${Button} variant="primary" onClick=${onSave} disabled=${isSaving}>
          ${isSaving ? t("common.saving") : t("common.save")}
        <//>
        ${savedOk && html`<span className="text-sm text-[var(--v2-positive)]">${t("common.saved")}</span>`}
      </div>
    <//>
  `;
}

// ── LifecycleSection ──────────────────────────────────────────────────────────

function LifecycleSection({
  status,
  isActioning,
  isPolling,
  showStopConfirm,
  onRequestStop,
  onConfirmStop,
  onCancelStop,
  onStart,
  t,
}) {
  const isRunning = status?.state === "running";
  const isBusy = isActioning || isPolling;

  return html`
    <${Card} padding="none" className="p-4 sm:p-5">
      <h3 className="mb-3 font-mono text-[11px] uppercase tracking-[0.14em] text-[var(--v2-accent-text)]">
        ${t("mcpServer.lifecycle")}
      </h3>
      ${showStopConfirm
        ? html`
            <div className="rounded-lg border border-yellow-400/30 bg-yellow-500/10 px-4 py-3">
              <p className="mb-3 text-sm text-[var(--v2-text-strong)]">${t("mcpServer.stopConfirm")}</p>
              <div className="flex gap-2">
                <${Button} variant="destructive" onClick=${onConfirmStop} disabled=${isBusy}>
                  ${t("mcpServer.stopConfirmYes")}
                <//>
                <${Button} variant="secondary" onClick=${onCancelStop} disabled=${isBusy}>
                  ${t("common.cancel")}
                <//>
              </div>
            </div>
          `
        : html`
            <div className="flex gap-2">
              <${Button}
                variant="primary"
                onClick=${onStart}
                disabled=${isBusy || isRunning}
              >
                ${isActioning && !isRunning ? t("mcpServer.starting") : t("mcpServer.start")}
              <//>
              <${Button}
                variant="secondary"
                onClick=${onRequestStop}
                disabled=${isBusy || !isRunning}
              >
                ${t("mcpServer.stop")}
              <//>
            </div>
          `
      }
    <//>
  `;
}

// ── McpServerSkeleton ─────────────────────────────────────────────────────────

function McpServerSkeleton() {
  return html`
    <div className="space-y-5 animate-pulse">
      <div className="h-24 rounded-xl bg-[var(--v2-surface-soft)]" />
      <div className="h-40 rounded-xl bg-[var(--v2-surface-soft)]" />
      <div className="h-24 rounded-xl bg-[var(--v2-surface-soft)]" />
    </div>
  `;
}
