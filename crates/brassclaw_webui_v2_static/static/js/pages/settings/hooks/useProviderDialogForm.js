import { React } from "../../../lib/html.js";
import {
  isValidProviderId,
  providerEffectiveBaseUrl,
  providerIdFromName,
  providerDefaultModel,
  providerPayload,
  providerModelDiscoveryPolicy,
  providerRequestPolicy,
  providerEmbeddingModels,
} from "../lib/llm-providers.js";

function initialForm(provider, overrides, cacheDefaults, discoveryDefaults, requestDefaults) {
  const cache = provider?.response_cache || cacheDefaults;
  const discovery = provider?.model_discovery || discoveryDefaults;
  const request = provider?.request_policy || requestDefaults;
  return {
    name: provider?.name || "",
    id: provider?.id || "",
    adapter: provider?.adapter || "open_ai_completions",
    baseUrl: provider ? providerEffectiveBaseUrl(provider, overrides) : "",
    model: provider ? providerDefaultModel(provider, overrides) : "",
    embeddingModels: Object.entries(provider?.embedding_models || {}).map(([model, contract]) => ({
      model, dimension: String(contract.dimension), cacheMaxEntries: String(contract.cache_max_entries), vectorRevision: String(contract.vector_revision ?? 1),
    })),
    retryBase: request?.retry_timing?.base_delay_ms ?? "",
    retryMax: request?.retry_timing?.max_backoff_ms ?? "",
    retryJitter: request?.retry_timing?.jitter_percent ?? "",
    retryAfterMax: request?.retry_timing?.max_retry_after_secs ?? "",
    requestTimeout: request?.timeout_secs ?? "",
    responseBytes: request ? request.max_response_bytes ?? 0 : "",
    requestRetries: request?.max_retries ?? "",
    circuitThreshold: request ? request.circuit_breaker_threshold ?? 0 : "",
    circuitRecovery: request?.circuit_breaker_recovery_secs ?? "",
    discoveryTimeout: discovery?.timeout_secs ?? "",
    discoveryBytes: discovery?.max_response_bytes ?? "",
    cacheEnabled: cache?.enabled === true,
    cacheEntries: cache?.max_entries ?? "",
    cacheTtlSecs: cache?.ttl_secs ?? "",
  };
}

