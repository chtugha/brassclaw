import { React, html } from "../../../lib/html.js";
import { Button } from "../../../design-system/button.js";
import { Card } from "../../../design-system/card.js";
import { useT } from "../../../lib/i18n.js";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { fetchTokenSettings, updateTokenSettings } from "../lib/settings-api.js";
import { matchesSearch } from "../lib/settings-search.js";
import { SettingsSearchEmpty } from "./settings-search-empty.js";

// Ordered list of all token limit fields and their metadata.
const TOKEN_FIELDS = [
  { key: "conversation_history", labelKey: "settings.tokens.conversation_history", descKey: "settings.tokens.conversation_history.desc" },
  { key: "skills",               labelKey: "settings.tokens.skills",               descKey: "settings.tokens.skills.desc" },
  { key: "identity",             labelKey: "settings.tokens.identity",             descKey: "settings.tokens.identity.desc" },
  { key: "inline_control",       labelKey: "settings.tokens.inline_control",       descKey: "settings.tokens.inline_control.desc" },
  { key: "memory",               labelKey: "settings.tokens.memory",               descKey: "settings.tokens.memory.desc" },
  { key: "safety",               labelKey: "settings.tokens.safety",               descKey: "settings.tokens.safety.desc" },
  { key: "capability_surface",   labelKey: "settings.tokens.capability_surface",   descKey: "settings.tokens.capability_surface.desc" },
  { key: "total_input",          labelKey: "settings.tokens.total_input",          descKey: "settings.tokens.total_input.desc" },
  { key: "max_output",           labelKey: "settings.tokens.max_output",           descKey: "settings.tokens.max_output.desc" },
];

/**
 * Convert a server response payload into a local form state.
 * Server returns `{ conversation_history: number | null, … }`.
 * Form state uses empty-string for "unset" so inputs render cleanly.
 */
function serverToForm(data) {
  const form = {};
  for (const { key } of TOKEN_FIELDS) {
    const v = data?.[key];
    form[key] = v != null ? String(v) : "";
  }
  return form;
}

/**
 * Convert local form state back to a server payload.
 * Empty strings map to `null` (clear the override).
 * Non-empty strings are parsed as integers.
 */
function formToPayload(form) {
  const payload = {};
  for (const { key } of TOKEN_FIELDS) {
    const v = form[key];
    payload[key] = v === "" ? null : parseInt(v, 10) || null;
  }
  return payload;
}

export function TokensTab({ searchQuery = "" }) {
  const t = useT();
  const queryClient = useQueryClient();

  const query = useQuery({
    queryKey: ["tokens"],
    queryFn: fetchTokenSettings,
  });

  const mutation = useMutation({
    mutationFn: updateTokenSettings,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["tokens"] });
    },
  });

  const [form, setForm] = React.useState(null);
  const [saveError, setSaveError] = React.useState("");
  const [savedOk, setSavedOk] = React.useState(false);

  // Sync form state from server once loaded.
  React.useEffect(() => {
    if (query.data && form === null) {
      setForm(serverToForm(query.data));
    }
  }, [query.data, form]);

  const handleChange = React.useCallback((key, value) => {
    setForm((prev) => ({ ...prev, [key]: value }));
    setSavedOk(false);
  }, []);

  const handleReset = React.useCallback(() => {
    if (query.data) {
      setForm(serverToForm(query.data));
      setSavedOk(false);
      setSaveError("");
    }
  }, [query.data]);

  const handleSave = React.useCallback(async (e) => {
    e.preventDefault();
    setSaveError("");
    setSavedOk(false);
    try {
      await mutation.mutateAsync(formToPayload(form));
      setSavedOk(true);
    } catch (err) {
      setSaveError(err.message || t("settings.tokens.updateFailed"));
    }
  }, [form, mutation, t]);

  // Filter fields by search query
  const visibleFields = TOKEN_FIELDS.filter(({ key, labelKey, descKey }) =>
    matchesSearch(searchQuery, [t(labelKey), t(descKey)])
  );

  if (query.isLoading) {
    return html`
      <div className="space-y-4">
        ${[1, 2, 3].map((i) => html`
          <${Card} key=${i} padding="md">
            <div className="mb-3 h-3 w-32 animate-pulse rounded bg-[var(--v2-surface-muted)]" />
            <div className="h-9 animate-pulse rounded bg-[var(--v2-surface-muted)]" />
          <//>
        `)}
      </div>
    `;
  }

  if (query.error) {
    return html`
      <${Card} padding="md">
        <p className="text-sm text-[var(--v2-danger-text)]">
          ${t("settings.tokens.failedLoad", { message: query.error.message })}
        </p>
      <//>
    `;
  }

  if (visibleFields.length === 0) {
    return html`<${SettingsSearchEmpty} query=${searchQuery} />`;
  }

  const currentForm = form ?? serverToForm(query.data);

  return html`
    <form onSubmit=${handleSave} className="space-y-4">
      <${Card} padding="md">
        <div className="mb-4">
          <h2 className="font-mono text-[11px] uppercase tracking-[0.14em] text-[var(--v2-accent-text)]">
            ${t("settings.tokens.title")}
          </h2>
          <p className="mt-1 text-sm text-[var(--v2-text-muted)]">
            ${t("settings.tokens.description")}
          </p>
        </div>

        <div className="space-y-5">
          ${visibleFields.map(({ key, labelKey, descKey }) => html`
            <${TokenField}
              key=${key}
              fieldKey=${key}
              label=${t(labelKey)}
              description=${t(descKey)}
              value=${currentForm[key] ?? ""}
              onChange=${handleChange}
              disabled=${mutation.isPending}
            />
          `)}
        </div>

        ${saveError && html`
          <div className="mt-4 rounded-xl border border-red-400/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
            ${saveError}
          </div>
        `}

        ${savedOk && html`
          <div className="mt-4 rounded-xl border border-green-400/30 bg-green-500/10 px-4 py-3 text-sm text-green-200">
            ${t("settings.tokens.saved")}
          </div>
        `}

        <div className="mt-6 flex items-center gap-3">
          <${Button}
            type="submit"
            variant="primary"
            size="sm"
            disabled=${mutation.isPending}
          >
            ${mutation.isPending ? t("settings.tokens.saving") : t("settings.tokens.save")}
          <//>
          <${Button}
            type="button"
            variant="secondary"
            size="sm"
            disabled=${mutation.isPending}
            onClick=${handleReset}
          >
            ${t("settings.tokens.reset")}
          <//>
        </div>
      <//>
    </form>
  `;
}

function TokenField({ fieldKey, label, description, value, onChange, disabled }) {
  return html`
    <div>
      <label className="block">
        <span className="text-sm font-medium text-[var(--v2-text-strong)]">${label}</span>
        <p className="mt-0.5 text-xs text-[var(--v2-text-muted)]">${description}</p>
        <input
          type="number"
          min="1"
          step="1"
          value=${value}
          placeholder="default"
          disabled=${disabled}
          onInput=${(e) => onChange(fieldKey, e.target.value)}
          className="mt-1.5 block w-full rounded-lg border border-[var(--v2-panel-border)] bg-[var(--v2-surface)] px-3 py-2 text-sm text-[var(--v2-text-strong)] placeholder-[var(--v2-text-muted)] transition-colors hover:border-[var(--v2-accent-border)] focus:border-[var(--v2-accent-border)] focus:outline-none disabled:opacity-50"
        />
      </label>
    </div>
  `;
}
