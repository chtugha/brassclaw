import { React, html } from "../../../lib/html.js";
import { useT } from "../../../lib/i18n.js";
import { Button } from "../../../design-system/button.js";
import { useQueryClient } from "@tanstack/react-query";
import { retainExtensionManualTokenOperation, recordExtensionManualTokenSetup,
  completeExtensionManualTokenOperation } from "../../chat/lib/manual-token-operation.js";
import { prepareExtensionManualToken, submitExtensionManualToken,
  continueExtensionManualToken, cancelExtensionManualToken } from "../lib/extensions-api.js";

// No React Query mutation variables/cache retain raw input. Only the live input
// and its one deliberate asynchronous submit hold it; all stored state is redacted.
export function ManualTokenSetup({ packageRef, secret }) {
  const t = useT(), queryClient = useQueryClient();
  const [token, setToken] = React.useState("");
  const [busy, setBusy] = React.useState(false);
  const [message, setMessage] = React.useState("");
  const [closed, setClosed] = React.useState(false);
  const operationRef = React.useRef(null), generation = React.useRef(0), active = React.useRef(false);
  const context = { packageId: packageRef.id, requirement: secret.name,
    provider: secret.provider, accountLabel: secret.setup?.account_label,
    invocationId: secret.setup?.invocation_id };
  const contextKey = JSON.stringify(context);
  const liveContext = React.useRef(contextKey);
  liveContext.current = contextKey;
  const initialTarget = () => secret.credential_ref
    ? { kind: "existing", account_id: secret.credential_ref } : { kind: "new" };

  React.useEffect(() => {
    generation.current += 1;
    operationRef.current = null;
    active.current = false;
    setToken(""); setBusy(false); setMessage(""); setClosed(false);
    try {
      const saved = retainExtensionManualTokenOperation(context, { create: false });
      operationRef.current = saved; setClosed(saved.closed);
    } catch {
      // Absence and corrupt persistence are resolved explicitly before transport;
      // neither creates a replacement original merely by rendering a card.
    }
    return () => { generation.current += 1; active.current = false; };
  }, [contextKey]);

  const run = async (action) => {
    if (active.current) return;
    active.current = true;
    const version = generation.current;
    const current = () => version === generation.current && liveContext.current === contextKey;
    setBusy(true); setMessage("");
    try {
      const operation = retainExtensionManualTokenOperation(context, { target: initialTarget(), create: action === "raw" });
      operationRef.current = operation;
      if (operation.closed) { setClosed(true); setMessage(t("extensions.manual.closed")); return; }
      if (action === "cancel") {
        const receipt = await cancelExtensionManualToken(packageRef, operation);
        if (!current()) return;
        const saved = recordExtensionManualTokenSetup(operation, context, receipt);
        operationRef.current = saved; setClosed(saved.closed);
        // Cancel and publish can race. A completed receipt needs explicit
        // reference confirmation; it is never displayed as canceled.
        setMessage(t(saved.closed ? "extensions.manual.closed" : "extensions.manual.confirmSaved"));
        return;
      }
      const receipt = await prepareExtensionManualToken(packageRef, operation);
      if (!current()) return;
      const prepared = recordExtensionManualTokenSetup(operation, context, receipt);
      operationRef.current = prepared;
      if (prepared.closed) { setClosed(true); setMessage(t("extensions.manual.closed")); return; }
      let result;
      if (action === "reference") result = await continueExtensionManualToken(packageRef, prepared);
      else {
        if (!token.trim()) { setMessage(t("extensions.manual.required")); return; }
        result = await submitExtensionManualToken(packageRef, prepared, token);
      }
      if (!current()) return;
      operationRef.current = completeExtensionManualTokenOperation(prepared, context, result);
      setClosed(true); setMessage(t("extensions.manual.saved"));
      queryClient.invalidateQueries({ queryKey: ["extensions"] });
      queryClient.invalidateQueries({ queryKey: ["extension-setup", packageRef.id] });
    } catch {
      if (current()) setMessage(t("extensions.manual.unknown"));
    } finally {
      if (current()) { setToken(""); setBusy(false); active.current = false; }
    }
  };
  const startNew = () => {
    try {
      operationRef.current = retainExtensionManualTokenOperation(context, { target: initialTarget(), startNew: true });
      generation.current += 1; setClosed(false); setToken(""); setMessage("");
    } catch { setMessage(t("extensions.manual.unknown")); }
  };
  return html`<div className="space-y-2">
    <p className="text-xs text-iron-300">${t("extensions.manual.help")}</p>
    <input type="password" aria-label=${secret.prompt || secret.name} value=${token}
      autoComplete="off" disabled=${busy || closed}
      onChange=${event => setToken(event.target.value)}
      className="h-10 w-full rounded-md border border-white/12 bg-white/[0.04] px-3 text-sm text-iron-100" />
    <div className="flex flex-wrap gap-2">
      <${Button} onClick=${() => run("raw")} disabled=${busy || closed || !token.trim()}>${t("extensions.manual.save")}<//>
      <${Button} variant="secondary" onClick=${() => run("reference")} disabled=${busy || closed}>${t("extensions.manual.continue")}<//>
      <${Button} variant="ghost" onClick=${() => run("cancel")} disabled=${busy || closed}>${t("extensions.manual.cancel")}<//>
      ${closed && html`<${Button} variant="secondary" onClick=${startNew} disabled=${busy}>${t("extensions.manual.new")}<//>`}
    </div>
    ${message && html`<p role="status" className="text-xs text-iron-300">${message}</p>`}
  </div>`;
}
