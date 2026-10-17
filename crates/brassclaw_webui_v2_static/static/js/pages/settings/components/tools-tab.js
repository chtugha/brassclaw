import { React, html } from "../../../lib/html.js";
import { Card } from "../../../design-system/card.js";
import { useT } from "../../../lib/i18n.js";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { fetchInstanceToolSettings, updateInstanceToolSetting } from "../lib/instance-tool-policy.js";
import { matchesSearch } from "../lib/settings-search.js";
import { SettingsSearchEmpty } from "./settings-search-empty.js";

const QUERY_KEY = ["instance-tool-policies"];

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

  async function change(tool, mode) {
    if (inFlight.current || uncertain.current || query.isFetching
        || !["allow", "deny"].includes(mode) || tool.enabled === (mode === "allow")) return;
    inFlight.current = true;
    setBusy(true);
    setError("");
    try {
      await queryClient.cancelQueries({ queryKey: QUERY_KEY });
      const updated = await updateInstanceToolSetting(tool.tool_id, tool.revision, mode === "allow");
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
  const filtered = tools.filter(tool => matchesSearch(searchQuery, [tool.tool_id, ...tool.capability_ids]));
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
        : filtered.map(tool => html`
          <div key=${tool.tool_id} className="flex items-start justify-between gap-4 border-t border-[var(--v2-panel-border)] py-4">
            <div className="min-w-0">
              <h3 className="break-words font-medium text-[var(--v2-text-strong)]">${tool.capability_ids.join(", ") || tool.tool_id}</h3>
              <p className="break-all text-xs text-[var(--v2-text-muted)]">${tool.tool_id}</p>
              <p className="text-xs text-[var(--v2-text-muted)]">${t("settings.tools.revision", { revision: tool.revision })}</p>
            </div>
            <select aria-label=${t("settings.tools.policyLabel", { tool: tool.tool_id })}
              value=${tool.enabled ? "allow" : "deny"}
              onChange=${event => change(tool, event.target.value)}
              disabled=${busy || needsRefresh || query.isFetching || !!query.error}
              className="rounded-lg border border-[var(--v2-panel-border)] bg-[var(--v2-surface)] px-3 py-1.5 text-sm disabled:opacity-50">
              <option value="allow">${t("settings.tools.permission.allow")}</option>
              <option value="deny">${t("settings.tools.permission.deny")}</option>
            </select>
          </div>
        `)}
    <//>
  `;
}
