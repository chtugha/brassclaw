import { React, html } from "../../../lib/html.js";
import { clientActionId } from "../../../lib/api.js";
import { Button } from "../../../design-system/button.js";
import { checkPrefixPairCases, fetchPrefixPairJob } from "../lib/settings-api.js";

const terminal = new Set(["Completed", "Failed", "Cancelled", "RecoveryRequired"]);
function remembered(key) {
  try { return sessionStorage.getItem(key) || ""; } catch { return ""; }
}
function remember(key, id) {
  try { sessionStorage.setItem(key, id); } catch { /* The server retains the original job. */ }
}

// One immutable proposal and original correlation ID. Reading never resubmits;
// ambiguous submission retries retain the ID instead of starting replacement work.
export function PrefixPairCases({ specification, proposal, t }) {
  const storageKey = `brassclaw.prefix.pair.${specification}.${proposal}`;
  const [open, setOpen] = React.useState(false);
  const [id, setId] = React.useState(() => remembered(storageKey));
  const [job, setJob] = React.useState(null);
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState(null);
  const inFlight = React.useRef(false);
  const mounted = React.useRef(true);
  const readGeneration = React.useRef(0);
  React.useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; readGeneration.current++; };
  }, []);

  function checkTarget(value, expectedId) {
    if (value?.target_kind !== "pair" || value.operation !== "check_cases" || value.mode !== "target" || value.specification !== specification || value.proposal !== proposal || value.job_id !== expectedId) {
      throw Error(t("prefix.pairWrongJob"));
    }
    return value;
  }
  async function refresh() {
    if (!id || inFlight.current) return;
    const generation = ++readGeneration.current;
    try {
      const value = checkTarget(await fetchPrefixPairJob(specification, proposal, id), id);
      if (mounted.current && generation === readGeneration.current) { setJob(value); setError(null); }
    } catch (err) {
      if (mounted.current && generation === readGeneration.current) setError(err.message || String(err));
    }
  }
  React.useEffect(() => { if (open && id) refresh(); }, [open, id]);

  async function submit(newJob = false) {
    if (inFlight.current || (newJob && !(terminal.has(job?.run_status) && job?.admission_phase === "settled"))) return;
    inFlight.current = true; setBusy(true); setError(null);
    // Fence an earlier GET, including one started by opening the panel.
    readGeneration.current++;
    try {
      const attempt = newJob || !id ? clientActionId() : id;
      setId(attempt); remember(storageKey, attempt);
      if (newJob) setJob(null);
      const value = checkTarget(await checkPrefixPairCases(specification, proposal, attempt), attempt);
      if (mounted.current) setJob(value);
    } catch (err) {
      if (mounted.current) setError(err.message || String(err));
    } finally { inFlight.current = false; if (mounted.current) setBusy(false); }
  }

  let comparison = null;
  try {
    const value = JSON.parse(job?.internal_receipt || "null");
    if (job?.run_status === "Completed" && job?.admission_phase === "settled" && value?.format === "brassclaw-prefix-pair-case-comparison/1" && typeof value.all_cases_match === "boolean" && value.specification === specification && value.proposal === proposal) comparison = value;
  } catch { /* Failure and pending states are not comparison verdicts. */ }
  return html`<div className="space-y-2">
    <${Button} size="sm" variant="ghost" onClick=${() => setOpen(value => !value)}>${t("prefix.pairCases")}<//>
    ${open && html`<div className="space-y-2 rounded border border-[var(--v2-panel-border)] p-3">
      <p>${t("prefix.pairCasesNotice")}</p>
      ${error && html`<p role="alert">${error}</p>`}
      <${Button} size="sm" disabled=${busy} onClick=${() => submit()}>${t(id ? "prefix.pairRetrySame" : "prefix.pairCheck")}<//>
      ${id && html`<div>
        <code className="break-all">${id}</code>
        <${Button} size="sm" variant="ghost" disabled=${busy} onClick=${refresh}>${t("common.refresh")}<//>
        <p role="status">${job?.run_status || t("prefix.measureUnknown")} · ${job?.admission_phase || ""}</p>
        ${comparison && html`<p role="status">${t(comparison.all_cases_match === true ? "prefix.pairMatches" : "prefix.pairMismatch")}</p>`}
        ${job && html`<details><summary>${t("prefix.measureDetails")}</summary><pre className="overflow-auto whitespace-pre-wrap break-all">${JSON.stringify(job, null, 2)}</pre></details>`}
        ${terminal.has(job?.run_status) && job?.admission_phase === "settled" && html`<${Button} size="sm" variant="ghost" disabled=${busy} onClick=${() => submit(true)}>${t("prefix.pairNewCheck")}<//>`}
      </div>`}
    </div>`}
  </div>`;
}
