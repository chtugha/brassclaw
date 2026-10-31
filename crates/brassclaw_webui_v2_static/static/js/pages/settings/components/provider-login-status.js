import { html } from "../../../lib/html.js";
import { useT } from "../../../lib/i18n.js";
import { Button } from "../../../design-system/button.js";

// Shared status surface for the NEAR AI / Codex login flows driven by
// `useProviderLogin`. Renders the Codex device code (when issued) plus the
// waiting / error messages for both providers. Both the onboarding screen and
// the Settings → Inference tab drop this in so the two surfaces stay identical.
export function ProviderLoginStatus({ login }) {
  const t = useT();
  const { nearaiBusy, nearaiError, codexBusy, codexError, codexCode, codexStatus, codexTarget, codexCanRetry } = login;

  return html`
    ${nearaiBusy &&
    html`<div className="text-center text-xs text-[var(--v2-text-muted)]">
      ${t("onboarding.nearaiWaiting")}
    </div>`}
    ${nearaiError &&
    html`<div className="text-center text-xs text-red-300">${nearaiError}</div>`}

    ${codexTarget && html`<p className="text-center text-xs text-[var(--v2-text-muted)]">
      ${codexTarget.provider_id} · ${codexTarget.role} · ${codexTarget.model}
    </p>`}
    ${codexStatus && html`<p role="status" className="text-center text-xs text-[var(--v2-text-muted)]">
      ${t(`onboarding.codexStatus.${codexStatus.status}`)}
      ${codexStatus.error && html` · ${t("onboarding.codexStatusError", { code: codexStatus.error })}`}
    </p>`}
    ${codexCanRetry && html`<div className="text-center">
      <${Button} type="button" variant="secondary" size="sm" onClick=${login.retryCodex}>${t("onboarding.codexResume")}<//>
    </div>`}
    ${codexCode &&
    html`<div
      className="mx-auto max-w-md rounded-lg border border-[var(--v2-border)] bg-[var(--v2-surface-raised)] p-4 text-center"
    >
      <div className="text-xs text-[var(--v2-text-muted)]">
        ${t("onboarding.codexEnterCode")}
      </div>
      <div className="mt-2 font-mono text-2xl font-semibold tracking-[0.3em] text-[var(--v2-text-strong)]">
        ${codexCode.userCode}
      </div>
      <a
        className="mt-2 inline-block text-xs underline hover:text-[var(--v2-text-strong)]"
        href=${codexCode.verificationUri}
        target="_blank"
        rel="noopener noreferrer"
      >
        ${codexCode.verificationUri}
      </a>
    </div>`}
    ${codexBusy &&
    html`<div className="text-center text-xs text-[var(--v2-text-muted)]">
      ${t("onboarding.codexWaiting")}
    </div>`}
    ${codexError &&
    html`<div className="text-center text-xs text-red-300">${codexError}</div>`}
  `;
}
