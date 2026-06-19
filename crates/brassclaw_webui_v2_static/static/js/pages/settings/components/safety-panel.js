import { React, html } from "../../../lib/html.js";
import { Card } from "../../../design-system/card.js";
import { useT } from "../../../lib/i18n.js";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  fetchSafetySensitivePaths,
  updateSafetySensitivePaths,
  fetchSafetyWorkspaceRules,
  updateSafetyWorkspaceRules,
  fetchSafetyBlockedPaths,
  updateSafetyBlockedPaths,
} from "../lib/settings-api.js";
import { matchesSearch } from "../lib/settings-search.js";
import { SettingsSearchEmpty } from "./settings-search-empty.js";

export function SafetyPanel({ searchQuery = "" }) {
  const t = useT();
  const queryClient = useQueryClient();

  const sensitivePaths = useQuery({
    queryKey: ["safety", "sensitive-paths"],
    queryFn: fetchSafetySensitivePaths,
  });

  const workspaceRules = useQuery({
    queryKey: ["safety", "workspace-rules"],
    queryFn: fetchSafetyWorkspaceRules,
  });

  const blockedPaths = useQuery({
    queryKey: ["safety", "blocked-paths"],
    queryFn: fetchSafetyBlockedPaths,
  });

  const updateSensitivePathsMutation = useMutation({
    mutationFn: updateSafetySensitivePaths,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["safety", "sensitive-paths"] });
    },
  });

  const updateWorkspaceRulesMutation = useMutation({
    mutationFn: updateSafetyWorkspaceRules,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["safety", "workspace-rules"] });
    },
  });

  const updateBlockedPathsMutation = useMutation({
    mutationFn: updateSafetyBlockedPaths,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["safety", "blocked-paths"] });
    },
  });

  const [updateError, setUpdateError] = React.useState("");

  const isLoading = sensitivePaths.isLoading || workspaceRules.isLoading || blockedPaths.isLoading;
  const hasError = sensitivePaths.error || workspaceRules.error || blockedPaths.error;

  if (isLoading) {
    return html`
      <div className="space-y-4">
        ${[1, 2, 3].map((i) => html`
          <${Card} key=${i} padding="md">
            <div className="mb-4 h-3 w-32 animate-pulse rounded bg-[var(--v2-surface-muted)]" />
            <div className="h-24 animate-pulse rounded bg-[var(--v2-surface-muted)]" />
          <//>
        `)}
      </div>
    `;
  }

  if (hasError) {
    const error = sensitivePaths.error || workspaceRules.error || blockedPaths.error;
    return html`
      <${Card} padding="md">
        <p className="text-sm text-[var(--v2-danger-text)]">
          ${t("settings.safety.failedLoad", { message: error.message })}
        </p>
      <//>
    `;
  }

  // Filter sections based on search query
  const sections = [
    {
      key: "sensitive-paths",
      title: t("settings.safety.sensitive_paths.title"),
      description: t("settings.safety.sensitive_paths.description"),
      data: sensitivePaths.data,
      mutation: updateSensitivePathsMutation,
      emptyText: t("settings.safety.sensitive_paths.empty"),
    },
    {
      key: "workspace-rules",
      title: t("settings.safety.workspace_rules.title"),
      description: t("settings.safety.workspace_rules.description"),
      data: workspaceRules.data,
      mutation: updateWorkspaceRulesMutation,
      emptyText: t("settings.safety.workspace_rules.empty"),
    },
    {
      key: "blocked-paths",
      title: t("settings.safety.blocked_paths.title"),
      description: t("settings.safety.blocked_paths.description"),
      data: blockedPaths.data,
      mutation: updateBlockedPathsMutation,
      emptyText: t("settings.safety.blocked_paths.empty"),
    },
  ];

  const filteredSections = sections.filter((section) =>
    matchesSearch(searchQuery, [
      section.title,
      section.description,
      ...(section.data?.entries || []).map((e) => e.pattern),
    ])
  );

  if (filteredSections.length === 0) {
    return html`<${SettingsSearchEmpty} query=${searchQuery} />`;
  }

  return html`
    <div className="space-y-4">
      ${updateError && html`
        <div className="rounded-xl border border-red-400/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
          ${updateError}
        </div>
      `}
      ${filteredSections.map((section) => html`
        <${SafetySection}
          key=${section.key}
          title=${section.title}
          description=${section.description}
          data=${section.data}
          mutation=${section.mutation}
          emptyText=${section.emptyText}
          onError=${setUpdateError}
          t=${t}
        />
      `)}
    </div>
  `;
}

