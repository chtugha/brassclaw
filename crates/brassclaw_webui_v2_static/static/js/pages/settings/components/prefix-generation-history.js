import { PrefixMeasurementPanel } from "./prefix-measurement-panel.js";
import { React, html } from "../../../lib/html.js";
import { Button } from "../../../design-system/button.js";
import { fetchPrefixGenerations, fetchPrefixGeneration, importPrefixGeneration } from "../lib/settings-api.js";

export function PrefixGenerationHistory({ entry, t }) {
  const [open, setOpen] = React.useState(false);
  const [inventory, setGenerations] = React.useState({ profile: entry.name, items: [] });
  const generations = inventory.profile === entry.name ? inventory.items : [];
  const [detailRecord, setDetail] = React.useState(null);
  const detail = detailRecord?.profile === entry.name ? detailRecord.data : null;
  const [errorRecord, setError] = React.useState(null);
  const error = errorRecord?.profile === entry.name ? errorRecord.message : null;
  const [busy, setBusy] = React.useState(false);
  const [requestRecord, setRequest] = React.useState(null);
  const request = requestRecord?.profile === entry.name ? requestRecord : null;
  const inFlight = React.useRef(false);
  const detailEpoch = React.useRef(0);
  const listEpoch = React.useRef(0);
  const session = React.useRef(null);
  const mounted = React.useRef(true);
  const [refresh, setRefresh] = React.useState(0);
  const current = token => mounted.current && token !== null && session.current === token;
  React.useLayoutEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; session.current = null; };
  }, []);
  React.useLayoutEffect(() => {
    // Commit-time fencing: passive cleanup can run after an older Promise.
    const token = open ? { profile: entry.name } : null;
    session.current = token;
    return () => {
      if (session.current === token) session.current = null;
      detailEpoch.current++; listEpoch.current++;
    };
  }, [open, entry.name]);
  React.useEffect(() => {
    if (!open) return;
    let stopped = false;
    const token = session.current, epoch = ++listEpoch.current, intent = detailEpoch.current;
    fetchPrefixGenerations(entry.name).then(values => {
      if (!stopped && current(token) && epoch === listEpoch.current) setGenerations({ profile: entry.name, items: values });
    }).catch(err => {
      if (!stopped && current(token) && epoch === listEpoch.current && intent === detailEpoch.current) setError({ profile: entry.name, message: err.message });
    });
    return () => { stopped = true; };
  }, [open, entry.name, refresh]);
  function toggle() {
    session.current = null; detailEpoch.current++; listEpoch.current++;
    setDetail(null); setError(null); setGenerations({ profile: entry.name, items: [] });
    setOpen(value => !value);
  }
  async function inspect(generation) {
    const token = session.current;
    if (!current(token) || token.profile !== entry.name) return;
    const epoch = ++detailEpoch.current; setError(null); setDetail(null);
    try {
      const value = await fetchPrefixGeneration(entry.name, generation);
      if (current(token) && epoch === detailEpoch.current) setDetail({ profile: entry.name, data: value });
    } catch (err) { if (current(token) && epoch === detailEpoch.current) setError({ profile: entry.name, message: err.message }); }
  }
  async function retain(value, profile = entry.name, token = session.current, chosenEpoch = null) {
    if (inFlight.current || !current(token) || token.profile !== profile) return;
    const epoch = chosenEpoch ?? ++detailEpoch.current;
    if (epoch !== detailEpoch.current) return;
    inFlight.current = true; setBusy(true); setError(null); setRequest({ profile, value });
    try {
      const retained = await importPrefixGeneration(profile, value);
      if (mounted.current) {
        // A confirmed write settles only its original retry. It is not cancelled
        // by closing the panel, and it cannot select a later view's detail.
        setRequest(previous => previous?.profile === profile && previous.value === value ? null : previous);
        if (session.current?.profile === profile) {
          listEpoch.current++;
          setGenerations(previous => ({ profile, items: [...(previous.profile === profile ? previous.items : []).filter(g => g.generation !== retained.summary.generation), retained.summary] }));
          setRefresh(value => value + 1);
        }
        if (current(token) && epoch === detailEpoch.current) setDetail({ profile, data: retained });
      }
    } catch (err) { if (current(token) && epoch === detailEpoch.current) setError({ profile, message: err.message }); }
    finally { inFlight.current = false; if (mounted.current) setBusy(false); }
  }
  async function choose(event) {
    const file = event.target.files?.[0];
    const token = session.current;
    if (!file || inFlight.current || !current(token) || token.profile !== entry.name) return;
    const epoch = ++detailEpoch.current;
    if (file.size > 64 * 1024 * 1024) { setError({ profile: entry.name, message: t("prefix.artifactTooLarge") }); return; }
    try {
      const bytes = await file.text();
      if (current(token) && epoch === detailEpoch.current) await retain({ expected_profile_revision: entry.profile_revision, generation_bytes: bytes }, entry.name, token, epoch);
    } catch (err) { if (current(token) && epoch === detailEpoch.current) setError({ profile: entry.name, message: err.message }); }
  }
  return html`<div className="mt-3 w-full text-xs">
    <${Button} size="sm" variant="ghost" onClick=${toggle}>${t("prefix.history")}<//>
    ${open && html`<div className="mt-2 space-y-2 rounded border border-[var(--v2-panel-border)] p-3">
      <p className="text-[var(--v2-text-muted)]">${t("prefix.retentionNotice")}</p>
      ${error && html`<p role="alert" className="text-red-300">${error}</p>`}
      ${["base-prompt", "prefix-maker"].includes(entry.name) && html`<label className="block">${t("prefix.importArtifact")}
        <input type="file" accept="application/json,.json" disabled=${busy} onChange=${choose} className="ml-2" />
      </label>`}
      ${request && html`<${Button} size="sm" disabled=${busy} onClick=${() => retain(request.value, request.profile)}>${t("prefix.retryImport")}<//>`}
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
