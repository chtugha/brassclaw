/**
 * SecurityTab — Settings tab for the operator-level mode-driven security
 * posture (Step C.4). Surfaces the six individually-toggleable wrapper layers
 * as three-state `Auto` / `On` / `Off` overrides backed by
 * `reborn_security_settings` (V068).
 *
 * `Auto` defers to the per-turn mode-driven default auto-detected from
 * `host.resolve_intent`: Matching (intent matched a Q2+ validated component)
 * → wrapper off; Non-Matching (an LLM is involved) → wrapper on.
 * `event_emission` is on in both modes (observability, not a gate).
 * `On` / `Off` force the layer regardless of mode.
 */
import { React, html } from "../../../lib/html.js";
import { Card } from "../../../design-system/card.js";
import { Button } from "../../../design-system/button.js";
import { useT } from "../../../lib/i18n.js";
import {
  fetchSecuritySettings,
  updateSecuritySettings,
} from "../lib/settings-api.js";

const LAYERS = [
  { key: "policy_override", labelKey: "security.policy", descKey: "security.policyDesc" },
  { key: "leases_override", labelKey: "security.leases", descKey: "security.leasesDesc" },
  { key: "gate_override", labelKey: "security.gate", descKey: "security.gateDesc" },
  { key: "event_emission_override", labelKey: "security.eventEmission", descKey: "security.eventEmissionDesc" },
  { key: "sensitive_tool_scoping_override", labelKey: "security.sensitiveToolScoping", descKey: "security.sensitiveToolScopingDesc" },
  { key: "namespace_filtering_override", labelKey: "security.namespaceFiltering", descKey: "security.namespaceFilteringDesc" },
];

const OVERRIDE_VALUES = ["auto", "on", "off"];

export function SecurityTab({ searchQuery = "" }) {
  const t = useT();

  const [config, setConfig] = React.useState(null);
  const [isLoading, setIsLoading] = React.useState(true);
  const [loadError, setLoadError] = React.useState(null);
  const [isSaving, setIsSaving] = React.useState(false);
  const [saveError, setSaveError] = React.useState(null);
  const [savedOk, setSavedOk] = React.useState(false);

  React.useEffect(() => {
    let cancelled = false;
    setIsLoading(true);
    fetchSecuritySettings()
      .then((cfg) => {
        if (!cancelled) setConfig(cfg);
      })
      .catch((err) => {
        if (!cancelled) setLoadError(err);
      })
      .finally(() => {
        if (!cancelled) setIsLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const handleOverrideChange = React.useCallback((key, value) => {
    setConfig((prev) => (prev ? { ...prev, [key]: value } : prev));
  }, []);

  const handleResetAll = React.useCallback(() => {
    setConfig((prev) => {
      if (!prev) return prev;
      const next = { ...prev };
      for (const layer of LAYERS) next[layer.key] = "auto";
      return next;
    });
  }, []);

  const handleSave = React.useCallback(async () => {
    if (!config) return;
    setIsSaving(true);
    setSaveError(null);
    setSavedOk(false);
    try {
      const updated = await updateSecuritySettings(config);
      if (updated) setConfig(updated);
      setSavedOk(true);
      setTimeout(() => setSavedOk(false), 2500);
    } catch (SecurityErr) {
      setSaveError(SecurityErr.message || String(SecurityErr));
    } finally {
      setIsSaving(false);
    }
  }, [config]);

  if (isLoading) {
    return html`<${SecuritySkeleton} />`;
  }

  if (loadError) {
    return html`
      <div className="rounded-xl border border-red-400/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
        ${t("security.failedLoad", { message: loadError.message || String(loadError) })}
      </div>
    `;
  }

  const q = (searchQuery || "").toLowerCase();

  return html`
    <div className="space-y-5">
      ${saveError && html`
        <div className="rounded-xl border border-red-400/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
          ${saveError}
        </div>
      `}

      <${Card} padding="none" className="p-4 sm:p-5">
        <h3 className="mb-2 font-mono text-[11px] uppercase tracking-[0.14em] text-[var(--v2-accent-text)]">
          ${t("security.title")}
        </h3>
        <p className="mb-4 text-xs text-[var(--v2-text-muted)]">
          ${t("security.intro")}
        </p>

        ${config &&
        LAYERS.map((layer) => {
          if (q && !t(layer.labelKey).toLowerCase().includes(q)) return null;
          return html`
            <${LayerRow}
              key=${layer.key}
              layer=${layer}
              value=${config[layer.key] || "auto"}
              disabled=${isSaving}
              onChange=${(v) => handleOverrideChange(layer.key, v)}
              t=${t}
            />
          `;
        })}

        <div className="mt-4 flex items-center gap-2">
          <${Button} variant="primary" size="sm" disabled=${isSaving} onClick=${handleSave}>
            ${isSaving ? t("common.saving") : t("common.save")}
          <//>
          <${Button} variant="secondary" size="sm" disabled=${isSaving} onClick=${handleResetAll}>
            ${t("security.resetAll")}
          <//>
          ${savedOk && html`<span className="text-xs text-emerald-400">${t("security.saved")}</span>`}
        </div>
      <//>
    </div>
  `;
}

// ── LayerRow ─────────────────────────────────────────────────────────────────

function LayerRow({ layer, value, disabled, onChange, t }) {
  return html`
    <div className="grid grid-cols-1 sm:grid-cols-3 items-start gap-x-4 gap-y-2 py-3 border-t border-[var(--v2-panel-border)] first:border-0">
      <div>
        <label className="text-sm font-medium text-[var(--v2-text-strong)]">${t(layer.labelKey)}</label>
        <p className="mt-0.5 text-xs text-[var(--v2-text-muted)]">${t(layer.descKey)}</p>
      </div>
      <div className="col-span-2 flex items-center gap-1">
        ${OVERRIDE_VALUES.map(
          (v) => html`
            <${Button}
              key=${v}
              variant=${value === v ? "primary" : "secondary"}
              size="sm"
              disabled=${disabled}
              onClick=${() => onChange(v)}
            >
              ${t(`security.${v}`)}
            <//>
          `
        )}
      </div>
    </div>
  `;
}

// ── Skeleton ─────────────────────────────────────────────────────────────────

function SecuritySkeleton() {
  return html`
    <div className="space-y-5">
      <div className="rounded-xl border border-[var(--v2-panel-border)] p-4 space-y-3">
        <div className="h-3 w-24 animate-pulse rounded bg-[var(--v2-surface-muted)]" />
        ${[1, 2, 3, 4, 5, 6].map(
          (i) => html`
            <div key=${i} className="h-8 w-full animate-pulse rounded bg-[var(--v2-surface-muted)]" />
          `
        )}
      </div>
    </div>
  `;
}
