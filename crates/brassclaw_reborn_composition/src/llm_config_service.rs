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
//! - API-key **values**            → scoped secret store (never DB)

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

use crate::llm_catalog::{apply_stored_api_key, resolve_against_registry};
use crate::LlmKeyStore;

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

    #[allow(dead_code)]
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

/// Operator-wide LLM configuration service backing the webui2 settings surface.
///
/// Requires a Postgres pool and provider repo — the service is DB-exclusive
/// after the V047/V048 migration.  There is no file-based fallback.
pub struct RebornLlmConfigService {
    keys: LlmKeyStore,
    reload: Option<Arc<dyn LlmReloadTrigger>>,
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
            keys,
            reload: None,
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

    /// Write a role's provider_id + model to `brassclaw_config`.
    ///
    /// Errors are logged at `debug!` level and swallowed — the DB write
    /// is best-effort and a failure must not prevent the operation from
    /// completing.
    ///
    /// When `provider_id` is empty the key is deleted so a cleared slot
    /// does not leave a stale row in the DB.
    async fn save_role_to_db(
        &self,
        provider_id_key: &str,
        provider_id_value: &str,
        model_key: &str,
        model_value: &str,
    ) {
        use crate::db_config::{ConfigWriteContext, delete_config_key, save_config_key};

        let pool = &*self.pg_pool;
        let tenant_id = self.db_tenant_id.as_str();

        // provider_id —— empty means "clear the role slot".
        let pid_result = if provider_id_value.is_empty() {
            delete_config_key(pool, tenant_id, provider_id_key).await
        } else {
            save_config_key(
                pool,
                tenant_id,
                provider_id_key,
                provider_id_value,
                ConfigWriteContext::Operator,
            )
            .await
        };
        if let Err(e) = pid_result {
            tracing::debug!(key = provider_id_key, error = %e, "role DB write failed");
        }

        // model —— write when non-empty; delete when empty (slot cleared or unset).
        let model_result = if model_value.is_empty() {
            delete_config_key(pool, tenant_id, model_key).await
        } else {
            save_config_key(
                pool,
                tenant_id,
                model_key,
                model_value,
                ConfigWriteContext::Operator,
            )
            .await
        };
        if let Err(e) = model_result {
            tracing::debug!(key = model_key, error = %e, "role DB write failed");
        }
    }

    /// Read the Sempai role selection from DB.
    async fn read_sempai_sel_from_db(&self) -> Option<LlmActiveSelection> {
        use crate::db_config::list_config_keys;
        let rows = list_config_keys(&self.pg_pool, &self.db_tenant_id).await.ok()?;
        let kv: std::collections::HashMap<String, String> = rows.into_iter().collect();
        let provider_id = kv.get("llm.sempai.provider_id")?.to_string();
        if provider_id.is_empty() {
            return None;
        }
        let model = kv
            .get("llm.sempai.model")
            .cloned()
            .filter(|s| !s.is_empty());
        Some(LlmActiveSelection { provider_id, model })
    }

    /// Read the Kohai (default) role selection from DB.
    ///
    /// Keys: `llm.default.provider_id` / `llm.default.model` in `brassclaw_config`.
    async fn read_kohai_sel_from_db(&self) -> Option<LlmActiveSelection> {
        use crate::db_config::list_config_keys;
        let rows = list_config_keys(&self.pg_pool, &self.db_tenant_id).await.ok()?;
        let kv: std::collections::HashMap<String, String> = rows.into_iter().collect();
        let provider_id = kv.get("llm.default.provider_id")?.to_string();
        if provider_id.is_empty() {
            return None;
        }
        let model = kv
            .get("llm.default.model")
            .cloned()
            .filter(|s| !s.is_empty());
        Some(LlmActiveSelection { provider_id, model })
    }

    /// Read the Embedding role selection from DB.
    ///
    /// Keys: `embedding.provider_id` / `embedding.model` in `brassclaw_config`.
    async fn read_embedding_sel_from_db(&self) -> Option<LlmActiveSelection> {
        use crate::db_config::list_config_keys;
        let rows = list_config_keys(&self.pg_pool, &self.db_tenant_id).await.ok()?;
        let kv: std::collections::HashMap<String, String> = rows.into_iter().collect();
        let provider_id = kv.get("embedding.provider_id")?.to_string();
        if provider_id.is_empty() {
            return None;
        }
        let model = kv
            .get("embedding.model")
            .cloned()
            .filter(|s| !s.is_empty());
        Some(LlmActiveSelection { provider_id, model })
    }

