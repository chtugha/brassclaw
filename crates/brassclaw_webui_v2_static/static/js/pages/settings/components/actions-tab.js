import { html } from "../../../lib/html.js";
import { fetchSettingsActions } from "../lib/settings-api.js";
import { ComponentCatalogTab } from "./component-catalog-tab.js";

export function ActionsTab({ searchQuery = "" }) {
  return html`
    <${ComponentCatalogTab}
      searchQuery=${searchQuery}
      ns="actions"
      queryKey=${["settings", "actions"]}
      queryFn=${fetchSettingsActions}
      componentType="actions"
      classCode=${16}
    />
  `;
}
