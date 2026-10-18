import { React, html } from "../../../lib/html.js";
import { Card } from "../../../design-system/card.js";
import { useT } from "../../../lib/i18n.js";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { fetchInstanceToolSettings, updateInstanceToolSetting, validToolOutputLimit } from "../lib/instance-tool-policy.js";
import { matchesSearch } from "../lib/settings-search.js";
import { SettingsSearchEmpty } from "./settings-search-empty.js";

const QUERY_KEY = ["instance-tool-policies"];

function ToolPolicyRow({ tool, disabled, change }) {
  const t = useT();
  const [bytes, setBytes] = React.useState(tool.limits.max_output_bytes ?? "");
  const limit = bytes === "" ? null : bytes;
  const dirty = limit !== tool.limits.max_output_bytes;
  const valid = validToolOutputLimit(limit);
  const helpId = `tool-output-help-${tool.tool_id}`;
  return html`
    <div className="border-t border-[var(--v2-panel-border)] py-4">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0">
          <h3 className="break-words font-medium text-[var(--v2-text-strong)]">${tool.capability_ids.join(", ") || tool.tool_id}</h3>
          <p className="break-all text-xs text-[var(--v2-text-muted)]">${tool.tool_id}</p>
          <p className="text-xs text-[var(--v2-text-muted)]">${t("settings.tools.revision", { revision: tool.revision })}</p>
        </div>
        <select aria-label=${t("settings.tools.policyLabel", { tool: tool.tool_id })}
          value=${tool.enabled ? "allow" : "deny"}
          onChange=${event => change(tool, event.target.value, tool.limits)}
          disabled=${disabled || dirty}
          className="rounded-lg border border-[var(--v2-panel-border)] bg-[var(--v2-surface)] px-3 py-1.5 text-sm disabled:opacity-50">
          <option value="allow">${t("settings.tools.permission.allow")}</option>
          <option value="deny">${t("settings.tools.permission.deny")}</option>
        </select>
      </div>
      <label className="mt-3 block text-sm" htmlFor=${`tool-output-${tool.tool_id}`}>${t("settings.tools.outputLimit")}</label>
      <div className="mt-1 flex flex-wrap items-center gap-2">
        <input id=${`tool-output-${tool.tool_id}`} type="text" inputMode="numeric"
          aria-label=${t("settings.tools.outputLimitLabel", { tool: tool.tool_id })}
          aria-describedby=${helpId} aria-invalid=${!valid}
          value=${bytes} placeholder=${t("settings.tools.outputUnlimited")}
          onChange=${event => setBytes(event.target.value)} disabled=${disabled}
          className="rounded-lg border border-[var(--v2-panel-border)] bg-[var(--v2-surface)] px-3 py-1.5 text-sm disabled:opacity-50" />
        <button type="button" aria-label=${t("settings.tools.saveOutputLimitLabel", { tool: tool.tool_id })}
          onClick=${() => change(tool, tool.enabled ? "allow" : "deny", { max_output_bytes: limit })}
          disabled=${disabled || !dirty || !valid}
          className="rounded-lg border border-[var(--v2-panel-border)] px-3 py-1.5 text-sm disabled:opacity-50">${t("settings.tools.saveOutputLimit")}</button>
      </div>
      <p id=${helpId} className="mt-1 text-xs text-[var(--v2-text-muted)]">${t("settings.tools.outputLimitHelp")}</p>
      ${!valid && html`<p role="alert" className="mt-1 text-xs text-[var(--v2-danger-text)]">${t("settings.tools.outputLimitInvalid")}</p>`}
      ${dirty && html`<p className="mt-1 text-xs text-[var(--v2-text-muted)]">${t("settings.tools.pendingOutputLimit")}</p>`}
    </div>
  `;
}

export function ToolsTab({ searchQuery = "" }) {
  const t = useT();
  const queryClient = useQueryClient();
  const query = useQuery({ queryKey: QUERY_KEY, queryFn: fetchInstanceToolSettings });
  const [error, setError] = React.useState("");
  const [busy, setBusy] = React.useState(false);
  const [needsRefresh, setNeedsRefresh] = React.useState(false);
  // Fence double events before React renders busy state.
  const inFlight = React.useRef(false);
  const uncertain = React.useRef(false);

  async function refresh() {
    if (inFlight.current) return;
    inFlight.current = true;
    setBusy(true);
    try {
      const result = await query.refetch({ throwOnError: true });
      if (result.error) throw result.error;
      uncertain.current = false;
      setNeedsRefresh(false);
      setError("");
    } catch (err) {
      setError(err.message || t("settings.tools.failedRefresh"));
    } finally {
      inFlight.current = false;
      setBusy(false);
    }
  }

  async function change(tool, mode, limits) {
    if (inFlight.current || uncertain.current || query.isFetching
        || !["allow", "deny"].includes(mode)
        || (tool.enabled === (mode === "allow") && tool.limits.max_output_bytes === limits.max_output_bytes)) return;
    inFlight.current = true;
    setBusy(true);
    setError("");
    try {
      await queryClient.cancelQueries({ queryKey: QUERY_KEY });
      const updated = await updateInstanceToolSetting(tool.tool_id, tool.revision, mode === "allow", limits);
      queryClient.setQueryData(QUERY_KEY, current =>
        current?.map(value => value.tool_id === updated.tool_id ? updated : value));
    } catch (err) {
      // A lost response may follow a committed write. Inspect current state;
      // never replay the edit with a guessed or newly observed revision.
      uncertain.current = true;
      setNeedsRefresh(true);
      setError(err.status === 409 ? t("settings.tools.conflict")
        : (err.message || t("settings.tools.updateFailed")));
    } finally {
      inFlight.current = false;
      setBusy(false);
    }
  }

  const tools = query.data || [];
  const filtered = tools.filter(tool => matchesSearch(searchQuery, [tool.tool_id, ...tool.capability_ids, t("settings.tools.outputLimit")]));
  return html`
    <${Card} padding="md">
      <div className="flex items-center justify-between gap-4">
        <h2 className="font-medium text-[var(--v2-text-strong)]">${t("settings.tools.title")}</h2>
        <button type="button" onClick=${refresh} disabled=${busy || query.isFetching}
          className="rounded-lg border border-[var(--v2-panel-border)] px-3 py-1.5 text-sm disabled:opacity-50">
          ${t("settings.tools.refresh")}
        </button>
      </div>
      <p className="my-3 text-sm text-[var(--v2-text-muted)]">${t("settings.tools.policyHelp")}</p>
      ${(error || query.error) && html`<p role="alert" className="mb-3 text-sm text-[var(--v2-danger-text)]">
        ${error || t("settings.tools.failedLoad", { message: query.error.message })}
      </p>`}
      ${needsRefresh && html`<p className="mb-3 text-sm text-[var(--v2-text-muted)]">${t("settings.tools.refreshRequired")}</p>`}
      ${query.isLoading ? html`<p role="status">${t("settings.tools.loading")}</p>`
        : !query.error && tools.length === 0 ? html`<p className="text-sm text-[var(--v2-text-muted)]">${t("settings.tools.emptyDesc")}</p>`
        : filtered.length === 0 && !query.error ? html`<${SettingsSearchEmpty} query=${searchQuery} />`
        : filtered.map(tool => html`<${ToolPolicyRow} key=${`${tool.tool_id}:${tool.revision}`}
            tool=${tool} change=${change} disabled=${busy || needsRefresh || query.isFetching || !!query.error} />`)}
    <//>
  `;
}
