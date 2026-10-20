import { PrefixMeasurementPanel } from "./prefix-measurement-panel.js";
import { React, html } from "../../../lib/html.js";
import { Button } from "../../../design-system/button.js";
import { fetchPrefixGenerations, fetchPrefixGeneration, importPrefixGeneration } from "../lib/settings-api.js";

export function PrefixGenerationHistory({ entry, t }) {
  const [open, setOpen] = React.useState(false);
  const [generations, setGenerations] = React.useState([]);
  const [detail, setDetail] = React.useState(null);
  const [error, setError] = React.useState(null);
  const [busy, setBusy] = React.useState(false);
  const [request, setRequest] = React.useState(null);
  const inFlight = React.useRef(false);
  const detailEpoch = React.useRef(0);
  React.useEffect(() => {
    if (!open) return;
    let stopped = false;
    fetchPrefixGenerations(entry.name).then(values => { if (!stopped) setGenerations(values); })
      .catch(err => { if (!stopped) setError(err.message); });
    return () => { stopped = true; detailEpoch.current++; };
  }, [open, entry.name]);
  async function inspect(generation) {
    const epoch = ++detailEpoch.current; setError(null);
    try {
      const value = await fetchPrefixGeneration(entry.name, generation);
      if (epoch === detailEpoch.current) setDetail(value);
    } catch (err) { if (epoch === detailEpoch.current) setError(err.message); }
  }
  async function retain(value) {
    if (inFlight.current) return;
    inFlight.current = true; setBusy(true); setError(null); setRequest(value);
    try {
      const retained = await importPrefixGeneration(entry.name, value);
      setDetail(retained);
      setGenerations(previous => [...previous.filter(g => g.generation !== retained.summary.generation), retained.summary]);
      setRequest(null);
    } catch (err) { setError(err.message); }
    finally { inFlight.current = false; setBusy(false); }
  }
  async function choose(event) {
    const file = event.target.files?.[0];
    if (!file) return;
    if (file.size > 64 * 1024 * 1024) { setError(t("prefix.artifactTooLarge")); return; }
    try {
      const bytes = await file.text();
      await retain({ expected_profile_revision: entry.profile_revision, generation_bytes: bytes });
    } catch (err) { setError(err.message); }
  }
  return html`<div className="mt-3 w-full text-xs">
    <${Button} size="sm" variant="ghost" onClick=${() => setOpen(value => !value)}>${t("prefix.history")}<//>
    ${open && html`<div className="mt-2 space-y-2 rounded border border-[var(--v2-panel-border)] p-3">
      <p className="text-[var(--v2-text-muted)]">${t("prefix.retentionNotice")}</p>
      ${error && html`<p role="alert" className="text-red-300">${error}</p>`}
      ${["base-prompt", "prefix-maker"].includes(entry.name) && html`<label className="block">${t("prefix.importArtifact")}
        <input type="file" accept="application/json,.json" disabled=${busy} onChange=${choose} className="ml-2" />
      </label>`}
      ${request && html`<${Button} size="sm" disabled=${busy} onClick=${() => retain(request)}>${t("prefix.retryImport")}<//>`}
      ${generations.length === 0 && html`<p>${t("prefix.noGenerations")}</p>`}
      ${generations.map(g => html`<div key=${g.generation} className="flex flex-wrap items-center gap-2">
        <code className="break-all">${g.generation}</code><span>${g.qualification}</span>
        <${Button} size="sm" variant="ghost" onClick=${() => inspect(g.generation)}>${t("prefix.inspectGeneration")}<//>
        ${g.kind === "compiler-package" && html`<${PrefixMeasurementPanel} entry=${entry} generation=${g} t=${t} />`}
      </div>`)}
      ${detail && html`<details><summary>${t("prefix.generationDetails")}</summary>
        <pre className="max-h-96 overflow-auto whitespace-pre-wrap break-all">${JSON.stringify(detail, null, 2)}</pre>
      </details>`}
    </div>`}
  </div>`;
}
