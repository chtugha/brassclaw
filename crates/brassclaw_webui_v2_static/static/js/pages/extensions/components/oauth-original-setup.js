import { React, html } from "../../../lib/html.js";
import { useT } from "../../../lib/i18n.js";
import { Button } from "../../../design-system/button.js";
import { findOauthOriginal, retainOauthOriginal, recordOauthOriginal, renewOauthOriginal, checkedOauthCleanup } from "../lib/oauth-original.js";
import { sendExtensionOauthOriginal, extensionOauthExpiry } from "../lib/extensions-api.js";
import { useOauthSetupProgress } from "../hooks/useExtensions.js";

export function OAuthOriginalSetup({ packageRef, secret }) {
  const t = useT();
  const { refreshSetupState, watchOauthProgress, clearWatcher } = useOauthSetupProgress(packageRef);
  const [operation, setOperation] = React.useState(null), [busy, setBusy] = React.useState(false);
  const [message, setMessage] = React.useState("");
  const [authorizationUrl,setAuthorizationUrl]=React.useState(null);
  const [cleanup,setCleanup]=React.useState(null);
  const generation = React.useRef(0), active = React.useRef(false);
  const context = { packageId: packageRef.id, requirement: secret.name, provider: secret.provider,
    accountLabel: secret.setup?.account_label || `${secret.provider} credential`,
    scopes: secret.setup?.scopes || [], invocationId: secret.setup?.invocation_id };
  const initialTarget = () => secret.credential_ref
    ? { kind: "existing", account_id: secret.credential_ref } : { kind: "new" };
  const key = JSON.stringify(context), liveContext = React.useRef(key);
  liveContext.current = key;

  React.useEffect(() => {
    generation.current += 1; active.current = false;
    setOperation(null); setBusy(false); setMessage(""); setAuthorizationUrl(null); setCleanup(null);
    try { setOperation(findOauthOriginal(context)); }
    catch { setMessage(t("extensions.oauth.unknown")); }
    return () => { generation.current += 1; active.current = false; clearWatcher(); };
  }, [key]);

  const run = async (action) => {
    if (active.current) return;
    active.current = true;
    const version = generation.current;
    const current = () => version === generation.current && liveContext.current === key;
    const requestedTarget = initialTarget();
    setBusy(true); setMessage(""); setAuthorizationUrl(null);
    let popup = null;
    try {
      let selected = findOauthOriginal(context);
      // Retain popup activation before any new policy or recovery await.
      if (action === "new" || (action === "start" && !selected?.receipt?.terminal)) {
        popup=window.open("about:blank","_blank","width=600,height=600");
        if (popup) popup.opener=null;
      }
      if (!selected) {
        if (action !== "start") throw new Error("OAuth original could not be verified.");
        const expiresAt = await extensionOauthExpiry();
        if (!current()) { if (popup && !popup.closed) popup.close(); return; }
        selected = retainOauthOriginal(context, { expiresAt, target: requestedTarget });
      }
      setOperation(selected);
      if (action === "new") {
        // Renew only after a CURRENT owner read of the retained original; local
        // time, another account's readiness or an old response is insufficient.
        const receipt = await sendExtensionOauthOriginal(packageRef, selected, "recover");
        if (!current()) {if (popup && !popup.closed) popup.close(); return;}
        selected = recordOauthOriginal(selected, context, receipt);
        if (!selected.receipt?.terminal) { if (popup && !popup.closed) popup.close(); setOperation(selected); setMessage(t("extensions.oauth.unknown")); return; }
        const expiresAt = await extensionOauthExpiry();
        if (!current()) { if (popup && !popup.closed) popup.close(); return; }
        selected = renewOauthOriginal(selected, context, { expiresAt, target: requestedTarget });
        setOperation(selected); setCleanup(null);
      } else if (selected.receipt?.terminal && action !== "recover" && action !== "reconcile") {
        setMessage(t("extensions.oauth.closed")); return;
      }
      const verb = action === "new" ? "start" : action === "reconcile" ? "cancel" : action;
      const receipt = await sendExtensionOauthOriginal(packageRef, selected, verb);
      // Even after unmount, the server retains its actual original. A late
      // reply must never navigate a replacement card's window or refresh it.
      if (!current()) { if (popup && !popup.closed) popup.close(); return; }
      selected = recordOauthOriginal(selected, context, receipt);
      setCleanup(checkedOauthCleanup(receipt.cleanup));
      setOperation(selected); refreshSetupState();
      if (selected.receipt.terminal) {
        if (popup && !popup.closed) popup.close();
        setMessage(t("extensions.oauth.closed")); clearWatcher();
      } else if (selected.receipt.expired || Date.parse(selected.expiresAt) <= Date.now()) {
        if (popup && !popup.closed) popup.close();
        setMessage(t("extensions.oauth.expired"));
      } else if (selected.receipt.hasError || selected.receipt.status !== "awaiting_user" || receipt.authorization_url === null) {
        if (popup && !popup.closed) popup.close();
        setMessage(t("extensions.oauth.unknown"));
      } else if (verb === "start") {
        setAuthorizationUrl(receipt.authorization_url);
        if (popup && !popup.closed) popup.location.href = receipt.authorization_url;
        else popup = window.open(receipt.authorization_url, "_blank", "noopener,noreferrer");
        // The recovery link also needs observation when a popup is blocked.
        watchOauthProgress(popup, receipt.expires_at);
        setMessage(t("extensions.oauth.opened"));
      }
    } catch {
      if (popup && !popup.closed) popup.close();
      if (current()) setMessage(t("extensions.oauth.unknown"));
    } finally {
      if (current()) { active.current = false; setBusy(false); }
    }
  };

  return html`<div className="space-y-2 rounded-md border border-white/12 bg-white/[0.04] px-3 py-2">
    <p className="text-xs text-iron-300">${t("extensions.oauth.help")}</p>
    <div className="flex flex-wrap gap-2">
      ${operation?.receipt?.terminal
        ? html`<${Button} onClick=${() => run("new")} disabled=${busy}>${t("extensions.oauth.new")}<//>`
        : html`<${Button} onClick=${() => run("start")} disabled=${busy}>${t(busy ? "extensions.oauth.busy" : "extensions.oauth.start")}<//>`}
      <${Button} variant="secondary" onClick=${() => run("recover")} disabled=${busy || !operation}>${t("extensions.oauth.check")}<//>
      ${operation && !operation.receipt?.terminal && html`<${Button} variant="secondary" onClick=${() => run("cancel")} disabled=${busy}>${t("extensions.oauth.cancel")}<//>`}
      ${operation?.receipt?.terminal && cleanup?.can_reconcile && html`<${Button} variant="secondary" onClick=${() => run("reconcile")} disabled=${busy}>${t("extensions.oauth.reconcile")}<//>`}
    </div>
    ${cleanup && cleanup.remote !== "retained" && html`<div className="space-y-1 text-xs text-iron-300" role="status">
      <p>${t(`extensions.oauth.remote.${cleanup.remote}`)}</p>
      <p>${t(`extensions.oauth.local.${cleanup.local}`)}</p>
      ${cleanup.issue && html`<p>${t(`extensions.oauth.issue.${cleanup.issue}`)}</p>`}
      ${cleanup.http_status !== null && html`<p>${t("extensions.oauth.cleanupHttp")} ${cleanup.http_status}</p>`}
      ${cleanup.error && html`<p>${t("extensions.oauth.cleanupError")} ${cleanup.error}</p>`}
    </div>`}
    ${authorizationUrl && html`<a href=${authorizationUrl} target="_blank" rel="noopener noreferrer" className="text-xs text-mint underline">${t("extensions.oauth.open")}</a>`}
    ${message && html`<p className="text-xs text-iron-300" role="status">${message}</p>`}
  </div>`;
}