export function useProviderDialogForm({
  provider,
  allProviderIds,
  builtinOverrides,
  responseCacheDefaults,
  modelDiscoveryDefaults,
  requestPolicyDefaults,
  open,
  onClose,
  onSave,
  onTest,
  onListModels,
  t,
}) {
  const [form, setForm] = React.useState(() => initialForm(provider, builtinOverrides, responseCacheDefaults, modelDiscoveryDefaults, requestPolicyDefaults));
  const [apiKey, setApiKey] = React.useState("");
  const [models, setModels] = React.useState([]);
  const [message, setMessage] = React.useState(null);
  const [busy, setBusy] = React.useState("");
  const idEditedRef = React.useRef(Boolean(provider));

  React.useEffect(() => {
    if (!open) return;
    setForm(initialForm(provider, builtinOverrides, responseCacheDefaults, modelDiscoveryDefaults, requestPolicyDefaults));
    setApiKey("");
    setModels([]);
    setMessage(null);
    setBusy("");
    idEditedRef.current = Boolean(provider);
  }, [open, provider, builtinOverrides, responseCacheDefaults, modelDiscoveryDefaults, requestPolicyDefaults]);

  const isBuiltin = provider?.builtin === true;
  const isEditing = Boolean(provider);

  const update = React.useCallback((key, value) => {
    setForm((prev) => {
      const next = { ...prev, [key]: value };
      if (key === "name" && !idEditedRef.current) next.id = providerIdFromName(value);
      return next;
    });
  }, []);

  const validate = React.useCallback(() => {
    if (!form.name.trim() || !form.id.trim()) return t("llm.fieldsRequired");
    if (!isValidProviderId(form.id.trim())) return t("llm.invalidId");
    if (!isEditing && !isBuiltin && allProviderIds.includes(form.id.trim())) {
      return t("llm.idTaken", { id: form.id.trim() });
    }
    if ([form.cacheEntries, form.cacheTtlSecs].some((value) => String(value).trim() === "" || !Number.isSafeInteger(Number(value)) || Number(value) < 0)) {
      return t("llm.responseCacheInvalid");
    }
    try {
      providerModelDiscoveryPolicy(form);
    } catch {
      return t("llm.modelDiscoveryInvalid");
    }
    try { providerRequestPolicy(form); } catch { return t("llm.requestPolicyInvalid"); }
    try { providerEmbeddingModels(form); } catch { return t("llm.embeddingModelsInvalid"); }
    return "";
  }, [allProviderIds, form.id, form.name, form.cacheEntries, form.cacheTtlSecs, form.discoveryTimeout, form.discoveryBytes, form.adapter, form.responseBytes, form.requestTimeout, form.requestRetries, form.circuitThreshold, form.circuitRecovery, form.retryBase, form.retryMax, form.retryJitter, form.retryAfterMax, form.embeddingModels, isBuiltin, isEditing, t]);

  const submit = React.useCallback(async () => {
    const error = validate();
    if (error) {
      setMessage({ tone: "error", text: error });
      return;
    }
    setBusy("save");
    try {
      await onSave({ form, apiKey, provider });
      onClose();
    } catch (err) {
      setMessage({ tone: "error", text: err.status === 409 ? t("llm.providerChanged") : err.message });
    } finally {
      setBusy("");
    }
  }, [apiKey, form, onClose, onSave, provider, validate]);

  const runTest = React.useCallback(async () => {
    try { providerModelDiscoveryPolicy(form); } catch {
      setMessage({ tone: "error", text: t("llm.modelDiscoveryInvalid") });
      return;
    }
    setBusy("test");
    try {
      const result = await onTest(providerPayload(provider, form, apiKey, builtinOverrides));
      setMessage({ tone: result.ok ? "success" : "error", text: result.message });
    } catch (err) {
      // Extract user-friendly error message from API error payload
      const errorText = err.payload?.field
        ? `${err.payload.field}: ${err.payload.validation_code || 'invalid'}`
        : err.body || err.message || t("llm.connectionTestFailed");
      setMessage({ tone: "error", text: errorText });
    } finally {
      setBusy("");
    }
  }, [apiKey, builtinOverrides, form, onTest, provider, t]);

  const fetchModels = React.useCallback(async () => {
    if (!form.baseUrl.trim()) {
      setMessage({ tone: "error", text: t("llm.baseUrlRequired") });
      return;
    }
    try { providerModelDiscoveryPolicy(form); } catch {
      setMessage({ tone: "error", text: t("llm.modelDiscoveryInvalid") });
      return;
    }
    setBusy("models");
    try {
      const result = await onListModels(providerPayload(provider, form, apiKey, builtinOverrides));
      if (!result.ok || !Array.isArray(result.models) || !result.models.length) {
        setMessage({ tone: "error", text: result.message || t("llm.modelsFetchFailed") });
      } else {
        setModels(result.models);
        // Discovery supplies suggestions; the operator keeps/selects the model.
        setMessage({ tone: "success", text: t("llm.modelsFetched", { count: result.models.length }) });
      }
    } catch (err) {
      setMessage({ tone: "error", text: err.status === 409 ? t("llm.providerChanged") : err.message });
    } finally {
      setBusy("");
    }
  }, [apiKey, builtinOverrides, form, onListModels, provider, t]);

  return {
    form,
    apiKey,
    models,
    message,
    busy,
    isBuiltin,
    isEditing,
    setApiKey,
    update,
    submit,
    runTest,
    fetchModels,
    markIdEdited: () => {
      idEditedRef.current = true;
    },
  };
}
