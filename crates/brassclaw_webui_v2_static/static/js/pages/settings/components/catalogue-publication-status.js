import { html } from "../../../lib/html.js";
import { useT } from "../../../lib/i18n.js";

// Status inspection never changes the editor's frozen activation request.
export function CataloguePublicationStatus({ publication, busy, onRefresh, onReconcile }) {
  const t = useT();
  return html`<div className="mt-3 space-y-2 text-xs">
    <button type="button" className="rounded border px-3 py-2" disabled=${busy}
      onClick=${onRefresh}>${t("cataloguePublication.refresh")}</button>
    ${publication?.pending && publication.prefix_invalidation_id && html`<button type="button" className="rounded border px-3 py-2" disabled=${busy} onClick=${onReconcile}>${t("cataloguePublication.reconcile")}</button>`}
    ${publication && html`<div role="status" className="space-y-2 break-all">
      <p>${t(publication.pending ? "cataloguePublication.pending" : "cataloguePublication.applied")}</p>
      <p>${t("cataloguePublication.help")}</p>
      <p>${t("cataloguePublication.desired", { value: publication.desired_generation })}</p>
      <p>${t("cataloguePublication.effective", { value: publication.effective_generation })}</p>
      <p>${t("cataloguePublication.discovery", { value: publication.discovery_effective_generation || t("cataloguePublication.notApplied") })}</p>
      <p>${t(publication.inline_prefix_ready ? "cataloguePublication.inlineReady" : "cataloguePublication.inlinePending")}</p>
      ${publication.invalidated_profiles?.length > 0 && html`<p>${t("cataloguePublication.profiles", { value: publication.invalidated_profiles.join(", ") })}</p>`}
      ${publication.prefix_rebuild_pending && html`<p>${t("cataloguePublication.rebuildPending")}</p>`}
    </div>`}
  </div>`;
}
