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
use brassclaw_llm::{NearWalletSignedMessage, OpenAiCodexConfig, OpenAiCodexSessionManager};
use brassclaw_product_workflow::{
    CodexLoginStart, LlmActiveSelection, LlmConfigService, LlmConfigServiceError,
    LlmConfigSnapshot, LlmModelsResult, LlmProbeRequest, LlmProbeResult, LlmProviderView,
    NearAiLoginRequest, NearAiLoginStart, NearAiWalletLoginRequest, NearAiWalletLoginResult,
    ProviderTokenBudgetView, SetActiveLlmRequest, UpsertLlmProviderRequest,
    WebUiAuthenticatedCaller,
};
use brassclaw_reborn_config::LlmSlotSelection;
use secrecy::{ExposeSecret as _, SecretString};

use crate::LlmKeyStore;
use crate::llm_catalog::resolve_against_registry_with_credentials;

const NEARAI_LOGIN_STATE_TTL: Duration = Duration::from_secs(15 * 60);
const CODEX_LOGIN_ATTEMPT_TTL: Duration = Duration::from_secs(15 * 60);

/// In-memory CSRF state for NEAR AI browser redirects. The login start endpoint
/// issues a state token, and the public callback must consume it before any
/// operator-wide credential write happens.
#[derive(Debug, Default)]
pub(crate) struct NearAiLoginStateStore {
    states: tokio::sync::Mutex<HashMap<String, Instant>>,
}

impl NearAiLoginStateStore {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) async fn issue(&self) -> String {
        let state = uuid::Uuid::new_v4().to_string();
        let mut states = self.states.lock().await;
        prune_expired(&mut states, Instant::now());
        states.insert(state.clone(), Instant::now() + NEARAI_LOGIN_STATE_TTL);
        state
    }

    pub(crate) async fn consume(&self, state: &str) -> bool {
        let mut states = self.states.lock().await;
        let now = Instant::now();
        prune_expired(&mut states, now);
        states
            .remove(state)
            .is_some_and(|expires_at| expires_at > now)
    }
}

#[derive(Debug, Clone)]
struct CodexLoginAttempt {
    id: uuid::Uuid,
    user_code: String,
    verification_uri: String,
    expires_at: Instant,
}

fn prune_expired(states: &mut HashMap<String, Instant>, now: Instant) {
    states.retain(|_, expires_at| *expires_at > now);
}

/// Live-reload seam. The runtime supplies an impl that re-resolves the LLM
/// config (including any stored key) and atomically swaps the running
/// provider's inner backend; tests / unwired runtimes leave it absent.
#[async_trait]
pub trait LlmReloadTrigger: Send + Sync {
    /// Re-resolve and hot-swap the active provider. The error string is for
    /// logging only and must stay free of secrets / backend internals.
    async fn reload(&self) -> Result<(), String>;

