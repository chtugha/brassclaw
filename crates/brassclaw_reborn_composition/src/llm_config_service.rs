//! Composition-side implementation of the WebChat v2 LLM-config port.
//!
//! As of V047/V048, all providers (builtin and custom) live exclusively in
//! `brassclaw_llm_providers` (Postgres).  The service reads and writes only the
//! DB; the file-based `ProviderRepo` fallback has been removed from the wired
//! production path.
//!
//! Persistence model:
//! - All provider definitions      → `brassclaw_llm_providers` (DB)
//! - Active provider + model       → `brassclaw_config` keys `llm.default.*`
//! - API-key **values**            → encrypted scoped secret store, never catalogue/config plaintext

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use brassclaw_llm::ProviderRole;
use brassclaw_llm::registry::{ProviderDefinition, ProviderProtocol, ProviderRegistry};
use brassclaw_product_workflow::{
    CodexLoginStart, LlmActiveSelection, LlmConfigService, LlmConfigServiceError,
    LlmConfigSnapshot, LlmModelsResult, LlmProbeRequest, LlmProbeResult, LlmProviderView,
    NearAiLoginRequest, NearAiLoginStart, NearAiWalletLoginRequest, NearAiWalletLoginResult,
    NearAiWalletLoginStart, NearAiWalletLoginStartRequest, ProviderTokenBudgetView,
    SetActiveLlmRequest, UpsertLlmProviderRequest, WebUiAuthenticatedCaller,
};
use brassclaw_reborn_config::LlmSlotSelection;
use secrecy::{ExposeSecret as _, SecretString};

use crate::LlmKeyStore;
use crate::llm_catalog::resolve_against_registry_with_credentials;

const NEARAI_LOGIN_STATE_TTL: Duration = Duration::from_secs(15 * 60);
#[path = "provider_login.rs"]
mod provider_login;
use provider_login::NearAiLoginContext;

/// One-use login state retains the exact original provider/auth/role context.
/// A completed commit is independently recoverable through its mutation receipt.
#[derive(Default)]
pub(crate) struct NearAiLoginStateStore {
    states: tokio::sync::Mutex<HashMap<String, (Instant, NearAiLoginContext)>>,
}

impl NearAiLoginStateStore {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    async fn issue(&self, context: NearAiLoginContext) {
        self.issue_for(context, NEARAI_LOGIN_STATE_TTL).await;
    }

    async fn issue_for(&self, context: NearAiLoginContext, lifetime: Duration) {
        let mut states = self.states.lock().await;
        let now = Instant::now();
        states.retain(|_, (expires, _)| *expires > now);
        states.insert(context.id.to_string(), (now + lifetime, context));
    }

    async fn consume_matching(
        &self,
        state: &str,
        matches: impl FnOnce(&NearAiLoginContext) -> bool,
    ) -> Option<NearAiLoginContext> {
        let mut states = self.states.lock().await;
        let now = Instant::now();
        states.retain(|_, (expires, _)| *expires > now);
        if !states
            .get(state)
            .is_some_and(|(_, context)| matches(context))
        {
            return None;
        }
        states.remove(state).map(|(_, context)| context)
    }
}

/// Live-reload seam. The runtime supplies an impl that re-resolves the LLM
/// config (including any stored key) and atomically swaps the running
/// provider's inner backend; tests / unwired runtimes leave it absent.
#[async_trait]
pub trait LlmReloadTrigger: Send + Sync {
    /// Re-resolve and hot-swap the active provider. The error string is for
    /// logging only and must stay free of secrets / backend internals.
    async fn reload(&self) -> Result<brassclaw_llm::ProviderPublication, String>;

    /// Actual published metadata, never a later independent database read.
    fn publication(&self) -> brassclaw_llm::ProviderPublication;

    /// Same-owner effective/failure snapshot. Custom legacy adapters without a
    /// captured failed target leave that target unknown.
    fn publication_status(&self) -> brassclaw_llm::ProviderPublicationStatus {
        brassclaw_llm::ProviderPublicationStatus {
            effective: self.publication(),
            failed: None,
        }
    }
}

struct CapturedProbe {
    protocol: ProviderProtocol,
    endpoint: String,
    key: Option<secrecy::SecretString>,
    model_discovery: brassclaw_llm::model_discovery::ModelDiscoveryPolicy,
}

/// Operator-wide LLM configuration service backing the webui2 settings surface.
/// Requires a Postgres pool and provider repo; there is no file-based fallback.
pub struct RebornLlmConfigService {
    publication: Arc<tokio::sync::Mutex<()>>,
    #[cfg(feature = "skills-db")]
    bootstrap_runtime: std::sync::OnceLock<std::sync::Weak<crate::RebornRuntime>>,
    keys: LlmKeyStore,
    reload: Option<Arc<dyn LlmReloadTrigger>>,
    memory_embedding_source: Option<Arc<dyn brassclaw_memory::EmbeddingProviderSource>>,
    embedding_publication: Option<Arc<crate::memory_embedding_source::PgMemoryEmbeddingSource>>,
    /// The runtime's NEAR AI session manager — the same instance the live
    /// provider reads its token from, so a completed login takes effect on
    /// reload. Absent when the runtime has no LLM seam wired.
    nearai_session: Option<Arc<brassclaw_llm::SessionManager>>,
    nearai_login_states: Arc<NearAiLoginStateStore>,
    /// Postgres pool for writing/reading role assignments to `brassclaw_config`.
    pg_pool: Arc<brassclaw_pg::PgPool>,
    /// DB-backed provider repo (exclusive source of truth for all providers).
    pg_provider_repo: Arc<crate::pg_provider_repo::PgProviderRepo>,
    /// Tenant ID for DB operations.
    db_tenant_id: String,
    /// Live hot-swap wrapper for the Sempai provider.
    #[cfg(feature = "root-llm-provider")]
    sempai_swappable: Option<Arc<brassclaw_llm::SwappableLlmProvider>>,
    /// Shared interceptor mode flag.
    #[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
    interceptor_mode: Option<brassclaw_interceptor::SharedInterceptorMode>,
}

struct PgSempaiPreparation {
    provider: std::sync::Weak<brassclaw_llm::SwappableLlmProvider>,
    configuration: RebornLlmConfigService,
}

#[async_trait]
impl brassclaw_llm::ProviderSelectionPreparer for PgSempaiPreparation {
    async fn prepare(&self) -> Result<(), brassclaw_llm::LlmError> {
        let provider = self.provider.upgrade().ok_or_else(|| {
            brassclaw_llm::LlmError::ConfigurationChanged {
                provider: "stored Sempai provider".into(),
            }
        })?;
        self.configuration
            .publish_sempai_on(&provider)
            .await
            .map_err(|_| brassclaw_llm::LlmError::ConfigurationChanged {
                provider: "stored Sempai provider".into(),
            })
    }
}

impl RebornLlmConfigService {
    async fn lookup_provider_mutation(
        &self,
        mutation: &crate::provider_mutation::ProviderMutation,
    ) -> Result<Option<brassclaw_product_workflow::ProviderMutationReceipt>, LlmConfigServiceError>
    {
        let mut client = self
            .pg_pool
            .get()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let transaction = client
            .transaction()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        mutation.lock(&transaction, &self.db_tenant_id).await?;
        let original = mutation
            .original(&transaction, &self.db_tenant_id, &self.keys)
            .await?;
        transaction
            .commit()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        Ok(original)
    }

    #[cfg(feature = "skills-db")]
    pub(crate) fn attach_prefix_bootstrap(
        &self,
        runtime: std::sync::Weak<crate::RebornRuntime>,
    ) -> Result<(), LlmConfigServiceError> {
        if let Some(original) = self.bootstrap_runtime.get() {
            if !original.ptr_eq(&runtime) {
                return Err(LlmConfigServiceError::Unavailable);
            }
            return Ok(());
        }
        match self.bootstrap_runtime.set(runtime) {
            Ok(()) => Ok(()),
            Err(runtime)
                if self
                    .bootstrap_runtime
                    .get()
                    .is_some_and(|original| original.ptr_eq(&runtime)) =>
            {
                Ok(())
            }
            Err(_) => Err(LlmConfigServiceError::Unavailable),
        }
    }
    #[cfg(feature = "skills-db")]
    fn bootstrap_runtime(
        &self,
    ) -> Result<Option<Arc<crate::RebornRuntime>>, LlmConfigServiceError> {
        self.bootstrap_runtime
            .get()
            .map(|weak| weak.upgrade().ok_or(LlmConfigServiceError::Unavailable))
            .transpose()
    }
    #[cfg(feature = "skills-db")]
    async fn finish_bootstrap(
        &self,
        receipt: &brassclaw_product_workflow::ProviderMutationReceipt,
    ) -> Result<(), LlmConfigServiceError> {
        if let Some(id) = receipt.bootstrap_job_id {
            let runtime = self
                .bootstrap_runtime()?
                .ok_or(LlmConfigServiceError::Unavailable)?;
            runtime.admit_provider_bootstrap(id).await?;
        }
        Ok(())
    }
    async fn finish_provider_mutation(
        &self,
        receipt: brassclaw_product_workflow::ProviderMutationReceipt,
    ) -> Result<LlmConfigSnapshot, LlmConfigServiceError> {
        // After commit only. A lost admission acknowledgement/reconnect uses
        // the original stored job; publication errors cannot skip this handoff.
        #[cfg(feature = "skills-db")]
        let bootstrap = self.finish_bootstrap(&receipt).await;
        // Reconcile current desired configuration; never republish the old
        // request's selection over a later writer. Its receipt stays original.
        match receipt.selected_role {
            Some(ProviderRole::Kohai) => self.refresh_running_provider().await?,
            Some(ProviderRole::Sempai) => self.restore_sempai_from_db_locked().await?,
            Some(ProviderRole::Embedding) => self.refresh_memory_embedding().await?,
            None => {
                // Definition/authentication edits can affect several roles.
                // Attempt every existing owner even if another role fails;
                // original mutation custody remains independently readable.
                let sempai = if self.sempai_swappable.is_some() {
                    self.restore_sempai_from_db_locked().await
                } else {
                    Ok(())
                };
                let primary = self.refresh_running_provider().await;
                let embedding = self.refresh_memory_embedding().await;
                sempai?;
                primary?;
                embedding?;
            }
        }
        #[cfg(feature = "skills-db")]
        bootstrap?;
        let mut snapshot = self.build_snapshot().await?;
        snapshot.mutation_receipt = Some(receipt);
        Ok(snapshot)
    }

    pub fn new(
        keys: LlmKeyStore,
        pg_pool: Arc<brassclaw_pg::PgPool>,
        pg_provider_repo: Arc<crate::pg_provider_repo::PgProviderRepo>,
        db_tenant_id: impl Into<String>,
    ) -> Self {
        Self {
            publication: Arc::new(tokio::sync::Mutex::new(())),
            #[cfg(feature = "skills-db")]
            bootstrap_runtime: std::sync::OnceLock::new(),
            keys,
            reload: None,
            memory_embedding_source: None,
            embedding_publication: None,
            nearai_session: None,
            nearai_login_states: Arc::new(NearAiLoginStateStore::new()),
            pg_pool,
            pg_provider_repo,
            db_tenant_id: db_tenant_id.into(),
            #[cfg(feature = "root-llm-provider")]
            sempai_swappable: None,
            #[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
            interceptor_mode: None,
        }
    }