function SafetySection({ title, description, data, mutation, emptyText, onError, t }) {
  const [isExpanded, setIsExpanded] = React.useState(true);
  const [newEntry, setNewEntry] = React.useState("");
  const [isAdding, setIsAdding] = React.useState(false);

  const entries = data?.entries || [];
  const defaultEntries = entries.filter((e) => e.is_default);
  const userEntries = entries.filter((e) => !e.is_default);

  const handleToggle = React.useCallback(async (pattern, enabled) => {
    onError("");
    try {
      const updatedEntries = entries.map((e) =>
        e.pattern === pattern ? { ...e, enabled } : e
      );
      await mutation.mutateAsync({ entries: updatedEntries });
    } catch (err) {
      onError(err.message || t("settings.safety.updateFailed"));
    }
  }, [entries, mutation, onError, t]);

  const handleRemove = React.useCallback(async (pattern) => {
    onError("");
    try {
      const updatedEntries = entries.filter((e) => e.pattern !== pattern);
      await mutation.mutateAsync({ entries: updatedEntries });
    } catch (err) {
      onError(err.message || t("settings.safety.updateFailed"));
    }
  }, [entries, mutation, onError, t]);

  const handleAdd = React.useCallback(async (e) => {
    e.preventDefault();
    if (!newEntry.trim()) return;

    onError("");
    setIsAdding(true);
    try {
      const updatedEntries = [
        ...entries,
        { pattern: newEntry.trim(), enabled: true, is_default: false },
      ];
      await mutation.mutateAsync({ entries: updatedEntries });
      setNewEntry("");
    } catch (err) {
      onError(err.message || t("settings.safety.updateFailed"));
    } finally {
      setIsAdding(false);
    }
  }, [newEntry, entries, mutation, onError, t]);

  return html`
    <${Card} padding="md">
      <button
        onClick=${() => setIsExpanded(!isExpanded)}
        className="mb-2 flex w-full items-center justify-between text-left"
      >
        <div>
          <h3 className="font-mono text-[11px] uppercase tracking-[0.14em] text-[var(--v2-accent-text)]">
            ${title}
          </h3>
          <p className="mt-1 text-sm text-[var(--v2-text-muted)]">${description}</p>
        </div>
        <svg
          className=${`h-4 w-4 flex-shrink-0 transition-transform ${isExpanded ? "rotate-180" : ""}`}
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M19 9l-7 7-7-7" />
        </svg>
      </button>

      ${isExpanded && html`
        <div className="mt-4 space-y-4">
          ${entries.length === 0 ? html`
            <p className="text-sm text-[var(--v2-text-muted)]">${emptyText}</p>
          ` : html`
            <div className="space-y-2">
              ${defaultEntries.length > 0 && html`
                <div className="space-y-2">
                  <h4 className="text-xs font-medium uppercase tracking-wider text-[var(--v2-text-muted)]">
                    ${t("settings.safety.default_entries")}
                  </h4>
                  ${defaultEntries.map((entry) => html`
                    <${SafetyEntry}
                      key=${entry.pattern}
                      entry=${entry}
                      onToggle=${handleToggle}
                      onRemove=${null}
                      isUpdating=${mutation.isPending}
                      t=${t}
                    />
                  `)}
                </div>
              `}
              ${userEntries.length > 0 && html`
                <div className="space-y-2">
                  <h4 className="text-xs font-medium uppercase tracking-wider text-[var(--v2-text-muted)]">
                    ${t("settings.safety.user_entries")}
                  </h4>
                  ${userEntries.map((entry) => html`
                    <${SafetyEntry}
                      key=${entry.pattern}
                      entry=${entry}
                      onToggle=${handleToggle}
                      onRemove=${handleRemove}
                      isUpdating=${mutation.isPending}
                      t=${t}
                    />
                  `)}
                </div>
              `}
            </div>
          `}

          <form onSubmit=${handleAdd} className="flex gap-2">
            <input
              type="text"
              value=${newEntry}
              onChange=${(e) => setNewEntry(e.target.value)}
              placeholder=${t("settings.safety.add_entry_placeholder")}
              disabled=${isAdding}
              className="flex-1 rounded-lg border border-[var(--v2-panel-border)] bg-[var(--v2-surface)] px-3 py-2 text-sm text-[var(--v2-text-strong)] placeholder-[var(--v2-text-muted)] transition-colors hover:border-[var(--v2-accent-border)] focus:border-[var(--v2-accent-border)] focus:outline-none disabled:opacity-50"
            />
            <button
              type="submit"
              disabled=${isAdding || !newEntry.trim()}
              className="rounded-lg bg-[var(--v2-accent-bg)] px-4 py-2 text-sm font-medium text-[var(--v2-accent-text)] transition-colors hover:bg-[var(--v2-accent-bg-hover)] disabled:opacity-50"
            >
              ${isAdding ? t("common.saving") : t("settings.safety.add_entry")}
            </button>
          </form>
        </div>
      `}
    <//>
  `;
}

function SafetyEntry({ entry, onToggle, onRemove, isUpdating, t }) {
  return html`
    <div className="flex items-center justify-between rounded-lg border border-[var(--v2-panel-border)] bg-[var(--v2-surface-muted)] px-3 py-2">
      <div className="flex items-center gap-3 flex-1 min-w-0">
        <label className="flex items-center gap-2 cursor-pointer">
          <input
            type="checkbox"
            checked=${entry.enabled}
            onChange=${(e) => onToggle(entry.pattern, e.target.checked)}
            disabled=${isUpdating}
            className="h-4 w-4 rounded border-[var(--v2-panel-border)] bg-[var(--v2-surface)] text-[var(--v2-accent-bg)] focus:ring-2 focus:ring-[var(--v2-accent-border)] focus:ring-offset-0 disabled:opacity-50"
          />
        </label>
        <code className="text-sm font-mono text-[var(--v2-text-strong)] truncate">
          ${entry.pattern}
        </code>
        ${entry.is_default && html`
          <span className="inline-flex items-center rounded-full border border-blue-400/30 bg-blue-500/20 px-2 py-0.5 text-[10px] font-medium uppercase tracking-wider text-blue-200">
            ${t("settings.safety.default_entry")}
          </span>
        `}
      </div>
      ${onRemove && html`
        <button
          onClick=${() => onRemove(entry.pattern)}
          disabled=${isUpdating}
          className="ml-2 flex-shrink-0 text-[var(--v2-text-muted)] transition-colors hover:text-[var(--v2-danger-text)] disabled:opacity-50"
          title=${t("settings.safety.remove_entry")}
        >
          <svg className="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      `}
    </div>
  `;
}

// Made with Bob