    /// Called by `refresh_running_provider` after a successful reload.
    /// `new_provider_id` is the provider that is now active.
    /// Implementations use this to refresh per-provider budget slots.
    /// The default is a no-op so existing impls (tests, stubs) keep compiling.
    async fn on_provider_changed(&self, _new_provider_id: &str) {}
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
    keys: LlmKeyStore,
    reload: Option<Arc<dyn LlmReloadTrigger>>,
    memory_embedding_source: Option<Arc<dyn brassclaw_memory::EmbeddingProviderSource>>,
    /// The runtime's NEAR AI session manager — the same instance the live
    /// provider reads its token from, so a completed login takes effect on
    /// reload. Absent when the runtime has no LLM seam wired.
    nearai_session: Option<Arc<brassclaw_llm::SessionManager>>,
    nearai_login_states: Arc<NearAiLoginStateStore>,
    codex_login_attempts: Arc<tokio::sync::Mutex<HashMap<String, CodexLoginAttempt>>>,
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

impl RebornLlmConfigService {
    pub fn new(
        keys: LlmKeyStore,
        pg_pool: Arc<brassclaw_pg::PgPool>,
        pg_provider_repo: Arc<crate::pg_provider_repo::PgProviderRepo>,
        db_tenant_id: impl Into<String>,
    ) -> Self {
        Self {
            publication: Arc::new(tokio::sync::Mutex::new(())),
            keys,
            reload: None,
            memory_embedding_source: None,
            nearai_session: None,
            nearai_login_states: Arc::new(NearAiLoginStateStore::new()),
            codex_login_attempts: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
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

    /// Write the complete role selection in one transaction. A failed write
    /// leaves both keys unchanged and returns an error before runtime uptake.
    async fn save_role_to_db(
        &self,
        provider_id_key: &str,
        provider_id_value: &str,
        model_key: &str,
        model_value: &str,
    ) -> Result<(), LlmConfigServiceError> {
        use crate::db_config::{ConfigWriteContext, save_config_changes};
        let provider = (!provider_id_value.is_empty()).then_some(provider_id_value);
        // Clearing a role also removes its model override.
        let model = (provider.is_some() && !model_value.is_empty()).then_some(model_value);
        save_config_changes(
            &self.pg_pool,
            &self.db_tenant_id,
            &[(provider_id_key, provider), (model_key, model)],
            ConfigWriteContext::Operator,
        )
        .await
        .map_err(|_| LlmConfigServiceError::Unavailable)
    }

    /// Retain database failures rather than interpreting an unavailable
    /// configuration store as an unconfigured role.
    async fn read_kohai_sel_from_db(
        &self,
    ) -> Result<Option<LlmActiveSelection>, LlmConfigServiceError> {
        let keys = crate::db_config::list_config_keys(&self.pg_pool, &self.db_tenant_id)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let keys: HashMap<String, String> = keys.into_iter().collect();
        Ok(selection_from_keys(
            &keys,
            "llm.default.provider_id",
            "llm.default.model",
        ))
    }

    /// Reload failures are visible to the writer. Budget publication follows
    /// an acknowledged reload, never a failed attempt with a different backend.
    /// Complete model-profile/revision publication remains a separate cutover.
    async fn refresh_running_provider(&self) -> Result<(), LlmConfigServiceError> {
        self.refresh_memory_embedding().await?;
        let reload = self
            .reload
            .as_ref()
            .ok_or(LlmConfigServiceError::Unavailable)?;
        reload.reload().await.map_err(|reason| {
            tracing::warn!(reason = %reason, "live LLM provider reload failed");
            LlmConfigServiceError::Unavailable
        })?;
        if let Some(selection) = self.read_kohai_sel_from_db().await? {
            reload.on_provider_changed(&selection.provider_id).await;
        }
        Ok(())
    }

    /// Build a live Sempai provider from the stored key + provider definition
    /// using `build_static_provider_chain`, the same path as Kohai.
    ///
    /// Returns `Err(reason_string)` if the provider cannot be resolved or built.
    #[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
    async fn build_sempai_provider(
        &self,
        provider_id: &str,
        model: Option<&str>,
    ) -> Result<Arc<dyn brassclaw_llm::LlmProvider>, String> {
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
                .map_err(|_| "Sempai provider snapshot is unavailable".to_owned())?
                .ok_or_else(|| "selected Sempai provider is unavailable".to_owned())?;
        self.build_sempai_selection(captured).await
    }

    async fn build_sempai_selection(
        &self,
        captured: crate::provider_admin::CapturedProviderSelection,
    ) -> Result<Arc<dyn brassclaw_llm::LlmProvider>, String> {
        let registry = ProviderRegistry::new(vec![captured.definition]);
        let config = crate::llm_catalog::resolve_against_registry_with_credentials(
            &captured.selection,
            &registry,
            captured.credential,
        )
        .map_err(|_| "stored Sempai configuration is invalid".to_string())?;
        let session = brassclaw_llm::create_session_manager(config.session.clone()).await;
        brassclaw_llm::build_static_provider_chain(&config, session)
            .await
            .map_err(|_| "stored Sempai provider could not be constructed".to_string())
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
        let captured =
            crate::RebornProviderAdmin::new(self.pg_pool.as_ref().clone(), &self.db_tenant_id)
                .capture_selection(
                    crate::provider_admin::ProviderSelectionSource::Role(ProviderRole::Sempai),
                    &self.keys,
                )
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let selected = captured.is_some();
        let (provider, provider_id): (Arc<dyn brassclaw_llm::LlmProvider>, String) = match captured
        {
            Some(captured) => {
                let identity = captured.definition.id.clone();
                let provider = self
                    .build_sempai_selection(captured)
                    .await
                    .map_err(|_| LlmConfigServiceError::Unavailable)?;
                (provider, identity)
            }
            None => (
                Arc::new(crate::runtime::PlaceholderLlmProvider),
                "unconfigured".into(),
            ),
        };
        swappable.swap_with_identity(provider, provider_id);
        if let Some(mode) = &self.interceptor_mode {
            if selected {
                mode.set_rerouting();
            } else {
                mode.set_routing();
            }
        }
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
        transaction
            .commit()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        // Release the connection before secret metadata checks acquire their
        // own leases; this also works with a one-connection administration pool.
        drop(client);
        let registry = brassclaw_llm::ProviderRegistry::new(
            all_defs
                .iter()
                .filter(|row| !row.inactive)
                .map(|row| row.definition.clone())
                .collect(),
        );
        crate::llm_catalog::validate_catalog(&registry)
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let canonical = |id: Option<&str>,
                         model: Option<&str>|
         -> Result<Option<LlmActiveSelection>, LlmConfigServiceError> {
            let Some(id) = id.filter(|id| !id.is_empty()) else {
                return Ok(None);
            };
            let definition = registry
                .find(id)
                .ok_or(LlmConfigServiceError::Unavailable)?;
            Ok(Some(LlmActiveSelection {
                provider_id: definition.id.clone(),
                model: model.filter(|model| !model.is_empty()).map(str::to_owned),
            }))
        };
        let role = |name: &str| config.llm.as_ref().and_then(|roles| roles.get(name));
        let kohai = role("default");
        let sempai = role("sempai");
        let embedding = config.embedding.as_ref();
        let kohai_sel = canonical(
            kohai.and_then(|slot| slot.provider_id.as_deref()),
            kohai.and_then(|slot| slot.model.as_deref()),
        )?;
        let sempai_sel = canonical(
            sempai.and_then(|slot| slot.provider_id.as_deref()),
            sempai.and_then(|slot| slot.model.as_deref()),
        )?;
        let embedding_sel = canonical(
            embedding.and_then(|slot| slot.provider_id.as_deref()),
            embedding.and_then(|slot| slot.model.as_deref()),
        )?;

        // Check key existence concurrently — was sequential before (N round-trips).
        let key_checks = all_defs
            .iter()
            .map(|row| self.keys.exists(row.definition.id.as_str()))
            .collect::<Vec<_>>();
        let key_results: Vec<bool> = futures::future::join_all(key_checks)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| LlmConfigServiceError::Unavailable)?;

        let mut providers = Vec::with_capacity(all_defs.len());
        let mut inactive_providers = Vec::new();
        let mut active: Option<LlmActiveSelection> = None;

        for (row, stored_key_set) in all_defs.into_iter().zip(key_results) {
            let inactive = row.inactive;
            let revision = row.revision;
            let is_builtin = row.is_builtin;
            let def = row.definition;
            let api_key_set = stored_key_set;

            let is_kohai = kohai_sel.as_ref().is_some_and(|s| s.provider_id == def.id);
            let active_model = is_kohai
                .then(|| kohai_sel.as_ref().and_then(|s| s.model.clone()))
                .flatten();
            if is_kohai && active.is_none() {
                active = Some(LlmActiveSelection {
                    provider_id: def.id.clone(),
                    model: active_model.clone(),
                });
            }

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

        Ok(LlmConfigSnapshot {
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

    async fn set_provider_async(
        &self,
        id: String,
        model: Option<String>,
    ) -> Result<(), crate::RebornProviderAdminError> {
        self.save_role_to_db(
            "llm.default.provider_id",
            &id,
            "llm.default.model",
            model.as_deref().unwrap_or(""),
        )
        .await
        .map_err(|error| crate::RebornProviderAdminError::LoadDatabase {
            reason: error.to_string(),
        })
    }

    // ------------------------------------------------------------------
    // Provider repo helpers (DB-exclusive — no file fallback)
    // ------------------------------------------------------------------

    /// Load a single active provider definition by id.
    async fn load_provider_by_id(
        &self,
        id: &str,
    ) -> Result<Option<ProviderDefinition>, LlmConfigServiceError> {
        self.pg_provider_repo
            .get(id)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)
    }

    /// Soft-delete a provider definition by id.
    ///
    async fn delete_provider_definition(
        &self,
        id: &str,
        expected_revision: u64,
    ) -> Result<bool, LlmConfigServiceError> {
        self.pg_provider_repo
            .delete_revision(id, expected_revision)
            .await
            .map_err(|error| match error {
                crate::pg_provider_repo::PgProviderRepoError::RevisionConflict => {
                    LlmConfigServiceError::Conflict {
                        reason: "Provider changed; reload its settings before removing it.".into(),
                    }
                }
                _ => LlmConfigServiceError::Unavailable,
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
        let id = validate_provider_id(&request.id)?;

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
        // Reactivation is explicit and verifies the retained authentication and
        // adapter configuration. Draft/new inactive setup can remain incomplete.
        if request.set_active || existing.as_ref().is_some_and(|row| row.inactive) {
            let key = if has_new_key {
                request.api_key.clone()
            } else {
                self.keys
                    .read(&id)
                    .await
                    .map_err(|_| LlmConfigServiceError::Unavailable)?
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
        }

        // Definition, encrypted key and requested primary selection share one
        // commit. A failed write leaves no earlier key publication to compensate.
        if request.set_active && self.reload.is_none() {
            return Err(LlmConfigServiceError::Unavailable);
        }
        let mut client = self
            .pg_pool
            .get()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let transaction = client
            .transaction()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        self.pg_provider_repo
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
        if has_new_key && let Some(key) = request.api_key {
            self.keys
                .put_in_transaction(&transaction, &id, key)
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable)?;
        }
        if request.set_active {
            crate::db_config::save_config_changes_in_transaction(
                &transaction,
                &self.db_tenant_id,
                &[
                    ("llm.default.provider_id", Some(id.as_str())),
                    (
                        "llm.default.model",
                        request.model.as_deref().filter(|model| !model.is_empty()),
                    ),
                ],
                crate::db_config::ConfigWriteContext::Operator,
            )
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        }
        transaction
            .commit()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        drop(client);
        // A runtime uptake failure remains visible. Never overwrite a newer
        // publication by compensating against independently read old values.
        if self.sempai_swappable.is_some() {
            self.restore_sempai_from_db_locked().await?;
        }
        self.refresh_running_provider().await?;
        self.snapshot(caller).await
    }

    async fn delete_provider(
        &self,
        caller: WebUiAuthenticatedCaller,
        provider_id: String,
        expected_revision: u64,
    ) -> Result<LlmConfigSnapshot, LlmConfigServiceError> {
        let _publication = self.publication.lock().await;
        let id = validate_provider_id(&provider_id)?;
        // Delete via DB-backed repo.
        let removed = self
            .delete_provider_definition(&id, expected_revision)
            .await?;
        if !removed {
            return Err(LlmConfigServiceError::NotFound);
        }
        // Removing a catalogue entry is reversible. Keep its encrypted key;
        // the repository cleared every affected role/model in the same commit.
        self.restore_sempai_from_db_locked().await?;
        self.refresh_running_provider().await?;
        self.snapshot(caller).await
    }

    async fn set_active(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: SetActiveLlmRequest,
    ) -> Result<LlmConfigSnapshot, LlmConfigServiceError> {
        let _publication = self.publication.lock().await;
        let role = request.role.unwrap_or(ProviderRole::Kohai);

        // Validate provider id (allow empty string to clear the slot).
        let mut id = if request.provider_id.trim().is_empty() {
            String::new()
        } else {
            validate_provider_id(&request.provider_id)?
        };

        // Roles select independently. Sharing a provider or endpoint does not
        // merge their prompts, model overrides, histories or task identities.
        if !id.is_empty() {
            id = self
                .load_provider_by_id(&id)
                .await?
                .ok_or(LlmConfigServiceError::NotFound)?
                .id;
        }

        match role {
            ProviderRole::Kohai => {
                if self.reload.is_none() {
                    return Err(LlmConfigServiceError::Unavailable);
                }
                self.set_provider_async(id, request.model)
                    .await
                    .map_err(map_admin_error)?;
                self.refresh_running_provider().await?;
            }
            ProviderRole::Sempai => {
                let swappable = self
                    .sempai_swappable
                    .as_ref()
                    .ok_or(LlmConfigServiceError::Unavailable)?;
                let new_provider: Arc<dyn brassclaw_llm::LlmProvider> = if id.is_empty() {
                    Arc::new(crate::runtime::PlaceholderLlmProvider)
                } else {
                    self.build_sempai_provider(&id, request.model.as_deref())
                        .await
                        .map_err(|_| LlmConfigServiceError::Unavailable)?
                };
                self.save_role_to_db(
                    "llm.sempai.provider_id",
                    &id,
                    "llm.sempai.model",
                    request.model.as_deref().unwrap_or(""),
                )
                .await?;
                swappable.swap_with_identity(
                    new_provider,
                    if id.is_empty() { "unconfigured" } else { &id },
                );
                if let Some(mode) = &self.interceptor_mode {
                    if id.is_empty() {
                        mode.set_routing();
                    } else {
                        mode.set_rerouting();
                    }
                }
            }
            ProviderRole::Embedding => {
                // DB is the sole write target — file write removed in V047/V048.
                self.save_role_to_db(
                    "embedding.provider_id",
                    if id.is_empty() { "" } else { &id },
                    "embedding.model",
                    request.model.as_deref().unwrap_or(""),
                )
                .await?;
                self.refresh_memory_embedding().await?;
            }
        }

        self.snapshot(caller).await
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
        _caller: WebUiAuthenticatedCaller,
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
        let state = self.nearai_login_states.issue().await;
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

        Ok(NearAiLoginStart {
            auth_url: auth_url.to_string(),
        })
    }

    async fn start_codex_login(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<CodexLoginStart, LlmConfigServiceError> {
        let attempt_key = codex_login_attempt_key(&caller);
        let now = Instant::now();
        {
            let mut attempts = self.codex_login_attempts.lock().await;
            attempts.retain(|_, attempt| attempt.expires_at > now);
            if let Some(attempt) = attempts.get(&attempt_key) {
                return Ok(CodexLoginStart {
                    user_code: attempt.user_code.clone(),
                    verification_uri: attempt.verification_uri.clone(),
                });
            }
        }

        // Point the login manager at the same session file the live openai_codex
        // provider reads on reload (mirror resolution.rs env precedence). The
        // model is irrelevant to the device-code flow, so leave it defaulted.
        let codex_config = OpenAiCodexConfig::build(
            None,
            nonempty_env("OPENAI_CODEX_AUTH_URL"),
            nonempty_env("OPENAI_CODEX_API_URL"),
            nonempty_env("OPENAI_CODEX_CLIENT_ID"),
            nonempty_env("OPENAI_CODEX_SESSION_PATH").map(std::path::PathBuf::from),
            None,
        );
        let manager = OpenAiCodexSessionManager::new(codex_config)
            .map_err(|_| LlmConfigServiceError::Internal)?;
        let start = manager
            .initiate_device_code()
            .await
            .map_err(|_| LlmConfigServiceError::Internal)?;

        let login = CodexLoginStart {
            user_code: start.user_code.clone(),
            verification_uri: start.verification_uri.clone(),
        };
        let attempt_id = uuid::Uuid::new_v4();
        {
            let mut attempts = self.codex_login_attempts.lock().await;
            attempts.insert(
                attempt_key.clone(),
                CodexLoginAttempt {
                    id: attempt_id,
                    user_code: login.user_code.clone(),
                    verification_uri: login.verification_uri.clone(),
                    expires_at: Instant::now() + CODEX_LOGIN_ATTEMPT_TTL,
                },
            );
        }

        // Poll for authorization off-thread: persist the tokens, make Codex the
        // active provider, and hot-swap the running provider. The frontend polls
        // the snapshot until openai_codex is active. The DB config is the source
        // of truth, so a reload failure still applies on restart.
        let reload = self.reload.clone();
        let attempts = Arc::clone(&self.codex_login_attempts);
        let codex_pool = self.pg_pool.clone();
        let codex_tenant = self.db_tenant_id.clone();
        let publication = Arc::clone(&self.publication);
        tokio::spawn(async move {
            if let Err(error) = manager.complete_device_code(&start).await {
                tracing::debug!(%error, "codex device login did not complete");
                remove_codex_attempt_if_current(&attempts, &attempt_key, attempt_id).await;
                return;
            }
            let _publication = publication.lock().await;
            if !remove_codex_attempt_if_current(&attempts, &attempt_key, attempt_id).await {
                tracing::debug!("codex login completed after a newer attempt superseded it");
                return;
            }
            if let Err(error) =
                write_kohai_selection_to_db(&codex_pool, &codex_tenant, "openai_codex", None).await
            {
                tracing::debug!(%error, "codex login: could not set active provider");
                return;
            }
            if let Some(reload) = reload
                && let Err(error) = reload.reload().await
            {
                tracing::debug!(%error, "codex login: live reload failed; applies on restart");
            }
        });

        Ok(login)
    }

    async fn complete_nearai_wallet_login(
        &self,
        _caller: WebUiAuthenticatedCaller,
        request: NearAiWalletLoginRequest,
    ) -> Result<NearAiWalletLoginResult, LlmConfigServiceError> {
        let session = self
            .nearai_session
            .as_ref()
            .ok_or(LlmConfigServiceError::Unavailable)?;

        // Exchange the browser-signed NEP-413 message for a NEAR AI session
        // token. NEAR AI is the authority on the message/recipient/nonce
        // constraints, so a bad signature comes back as an error here; surface a
        // generic failure rather than leaking the provider's reason.
        let signed = NearWalletSignedMessage {
            account_id: request.account_id,
            public_key: request.public_key,
            signature: request.signature,
            message: request.message,
            recipient: request.recipient,
            nonce: request.nonce,
            callback_url: request.callback_url,
        };
        let token = session.near_wallet_login(&signed).await.map_err(|error| {
            tracing::debug!(%error, "NEAR AI wallet login exchange failed");
            LlmConfigServiceError::InvalidRequest {
                field: None,
                reason: "NEAR wallet sign-in failed".to_string(),
            }
        })?;

        let _publication = self.publication.lock().await;
        // Apply the token the same way the SSO callback does: persist it, make
        // NEAR AI active, and hot-swap the running provider. Without a reload
        // seam the selection still persists and applies on restart.
        session
            .save_session_for_renewer(&token, Some("nearai"))
            .await
            .map_err(|error| {
                tracing::debug!(%error, "NEAR AI wallet login: token persist failed");
                LlmConfigServiceError::Internal
            })?;
        write_kohai_selection_to_db(&self.pg_pool, &self.db_tenant_id, "nearai", None)
            .await
            .map_err(|error| {
                tracing::debug!(%error, "NEAR AI wallet login: set active failed");
                LlmConfigServiceError::Internal
            })?;
        let active = match &self.reload {
            Some(reload) => {
                reload.reload().await.map_err(|error| {
                    tracing::debug!(%error, "NEAR AI wallet login: live reload failed");
                    LlmConfigServiceError::Internal
                })?;
                true
            }
            None => false,
        };
        Ok(NearAiWalletLoginResult { active })
    }
}

/// Read an env var, treating empty/whitespace as absent. Mirrors the precedence
/// `brassclaw_llm::resolution` uses so the Codex login manager resolves the same
/// session path / client id / auth URL as the live provider.
fn nonempty_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn codex_login_attempt_key(caller: &WebUiAuthenticatedCaller) -> String {
    format!("{}:{}", caller.tenant_id.as_str(), caller.user_id.as_str())
}

async fn remove_codex_attempt_if_current(
    attempts: &tokio::sync::Mutex<HashMap<String, CodexLoginAttempt>>,
    key: &str,
    attempt_id: uuid::Uuid,
) -> bool {
    let mut attempts = attempts.lock().await;
    let Some(attempt) = attempts.get(key) else {
        return false;
    };
    if attempt.id != attempt_id {
        return false;
    }
    attempts.remove(key);
    true
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

/// Apply a completed NEAR AI login: store the session token on the live
/// session and hot-swap the running provider. Shared by the public callback
/// route. Errors are log-only strings.
///
/// Note: the Kohai selection DB write (`llm.default.provider_id = "nearai"`)
/// must be performed by the caller before invoking this function (via
/// `set_active(Kohai)` or the new `write_kohai_selection_to_db` helper).
pub(crate) async fn apply_nearai_login(
    session: &brassclaw_llm::SessionManager,
    reload: &dyn LlmReloadTrigger,
    token: &str,
) -> Result<(), String> {
    session
        .save_session_for_renewer(token, Some("nearai"))
        .await
        .map_err(|error| error.to_string())?;
    reload.reload().await
}

/// Write the Kohai provider + model to `brassclaw_config`.
///
/// Used from background tasks (Codex login, NEAR AI wallet login) that lack
/// `self` access. Postgres is required — `pg_pool` is a mandatory field on
/// `RebornLlmConfigService` after the V047/V048 migration.
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
fn is_masked_sentinel(value: &SecretString) -> bool {
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

fn selection_from_keys(
    keys: &HashMap<String, String>,
    provider_key: &str,
    model_key: &str,
) -> Option<LlmActiveSelection> {
    let provider_id = keys
        .get(provider_key)
        .filter(|value| !value.is_empty())?
        .clone();
    let model = keys
        .get(model_key)
        .filter(|value| !value.is_empty())
        .cloned();
    Some(LlmActiveSelection { provider_id, model })
}

fn map_admin_error(error: crate::RebornProviderAdminError) -> LlmConfigServiceError {
    use crate::RebornProviderAdminError as E;
    match error {
        E::UnknownProvider { .. } => LlmConfigServiceError::NotFound,
        E::InvalidRequest { reason } => LlmConfigServiceError::InvalidRequest {
            field: None,
            reason,
        },
        E::LoadRegistry { .. } | E::LoadDatabase { .. } => LlmConfigServiceError::Unavailable,
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
        let keys = LlmKeyStore::new(Arc::new(
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
        let request = |model: &str| SetActiveLlmRequest {
            provider_id: "shared-provider".into(),
            model: Some(model.into()),
            role: Some(ProviderRole::Sempai),
        };
        let snapshot = service
            .set_active(caller.clone(), request("sempai-model"))
            .await
            .unwrap();
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
                .set_active(caller.clone(), request("rejected-model"))
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
                .set_active(caller.clone(), request("unreadable-key-model"))
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
            .set_active(caller.clone(), request("sempai-restored-model"))
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
                .delete_provider(caller.clone(), "shared-provider".into(), selected_revision)
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
            .delete_provider(caller, "shared-provider".into(), selected_revision)
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

    #[tokio::test]
    async fn nearai_login_state_is_single_use() {
        let store = NearAiLoginStateStore::new();
        let state = store.issue().await;

        assert!(store.consume(&state).await);
        assert!(
            !store.consume(&state).await,
            "state must not be reusable after a successful callback"
        );
        assert!(!store.consume("missing-state").await);
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
