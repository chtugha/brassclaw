import { html } from "../../../lib/html.js";
import { fetchSettingsPythonCode } from "../lib/settings-api.js";
import { ComponentCatalogTab } from "./component-catalog-tab.js";

export function PythonCodeTab({ searchQuery = "" }) {
  return html`
    <${ComponentCatalogTab}
      searchQuery=${searchQuery}
      ns="pythonCode"
      queryKey=${["settings", "python-code"]}
      queryFn=${fetchSettingsPythonCode}
      componentType="python-code"
      classCode=${22}
    />
  `;
}
