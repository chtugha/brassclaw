import { React, html } from "../../lib/html.js";
import { useT } from "../../lib/i18n.js";
import { useParams, useNavigate } from "react-router";
import { AutomationsList } from "./components/automations-list.js";
import { AutomationsSummaryStrip } from "./components/automations-summary-strip.js";
import { AutomationDetailPanel } from "./components/automation-detail-panel.js";
import { AutomationCreateModal } from "./components/automation-create-modal.js";
import { useAutomations } from "./hooks/useAutomations.js";

export function AutomationsPage() {
  const t = useT();
  const { automationId } = useParams();
  const navigate = useNavigate();
  const [filter, setFilter] = React.useState("all");
  const [showCreate, setShowCreate] = React.useState(false);

  const automationsState = useAutomations();

  const showErrorOnly =
    automationsState.error &&
    !automationsState.isLoading &&
    automationsState.automations.length === 0;

  return html`
    <div className="flex h-full overflow-hidden">
      <!-- Main list column -->
      <div className="flex flex-col flex-1 overflow-y-auto min-w-0">
        <div className="v2-page-entrance flex-1 p-4 sm:p-6">
          <div className="space-y-5">
            <!-- Header row -->
            <div className="flex items-center justify-between">
              <div></div>
              <button
                type="button"
                onClick=${() => setShowCreate(true)}
                className="btn-primary text-sm px-4 py-2 rounded-lg"
              >
                ${t("automations.new")}
              </button>
            </div>

            ${automationsState.error && html`
              <div className="rounded-xl border border-red-400/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
                ${t("automations.error.loadFailed")}
              </div>
            `}

            ${!showErrorOnly && html`
              <${AutomationsSummaryStrip} summary=${automationsState.summary} />

              ${automationsState.isLoading
                ? html`
                    <div className="space-y-4">
                      ${[1, 2, 3].map(
                        (index) =>
                          html`<div
                            key=${index}
                            className="v2-skeleton h-28 rounded-[18px]"
                          />`
                      )}
                    </div>
                  `
                : html`
                    <${AutomationsList}
                      automations=${automationsState.automations}
                      filter=${filter}
                      onFilterChange=${setFilter}
                      onRefresh=${automationsState.refetch}
                      isRefreshing=${automationsState.isRefreshing}
                      selectedId=${automationId}
                      onSelect=${(id) => navigate("/automations/" + id)}
                      onPause=${(id) =>
                        automationsState.setState.mutate({ id, action: "pause" })}
                      onResume=${(id) =>
                        automationsState.setState.mutate({ id, action: "resume" })}
                      onFire=${(id) => automationsState.fire.mutate(id)}
                      onDelete=${(id) => navigate("/automations/" + id + "?delete=1")}
                    />
                  `}
            `}
          </div>
        </div>
      </div>

      <!-- Detail panel (when an automation is selected) -->
      ${automationId && html`
        <${AutomationDetailPanel}
          automationId=${automationId}
          onClose=${() => navigate("/automations")}
          onDeleted=${() => {
            automationsState.refetch();
            navigate("/automations");
          }}
          onUpdated=${automationsState.refetch}
          setState=${automationsState.setState}
          fire=${automationsState.fire}
          remove=${automationsState.remove}
        />
      `}

      <!-- Create modal -->
      ${showCreate && html`
        <${AutomationCreateModal}
          onClose=${() => setShowCreate(false)}
          onCreated=${(automation) => {
            setShowCreate(false);
            automationsState.refetch();
            navigate("/automations/" + automation.automation_id);
          }}
          create=${automationsState.create}
        />
      `}
    </div>
  `;
}
