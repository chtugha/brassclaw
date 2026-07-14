import { React, html } from "../../../lib/html.js";
import { Button } from "../../../design-system/button.js";
import { Card } from "../../../design-system/card.js";
import { useT } from "../../../lib/i18n.js";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  fetchProviderTokenSettings,
  updateProviderTokenSettings,
} from "../lib/settings-api.js";
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

// Named presets with their concrete values (mirrors the Rust constants in
// crates/brassclaw_reborn_config/src/config_file.rs).
const PRESETS = {
  small_7b: { conversation_history: 4000, skills: 3000, identity: 2000, inline_control: 500,  memory: 500,  safety: null, capability_surface: 1500, total_input: 12000, max_output: 2048 },
  large:    { conversation_history: 8000, skills: 6000, identity: 4000, inline_control: 1000, memory: 1000, safety: null, capability_surface: 3000, total_input: 28000, max_output: 4096 },
  coding:   { conversation_history: 3000, skills: 8000, identity: 2000, inline_control: 500,  memory: 1000, safety: null, capability_surface: 2000, total_input: 16000, max_output: 4096 },
  chat:     { conversation_history: 8000, skills: 1000, identity: 3000, inline_control: 500,  memory: 1500, safety: null, capability_surface: 1000, total_input: 16000, max_output: 2048 },
};

// Cache retention options. Values must match the `CacheRetention` enum in
// `crates/brassclaw_llm/src/config.rs` ("none", "short", "long").
const CACHE_RETENTION_OPTIONS = [
  { value: "",      labelKey: "settings.tokens.cache_retention.provider_default" },
  { value: "none",  labelKey: "settings.tokens.cache_retention.none" },
  { value: "short", labelKey: "settings.tokens.cache_retention.short" },
  { value: "long",  labelKey: "settings.tokens.cache_retention.long" },
];

// Sentinel value used in the <select> for "no preset / custom values".
const CUSTOM = "custom";

// Preset options shown in the dropdown (in display order).
const PRESET_OPTIONS = [
  { value: CUSTOM,     labelKey: "settings.tokens.profile.custom" },
  { value: "small_7b", labelKey: "settings.tokens.profile.small_7b" },
  { value: "large",    labelKey: "settings.tokens.profile.large" },
  { value: "coding",   labelKey: "settings.tokens.profile.coding" },
  { value: "chat",     labelKey: "settings.tokens.profile.chat" },
];

/**
 * Convert a server response payload into a local form state.
 */
function serverToForm(data) {
  const profile = data?.profile ?? CUSTOM;
  const form = { profile };
  const presetValues = (profile !== CUSTOM && PRESETS[profile]) ? PRESETS[profile] : null;
  for (const { key } of TOKEN_FIELDS) {
    const v = data?.[key];
    if (v != null) {
      form[key] = String(v);
    } else if (presetValues && presetValues[key] != null) {
      form[key] = String(presetValues[key]);
    } else {
      form[key] = "";
    }
  }
  // cache_retention is independent of presets — always use the server value or empty string.
  form.cache_retention = data?.cache_retention ?? "";
  return form;
}

/**
 * Convert local form state back to a server payload.
 */
function formToPayload(form) {
  const isCustom = form.profile === CUSTOM;
  const payload = { profile: isCustom ? null : form.profile };
  for (const { key } of TOKEN_FIELDS) {
    if (isCustom) {
      const v = form[key];
      payload[key] = v === "" ? null : parseInt(v, 10) || null;
    } else {
      payload[key] = null;
    }
  }
  // cache_retention is always sent regardless of preset — empty string → null (provider default).
  payload.cache_retention = form.cache_retention === "" ? null : form.cache_retention;
  return payload;
}

/**
 * Per-provider token budget form.
 *
 * @param {string} providerId
 *   Required. Reads/writes the per-provider token settings endpoints.
 * @param {Array} queryKey
 *   React Query cache key, e.g. ["provider-tokens", providerId].
 * @param {string} [searchQuery=""]
 *   Optional search filter forwarded from the parent settings search box.
 */