    /// Attach the live-reload trigger (from the runtime).
    pub fn with_reload_trigger(mut self, reload: Arc<dyn LlmReloadTrigger>) -> Self {
        self.reload = Some(reload);
        self
    }

    pub(crate) fn with_memory_embedding_source(
        mut self,
        source: Arc<dyn brassclaw_memory::EmbeddingProviderSource>,
    ) -> Self {
        self.memory_embedding_source = Some(source);
        self
    }

    pub(crate) fn with_embedding_publication(
        mut self,
        source: Arc<crate::memory_embedding_source::PgMemoryEmbeddingSource>,
    ) -> Self {
        self.embedding_publication = Some(source);
        self
    }

    async fn refresh_memory_embedding(&self) -> Result<(), LlmConfigServiceError> {
        if let Some(source) = &self.memory_embedding_source {
            source
                .snapshot()
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable)?;
        }
        Ok(())
    }

    /// Attach the runtime's NEAR AI session manager (enables NEAR AI login).
    pub fn with_nearai_session(mut self, session: Arc<brassclaw_llm::SessionManager>) -> Self {
        self.nearai_session = Some(session);
        self
    }

    /// Attach the runtime's NEAR AI login-state store. The start endpoint and
    /// public callback must share the same store.
    pub(crate) fn with_nearai_login_states(mut self, states: Arc<NearAiLoginStateStore>) -> Self {
        self.nearai_login_states = states;
        self
    }

    /// Attach the Sempai live-swap wrapper from the runtime.
    #[cfg(feature = "root-llm-provider")]
    pub fn with_sempai_swappable(
        mut self,
        swappable: Arc<brassclaw_llm::SwappableLlmProvider>,
    ) -> Self {
        self.sempai_swappable = Some(swappable);
        self
    }

    /// Attach the shared interceptor mode flag from the runtime.
    #[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
    pub fn with_interceptor_mode(
        mut self,
        mode: brassclaw_interceptor::SharedInterceptorMode,
    ) -> Self {
        self.interceptor_mode = Some(mode);
        self
    }

    /// Reload failures are visible to the writer. The acknowledgement identifies
    /// the actual published client; model-profile/budget coordination has its
    /// separate owning contract and is not inferred from a no-op callback.
    async fn refresh_running_provider(&self) -> Result<(), LlmConfigServiceError> {
        let reload = self
            .reload
            .as_ref()
            .ok_or(LlmConfigServiceError::Unavailable)?;
        reload.reload().await.map_err(|reason| {
            tracing::warn!(reason = %reason, "live LLM provider reload failed");
            LlmConfigServiceError::Unavailable
        })?;
        Ok(())
    }

    /// Prepare a role from the actual DB/credential snapshot before changing
    /// its selection. Reuse the normal factories; this performs no inference.
    #[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
    async fn prepare_provider_selection(
        &self,
        provider_id: &str,
        model: Option<&str>,
        role: ProviderRole,
    ) -> Result<Option<crate::llm_catalog::PreparedProviderSelection>, LlmConfigServiceError> {
        let captured =
            crate::RebornProviderAdmin::new(self.pg_pool.as_ref().clone(), &self.db_tenant_id)
                .capture_selection(
                    crate::provider_admin::ProviderSelectionSource::Explicit(LlmSlotSelection {
                        provider_id: Some(provider_id.into()),
                        model: model.map(str::to_owned),
                        api_key_env: None,
                        base_url: None,
                    }),
                    &self.keys,
                )
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable)?
                .ok_or(LlmConfigServiceError::NotFound)?;
        if role == ProviderRole::Embedding {
            crate::factory::build_embeddings_config_from_provider(
                &captured.definition,
                captured.selection.model,
                captured.credential,
            )
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
            return Ok(None);
        }
        let (provider, mut prepared) = self
            .prepare_retained_provider(captured)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        drop(provider);
        // Explicit Use will persist this actual resolved model. Runtime restore
        // below retains the original stored slot/configuration-ref semantics.
        let crate::llm_catalog::PreparedProviderOrigin::Retained {
            definition,
            selection,
            revision,
            credential_ref,
        } = &mut prepared.origin
        else {
            return Err(LlmConfigServiceError::Unavailable);
        };
        selection.model = Some(prepared.config.active_model_name());
        *revision = crate::provider_admin::selection_revision(
            &self.db_tenant_id,
            selection,
            definition,
            revision.definition_revision,
            credential_ref.as_deref(),
        )
        .map_err(|_| LlmConfigServiceError::Unavailable)?;
        Ok(Some(prepared))
    }

    async fn prepare_retained_provider(
        &self,
        captured: crate::provider_admin::CapturedProviderSelection,
    ) -> Result<
        (
            Arc<dyn brassclaw_llm::LlmProvider>,
            crate::llm_catalog::PreparedProviderSelection,
        ),
        String,
    > {
        let guard = crate::provider_dispatch::PgProviderDispatchGuard::from_capture(
            self.pg_pool.as_ref().clone(),
            &self.db_tenant_id,
            self.keys.clone(),
            &captured,
        )
        .map_err(|_| "stored Sempai compatibility could not be captured".to_string())?;
        let registry = ProviderRegistry::new(vec![captured.definition.clone()]);
        let config = resolve_against_registry_with_credentials(
            &captured.selection,
            &registry,
            captured.credential,
        )
        .map_err(|_| "stored Sempai configuration is invalid".to_string())?;
        let mut revision = captured.revision;
        revision.model = config.active_model_name();
        let session = brassclaw_llm::create_session_manager(config.session.clone()).await;
        let provider =
            brassclaw_llm::build_static_provider_chain_with_guard(&config, session, guard)
                .await
                .map_err(|_| "stored Sempai provider could not be constructed".to_string())?;
        Ok((
            provider,
            crate::llm_catalog::PreparedProviderSelection {
                config,
                origin: crate::llm_catalog::PreparedProviderOrigin::Retained {
                    definition: captured.definition,
                    selection: captured.selection,
                    revision,
                    credential_ref: captured.credential_ref,
                },
            },
        ))
    }

    async fn build_sempai_selection(
        &self,
        captured: crate::provider_admin::CapturedProviderSelection,
    ) -> Result<
        (
            Arc<dyn brassclaw_llm::LlmProvider>,
            brassclaw_llm::ProviderSelectionRevision,
        ),
        String,
    > {
        let (provider, prepared) = self.prepare_retained_provider(captured).await?;
        let crate::llm_catalog::PreparedProviderOrigin::Retained { revision, .. } = prepared.origin
        else {
            return Err("stored provider preparation origin differs".into());
        };
        Ok((provider, revision))
    }

    /// Restore the stored role before workers, review delivery or ingress start.
    /// This reads one consistent provider/selection snapshot, never writes the
    /// selection or seeds providers, and publishes only a fully built backend.
    pub(crate) async fn restore_sempai_from_db(&self) -> Result<(), LlmConfigServiceError> {
        let _publication = self.publication.lock().await;
        self.restore_sempai_from_db_locked().await
    }

    async fn restore_sempai_from_db_locked(&self) -> Result<(), LlmConfigServiceError> {
        let swappable = self
            .sempai_swappable
            .as_ref()
            .ok_or(LlmConfigServiceError::Unavailable)?;
        self.publish_sempai_on(swappable).await
    }

    /// Transfer the startup owner to a weak preparation callback. Other role
    /// handles are not retained by the callback, avoiding runtime ownership cycles.
    pub(crate) fn into_sempai_preparation(
        mut self,
    ) -> Result<Arc<dyn brassclaw_llm::ProviderSelectionPreparer>, LlmConfigServiceError> {
        let provider = self
            .sempai_swappable
            .take()
            .ok_or(LlmConfigServiceError::Unavailable)?;
        self.reload = None;
        self.memory_embedding_source = None;
        self.embedding_publication = None;
        self.nearai_session = None;
        let source: Arc<dyn brassclaw_llm::ProviderSelectionPreparer> =
            Arc::new(PgSempaiPreparation {
                provider: Arc::downgrade(&provider),
                configuration: self,
            });
        provider
            .attach_preparation(source.clone())
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        Ok(source)
    }

    async fn publish_sempai_on(
        &self,
        swappable: &brassclaw_llm::SwappableLlmProvider,
    ) -> Result<(), LlmConfigServiceError> {
        swappable.publish_from(|| async {
            let captured = crate::RebornProviderAdmin::new(self.pg_pool.as_ref().clone(), &self.db_tenant_id)
                .capture_selection(crate::provider_admin::ProviderSelectionSource::Role(ProviderRole::Sempai), &self.keys)
                .await.map_err(|_| brassclaw_llm::LlmError::ConfigurationChanged {
                    provider: "stored Sempai provider".into(),
                })?;
            let Some(captured) = captured else {
                let provider: Arc<dyn brassclaw_llm::LlmProvider> = Arc::new(crate::runtime::PlaceholderLlmProvider);
                return Ok((provider, brassclaw_llm::ProviderPublication::Unconfigured));
            };
            // The resolved model is checked again by the concrete publication.
                    let (provider, current) = swappable.published_provider();
            if matches!(&current, brassclaw_llm::ProviderPublication::Configured { selection }
                if selection.configuration_ref == captured.revision.configuration_ref)
            {
                        return Ok((provider, current));
            }
            let desired = brassclaw_llm::ProviderPublication::Configured { selection: captured.revision.clone() };
            let (provider, selection) = self.build_sempai_selection(captured).await
                .map_err(|_| brassclaw_llm::ProviderPreparationFailure::selected(
                    brassclaw_llm::LlmError::ConfigurationChanged { provider: "stored Sempai provider".into() }, desired))?;
            Ok((provider, brassclaw_llm::ProviderPublication::Configured { selection }))
        }, |publication| {
            // Keep mode and this role publication serialized across every facade.
            if let Some(mode) = &self.interceptor_mode {
                if matches!(publication, brassclaw_llm::ProviderPublication::Configured { .. }) {
                    mode.set_rerouting();
                } else {
                    mode.set_routing();
                }
            }
        }).await.map_err(|_| LlmConfigServiceError::Unavailable)?;
        Ok(())
    }

    async fn build_snapshot(&self) -> Result<LlmConfigSnapshot, LlmConfigServiceError> {
        use crate::provider_admin::provider_protocol_wire_name;

        let mut client = self
            .pg_pool
            .get()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let transaction = tokio_postgres::Client::build_transaction(&mut client)
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let all_defs = self
            .pg_provider_repo
            .load_revisions_from_client(&transaction)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let config =
            crate::db_config::load_config_snapshot_from_client(&transaction, &self.db_tenant_id)
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let role_selection_refs =
            crate::db_config::model_selection_refs(&transaction, &self.db_tenant_id)
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let registry = brassclaw_llm::ProviderRegistry::new(
            all_defs
                .iter()
                .filter(|row| !row.inactive)
                .map(|row| row.definition.clone())
                .collect(),
        );
        crate::llm_catalog::validate_catalog(&registry)
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        // Authenticate reference metadata in this same READ ONLY snapshot.
        // GET neither migrates legacy rows nor issues/consumes secret leases.
        let key_checks = all_defs.iter().map(|row| {
            self.keys
                .inspect_record_reference_in_transaction(&transaction, &row.definition.id)
        });
        let key_refs = futures::future::join_all(key_checks)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let mut desired_publications = std::collections::BTreeMap::new();
        let mut selections = std::collections::BTreeMap::new();
        for (name, role) in [
            ("kohai", ProviderRole::Kohai),
            ("sempai", ProviderRole::Sempai),
            ("embedding", ProviderRole::Embedding),
        ] {
            let selected = crate::provider_admin::stored_role_selection(&config, role);
            let selected = selected
                .as_ref()
                .filter(|slot| slot.provider_id.as_deref().is_some_and(|id| !id.is_empty()));
            let (view, desired) = match selected {
                None => (None, brassclaw_llm::ProviderPublication::Unconfigured),
                Some(slot) => {
                    let id = slot.provider_id.as_deref().unwrap_or_default();
                    // Preserve an invalid external selection for operator repair.
                    // It is not unconfiguration or authority to use the old client.
                    let definition = registry.find(id);
                    let view = Some(LlmActiveSelection {
                        provider_id: definition
                            .map_or(id, |definition| definition.id.as_str())
                            .to_owned(),
                        model: slot.model.clone().filter(|model| !model.is_empty()),
                    });
                    let desired = match definition {
                        None => brassclaw_llm::ProviderPublication::Untracked,
                        Some(definition) => {
                            let index = all_defs
                                .iter()
                                .position(|row| row.definition.id == definition.id)
                                .ok_or(LlmConfigServiceError::Unavailable)?;
                            let revision = crate::provider_admin::selection_revision(
                                &self.db_tenant_id,
                                slot,
                                definition,
                                all_defs[index].revision,
                                key_refs[index].as_deref(),
                            )
                            .map_err(|_| LlmConfigServiceError::Unavailable)?;
                            brassclaw_llm::ProviderPublication::Configured {
                                selection: revision,
                            }
                        }
                    };
                    (view, desired)
                }
            };
            selections.insert(name, view);
            desired_publications.insert(name.to_owned(), desired);
        }
        let kohai_sel = selections.remove("kohai").flatten();
        let sempai_sel = selections.remove("sempai").flatten();
        let embedding_sel = selections.remove("embedding").flatten();
        transaction
            .commit()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        drop(client);

        let mut providers = Vec::with_capacity(all_defs.len());
        let mut inactive_providers = Vec::new();
        let active = kohai_sel.clone();

        for (row, key_ref) in all_defs.into_iter().zip(key_refs) {
            let inactive = row.inactive;
            let revision = row.revision;
            let is_builtin = row.is_builtin;
            let def = row.definition;
            let api_key_set = key_ref.is_some();

            let is_kohai = kohai_sel.as_ref().is_some_and(|s| s.provider_id == def.id);
            let active_model = is_kohai
                .then(|| kohai_sel.as_ref().and_then(|s| s.model.clone()))
                .flatten();

            let can_list_models = brassclaw_llm::model_discovery::supports_protocol(def.protocol);
            let accepts_api_key = def.api_key_required
                || def.api_key_env.is_some()
                || def
                    .setup
                    .as_ref()
                    .is_some_and(brassclaw_llm::registry::SetupHint::accepts_api_key);
            let adapter = provider_protocol_wire_name(def.protocol);
            let token_budget = def.token_budget.as_ref().map(|b| ProviderTokenBudgetView {
                profile: b.profile.clone(),
                conversation_history: b.conversation_history,
                skills: b.skills,
                identity: b.identity,
                inline_control: b.inline_control,
                memory: b.memory,
                safety: b.safety,
                capability_surface: b.capability_surface,
                total_input: b.total_input,
                max_output: b.max_output,
            });

            let view = LlmProviderView {
                embedding_models: def.embedding_models.clone(),
                model_token_limits: def.model_token_limits.clone(),
                model_discovery: def.model_discovery,
                request_policy: def.request_policy,
                response_cache: def.response_cache,
                revision,
                id: def.id.clone(),
                description: if def.description.is_empty() {
                    def.id.clone()
                } else {
                    def.description.clone()
                },
                adapter,
                default_model: def.default_model.clone(),
                base_url: def.default_base_url.clone(),
                builtin: is_builtin,
                active: is_kohai,
                active_model,
                api_key_required: def.api_key_required,
                accepts_api_key,
                api_key_set,
                can_list_models,
                token_budget,
                context_window_tokens: def.context_window_tokens,
                is_kohai,
                is_sempai: sempai_sel.as_ref().is_some_and(|s| s.provider_id == def.id),
                is_embedding: embedding_sel
                    .as_ref()
                    .is_some_and(|s| s.provider_id == def.id),
            };
            if inactive {
                inactive_providers.push(view);
            } else {
                providers.push(view);
            }
        }

        let unknown = || brassclaw_llm::ProviderPublicationStatus {
            effective: brassclaw_llm::ProviderPublication::Untracked,
            failed: None,
        };
        let statuses = [
            (
                "kohai",
                self.reload
                    .as_ref()
                    .map_or_else(unknown, |reload| reload.publication_status()),
            ),
            (
                "sempai",
                self.sempai_swappable
                    .as_ref()
                    .map_or_else(unknown, |provider| provider.publication_status()),
            ),
            (
                "embedding",
                match &self.embedding_publication {
                    Some(source) => source.publication_status().await,
                    None => unknown(),
                },
            ),
        ];
        let runtime_publications = statuses
            .iter()
            .map(|(role, status)| (role.to_string(), status.effective.clone()))
            .collect();
        let runtime_publication_failures = statuses
            .into_iter()
            .filter_map(|(role, status)| status.failed.map(|failed| (role.to_string(), failed)))
            .collect();
        Ok(LlmConfigSnapshot {
            desired_publications,
            role_selection_refs,
            runtime_publications,
            runtime_publication_failures,
            mutation_receipt: None,
            model_discovery_defaults: Default::default(),
            request_policy_defaults: Default::default(),
            providers,
            inactive_providers,
            response_cache_defaults: Default::default(),
            active: active.clone(),
            kohai_active: active,
            sempai_active: sempai_sel,
            embedding_active: embedding_sel,
        })
    }

    /// Capture endpoint, adapter, policy and credential in one snapshot.
    /// Release the connection before contacting the external endpoint.
    async fn capture_probe(
        &self,
        request: &LlmProbeRequest,
    ) -> Result<CapturedProbe, LlmConfigServiceError> {
        let protocol = parse_adapter(&request.adapter).ok_or_else(|| {
            LlmConfigServiceError::InvalidRequest {
                field: Some("adapter".into()),
                reason: "Unsupported provider adapter.".into(),
            }
        })?;
        let mut client = self
            .pg_pool
            .get()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let transaction = tokio_postgres::Client::build_transaction(&mut client)
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .start()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let definition = self
            .pg_provider_repo
            .get_from_client(&transaction, &request.provider_id)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let endpoint = match request.base_url.as_deref() {
            Some(value) => value.to_owned(),
            None => definition
                .as_ref()
                .and_then(|value| value.default_base_url.clone())
                .unwrap_or_default(),
        };
        let parsed =
            url::Url::parse(&endpoint).map_err(|_| LlmConfigServiceError::InvalidRequest {
                field: Some("base_url".into()),
                reason: "Configure an HTTP(S) endpoint.".into(),
            })?;
        if !matches!(parsed.scheme(), "http" | "https")
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            return Err(LlmConfigServiceError::InvalidRequest {
                field: Some("base_url".into()),
                reason: "Endpoint credentials, query and fragment are not supported.".into(),
            });
        }
        let policy = request.model_discovery.unwrap_or_else(|| {
            definition
                .as_ref()
                .map(|value| value.model_discovery)
                .unwrap_or_default()
        });
        policy
            .validate()
            .map_err(|error| LlmConfigServiceError::InvalidRequest {
                field: Some("model_discovery".into()),
                reason: error.to_string(),
            })?;
        let key = if let Some(key) = request
            .api_key
            .as_ref()
            .filter(|key| !is_masked_sentinel(key))
        {
            Some(key.clone())
        } else if definition.as_ref().is_some_and(|value| {
            value.protocol == protocol
                && normalized_endpoint(Some(&endpoint))
                    == normalized_endpoint(value.default_base_url.as_deref())
        }) {
            self.keys
                .read_in_transaction(&transaction, &request.provider_id)
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable)?
        } else {
            None
        };
        transaction
            .commit()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        drop(client);
        Ok(CapturedProbe {
            protocol,
            endpoint,
            key,
            model_discovery: policy,
        })
    }
}

