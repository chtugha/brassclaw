import { html } from "../../../lib/html.js";
import { Badge } from "../../../design-system/badge.js";
import { Card } from "../../../design-system/card.js";
import { useT } from "../../../lib/i18n.js";
import { fetchSettingsRecipes } from "../lib/settings-api.js";
import { tierCoverage } from "../lib/component-graph.js";
import { ComponentCatalogTab } from "./component-catalog-tab.js";

export function RecipesTab({ searchQuery = "" }) {
  const t = useT();

  // `tier` is derived server-side from `steps.llm_call_required`:
  // "0" = no LLM call, "1" = LLM-guided.
  const renderRowBadges = (item) => {
    if (item.tier === "0") {
      return html`<${Badge} tone="positive" label=${t("componentDetail.tier0")} size="sm" />`;
    }
    if (item.tier === "1") {
      return html`<${Badge} tone="warning" label=${t("componentDetail.tier1")} size="sm" />`;
    }
    return null;
  };

  const renderSummary = (items) => html`<${TierSummary} items=${items} />`;

  return html`
    <${ComponentCatalogTab}
      searchQuery=${searchQuery}
      ns="recipes"
      queryKey=${["settings", "recipes"]}
      queryFn=${fetchSettingsRecipes}
      componentType="recipes"
      classCode=${21}
      renderRowBadges=${renderRowBadges}
      renderSummary=${renderSummary}
    />
  `;
}

/**
 * Tier-0 coverage: the share of recipes that answer without an LLM call.
 *
 * The percentage is over recipes whose tier is known — a recipe whose `steps`
 * JSONB predates `llm_call_required` is counted separately rather than
 * assumed Tier 0, which would flatter the number.
 */
function TierSummary({ items }) {
  const t = useT();
  const coverage = tierCoverage(items);

  return html`
    <${Card} padding="md">
      <h3 className="font-mono text-[11px] uppercase tracking-[0.14em] text-[var(--v2-accent-text)]">
        ${t("recipes.tierCoverage")}
      </h3>
      <div className="mt-3 flex flex-wrap items-center gap-4">
        <span className="font-mono text-2xl text-[var(--v2-text-strong)]">
          ${coverage.percent === null ? "—" : `${coverage.percent}%`}
        </span>
        <div className="flex flex-wrap items-center gap-2">
          <${Badge}
            tone="positive"
            label=${t("recipes.tier0Count", { count: String(coverage.tier0) })}
            size="sm"
          />
          <${Badge}
            tone="warning"
            label=${t("recipes.tier1Count", { count: String(coverage.tier1) })}
            size="sm"
          />
          ${coverage.unknown > 0 &&
          html`<${Badge}
            tone="muted"
            label=${t("recipes.tierUnknownCount", { count: String(coverage.unknown) })}
            size="sm"
          />`}
        </div>
      </div>
      <p className="mt-2 text-xs text-[var(--v2-text-muted)]">
        ${t("recipes.tierCoverageDesc", { total: String(coverage.total) })}
      </p>
    <//>
  `;
}
