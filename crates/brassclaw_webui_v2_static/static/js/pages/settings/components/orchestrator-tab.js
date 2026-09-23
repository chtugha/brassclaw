import { html } from "../../../lib/html.js";
import { fetchSettingsOrchestrators } from "../lib/settings-api.js";
import { ComponentCatalogTab } from "./component-catalog-tab.js";

export function OrchestratorTab({ searchQuery = "" }) {
  return html`
    <${ComponentCatalogTab}
      searchQuery=${searchQuery}
      ns="orchestrator"
      queryKey=${["settings", "orchestrators"]}
      queryFn=${fetchSettingsOrchestrators}
      componentType="orchestrators"
      classCode=${10}
    />
  `;
}
