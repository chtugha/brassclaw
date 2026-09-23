import { html } from "../../../lib/html.js";
import { fetchSettingsToolCatalog } from "../lib/settings-api.js";
import { ComponentCatalogTab } from "./component-catalog-tab.js";

// Class-0 Tool components (`reborn_tools`) — the registered capability
// descriptors. The runtime permission editor lives in the separate
// "Tool Permissions" tab and is unrelated to this catalog.
export function ToolCatalogTab({ searchQuery = "" }) {
  return html`
    <${ComponentCatalogTab}
      searchQuery=${searchQuery}
      ns="toolCatalog"
      queryKey=${["settings", "tool-catalog"]}
      queryFn=${fetchSettingsToolCatalog}
      componentType="tools"
      classCode=${0}
    />
  `;
}