    /// Persist-then-reload: the DB write already happened; refresh the
    /// running provider. A reload failure is logged, not fatal — the DB
    /// config is authoritative and applies on next restart.
    ///
    /// The reload swaps the live provider's *inner* backend. It does NOT yet
    /// update the model gateway's pinned model-profile route or cost table
    /// (those are built once at boot), so changing the active *model* fully
    /// applies on restart; for providers that honor per-request model overrides
    /// the gateway still pins the boot model until then. A swappable model
    /// gateway (and live reload from a no-LLM cold boot, where no reload handle
    /// exists at all) is owned by the first-run provider work.
    async fn refresh_running_provider(&self) {
        let Some(reload) = self.reload.as_ref() else {
            // Cold boot: no LLM was configured at startup, so there is no live
            // provider to swap into. Don't fail silently — tell the operator the
            // saved config needs a restart to take effect.
            tracing::warn!(
                "LLM configuration saved, but no live LLM provider was configured at startup \
                 (no config.toml or provider env creds), so it cannot be applied to the running \
                 process. Restart the server to use the new configuration."
            );
            return;
        };
        if let Err(reason) = reload.reload().await {
            tracing::warn!(
                reason = %reason,
                "LLM config persisted but live provider reload failed; change applies on restart"
            );
        }
        // Tell budget slots which provider is now active so they can re-read
        // per-provider settings without a restart.
        if let Some(sel) = self.read_kohai_sel_from_db().await {
            reload.on_provider_changed(&sel.provider_id).await;
        }
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
        use crate::llm_catalog::resolve_against_registry;
        // LlmSlotSelection is in brassclaw_reborn_config, not brassclaw_llm — the
        // top-level import at line 35 covers this; suppress the inner re-import.

        // Load from DB (all providers — builtins and custom — are seeded there).
        let definition = self
            .load_provider_by_id(provider_id)
            .await
            .map_err(|e| format!("provider load: {e}"))?
            .ok_or_else(|| format!("provider not found: {provider_id}"))?;

        let registry = brassclaw_llm::registry::ProviderRegistry::new(vec![definition]);
        let selection = LlmSlotSelection {
            provider_id: Some(provider_id.to_string()),
            model: model.map(|s| s.to_string()),
            api_key_env: None,
            base_url: None,
        };
        let mut config = resolve_against_registry(&selection, &registry)
            .map_err(|e| format!("resolve: {e}"))?;
        if let Ok(Some(stored)) = self.keys.read(provider_id).await {
            crate::llm_catalog::apply_stored_api_key(&mut config, stored);
        }
        let session = brassclaw_llm::create_session_manager(config.session.clone()).await;
        brassclaw_llm::build_static_provider_chain(&config, session)
            .await
            .map_err(|e| format!("build: {e}"))
    }

    async fn build_snapshot(&self) -> Result<LlmConfigSnapshot, LlmConfigServiceError> {
        use crate::provider_admin::provider_protocol_wire_name;

        // Single DB query — all providers (builtin + custom), builtins first.
        let all_defs = self
            .pg_provider_repo
            .load_all()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;

        let kohai_sel = self.read_kohai_sel_from_db().await;
        let sempai_sel = self.read_sempai_sel_from_db().await;
        let embedding_sel = self.read_embedding_sel_from_db().await;

        // Check key existence concurrently — was sequential before (N round-trips).
        let key_checks = all_defs
            .iter()
            .map(|(d, _)| self.keys.exists(d.id.as_str()))
            .collect::<Vec<_>>();
        let key_results: Vec<bool> = futures::future::join_all(key_checks)
            .await
            .into_iter()
            .map(|r| r.unwrap_or(false))
            .collect();

        let mut providers = Vec::with_capacity(all_defs.len());
        let mut active: Option<LlmActiveSelection> = None;

        for ((def, is_builtin), stored_key_set) in all_defs.into_iter().zip(key_results) {
            let env_key_set = def
                .api_key_env
                .as_ref()
                .is_some_and(|env| std::env::var(env).is_ok());
            let api_key_set = stored_key_set || env_key_set;

            let is_kohai = kohai_sel
                .as_ref()
                .is_some_and(|s| s.provider_id == def.id);
            let active_model = is_kohai
                .then(|| kohai_sel.as_ref().and_then(|s| s.model.clone()))
                .flatten();
            if is_kohai && active.is_none() {
                active = Some(LlmActiveSelection {
                    provider_id: def.id.clone(),
                    model: active_model.clone(),
                });
            }

            let can_list_models = def
                .setup
                .as_ref()
                .is_some_and(brassclaw_llm::registry::SetupHint::can_list_models);
            let accepts_api_key = def.api_key_env.is_some()
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

            providers.push(LlmProviderView {
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
            });
        }

        Ok(LlmConfigSnapshot {
            providers,
            active: active.clone(),
            kohai_active: active,
            sempai_active: sempai_sel,
            embedding_active: embedding_sel,
        })
    }

