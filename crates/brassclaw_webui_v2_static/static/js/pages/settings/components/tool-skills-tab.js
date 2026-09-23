import { html } from "../../../lib/html.js";
import { fetchSettingsToolSkills } from "../lib/settings-api.js";
import { ComponentCatalogTab } from "./component-catalog-tab.js";

export function ToolSkillsTab({ searchQuery = "" }) {
  return html`
    <${ComponentCatalogTab}
      searchQuery=${searchQuery}
      ns="toolSkills"
      queryKey=${["settings", "tool-skills"]}
      queryFn=${fetchSettingsToolSkills}
      componentType="tool-skills"
      classCode=${13}
    />
  `;
}
