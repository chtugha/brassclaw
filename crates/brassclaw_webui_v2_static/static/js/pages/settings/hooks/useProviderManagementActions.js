import { React } from "../../../lib/html.js";
import { useLlmProviders } from "./useLlmProviders.js";

function matchesProvider(provider, query) {
  if (!query) return true;
  const q = query.toLowerCase();
  return [provider.id, provider.name, provider.adapter, provider.base_url, provider.default_model]
    .filter(Boolean)
    .some((value) => String(value).toLowerCase().includes(q));
}

export function useProviderManagementActions({ settings, gatewayStatus, searchQuery, t }) {
  const providerState = useLlmProviders({ settings, gatewayStatus });
  const [dialogProvider, setDialogProvider] = React.useState(null);
  const [dialogSelectionRefs, setDialogSelectionRefs] = React.useState(null);
  const [dialogDefaults, setDialogDefaults] = React.useState(null);
  const [isDialogOpen, setIsDialogOpen] = React.useState(false);
  const [message, setMessage] = React.useState(null);
  const messageTimerRef = React.useRef(null);

  const showMessage = React.useCallback((tone, text) => {
    if (messageTimerRef.current) window.clearTimeout(messageTimerRef.current);
    setMessage({ tone, text });
    messageTimerRef.current = window.setTimeout(() => setMessage(null), 3500);
  }, []);

  React.useEffect(
    () => () => {
      if (messageTimerRef.current) window.clearTimeout(messageTimerRef.current);
    },
    []
  );

  const openDialog = React.useCallback((provider = null) => {
    setDialogProvider(provider);
    setDialogSelectionRefs({ ...(provider?.selection_refs || providerState.roleSelectionRefs) });
    setDialogDefaults({
      responseCacheDefaults: providerState.responseCacheDefaults,
      modelDiscoveryDefaults: providerState.modelDiscoveryDefaults,
      requestPolicyDefaults: providerState.requestPolicyDefaults,
    });
    setIsDialogOpen(true);
  }, [providerState.roleSelectionRefs, providerState.responseCacheDefaults, providerState.modelDiscoveryDefaults, providerState.requestPolicyDefaults]);

  const handleUse = React.useCallback(
    async (provider) => {
      try {
        await providerState.setActiveProvider(provider);
        showMessage("success", t("llm.providerActivated", { name: provider.name || provider.id }));
        return true;
      } catch (err) {
        if (err.message === "base_url" || err.message === "api_key" || err.message === "model") {
          openDialog(provider);
          showMessage(
            "error",
            t(err.message === "base_url" ? "llm.baseUrlRequired" : err.message === "model" ? "llm.modelRequired" : "llm.configureToUse")
          );
        } else {
          showMessage("error", err.status === 409 ? t("llm.providerChanged") : err.message);
        }
        return false;
      }
    },
    [openDialog, providerState, showMessage, t]
  );

  const handleUseSempai = React.useCallback(
    async (provider) => {
      try {
        await providerState.setSempaiProvider(provider);
        showMessage("success", t("llm.sempaiActivated", { name: provider.name || provider.id }));
      } catch (err) {
        showMessage("error", err.status === 409 ? t("llm.providerChanged") : err.message);
      }
    },
    [providerState, showMessage, t]
  );

  const handleUseEmbedding = React.useCallback(
    async (provider) => {
      try {
        await providerState.setEmbeddingProvider(provider);
        showMessage("success", t("llm.embeddingActivated", { name: provider.name || provider.id }));
      } catch (err) {
        showMessage("error", err.status === 409 ? t("llm.providerChanged") : err.message);
      }
    },
    [providerState, showMessage, t]
  );

  const handleSave = React.useCallback(
    async ({ form, apiKey, provider, activate = false }) => {
      if (provider?.builtin) {
        const saved = await providerState.saveBuiltinProvider({ provider, form, apiKey, activate, selectionRefs: dialogSelectionRefs });
        showMessage("success", t("llm.providerConfigured", { name: provider.name || provider.id }));
        return saved;
      }
      const saved = await providerState.saveCustomProvider({ form, apiKey, editingProvider: provider, activate, selectionRefs: dialogSelectionRefs });
      showMessage("success", t(provider ? "llm.providerUpdated" : "llm.providerAdded", { name: saved.name || saved.id }));
      return saved;
    },
    [providerState, dialogSelectionRefs, showMessage, t]
  );

  const handleDeactivate = React.useCallback(
    async (provider) => {
      if (!window.confirm(t("llm.confirmDeactivate", { name: provider.name || provider.id }))) return;
      try {
        await providerState.deactivateProvider(provider);
        showMessage("success", t("llm.providerDeactivated"));
      } catch (err) {
        showMessage("error", err.status === 409 ? t("llm.providerChanged") : err.message);
      }
    },
    [providerState, showMessage, t]
  );

  const handleDelete = React.useCallback(
    async (provider) => {
      if (!window.confirm(t("llm.confirmDelete", { id: provider.id }))) return;
      try {
        await providerState.deleteCustomProvider(provider);
        showMessage("success", t("llm.providerDeleted"));
      } catch (err) {
        showMessage("error", err.status === 409 ? t("llm.providerChanged") : err.message);
      }
    },
    [providerState, showMessage, t]
  );

  return {
    providerState,
    dialogProvider,
    dialogDefaults,
    isDialogOpen,
    message,
    filteredProviders: providerState.providers.filter((provider) =>
      matchesProvider(provider, searchQuery)
    ),
    allProviderIds: providerState.providers.map((provider) => provider.id),
    openDialog,
    closeDialog: () => setIsDialogOpen(false),
    handleUse,
    handleUseSempai,
    handleUseEmbedding,
    handleSave,
    handleDeactivate,
    handleDelete,
  };
}