    /// Build a transient provider from a probe request and run a closure
    /// against it. Reused by `test_connection` and `list_models`.
    async fn probe_provider(
        &self,
        request: &LlmProbeRequest,
    ) -> Result<Arc<dyn brassclaw_llm::LlmProvider>, LlmConfigServiceError> {
        let protocol = parse_adapter(&request.adapter).ok_or_else(|| {
            LlmConfigServiceError::InvalidRequest {
                field: Some("adapter".to_string()),
                reason: format!("unknown adapter `{}`", request.adapter),
            }
        })?;
        let base_url = request
            .base_url
            .clone()
            .filter(|url| !url.trim().is_empty());
        let model = request
            .model
            .clone()
            .filter(|model| !model.trim().is_empty())
            .unwrap_or_default();

        let definition = custom_definition(&request.provider_id, protocol, base_url.clone(), model);
        let registry = ProviderRegistry::new(vec![definition]);
        let stored_key_allowed = self.probe_matches_persisted_provider(request).await?;
        let selection = LlmSlotSelection {
            provider_id: Some(request.provider_id.clone()),
            model: request
                .model
                .clone()
                .filter(|model| !model.trim().is_empty()),
            api_key_env: None,
            base_url,
        };
        let mut config = resolve_against_registry(&selection, &registry).map_err(|error| {
            LlmConfigServiceError::InvalidRequest {
                field: None,
                reason: error.to_string(),
            }
        })?;

        // Prefer the request's inline key. Stored operator credentials are only
        // safe when the probe targets the persisted provider endpoint; otherwise
        // a caller-controlled base_url could exfiltrate that key.
        if let Some(key) = request.api_key.as_ref() {
            apply_stored_api_key(&mut config, key.clone());
        } else if stored_key_allowed
            && let Some(stored) = self
                .keys
                .read(&request.provider_id)
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable)?
        {
            apply_stored_api_key(&mut config, stored);
        }
        // If no API key is available, continue anyway - the provider will fail
        // with a proper authentication error, which is more useful than blocking
        // the test entirely

