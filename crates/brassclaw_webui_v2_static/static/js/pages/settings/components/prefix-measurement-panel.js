import { React, html } from "../../../lib/html.js";
import { clientActionId } from "../../../lib/api.js";
import { Button } from "../../../design-system/button.js";
import { fetchPrefixModelDesigns, fetchPrefixJob, measurePrefix } from "../lib/settings-api.js";

const terminal = new Set(["Completed", "Failed", "Cancelled", "RecoveryRequired"]);
const exampleProbes = JSON.stringify(["Probe A", "Probe B"].map(content => ({
  kind: "chat", context_json: JSON.stringify({ messages: [{ role: "user", content }], add_generation_prompt: true, enable_thinking: false }),
})), null, 2);

function integer(text, minimum) {
  if (!/^(0|[1-9][0-9]*)$/.test(text)) throw Error("prefix.measureInvalidLimits");
  const value = Number(text);
  if (!Number.isSafeInteger(value) || value < minimum) throw Error("prefix.measureInvalidLimits");
  return value;
}
function remembered(key) {
  try { return sessionStorage.getItem(key) || ""; } catch { return ""; }
}
function remember(key, id) {
  try { sessionStorage.setItem(key, id); } catch { /* The durable job still owns its state. */ }
}

// This is an offline diagnostic on a retained artifact, separate from Generate.
// Browser storage contains only a correlation UUID, never probes or source text.
export function PrefixMeasurementPanel({ entry, generation, t }) {
  const storageKey = `brassclaw.prefix.measurement.${entry.name}.${generation.generation}`;
  const [open, setOpen] = React.useState(false);
  const [catalogue, setCatalogue] = React.useState(null);
  const [proposalId, setProposalId] = React.useState("");
  const [artifactId, setArtifactId] = React.useState("");
  const [context, setContext] = React.useState("");
  const [additional, setAdditional] = React.useState("0");
  const [probes, setProbes] = React.useState(exampleProbes);
  const [trackingId, setTrackingId] = React.useState(() => remembered(storageKey));
  const [job, setJob] = React.useState(null);
  const [pending, setPending] = React.useState(null);
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState(null);
  const inFlight = React.useRef(false);
  const mounted = React.useRef(true);
  React.useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  React.useEffect(() => {
    if (!open) return;
    let stopped = false;
    fetchPrefixModelDesigns().then(value => { if (!stopped) setCatalogue(value); })
      .catch(err => { if (!stopped) setError(err.message || String(err)); });
    return () => { stopped = true; };
  }, [open, entry.name]);
  React.useEffect(() => {
    if (!open || !trackingId || terminal.has(job?.run_status)) return;
    let stopped = false, timer;
    async function poll() {
      try {
        const value = await fetchPrefixJob(entry.name, trackingId);
        if (stopped) return;
        setJob(value); setPending(null);
        if (!terminal.has(value.run_status)) timer = setTimeout(poll, 2000);
      } catch (err) { if (!stopped) setError(err.message || String(err)); }
    }
    timer = setTimeout(poll, 2000);
    return () => { stopped = true; clearTimeout(timer); };
  }, [open, entry.name, trackingId, job?.run_status]);
  const proposals = (catalogue?.proposals || []).filter(value => value.profile === entry.name);
  const proposal = proposals.find(value => value.id === proposalId);
  const artifacts = (catalogue?.artifact_sets || []).filter(value => value.specification === proposal?.specification);
  function selectProposal(id) {
    setProposalId(id); setArtifactId(""); setContext("");
    const selected = proposals.find(value => value.id === id);
    const spec = catalogue?.specifications?.find(value => value.id === selected?.specification);
    try {
      const configured = JSON.parse(spec?.specification_bytes || "null")?.configured_context_window;
      if (Number.isSafeInteger(configured) && configured > 0) setContext(String(configured));
    } catch { /* Unknown limits require an explicit operator assumption. */ }
  }
  async function submit() {
    if (inFlight.current) return;
    inFlight.current = true; setBusy(true); setError(null);
    try {
      let attempt = pending;
      if (!attempt) {
        if (!proposal || !artifacts.some(value => value.id === artifactId)) throw Error("prefix.measureSelectInputs");
        const parsed = JSON.parse(probes);
        if (!Array.isArray(parsed) || parsed.length < 2 || parsed.length > 64 || parsed.some(value => value?.kind !== "chat" || typeof value.context_json !== "string")) throw Error("prefix.measureInvalidProbes");
        attempt = { id: clientActionId(), request: {
          expected_profile_revision: entry.profile_revision, generation: generation.generation,
          proposal: proposal.id, artifact_set: artifactId, probes_json: probes,
          context_window_tokens: integer(context, 1), reserved_additional_input_tokens: integer(additional, 0),
        } };
        setPending(attempt); setJob(null); setTrackingId(attempt.id); remember(storageKey, attempt.id);
      }
      const value = await measurePrefix(entry.name, attempt.id, attempt.request);
      if (mounted.current) { setJob(value); setPending(null); }
    } catch (err) {
      if (mounted.current) setError(err.message?.startsWith("prefix.") ? t(err.message) : err.message || String(err));
    } finally { inFlight.current = false; if (mounted.current) setBusy(false); }
  }
  async function refresh() {
    try {
      const value = await fetchPrefixJob(entry.name, trackingId);
      if (mounted.current) { setJob(value); setPending(null); setError(null); }
    } catch (err) { if (mounted.current) setError(err.message || String(err)); }
  }
  let comparison = null;
  try {
    const value = JSON.parse(job?.internal_receipt || "null");
    if (value?.format === "brassclaw-prefix-capacity-comparison/1") comparison = value;
  } catch { /* A failure/status record is not a capacity verdict. */ }
  return html`<div className="w-full">
    <${Button} size="sm" variant="ghost" onClick=${() => setOpen(value => !value)}>${t("prefix.measureOffline")}<//>
    ${open && html`<div className="space-y-3 rounded border border-[var(--v2-panel-border)] p-3">
      <p className="text-[var(--v2-text-muted)]">${t("prefix.measureNotice")}</p>
      ${error && html`<p role="alert" className="text-red-300">${error}</p>`}
      <label className="block">${t("prefix.measureProposal")}
        <select value=${proposalId} disabled=${busy || !!pending} onChange=${event => selectProposal(event.target.value)} className="block w-full">
          <option value="">${t("prefix.measureSelectInputs")}</option>
          ${proposals.map(value => {
            const spec = catalogue?.specifications?.find(spec => spec.id === value.specification);
            return html`<option key=${value.id} value=${value.id}>${spec?.provider_id} · ${spec?.model} · ${value.id}</option>`;
          })}
        </select>
      </label>
      <label className="block">${t("prefix.measureArtifacts")}
        <select value=${artifactId} disabled=${busy || !!pending} onChange=${event => setArtifactId(event.target.value)} className="block w-full">
          <option value="">${t("prefix.measureSelectInputs")}</option>
          ${artifacts.map(value => html`<option key=${value.id} value=${value.id}>${value.id} · ${value.qualification}</option>`)}
        </select>
      </label>
      <label className="block">${t("prefix.measureContext")}<input aria-label=${t("prefix.measureContext")} inputMode="numeric" value=${context} disabled=${busy || !!pending} onChange=${event => setContext(event.target.value)} className="block w-full" /></label>
      <label className="block">${t("prefix.measureAdditional")}<input aria-label=${t("prefix.measureAdditional")} inputMode="numeric" value=${additional} disabled=${busy || !!pending} onChange=${event => setAdditional(event.target.value)} className="block w-full" /></label>
      <label className="block">${t("prefix.measureProbes")}<textarea rows="7" value=${probes} disabled=${busy || !!pending} onChange=${event => setProbes(event.target.value)} className="block w-full font-mono" /></label>
      <${Button} size="sm" disabled=${busy || (!pending && (!proposal || !artifactId || !context))} onClick=${submit}>${t(pending ? "prefix.measureRetry" : "prefix.measureStart")}<//>
      ${pending && html`<div>
        <p className="text-[var(--v2-text-muted)]">${t("prefix.measureNewNotice")}</p>
        <${Button} size="sm" variant="ghost" disabled=${busy} onClick=${() => { setPending(null); setError(null); }}>${t("prefix.measureEditNew")}<//>
      </div>`}
      ${trackingId && html`<div className="space-y-1">
        <code className="break-all">${trackingId}</code>
        <${Button} size="sm" variant="ghost" onClick=${refresh}>${t("common.refresh")}<//>
        <p role="status">${job?.run_status || t("prefix.measureUnknown")} · ${job?.answered_tool_calls ?? 0} ${t("prefix.measureCalls")}</p>
        ${comparison && html`<p role="status">${t(comparison.fits ? "prefix.measureFits" : "prefix.measureDoesNotFit")} · ${comparison.required_tokens} / ${comparison.context_window_tokens}</p>`}
        ${job && html`<details><summary>${t("prefix.measureDetails")}</summary><pre className="max-h-80 overflow-auto whitespace-pre-wrap break-all">${JSON.stringify(comparison || job, null, 2)}</pre></details>`}
      </div>`}
    </div>`}
  </div>`;
}
