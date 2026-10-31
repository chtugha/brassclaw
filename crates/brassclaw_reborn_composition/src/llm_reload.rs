use std::sync::Arc;

use async_trait::async_trait;

use crate::LlmKeyStore;
use crate::llm_catalog::resolve_captured_provider_selection;
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
    /// Attach the authoritative source once before ingress. Retain weak runtime
    /// ownership so the provider and its preparation callback cannot form a cycle.
    pub(crate) fn attach_preparation(
        &self,
        sempai: Option<Arc<dyn brassclaw_llm::ProviderSelectionPreparer>>,
    ) -> Result<(), brassclaw_llm::LlmError> {
        self.reload_handle
            .attach_primary_preparation(Arc::new(PgPrimaryPreparation {
                reload_handle: Arc::downgrade(&self.reload_handle),
                session: self.session.clone(),
                keys: self.keys.clone(),
                pg_pool: self.pg_pool.clone(),
                tenant_id: self.tenant_id.clone(),
                sempai,
            }))
    }
}

struct PgPrimaryPreparation {
    reload_handle: std::sync::Weak<brassclaw_llm::LlmReloadHandle>,
    session: Arc<brassclaw_llm::SessionManager>,
    keys: LlmKeyStore,
    pg_pool: Arc<brassclaw_pg::PgPool>,
    tenant_id: String,
    sempai: Option<Arc<dyn brassclaw_llm::ProviderSelectionPreparer>>,
}

#[async_trait]
impl brassclaw_llm::ProviderSelectionPreparer for PgPrimaryPreparation {
    async fn prepare(&self) -> Result<(), brassclaw_llm::LlmError> {
        // Reconcile the optional role/mode before a new primary work item
        // decides whether the already-existing interceptor applies.
        if let Some(sempai) = &self.sempai {
            sempai.prepare().await?;
        }
        let handle = self.reload_handle.upgrade().ok_or_else(|| {
            brassclaw_llm::LlmError::ConfigurationChanged {
                provider: "stored primary provider".into(),
            }
        })?;
        RebornLlmReloadAdapter::new(
            handle,
            self.session.clone(),
            self.keys.clone(),
            self.pg_pool.clone(),
            &self.tenant_id,
        )
        .reload()
        .await
        .map(|_| ())
        .map_err(|_| brassclaw_llm::LlmError::ConfigurationChanged {
            provider: "stored primary provider".into(),
        })
    }
}

#[async_trait]
impl LlmReloadTrigger for RebornLlmReloadAdapter {
    async fn reload(&self) -> Result<brassclaw_llm::ProviderPublication, String> {
        self.reload_handle
            .publish_primary_from(|| async {
                let captured =
                    crate::RebornProviderAdmin::new(self.pg_pool.as_ref().clone(), &self.tenant_id)
                        .capture_selection(
                            crate::provider_admin::ProviderSelectionSource::Role(
                                brassclaw_llm::ProviderRole::Kohai,
                            ),
                            &self.keys,
                        )
                        .await
                        .map_err(|_| brassclaw_llm::LlmError::ConfigurationChanged {
                            provider: "stored primary provider".into(),
                        })?;
                let Some(captured) = captured else {
                    let provider: Arc<dyn brassclaw_llm::LlmProvider> =
                        Arc::new(crate::runtime::PlaceholderLlmProvider);
                    return Ok((provider, brassclaw_llm::ProviderPublication::Unconfigured));
                };
                let desired = brassclaw_llm::ProviderPublication::Configured {
                    selection: captured.revision.clone(),
                };
                let selected = resolve_captured_provider_selection(
                    &self.pg_pool,
                    &self.tenant_id,
                    &self.keys,
                    captured,
                )
                .map_err(|_| {
                    brassclaw_llm::ProviderPreparationFailure::selected(
                        brassclaw_llm::LlmError::ConfigurationChanged {
                            provider: "stored primary provider".into(),
                        },
                        desired,
                    )
                })?;
                let publication = brassclaw_llm::ProviderPublication::Configured {
                    selection: selected.revision,
                };
                // Reconciliation of an unchanged selection preserves its cache,
                // circuit and admitted backend instead of rebuilding it.
                let (provider, current) = self.reload_handle.published_primary_provider();
                if current == publication {
                    return Ok((provider, publication));
                }
                let provider = brassclaw_llm::build_static_provider_chain_with_guard(
                    &selected.config,
                    Arc::clone(&self.session),
                    selected.guard,
                )
                .await
                .map_err(|error| {
                    brassclaw_llm::ProviderPreparationFailure::selected(error, publication.clone())
                })?;
                Ok((provider, publication))
            })
            .await
            .map_err(|_| "stored primary provider could not be captured and published".to_owned())
    }

    fn publication(&self) -> brassclaw_llm::ProviderPublication {
        self.reload_handle.primary_publication()
    }

    fn publication_status(&self) -> brassclaw_llm::ProviderPublicationStatus {
        self.reload_handle.primary_publication_status()
    }
}
