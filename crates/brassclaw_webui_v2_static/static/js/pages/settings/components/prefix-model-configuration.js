import { React, html } from "../../../lib/html.js";
import { Button } from "../../../design-system/button.js";
import { fetchPrefixModelArtifacts } from "../lib/settings-api.js";
import { parseModelConfigurationObservation } from "../lib/prefix-model-configuration.js";

export function PrefixModelConfiguration({ specification, artifactSet, t }) {
  const [report, setReport] = React.useState(null);
  const [error, setError] = React.useState(null);
  const [busy, setBusy] = React.useState(false);
  const [open, setOpen] = React.useState(false);
  const active = React.useRef(0);
  const inFlight = React.useRef(false);
  React.useEffect(() => {
    active.current += 1;
    setReport(null); setError(null); setBusy(false); setOpen(false);
    inFlight.current = false;
    return () => { active.current += 1; };
  }, [specification, artifactSet.id]);
  async function inspect() {
    if (inFlight.current) return;
    if (report) { setOpen(value => !value); return; }
    inFlight.current = true; setBusy(true); setError(null); setOpen(true);
    const version = active.current;
    try {
      const detail = await fetchPrefixModelArtifacts(specification, artifactSet.id);
      const value = parseModelConfigurationObservation(detail, specification, artifactSet);
      if (version === active.current) setReport(value);
    } catch (err) {
      if (version === active.current) setError(err.message?.startsWith("prefix.") ? t(err.message) : err.message || String(err));
    } finally {
      if (version === active.current) { inFlight.current = false; setBusy(false); }
    }
  }
  return html`<div className="space-y-2">
    <${Button} size="sm" variant="ghost" disabled=${busy} onClick=${inspect}>${t("prefix.inspectModelConfig")}<//>
    ${open && html`<div className="space-y-2">
      <p className="text-[var(--v2-text-muted)]">${t("prefix.modelConfigNotice")}</p>
      ${busy && html`<p role="status">${t("common.loading")}</p>`}
      ${error && html`<p role="alert">${error}</p>`}
      ${report && html`<div>
        <p role="status">${t(`prefix.modelConfigStatus.${report.status}`)}</p>
        <pre className="max-h-96 overflow-auto whitespace-pre-wrap break-all">${report.text}</pre>
      </div>`}
    </div>`}
  </div>`;
}
