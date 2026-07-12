use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_reborn_config::{RebornBootConfig, RebornConfigFile};

use crate::LlmKeyStore;
use crate::llm_catalog::{apply_stored_api_key, resolve_reborn_runtime_llm};
use crate::llm_config_service::LlmReloadTrigger;

/// Live-reload adapter wired by the runtime. Re-resolves the LLM config from
/// `config.toml` + `providers.json` + the stored key, then hot-swaps the
/// running provider's inner backend via the `brassclaw_llm` reload handle.
pub(crate) struct RebornLlmReloadAdapter {
    boot: RebornBootConfig,
    reload_handle: Arc<brassclaw_llm::LlmReloadHandle>,
    session: Arc<brassclaw_llm::SessionManager>,
    keys: LlmKeyStore,
    /// Optional callback invoked after a successful provider reload.
    /// Used to refresh live token-budget slots with the new provider's settings.
    on_provider_changed: Option<Arc<dyn Fn(&str) + Send + Sync>>,
}

impl RebornLlmReloadAdapter {
    pub(crate) fn new(
        boot: RebornBootConfig,
        reload_handle: Arc<brassclaw_llm::LlmReloadHandle>,
        session: Arc<brassclaw_llm::SessionManager>,
        keys: LlmKeyStore,
    ) -> Self {
        Self {
            boot,
            reload_handle,
            session,
            keys,
            on_provider_changed: None,
        }
    }

    /// Attach a callback that fires after a successful provider reload.
    /// The callback receives the new active provider ID and can update
    /// live budget slots, context-window overrides, etc.
    pub(crate) fn with_on_provider_changed(
        mut self,
        f: Arc<dyn Fn(&str) + Send + Sync>,
    ) -> Self {
        self.on_provider_changed = Some(f);
        self
    }
}

#[async_trait]
impl LlmReloadTrigger for RebornLlmReloadAdapter {
    async fn reload(&self) -> Result<(), String> {
        let config_file = RebornConfigFile::load(&self.boot.home().config_file_path())
            .map_err(|error| error.to_string())?;
        let Some(resolved) = resolve_reborn_runtime_llm(&self.boot, config_file.as_ref())
            .map_err(|error| error.to_string())?
        else {
            // No provider selected yet, so there is nothing to swap.
            return Ok(());
        };
        let provider_id = resolved.provider_id().to_string();
        let mut config = resolved.config;
        if let Some(stored) = self
            .keys
            .read(&provider_id)
            .await
            .map_err(|error| error.to_string())?
        {
            apply_stored_api_key(&mut config, stored);
        }
        self.reload_handle
            .reload(&config, Arc::clone(&self.session))
            .await
            .map_err(|error| error.to_string())
    }

    async fn on_provider_changed(&self, new_provider_id: &str) {
        if let Some(f) = &self.on_provider_changed {
            f(new_provider_id);
        }
    }
}