#[async_trait]
impl LlmConfigService for RebornLlmConfigService {
    async fn snapshot(
        &self,
        _caller: WebUiAuthenticatedCaller,
    ) -> Result<LlmConfigSnapshot, LlmConfigServiceError> {
        self.build_snapshot().await
    }

    async fn upsert_provider(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: UpsertLlmProviderRequest,
    ) -> Result<LlmConfigSnapshot, LlmConfigServiceError> {
        let _publication = self.publication.lock().await;
        let mutation = crate::provider_mutation::ProviderMutation::upsert(&request)?;
        let receipt = if let Some(original) = self.lookup_provider_mutation(&mutation).await? {
            original
        } else {
            match self
                .commit_provider_upsert(caller, request, &mutation)
                .await
            {
                Ok(receipt) => receipt,
                Err(error) => self
                    .lookup_provider_mutation(&mutation)
                    .await?
                    .ok_or(error)?,
            }
        };
        self.finish_provider_mutation(receipt).await
    }

    async fn delete_provider(
        &self,
        caller: WebUiAuthenticatedCaller,
        provider_id: String,
        expected_revision: u64,
        mutation_id: uuid::Uuid,
    ) -> Result<LlmConfigSnapshot, LlmConfigServiceError> {
        let _publication = self.publication.lock().await;
        let mutation = crate::provider_mutation::ProviderMutation::new(
            mutation_id,
            "deactivate",
            serde_json::json!({"id":provider_id,"expected_revision":expected_revision}),
            None,
        )?;
        let receipt = if let Some(original) = self.lookup_provider_mutation(&mutation).await? {
            original
        } else {
            match self
                .commit_provider_deactivation(&caller, &provider_id, expected_revision, &mutation)
                .await
            {
                Ok(receipt) => receipt,
                Err(error) => self
                    .lookup_provider_mutation(&mutation)
                    .await?
                    .ok_or(error)?,
            }
        };
        self.finish_provider_mutation(receipt).await
    }

    async fn provider_mutation(
        &self,
        _caller: WebUiAuthenticatedCaller,
        mutation_id: uuid::Uuid,
    ) -> Result<brassclaw_product_workflow::ProviderMutationReceipt, LlmConfigServiceError> {
        let client = self
            .pg_pool
            .get()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        crate::provider_mutation::read_original(&**client, &self.db_tenant_id, mutation_id).await
    }

