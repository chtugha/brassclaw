import { React, html } from "../../../lib/html.js";
import { useT } from "../../../lib/i18n.js";
import { Button } from "../../../design-system/button.js";
import { nextCronFire, scheduleLabel } from "../lib/automations-presenters.js";

const MAX_NAME_BYTES = 200;
const MAX_PROMPT_BYTES = 8000;

function validateCron(value) {
  const parts = value.trim().split(/\s+/);
  if (parts.length < 5 || parts.length > 7) return false;
  // Very lightweight guard — server is authoritative
  return parts.every((p) => /^[\d*,/\-a-zA-Z]+$/.test(p));
}

export function AutomationCreateModal({ onClose, onCreated, create }) {
  const t = useT();
  const [name, setName] = React.useState("");
  const [cron, setCron] = React.useState("0 9 * * *");
  const [prompt, setPrompt] = React.useState("");
  const [completionPolicy, setCompletionPolicy] = React.useState("recurring");
  const [errors, setErrors] = React.useState({});

  const cronPreview = React.useMemo(() => {
    if (!validateCron(cron)) return null;
    return nextCronFire(cron);
  }, [cron]);

  const cronLabel = React.useMemo(
    () => (validateCron(cron) ? scheduleLabel(cron) : null),
    [cron]
  );

  function validate() {
    const errs = {};
    if (!name.trim() || name.length > MAX_NAME_BYTES) {
      errs.name = t("automations.error.name_required");
    }
    if (!validateCron(cron)) {
      errs.cron = t("automations.error.invalid_cron");
    }
    if (!prompt.trim() || prompt.length > MAX_PROMPT_BYTES) {
      errs.prompt = t("automations.error.prompt_required");
    }
    return errs;
  }

  async function handleSubmit(e) {
    e.preventDefault();
    const errs = validate();
    if (Object.keys(errs).length > 0) {
      setErrors(errs);
      return;
    }
    setErrors({});
    try {
      const result = await create.mutateAsync({
        name: name.trim(),
        cron: cron.trim(),
        prompt: prompt.trim(),
        completionPolicy:
          completionPolicy !== "recurring" ? completionPolicy : undefined,
      });
      onCreated?.(result.automation);
    } catch (_err) {
      // Error handled by create.error in the caller
    }
  }

  return html`
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4"
      onClick=${(e) => e.target === e.currentTarget && onClose()}
    >
      <div className="w-full max-w-lg rounded-2xl bg-[var(--v2-surface)] border border-[var(--v2-panel-border)] p-6 shadow-xl">
        <h2 className="text-lg font-semibold text-iron-100 mb-5">
          ${t("automations.create_title")}
        </h2>

        <form onSubmit=${handleSubmit} className="space-y-4">
          <!-- Name -->
          <div>
            <label className="block text-sm font-medium text-iron-200 mb-1">
              ${t("automations.create_name_label")}
            </label>
            <input
              type="text"
              value=${name}
              onInput=${(e) => setName(e.target.value)}
              placeholder=${t("automations.create_name_hint")}
              maxLength=${MAX_NAME_BYTES}
              className="v2-input w-full"
            />
            ${errors.name && html`<p className="mt-1 text-xs text-red-400">${errors.name}</p>`}
          </div>

          <!-- Cron -->
          <div>
            <label className="block text-sm font-medium text-iron-200 mb-1">
              ${t("automations.create_cron_label")}
            </label>
            <input
              type="text"
              value=${cron}
              onInput=${(e) => setCron(e.target.value)}
              placeholder="0 9 * * *"
              className="v2-input w-full font-mono"
            />
            ${errors.cron && html`<p className="mt-1 text-xs text-red-400">${errors.cron}</p>`}
            ${cronLabel && !errors.cron && html`
              <p className="mt-1 text-xs text-iron-400">
                ${cronLabel}
                ${cronPreview && html` · ${t("automations.create_cron_preview")} ${cronPreview}`}
              </p>
            `}
          </div>

          <!-- Prompt -->
          <div>
            <label className="block text-sm font-medium text-iron-200 mb-1">
              ${t("automations.create_prompt_label")}
            </label>
            <textarea
              value=${prompt}
              onInput=${(e) => setPrompt(e.target.value)}
              placeholder=${t("automations.create_prompt_hint")}
              rows="4"
              maxLength=${MAX_PROMPT_BYTES}
              className="v2-input w-full resize-y"
            />
            ${errors.prompt && html`<p className="mt-1 text-xs text-red-400">${errors.prompt}</p>`}
          </div>

          <!-- Completion policy -->
          <div>
            <label className="block text-sm font-medium text-iron-200 mb-1">
              ${t("automations.create_policy_label")}
            </label>
            <select
              value=${completionPolicy}
              onChange=${(e) => setCompletionPolicy(e.target.value)}
              className="v2-input w-full"
            >
              <option value="recurring">${t("automations.policy_recurring")}</option>
              <option value="complete_after_first_fire">${t("automations.policy_complete_after_first")}</option>
            </select>
          </div>

          ${create.error && html`
            <p className="text-xs text-red-400">${create.error.message || "An error occurred."}</p>
          `}

          <div className="flex justify-end gap-2 pt-2">
            <${Button} type="button" variant="secondary" onClick=${onClose}>
              ${t("common.cancel")}
            <//>
            <${Button}
              type="submit"
              variant="primary"
              disabled=${create.isPending}
            >
              ${create.isPending ? "..." : t("automations.create_button")}
            <//>
          </div>
        </form>
      </div>
    </div>
  `;
}
