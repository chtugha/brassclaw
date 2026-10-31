import { React, html } from "../../../lib/html.js";
import { Button } from "../../../design-system/button.js";
import { Modal } from "../../../design-system/modal.js";
import { fetchPrefixSources, fetchPrefixGenerations, registerPrefixSource } from "../lib/settings-api.js";

function emptySourceDraft(profile) {
  return { profile, reference: "", title: "", applicability: "", required: false };
}

export function PrefixSourcesPanel({ entry, t }) {
  const [open, setOpen] = React.useState(false);
  const [selection, setSelection] = React.useState({ profile: entry.name, generation: "" });
  const generation = selection.profile === entry.name ? selection.generation : "";
  const [retained, setGenerations] = React.useState({ profile: entry.name, items: [] });
  const generations = retained.profile === entry.name ? retained.items : [];
  const [inventoryRecord, setInventory] = React.useState(null);
  const inventory = inventoryRecord?.profile === entry.name && inventoryRecord.generation === generation ? inventoryRecord.data : null;
  const [storedDraft, setDraft] = React.useState(() => emptySourceDraft(entry.name));
  const draft = storedDraft.profile === entry.name ? storedDraft : emptySourceDraft(entry.name);
  const { reference, title, applicability, required } = draft;
  const [errorRecord, setError] = React.useState(null);
  const [generationErrorRecord, setGenerationError] = React.useState(null);
  const [saveError, setSaveError] = React.useState(null);
  const inventoryError = errorRecord?.profile === entry.name && errorRecord.generation === generation ? errorRecord.message : null;
  const generationError = generationErrorRecord?.profile === entry.name && generationErrorRecord.generation === generation ? generationErrorRecord.message : null;
  const saveMessage = saveError?.profile === entry.name ? saveError.message : null;
  const [saving, setSaving] = React.useState(false);
  const [tick, setTick] = React.useState(0);
  const busy = React.useRef(false);
  React.useEffect(() => {
    if (!open) return;
    let stopped = false; setInventory(null); setError(null); setGenerationError(null);
    // These independent reads must not discard each other's successful data.
    // Missing inventory is never replaced with invented source/status rows.
    fetchPrefixSources(entry.name, generation)
      .then(sources => { if (!stopped) setInventory({ profile: entry.name, generation, data: sources }); })
      .catch(err => { if (!stopped) setError({ profile: entry.name, generation, message: err.message }); });
    fetchPrefixGenerations(entry.name)
      .then(items => { if (!stopped) setGenerations({ profile: entry.name, items }); })
      .catch(err => { if (!stopped) setGenerationError({ profile: entry.name, generation, message: err.message }); });
    return () => { stopped = true; };
  }, [open, generation, entry.name, tick]);
  function editDraft(field, value) {
    setDraft(current => ({ ...(current.profile === entry.name ? current : emptySourceDraft(entry.name)), [field]: value }));
  }
  async function save(event) {
    event.preventDefault(); if (busy.current) return;
    busy.current = true; setSaving(true); setSaveError(null);
    try {
      await registerPrefixSource(entry.name, { reference, title: title || reference, applicability: applicability || entry.description, required });
      // A completion belongs to this exact submitted draft. Edits and profile
      // changes made during the request must not be cleared by its late reply.
      setDraft(current => current === draft ? emptySourceDraft(entry.name) : current);
      setTick(value => value + 1);
    } catch (err) { setSaveError({ profile: entry.name, message: err.message }); }
    finally { busy.current = false; setSaving(false); }
  }
  return html`<${React.Fragment}>
    <${Button} size="sm" variant="ghost" onClick=${() => setOpen(true)}>${t("prefix.sources")}<//>
    ${open && html`<${Modal} open=${open} onClose=${() => setOpen(false)} size="xl" title=${t("prefix.sources") + " — " + (entry.label || entry.name)}>
      <section className="space-y-4 overflow-auto p-5 text-sm">
        ${inventoryError && html`<p role="alert" className="text-red-300">${inventoryError}</p>`}
        ${generationError && html`<p role="alert" className="text-red-300">${generationError}</p>`}
        ${saveMessage && html`<p role="alert" className="text-red-300">${saveMessage}</p>`}
        <label>${t("prefix.sourceGeneration")}
          <select value=${generation} onChange=${event => setSelection({ profile: entry.name, generation: event.target.value })} className="ml-2 bg-transparent">
            <option value="">${t("prefix.registeredOnly")}</option>
            ${generations.map(g => html`<option key=${g.generation} value=${g.generation}>${g.generation} (${g.qualification})</option>`)}
          </select>
        </label>
        ${!inventory && !inventoryError && html`<p role="status">${t("common.loading")}</p>`}
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
          <label className="block">${t("prefix.addSource")}<input required value=${reference} onChange=${event => editDraft("reference", event.target.value)} className="block w-full rounded border bg-transparent p-2" /></label>
          <label className="block">${t("prefix.sourceTitle")}<input value=${title} onChange=${event => editDraft("title", event.target.value)} className="block w-full rounded border bg-transparent p-2" /></label>
          <label className="block">${t("prefix.sourceApplicability")}<input value=${applicability} onChange=${event => editDraft("applicability", event.target.value)} className="block w-full rounded border bg-transparent p-2" /></label>
          <label><input type="checkbox" checked=${required} onChange=${event => editDraft("required", event.target.checked)} /> ${t("prefix.sourceRequired")}</label>
          <${Button} type="submit" size="sm" disabled=${saving || !reference.trim()}>${t("prefix.saveSource")}<//>
        </form>
      </section>
    <//>`}
  <//>`;
}
