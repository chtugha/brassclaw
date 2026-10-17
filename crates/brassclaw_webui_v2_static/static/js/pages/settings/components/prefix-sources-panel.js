import { React, html } from "../../../lib/html.js";
import { Button } from "../../../design-system/button.js";
import { Modal } from "../../../design-system/modal.js";
import { fetchPrefixSources, fetchPrefixGenerations, registerPrefixSource } from "../lib/settings-api.js";

export function PrefixSourcesPanel({ entry, t }) {
  const [open, setOpen] = React.useState(false);
  const [generation, setGeneration] = React.useState("");
  const [generations, setGenerations] = React.useState([]);
  const [inventory, setInventory] = React.useState(null);
  const [reference, setReference] = React.useState("");
  const [title, setTitle] = React.useState("");
  const [applicability, setApplicability] = React.useState("");
  const [required, setRequired] = React.useState(false);
  const [error, setError] = React.useState(null);
  const [saving, setSaving] = React.useState(false);
  const [tick, setTick] = React.useState(0);
  const busy = React.useRef(false);
  React.useEffect(() => {
    if (!open) return;
    let stopped = false; setInventory(null); setError(null);
    Promise.all([fetchPrefixSources(entry.name, generation), fetchPrefixGenerations(entry.name)])
      .then(([sources, retained]) => { if (!stopped) { setInventory(sources); setGenerations(retained); } })
      .catch(err => { if (!stopped) setError(err.message); });
    return () => { stopped = true; };
  }, [open, generation, entry.name, tick]);
  async function save(event) {
    event.preventDefault(); if (busy.current) return;
    busy.current = true; setSaving(true); setError(null);
    try {
      await registerPrefixSource(entry.name, { reference, title: title || reference, applicability: applicability || entry.description, required });
      setReference(""); setTitle(""); setApplicability(""); setTick(value => value + 1);
    } catch (err) { setError(err.message); }
    finally { busy.current = false; setSaving(false); }
  }
  return html`<${React.Fragment}>
    <${Button} size="sm" variant="ghost" onClick=${() => setOpen(true)}>${t("prefix.sources")}<//>
    ${open && html`<${Modal} open=${open} onClose=${() => setOpen(false)} size="xl" title=${t("prefix.sources") + " — " + (entry.label || entry.name)}>
      <section className="space-y-4 overflow-auto p-5 text-sm">
        ${error && html`<p role="alert" className="text-red-300">${error}</p>`}
        <label>${t("prefix.sourceGeneration")}
          <select value=${generation} onChange=${event => setGeneration(event.target.value)} className="ml-2 bg-transparent">
            <option value="">${t("prefix.registeredOnly")}</option>
            ${generations.map(g => html`<option key=${g.generation} value=${g.generation}>${g.generation} (${g.qualification})</option>`)}
          </select>
        </label>
        ${!inventory && !error && html`<p role="status">${t("common.loading")}</p>`}
        ${inventory && html`<div className="space-y-3">
          <h4>${t("prefix.usedSources")}</h4>
          ${inventory.used.length === 0 && html`<p>${t("prefix.noUsedSources")}</p>`}
          ${inventory.used.map(source => html`<div key=${source.reference} className="rounded border border-[var(--v2-panel-border)] p-2">
            <code className="break-all">${source.reference}</code><p>${source.disposition} · ${source.reason}</p>
            <p>${t("prefix.archivedSource")}: ${source.archived ? t("common.yes") : t("common.no")}</p>
            <details><summary>${t("prefix.fingerprint")}</summary><code>${source.checksum}</code></details>
          </div>`)}
          <h4>${t("prefix.registeredSources")}</h4>
          ${inventory.registered.map(source => html`<div key=${source.id} className="rounded border border-[var(--v2-panel-border)] p-2">
            <p>${source.title}</p><code className="break-all">${source.reference}</code>
            <p role="status" className=${source.availability === "available" ? "text-green-400" : source.availability === "unavailable" ? "text-red-400" : "text-[var(--v2-text-muted)]"}>${t("prefix.sourceAvailability." + source.availability)}</p>
            <p>${t("prefix.sourcePendingReview")}</p>
          </div>`)}
          ${!inventory.checks_available && html`<p>${t(inventory.check_blocked_reason || "prefix.sourceCheckPrerequisites")}</p>`}
          <${Button} size="sm" disabled=${!inventory.checks_available}>${t("prefix.checkSources")}<//>
        </div>`}
        <form onSubmit=${save} className="space-y-2 border-t border-[var(--v2-panel-border)] pt-3">
          <p>${t("prefix.sourceSaveNotice")}</p>
          <label className="block">${t("prefix.addSource")}<input required value=${reference} onChange=${event => setReference(event.target.value)} className="block w-full rounded border bg-transparent p-2" /></label>
          <label className="block">${t("prefix.sourceTitle")}<input value=${title} onChange=${event => setTitle(event.target.value)} className="block w-full rounded border bg-transparent p-2" /></label>
          <label className="block">${t("prefix.sourceApplicability")}<input value=${applicability} onChange=${event => setApplicability(event.target.value)} className="block w-full rounded border bg-transparent p-2" /></label>
          <label><input type="checkbox" checked=${required} onChange=${event => setRequired(event.target.checked)} /> ${t("prefix.sourceRequired")}</label>
          <${Button} type="submit" size="sm" disabled=${saving || !reference.trim()}>${t("prefix.saveSource")}<//>
        </form>
      </section>
    <//>`}
  <//>`;
}
