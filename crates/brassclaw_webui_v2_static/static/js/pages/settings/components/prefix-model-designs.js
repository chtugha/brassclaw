import { PrefixModelConfiguration } from "./prefix-model-configuration.js";
import { ModelTokenLimits } from "./model-token-limits.js";
import { React, html } from "../../../lib/html.js";
import { Button } from "../../../design-system/button.js";
import { fetchPrefixModelDesigns, capturePrefixModel, retainPrefixModelArtifacts } from "../lib/settings-api.js";

import { prepareModelArtifactImport } from "../lib/prefix-model-artifacts.js";

// Targets come from the existing provider catalogue. A captured configuration
// never asserts that weights, tokenization or recurrent-cache behavior is known.
export function PrefixModelDesigns({ t }) {
  const [catalogue, setCatalogue] = React.useState(null);
  const [selected, setSelected] = React.useState("");
  const [error, setError] = React.useState(null);
  const [busy, setBusy] = React.useState(false);
  const inFlight = React.useRef(false);
  const mounted = React.useRef(true);
  async function reload() {
    try { const result = await fetchPrefixModelDesigns(); if (mounted.current) { setCatalogue(result); setError(null); } }
    catch (err) { if (mounted.current) setError(err.message || String(err)); }
  }
  React.useEffect(() => {
    mounted.current = true; reload();
    const refresh = () => reload();
    globalThis.addEventListener?.("brassclaw-provider-settings-changed", refresh);
    return () => { mounted.current = false; globalThis.removeEventListener?.("brassclaw-provider-settings-changed", refresh); };
  }, []);
  const targets = catalogue?.targets || [];
  // Keep the selected serving identity when its canonical settings revision changes.
  // The current target still supplies the exact revision for every capture.
  function key(target) { return JSON.stringify([target.provider_id, target.model]); }
  const uniqueTargets = targets.filter((target, index) => targets.findIndex(other => key(other) === key(target)) === index);
  const target = uniqueTargets.find(item => key(item) === selected);
  async function capture() {
    if (!target || inFlight.current) return;
    inFlight.current = true; setBusy(true); setError(null);
    try {
      await capturePrefixModel({ provider_id: target.provider_id, expected_provider_revision: target.provider_revision, model: target.model });
      await reload();
    } catch (err) { if (mounted.current) setError(err.message || String(err)); }
    finally { inFlight.current = false; if (mounted.current) setBusy(false); }
  }
  async function importArtifacts(spec, files) {
    if (inFlight.current || !files?.length) return;
    inFlight.current = true; setBusy(true); setError(null);
    try {
      const artifact_set_bytes = await prepareModelArtifactImport(files);
      await retainPrefixModelArtifacts({ specification: spec.id, artifact_set_bytes });
      await reload();
    } catch (err) { if (mounted.current) setError(err.message?.startsWith("prefix.") ? t(err.message) : err.message || String(err)); }
    finally { inFlight.current = false; if (mounted.current) setBusy(false); }
  }
  return html`<section className="rounded-md border border-[var(--v2-panel-border)] p-4 space-y-3" aria-label=${t("prefix.modelDesigns")}>
    <h3 className="font-mono text-sm">${t("prefix.modelDesigns")}</h3>
    <p className="text-xs text-[var(--v2-text-muted)]">${t("prefix.modelContractNotice")}</p>
    ${error && html`<p role="alert">${error}</p>`}
    <label className="block text-sm">${t("prefix.activeModel")}
      <select value=${selected} onChange=${event => setSelected(event.target.value)} disabled=${busy} className="block w-full bg-[var(--v2-surface-soft)] p-2">
        <option value="">${t("prefix.selectModel")}</option>
        ${uniqueTargets.map(item => html`<option key=${key(item)} value=${key(item)}>${item.provider_id} / ${item.model} (${t("prefix.revision")} ${item.provider_revision})</option>`)}
      </select>
    </label>
    ${target && html`<${ModelTokenLimits} key=${JSON.stringify([target.provider_id, target.model])} providerId=${target.provider_id} model=${target.model} t=${t} />`}
    <div className="flex gap-2">
      ${html`<${Button} size="sm" disabled=${!target || busy} onClick=${capture}>${t("prefix.captureModel")}<//>`}
      ${html`<${Button} size="sm" disabled=${true}>${t("prefix.generateModelDesign")}<//>`}
      ${html`<${Button} size="sm" variant="ghost" disabled=${busy} onClick=${reload}>${t("common.refresh")}<//>`}
    </div>
    <p role="status" className="text-xs">${t(catalogue?.blocked_reason || "prefix.modelDesignPrerequisites")}</p>
    ${(catalogue?.specifications || []).map(spec => html`<details key=${spec.id} className="text-xs">
      <summary>${spec.provider_id} / ${spec.model} · ${t("prefix.revision")} ${spec.provider_revision} · ${t("prefix.unknownModelArtifacts")}</summary>
      <pre className="overflow-auto whitespace-pre-wrap">${spec.specification_bytes}</pre>
      <p>${t("prefix.modelArtifactNotice")}</p>
      <label className="block">${t("prefix.importModelArtifacts")}
        <input type="file" multiple disabled=${busy} onChange=${event => { const files = Array.from(event.target.files || []); event.target.value = ""; importArtifacts(spec, files); }} />
      </label>
      ${(catalogue?.artifact_sets || []).filter(set => set.specification === spec.id).map(set => html`<details key=${set.id}>
        <summary>${t("prefix.retainedModelArtifacts")} · ${t("prefix.unknownModelArtifacts")}</summary>
        <pre className="overflow-auto whitespace-pre-wrap">${set.binding_bytes}</pre>
        <ul>${set.files.map(file => html`<li key=${file.name}>${file.name} · ${file.bytes} bytes · ${file.sha256}</li>`)}</ul>
        <${PrefixModelConfiguration} specification=${spec.id} artifactSet=${set} t=${t} />
      </details>`)}
    </details>`)}
    ${(catalogue?.proposals || []).map(proposal => html`<details key=${proposal.id} className="text-xs">
      <summary>${proposal.profile} · ${proposal.id} · ${t("prefix.unacceptedProposal")}</summary>
      <pre className="overflow-auto whitespace-pre-wrap">${proposal.proposal_bytes}</pre>
    </details>`)}
  </section>`;
}
