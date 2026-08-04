use std::sync::Arc;

use async_trait::async_trait;

use crate::LlmKeyStore;
use crate::llm_catalog::{apply_stored_api_key, resolve_llm_selection_against_catalog_db};
use crate::llm_config_service::LlmReloadTrigger;

/// Live-reload adapter wired by the runtime. Re-resolves the LLM config from
/// the DB (`brassclaw_config` + `brassclaw_llm_providers`) and atomically
/// hot-swaps the running provider's inner backend via the `brassclaw_llm`
/// reload handle.
pub(crate) struct RebornLlmReloadAdapter {
    reload_handle: Arc<brassclaw_llm::LlmReloadHandle>,
    session: Arc<brassclaw_llm::SessionManager>,
    keys: LlmKeyStore,
    pg_pool: Arc<brassclaw_pg::PgPool>,
    tenant_id: String,
}

impl RebornLlmReloadAdapter {
    pub(crate) fn new(
        reload_handle: Arc<brassclaw_llm::LlmReloadHandle>,
        session: Arc<brassclaw_llm::SessionManager>,
        keys: LlmKeyStore,
        pg_pool: Arc<brassclaw_pg::PgPool>,
        tenant_id: impl Into<String>,
    ) -> Self {
        Self {
            reload_handle,
            session,
            keys,
            pg_pool,
            tenant_id: tenant_id.into(),
        }
    }
}

#[async_trait]
impl LlmReloadTrigger for RebornLlmReloadAdapter {
    async fn reload(&self) -> Result<(), String> {
        let Some(resolved) =
            resolve_llm_selection_against_catalog_db(&self.pg_pool, &self.tenant_id)
                .await
                .map_err(|e| e.to_string())?
        else {
            // No provider selected yet, so there is nothing to swap.
            return Ok(());
        };
        let provider_id = resolved.active_provider_id();
        let mut config = resolved;
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
}
