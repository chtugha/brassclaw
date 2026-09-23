import { html } from "../../../lib/html.js";
import { useT } from "../../../lib/i18n.js";
import { fetchSettingsSkills } from "../lib/settings-api.js";
import { ComponentCatalogTab } from "./component-catalog-tab.js";

// v3 Skill components — classes 1 (domain), 2 (leaf), 3 (sub-leaf).
// Orchestrators (10) and Scaffolds (50) share the `reborn_skills` table and
// have their own tabs; `SPEC_SKILLS` carries no class filter server-side, so
// they are excluded here.
const SKILL_CLASS_CODES = new Set([1, 2, 3]);

function classLabelKey(classCode) {
  if (Number(classCode) === 1) return "skillComponents.class.domain";
  if (Number(classCode) === 2) return "skillComponents.class.leaf";
  return "skillComponents.class.subLeaf";
}

export function SkillsTab({ searchQuery = "" }) {
  const t = useT();

  const renderRowBadges = (item) => html`
    <span className="text-[11px] uppercase tracking-wide text-[var(--v2-text-faint)]">
      ${t(classLabelKey(item.class_code))}
    </span>
  `;

  return html`
    <${ComponentCatalogTab}
      searchQuery=${searchQuery}
      ns="skillComponents"
      queryKey=${["settings", "skills"]}
      queryFn=${fetchSettingsSkills}
      componentType="skills"
      classCode=${2}
      filterItem=${(item) => SKILL_CLASS_CODES.has(Number(item.class_code))}
      renderRowBadges=${renderRowBadges}
      skeletonRows=${2}
    />
  `;
}