    async fn reconcile_provider_mutation(
        &self,
        caller: WebUiAuthenticatedCaller,
        mutation_id: uuid::Uuid,
    ) -> Result<LlmConfigSnapshot, LlmConfigServiceError> {
        let _publication = self.publication.lock().await;
        let receipt = self.provider_mutation(caller, mutation_id).await?;
        self.finish_provider_mutation(receipt).await
    }

    async fn set_active(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: SetActiveLlmRequest,
    ) -> Result<LlmConfigSnapshot, LlmConfigServiceError> {
        let _publication = self.publication.lock().await;
        let role = request.role.unwrap_or(ProviderRole::Kohai);
        let mut payload = serde_json::json!({"provider_id":request.provider_id,"model":request.model,"role":role,"expected_revision":request.expected_revision});
        if let Some(reference) = &request.expected_selection_ref {
            payload["expected_selection_ref"] = serde_json::json!(reference);
        }
        let mutation = crate::provider_mutation::ProviderMutation::new(
            request.mutation_id,
            "select",
            payload,
            None,
        )?;
        let receipt = if let Some(original) = self.lookup_provider_mutation(&mutation).await? {
            original
        } else {
            match self
                .commit_provider_selection(&caller, &request, role, &mutation)
                .await
            {
                Ok(receipt) => receipt,
                Err(error) => self
                    .lookup_provider_mutation(&mutation)
                    .await?
                    .ok_or(error)?,
            }
        };
        self.finish_provider_mutation(receipt).await
    }

    async fn test_connection(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: LlmProbeRequest,
    ) -> Result<LlmProbeResult, LlmConfigServiceError> {
        // Provider constructors and a default empty list_models implementation
        // do not establish connectivity. Use the same explicit, bounded
        // exchange as discovery and preserve its classified failures.
        let result = self.list_models(caller, request).await?;
        Ok(LlmProbeResult {
            ok: result.ok,
            message: if result.ok {
                format!("connection ok — {} models available", result.models.len())
            } else {
                result.message
            },
        })
    }

    async fn list_models(
        &self,
        _caller: WebUiAuthenticatedCaller,
        request: LlmProbeRequest,
    ) -> Result<LlmModelsResult, LlmConfigServiceError> {
        let CapturedProbe {
            protocol,
            endpoint,
            key,
            model_discovery: policy,
        } = self.capture_probe(&request).await?;
        let pairs = match brassclaw_llm::model_discovery::fetch_models(
            protocol,
            &endpoint,
            key.as_ref().map(|value| value.expose_secret()),
            policy,
        )
        .await
        {
            Ok(models) => models,
            Err(error) => {
                return Ok(LlmModelsResult {
                    ok: false,
                    models: Vec::new(),
                    message: error.to_string(),
                });
            }
        };

        if pairs.is_empty() {
            Ok(LlmModelsResult {
                ok: false,
                models: Vec::new(),
                message: "No models were returned by the provider endpoint.".to_string(),
            })
        } else {
            Ok(LlmModelsResult {
                ok: true,
                models: pairs.into_iter().map(|(id, _label)| id).collect(),
                message: String::new(),
            })
        }
    }

    async fn start_nearai_login(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: NearAiLoginRequest,
    ) -> Result<NearAiLoginStart, LlmConfigServiceError> {
        let session = self
            .nearai_session
            .as_ref()
            .ok_or(LlmConfigServiceError::Unavailable)?;

        // Point NEAR AI at the server's own public callback route (aligned with
        // the SSO PublicRouteMount pattern, not a second loopback listener).
        // NEAR AI redirects to `<frontend_callback>/auth/callback?token=...`, so
        // `frontend_callback` is this server's NEAR AI route prefix on the
        // browser's own origin (validated to a bare scheme://host[:port]).
        let origin = sanitize_origin(&request.origin).ok_or_else(|| {
            LlmConfigServiceError::InvalidRequest {
                field: Some("origin".to_string()),
                reason: "origin must be a bare http(s) origin".to_string(),
            }
        })?;
        let context = self
            .capture_nearai_login(&caller, &origin, request.provider.as_path())
            .await?;
        let mutation_id = context.id;
        let state = context.id.to_string();
        let frontend_callback = format!("{origin}{NEARAI_LOGIN_PREFIX}/{state}");
        let mut auth_url = url::Url::parse(&format!(
            "{}/v1/auth/{}",
            session.auth_base_url(),
            request.provider.as_path()
        ))
        .map_err(|_| LlmConfigServiceError::Internal)?;
        auth_url
            .query_pairs_mut()
            .append_pair("frontend_callback", &frontend_callback);

        self.nearai_login_states.issue(context).await;
        Ok(NearAiLoginStart {
            mutation_id,
            auth_url: auth_url.to_string(),
        })
    }

    // The native Codex device-login path is owned by Root's native auth
    // publication owner. Legacy session-file polling is retired here; the
    // new trait contract accepts a SetActiveLlmRequest and returns the
    // mutation-scoped start record. That wiring is not yet complete.
    async fn start_codex_login(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _request: brassclaw_product_workflow::SetActiveLlmRequest,
    ) -> Result<CodexLoginStart, LlmConfigServiceError> {
        Err(LlmConfigServiceError::Unavailable)
    }

    async fn codex_login_status(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _id: uuid::Uuid,
    ) -> Result<brassclaw_product_workflow::CodexLoginStatus, LlmConfigServiceError> {
        Err(LlmConfigServiceError::Unavailable)
    }

    async fn start_nearai_wallet_login(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: NearAiWalletLoginStartRequest,
    ) -> Result<NearAiWalletLoginStart, LlmConfigServiceError> {
        self.begin_nearai_wallet_login(&caller, request).await
    }

    async fn complete_nearai_wallet_login(
        &self,
        _caller: WebUiAuthenticatedCaller,
        request: NearAiWalletLoginRequest,
    ) -> Result<NearAiWalletLoginResult, LlmConfigServiceError> {
        self.finish_nearai_wallet_login(request).await
    }
}

/// Server route prefix handed to NEAR AI as `frontend_callback`, with an issued
/// state segment appended per login flow. NEAR AI appends
/// `/auth/callback?token=...`, so the public callback route is
/// `{NEARAI_LOGIN_PREFIX}/{state}/auth/callback`.
pub(crate) const NEARAI_LOGIN_PREFIX: &str = "/api/webchat/v2/llm/nearai";

/// The public callback path NEAR AI redirects to (token in the query). The
/// `{state}` segment must match an authenticated start request before the
/// callback can write the operator-wide session.
pub(crate) const NEARAI_LOGIN_CALLBACK_PATH: &str =
    "/api/webchat/v2/llm/nearai/{state}/auth/callback";

/// Reduce a browser-supplied origin to a bare `scheme://host[:port]`, rejecting
/// anything with a path/query or a non-http scheme. NEAR AI redirects the token
/// here, so it must be a clean same-machine origin.
fn sanitize_origin(raw: &str) -> Option<String> {
    let parsed = url::Url::parse(raw.trim()).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    let host = parsed.host_str()?;
    let mut origin = format!("{}://{host}", parsed.scheme());
    if let Some(port) = parsed.port() {
        origin.push_str(&format!(":{port}"));
    }
    Some(origin)
}

/// Write the Kohai provider + model to `brassclaw_config`.
///
/// Used from background tasks (Codex login, NEAR AI wallet login) that lack
/// `self` access. Postgres is required — `pg_pool` is a mandatory field on
/// `RebornLlmConfigService` after the V047/V048 migration.
#[cfg(test)]
pub(crate) async fn write_kohai_selection_to_db(
    pool: &Arc<brassclaw_pg::PgPool>,
    tenant_id: &str,
    provider_id: &str,
    model: Option<&str>,
) -> Result<(), String> {
    use crate::db_config::{ConfigWriteContext, save_config_changes};
    let provider = (!provider_id.is_empty()).then_some(provider_id);
    let model = model.filter(|model| provider.is_some() && !model.is_empty());
    save_config_changes(
        pool,
        tenant_id,
        &[
            ("llm.default.provider_id", provider),
            ("llm.default.model", model),
        ],
        ConfigWriteContext::Operator,
    )
    .await
    .map_err(|error| error.to_string())
}

/// Parse a wire adapter name (e.g. `open_ai_completions`) into a protocol.
fn parse_adapter(adapter: &str) -> Option<ProviderProtocol> {
    serde_json::from_value(serde_json::Value::String(adapter.to_string())).ok()
}

fn normalized_endpoint(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.trim_end_matches('/').to_string())
}

/// Resolve the overlay `ProviderDefinition` to write for an upsert.
///
/// When `builtin` is `Some` the id names a compiled-in provider: clone its
/// definition (preserving protocol, setup hints, env-var names) and overlay
/// only the operator's `base_url`/`model`, retaining authentication requirements.
/// Stored credentials are supplied to resolution separately. When `builtin`
/// is `None` it's a brand-new
/// custom provider, which needs a valid `adapter`.
fn build_overlay_definition(
    id: &str,
    builtin: Option<&ProviderDefinition>,
    adapter: &str,
    base_url: Option<String>,
    model: Option<String>,
    name: Option<&str>,
) -> Result<ProviderDefinition, LlmConfigServiceError> {
    if let Some(builtin) = builtin {
        let mut def = builtin.clone();
        if let Some(base_url) = base_url {
            def.default_base_url = Some(base_url);
        }
        if let Some(model) = model {
            def.default_model = model;
        }
        return Ok(def);
    }

    let protocol = parse_adapter(adapter).ok_or_else(|| LlmConfigServiceError::InvalidRequest {
        field: Some("adapter".to_string()),
        reason: format!("unknown adapter `{adapter}`"),
    })?;
    let mut def = custom_definition(id, protocol, base_url, model.unwrap_or_default());
    def.description = name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| id.to_string());
    Ok(def)
}

/// Build a custom (operator-defined) provider definition. The API key is never
/// stored in the catalogue. Custom endpoints may use optional credentials;
/// localhost OpenAI-compatible endpoints need no artificial API key.
fn custom_definition(
    id: &str,
    protocol: ProviderProtocol,
    base_url: Option<String>,
    default_model: String,
) -> ProviderDefinition {
    ProviderDefinition {
        embedding_models: Default::default(),
        model_token_limits: Default::default(),
        model_discovery: Default::default(),
        request_policy: Default::default(),
        id: id.to_string(),
        aliases: Vec::new(),
        protocol,
        default_base_url: base_url,
        base_url_env: None,
        base_url_required: false,
        api_key_env: None,
        api_key_required: false,
        model_env: synthetic_model_env(id),
        default_model,
        description: id.to_string(),
        extra_headers_env: None,
        unsupported_params: Vec::new(),
        setup: None,
        token_budget: None,
        context_window_tokens: None,
        cache_retention: None,
        response_cache: Default::default(),
    }
}

