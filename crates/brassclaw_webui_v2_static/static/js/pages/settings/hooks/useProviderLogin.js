import { useQueryClient } from "@tanstack/react-query";
import { React } from "../../../lib/html.js";
import { useT } from "../../../lib/i18n.js";
import { waitForProviderLogin, codexLoginSelection, findCodexLoginOriginal,
  codexLoginIsTerminal, runCodexLogin } from "../lib/provider-login.js";
import { readStoredToken, readOriginalAuthContext, originalAuthContextIsCurrent } from "../../../lib/api.js";
import {
  startNearaiLogin, fetchLlmProviders,
} from "../lib/settings-api.js";

// NEAR's separate legacy observation contract is unchanged by Codex036.
const NEARAI_POLL_DEADLINE_MS = 300_000;
const NEARAI_POLL_INTERVAL_MS = 2000;

// Shared login presentation for onboarding and Inference. NEAR browser login
// waits for its exact publication receipt. Codex retains exact original bytes
// and observes its own native status/receipt, never the active provider name.
export function useProviderLogin({ onSuccess } = {}) {
  const t = useT();
  const queryClient = useQueryClient();
  const loginGeneration = React.useRef(0);
  const codexFlight = React.useRef(null), loginAbort = React.useRef(null), lastCodex = React.useRef(null);
  React.useEffect(() => () => { loginGeneration.current += 1; loginAbort.current?.abort(); }, []);

  const [nearaiBusy, setNearaiBusy] = React.useState(false);
  const [nearaiError, setNearaiError] = React.useState("");
  const [codexBusy, setCodexBusy] = React.useState(false);
  const [codexError, setCodexError] = React.useState("");
  const [codexCode, setCodexCode] = React.useState(null);
  const [codexStatus, setCodexStatus] = React.useState(null);
  const [codexCanRetry, setCodexCanRetry] = React.useState(false);
  const [codexTarget, setCodexTarget] = React.useState(null);

  const finishActive = React.useCallback(async (isCurrent = () => true, original = null) => {
    if (!isCurrent()) return;
    if (original) {
      // fetchQuery otherwise deduplicates against a pre-commit GET, even with
      // staleTime: 0. Cancel and await that exact key before starting a new GET.
      await queryClient.cancelQueries({ queryKey: ["llm-providers"], exact: true });
      if (!isCurrent()) return;
    }
    await queryClient.invalidateQueries({ queryKey: ["llm-providers"], exact: true,
      ...(original ? { refetchType: "none" } : {}) });
    if (!isCurrent()) return;
    if (original) {
      const snapshot = await queryClient.fetchQuery({ queryKey: ["llm-providers"], queryFn: fetchLlmProviders, staleTime: 0 });
      if (!isCurrent()) return;
      const selection = snapshot[original.role === "kohai" ? "kohai_active" : "sempai_active"];
      const provider = snapshot.providers?.find(provider => provider.id === original.provider_id);
      // The old receipt confirms only its original. A newer head or role must
      // remain visible and cannot trigger success navigation for that original.
      if (selection?.provider_id !== original.provider_id || selection?.model !== original.model ||
          provider?.native_auth?.login_mutation_id !== original.mutation_id ||
          provider.native_auth.registration !== "bound" || provider.native_auth.access !== "current") return;
    }
    if (onSuccess && isCurrent()) onSuccess();
  }, [queryClient, onSuccess]);

  const startNearai = React.useCallback(
    async (provider) => {
      loginAbort.current?.abort();
      setCodexBusy(false);
      const generation = ++loginGeneration.current;
      const isCurrent = () => loginGeneration.current === generation;
      setNearaiError("");
      setNearaiBusy(true);
      try {
        const { auth_url: authUrl, mutation_id: mutationId } = await startNearaiLogin({
          provider,
          origin: window.location.origin,
        });
        if (!isCurrent()) return;
        window.open(authUrl, "_blank", "noopener");
        if (await waitForProviderLogin(mutationId, "nearai", NEARAI_POLL_DEADLINE_MS, NEARAI_POLL_INTERVAL_MS, isCurrent)) {
          await finishActive(isCurrent);
          return;
        }
        if (isCurrent()) setNearaiError(t("onboarding.nearaiTimeout"));
      } catch (_err) {
        if (isCurrent()) setNearaiError(t("onboarding.nearaiFailed"));
      } finally {
        if (isCurrent()) setNearaiBusy(false);
      }
    },
    [finishActive, t]
  );

  const startCodex = React.useCallback((provider, role = "kohai", model = provider?.default_model, { retryOnly = false } = {}) => {
    let selection, context, token, original;
    try {
      // Freeze every clicked precondition before any asynchronous work.
      selection = codexLoginSelection(provider, role, model);
      token = readStoredToken();
      context = readOriginalAuthContext();
      original = findCodexLoginOriginal(context, selection);
    } catch {
      setCodexError(t("onboarding.codexFailed"));
      setCodexBusy(false);
      return Promise.resolve();
    }
    const key = JSON.stringify([context.origin, context.nonce, selection.provider_id, role]);
    if (codexFlight.current?.key === key) return codexFlight.current.promise;
    const newOriginal = !retryOnly && codexLoginIsTerminal(original);
    // Merely receiving a terminal status never starts a new issuer run.
    if (newOriginal && !window.confirm(t("onboarding.codexNewConfirm"))) return Promise.resolve();
    const generation = ++loginGeneration.current;
    loginAbort.current?.abort();
    const abort = new AbortController();
    loginAbort.current = abort;
    const current = () => {
      if (loginGeneration.current !== generation || abort.signal.aborted) return false;
      return originalAuthContextIsCurrent(context, token);
    };
    // Popup activation must remain in the original synchronous click stack.
    let popup = null;
    try {
      popup = window.open("about:blank", "_blank", "width=600,height=600");
      if (popup) popup.opener = null;
    } catch { popup = null; } // Code/link remains visible when the browser blocks a popup.
    let navigated = false, terminal = false;
    lastCodex.current = { provider: { id: provider.id, adapter: provider.adapter, revision: provider.revision,
      default_model: provider.default_model, selection_refs: { ...provider.selection_refs } }, role, model };
    setNearaiBusy(false); setCodexError(""); setCodexCode(null); setCodexStatus(null); setCodexBusy(true); setCodexCanRetry(false);
    const promise = (async () => {
      try {
        const result = await runCodexLogin(context, selection, { newOriginal, retryOnly, signal: abort.signal,
          isCurrent: current,
          onOriginal: original => { if (current()) { setCodexCanRetry(true); setCodexTarget(JSON.parse(original.bytes)); } },
          onCode: code => {
            if (!current()) return;
            setCodexCode(code);
            if (!code) { if (popup && !popup.closed) popup.close(); return; }
            if (popup && !popup.closed) { popup.location.href = code.verificationUri; navigated = true; }
          },
          onStatus: status => {
            if (!current()) return;
            setCodexStatus(status);
            if (["completed", "failed", "expired", "canceled"].includes(status.status)) setCodexCode(null);
            if (["callback_received", "completing", "completed", "failed", "expired", "canceled"].includes(status.status) && popup && !popup.closed) popup.close();
          },
        });
        if (!current()) return;
        terminal = Boolean(result.status && ["completed", "failed", "expired", "canceled"].includes(result.status.status));
        if (result.completed) await finishActive(current, { ...selection, mutation_id: result.status.mutation_id });
      } catch {
        if (current()) setCodexError(t("onboarding.codexFailed"));
      } finally {
        if (popup && !popup.closed && (terminal || !navigated || !current())) popup.close();
        if (codexFlight.current?.generation === generation) codexFlight.current = null;
        if (current()) setCodexBusy(false);
      }
    })();
    codexFlight.current = { key, generation, promise };
    return promise;
  }, [finishActive, t]);
  const retryCodex = React.useCallback(() => {
    const target = lastCodex.current;
    if (!target) return Promise.resolve();
    // Explicit resume supersedes only this observation, never the native ID.
    loginAbort.current?.abort();
    codexFlight.current = null;
    setCodexBusy(false);
    return startCodex(target.provider, target.role, target.model, { retryOnly: true });
  }, [startCodex]);

  return {
    nearaiBusy,
    nearaiError,
    codexBusy,
    codexError,
    codexCode,
    codexStatus,
    codexCanRetry,
    codexTarget,
    retryCodex,
    startNearai,
    startCodex,
  };
}
