import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  deleteLlmProvider,
  fetchLlmProviders,
  listLlmProviderModels,
  setActiveLlm,
  testLlmProviderConnection,
  upsertLlmProvider,
} from "../lib/settings-api.js";
import {
  isProviderConfigured,
  providerDefaultModel,
  providerMissingReason,
  providerModelDiscoveryPolicy,
  providerRequestPolicy,
  providerEmbeddingModels,
} from "../lib/llm-providers.js";

// The v2 `/llm/providers` snapshot is the single source of truth: a unified
// provider list (built-in + operator-defined) already annotated with the active
// selection, `builtin`, and `api_key_set`. Overrides are no longer a separate
// client-side merge — the backend resolves them — so `builtinOverrides` is kept
// as an empty object purely for the shared helper signatures.
export function useLlmProviders({ settings: _settings, gatewayStatus }) {
  const queryClient = useQueryClient();
  const providersQuery = useQuery({
    queryKey: ["llm-providers"],
    queryFn: fetchLlmProviders,
    staleTime: 60_000,
  });

  const snapshot = providersQuery.data || { providers: [], active: null };
  const builtinOverrides = {};
  // Map the wire view onto the field names the components/helpers expect.
  const allProviders = (snapshot.providers || []).map((provider) => ({
    ...provider,
    name: provider.description,
    has_api_key: provider.api_key_set === true,
  }));
  const inactiveProviders = (snapshot.inactive_providers || []).map((provider) => ({
    ...provider, name: provider.description, has_api_key: provider.api_key_set === true,
  }));
  // Whether the backend has a usable active provider. Prefer the persisted
  // operator snapshot, but also honor runtime/env-configured LLMs surfaced by
  // gateway status so first-run onboarding does not mask an already-live model.
  const hasActiveProvider = Boolean(
    snapshot.active?.provider_id || gatewayStatus?.llm_backend
  );
  const activeProviderId =
    snapshot.active?.provider_id || gatewayStatus?.llm_backend || "";
  const selectedModel = snapshot.active?.model || gatewayStatus?.llm_model || "";
  const builtinProviders = allProviders.filter((provider) => provider.builtin);
  const customProviders = allProviders.filter((provider) => !provider.builtin);
  const providers = [...allProviders].sort((a, b) => {
    if (a.id === activeProviderId) return -1;
    if (b.id === activeProviderId) return 1;
    return (a.name || a.id).localeCompare(b.name || b.id);
  });

  const refresh = () => {
    queryClient.invalidateQueries({ queryKey: ["llm-providers"] });
  };

  const setActiveMutation = useMutation({
    mutationFn: async (provider) => {
      if (!isProviderConfigured(provider, builtinOverrides)) {
        const reason = providerMissingReason(provider, builtinOverrides);
        throw new Error(reason === "base_url" ? "base_url" : "api_key");
      }
      const model = providerDefaultModel(provider, builtinOverrides);
      if (!model) throw new Error("model");
      await setActiveLlm({ provider_id: provider.id, model });
      return provider;
    },
    onSuccess: refresh,
  });

  // All origins edit the same DB definition with its displayed revision.
  const saveProviderMutation = useMutation({
    mutationFn: async ({ provider, form, apiKey, editingProvider }) => {
      const editing = provider || editingProvider;
      const id = (editing?.id || form.id.trim()).trim();
      const payload = {
        id,
        expected_revision: editing?.revision,
        model_discovery: providerModelDiscoveryPolicy(form),
        request_policy: providerRequestPolicy(form),
        embedding_models: providerEmbeddingModels(form),
        response_cache: { enabled: form.cacheEnabled === true, max_entries: Number(form.cacheEntries), ttl_secs: Number(form.cacheTtlSecs) },
        name: form.name.trim(),
        adapter: form.adapter,
        base_url: form.baseUrl.trim() || editing?.base_url || "",
        default_model: form.model.trim() || undefined,
      };
      // Only send a key when a new value was typed; otherwise leave the stored
      // one untouched (omitting the field is "unchanged" on the backend).
      if (apiKey.trim()) {
        payload.api_key = apiKey.trim();
      }
      if ((editingProvider || provider)?.id === activeProviderId && payload.default_model) {
        payload.set_active = true;
        payload.model = payload.default_model;
      }
      await upsertLlmProvider(payload);
      return payload;
    },
    onSuccess: refresh,
    onError: refresh,
  });

  const deleteCustomMutation = useMutation({
    mutationFn: async (provider) => {
      await deleteLlmProvider(provider.id, provider.revision);
      return provider;
    },
    onSuccess: refresh,
    onError: refresh,
  });

  const deactivateMutation = useMutation({
    mutationFn: async () => {
      // Deactivate by setting empty provider_id
      await setActiveLlm({ provider_id: "", model: "" });
    },
    onSuccess: refresh,
  });

  const setSempaiMutation = useMutation({
    mutationFn: async (provider) => {
      const model = providerDefaultModel(provider, builtinOverrides);
      await setActiveLlm({ provider_id: provider.id, model, role: "sempai" });
      return provider;
    },
    onSuccess: refresh,
  });

  const setEmbeddingMutation = useMutation({
    mutationFn: async (provider) => {
      const model = providerDefaultModel(provider, builtinOverrides);
      await setActiveLlm({ provider_id: provider.id, model, role: "embedding" });
      return provider;
    },
    onSuccess: refresh,
  });

  // Derived embedding state from snapshot
  const embeddingProviderId = snapshot.embedding_active?.provider_id || "";

  return {
    providers,
    builtinProviders,
    customProviders,
    builtinOverrides,
    inactiveProviders,
    responseCacheDefaults: snapshot.response_cache_defaults,
    modelDiscoveryDefaults: snapshot.model_discovery_defaults,
    requestPolicyDefaults: snapshot.request_policy_defaults,
    activeProviderId,
    selectedModel,
    hasActiveProvider,
    embeddingProviderId,
    isLoading: providersQuery.isLoading,
    error: providersQuery.error,
    setActiveProvider: (provider) => setActiveMutation.mutateAsync(provider),
    deactivateProvider: () => deactivateMutation.mutateAsync(),
    saveCustomProvider: (payload) => saveProviderMutation.mutateAsync(payload),
    saveBuiltinProvider: (payload) => saveProviderMutation.mutateAsync(payload),
    deleteCustomProvider: (provider) => deleteCustomMutation.mutateAsync(provider),
    setSempaiProvider: (provider) => setSempaiMutation.mutateAsync(provider),
    setEmbeddingProvider: (provider) => setEmbeddingMutation.mutateAsync(provider),
    testConnection: testLlmProviderConnection,
    listModels: listLlmProviderModels,
    isBusy:
      setActiveMutation.isPending ||
      saveProviderMutation.isPending ||
      deleteCustomMutation.isPending ||
      deactivateMutation.isPending ||
      setSempaiMutation.isPending ||
      setEmbeddingMutation.isPending,
  };
}