fn synthetic_model_env(id: &str) -> String {
    let upper: String = id
        .chars()
        .map(|c| {
            if c == '-' {
                '_'
            } else {
                c.to_ascii_uppercase()
            }
        })
        .collect();
    format!("LLM_CUSTOM_{upper}_MODEL")
}

/// The masked sentinel the UI sends for "key unchanged".
pub(crate) fn is_masked_sentinel(value: &SecretString) -> bool {
    value.expose_secret().chars().all(|c| c == '\u{2022}')
}

const PROVIDER_ID_MAX_LEN: usize = 64;

fn validate_provider_id(id: &str) -> Result<String, LlmConfigServiceError> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return Err(LlmConfigServiceError::InvalidRequest {
            field: Some("id".to_string()),
            reason: "provider id cannot be empty".to_string(),
        });
    }
    if trimmed.len() > PROVIDER_ID_MAX_LEN {
        return Err(LlmConfigServiceError::InvalidRequest {
            field: Some("id".to_string()),
            reason: format!(
                "provider id must be ≤ {} characters, got {}",
                PROVIDER_ID_MAX_LEN,
                trimmed.len()
            ),
        });
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
    {
        return Err(LlmConfigServiceError::InvalidRequest {
            field: Some("id".to_string()),
            reason: "provider id may only contain lowercase letters, digits, '_' or '-'"
                .to_string(),
        });
    }
    Ok(trimmed.to_string())
}

fn required_selection_ref(reference: Option<&str>) -> Result<&str, LlmConfigServiceError> {
    reference
        .filter(|value| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
        .ok_or_else(|| LlmConfigServiceError::InvalidRequest {
            field: Some("expected_selection_ref".into()),
            reason: "Use the model selection reference from the displayed settings.".into(),
        })
}

fn map_selection_edit(error: crate::db_config::ConfigError) -> LlmConfigServiceError {
    match error {
        crate::db_config::ConfigError::ModelSelectionConflict => LlmConfigServiceError::Conflict {
            reason: "Model selection changed; reload its settings before saving.".into(),
        },
        _ => LlmConfigServiceError::Unavailable,
    }
}

impl RebornLlmConfigService {
    async fn commit_provider_upsert(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: UpsertLlmProviderRequest,
        mutation: &crate::provider_mutation::ProviderMutation,
    ) -> Result<brassclaw_product_workflow::ProviderMutationReceipt, LlmConfigServiceError> {
        let id = validate_provider_id(&request.id)?;
        let expected_selection_ref = if request.set_active {
            Some(required_selection_ref(request.expected_selection_ref.as_deref())?.to_owned())
        } else {
            None
        };

        let base_url = request
            .base_url
            .clone()
            .filter(|url| !url.trim().is_empty());
        let model = request
            .default_model
            .clone()
            .filter(|model| !model.trim().is_empty());
        let has_new_key = request
            .api_key
            .as_ref()
            .is_some_and(|key| !is_masked_sentinel(key));
        let existing = self
            .pg_provider_repo
            .get_revision(&id, true)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        if request.expected_revision != existing.as_ref().map(|row| row.revision) {
            return Err(LlmConfigServiceError::Conflict {
                reason: "Provider changed; reload its settings before saving.".into(),
            });
        }
        let previous_definition = existing.as_ref().map(|row| row.definition.clone());

        let protocol = parse_adapter(&request.adapter).ok_or_else(|| {
            LlmConfigServiceError::InvalidRequest {
                field: Some("adapter".into()),
                reason: "Unsupported provider adapter.".into(),
            }
        })?;
        // Origin never restricts editing. Retain unedited catalogue metadata for
        // either origin while the adapter remains compatible.
        let mut definition = if let Some(row) = existing
            .as_ref()
            .filter(|row| row.definition.protocol == protocol)
        {
            let existing_def = &row.definition;
            let mut merged = existing_def.clone();
            if let Some(url) = base_url {
                merged.default_base_url = Some(url);
            }
            if let Some(m) = model {
                merged.default_model = m;
            }
            if let Some(name) = request.name.as_deref().filter(|n| !n.trim().is_empty()) {
                merged.description = name.to_string();
            }
            // Authentication requirements are definition data. An encrypted
            // credential satisfies them without rewriting that contract.
            merged
        } else {
            // New identity or explicitly changed adapter: prepare its configuration.
            build_overlay_definition(
                &id,
                None,
                &request.adapter,
                base_url,
                model,
                request.name.as_deref(),
            )?
        };

        // Merge token_budget from the request.  When the request carries a
        // budget, convert the view type to the catalog type (same fields,
        // different crate).  When the request omits it, keep the existing
        // budget from the previous overlay (if any), or leave it unset for
        // new providers.
        if let Some(v) = request.token_budget.as_ref() {
            definition.token_budget = Some(brassclaw_llm::ProviderTokenBudget {
                profile: v.profile.clone(),
                conversation_history: v.conversation_history,
                skills: v.skills,
                identity: v.identity,
                inline_control: v.inline_control,
                memory: v.memory,
                safety: v.safety,
                capability_surface: v.capability_surface,
                total_input: v.total_input,
                max_output: v.max_output,
            });
        } else if let Some(prev) = previous_definition.as_ref() {
            // No new budget sent: preserve whatever was already on the definition.
            definition.token_budget = prev.token_budget.clone();
        }

        if let Some(policy) = request.response_cache {
            definition.response_cache = policy;
        } else if let Some(previous) = previous_definition.as_ref() {
            definition.response_cache = previous.response_cache;
        }
        if let Some(policy) = request.model_discovery {
            definition.model_discovery = policy;
        } else if let Some(previous) = previous_definition.as_ref() {
            definition.model_discovery = previous.model_discovery;
        }
        if let Some(policy) = request.request_policy {
            definition.request_policy = policy;
        } else if let Some(previous) = previous_definition.as_ref() {
            definition.request_policy = previous.request_policy;
        }
        if let Some(models) = request.embedding_models {
            definition.embedding_models = models;
        } else if let Some(previous) = previous_definition.as_ref() {
            definition.embedding_models = previous.embedding_models.clone();
        }
        if let Some(limits) = request.model_token_limits {
            definition.model_token_limits = limits;
        } else if let Some(previous) = previous_definition.as_ref() {
            definition.model_token_limits = previous.model_token_limits.clone();
        }
        let registry = ProviderRegistry::new(vec![definition.clone()]);
        crate::llm_catalog::validate_catalog(&registry).map_err(|_| {
            LlmConfigServiceError::InvalidRequest {
                field: Some("provider".into()),
                reason: "Provider definition is invalid.".into(),
            }
        })?;
        if let Some(endpoint) = definition.default_base_url.as_deref() {
            let parsed =
                url::Url::parse(endpoint).map_err(|_| LlmConfigServiceError::InvalidRequest {
                    field: Some("base_url".into()),
                    reason: "Enter a valid HTTP or HTTPS endpoint.".into(),
                })?;
            if !matches!(parsed.scheme(), "http" | "https")
                || parsed.host_str().is_none()
                || !parsed.username().is_empty()
                || parsed.password().is_some()
                || parsed.query().is_some()
                || parsed.fragment().is_some()
            {
                return Err(LlmConfigServiceError::InvalidRequest {
                    field: Some("base_url".into()),
                    reason:
                        "Endpoint must use HTTP or HTTPS without credentials, query or fragment."
                            .into(),
                });
            }
        }
        #[cfg(feature = "skills-db")]
        let bootstrap_runtime = if request.set_active {
            self.bootstrap_runtime()?
        } else {
            None
        };
        let mut prepared = None;
        // Reactivation is explicit and verifies the retained authentication and
        // adapter configuration. Draft/new inactive setup can remain incomplete.
        if request.set_active || existing.as_ref().is_some_and(|row| row.inactive) {
            // Authenticate the original generation before constructing a client;
            // release all DB resources before any SDK/session preparation.
            let (key, credential) = if has_new_key {
                let material = request
                    .api_key
                    .clone()
                    .ok_or(LlmConfigServiceError::Unavailable)?;
                (
                    Some(material.clone()),
                    crate::llm_catalog::PreparedCredentialOrigin::Supplied { material },
                )
            } else {
                let mut client = self
                    .pg_pool
                    .get()
                    .await
                    .map_err(|_| LlmConfigServiceError::Unavailable)?;
                let tx = client
                    .transaction()
                    .await
                    .map_err(|_| LlmConfigServiceError::Unavailable)?;
                let authenticated_ref = self
                    .keys
                    .record_reference_in_transaction(&tx, &id)
                    .await
                    .map_err(|_| LlmConfigServiceError::Unavailable)?;
                let material = self
                    .keys
                    .read_in_transaction(&tx, &id)
                    .await
                    .map_err(|_| LlmConfigServiceError::Unavailable)?;
                tx.commit()
                    .await
                    .map_err(|_| LlmConfigServiceError::Unavailable)?;
                (
                    material,
                    crate::llm_catalog::PreparedCredentialOrigin::Unchanged { authenticated_ref },
                )
            };
            let selection = LlmSlotSelection {
                provider_id: Some(id.clone()),
                model: request.set_active.then(|| request.model.clone()).flatten(),
                api_key_env: None,
                base_url: None,
            };
            let config = resolve_against_registry_with_credentials(&selection, &registry, key)
                .map_err(|_| LlmConfigServiceError::InvalidRequest {
                    field: Some("provider".into()), reason: "Complete the provider endpoint, model and authentication before activation.".into(),
                })?;
            let session = brassclaw_llm::create_session_manager(config.session.clone()).await;
            drop(
                brassclaw_llm::build_static_provider_chain(&config, session)
                    .await
                    .map_err(|_| LlmConfigServiceError::Unavailable)?,
            );
            let mut selection = selection;
            selection.model = Some(config.active_model_name());
            prepared = Some(crate::llm_catalog::PreparedProviderSelection {
                config,
                origin: crate::llm_catalog::PreparedProviderOrigin::Draft {
                    definition: definition.clone(),
                    selection,
                    credential,
                },
            });
        }

        // Definition, encrypted key and requested primary selection share one
        // commit. A failed write leaves no earlier key publication to compensate.
        if request.set_active && self.reload.is_none() {
            return Err(LlmConfigServiceError::Unavailable);
        }
        #[cfg(feature = "skills-db")]
        let bootstrap_prepared = if let Some(runtime) = &bootstrap_runtime {
            Some(runtime.prepare_provider_bootstrap().await?)
        } else {
            None
        };
        // Keep RC: mutation.lock waits then reads the committed original; the
        // role owner checks current values including previously absent keys.
        let mut client = self
            .pg_pool
            .get()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let transaction = client
            .transaction()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        mutation.lock(&transaction, &self.db_tenant_id).await?;
        if let Some(original) = mutation
            .original(&transaction, &self.db_tenant_id, &self.keys)
            .await?
        {
            transaction
                .commit()
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable)?;
            return Ok(original);
        }
        let retained = self
            .pg_provider_repo
            .replace_revision_in_transaction(&transaction, definition, request.expected_revision)
            .await
            .map_err(|error| match error {
                crate::pg_provider_repo::PgProviderRepoError::RevisionConflict => {
                    LlmConfigServiceError::Conflict {
                        reason: "Provider changed; reload its settings before saving.".into(),
                    }
                }
                _ => LlmConfigServiceError::Unavailable,
            })?;
        let prepared_revision = if let Some(prepared) = &prepared {
            let crate::llm_catalog::PreparedProviderOrigin::Draft {
                definition,
                selection,
                credential,
            } = &prepared.origin
            else {
                return Err(LlmConfigServiceError::Unavailable);
            };
            if serde_json::to_value(definition).map_err(|_| LlmConfigServiceError::Unavailable)?
                != serde_json::to_value(&retained.definition)
                    .map_err(|_| LlmConfigServiceError::Unavailable)?
            {
                return Err(LlmConfigServiceError::Conflict {
                    reason: "Committed definition differs from prepared provider.".into(),
                });
            }
            match credential {
                crate::llm_catalog::PreparedCredentialOrigin::Unchanged { authenticated_ref } => {
                    let current = self
                        .keys
                        .record_reference_in_transaction(&transaction, &id)
                        .await
                        .map_err(|_| LlmConfigServiceError::Unavailable)?;
                    if &current != authenticated_ref {
                        return Err(LlmConfigServiceError::Conflict {
                            reason: "Authentication changed during provider preparation.".into(),
                        });
                    }
                }
                crate::llm_catalog::PreparedCredentialOrigin::Supplied { material } => {
                    self.keys
                        .put_in_transaction(&transaction, &id, material.clone())
                        .await
                        .map_err(|_| LlmConfigServiceError::Unavailable)?;
                }
            }
            let credential_ref = self
                .keys
                .record_reference_in_transaction(&transaction, &id)
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable)?;
            let revision = crate::provider_admin::selection_revision(
                &self.db_tenant_id,
                selection,
                &retained.definition,
                retained.revision,
                credential_ref.as_deref(),
            )
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
            if revision.model != prepared.config.active_model_name() {
                return Err(LlmConfigServiceError::Unavailable);
            }
            Some(revision)
        } else {
            if has_new_key && let Some(key) = request.api_key {
                self.keys
                    .put_in_transaction(&transaction, &id, key)
                    .await
                    .map_err(|_| LlmConfigServiceError::Unavailable)?;
            }
            None
        };
        if let Some(expected) = expected_selection_ref.as_deref() {
            let prepared = prepared_revision
                .as_ref()
                .ok_or(LlmConfigServiceError::Unavailable)?;
            crate::db_config::save_model_selection_in_transaction(
                &transaction,
                &self.db_tenant_id,
                "kohai",
                expected,
                Some(id.as_str()),
                Some(prepared.model.as_str()),
            )
            .await
            .map_err(map_selection_edit)?;
        }
        #[cfg(feature = "skills-db")]
        let bootstrap_job_id = if let (Some(runtime), Some(bootstrap), Some(prepared)) =
            (&bootstrap_runtime, &bootstrap_prepared, &prepared_revision)
        {
            Some(
                runtime
                    .reserve_provider_bootstrap(
                        &transaction,
                        bootstrap,
                        mutation.id(),
                        caller,
                        &retained,
                        &prepared.model,
                    )
                    .await?,
            )
        } else {
            None
        };
        #[cfg(not(feature = "skills-db"))]
        let bootstrap_job_id = None;
        let receipt = mutation
            .record_with_links(
                &transaction,
                &self.db_tenant_id,
                caller.user_id.as_str(),
                Some((&id, retained.revision)),
                &self.keys,
                crate::provider_mutation::ProviderMutationLinks {
                    bootstrap_job_id,
                    ..Default::default()
                },
            )
            .await?;
        transaction
            .commit()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        drop(client);
        Ok(receipt)
    }
}

