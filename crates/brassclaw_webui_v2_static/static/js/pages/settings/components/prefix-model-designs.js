import { PrefixModelConfiguration } from "./prefix-model-configuration.js";
import { PrefixPairCases } from "./prefix-pair-cases.js";
import { PrefixModelSetupDraft } from "./prefix-model-setup-draft.js";
import { ModelTokenLimits } from "./model-token-limits.js";
import { React, html } from "../../../lib/html.js";
import { Button } from "../../../design-system/button.js";
import { fetchPrefixModelDesigns, capturePrefixModel, retainPrefixModelArtifacts, retainPrefixPairConstruction } from "../lib/settings-api.js";

import { prepareModelArtifactImport } from "../lib/prefix-model-artifacts.js";

// Source transport bound; distinct from model token limits and never truncates.
const MAX_PAIR_PROPOSAL_FILE_BYTES = 1024 * 1024;

// Targets come from the existing provider catalogue. A captured configuration
// never asserts that weights, tokenization or recurrent-cache behavior is known.
export function PrefixModelDesigns({ t }) {
  const [catalogue, setCatalogue] = React.useState(null);
  const [selected, setSelected] = React.useState("");
  const [openSpecifications, setOpenSpecifications] = React.useState(() => new Set());
  const [error, setError] = React.useState(null);
  const [busy, setBusy] = React.useState(false);
  const inFlight = React.useRef(false);
  const mounted = React.useRef(true);
  const reloadGeneration = React.useRef(0);
  async function reload() {
    const generation = ++reloadGeneration.current;
    try { const result = await fetchPrefixModelDesigns(); if (mounted.current && generation === reloadGeneration.current) { setCatalogue(result); setError(null); } }
    catch (err) { if (mounted.current && generation === reloadGeneration.current) setError(err.message || String(err)); }
  }
  React.useEffect(() => {
    mounted.current = true; reload();
    const refresh = () => reload();
    globalThis.addEventListener?.("brassclaw-provider-settings-changed", refresh);
    return () => { mounted.current = false; reloadGeneration.current++; globalThis.removeEventListener?.("brassclaw-provider-settings-changed", refresh); };
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
  async function importPairProposal(spec, files) {
    if (inFlight.current || !files?.length) return;
    inFlight.current = true; setBusy(true); setError(null);
    try {
      const file = files[0];
      if (file.size > MAX_PAIR_PROPOSAL_FILE_BYTES) throw Error(t("prefix.pairProposalTooLarge"));
      const proposal_bytes = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(await file.arrayBuffer());
      const document = JSON.parse(proposal_bytes);
      if (document?.format !== "brassclaw-prefix-pair-construction-proposal/1" || document.specification !== spec.id) throw Error(t("prefix.pairProposalInvalid"));
      await retainPrefixPairConstruction({ specification: spec.id, proposal_bytes });
      await reload();
    } catch (err) { if (mounted.current) setError(err.message || String(err)); }
    finally { inFlight.current = false; if (mounted.current) setBusy(false); }
  }
  return html`<section className="rounded-md border border-[var(--v2-panel-border)] p-4 space-y-3" aria-label=${t("prefix.modelDesigns")}>
    <h3 className="font-mono text-sm">${t("prefix.modelDesigns")}</h3>
    <p className="text-xs text-[var(--v2-text-muted)]">${t("prefix.modelContractNotice")}</p>
    ${error && html`<p role="alert">${error}</p>`}
    <label className="block text-sm">${t("prefix.activeModel")}
      <select value=${selected} onChange=${event => setSelected(event.target.value)} disabled=${busy} className="block w-full bg-[var(--v2-surface-soft)] p-2">
        <option value="">${t("prefix.selectModel")}</option>
        ${uniqueTargets.map(item => html`<option key=${key(item)} value=${key(item)}>${item.provider_id} / ${item.model} (${t("prefix.providerConfigurationRevision")} ${item.provider_revision})</option>`)}
      </select>
    </label>
    ${target && html`<${ModelTokenLimits} key=${JSON.stringify([target.provider_id, target.model])} providerId=${target.provider_id} model=${target.model} t=${t} />`}
    <div className="flex gap-2">
      ${html`<${Button} size="sm" disabled=${!target || busy} onClick=${capture}>${t("prefix.captureModel")}<//>`}
      ${html`<${Button} size="sm" disabled=${true}>${t("prefix.generateModelDesign")}<//>`}
      ${html`<${Button} size="sm" variant="ghost" disabled=${busy} onClick=${reload}>${t("common.refresh")}<//>`}
    </div>
    <p role="status" className="text-xs">${t(catalogue?.blocked_reason || "prefix.modelDesignPrerequisites")}</p>
    ${(catalogue?.specifications || []).map(spec => html`<details key=${spec.id} className="text-xs" onToggle=${event => {
      const open = event.currentTarget.open;
      setOpenSpecifications(previous => { const next = new Set(previous); if (open) next.add(spec.id); else next.delete(spec.id); return next; });
    }}>
      <summary>${spec.provider_id} / ${spec.model} · ${t("prefix.providerConfigurationRevision")} ${spec.provider_revision} · ${t("prefix.unknownModelArtifacts")}</summary>
      ${openSpecifications.has(spec.id) && html`<${PrefixModelSetupDraft} key=${spec.id} specification=${spec} t=${t} />`}
      <dl className="space-y-1 break-all">
        <dt>${t("llm.providerId")}</dt><dd>${spec.provider_id}</dd>
        <dt>${t("llm.model")}</dt><dd>${spec.model}</dd>
        <dt>${t("prefix.providerConfigurationRevision")}</dt><dd>${spec.provider_revision}</dd>
        <dt>${t("prefix.specificationId")}</dt><dd><code>${spec.id}</code></dd>
      </dl>
      <details>
        <summary>${t("prefix.originalSpecification")}</summary>
        <pre className="max-h-96 overflow-auto whitespace-pre-wrap break-all">${spec.specification_bytes}</pre>
      </details>
      <p>${t("prefix.modelArtifactNotice")}</p>
      <label className="block">${t("prefix.importModelArtifacts")}
        <input type="file" multiple disabled=${busy} onChange=${event => { const files = Array.from(event.target.files || []); event.target.value = ""; importArtifacts(spec, files); }} />
      </label>
      <p>${t("prefix.pairProposalNotice")}</p>
      <label className="block">${t("prefix.importPairProposal")}
        <input type="file" accept=".json,application/json" disabled=${busy} onChange=${event => { const files = Array.from(event.target.files || []); event.target.value = ""; importPairProposal(spec, files); }} />
      </label>
      ${(catalogue?.pair_proposals || []).filter(proposal => proposal.specification === spec.id).map(proposal => html`<details key=${proposal.id}>
        <summary>${t("prefix.unacceptedPairProposal")} · ${proposal.id}</summary>
        <dl className="space-y-1 break-all">
          <dt>${t("prefix.proposalId")}</dt><dd><code>${proposal.id}</code></dd>
          <dt>${t("prefix.specificationId")}</dt><dd><code>${proposal.specification}</code></dd>
        </dl>
        <details>
          <summary>${t("prefix.originalProposal")}</summary>
          <pre className="max-h-96 overflow-auto whitespace-pre-wrap break-all">${proposal.proposal_bytes}</pre>
        </details>
        <${PrefixPairCases} key=${proposal.id} specification=${spec.id} proposal=${proposal.id} t=${t} />
      </details>`)}
      ${(catalogue?.artifact_sets || []).filter(set => set.specification === spec.id).map(set => html`<details key=${set.id}>
        <summary>${t("prefix.retainedModelArtifacts")} · ${t("prefix.unknownModelArtifacts")}</summary>
        <dl className="space-y-1 break-all">
          <dt>${t("prefix.artifactSetId")}</dt><dd><code>${set.id}</code></dd>
          <dt>${t("prefix.specificationId")}</dt><dd><code>${set.specification}</code></dd>
        </dl>
        <details>
          <summary>${t("prefix.originalArtifactBinding")}</summary>
          <pre className="max-h-96 overflow-auto whitespace-pre-wrap break-all">${set.binding_bytes}</pre>
        </details>
        <ul>${set.files.map(file => html`<li key=${file.name}>${file.name} · ${file.bytes} bytes · ${file.sha256}</li>`)}</ul>
        <${PrefixModelConfiguration} specification=${spec.id} artifactSet=${set} t=${t} />
      </details>`)}
    </details>`)}
    ${(catalogue?.proposals || []).map(proposal => html`<details key=${proposal.id} className="text-xs">
      <summary>${proposal.profile} · ${proposal.id} · ${t("prefix.unacceptedProposal")}</summary>
      <dl className="space-y-1 break-all">
        <dt>${t("prefix.proposalId")}</dt><dd><code>${proposal.id}</code></dd>
        <dt>${t("prefix.specificationId")}</dt><dd><code>${proposal.specification}</code></dd>
      </dl>
      <details>
        <summary>${t("prefix.originalProposal")}</summary>
        <pre className="max-h-96 overflow-auto whitespace-pre-wrap break-all">${proposal.proposal_bytes}</pre>
      </details>
    </details>`)}
  </section>`;
}
