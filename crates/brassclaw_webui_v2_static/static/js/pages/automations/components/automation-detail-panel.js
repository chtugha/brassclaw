import { React, html } from "../../../lib/html.js";
import { useT } from "../../../lib/i18n.js";
import { Button } from "../../../design-system/button.js";
import { StatusPill } from "../../../design-system/primitives.js";
import {
  formatAutomationDate,
  durationLabel,
  runStatusTone,
  runStatusLabel,
  stateLabel,
  stateTone,
} from "../lib/automations-presenters.js";
import { useAutomationDetail } from "../hooks/useAutomationDetail.js";
import { AutomationDeleteConfirm } from "./automation-delete-confirm.js";

const TABS = ["overview", "runs", "raw"];

export function AutomationDetailPanel({
  automationId,
  onClose,
  onDeleted,
  onUpdated,
  setState,
  fire,
  remove,
}) {
  const t = useT();
  const { automation, isLoadingDetail, detailError, runs, isLoadingRuns, update } =
    useAutomationDetail(automationId);

  const [tab, setTab] = React.useState("overview");
  const [editName, setEditName] = React.useState(null);
  const [editPrompt, setEditPrompt] = React.useState(null);
  const [showDelete, setShowDelete] = React.useState(false);

  const isDirty = editName !== null || editPrompt !== null;

  async function handleSave() {
    const patch = {};
    if (editName !== null) patch.name = editName;
    if (editPrompt !== null) patch.prompt = editPrompt;
    try {
      await update.mutateAsync(patch);
      setEditName(null);
      setEditPrompt(null);
      onUpdated?.();
    } catch (_err) { /* error shown via update.error */ }
  }

  function handleDiscard() {
    setEditName(null);
    setEditPrompt(null);
  }

  if (isLoadingDetail) {
    return html`
      <aside className="v2-detail-panel flex flex-col">
        <div className="p-4 text-sm text-iron-400">${t("common.loading")}</div>
      </aside>
    `;
  }

  if (detailError || !automation) {
    return html`
      <aside className="v2-detail-panel flex flex-col">
        <div className="p-4 text-sm text-red-400">${t("automations.error_not_found")}</div>
      </aside>
    `;
  }

  const displayName = editName ?? automation.name ?? "";
  const displayPrompt = editPrompt ?? automation.prompt ?? "";
  const cronExpr = automation.source?.cron ?? "";
  const policy = automation.completion_policy ?? "recurring";

  return html`
    <aside className="v2-detail-panel flex flex-col border-l border-[var(--v2-panel-border)] w-full max-w-md bg-[var(--v2-surface)] overflow-y-auto">
      <!-- Header -->
      <div className="flex items-center justify-between px-5 py-4 border-b border-[var(--v2-panel-border)]">
        <${StatusPill} tone=${stateTone(automation.state)} label=${stateLabel(automation.state)} />
        <div className="flex items-center gap-2">
          <${Button}
            variant="secondary"
            size="sm"
            disabled=${automation.is_active || automation.state === "paused"}
            onClick=${() => fire?.mutate(automationId)}
          >
            ${t("automations.action_fire")}
          <//>
          <${Button} variant="ghost" size="icon-sm" onClick=${onClose}>✕<//>
        </div>
      </div>

      <!-- Tabs -->
      <div className="flex border-b border-[var(--v2-panel-border)] px-5">
        ${TABS.map((key) => html`
          <button
            key=${key}
            type="button"
            onClick=${() => setTab(key)}
            className=${
              "px-3 py-3 text-sm font-medium border-b-2 -mb-px " +
              (tab === key
                ? "border-[var(--v2-accent)] text-[var(--v2-accent-text)]"
                : "border-transparent text-iron-400 hover:text-iron-200")
            }
          >
            ${t("automations.tab_" + key)}
          </button>
        `)}
      </div>

      <!-- Tab content -->
      <div className="flex-1 overflow-y-auto p-5">
        ${tab === "overview" && html`
          <div className="space-y-4">
            <!-- Name (inline edit) -->
            <div>
              <p className="text-xs text-iron-400 mb-1">${t("automations.field_name")}</p>
              <input
                type="text"
                value=${displayName}
                onInput=${(e) => setEditName(e.target.value)}
                className="v2-input w-full"
              />
            </div>

            <!-- Schedule -->
            <div>
              <p className="text-xs text-iron-400 mb-1">${t("automations.field_schedule")}</p>
              <p className="text-sm text-iron-100 font-mono">${cronExpr}</p>
            </div>

            <!-- Next fire / last run -->
            <div className="grid grid-cols-2 gap-4">
              <div>
                <p className="text-xs text-iron-400 mb-1">${t("automations.field_next_fire")}</p>
                <p className="text-sm text-iron-200">${formatAutomationDate(automation.next_run_at, "—")}</p>
              </div>
              <div>
                <p className="text-xs text-iron-400 mb-1">${t("automations.field_last_run")}</p>
                <p className="text-sm text-iron-200">${formatAutomationDate(automation.last_run_at, "—")}</p>
              </div>
            </div>

            <!-- Prompt (editable) -->
            <div>
              <p className="text-xs text-iron-400 mb-1">${t("automations.field_prompt")}</p>
              <textarea
                value=${displayPrompt}
                onInput=${(e) => setEditPrompt(e.target.value)}
                rows="5"
                className="v2-input w-full resize-y"
              />
            </div>

            <!-- Completion policy -->
            <div>
              <p className="text-xs text-iron-400 mb-1">${t("automations.field_policy")}</p>
              <p className="text-sm text-iron-200">
                ${policy === "complete_after_first_fire"
                  ? t("automations.policy_complete_after_first")
                  : t("automations.policy_recurring")}
              </p>
            </div>

            ${update.error && html`
              <p className="text-xs text-red-400">${update.error.message || "Save failed."}</p>
            `}

            <!-- Save / Discard -->
            ${isDirty && html`
              <div className="flex gap-2">
                <${Button}
                  variant="primary"
                  size="sm"
                  disabled=${update.isPending}
                  onClick=${handleSave}
                >
                  ${update.isPending ? "..." : t("automations.action_save")}
                <//>
                <${Button} variant="secondary" size="sm" onClick=${handleDiscard}>
                  ${t("automations.action_discard")}
                <//>
              </div>
            `}

            <!-- State actions -->
            <div className="pt-2 flex gap-2 border-t border-[var(--v2-panel-border)]">
              ${automation.state === "scheduled" || automation.state === "active"
                ? html`
                    <${Button}
                      variant="secondary"
                      size="sm"
                      onClick=${() => setState?.mutate({ id: automationId, action: "pause" })}
                    >
                      ${t("automations.action_pause")}
                    <//>
                  `
                : automation.state === "paused" && html`
                    <${Button}
                      variant="secondary"
                      size="sm"
                      onClick=${() => setState?.mutate({ id: automationId, action: "resume" })}
                    >
                      ${t("automations.action_resume")}
                    <//>
                  `}
              <${Button}
                variant="danger"
                size="sm"
                onClick=${() => setShowDelete(true)}
              >
                ${t("automations.action_delete")}
              <//>
            </div>
          </div>
        `}

        ${tab === "runs" && html`
          <div>
            ${isLoadingRuns
              ? html`<p className="text-sm text-iron-400">${t("common.loading")}</p>`
              : runs.length === 0
              ? html`<p className="text-sm text-iron-400">${t("automations.runs_empty")}</p>`
              : html`
                  <div className="space-y-2">
                    ${runs.map((run) => html`
                      <div
                        key=${run.run_id}
                        className="rounded-xl border border-[var(--v2-panel-border)] bg-[var(--v2-surface-soft)] p-3"
                      >
                        <div className="flex items-center justify-between mb-2">
                          <${StatusPill}
                            tone=${runStatusTone(run.status)}
                            label=${runStatusLabel(run.status)}
                          />
                          <span className="text-xs text-iron-400 font-mono">
                            ${durationLabel(run.started_at, run.finished_at)}
                          </span>
                        </div>
                        <div className="text-xs text-iron-400">
                          ${t("automations.run_started")}: ${formatAutomationDate(run.started_at)}
                        </div>
                        ${run.finished_at && html`
                          <div className="text-xs text-iron-400">
                            ${t("automations.run_finished")}: ${formatAutomationDate(run.finished_at)}
                          </div>
                        `}
                      </div>
                    `)}
                  </div>
                `}
          </div>
        `}

        ${tab === "raw" && html`
          <pre className="text-xs text-iron-300 overflow-auto whitespace-pre-wrap break-all">
            ${JSON.stringify(automation, null, 2)}
          </pre>
        `}
      </div>

      <!-- Delete confirmation modal -->
      ${showDelete && html`
        <${AutomationDeleteConfirm}
          automation=${automation}
          onClose=${() => setShowDelete(false)}
          onDeleted=${() => {
            setShowDelete(false);
            onDeleted?.();
          }}
          remove=${remove}
        />
      `}
    </aside>
  `;
}
