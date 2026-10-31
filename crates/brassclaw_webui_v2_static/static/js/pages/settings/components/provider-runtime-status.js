import { html } from "../../../lib/html.js";

function identity(publication, unresolved, t) {
  if (publication?.state === "unconfigured") return t("llm.publicationNone");
  if (publication?.state !== "configured") return unresolved?.provider_id || t("llm.publicationUnknown");
  const selected = publication.selection;
  return `${selected.provider_id} · ${selected.model} · r${selected.definition_revision} · ${selected.configuration_ref}`;
}

export function ProviderRuntimeStatus({ rows = [], t }) {
  if (!rows.length) return null;
  return html`<section className="my-3 space-y-2 rounded-md border p-3" aria-label=${t("llm.publicationTitle")}>
    <h4>${t("llm.publicationTitle")}</h4>
    <p className="text-xs">${t("llm.publicationHelp")}</p>
    ${rows.some(row => row.failed) && html`<p className="text-xs">${t("llm.publicationFailureHelp")}</p>`}
    ${rows.map(row => html`<div key=${row.role} className="space-y-1 break-all text-xs" role=${["pending", "invalid", "failed"].includes(row.state) ? "status" : undefined}>
      <strong>${t(`llm.publicationRole.${row.role}`)} · ${t(`llm.publicationState.${row.state}`)}</strong>
      <p>${t("llm.publicationDesired")}: ${identity(row.desired, row.unresolved, t)}</p>
      <p>${t("llm.publicationEffective")}: ${identity(row.effective, null, t)}</p>
      ${row.failed && html`<p>${t("llm.publicationFailedTarget")}: ${identity(row.failed, null, t)}</p>`}
    </div>`)}
  </section>`;
}
