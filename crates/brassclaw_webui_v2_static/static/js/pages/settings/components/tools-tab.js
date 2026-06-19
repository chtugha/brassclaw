import { React, html } from "../../../lib/html.js";
import { Card } from "../../../design-system/card.js";
import { useT } from "../../../lib/i18n.js";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { fetchTools, updateToolPermission } from "../lib/settings-api.js";
import { matchesSearch } from "../lib/settings-search.js";
import { SettingsSearchEmpty } from "./settings-search-empty.js";
import { SafetyPanel } from "./safety-panel.js";

export function ToolsTab({ searchQuery = "" }) {
  const t = useT();
  const queryClient = useQueryClient();
  
  const query = useQuery({
    queryKey: ["tools"],
    queryFn: fetchTools,
  });

  const updatePermissionMutation = useMutation({
    mutationFn: ({ id, mode }) => updateToolPermission(id, mode),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["tools"] });
    },
  });

  const [updateError, setUpdateError] = React.useState("");

  const handlePermissionChange = React.useCallback(async (toolId, newMode) => {
    setUpdateError("");
    try {
      await updatePermissionMutation.mutateAsync({ id: toolId, mode: newMode });
    } catch (err) {
      setUpdateError(err.message || t("settings.tools.updateFailed"));
    }
  }, [updatePermissionMutation, t]);

  if (query.isLoading) {
    return html`
      <div className="space-y-4">
        <${Card} padding="md">
          <div className="mb-4 h-3 w-24 animate-pulse rounded bg-[var(--v2-surface-muted)]" />
          ${[1, 2, 3].map((i) => html`
            <div key=${i} className="flex items-center justify-between border-t border-[var(--v2-panel-border)] py-4 first:border-0">
              <div className="flex-1">
                <div className="h-4 w-32 animate-pulse rounded bg-[var(--v2-surface-muted)]" />
                <div className="mt-1 h-3 w-48 animate-pulse rounded bg-[var(--v2-surface-muted)]" />
              </div>
              <div className="h-8 w-24 animate-pulse rounded bg-[var(--v2-surface-muted)]" />
            </div>
          `)}
        <//>
      </div>
    `;
  }

  if (query.error) {
    return html`
      <${Card} padding="md">
        <p className="text-sm text-[var(--v2-danger-text)]">
          ${t("settings.tools.failedLoad", { message: query.error.message })}
        </p>
      <//>
    `;
  }

  const tools = query.data?.capabilities || [];
  
  const filteredTools = tools.filter((tool) =>
    matchesSearch(searchQuery, [
      tool.name,
      tool.id,
      tool.description,
      tool.provider,
      ...(tool.effect_kinds || []),
    ])
  );

  // Group tools by provider
  const groupedTools = new Map();
  for (const tool of filteredTools) {
    const provider = tool.provider || "unknown";
    if (!groupedTools.has(provider)) {
      groupedTools.set(provider, []);
    }
    groupedTools.get(provider).push(tool);
  }

  if (tools.length === 0) {
    return html`
      <${Card} padding="lg">
        <h3 className="text-lg font-semibold text-[var(--v2-text-strong)]">
          ${t("settings.tools.empty")}
        </h3>
        <p className="mt-2 max-w-md text-sm leading-6 text-[var(--v2-text-muted)]">
          ${t("settings.tools.emptyDesc")}
        </p>
      <//>
    `;
  }

  if (filteredTools.length === 0) {
    return html`<${SettingsSearchEmpty} query=${searchQuery} />`;
  }

  return html`
    <div className="space-y-6">
      ${updateError && html`
        <div className="rounded-xl border border-red-400/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
          ${updateError}
        </div>
      `}
      
      <div className="space-y-4">
        <h2 className="text-lg font-semibold text-[var(--v2-text-strong)]">
          ${t("settings.tools.title")}
        </h2>
        ${Array.from(groupedTools.entries()).map(([provider, providerTools]) => html`
          <${ProviderGroup}
            key=${provider}
            provider=${provider}
            tools=${providerTools}
            onPermissionChange=${handlePermissionChange}
            isUpdating=${updatePermissionMutation.isPending}
            t=${t}
          />
        `)}
      </div>

      <div className="space-y-4">
        <h2 className="text-lg font-semibold text-[var(--v2-text-strong)]">
          ${t("settings.safety.title")}
        </h2>
        <${SafetyPanel} searchQuery=${searchQuery} />
      </div>
    </div>
  `;
}

