import { React, html } from "../../../lib/html.js";
import { Card } from "../../../design-system/card.js";
import { useT } from "../../../lib/i18n.js";
import { clientActionId } from "../../../lib/api.js";
import { fetchComponentReviewSubmission, fetchCatalogueActivationStatus, prepareComponentDeployment, activateComponentDeployment } from "../lib/settings-api.js";

// Preparation reads immutable subjects and verified approvals. Only explicit
// activation publishes. Unknown commits lock the exact request/ID for retry.
export function ComponentDeploymentPanel() {
  const t = useT();
  const [ids, setIds] = React.useState("");
  const [workflowClass, setWorkflowClass] = React.useState("deterministic");
  const [approvals, setApprovals] = React.useState("{}");
  const [plan, setPlan] = React.useState(null);
  const [status, setStatus] = React.useState(null);
  const [receipt, setReceipt] = React.useState(null);
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState("");
  const pending = React.useRef(null);
  const inFlight = React.useRef(false);
  const uncertain = React.useRef(false);
  const prepare = async () => {
    if (inFlight.current || pending.current || receipt) return;
    inFlight.current = true; setBusy(true); setError(""); setPlan(null);
    try {
      const selectedIds = ids.trim().split(/\s+/).filter(Boolean);
      if (!selectedIds.length || selectedIds.length > 512 || new Set(selectedIds).size !== selectedIds.length) throw Error(t("componentDeployment.idsInvalid"));
      const usageApprovals = JSON.parse(approvals);
      if (!usageApprovals || Array.isArray(usageApprovals) || typeof usageApprovals !== "object" || Object.values(usageApprovals).some(value => typeof value !== "string")) throw Error(t("componentDeployment.approvalsInvalid"));
      const current = await fetchCatalogueActivationStatus();
      setStatus(current);
      const subjects = [];
      const usedSkills = new Set();
      // Bounded sequential reads avoid flooding the operator gateway.
      for (const id of selectedIds) {
        const view = await fetchComponentReviewSubmission(id);
        const subject = view.receipt;
        if (subject.candidate.class_code !== 21) throw Error(t("componentDeployment.recipeRequired"));
        const skills = subject.dependencies.filter(ref => [1, 2, 3].includes(ref.class_code)).map(ref => ref.uuid);
        skills.forEach(skill => usedSkills.add(skill));
        subjects.push({ subject: { submission_id: subject.submission_id, subject_checksum: subject.subject_checksum }, workflow_class: workflowClass,
          association_approvals: Object.fromEntries(Object.entries(usageApprovals).filter(([skill]) => skills.includes(skill))) });
      }
      if (Object.keys(usageApprovals).some(skill => !usedSkills.has(skill))) throw Error(t("componentDeployment.unusedApproval"));
      const prepared = await prepareComponentDeployment({ activation_id: clientActionId(), expected_generation: current.generation, recipes: subjects });
      if (new TextEncoder().encode(JSON.stringify(prepared.activation)).byteLength > 14 * 1024 * 1024) throw Error(t("componentDeployment.capacity"));
      setPlan(prepared);
    } catch (err) { setError(err.message); }
    finally { inFlight.current = false; setBusy(false); }
  };
  const activate = async () => {
    if (inFlight.current || receipt || !plan) return;
    inFlight.current = true; setBusy(true); setError("");
    try {
      pending.current ??= JSON.parse(JSON.stringify(plan.activation));
      setReceipt(await activateComponentDeployment(pending.current));
    } catch (err) {
      if (!uncertain.current && [400, 401, 403, 404, 409, 413, 422, 429].includes(err.status)) {
        pending.current = null;
      } else { uncertain.current = true; }
      setError(err.message);
    }
    finally { inFlight.current = false; setBusy(false); }
  };
  const reset = () => {
    if (inFlight.current || (pending.current && !receipt)) return;
    pending.current = null; uncertain.current = false;
    setPlan(null); setReceipt(null); setStatus(null); setError("");
  };
  const locked = busy || Boolean(pending.current) || Boolean(receipt);
  return html`
    <${Card} padding="md">
      <h3 className="font-semibold">${t("componentDeployment.title")}</h3>
      <p className="mt-2 text-sm text-[var(--v2-text-muted)]">${t("componentDeployment.scope")}</p>
      <div className="mt-4 space-y-3">
        <label className="block text-sm">${t("componentDeployment.ids")}
          <textarea className="mt-1 w-full rounded border p-2 bg-transparent font-mono text-xs" rows="3" value=${ids} disabled=${locked}
            onChange=${e => { setIds(e.target.value); setPlan(null); }} />
        </label>
        <label className="block text-sm">${t("componentDeployment.tier")}
          <select className="ml-2 rounded border p-2 bg-transparent" value=${workflowClass} disabled=${locked} onChange=${e => { setWorkflowClass(e.target.value); setPlan(null); }}>
            <option value="deterministic">${t("componentDeployment.deterministic")}</option>
            <option value="requires_model">${t("componentDeployment.requiresModel")}</option>
          </select>
        </label>
        <label className="block text-sm">${t("componentDeployment.approvals")}
          <textarea className="mt-1 w-full rounded border p-2 bg-transparent font-mono text-xs" rows="3" value=${approvals} disabled=${locked}
            onChange=${e => { setApprovals(e.target.value); setPlan(null); }} />
        </label>
        <p className="text-xs text-[var(--v2-text-muted)]">${t("componentDeployment.approvalsHelp")}</p>
        <button type="button" className="rounded border px-3 py-2 text-sm" disabled=${locked || !ids.trim()} onClick=${prepare}>${t("componentDeployment.prepare")}</button>
        ${status && html`<p className="break-all text-xs">${t("componentDeployment.generation", { value: status.generation })}</p>`}
        ${plan && html`<div className="space-y-3">
          <p className="text-sm">${t("componentDeployment.preview", { preserved: plan.preserved_recipe_ids.length, replaced: plan.replaced_recipe_ids.length })}</p>
          <details><summary>${t("componentDeployment.selections")}</summary>
            <pre className="max-h-96 overflow-auto whitespace-pre-wrap break-all text-xs">${JSON.stringify(plan.activation, null, 2)}</pre>
          </details>
          <button type="button" className="rounded border px-3 py-2 text-sm" disabled=${busy || Boolean(receipt)} onClick=${activate}>${t(pending.current ? "componentDeployment.retry" : "componentDeployment.activate")}</button>
        </div>`}
        ${receipt && html`<p role="status" className="break-all text-sm">${t(receipt.catalogue_activated ? "componentDeployment.active" : "componentDeployment.historical", { generation: receipt.generation })}</p>`}
        ${error && html`<p role="alert" className="text-sm text-[var(--v2-danger-text)]">${error}</p>`}
        <button type="button" className="rounded border px-3 py-2 text-sm" disabled=${busy || (Boolean(pending.current) && !receipt)} onClick=${reset}>${t("componentDeployment.another")}</button>
      </div>
    <//>`;
}