        let session = brassclaw_llm::create_session_manager(config.session.clone()).await;
        brassclaw_llm::build_static_provider_chain(&config, session)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)
    }

    async fn probe_matches_persisted_provider(
        &self,
        request: &LlmProbeRequest,
    ) -> Result<bool, LlmConfigServiceError> {
        // All providers (builtin and custom) are seeded into the DB.
        // If not found there, the provider does not exist.
        let Some(definition) = self.load_provider_by_id(&request.provider_id).await? else {
            return Ok(false);
        };
        let Some(protocol) = parse_adapter(&request.adapter) else {
            return Ok(false);
        };
        Ok(protocol == definition.protocol
            && normalized_endpoint(request.base_url.as_deref())
                == normalized_endpoint(definition.default_base_url.as_deref()))
    }

    async fn set_provider_async(
        &self,
        id: String,
        model: Option<String>,
    ) -> Result<(), crate::RebornProviderAdminError> {
        use crate::db_config::{ConfigWriteContext, save_config_key};
        let tenant = &self.db_tenant_id;
        if let Err(e) = save_config_key(
            &self.pg_pool,
            tenant,
            "llm.default.provider_id",
            &id,
            ConfigWriteContext::Operator,
        )
        .await
        {
            tracing::debug!(error = %e, "set_provider_async: DB write failed");
            return Err(crate::RebornProviderAdminError::InvalidRequest {
                reason: format!("provider DB write failed: {e}"),
            });
        }
        let model_val = model.as_deref().unwrap_or("");
        if let Err(e) = save_config_key(
            &self.pg_pool,
            tenant,
            "llm.default.model",
            model_val,
            ConfigWriteContext::Operator,
        )
        .await
        {
            tracing::debug!(error = %e, "set_provider_async: model DB write failed");
        }
        Ok(())
    }

    async fn rollback_provider_definition(
        &self,
        id: &str,
        previous_definition: Option<ProviderDefinition>,
    ) {
        let overlay_result = if let Some(previous_definition) = previous_definition {
            self.upsert_provider_definition(previous_definition)
                .await
                .map(|_| ())
        } else {
            self.delete_provider_definition(id).await.map(|_| ())
        };
        if let Err(error) = overlay_result {
            tracing::warn!(
                provider_id = %id,
                error = %error,
                "failed to roll back LLM provider overlay after active-selection failure",
            );
        }
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

    /// Upsert a custom provider definition.
    async fn upsert_provider_definition(
        &self,
        definition: ProviderDefinition,
    ) -> Result<bool, LlmConfigServiceError> {
        self.pg_provider_repo
            .upsert(definition)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)
    }

    /// Soft-delete a provider definition by id.
    ///
    /// Returns `Err(CannotDeleteBuiltin)` if the provider is a builtin.
    async fn delete_provider_definition(&self, id: &str) -> Result<bool, LlmConfigServiceError> {
        self.pg_provider_repo.delete(id).await.map_err(|e| {
            if matches!(e, crate::pg_provider_repo::PgProviderRepoError::CannotDeleteBuiltin) {
                LlmConfigServiceError::CannotDeleteBuiltin
            } else {
                LlmConfigServiceError::Unavailable
            }
        })
    }

    async fn rollback_provider_key(&self, id: &str, previous_key: Option<SecretString>) {
        let key_result = if let Some(previous_key) = previous_key {
            self.keys.put(id, previous_key).await.map(|_| ())
        } else {
            self.keys.delete(id).await.map(|_| ())
        };
        if let Err(error) = key_result {
            tracing::warn!(
                provider_id = %id,
                error = %error,
                "failed to roll back LLM provider key after active-selection failure",
            );
        }
    }

    async fn rollback_upsert(
        &self,
        id: &str,
        previous_definition: Option<ProviderDefinition>,
        previous_key: Option<SecretString>,
        key_was_updated: bool,
    ) {
        self.rollback_provider_definition(id, previous_definition)
            .await;
        if key_was_updated {
            self.rollback_provider_key(id, previous_key).await;
        }
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
        let previous_key = if has_new_key {
            self.keys
                .read(&id)
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable)?
        } else {
            None
        };
        let stored_key_present = if has_new_key {
            previous_key.is_some()
        } else {
            self.keys
                .exists(&id)
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable)?
        };
        // Load the current DB row to determine if it's a builtin and for rollback.
        let existing = self
            .pg_provider_repo
            .get_full(&id)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let previous_definition = existing.as_ref().map(|(def, _)| def.clone());

        // Build the merged definition:
        //   - Builtin row: preserve protocol/setup/aliases from the DB definition;
        //     overlay only operator-editable fields.
        //   - Custom row or new provider: build entirely from the request.
        let mut definition = if let Some((existing_def, true)) = existing.as_ref() {
            // Builtin: start from existing DB definition (has protocol/setup intact),
            // overlay the operator-editable fields from the request.
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
            let key_present = has_new_key || stored_key_present
                || existing_def.api_key_env.as_ref().is_some_and(|env| std::env::var(env).is_ok());
            merged.api_key_required = !key_present;
            merged
        } else {
            // Custom or new provider: build from request.
            let key_present = has_new_key || stored_key_present;
            build_overlay_definition(
                &id,
                None,
                &request.adapter,
                base_url,
                model,
                key_present,
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

        // Store the key value only when a real (non-sentinel) one was supplied.
        if has_new_key && let Some(key) = request.api_key.as_ref() {
            self.keys
                .put(&id, key.clone())
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable)?;
        }

        // Persist: builtin rows go through upsert_builtin (preserves is_builtin=TRUE),
        // custom rows go through upsert.
        let is_builtin_row = existing.as_ref().is_some_and(|(_, is_builtin)| *is_builtin);
        let upsert_result = if is_builtin_row {
            self.pg_provider_repo
                .upsert_builtin(definition)
                .await
                .map(|_| true)
                .map_err(|_| LlmConfigServiceError::Unavailable)
        } else {
            self.upsert_provider_definition(definition).await
        };
        if upsert_result.is_err() {
            if has_new_key {
                self.rollback_provider_key(&id, previous_key).await;
            }
            return Err(LlmConfigServiceError::Unavailable);
        }

        if request.set_active {
            let active_result = self
                .set_provider_async(id.clone(), request.model.clone())
                .await;
            if let Err(error) = active_result {
                self.rollback_upsert(&id, previous_definition, previous_key, has_new_key)
                    .await;
                return Err(map_admin_error(error));
            }
        }

        self.refresh_running_provider().await;
        self.snapshot(caller).await
    }

    async fn delete_provider(
        &self,
        caller: WebUiAuthenticatedCaller,
        provider_id: String,
    ) -> Result<LlmConfigSnapshot, LlmConfigServiceError> {
        let id = validate_provider_id(&provider_id)?;
        // Delete via DB-backed repo.
        let removed = self.delete_provider_definition(&id).await?;
        if !removed {
            return Err(LlmConfigServiceError::NotFound);
        }
        // Best-effort: drop any stored key for the deleted provider.
        if let Err(e) = self.keys.delete(&id).await {
            tracing::debug!(provider_id = %id, error = %e, "llm config: failed to delete stored key for removed provider");
        }

        self.refresh_running_provider().await;
        self.snapshot(caller).await
    }

    async fn set_active(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: SetActiveLlmRequest,
    ) -> Result<LlmConfigSnapshot, LlmConfigServiceError> {
        let role = request.role.unwrap_or(ProviderRole::Kohai);

        // Validate provider id (allow empty string to clear the slot).
        let id = if request.provider_id.trim().is_empty() {
            String::new()
        } else {
            validate_provider_id(&request.provider_id)?
        };

        // Guard: same provider cannot occupy both roles simultaneously.
        // Embedding may coexist with Kohai or Sempai; Kohai+Sempai still conflict.
        if !id.is_empty() {
            let conflict = match role {
                ProviderRole::Kohai => {
                    // Would be Kohai — check that it is not already the Sempai.
                    // DB-authoritative read (no file fallback after Phase 8).
                    let sempai_sel = self.read_sempai_sel_from_db().await;
                    sempai_sel.is_some_and(|sel| sel.provider_id == id)
                }
                ProviderRole::Sempai => {
                    // Would be Sempai — check that it is not already the Kohai.
                    // DB-authoritative read (no file fallback after Phase 8).
                    let kohai_sel = self.read_kohai_sel_from_db().await;
                    kohai_sel.is_some_and(|sel| sel.provider_id == id)
                }
                // Embedding may coexist with any other role — no conflict check.
                ProviderRole::Embedding => false,
            };
            if conflict {
                return Err(LlmConfigServiceError::Conflict {
                    reason: "provider_already_assigned_to_other_role".into(),
                });
            }
        }

        match role {
            ProviderRole::Kohai => {
                self.set_provider_async(id, request.model)
                    .await
                    .map_err(map_admin_error)?;
                self.refresh_running_provider().await;
            }
            ProviderRole::Sempai => {
                // Persist to DB (sole write target for the Sempai slot).
                // No #[cfg] guard — postgres is required; the struct fields
                // pg_pool/pg_provider_repo are always present.
                self.save_role_to_db(
                    "llm.sempai.provider_id",
                    if id.is_empty() { "" } else { &id },
                    "llm.sempai.model",
                    request.model.as_deref().unwrap_or(""),
                )
                .await;

                // Live-swap the Sempai provider + flip the interceptor mode.
                #[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
                {
                    use brassclaw_llm::LlmProvider;
                    if let Some(swappable) = &self.sempai_swappable {
                        let new_provider: Arc<dyn LlmProvider> = if id.is_empty() {
                            Arc::new(crate::runtime::PlaceholderLlmProvider)
                        } else {
                            match self.build_sempai_provider(&id, request.model.as_deref()).await {
                                Ok(p) => p,
                                Err(e) => {
                                    tracing::debug!(
                                        error = %e,
                                        "sempai build failed; mode stays Routing"
                                    );
                                    // Fall through without swapping — the stored
                                    // selection persisted; take effect on restart.
                                    return self.snapshot(caller).await;
                                }
                            }
                        };
                        swappable.swap(new_provider);
                        if let Some(mode) = &self.interceptor_mode {
                            if id.is_empty() {
                                mode.set_routing();
                            } else {
                                mode.set_rerouting();
                            }
                        }
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
                .await;
            }
        }

        self.snapshot(caller).await
    }

    async fn test_connection(
        &self,
        _caller: WebUiAuthenticatedCaller,
        request: LlmProbeRequest,
    ) -> Result<LlmProbeResult, LlmConfigServiceError> {
        let provider = self.probe_provider(&request).await?;
        match provider.list_models().await {
            Ok(models) if !models.is_empty() => Ok(LlmProbeResult {
                ok: true,
                message: format!("connection ok — {} models available", models.len()),
            }),
            Ok(_) => Ok(LlmProbeResult {
                ok: true,
                message: "Connection successful! Provider is configured and ready to use."
                    .to_string(),
            }),
            Err(_) => Ok(LlmProbeResult {
                ok: false,
                message: "could not reach the provider with these settings".to_string(),
            }),
        }
    }

    async fn list_models(
        &self,
        _caller: WebUiAuthenticatedCaller,
        request: LlmProbeRequest,
    ) -> Result<LlmModelsResult, LlmConfigServiceError> {
        // Resolve the API key the same way probe_provider does: prefer the
        // inline key from the request; fall back to the stored key when the
        // probe targets the persisted provider endpoint (SSRF-safe).
        let stored_key_allowed = self.probe_matches_persisted_provider(&request).await?;
        let api_key: Option<String> = if let Some(key) = request.api_key.as_ref() {
            Some(key.expose_secret().to_string())
        } else if stored_key_allowed {
            self.keys
                .read(&request.provider_id)
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable)?
                .map(|s| s.expose_secret().to_string())
        } else {
            None
        };

        // Use the models module directly rather than going through a provider
        // chain — the default LlmProvider::list_models() returns empty for most
        // adapters; fetch_models_for hits the real /v1/models endpoint.
        let base_url = request.base_url.as_deref().filter(|u| !u.trim().is_empty());
        let pairs = brassclaw_llm::models::fetch_models_for(
            &request.provider_id,
            &brassclaw_llm::models::ModelFetchOptions {
                api_key: api_key.as_deref(),
                base_url,
            },
        )
        .await;

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
        tokio::spawn(async move {
            if let Err(error) = manager.complete_device_code(&start).await {
                tracing::debug!(%error, "codex device login did not complete");
                remove_codex_attempt_if_current(&attempts, &attempt_key, attempt_id).await;
                return;
            }
            if !remove_codex_attempt_if_current(&attempts, &attempt_key, attempt_id).await {
                tracing::debug!("codex login completed after a newer attempt superseded it");
                return;
            }
            if let Err(error) = write_kohai_selection_to_db(
                &codex_pool,
                &codex_tenant,
                "openai_codex",
                None,
            )
            .await
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
    use crate::db_config::{ConfigWriteContext, save_config_key};
    save_config_key(
        pool,
        tenant_id,
        "llm.default.provider_id",
        provider_id,
        ConfigWriteContext::Operator,
    )
    .await
    .map_err(|e| e.to_string())?;
    if let Some(m) = model {
        save_config_key(
            pool,
            tenant_id,
            "llm.default.model",
            m,
            ConfigWriteContext::Operator,
        )
        .await
        .map_err(|e| e.to_string())?;
    }
    Ok(())
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
/// only the operator's `base_url`/`model`, relaxing `api_key_required` when a
/// key is stored (so resolution doesn't demand the env var; the stored value is
/// injected at provider-build time). When `builtin` is `None` it's a brand-new
/// custom provider, which needs a valid `adapter`.
fn build_overlay_definition(
    id: &str,
    builtin: Option<&ProviderDefinition>,
    adapter: &str,
    base_url: Option<String>,
    model: Option<String>,
    key_present: bool,
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
        if key_present {
            def.api_key_required = false;
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
/// stored in the catalog — `api_key_required = false` so resolution succeeds
/// without an env var, and the stored value is injected at provider-build time.
fn custom_definition(
    id: &str,
    protocol: ProviderProtocol,
    base_url: Option<String>,
    default_model: String,
) -> ProviderDefinition {
    ProviderDefinition {
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

fn map_admin_error(error: crate::RebornProviderAdminError) -> LlmConfigServiceError {
    use crate::RebornProviderAdminError as E;
    match error {
        E::UnknownProvider { .. } => LlmConfigServiceError::NotFound,
        E::InvalidRequest { reason } => LlmConfigServiceError::InvalidRequest {
            field: None,
            reason,
        },
        E::LoadRegistry { .. } | E::LoadConfig { .. } => LlmConfigServiceError::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn editing_a_builtin_preserves_protocol_and_setup() {
        // openai_codex is a built-in with a dedicated protocol + OAuth setup.
        let registry = brassclaw_llm::ProviderRegistry::try_load_from_path(None).expect("registry");
        let builtin = registry.find("openai_codex").expect("openai_codex builtin");
        assert_eq!(builtin.protocol, ProviderProtocol::OpenAiCodex);
        let had_setup = builtin.setup.is_some();

        let def = build_overlay_definition(
            "openai_codex",
            Some(builtin),
            "ignored_adapter",
            None,
            Some("gpt-5.3-codex".to_string()),
            false,
            None,
        )
        .expect("overlay def");

        // Protocol + setup preserved; only the model changed.
        assert_eq!(def.protocol, ProviderProtocol::OpenAiCodex);
        assert_eq!(def.setup.is_some(), had_setup);
        assert_eq!(def.default_model, "gpt-5.3-codex");
        assert_eq!(def.id, "openai_codex");
    }

    #[test]
    fn editing_a_builtin_relaxes_key_requirement_when_key_stored() {
        let registry = brassclaw_llm::ProviderRegistry::try_load_from_path(None).expect("registry");
        let openai = registry.find("openai").expect("openai builtin");
        assert!(openai.api_key_required, "openai requires a key by default");

        let def = build_overlay_definition(
            "openai",
            Some(openai),
            "open_ai_completions",
            None,
            None,
            true, // a key is stored
            None,
        )
        .expect("overlay def");
        assert!(
            !def.api_key_required,
            "stored key means resolution must not demand the env var"
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
            false,
            Some("Acme"),
        )
        .expect("overlay def");
        assert_eq!(def.protocol, ProviderProtocol::Anthropic);
        assert_eq!(def.description, "Acme");
        assert!(!def.api_key_required);
    }

    #[test]
    fn brand_new_custom_provider_rejects_unknown_adapter() {
        let err = build_overlay_definition("acme", None, "nonsense", None, None, false, None)
            .expect_err("unknown adapter must fail");
        assert!(matches!(err, LlmConfigServiceError::InvalidRequest { .. }));
    }

}

// ─── Integration tests ─────────────────────────────────────────────────────
//
// These tests require a real Postgres instance (they call service methods that
// write to `brassclaw_llm_providers` / `brassclaw_config`).
//
// Run with:
//   cargo test -p brassclaw_reborn_composition --features integration
//
// Coverage migrated from the old file-backed unit tests (removed in V047/V048
// when `boot` and `ProviderRepo` were stripped from `RebornLlmConfigService`):
//   - upsert_provider_persists_overlay_stores_key_and_preserves_existing_key
//   - probe_override_requires_inline_key_before_using_stored_key
//   - upsert_builtin_remains_builtin_in_snapshot
//   - nearai_snapshot_exposes_api_key_as_supported_but_not_required
//   - set_active_absent_role_defaults_to_kohai
//   - set_active_sempai_role_succeeds_without_error
//   - set_active_sempai_conflict_with_kohai_is_rejected
//   - set_active_sempai_clear_succeeds_without_error
//   - set_active_embedding_role_updates_snapshot
//   - set_active_embedding_clear_removes_embedding_active
//   - set_active_embedding_may_coexist_with_kohai
//
// Deleted (tested file-only paths that no longer exist):
//   - upsert_active_failure_rolls_back_overlay_and_new_key  (was file rollback)
//   - set_active_kohai_after_sempai_without_pool_does_not_conflict  (was no-pool path)
