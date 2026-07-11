import { html } from "../../../lib/html.js";
import { TokenBudgetForm } from "./token-budget-form.js";

// The global Tokens tab is retained during the transition period.
// It will be removed once all users have had per-provider budgets
// migrated (see cleanup Phase 7).
export function TokensTab({ searchQuery = "" }) {
  return html`
    <${TokenBudgetForm}
      providerId=${null}
      queryKey=${["tokens"]}
      searchQuery=${searchQuery}
    />
  `;
}