impl RebornLlmConfigService {
    async fn commit_provider_deactivation(
        &self,
        caller: &WebUiAuthenticatedCaller,
        provider_id: &str,
        expected_revision: u64,
        mutation: &crate::provider_mutation::ProviderMutation,
    ) -> Result<brassclaw_product_workflow::ProviderMutationReceipt, LlmConfigServiceError> {
        let id = validate_provider_id(provider_id)?;
        let expected = i64::try_from(expected_revision)
            .ok()
            .filter(|revision| *revision > 0)
            .ok_or_else(|| LlmConfigServiceError::InvalidRequest {
                field: Some("expected_revision".into()),
                reason: "Use a positive stored provider revision.".into(),
            })?;
        let mut client = self
            .pg_pool
            .get()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let transaction = client
            .transaction()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        mutation.lock(&transaction, &self.db_tenant_id).await?;
        if let Some(original) = mutation
            .original(&transaction, &self.db_tenant_id, &self.keys)
            .await?
        {
            transaction
                .commit()
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable)?;
            return Ok(original);
        }
        let removed = self
            .pg_provider_repo
            .deactivate_in_transaction(&transaction, &id, Some(expected))
            .await
            .map_err(|error| match error {
                crate::pg_provider_repo::PgProviderRepoError::RevisionConflict => {
                    LlmConfigServiceError::Conflict {
                        reason: "Provider changed; reload its settings before removing it.".into(),
                    }
                }
                _ => LlmConfigServiceError::Unavailable,
            })?;
        if !removed {
            return Err(LlmConfigServiceError::NotFound);
        }
        let retained = self
            .pg_provider_repo
            .get_revision_from_client(&*transaction, &id, true)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?
            .ok_or(LlmConfigServiceError::Unavailable)?;
        let receipt = mutation
            .record(
                &transaction,
                &self.db_tenant_id,
                caller.user_id.as_str(),
                Some((&id, retained.revision)),
                &self.keys,
            )
            .await?;
        transaction
            .commit()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        Ok(receipt)
    }
}

