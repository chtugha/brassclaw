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
  modelSelectionRef,
  providerPublicationRows,
} from "../lib/llm-providers.js";

const EMPTY_BUILTIN_OVERRIDES = Object.freeze({});

// The v2 `/llm/providers` snapshot is the single source of truth: a unified
// provider list (built-in + operator-defined) already annotated with the active
// selection, `builtin`, and `api_key_set`. Overrides are no longer a separate
// client-side merge — the backend resolves them — so `builtinOverrides` is kept
// as an empty object purely for the shared helper signatures.
export function useLlmProviders() {
  const queryClient = useQueryClient();
  const providersQuery = useQuery({
    queryKey: ["llm-providers"],
    queryFn: fetchLlmProviders,
    staleTime: 60_000,
  });

  const snapshot = providersQuery.data || { providers: [], active: null };
  const builtinOverrides = EMPTY_BUILTIN_OVERRIDES;
  // Map the wire view onto the field names the components/helpers expect.
  const allProviders = (snapshot.providers || []).map((provider) => ({
    ...provider,
    name: provider.description,
    has_api_key: provider.api_key_set === true,
    selection_refs: { ...snapshot.role_selection_refs },
  }));
  const inactiveProviders = (snapshot.inactive_providers || []).map((provider) => ({
    ...provider, name: provider.description, has_api_key: provider.api_key_set === true,
    selection_refs: { ...snapshot.role_selection_refs },
  }));
  // Desired selection is exclusively the canonical DB snapshot. Legacy
  // gateway/settings metadata cannot resurrect an explicitly cleared role.
  const activeProviderId = snapshot.active?.provider_id || "";
  const selectedModel = snapshot.active?.model || "";
  const hasActiveProvider = Boolean(activeProviderId);
  const builtinProviders = allProviders.filter((provider) => provider.builtin);
  const customProviders = allProviders.filter((provider) => !provider.builtin);
  const providers = [...allProviders].sort((a, b) => {
    if (a.id === activeProviderId) return -1;
    if (b.id === activeProviderId) return 1;
    return (a.name || a.id).localeCompare(b.name || b.id);
  });

  const refresh = () => queryClient.invalidateQueries({ queryKey: ["llm-providers"], exact: true });

  const setActiveMutation = useMutation({
    mutationFn: async (provider) => {
      if (!isProviderConfigured(provider, builtinOverrides)) {
        const reason = providerMissingReason(provider, builtinOverrides);
        throw new Error(reason === "base_url" ? "base_url" : "api_key");
      }
      const model = provider.active_model || providerDefaultModel(provider, builtinOverrides);
      if (!model) throw new Error("model");
      await setActiveLlm({ provider_id: provider.id, expected_revision: provider.revision, model,
        expected_selection_ref: modelSelectionRef(provider.selection_refs, "kohai") });
      return provider;
    },
    onSuccess: refresh,
    onError: refresh,
  });

  // All origins edit the same DB definition with its displayed revision.
  const saveProviderMutation = useMutation({
    mutationFn: async ({ provider, form, apiKey, editingProvider, activate = false, selectionRefs }) => {
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
      // Editing a definition must not overwrite an explicit active model.
      // Onboarding may request one atomic definition + role publication.
      if (activate) {
        payload.set_active = true;
        payload.model = payload.default_model;
        payload.expected_selection_ref = modelSelectionRef(selectionRefs, "kohai");
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
    mutationFn: async (provider) => {
      // Deactivate by setting empty provider_id
      await setActiveLlm({ provider_id: "", model: "",
        expected_selection_ref: modelSelectionRef(provider?.selection_refs, "kohai") });
    },
    onSuccess: refresh,
    onError: refresh,
  });

  const setSempaiMutation = useMutation({
    mutationFn: async (provider) => {
      const model = providerDefaultModel(provider, builtinOverrides);
      await setActiveLlm({ provider_id: provider.id, expected_revision: provider.revision, model, role: "sempai",
        expected_selection_ref: modelSelectionRef(provider.selection_refs, "sempai") });
      return provider;
    },
    onSuccess: refresh,
    onError: refresh,
  });

  const setEmbeddingMutation = useMutation({
    mutationFn: async (provider) => {
      const model = providerDefaultModel(provider, builtinOverrides);
      await setActiveLlm({ provider_id: provider.id, expected_revision: provider.revision, model, role: "embedding",
        expected_selection_ref: modelSelectionRef(provider.selection_refs, "embedding") });
      return provider;
    },
    onSuccess: refresh,
    onError: refresh,
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
    runtimePublications: snapshot.runtime_publications || {},
    providerPublications: providerPublicationRows(snapshot),
    roleSelectionRefs: snapshot.role_selection_refs,
    embeddingProviderId,
    isLoading: providersQuery.isLoading,
    error: providersQuery.error,
    setActiveProvider: (provider) => setActiveMutation.mutateAsync(provider),
    deactivateProvider: (provider) => deactivateMutation.mutateAsync(provider),
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
