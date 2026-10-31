import { React, html } from "../../../lib/html.js";
import { Button } from "../../../design-system/button.js";
import { SETUP_FIELDS, loadSetupDraft, pendingSetupDraft, retrySetupDraft, saveSetupDraft, setupEdits, setupStrings, subscribeSetupDraft } from "../lib/model-setup-draft.js";

const labels = { max_prefix_tokens: "llm.maxPrefixTokens", max_prompt_tokens: "llm.maxPromptTokens", reserved_output_tokens: "prefix.setupOutput", additional_framing_tokens: "prefix.setupFraming" };

// Mounted once per immutable specification, independent of the active pair.
export function PrefixModelSetupDraft({ specification, t }) {
  const id = specification.id;
  const [basis, setBasis] = React.useState(null);
  const [strings, setStrings] = React.useState({});
  const [touched, setTouched] = React.useState({});
  const [error, setError] = React.useState(null);
  const [busy, setBusy] = React.useState(false);
  const [stale, setStale] = React.useState(false);
  const [, redraw] = React.useReducer(value => value + 1, 0);
  const epoch = React.useRef(0);
  const flight = React.useRef(false);
  const adopt = draft => { setBasis(draft); setStrings(setupStrings(draft)); setTouched({}); setStale(false); };
  const pending = pendingSetupDraft(id);
  React.useEffect(() => {
    const generation = ++epoch.current;
    const held = pendingSetupDraft(id);
    const unsubscribe = subscribeSetupDraft(event => {
      redraw();
      if (epoch.current === generation && held && event?.specification === id && event.operation_id === held.request.operation_id) adopt(event.current);
    });
    if (held) {
      adopt(held.basis);
      setStrings({ ...setupStrings(held.basis), ...Object.fromEntries(held.request.edits.map(edit => [edit.field, edit.value === null ? "" : String(edit.value)])) });
      setTouched(Object.fromEntries(held.request.edits.map(edit => [edit.field, true])));
    } else {
      setBusy(true);
      loadSetupDraft(id).then(draft => { if (epoch.current === generation) adopt(draft); })
        .catch(err => { if (epoch.current === generation) setError(err.message); })
        .finally(() => { if (epoch.current === generation) setBusy(false); });
    }
    return () => { epoch.current++; unsubscribe(); };
  }, [id]);
  async function run(action, writing = false) {
    if (flight.current) return;
    flight.current = true; const generation = epoch.current; setBusy(true); setError(null);
    try {
      const draft = await action();
      if (epoch.current === generation) adopt(draft);
    } catch (err) {
      if (epoch.current === generation) { setError(err.message || String(err)); if (writing && !pendingSetupDraft(id)) setStale(true); }
    } finally { flight.current = false; if (epoch.current === generation) setBusy(false); }
  }
  const validBasis = basis?.specification === id;
  let valid = false;
  try { valid = validBasis && setupEdits(strings, touched).length > 0; } catch { /* visible invalid state; no dispatch */ }
  let configuredContext = null;
  try {
    const captured = JSON.parse(specification.specification_bytes);
    if (Number.isSafeInteger(captured.configured_context_window) && captured.configured_context_window > 0) configuredContext = captured.configured_context_window;
  } catch { /* original specification remains visible; never infer model limits */ }
  const dirty = Object.keys(touched).length > 0;
  const locked = busy || !!pending;
  return html`<section aria-label=${t("prefix.setupTitle")} data-setup-specification=${id} className="rounded-md border border-[var(--v2-panel-border)] p-3 space-y-2">
    <h4 className="font-mono text-sm">${t("prefix.setupTitle")}</h4>
    <p>${t("prefix.setupNotice")}</p>
    <p>${t("prefix.setupContext")}: ${configuredContext ?? t("prefix.setupUnknown")}</p>
    ${validBasis && html`<p data-setup-revision=${basis.revision}>${t("prefix.revision")} ${basis.revision}</p>`}
    ${error && html`<p role="alert">${error.startsWith("prefix.") ? t(error) : error}</p>`}
    ${pending && html`<p role="status">${t("prefix.setupPending")} <code>${pending.request.operation_id}</code></p>`}
    ${validBasis && html`<div className="grid gap-2 sm:grid-cols-2">${SETUP_FIELDS.map(field => html`<label key=${field} className="block">${t(labels[field])}
      <input data-setup-field=${field} type="text" inputMode="numeric" value=${strings[field] ?? ""} disabled=${locked} placeholder=${t("prefix.setupUnknown")} className="block w-full bg-[var(--v2-surface-soft)] p-2"
        onInput=${event => { const value = event.currentTarget.value; setStrings(previous => ({ ...previous, [field]: value })); setTouched(previous => ({ ...previous, [field]: true })); setError(null); }} />
      <small>${t(touched[field] ? "prefix.setupOriginManual" : `prefix.setupOrigin_${basis.fields[field].origin}`)}</small>
    </label>`)}</div>`}
    ${dirty && !valid && html`<p role="alert">${t("prefix.setupInvalid")}</p>`}
    <div className="flex gap-2">
      <${Button} size="sm" disabled=${locked || stale || !valid} onClick=${() => run(() => saveSetupDraft(basis, strings, touched), true)}>${t("prefix.setupSave")}<//>
      ${pending ? html`<${Button} size="sm" disabled=${busy || pending.busy} onClick=${() => run(() => retrySetupDraft(id), true)}>${t("prefix.setupRetry")}<//>` : html`<${Button} size="sm" variant="ghost" disabled=${busy} onClick=${() => run(() => loadSetupDraft(id))}>${t(dirty ? "prefix.setupDiscardReload" : "common.refresh")}<//>`}
    </div>
  </section>`;
}
