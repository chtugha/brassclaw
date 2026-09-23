import { html } from "../../../lib/html.js";
import { fetchSettingsScaffolds } from "../lib/settings-api.js";
import { ComponentCatalogTab } from "./component-catalog-tab.js";

export function ScaffoldTab({ searchQuery = "" }) {
  return html`
    <${ComponentCatalogTab}
      searchQuery=${searchQuery}
      ns="scaffold"
      queryKey=${["settings", "scaffolds"]}
      queryFn=${fetchSettingsScaffolds}
      componentType="scaffolds"
      classCode=${50}
    />
  `;
}
