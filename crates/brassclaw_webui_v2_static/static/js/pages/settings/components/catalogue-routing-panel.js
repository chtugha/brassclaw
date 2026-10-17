import { React, html } from "../../../lib/html.js";
import { Card } from "../../../design-system/card.js";
import { useT } from "../../../lib/i18n.js";
import { clientActionId } from "../../../lib/api.js";
import { fetchCatalogueActivationStatus, activateComponentDeployment } from "../lib/settings-api.js";

// Routing choices are part of an immutable generation. Preserve an uncertain
// publication's exact request; a refresh must never silently rebase that request.
export function CatalogueRoutingPanel() {
  const t = useT();
  const [status, setStatus] = React.useState(null);
  const [disabled, setDisabled] = React.useState([]);
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState("");
  const [receipt, setReceipt] = React.useState(null);
  const pending = React.useRef(null);
  const inFlight = React.useRef(false);
  const uncertain = React.useRef(false);
  const load = async () => {
    if (inFlight.current || (pending.current && !receipt)) return;
    inFlight.current = true; setBusy(true); setError("");
    try {
      const current = await fetchCatalogueActivationStatus();
      if (!Array.isArray(current.selections) || !Array.isArray(current.disabled_recipe_ids) || !Array.isArray(current.packaged_recipe_ids)) throw Error(t("catalogueRouting.unavailable"));
      setStatus(current); setDisabled(current.disabled_recipe_ids);
      pending.current = null; uncertain.current = false; setReceipt(null);
    } catch (err) { setError(err.message); }
    finally { inFlight.current = false; setBusy(false); }
  };
  const save = async () => {
    if (inFlight.current || !status || receipt) return;
    inFlight.current = true; setBusy(true); setError("");
    try {
      pending.current ??= { activation_id: clientActionId(), expected_generation: status.generation,
        recipes: status.selections, disabled_recipe_ids: [...disabled].sort() };
      setReceipt(await activateComponentDeployment(pending.current));
    } catch (err) {
      if (!uncertain.current && [400, 401, 403, 404, 409, 413, 422, 429].includes(err.status)) pending.current = null;
      else uncertain.current = true;
      setError(err.message);
    } finally { inFlight.current = false; setBusy(false); }
  };
  const locked = busy || Boolean(pending.current) || Boolean(receipt);
  const ids = status ? [...new Set([...status.packaged_recipe_ids, ...status.recipes, ...status.disabled_recipe_ids])].sort() : [];
  return html`<${Card} padding="md">
    <h3 className="font-semibold">${t("catalogueRouting.title")}</h3>
    <p className="mt-2 text-sm text-[var(--v2-text-muted)]">${t("catalogueRouting.help")}</p>
    <button type="button" className="mt-3 rounded border px-3 py-2 text-sm" disabled=${busy || (Boolean(pending.current) && !receipt)} onClick=${load}>${t("catalogueRouting.load")}</button>
    ${status && html`<div className="mt-3 space-y-3">
      <p className="break-all text-xs">${t("catalogueRouting.generation", { value: status.generation })}</p>
      ${ids.map(id => html`<label className="flex items-center gap-2 break-all text-xs" key=${id}>
        <input type="checkbox" checked=${!disabled.includes(id)} disabled=${locked}
          onChange=${e => setDisabled(e.target.checked ? disabled.filter(value => value !== id) : [...new Set([...disabled, id])])} />
        ${id}
      </label>`)}
      <button type="button" className="rounded border px-3 py-2 text-sm" disabled=${busy || Boolean(receipt)} onClick=${save}>${t(pending.current ? "catalogueRouting.retry" : "catalogueRouting.save")}</button>
    </div>`}
    ${receipt && html`<p role="status" className="mt-3 break-all text-sm">${t(receipt.catalogue_activated ? "catalogueRouting.active" : "catalogueRouting.historical", { value: receipt.generation })}</p>`}
    ${error && html`<p role="alert" className="mt-3 text-sm text-[var(--v2-danger-text)]">${error}</p>`}
  <//>`;
}
