/**
 * AuthTokenCard — rendered when `gate.challengeKind === "manual_token"`.
 *
 * Status Pill + Drawer presentation (AuthGateShell). The drawer holds the
 * masked token input and submit/cancel actions. The token never leaves the
 * submit handler and is cleared on success.
 */
import { React, html } from "../../../lib/html.js";
import { useT } from "../../../lib/i18n.js";
import { Button } from "../../../design-system/button.js";
import { Input } from "../../../design-system/input.js";
import { AuthGateShell } from "./auth-gate-shell.js";

const TERMINAL_ERRORS = Object.freeze({
  manual_setup_canceled: "authGate.setupCanceled",
  manual_setup_expired: "authGate.setupExpired",
  manual_setup_failed: "authGate.setupFailed",
  manual_setup_unavailable: "authGate.setupUnavailable",
  manual_setup_terminal: "authGate.setupTerminal",
});

export function AuthTokenCard({ gate, onSubmit, onContinue, onCancel }) {
  const t = useT();
  const [token, setToken] = React.useState("");
  const [error, setError] = React.useState("");
  const [isSubmitting, setIsSubmitting] = React.useState(false);
  const [isTerminal, setIsTerminal] = React.useState(false);
  const recordFailure = React.useCallback((failure, fallback) => {
    const code = failure?.safeAuthGateCode;
    const terminal = typeof code === "string" && Object.hasOwn(TERMINAL_ERRORS, code)
      ? TERMINAL_ERRORS[code] : null;
    if (terminal) {
      setIsTerminal(true);
      setToken("");
    }
    setError(t(terminal || fallback));
  }, [t]);

  const submit = React.useCallback(
    async (event) => {
      event.preventDefault();
      const value = token;
      if (!value.trim()) {
        setError(t("authGate.tokenRequired"));
        return;
      }
      setError("");
      setIsSubmitting(true);
      try {
        await onSubmit(value);
        setToken("");
      } catch (failure) {
        recordFailure(failure, "authGate.operationUnconfirmed");
      } finally {
        setIsSubmitting(false);
      }
    },
    [onSubmit, recordFailure, t, token],
  );

  const continueSaved = React.useCallback(async () => {
    setError("");
    setIsSubmitting(true);
    try {
      await onContinue();
      setToken("");
    } catch (failure) {
      recordFailure(failure, "authGate.continueSavedFailed");
    } finally {
      setIsSubmitting(false);
    }
  }, [onContinue, recordFailure]);

  return html`
    <${AuthGateShell}
      icon="lock"
      headline=${gate?.headline || t("authGate.title")}
      provider=${gate?.provider || ""}
      accountLabel=${gate?.accountLabel || ""}
      body=${gate?.body || ""}
      pillHint=${t("authGate.pillEnterToken")}
    >
      <form onSubmit=${submit}>
        <div className="mb-3">
          <${Input}
            type="password"
            autoComplete="off"
            spellCheck=${false}
            value=${token}
            disabled=${isSubmitting || isTerminal}
            placeholder=${t("authGate.tokenPlaceholder")}
            aria-label=${t("authGate.tokenLabel")}
            error=${Boolean(error)}
            onInput=${(event) => setToken(event.currentTarget.value)}
          />
          ${error &&
          html`
            <p className="mt-2 text-xs text-[var(--v2-danger-text)]" role="alert">
              ${error}
            </p>
          `}
        </div>
        <div className="flex flex-wrap gap-2">
          <${Button} type="submit" variant="primary" disabled=${isSubmitting || isTerminal}>
            ${isSubmitting ? t("authGate.submitting") : t("authGate.submit")}
          <//>
          ${onContinue && html`
            <${Button} type="button" variant="secondary" disabled=${isSubmitting || isTerminal}
              onClick=${continueSaved} title=${t("authGate.continueSavedHelp")}>
              ${t("authGate.continueSaved")}
            <//>
          `}
          <${Button}
            type="button"
            variant="secondary"
            disabled=${isSubmitting}
            onClick=${() => onCancel?.()}
          >
            ${t("authGate.cancel")}
          <//>
        </div>
      </form>
    <//>
  `;
}
