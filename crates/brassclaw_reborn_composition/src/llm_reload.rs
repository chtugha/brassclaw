use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_reborn_config::{RebornBootConfig, RebornConfigFile};

use crate::LlmKeyStore;
use crate::llm_catalog::{apply_stored_api_key, resolve_reborn_runtime_llm};
use crate::llm_config_service::LlmReloadTrigger;

/// Live-reload adapter wired by the runtime. Re-resolves the LLM config from
/// the DB (`brassclaw_config`) when a pool is present, falling back to
/// `config.toml` + `providers.json` for non-postgres builds. After resolution
/// the running provider's inner backend is hot-swapped via the
/// `brassclaw_llm` reload handle.
type ProviderChangedCallback = Arc<dyn Fn(&str) + Send + Sync>;

pub(crate) struct RebornLlmReloadAdapter {
    boot: RebornBootConfig,
    reload_handle: Arc<brassclaw_llm::LlmReloadHandle>,
    session: Arc<brassclaw_llm::SessionManager>,
    keys: LlmKeyStore,
    /// Optional callback invoked after a successful provider reload.
    /// Used to refresh live token-budget slots with the new provider's settings.
    on_provider_changed: Option<ProviderChangedCallback>,
    /// Postgres pool for reading the LLM config from `brassclaw_config`
    /// instead of `config.toml`. When absent the file-based path is used
    /// (non-postgres builds and local-dev without embedded PG).
    #[cfg(feature = "postgres")]
    pg_pool: Option<Arc<brassclaw_pg::PgPool>>,
    /// Tenant ID for DB config reads. Defaults to `"default"`.
    #[cfg(feature = "postgres")]
    tenant_id: String,
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
            #[cfg(feature = "postgres")]
            pg_pool: None,
            #[cfg(feature = "postgres")]
            tenant_id: "default".to_string(),
        }
    }

    /// Attach a Postgres pool so the reload adapter reads config from DB
    /// rather than from `config.toml`.
    #[cfg(feature = "postgres")]
    pub(crate) fn with_pg_pool(
        mut self,
        pool: Arc<brassclaw_pg::PgPool>,
        tenant_id: impl Into<String>,
    ) -> Self {
        self.pg_pool = Some(pool);
        self.tenant_id = tenant_id.into();
        self
    }
}

#[async_trait]
impl LlmReloadTrigger for RebornLlmReloadAdapter {
    async fn reload(&self) -> Result<(), String> {
        // Prefer DB-backed config snapshot (PG-2 plan requirement) when a pool
        // is available; fall back to the on-disk config.toml for non-postgres
        // builds or when the pool has not been wired yet.
        let config_file: Option<RebornConfigFile>;

        #[cfg(feature = "postgres")]
        {
            if let Some(pool) = self.pg_pool.as_ref() {
                let snapshot =
                    crate::db_config::load_config_snapshot(pool, &self.tenant_id)
                        .await
                        .map_err(|e| e.to_string())?;
                config_file = Some(snapshot);
            } else {
                config_file =
                    RebornConfigFile::load(&self.boot.home().path().join("config.toml"))
                        .map_err(|e| e.to_string())?;
            }
        }
        #[cfg(not(feature = "postgres"))]
        {
            config_file =
                RebornConfigFile::load(&self.boot.home().path().join("config.toml"))
                    .map_err(|e| e.to_string())?;
        }

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
