import { useQueryClient } from "@tanstack/react-query";
import { React } from "../../../lib/html.js";
import { useT } from "../../../lib/i18n.js";
import { waitForProviderLogin } from "../lib/provider-login.js";
import {
  fetchLlmProviders,
  startCodexLogin,
  startNearaiLogin,
} from "../lib/settings-api.js";

// How long to keep polling the snapshot for a login to land before giving up.
// NEAR AI is a quick browser redirect; Codex device codes live ~15 minutes.
const NEARAI_POLL_DEADLINE_MS = 300_000;
const CODEX_POLL_DEADLINE_MS = 900_000;
const POLL_INTERVAL_MS = 2000;

// Poll the LLM snapshot until `providerId` becomes the active provider, or the
// deadline passes. Returns true on success.
async function pollUntilActive(providerId, deadlineMs) {
  const deadline = Date.now() + deadlineMs;
  while (Date.now() < deadline) {
    await new Promise((resolve) => setTimeout(resolve, POLL_INTERVAL_MS));
    const snapshot = await fetchLlmProviders().catch(() => null);
    if (snapshot?.active?.provider_id === providerId) {
      return true;
    }
  }
  return false;
}

// Shared login presentation for onboarding and Inference. NEAR browser login
// waits for its exact publication receipt; Codex retains the legacy provider
// polling until its dedicated credential/controller migration is completed.
export function useProviderLogin({ onSuccess } = {}) {
  const t = useT();
  const queryClient = useQueryClient();
  const loginGeneration = React.useRef(0);
  React.useEffect(() => () => { loginGeneration.current += 1; }, []);

  const [nearaiBusy, setNearaiBusy] = React.useState(false);
  const [nearaiError, setNearaiError] = React.useState("");
  const [codexBusy, setCodexBusy] = React.useState(false);
  const [codexError, setCodexError] = React.useState("");
  const [codexCode, setCodexCode] = React.useState(null);

  const finishActive = React.useCallback(async (isCurrent = () => true) => {
    await queryClient.invalidateQueries({ queryKey: ["llm-providers"] });
    if (onSuccess && isCurrent()) {
      onSuccess();
    }
  }, [queryClient, onSuccess]);

  const startNearai = React.useCallback(
    async (provider) => {
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
        if (await waitForProviderLogin(mutationId, "nearai", NEARAI_POLL_DEADLINE_MS, POLL_INTERVAL_MS, isCurrent)) {
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

  const startCodex = React.useCallback(async () => {
    setCodexError("");
    setCodexCode(null);
    setCodexBusy(true);
    try {
      const { user_code: userCode, verification_uri: verificationUri } =
        await startCodexLogin();
      setCodexCode({ userCode, verificationUri });
      window.open(verificationUri, "_blank", "noopener");
      if (await pollUntilActive("openai_codex", CODEX_POLL_DEADLINE_MS)) {
        await finishActive();
        return;
      }
      setCodexError(t("onboarding.codexTimeout"));
    } catch (_err) {
      setCodexError(t("onboarding.codexFailed"));
    } finally {
      setCodexBusy(false);
    }
  }, [finishActive, t]);

  return {
    nearaiBusy,
    nearaiError,
    codexBusy,
    codexError,
    codexCode,
    startNearai,
    startCodex,
  };
}