impl RebornLlmConfigService {
    async fn commit_provider_selection(
        &self,
        caller: &WebUiAuthenticatedCaller,
        request: &SetActiveLlmRequest,
        role: ProviderRole,
        mutation: &crate::provider_mutation::ProviderMutation,
    ) -> Result<brassclaw_product_workflow::ProviderMutationReceipt, LlmConfigServiceError> {
        let expected_selection_ref =
            required_selection_ref(request.expected_selection_ref.as_deref())?;
        if matches!(role, ProviderRole::Kohai) && self.reload.is_none()
            || matches!(role, ProviderRole::Sempai) && self.sempai_swappable.is_none()
        {
            return Err(LlmConfigServiceError::Unavailable);
        }
        #[cfg(feature = "skills-db")]
        let bootstrap_runtime =
            if role != ProviderRole::Embedding && !request.provider_id.trim().is_empty() {
                self.bootstrap_runtime()?
            } else {
                None
            };
        let mut prepared = None;
        let selected = if request.provider_id.trim().is_empty() {
            None
        } else {
            let id = validate_provider_id(&request.provider_id)?;
            let selected = self
                .pg_provider_repo
                .get_revision(&id, false)
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable)?
                .ok_or(LlmConfigServiceError::NotFound)?;
            let expected_revision =
                request
                    .expected_revision
                    .ok_or_else(|| LlmConfigServiceError::InvalidRequest {
                        field: Some("expected_revision".into()),
                        reason: "Use the provider revision from the displayed settings.".into(),
                    })?;
            if expected_revision != selected.revision {
                return Err(LlmConfigServiceError::Conflict {
                    reason: "Provider changed; reload its settings before selecting it.".into(),
                });
            }
            // Construct outside the DB transaction. The selected definition is
            // fenced again under a row lock before recording this publication.
            prepared = self
                .prepare_provider_selection(&selected.definition.id, request.model.as_deref(), role)
                .await?;
            Some(selected)
        };
        #[cfg(feature = "skills-db")]
        let bootstrap_prepared = if let Some(runtime) = &bootstrap_runtime {
            Some(runtime.prepare_provider_bootstrap().await?)
        } else {
            None
        };
        // Keep RC: mutation.lock waits then reads the committed original; the
        // role owner checks current values including previously absent keys.
        let mut client = self
            .pg_pool
            .get()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let transaction = client
            .transaction()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        mutation.lock(&transaction, &self.db_tenant_id).await?;
        if let Some(original) = mutation
            .original(&transaction, &self.db_tenant_id, &self.keys)
            .await?
        {
            transaction
                .commit()
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable)?;
            return Ok(original);
        }
        if let Some(selected) = &selected {
            let current = self
                .pg_provider_repo
                .lock_active_revision(&transaction, &selected.definition.id)
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable)?
                .ok_or(LlmConfigServiceError::NotFound)?;
            if current.revision != selected.revision {
                return Err(LlmConfigServiceError::Conflict {
                    reason:
                        "Provider changed while preparing its selection; reload before retrying."
                            .into(),
                });
            }
        }
        if let Some(prepared) = &prepared {
            let crate::llm_catalog::PreparedProviderOrigin::Retained {
                definition,
                selection,
                revision,
                credential_ref,
            } = &prepared.origin
            else {
                return Err(LlmConfigServiceError::Unavailable);
            };
            let selected = selected
                .as_ref()
                .ok_or(LlmConfigServiceError::Unavailable)?;
            let actual_credential = self
                .keys
                .record_reference_in_transaction(&transaction, &definition.id)
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable)?;
            if revision.definition_revision != selected.revision
                || definition.id != selected.definition.id
                || serde_json::to_value(definition)
                    .map_err(|_| LlmConfigServiceError::Unavailable)?
                    != serde_json::to_value(&selected.definition)
                        .map_err(|_| LlmConfigServiceError::Unavailable)?
                || &actual_credential != credential_ref
                || revision.model != prepared.config.active_model_name()
                || selection.model.as_deref() != Some(revision.model.as_str())
                || selection.provider_id.as_deref() != Some(definition.id.as_str())
            {
                return Err(LlmConfigServiceError::Conflict{reason:"Provider or authentication changed during preparation; reload before retrying.".into()});
            }
        }
        let role_name = match role {
            ProviderRole::Kohai => "kohai",
            ProviderRole::Sempai => "sempai",
            ProviderRole::Embedding => "embedding",
        };
        let provider = selected.as_ref().map(|row| row.definition.id.as_str());
        let model = prepared
            .as_ref()
            .map(|p| p.config.active_model_name())
            .or_else(|| {
                provider
                    .and(request.model.as_deref())
                    .filter(|model| !model.is_empty())
                    .map(str::to_owned)
            });
        crate::db_config::save_model_selection_in_transaction(
            &transaction,
            &self.db_tenant_id,
            role_name,
            expected_selection_ref,
            provider,
            model.as_deref(),
        )
        .await
        .map_err(map_selection_edit)?;
        #[cfg(feature = "skills-db")]
        let bootstrap_job_id =
            if let (Some(runtime), Some(bootstrap), Some(selected), Some(_prepared)) = (
                &bootstrap_runtime,
                &bootstrap_prepared,
                &selected,
                &prepared,
            ) {
                Some(
                    runtime
                        .reserve_provider_bootstrap(
                            &transaction,
                            bootstrap,
                            mutation.id(),
                            caller,
                            selected,
                            model.as_deref().ok_or(LlmConfigServiceError::Unavailable)?,
                        )
                        .await?,
                )
            } else {
                None
            };
        #[cfg(not(feature = "skills-db"))]
        let bootstrap_job_id = None;
        let receipt = mutation
            .record_with_links(
                &transaction,
                &self.db_tenant_id,
                caller.user_id.as_str(),
                selected
                    .as_ref()
                    .map(|row| (row.definition.id.as_str(), row.revision)),
                &self.keys,
                crate::provider_mutation::ProviderMutationLinks {
                    bootstrap_job_id,
                    ..Default::default()
                },
            )
            .await?;
        transaction
            .commit()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        Ok(receipt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn native_role_publication_is_atomic_and_allows_shared_provider() {
        use crate::db_config::{ConfigWriteContext, list_config_keys, save_config_changes};
        use brassclaw_host_api::{TenantId, UserId};
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let pool = rig.pool.clone();
        let repo = Arc::new(crate::PgProviderRepo::new((*pool).clone(), "default"));
        let mut definition = custom_definition(
            "shared-provider",
            ProviderProtocol::OpenAiCompletions,
            Some("http://127.0.0.1:8000/v1".into()),
            "default-model".into(),
        );
        definition.api_key_required = true;
        definition.aliases = vec!["shared-alias".into()];
        repo.upsert(definition).await.unwrap();
        save_config_changes(
            &pool,
            "default",
            &[
                ("llm.default.provider_id", Some("shared-provider")),
                ("llm.default.model", Some("kohai-model")),
            ],
            ConfigWriteContext::Operator,
        )
        .await
        .unwrap();
        let keys = LlmKeyStore::new_postgres(Arc::new(
            brassclaw_secrets::PgSecretStore::new(
                (*pool).clone(),
                SecretString::from("native-configuration-test-master-key"),
                "default",
            )
            .unwrap(),
        ));
        keys.put(
            "shared-provider",
            SecretString::from("native-test-stored-provider-credential"),
        )
        .await
        .unwrap();
        assert!(keys.read("shared-provider").await.unwrap().is_some());
        let swappable = Arc::new(brassclaw_llm::SwappableLlmProvider::new(Arc::new(
            crate::runtime::PlaceholderLlmProvider,
        )));
        let mode = brassclaw_interceptor::SharedInterceptorMode::new();
        let kohai = Arc::new(brassclaw_llm::SwappableLlmProvider::new(Arc::new(
            crate::runtime::PlaceholderLlmProvider,
        )));
        let cheap = Arc::new(brassclaw_llm::SwappableLlmProvider::new(Arc::new(
            crate::runtime::PlaceholderLlmProvider,
        )));
        let handle = Arc::new(brassclaw_llm::LlmReloadHandle::new_with_sempai(
            kohai.clone(),
            Some(cheap.clone()),
            Some(swappable.clone()),
        ));
        let session =
            brassclaw_llm::create_session_manager(brassclaw_llm::SessionConfig::default()).await;
        let reload = Arc::new(crate::llm_reload::RebornLlmReloadAdapter::new(
            handle,
            session,
            keys.clone(),
            pool.clone(),
            "default",
        ));
        reload.reload().await.unwrap();
        assert_eq!(kohai.pinned_provider().1, "kohai-model");
        let retained_kohai = kohai.pinned_provider().0;
        let service = RebornLlmConfigService::new(keys, pool.clone(), repo, "default")
            .with_reload_trigger(reload)
            .with_sempai_swappable(swappable.clone())
            .with_interceptor_mode(mode.clone());
        let caller = WebUiAuthenticatedCaller::new(
            TenantId::new("default").unwrap(),
            UserId::new("operator").unwrap(),
            None,
            None,
        );
        let observed = service.snapshot(caller.clone()).await.unwrap();
        let definition_revision = observed
            .providers
            .iter()
            .find(|row| row.id == "shared-provider")
            .unwrap()
            .revision;
        let request =
            |model: &str, refs: &std::collections::BTreeMap<String, String>| SetActiveLlmRequest {
                mutation_id: uuid::Uuid::new_v4(),
                expected_revision: Some(definition_revision),
                expected_selection_ref: Some(refs["sempai"].clone()),
                provider_id: "shared-provider".into(),
                model: Some(model.into()),
                role: Some(ProviderRole::Sempai),
            };
        let admitted_primary = kohai.pinned_provider().0;
        let snapshot = service
            .set_active(
                caller.clone(),
                request("sempai-model", &observed.role_selection_refs),
            )
            .await
            .unwrap();
        assert!(
            Arc::ptr_eq(&admitted_primary, &kohai.pinned_provider().0),
            "selecting Sempai must preserve the independent Kohai client/cache/circuit"
        );
        assert_eq!(
            snapshot.mutation_receipt.as_ref().unwrap().selected_role,
            Some(ProviderRole::Sempai)
        );
        let before_invalid_embedding = list_config_keys(&pool, "default").await.unwrap();
        let invalid_embedding_id = uuid::Uuid::new_v4();
        let admitted_sempai = swappable.pinned_provider().0;
        assert!(matches!(
            service
                .set_active(
                    caller.clone(),
                    SetActiveLlmRequest {
                        mutation_id: invalid_embedding_id,
                        expected_revision: Some(definition_revision),
                        expected_selection_ref: Some(
                            snapshot.role_selection_refs["embedding"].clone()
                        ),
                        provider_id: "shared-provider".into(),
                        model: Some("missing-embedding-contract".into()),
                        role: Some(ProviderRole::Embedding),
                    }
                )
                .await,
            Err(LlmConfigServiceError::InvalidRequest { .. })
        ));
        assert!(matches!(
            service
                .provider_mutation(caller.clone(), invalid_embedding_id)
                .await,
            Err(LlmConfigServiceError::NotFound)
        ));
        assert_eq!(
            before_invalid_embedding,
            list_config_keys(&pool, "default").await.unwrap()
        );
        assert!(Arc::ptr_eq(&admitted_primary, &kohai.pinned_provider().0));
        assert!(Arc::ptr_eq(
            &admitted_sempai,
            &swappable.pinned_provider().0
        ));
        let restored = Arc::new(brassclaw_llm::SwappableLlmProvider::new(Arc::new(
            crate::runtime::PlaceholderLlmProvider,
        )));
        let restored_mode = brassclaw_interceptor::SharedInterceptorMode::new();
        RebornLlmConfigService::new(
            service.keys.clone(),
            pool.clone(),
            service.pg_provider_repo.clone(),
            "default",
        )
        .with_sempai_swappable(restored.clone())
        .with_interceptor_mode(restored_mode.clone())
        .restore_sempai_from_db()
        .await
        .unwrap();
        assert_eq!(restored.pinned_provider().1, "sempai-model");
        assert_eq!(
            restored_mode.get(),
            brassclaw_interceptor::InterceptorMode::Rerouting
        );
        assert_eq!(
            snapshot.kohai_active.as_ref().unwrap().provider_id,
            "shared-provider"
        );
        assert_eq!(
            snapshot.kohai_active.as_ref().unwrap().model.as_deref(),
            Some("kohai-model")
        );
        assert_eq!(
            snapshot.sempai_active.as_ref().unwrap().provider_id,
            "shared-provider"
        );
        assert_eq!(
            snapshot.sempai_active.as_ref().unwrap().model.as_deref(),
            Some("sempai-model")
        );
        let (retained_provider, retained_model) = swappable.pinned_provider();
        assert_eq!(retained_model, "sempai-model");
        assert_eq!(
            mode.get(),
            brassclaw_interceptor::InterceptorMode::Rerouting
        );
        let cleared_kohai = service
            .set_active(
                caller.clone(),
                SetActiveLlmRequest {
                    mutation_id: uuid::Uuid::new_v4(),
                    expected_revision: None,
                    expected_selection_ref: Some(snapshot.role_selection_refs["kohai"].clone()),
                    provider_id: String::new(),
                    model: Some("must-not-survive-clear".into()),
                    role: Some(ProviderRole::Kohai),
                },
            )
            .await
            .unwrap();
        assert!(cleared_kohai.kohai_active.is_none());
        assert_eq!(retained_kohai.active_model_name(), "kohai-model");
        assert_eq!(kohai.pinned_provider().1, "unconfigured");
        assert_eq!(cheap.pinned_provider().1, "unconfigured");
        assert_eq!(swappable.pinned_provider().1, "sempai-model");
        service
            .set_active(
                caller.clone(),
                SetActiveLlmRequest {
                    mutation_id: uuid::Uuid::new_v4(),
                    expected_revision: Some(definition_revision),
                    expected_selection_ref: Some(
                        cleared_kohai.role_selection_refs["kohai"].clone(),
                    ),
                    provider_id: "shared-provider".into(),
                    model: Some("kohai-model".into()),
                    role: Some(ProviderRole::Kohai),
                },
            )
            .await
            .unwrap();
        assert_eq!(kohai.pinned_provider().1, "kohai-model");
        assert!(
            write_kohai_selection_to_db(&pool, "default", "shared-provider", Some("kohai-model"))
                .await
                .is_ok()
        );
        let client = pool.get().await.unwrap();
        // Invalid named selections fail without clearing the preceding backend.
        write_kohai_selection_to_db(&pool, "default", "missing-provider", Some("bad-model"))
            .await
            .unwrap();
        assert!(service.refresh_running_provider().await.is_err());
        assert_eq!(kohai.pinned_provider().1, "kohai-model");
        write_kohai_selection_to_db(&pool, "default", "shared-provider", Some("kohai-model"))
            .await
            .unwrap();
        // Sorted publication writes model first. Reject its following provider
        // write to prove neither a half-selection nor a provider swap escapes.
        client.batch_execute("CREATE FUNCTION reject_role_provider() RETURNS trigger LANGUAGE plpgsql AS $$
            BEGIN IF NEW.key IN ('llm.sempai.provider_id','llm.default.provider_id') THEN RAISE EXCEPTION 'deliberate role rejection'; END IF;
            RETURN NEW; END $$;
            CREATE TRIGGER reject_role_provider BEFORE INSERT OR UPDATE ON brassclaw_config
            FOR EACH ROW EXECUTE FUNCTION reject_role_provider()").await.unwrap();
        assert!(matches!(
            service
                .set_active(
                    caller.clone(),
                    request("rejected-model", &snapshot.role_selection_refs)
                )
                .await,
            Err(LlmConfigServiceError::Unavailable)
        ));
        let keys: HashMap<_, _> = list_config_keys(&pool, "default")
            .await
            .unwrap()
            .into_iter()
            .collect();
        assert_eq!(keys["llm.sempai.model"], "sempai-model");
        assert_eq!(keys["llm.sempai.provider_id"], "shared-provider");
        assert_eq!(swappable.pinned_provider().1, "sempai-model");
        assert!(
            write_kohai_selection_to_db(
                &pool,
                "default",
                "shared-provider",
                Some("rejected-login-model")
            )
            .await
            .is_err()
        );
        let preserved: HashMap<_, _> = list_config_keys(&pool, "default")
            .await
            .unwrap()
            .into_iter()
            .collect();
        assert_eq!(preserved["llm.default.model"], "kohai-model");
        assert_eq!(preserved["llm.default.provider_id"], "shared-provider");
        client.batch_execute("DROP TRIGGER reject_role_provider ON brassclaw_config; DROP FUNCTION reject_role_provider();
            ALTER TABLE brassclaw_secrets RENAME TO unavailable_role_secrets").await.unwrap();
        assert!(matches!(
            service
                .set_active(
                    caller.clone(),
                    request("unreadable-key-model", &snapshot.role_selection_refs)
                )
                .await,
            Err(LlmConfigServiceError::Unavailable)
        ));
        assert!(matches!(
            service.snapshot(caller.clone()).await,
            Err(LlmConfigServiceError::Unavailable)
        ));
        assert_eq!(swappable.pinned_provider().1, "sempai-model");
        client
            .batch_execute("ALTER TABLE unavailable_role_secrets RENAME TO brassclaw_secrets")
            .await
            .unwrap();
        assert!(matches!(
            save_config_changes(
                &pool,
                "default",
                &[("duplicate", Some("a")), ("duplicate", Some("b"))],
                ConfigWriteContext::Operator
            )
            .await,
            Err(crate::db_config::ConfigError::DuplicateKey { .. })
        ));
        assert!(matches!(
            save_config_changes(
                &pool,
                "default",
                &[("llm.default.api_key_env", None)],
                ConfigWriteContext::AgentSession
            )
            .await,
            Err(crate::db_config::ConfigError::EnvKeyWriteForbidden { .. })
        ));
        let cleared = service
            .set_active(
                caller.clone(),
                SetActiveLlmRequest {
                    mutation_id: uuid::Uuid::new_v4(),
                    expected_revision: None,
                    expected_selection_ref: Some(snapshot.role_selection_refs["sempai"].clone()),
                    provider_id: String::new(),
                    model: Some("must-not-survive-clear".into()),
                    role: Some(ProviderRole::Sempai),
                },
            )
            .await
            .unwrap();
        assert!(cleared.sempai_active.is_none());
        assert_eq!(
            cleared.kohai_active.unwrap().model.as_deref(),
            Some("kohai-model")
        );
        assert_eq!(mode.get(), brassclaw_interceptor::InterceptorMode::Routing);
        let keys: HashMap<_, _> = list_config_keys(&pool, "default")
            .await
            .unwrap()
            .into_iter()
            .collect();
        assert!(!keys.contains_key("llm.sempai.provider_id"));
        assert!(!keys.contains_key("llm.sempai.model"));
        // Retained work still owns the actual preceding provider selection.
        assert_eq!(retained_provider.active_model_name(), retained_model);
        service
            .set_active(
                caller.clone(),
                request("sempai-restored-model", &cleared.role_selection_refs),
            )
            .await
            .unwrap();
        save_config_changes(
            &pool,
            "default",
            &[
                ("llm.archive.provider_id", Some("SHARED-ALIAS")),
                ("llm.archive.model", Some("archived-role-model")),
                ("embedding.provider_id", Some("shared-provider")),
                ("embedding.model", Some("embedding-model")),
                ("unrelated.setting", Some("preserved")),
            ],
            ConfigWriteContext::Operator,
        )
        .await
        .unwrap();
        client.execute("UPDATE brassclaw_llm_providers SET is_builtin = TRUE WHERE tenant_id = 'default' AND id = 'shared-provider'", &[]).await.unwrap();
        let retained_definition = service
            .pg_provider_repo
            .get("shared-provider")
            .await
            .unwrap()
            .unwrap();
        let selected_revision = service
            .pg_provider_repo
            .get_revision("shared-provider", false)
            .await
            .unwrap()
            .unwrap()
            .revision;
        client.batch_execute("CREATE FUNCTION reject_deactivation() RETURNS trigger LANGUAGE plpgsql AS $$
            BEGIN IF OLD.key = 'llm.default.provider_id' THEN RAISE EXCEPTION 'deliberate deactivation rejection'; END IF;
            RETURN OLD; END $$;
            CREATE TRIGGER reject_deactivation BEFORE DELETE ON brassclaw_config
            FOR EACH ROW EXECUTE FUNCTION reject_deactivation()").await.unwrap();
        assert!(matches!(
            service
                .delete_provider(
                    caller.clone(),
                    "shared-provider".into(),
                    selected_revision,
                    uuid::Uuid::new_v4()
                )
                .await,
            Err(LlmConfigServiceError::Unavailable)
        ));
        assert!(
            service
                .pg_provider_repo
                .get("shared-provider")
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(kohai.pinned_provider().1, "kohai-model");
        assert_eq!(swappable.pinned_provider().1, "sempai-restored-model");
        client.batch_execute("DROP TRIGGER reject_deactivation ON brassclaw_config; DROP FUNCTION reject_deactivation()").await.unwrap();
        let removed = service
            .delete_provider(
                caller,
                "shared-provider".into(),
                selected_revision,
                uuid::Uuid::new_v4(),
            )
            .await
            .unwrap();
        assert!(removed.kohai_active.is_none());
        assert!(removed.sempai_active.is_none());
        assert!(removed.embedding_active.is_none());
        assert_eq!(kohai.pinned_provider().1, "unconfigured");
        assert_eq!(cheap.pinned_provider().1, "unconfigured");
        assert_eq!(swappable.pinned_provider().1, "unconfigured");
        assert!(
            service
                .keys
                .read("shared-provider")
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            !service
                .pg_provider_repo
                .upsert_builtin(retained_definition)
                .await
                .unwrap()
        );
        assert!(
            service
                .pg_provider_repo
                .get("shared-provider")
                .await
                .unwrap()
                .is_none()
        );
        let remaining: HashMap<_, _> = list_config_keys(&pool, "default")
            .await
            .unwrap()
            .into_iter()
            .collect();
        assert_eq!(remaining["unrelated.setting"], "preserved");
        assert!(
            !remaining
                .keys()
                .any(|key| key.starts_with("llm.") || key.starts_with("embedding."))
        );
        assert_eq!(retained_kohai.active_model_name(), "kohai-model");
    }

    #[test]
    fn parses_known_adapters() {
        assert_eq!(
            parse_adapter("open_ai_completions"),
            Some(ProviderProtocol::OpenAiCompletions)
        );
        assert_eq!(
            parse_adapter("anthropic"),
            Some(ProviderProtocol::Anthropic)
        );
        assert_eq!(parse_adapter("ollama"), Some(ProviderProtocol::Ollama));
        assert_eq!(parse_adapter("nearai"), Some(ProviderProtocol::NearAi));
        assert_eq!(parse_adapter("near_ai"), Some(ProviderProtocol::NearAi));
        assert_eq!(parse_adapter("not_a_real_adapter"), None);
    }

    #[test]
    fn custom_definition_never_requires_or_names_a_key() {
        let def = custom_definition(
            "acme",
            ProviderProtocol::OpenAiCompletions,
            Some("https://api.acme.test/v1".to_string()),
            "acme-large".to_string(),
        );
        assert!(!def.api_key_required);
        assert!(def.api_key_env.is_none());
        assert_eq!(def.model_env, "LLM_CUSTOM_ACME_MODEL");
        assert_eq!(def.default_model, "acme-large");
    }

    #[test]
    fn masked_sentinel_detected() {
        assert!(is_masked_sentinel(&SecretString::from(
            "\u{2022}\u{2022}\u{2022}"
        )));
        assert!(!is_masked_sentinel(&SecretString::from("sk-real-key")));
    }

    #[test]
    fn provider_id_validation_rejects_bad_input() {
        assert!(validate_provider_id("acme_1").is_ok());
        assert!(validate_provider_id("Acme").is_err());
        assert!(validate_provider_id("has space").is_err());
        assert!(validate_provider_id("  ").is_err());
        // Length limit: exactly 64 chars is ok, 65 is rejected.
        assert!(validate_provider_id(&"a".repeat(64)).is_ok());
        assert!(validate_provider_id(&"a".repeat(65)).is_err());
    }

    #[test]
    fn editing_a_stored_provider_preserves_protocol_and_setup() {
        let stored: ProviderDefinition = serde_json::from_value(serde_json::json!({
            "id":"stored-dedicated", "protocol":"openai_codex", "model_env":"RETIRED_METADATA",
            "default_model":"fixture-original-model", "description":"Stored dedicated fixture",
            "setup":{"kind":"o_auth_device_code","backend":"openai_codex","display_name":"Stored login"}
        })).unwrap();
        let builtin = &stored;
        assert_eq!(builtin.protocol, ProviderProtocol::OpenAiCodex);
        let had_setup = builtin.setup.is_some();

        let def = build_overlay_definition(
            "stored-dedicated",
            Some(builtin),
            "ignored_adapter",
            None,
            Some("gpt-5.3-codex".to_string()),
            None,
        )
        .expect("overlay def");

        // Protocol + setup preserved; only the model changed.
        assert_eq!(def.protocol, ProviderProtocol::OpenAiCodex);
        assert_eq!(def.setup.is_some(), had_setup);
        assert_eq!(def.default_model, "gpt-5.3-codex");
        assert_eq!(def.id, "stored-dedicated");
        assert_eq!(
            serde_json::to_value(&def.setup).unwrap(),
            serde_json::to_value(&stored.setup).unwrap()
        );
    }

    #[test]
    fn editing_a_stored_provider_preserves_key_requirement() {
        let stored: ProviderDefinition = serde_json::from_value(serde_json::json!({
            "id":"stored-key-provider", "protocol":"open_ai_completions", "api_key_required":true,
            "model_env":"RETIRED_METADATA", "default_model":"fixture-original-model", "description":"Stored key fixture"
        })).unwrap();
        let openai = &stored;
        assert!(openai.api_key_required, "openai requires a key by default");

        let def = build_overlay_definition(
            "stored-key-provider",
            Some(openai),
            "open_ai_completions",
            None,
            None,
            None,
        )
        .expect("overlay def");
        assert!(
            def.api_key_required,
            "stored credentials must satisfy, not weaken, the required-key contract"
        );
        assert_eq!(def.protocol, ProviderProtocol::OpenAiCompletions);
    }

    #[test]
    fn brand_new_custom_provider_uses_the_request_adapter() {
        let def = build_overlay_definition(
            "acme",
            None,
            "anthropic",
            Some("https://acme.test/v1".to_string()),
            Some("acme-1".to_string()),
            Some("Acme"),
        )
        .expect("overlay def");
        assert_eq!(def.protocol, ProviderProtocol::Anthropic);
        assert_eq!(def.description, "Acme");
        assert!(!def.api_key_required);
    }

    #[test]
    fn brand_new_custom_provider_rejects_unknown_adapter() {
        let err = build_overlay_definition("acme", None, "nonsense", None, None, None)
            .expect_err("unknown adapter must fail");
        assert!(matches!(err, LlmConfigServiceError::InvalidRequest { .. }));
    }
}

// The native-role regression above uses bundled PostgreSQL, production
// migrations, the encrypted secret store and the actual swappable provider.
// It verifies atomic role publication, shared-provider selection, database and
// credential failures, clearing a role and retention of the preceding backend.
// It does not establish inference, Kohai reload or embedding runtime uptake.
