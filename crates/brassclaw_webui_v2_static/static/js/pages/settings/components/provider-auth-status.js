import { html } from "../../../lib/html.js";
import { useT } from "../../../lib/i18n.js";

export function ProviderAuthStatus({ provider, compact = false }) {
  const t = useT(), status = provider?.native_auth;
  if (!status) return null;
  const registrationLabel = status.account
    ? `${t("llm.nativeAuth.account")}: ${t(`llm.nativeAuth.account.${status.account.status}`)}`
    : `${t("llm.nativeAuth.registration")}: ${t(`llm.nativeAuth.registration.${status.registration}`)}`;
  if (compact) return html`<div className="min-w-0 space-y-1 break-words px-4 py-2 text-xs" role="status">
    <p>${t("llm.nativeAuth.access")}: ${t(`llm.nativeAuth.access.${status.access}`)} · ${registrationLabel}</p>
    <p>${t("llm.nativeAuth.refresh")}: ${t(`llm.nativeAuth.refresh.${status.refresh}`)}${status.blocking_refresh?.error && html` · ${t("onboarding.codexStatusError", { code: status.blocking_refresh.error })}`}</p>
    ${status.capability !== "available" && html`<p>${t(`llm.nativeAuth.capability.${status.capability}`)}</p>`}
    ${!status.login_supported && html`<p>${t("llm.nativeAuth.loginUnsupported")}</p>`}
  </div>`;
  return html`<div className="min-w-0 space-y-1 break-words text-xs" role="status">
    <p>${t("llm.nativeAuth.title")} · ${t(`llm.nativeAuth.capability.${status.capability}`)}</p>
    <p>${t("llm.nativeAuth.registration")}: ${t(`llm.nativeAuth.registration.${status.registration}`)}</p>
    ${status.account && html`<p>${t("llm.nativeAuth.account")}: ${t(`llm.nativeAuth.account.${status.account.status}`)}</p>`}
    <p>${t("llm.nativeAuth.access")}: ${t(`llm.nativeAuth.access.${status.access}`)}</p>
    <p>${t("llm.nativeAuth.lifetime")}: ${t(`llm.nativeAuth.lifetime.${status.lifetime}`)}</p>
    ${status.account_expires_at && html`<p>${t("llm.nativeAuth.accountExpiry")}: ${status.account_expires_at}</p>`}
    ${status.secret_expires_at && html`<p>${t("llm.nativeAuth.secretExpiry")}: ${status.secret_expires_at}</p>`}
    <p>${t("llm.nativeAuth.refresh")}: ${t(`llm.nativeAuth.refresh.${status.refresh}`)}</p>
    ${status.blocking_refresh && html`<p>${t("llm.nativeAuth.original")}: ${status.blocking_refresh.id} · ${t(`llm.nativeAuth.outcome.${status.blocking_refresh.outcome}`)} · ${t(`llm.nativeAuth.publication.${status.blocking_refresh.publication}`)}${status.blocking_refresh.error && html` · ${t("onboarding.codexStatusError", { code: status.blocking_refresh.error })}`}</p>`}
    ${status.recovery && html`<p>${t(`llm.nativeAuth.recovery.${status.recovery}`)}</p>`}
    ${!status.login_supported && html`<p>${t("llm.nativeAuth.loginUnsupported")}</p>`}
    <p>${t("llm.nativeAuth.observed")}: ${status.observed_at}</p>
    <p>${t("llm.nativeAuth.help")}</p>
    <p>${t("llm.nativeAuth.serving")}</p>
  </div>`;
}