function ProviderGroup({ provider, tools, onPermissionChange, isUpdating, t }) {
  const [isExpanded, setIsExpanded] = React.useState(true);

  return html`
    <${Card} padding="md">
      <button
        onClick=${() => setIsExpanded(!isExpanded)}
        className="mb-4 flex w-full items-center justify-between text-left"
      >
        <h3 className="font-mono text-[11px] uppercase tracking-[0.14em] text-[var(--v2-accent-text)]">
          ${provider} (${tools.length})
        </h3>
        <svg
          className=${`h-4 w-4 transition-transform ${isExpanded ? "rotate-180" : ""}`}
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M19 9l-7 7-7-7" />
        </svg>
      </button>
      ${isExpanded && html`
        <div className="space-y-0">
          ${tools.map((tool) => html`
            <${ToolRow}
              key=${tool.id}
              tool=${tool}
              onPermissionChange=${onPermissionChange}
              isUpdating=${isUpdating}
              t=${t}
            />
          `)}
        </div>
      `}
    <//>
  `;
}

function ToolRow({ tool, onPermissionChange, isUpdating, t }) {
  const permissionMode = tool.permission_mode || "ask";

  return html`
    <div className="flex items-start justify-between border-t border-[var(--v2-panel-border)] py-4 first:border-0">
      <div className="flex-1 pr-4">
        <div className="flex items-center gap-2">
          <h4 className="font-medium text-[var(--v2-text-strong)]">${tool.name}</h4>
          ${tool.effect_kinds && tool.effect_kinds.length > 0 && html`
            <div className="flex gap-1">
              ${tool.effect_kinds.map((effect) => html`
                <${EffectBadge} key=${effect} effect=${effect} t=${t} />
              `)}
            </div>
          `}
        </div>
        ${tool.description && html`
          <p className="mt-1 text-sm text-[var(--v2-text-muted)]">${tool.description}</p>
        `}
      </div>
      <div className="flex-shrink-0">
        <select
          value=${permissionMode}
          onChange=${(e) => onPermissionChange(tool.id, e.target.value)}
          disabled=${isUpdating}
          className="rounded-lg border border-[var(--v2-panel-border)] bg-[var(--v2-surface)] px-3 py-1.5 text-sm text-[var(--v2-text-strong)] transition-colors hover:border-[var(--v2-accent-border)] focus:border-[var(--v2-accent-border)] focus:outline-none disabled:opacity-50"
        >
          <option value="allow">${t("settings.tools.permission.allow")}</option>
          <option value="ask">${t("settings.tools.permission.ask")}</option>
          <option value="deny">${t("settings.tools.permission.deny")}</option>
        </select>
      </div>
    </div>
  `;
}

function EffectBadge({ effect, t }) {
  const colorMap = {
    read: "bg-blue-500/20 text-blue-200 border-blue-400/30",
    write: "bg-amber-500/20 text-amber-200 border-amber-400/30",
    execute: "bg-red-500/20 text-red-200 border-red-400/30",
    network: "bg-purple-500/20 text-purple-200 border-purple-400/30",
    system: "bg-orange-500/20 text-orange-200 border-orange-400/30",
  };
  
  const colorClass = colorMap[effect] || "bg-gray-500/20 text-gray-200 border-gray-400/30";
  
  return html`
    <span
      className=${`inline-flex items-center rounded-full border px-2 py-0.5 text-[10px] font-medium uppercase tracking-wider ${colorClass}`}
      title=${t(`settings.tools.effects.${effect}`, effect)}
    >
      ${effect}
    </span>
  `;
}