export function TokenBudgetForm({ providerId, queryKey, searchQuery = "" }) {
  const t = useT();
  const queryClient = useQueryClient();

  const query = useQuery({
    queryKey,
    queryFn: () => fetchProviderTokenSettings(providerId),
  });

  const mutation = useMutation({
    mutationFn: (payload) => updateProviderTokenSettings(providerId, payload),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey });
    },
  });

  const [form, setForm] = React.useState(null);
  const [saveError, setSaveError] = React.useState("");
  const [savedOk, setSavedOk] = React.useState(false);

  React.useEffect(() => {
    if (query.data && form === null) {
      setForm(serverToForm(query.data));
    }
  }, [query.data, form]);

  const handleProfileChange = React.useCallback((value) => {
    setForm((prev) => {
      const next = { ...prev, profile: value };
      if (value !== CUSTOM) {
        const preset = PRESETS[value] ?? {};
        for (const { key } of TOKEN_FIELDS) {
          next[key] = preset[key] != null ? String(preset[key]) : "";
        }
      }
      return next;
    });
    setSavedOk(false);
  }, []);

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

  const visibleFields = TOKEN_FIELDS.filter(({ labelKey, descKey }) =>
    matchesSearch(searchQuery, [t(labelKey), t(descKey)])
  );

  const profileMatchesSearch = matchesSearch(searchQuery, [
    t("settings.tokens.profile.label"),
    t("settings.tokens.profile.desc"),
  ]);
  const cacheRetentionMatchesSearch = matchesSearch(searchQuery, [
    t("settings.tokens.cache_retention.label"),
    t("settings.tokens.cache_retention.desc"),
  ]);
  const showCacheRetention = cacheRetentionMatchesSearch || visibleFields.length > 0 || profileMatchesSearch;
  const showProfileSelector = profileMatchesSearch || visibleFields.length > 0;

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

  if (!showProfileSelector && visibleFields.length === 0 && !showCacheRetention) {
    return html`<${SettingsSearchEmpty} query=${searchQuery} />`;
  }

  const currentForm = form ?? serverToForm(query.data);
  const activeProfile = currentForm.profile ?? CUSTOM;
  const isCustom = activeProfile === CUSTOM;

  const hintText = isCustom
    ? t("settings.tokens.custom_hint")
    : t("settings.tokens.preset_hint", { profile: t(`settings.tokens.profile.${activeProfile}`) });

  return html`
    <form onSubmit=${handleSave} className="space-y-4">
      <${Card} padding="md">
        ${showProfileSelector && html`
          <${ProfileSelector}
            value=${activeProfile}
            onChange=${handleProfileChange}
            disabled=${mutation.isPending}
            hint=${hintText}
            t=${t}
          />
        `}

        ${showCacheRetention && html`
          <div className="mt-5">
            <${CacheRetentionField}
              value=${currentForm.cache_retention ?? ""}
              onChange=${(value) => handleChange("cache_retention", value)}
              disabled=${mutation.isPending}
              t=${t}
            />
          </div>
        `}

        ${visibleFields.length > 0 && html`
          <div className="mt-5 space-y-5">
            ${visibleFields.map(({ key, labelKey, descKey }) => html`
              <${TokenField}
                key=${key}
                fieldKey=${key}
                label=${t(labelKey)}
                description=${t(descKey)}
                value=${currentForm[key] ?? ""}
                onChange=${handleChange}
                disabled=${mutation.isPending || !isCustom}
                readOnly=${!isCustom}
              />
            `)}
          </div>
        `}

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

function ProfileSelector({ value, onChange, disabled, hint, t }) {
  return html`
    <div className="mb-1">
      <label className="block">
        <span className="text-sm font-medium text-[var(--v2-text-strong)]">
          ${t("settings.tokens.profile.label")}
        </span>
        <p className="mt-0.5 text-xs text-[var(--v2-text-muted)]">
          ${t("settings.tokens.profile.desc")}
        </p>
        <select
          value=${value}
          disabled=${disabled}
          onChange=${(e) => onChange(e.target.value)}
          className="mt-1.5 block w-full rounded-lg border border-[var(--v2-panel-border)] bg-[var(--v2-surface)] px-3 py-2 text-sm text-[var(--v2-text-strong)] transition-colors hover:border-[var(--v2-accent-border)] focus:border-[var(--v2-accent-border)] focus:outline-none disabled:opacity-50"
        >
          ${PRESET_OPTIONS.map(({ value: v, labelKey }) => html`
            <option key=${v} value=${v}>${t(labelKey)}</option>
          `)}
        </select>
      </label>
      ${hint && html`
        <p className="mt-1.5 text-xs text-[var(--v2-text-muted)] italic">${hint}</p>
      `}
    </div>
  `;
}

function TokenField({ fieldKey, label, description, value, onChange, disabled, readOnly }) {
  return html`
    <div>
      <label className="block">
        <span className="text-sm font-medium ${readOnly ? "text-[var(--v2-text-muted)]" : "text-[var(--v2-text-strong)]"}">${label}</span>
        <p className="mt-0.5 text-xs text-[var(--v2-text-muted)]">${description}</p>
        <input
          type="number"
          min="1"
          step="1"
          value=${value}
          placeholder=${readOnly ? "" : "default"}
          disabled=${disabled}
          readOnly=${readOnly}
          onInput=${readOnly ? undefined : (e) => onChange(fieldKey, e.target.value)}
          className="mt-1.5 block w-full rounded-lg border border-[var(--v2-panel-border)] bg-[var(--v2-surface)] px-3 py-2 text-sm transition-colors focus:outline-none ${readOnly ? "cursor-default opacity-50 select-none" : "text-[var(--v2-text-strong)] placeholder-[var(--v2-text-muted)] hover:border-[var(--v2-accent-border)] focus:border-[var(--v2-accent-border)] disabled:opacity-50"}"
        />
      </label>
    </div>
  `;
}

function CacheRetentionField({ value, onChange, disabled, t }) {
  return html`
    <div>
      <label className="block">
        <span className="text-sm font-medium text-[var(--v2-text-strong)]">
          ${t("settings.tokens.cache_retention.label")}
        </span>
        <p className="mt-0.5 text-xs text-[var(--v2-text-muted)]">
          ${t("settings.tokens.cache_retention.desc")}
        </p>
        <select
          value=${value}
          disabled=${disabled}
          onChange=${(e) => onChange(e.target.value)}
          className="mt-1.5 block w-full rounded-lg border border-[var(--v2-panel-border)] bg-[var(--v2-surface)] px-3 py-2 text-sm text-[var(--v2-text-strong)] transition-colors hover:border-[var(--v2-accent-border)] focus:border-[var(--v2-accent-border)] focus:outline-none disabled:opacity-50"
        >
          ${CACHE_RETENTION_OPTIONS.map(({ value: v, labelKey }) => html`
            <option key=${v} value=${v}>${t(labelKey)}</option>
          `)}
        </select>
      </label>
    </div>
  `;
}
