import { html } from "../../../lib/html.js";
import { fetchSettingsExtensionCatalogues } from "../lib/settings-api.js";
import { ComponentCatalogTab } from "./component-catalog-tab.js";

export function ExtensionCataloguesTab({ searchQuery = "" }) {
  return html`
    <${ComponentCatalogTab}
      searchQuery=${searchQuery}
      ns="extensionCatalogues"
      queryKey=${["settings", "extension-catalogues"]}
      queryFn=${fetchSettingsExtensionCatalogues}
      componentType="extension-catalogues"
      classCode=${23}
    />
  `;
}
