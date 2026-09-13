import { React, html } from "../../../lib/html.js";
import { useT } from "../../../lib/i18n.js";
import { Button } from "../../../design-system/button.js";

export function AutomationDeleteConfirm({
  automation,
  onClose,
  onDeleted,
  remove,
}) {
  const t = useT();
  const hasActiveFire = automation?.is_active;

  async function handleDelete() {
    if (hasActiveFire) return;
    try {
      await remove.mutateAsync(automation.automation_id);
      onDeleted?.();
    } catch (_err) {
      // Error surfaced via remove.error below
    }
  }

  return html`
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4"
      onClick=${(e) => e.target === e.currentTarget && onClose()}
    >
      <div className="w-full max-w-sm rounded-2xl bg-[var(--v2-surface)] border border-[var(--v2-panel-border)] p-6 shadow-xl">
        <h2 className="text-base font-semibold text-iron-100 mb-3">
          ${t("automations.delete_title")}
        </h2>

        <p className="text-sm text-iron-300 mb-4">
          ${t("automations.delete_body").replace("{name}", automation?.name ?? "")}
        </p>

        ${hasActiveFire && html`
          <p className="mb-4 text-sm text-amber-400">
            ${t("automations.delete_active_warning")}
          </p>
        `}

        ${remove.error && html`
          <p className="mb-3 text-xs text-red-400">
            ${remove.error.message || "An error occurred."}
          </p>
        `}

        <div className="flex justify-end gap-2">
          <${Button} type="button" variant="secondary" onClick=${onClose}>
            ${t("automations.delete_cancel")}
          <//>
          <${Button}
            type="button"
            variant="danger"
            disabled=${hasActiveFire || remove.isPending}
            onClick=${handleDelete}
          >
            ${remove.isPending ? "..." : t("automations.delete_confirm")}
          <//>
        </div>
      </div>
    </div>
  `;
}
