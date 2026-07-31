//! Composition-side implementation of the WebChat v2 LLM-config port.
//!
//! Ties together the read/set-active surface ([`RebornProviderAdmin`]), the
//! custom-provider overlay writer ([`ProviderRepo`]), the operator-scoped key
//! store ([`LlmKeyStore`]), and the live provider-reload seam
//! ([`LlmReloadTrigger`]). Everything the webui2 Inference tab needs lands here;
//! the product facade stays a thin, sanitized pass-through.
//!
//! Persistence is operator-wide and split across three surfaces, mirroring how
//! reborn already resolves an LLM at boot:
//! - custom provider definitions  → `$BRASSCLAW_REBORN_HOME/providers.json`
//! - active provider + model      → `config.toml [llm.default]`
//! - API-key **values**           → scoped secret store (never the file)
//!
//! After a successful write the running provider's inner backend is hot-swapped
//! via the reload trigger. The on-disk files are the source of truth: if reload
//! fails the change is still persisted and applies on the next restart, so the
//! operator is never left with a silently-dropped edit (the failure is logged).

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
use brassclaw_reborn_config::{LlmSlotSelection, RebornBootConfig};
use secrecy::{ExposeSecret as _, SecretString};

use crate::llm_catalog::{apply_stored_api_key, resolve_against_registry};
use crate::{LlmKeyStore, ProviderRepo, RebornProviderAdmin};

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
pub struct RebornLlmConfigService {
    boot: RebornBootConfig,
    repo: ProviderRepo,
    keys: LlmKeyStore,
    reload: Option<Arc<dyn LlmReloadTrigger>>,
    /// The runtime's NEAR AI session manager — the same instance the live
    /// provider reads its token from, so a completed login takes effect on
    /// reload. Absent when the runtime has no LLM seam wired.
    nearai_session: Option<Arc<brassclaw_llm::SessionManager>>,
    nearai_login_states: Arc<NearAiLoginStateStore>,
    codex_login_attempts: Arc<tokio::sync::Mutex<HashMap<String, CodexLoginAttempt>>>,
    /// Postgres pool for dual-writing role assignments to `brassclaw_config`
    /// and for reading role selections (Sempai/Embedding) from DB.
    ///
    /// When present, `set_active(Sempai/Embedding)` writes to
    /// `brassclaw_config` in addition to the local JSON file so the factory's
    /// `resolve_pg_embedding_provider` picks up the selection on the next
    /// restart. `build_snapshot` also reads role selections from DB when the
    /// pool is present, so the WebUI always reflects the DB-authoritative state.
    #[cfg(feature = "postgres")]
    pg_pool: Option<Arc<brassclaw_pg::PgPool>>,
    /// DB-backed provider repo for upsert/delete operations.
    ///
    /// When present, `upsert_provider` and `delete_provider` write to
    /// `brassclaw_llm_providers` instead of `providers.json`. The file-based
    /// `repo` is retained as fallback for non-postgres configurations.
    #[cfg(feature = "postgres")]
    pg_provider_repo: Option<Arc<crate::pg_provider_repo::PgProviderRepo>>,
    /// Tenant ID for DB operations.
    #[cfg(feature = "postgres")]
    db_tenant_id: String,
    /// Live hot-swap wrapper for the Sempai provider.  When `Some`,
    /// `set_active(Sempai, id)` swaps the inner provider so the interceptor
    /// immediately starts using the new model without a restart.
    #[cfg(feature = "root-llm-provider")]
    sempai_swappable: Option<Arc<brassclaw_llm::SwappableLlmProvider>>,
    /// Shared interceptor mode flag.  When `Some`, `set_active(Sempai, id)`
    /// flips this to `Rerouting`; clearing the slot flips it back to `Routing`.
    #[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
    interceptor_mode: Option<brassclaw_interceptor::SharedInterceptorMode>,
}

impl RebornLlmConfigService {
    pub fn new(boot: RebornBootConfig, keys: LlmKeyStore) -> Self {
        let repo = ProviderRepo::new(boot.home().path().join("providers.json"));
        Self {
            boot,
            repo,
            keys,
            reload: None,
            nearai_session: None,
            nearai_login_states: Arc::new(NearAiLoginStateStore::new()),
            codex_login_attempts: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            #[cfg(feature = "postgres")]
            pg_pool: None,
            #[cfg(feature = "postgres")]
            pg_provider_repo: None,
            #[cfg(feature = "postgres")]
            db_tenant_id: "default".to_string(),
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

    /// Attach a Postgres pool for dual-writing role assignments to `brassclaw_config`
    /// and reading role selections from DB.
    ///
    /// When set, `set_active(Sempai/Embedding)` writes to `brassclaw_config`
    /// and `build_snapshot` reads role selections from DB.
    #[cfg(feature = "postgres")]
    pub fn with_pg_pool(mut self, pool: Arc<brassclaw_pg::PgPool>) -> Self {
        self.pg_pool = Some(pool);
        self
    }

    /// Attach the DB-backed provider repo and tenant ID.
    ///
    /// When set, `upsert_provider` and `delete_provider` write to
    /// `brassclaw_llm_providers` instead of `providers.json`.
    #[cfg(feature = "postgres")]
    pub fn with_pg_provider_repo(
        mut self,
        repo: Arc<crate::pg_provider_repo::PgProviderRepo>,
        tenant_id: impl Into<String>,
    ) -> Self {
        self.pg_provider_repo = Some(repo);
        self.db_tenant_id = tenant_id.into();
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

    /// Attach the Sempai live-swap wrapper from the runtime.  When set,
    /// `set_active(Sempai, id)` atomically swaps the inner provider without
    /// a restart.
    #[cfg(feature = "root-llm-provider")]
    pub fn with_sempai_swappable(
        mut self,
        swappable: Arc<brassclaw_llm::SwappableLlmProvider>,
    ) -> Self {
        self.sempai_swappable = Some(swappable);
        self
    }

    /// Attach the shared interceptor mode flag from the runtime.  When set,
    /// `set_active(Sempai, id)` flips the mode to `Rerouting`; clearing
    /// the Sempai slot flips it back to `Routing`.
    #[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
    pub fn with_interceptor_mode(
        mut self,
        mode: brassclaw_interceptor::SharedInterceptorMode,
    ) -> Self {
        self.interceptor_mode = Some(mode);
        self
    }

    fn admin(&self) -> RebornProviderAdmin {
        RebornProviderAdmin::new(self.boot.clone())
    }

    /// Dual-write a role's provider_id + model to `brassclaw_config`.
    ///
    /// Called after the file write succeeds.  Errors are logged at `debug!`
    /// level and swallowed — the file is the authoritative source and a
    /// DB write failure must not undo an already-committed file write.
    ///
    /// When `provider_id` is empty the key is deleted so a cleared slot
    /// does not leave a stale row in the DB.
    #[cfg(feature = "postgres")]
    async fn save_role_to_db(
        &self,
        provider_id_key: &str,
        provider_id_value: &str,
        model_key: &str,
        model_value: &str,
    ) {
        use crate::db_config::{ConfigWriteContext, delete_config_key, save_config_key};

        let Some(pool) = self.pg_pool.as_ref() else {
            return;
        };
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
            tracing::debug!(key = provider_id_key, error = %e,
                            "role DB write failed (file write already succeeded)");
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
            tracing::debug!(key = model_key, error = %e,
                            "role DB write failed (file write already succeeded)");
        }
    }

    /// Read a role's `LlmActiveSelection` from `brassclaw_config` (DB) when a pool
    /// is available, falling back to the legacy JSON file when not.
    ///
    /// This is the read counterpart to `save_role_to_db`. It ensures that
    /// `build_snapshot` and `set_active` conflict checks reflect the DB-authoritative
    /// role assignments rather than potentially-stale JSON files.
    #[cfg(feature = "postgres")]
    async fn read_role_sel_from_db_or_file(
        &self,
        provider_id_key: &str,
        model_key: &str,
        file_path: std::path::PathBuf,
    ) -> Option<LlmActiveSelection> {
        use crate::db_config::list_config_keys;

        if let Some(pool) = self.pg_pool.as_ref() {
            let tenant_id = &self.db_tenant_id;
            // Best-effort: if DB read fails, fall back to file.
            if let Ok(rows) = list_config_keys(pool, tenant_id).await {
                let kv: std::collections::HashMap<String, String> = rows.into_iter().collect();
                let provider_id = kv.get(provider_id_key)?.to_string();
                if provider_id.is_empty() {
                    return None;
                }
                let model = kv.get(model_key).cloned().filter(|s| !s.is_empty());
                return Some(LlmActiveSelection { provider_id, model });
            }
        }
        // Fallback: read from the legacy file (pre-postgres or non-postgres builds).
        read_role_selection(file_path)
    }

    /// Non-postgres build: always falls back to the file.
    #[cfg(not(feature = "postgres"))]
    async fn read_role_sel_from_db_or_file(
        &self,
        _provider_id_key: &str,
        _model_key: &str,
        file_path: std::path::PathBuf,
    ) -> Option<LlmActiveSelection> {
        read_role_selection(file_path)
    }

    /// Read the Sempai role selection from DB only (no file fallback after Phase 8).
    #[cfg(feature = "postgres")]
    async fn read_sempai_sel_from_db(&self) -> Option<LlmActiveSelection> {
        use crate::db_config::list_config_keys;
        let pool = self.pg_pool.as_ref()?;
        let rows = list_config_keys(pool, &self.db_tenant_id).await.ok()?;
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

    /// Non-postgres build: Sempai is not supported.
    #[cfg(not(feature = "postgres"))]
    async fn read_sempai_sel_from_db(&self) -> Option<LlmActiveSelection> {
        None
    }

    /// Read the Kohai (default) role selection from DB only.
    ///
    /// Keys: `llm.default.provider_id` / `llm.default.model` in `brassclaw_config`.
    #[cfg(feature = "postgres")]
    async fn read_kohai_sel_from_db(&self) -> Option<LlmActiveSelection> {
        use crate::db_config::list_config_keys;
        let pool = self.pg_pool.as_ref()?;
        let rows = list_config_keys(pool, &self.db_tenant_id).await.ok()?;
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

    /// Non-postgres build: Kohai DB selection not supported.
    #[cfg(not(feature = "postgres"))]
    async fn read_kohai_sel_from_db(&self) -> Option<LlmActiveSelection> {
        None
    }

    /// Read the Embedding role selection from DB only (no file fallback).
    ///
    /// Keys: `embedding.provider_id` / `embedding.model` in `brassclaw_config`.
    #[cfg(feature = "postgres")]
    async fn read_embedding_sel_from_db(&self) -> Option<LlmActiveSelection> {
        use crate::db_config::list_config_keys;
        let pool = self.pg_pool.as_ref()?;
        let rows = list_config_keys(pool, &self.db_tenant_id).await.ok()?;
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

    /// Non-postgres build: embedding DB selection not supported.
    #[cfg(not(feature = "postgres"))]
    async fn read_embedding_sel_from_db(&self) -> Option<LlmActiveSelection> {
        None
    }

    /// Persist-then-reload: the file write already happened; refresh the
    /// running provider. A reload failure is logged, not fatal — the on-disk
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
        if let Ok(list) = self.admin_list_async().await
            && let Some(active) = list.providers.iter().find(|p| p.active)
        {
            reload.on_provider_changed(&active.id).await;
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

        // Try DB-backed repo first, then built-in registry (same pattern as
        // `probe_matches_persisted_provider`).
        let definition = self
            .load_provider_by_id(provider_id)
            .await
            .map_err(|e| format!("provider load: {e}"))?
            .or_else(|| {
                brassclaw_llm::ProviderRegistry::try_load_from_path(None)
                    .ok()
                    .and_then(|r| r.find(provider_id).cloned())
            })
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
        let list = self.admin_list_async().await.map_err(map_admin_error)?;
        let builtin_registry = brassclaw_llm::ProviderRegistry::try_load_from_path(None)
            .map_err(|_| LlmConfigServiceError::Unavailable)?;

        // Read the persisted Sempai selection (DB only after Phase 8).
        // Embedding retains a file fallback for non-postgres builds.
        let sempai_sel = self.read_sempai_sel_from_db().await;
        let embedding_sel = self
            .read_role_sel_from_db_or_file(
                "embedding.provider_id",
                "embedding.model",
                self.boot.home().embedding_provider_file_path(),
            )
            .await;

        // Read the DB-stored Kohai selection so that custom (non-builtin)
        // providers that were set active via `set_active` can be correctly
        // marked as active in the snapshot.  Builtins resolve `active` via
        // `config.toml` through `admin_list_async`; custom providers have no
        // entry there and need this DB read.
        #[cfg(feature = "postgres")]
        let kohai_sel_from_db: Option<LlmActiveSelection> = {
            use crate::db_config::list_config_keys;
            if let Some(pool) = self.pg_pool.as_ref() {
                list_config_keys(pool, &self.db_tenant_id)
                    .await
                    .ok()
                    .and_then(|rows| {
                        let kv: std::collections::HashMap<String, String> =
                            rows.into_iter().collect();
                        let provider_id = kv.get("llm.default.provider_id")?.to_string();
                        if provider_id.is_empty() {
                            return None;
                        }
                        let model = kv
                            .get("llm.default.model")
                            .cloned()
                            .filter(|s| !s.is_empty());
                        Some(LlmActiveSelection { provider_id, model })
                    })
            } else {
                None
            }
        };
        #[cfg(not(feature = "postgres"))]
        let kohai_sel_from_db: Option<LlmActiveSelection> = None;

        // Load custom providers from the DB-backed repo (when available).
        // These are user-defined providers that are never in the builtin registry.
        #[cfg(feature = "postgres")]
        let db_custom_defs: Vec<brassclaw_llm::registry::ProviderDefinition> = {
            if let Some(pg_repo) = self.pg_provider_repo.as_ref() {
                pg_repo
                    .load()
                    .await
                    .map_err(|_| LlmConfigServiceError::Unavailable)?
            } else {
                Vec::new()
            }
        };
        #[cfg(not(feature = "postgres"))]
        let db_custom_defs: Vec<brassclaw_llm::registry::ProviderDefinition> = Vec::new();

        // Collect the ids of builtin providers so we can skip DB entries that
        // shadow a builtin (those are overlays handled by the builtin path).
        let builtin_ids: std::collections::HashSet<String> = list
            .providers
            .iter()
            .map(|p| p.id.clone())
            .collect();

        let capacity = list.providers.len() + db_custom_defs.len();
        let mut providers = Vec::with_capacity(capacity);
        let mut active = None;

        // ── Builtin providers (from admin_list_async / config.toml) ─────────
        for info in list.providers {
            let stored_key_set = self
                .keys
                .exists(&info.id)
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable)?;
            let builtin = builtin_registry.find(&info.id).is_some();
            let metadata = info.metadata;
            let env_key_set = metadata.as_ref().is_some_and(metadata_env_key_set);
            let api_key_set = stored_key_set || env_key_set;

            // For builtins, `info.active` is set by config.toml.  Also check
            // the DB-stored kohai selection for the (edge) case where a builtin
            // was activated via the WebUI after a custom provider was replaced.
            let is_kohai = info.active
                || kohai_sel_from_db
                    .as_ref()
                    .is_some_and(|sel| sel.provider_id == info.id);
            let active_model = if is_kohai {
                info.active_model
                    .clone()
                    .or_else(|| kohai_sel_from_db.as_ref().and_then(|sel| sel.model.clone()))
            } else {
                None
            };
            if is_kohai && active.is_none() {
                active = Some(LlmActiveSelection {
                    provider_id: info.id.clone(),
                    model: active_model.clone(),
                });
            }
            let builtin_def = builtin_registry.find(&info.id);
            let definition_budget =
                builtin_def
                    .and_then(|def| def.token_budget.as_ref())
                    .map(|b| ProviderTokenBudgetView {
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
            let definition_context_window = builtin_def.and_then(|def| def.context_window_tokens);
            let is_sempai = sempai_sel
                .as_ref()
                .is_some_and(|s| s.provider_id == info.id);
            let is_embedding = embedding_sel
                .as_ref()
                .is_some_and(|s| s.provider_id == info.id);
            providers.push(LlmProviderView {
                id: info.id,
                description: info.description,
                adapter: metadata
                    .as_ref()
                    .map(|meta| meta.protocol.clone())
                    .unwrap_or_default(),
                default_model: info.default_model,
                base_url: metadata.as_ref().and_then(|meta| meta.base_url.clone()),
                builtin,
                active: is_kohai,
                active_model,
                api_key_required: metadata
                    .as_ref()
                    .map(|meta| meta.api_key_required)
                    .unwrap_or(false),
                accepts_api_key: metadata
                    .as_ref()
                    .map(|meta| meta.accepts_api_key)
                    .unwrap_or(false),
                api_key_set,
                can_list_models: metadata
                    .as_ref()
                    .map(|meta| meta.can_list_models)
                    .unwrap_or(false),
                token_budget: definition_budget,
                context_window_tokens: definition_context_window,
                is_kohai,
                is_sempai,
                is_embedding,
            });
        }

        // ── Custom (DB-backed) providers ─────────────────────────────────────
        // Skip any that shadow a builtin — those are overlays already included
        // via the builtin path above.
        for def in db_custom_defs {
            if builtin_ids.contains(&def.id) {
                continue;
            }
            let stored_key_set = self
                .keys
                .exists(&def.id)
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable)?;
            let is_kohai = kohai_sel_from_db
                .as_ref()
                .is_some_and(|sel| sel.provider_id == def.id);
            let active_model = if is_kohai {
                kohai_sel_from_db.as_ref().and_then(|sel| sel.model.clone())
            } else {
                None
            };
            if is_kohai && active.is_none() {
                active = Some(LlmActiveSelection {
                    provider_id: def.id.clone(),
                    model: active_model.clone(),
                });
            }
            let is_sempai = sempai_sel
                .as_ref()
                .is_some_and(|s| s.provider_id == def.id);
            let is_embedding = embedding_sel
                .as_ref()
                .is_some_and(|s| s.provider_id == def.id);
            // Convert the ProviderProtocol enum to its wire string.
            let adapter = serde_json::to_value(def.protocol)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default();
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
                builtin: false,
                active: is_kohai,
                active_model,
                api_key_required: def.api_key_required,
                accepts_api_key: def.api_key_env.is_some(),
                api_key_set: stored_key_set,
                can_list_models: false,
                token_budget: None,
                context_window_tokens: def.context_window_tokens,
                is_kohai,
                is_sempai,
                is_embedding,
            });
        }

        Ok(LlmConfigSnapshot {
            providers,
            active: active.clone(),
            // `active` and `kohai_active` are always kept in sync so that old
            // clients reading `active` observe the Kohai selection unchanged.
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
        // Try DB-backed repo first (single provider lookup, no full registry scan).
        let definition = self.load_provider_by_id(&request.provider_id).await?;

        let definition = if let Some(d) = definition {
            d
        } else {
            // Fall back to built-in registry (compiled-in providers are not stored in DB).
            let builtin_registry = brassclaw_llm::ProviderRegistry::try_load_from_path(None)
                .map_err(|_| LlmConfigServiceError::Unavailable)?;
            let Some(d) = builtin_registry.find(&request.provider_id).cloned() else {
                return Ok(false);
            };
            d
        };

        let Some(protocol) = parse_adapter(&request.adapter) else {
            return Ok(false);
        };
        Ok(protocol == definition.protocol
            && normalized_endpoint(request.base_url.as_deref())
                == normalized_endpoint(definition.default_base_url.as_deref()))
    }

    async fn admin_list_async(
        &self,
    ) -> Result<crate::RebornProviderList, crate::RebornProviderAdminError> {
        let admin = self.admin();
        tokio::task::spawn_blocking(move || admin.list(None, true))
            .await
            .map_err(|error| crate::RebornProviderAdminError::InvalidRequest {
                reason: format!("provider-admin task failed: {error}"),
            })?
    }

    async fn set_provider_async(
        &self,
        id: String,
        model: Option<String>,
    ) -> Result<(), crate::RebornProviderAdminError> {
        #[cfg(feature = "postgres")]
        {
            use crate::db_config::{ConfigWriteContext, save_config_key};
            if let Some(pool) = self.pg_pool.as_ref() {
                let tenant = &self.db_tenant_id;
                if let Err(e) = save_config_key(
                    pool,
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
                    pool,
                    tenant,
                    "llm.default.model",
                    model_val,
                    ConfigWriteContext::Operator,
                )
                .await
                {
                    tracing::debug!(error = %e, "set_provider_async: model DB write failed");
                }
                return Ok(());
            }
        }
        // Non-postgres build: no file-based write path remains after Phase 8.
        // The selection will apply on next restart when config is migrated.
        let _ = (id, model);
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
    // Provider repo abstraction helpers (DB or file, depending on wiring)
    // ------------------------------------------------------------------

    /// Load a single provider definition by id (for rollback / previous-state tracking).
    ///
    /// Uses the DB-backed repo when wired; falls back to the file-based repo.
    async fn load_provider_by_id(
        &self,
        id: &str,
    ) -> Result<Option<ProviderDefinition>, LlmConfigServiceError> {
        #[cfg(feature = "postgres")]
        if let Some(pg_repo) = self.pg_provider_repo.as_ref() {
            return pg_repo
                .get(id)
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable);
        }
        // File-based fallback.
        let overlay = self
            .repo
            .load_async()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        Ok(overlay.into_iter().find(|d| d.id.eq_ignore_ascii_case(id)))
    }

    /// Upsert a provider definition into the DB-backed repo (or file fallback).
    async fn upsert_provider_definition(
        &self,
        definition: ProviderDefinition,
    ) -> Result<bool, LlmConfigServiceError> {
        #[cfg(feature = "postgres")]
        if let Some(pg_repo) = self.pg_provider_repo.as_ref() {
            return pg_repo
                .upsert(definition)
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable);
        }
        self.repo
            .upsert_async(definition)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)
    }

    /// Delete a provider definition from the DB-backed repo (or file fallback).
    ///
    /// Returns `true` if the provider was found and deleted.
    /// Returns `Err(CannotDeleteBuiltin)` if the provider is a builtin.
    async fn delete_provider_definition(&self, id: &str) -> Result<bool, LlmConfigServiceError> {
        #[cfg(feature = "postgres")]
        if let Some(pg_repo) = self.pg_provider_repo.as_ref() {
            return pg_repo.delete(id).await.map_err(|e| {
                if matches!(e, crate::pg_provider_repo::PgProviderRepoError::CannotDeleteBuiltin) {
                    LlmConfigServiceError::CannotDeleteBuiltin
                } else {
                    LlmConfigServiceError::Unavailable
                }
            });
        }
        self.repo
            .delete_async(id)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)
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
        // Load the current overlay to find the previous definition for rollback.
        // Use the DB-backed repo when available; fall back to the file-based repo.
        let previous_definition = self.load_provider_by_id(&id).await?;

        // Editing a built-in must PRESERVE its compiled-in definition (protocol,
        // setup hints, env-var names) and overlay only what the operator
        // changed. Writing a fresh generic definition would strip OAuth/setup
        // from providers like openai_codex, gemini_oauth, nearai, and bedrock.
        let builtin_registry = brassclaw_llm::ProviderRegistry::try_load_from_path(None)
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let builtin = builtin_registry.find(&id);
        let key_present =
            has_new_key || stored_key_present || builtin.is_some_and(definition_env_key_set);
        let mut definition = build_overlay_definition(
            &id,
            builtin,
            &request.adapter,
            base_url,
            model,
            key_present,
            request.name.as_deref(),
        )?;

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

        // Upsert via DB-backed repo (when available) or file-based repo.
        if self.upsert_provider_definition(definition).await.is_err() {
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
        // Delete via DB-backed repo (when available) or file-based repo.
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
                    self.admin_list_async()
                        .await
                        .map_err(map_admin_error)?
                        .providers
                        .into_iter()
                        .any(|p| p.active && p.id == id)
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
                #[cfg(feature = "postgres")]
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
                let embedding_path = self.boot.home().embedding_provider_file_path();
                write_role_selection(
                    embedding_path,
                    if id.is_empty() {
                        None
                    } else {
                        Some(LlmActiveSelection {
                            provider_id: id.clone(),
                            model: request.model.clone(),
                        })
                    },
                )
                .map_err(|_| LlmConfigServiceError::Unavailable)?;
                // Dual-write to brassclaw_config so the production factory reads
                // `embedding.provider_id` on restart (§3, §4.2).
                #[cfg(feature = "postgres")]
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
        // the snapshot until openai_codex is active. The on-disk session file is
        // the source of truth, so a reload failure still applies on restart.
        let reload = self.reload.clone();
        let attempts = Arc::clone(&self.codex_login_attempts);
        #[cfg(feature = "postgres")]
        let codex_pool = self.pg_pool.clone();
        #[cfg(feature = "postgres")]
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
            #[cfg(feature = "postgres")]
            if let Err(error) = write_kohai_selection_to_db(
                codex_pool.as_deref(),
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
        #[cfg(feature = "postgres")]
        write_kohai_selection_to_db(self.pg_pool.as_deref(), &self.db_tenant_id, "nearai", None)
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
/// `self` access. Falls back silently when no pool is provided or the
/// feature flag is disabled (non-postgres builds).
#[cfg(feature = "postgres")]
pub(crate) async fn write_kohai_selection_to_db(
    pool: Option<&brassclaw_pg::PgPool>,
    tenant_id: &str,
    provider_id: &str,
    model: Option<&str>,
) -> Result<(), String> {
    use crate::db_config::{ConfigWriteContext, save_config_key};
    let Some(pool) = pool else {
        return Ok(());
    };
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

fn metadata_env_key_set(metadata: &crate::RebornProviderMetadata) -> bool {
    metadata.api_key_env.as_deref().is_some_and(env_var_present)
}

fn definition_env_key_set(definition: &ProviderDefinition) -> bool {
    definition
        .api_key_env
        .as_deref()
        .is_some_and(env_var_present)
}

fn env_var_present(name: &str) -> bool {
    std::env::var_os(name).is_some()
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

/// Read a persisted provider role selection from a JSON file.
///
/// Returns `None` if the file is absent, empty, or malformed. This is a
/// synchronous disk read — callers that need async must wrap with
/// `spawn_blocking`.
fn read_role_selection(path: std::path::PathBuf) -> Option<LlmActiveSelection> {
    let bytes = std::fs::read(&path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Write (or clear) a persisted provider role selection file.
///
/// Passing `None` removes the file so that the absent-file case correctly
/// means "role not configured". Write errors are propagated to the caller.
fn write_role_selection(
    path: std::path::PathBuf,
    selection: Option<LlmActiveSelection>,
) -> std::io::Result<()> {
    match selection {
        None => {
            // Remove the file; ignore "not found" since the goal is no file.
            match std::fs::remove_file(&path) {
                Ok(()) | Err(_) => Ok(()),
            }
        }
        Some(sel) => {
            // Ensure the parent directory exists (create on first use).
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let json = serde_json::to_vec(&sel)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
            std::fs::write(&path, json)
        }
    }
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
    use brassclaw_host_api::{AgentId, ProjectId, TenantId, UserId};
    use brassclaw_llm::ProviderRole;
    use brassclaw_reborn_config::RebornHome;
    use brassclaw_secrets::InMemorySecretStore;

    fn boot_for_home(reborn_home: &std::path::Path) -> RebornBootConfig {
        let home = RebornHome::resolve_from_env_parts(
            Some(reborn_home.as_os_str().to_os_string()),
            None,
            None,
        )
        .expect("valid reborn home");
        RebornBootConfig::new(home)
    }

    fn key_store() -> LlmKeyStore {
        LlmKeyStore::new(Arc::new(InMemorySecretStore::new()))
    }

    fn caller() -> WebUiAuthenticatedCaller {
        WebUiAuthenticatedCaller::new(
            TenantId::new("tenant-alpha").expect("tenant"),
            UserId::new("user-alpha").expect("user"),
            Some(AgentId::new("agent-alpha").expect("agent")),
            Some(ProjectId::new("project-alpha").expect("project")),
        )
    }

    fn upsert_request(
        id: &str,
        api_key: Option<&str>,
        set_active: bool,
    ) -> UpsertLlmProviderRequest {
        UpsertLlmProviderRequest {
            id: id.to_string(),
            name: Some("Acme".to_string()),
            adapter: "open_ai_completions".to_string(),
            base_url: Some("https://api.acme.test/v1".to_string()),
            default_model: Some("acme-1".to_string()),
            api_key: api_key.map(SecretString::from),
            set_active,
            model: Some("acme-1".to_string()),
            token_budget: None,
        }
    }

    fn probe_request(id: &str, base_url: &str, api_key: Option<&str>) -> LlmProbeRequest {
        LlmProbeRequest {
            provider_id: id.to_string(),
            adapter: "open_ai_completions".to_string(),
            base_url: Some(base_url.to_string()),
            model: Some("acme-1".to_string()),
            api_key: api_key.map(SecretString::from),
        }
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

    #[tokio::test]
    async fn upsert_provider_persists_overlay_stores_key_and_preserves_existing_key() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reborn_home = temp.path().join("reborn-home");
        let boot = boot_for_home(&reborn_home);
        let keys = key_store();
        let service = RebornLlmConfigService::new(boot.clone(), keys.clone());

        let snapshot = service
            .upsert_provider(caller(), upsert_request("acme", Some("sk-original"), true))
            .await
            .expect("upsert with key");

        let acme = snapshot
            .providers
            .iter()
            .find(|provider| provider.id == "acme")
            .expect("acme provider in snapshot");
        assert!(!acme.builtin);
        assert!(acme.api_key_set);
        assert_eq!(snapshot.active.expect("active").provider_id, "acme");
        let overlay = ProviderRepo::new(boot.home().path().join("providers.json"))
            .load()
            .expect("load overlay");
        assert_eq!(
            overlay
                .iter()
                .filter(|provider| provider.id == "acme")
                .count(),
            1
        );
        assert_eq!(
            keys.read("acme")
                .await
                .expect("read key")
                .expect("stored key")
                .expose_secret(),
            "sk-original"
        );

        service
            .upsert_provider(
                caller(),
                upsert_request("acme", Some("\u{2022}\u{2022}\u{2022}"), false),
            )
            .await
            .expect("masked-key upsert");

        assert_eq!(
            keys.read("acme")
                .await
                .expect("read key")
                .expect("stored key")
                .expose_secret(),
            "sk-original",
            "masked sentinel must preserve the existing stored key"
        );
    }

    #[tokio::test]
    async fn probe_override_requires_inline_key_before_using_stored_key() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reborn_home = temp.path().join("reborn-home");
        let boot = boot_for_home(&reborn_home);
        let keys = key_store();
        let service = RebornLlmConfigService::new(boot, keys);

        service
            .upsert_provider(
                caller(),
                upsert_request("acme", Some("sk-stored-secret"), false),
            )
            .await
            .expect("persist provider and stored key");

        let error = service
            .list_models(
                caller(),
                probe_request("acme", "https://attacker.example.test/v1", None),
            )
            .await
            .expect_err("overridden endpoint requires an inline key");

        assert!(
            matches!(
                error,
                LlmConfigServiceError::InvalidRequest {
                    field: Some(ref field),
                    ref reason,
                } if field == "api_key" && reason.contains("overridden provider endpoint")
            ),
            "stored operator keys must not be applied to caller-controlled probe endpoints"
        );
    }

    #[tokio::test]
    async fn upsert_builtin_remains_builtin_in_snapshot() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reborn_home = temp.path().join("reborn-home");
        let boot = boot_for_home(&reborn_home);
        let service = RebornLlmConfigService::new(boot, key_store());

        let snapshot = service
            .upsert_provider(caller(), upsert_request("openai", Some("sk-openai"), false))
            .await
            .expect("upsert builtin");

        let openai = snapshot
            .providers
            .iter()
            .find(|provider| provider.id == "openai")
            .expect("openai provider in snapshot");
        assert!(
            openai.builtin,
            "overlay edits must not make built-ins custom"
        );
        assert!(openai.api_key_set);
    }

    #[tokio::test]
    async fn nearai_snapshot_exposes_api_key_as_supported_but_not_required() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reborn_home = temp.path().join("reborn-home");
        let boot = boot_for_home(&reborn_home);
        let service = RebornLlmConfigService::new(boot, key_store());

        let snapshot = service.snapshot(caller()).await.expect("snapshot");
        let nearai = snapshot
            .providers
            .iter()
            .find(|provider| provider.id == "nearai")
            .expect("nearai provider in snapshot");

        assert!(nearai.builtin);
        assert!(
            nearai.accepts_api_key,
            "NEAR AI supports API-key auth in addition to session-token login"
        );
        assert!(
            !nearai.api_key_required,
            "NEAR AI session-token login means API key is not the only setup path"
        );
    }

    #[tokio::test]
    async fn upsert_active_failure_rolls_back_overlay_and_new_key() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reborn_home = temp.path().join("reborn-home");
        std::fs::create_dir_all(reborn_home.join("config.toml")).expect("mkdir config path");
        let boot = boot_for_home(&reborn_home);
        let keys = key_store();
        let service = RebornLlmConfigService::new(boot.clone(), keys.clone());

        let error = service
            .upsert_provider(caller(), upsert_request("acme", Some("sk-rollback"), true))
            .await
            .expect_err("config write must fail");

        assert!(matches!(error, LlmConfigServiceError::Unavailable));
        let overlay = ProviderRepo::new(boot.home().path().join("providers.json"))
            .load()
            .expect("load overlay");
        assert!(
            overlay.is_empty(),
            "overlay must roll back when active selection fails"
        );
        assert!(
            !keys.exists("acme").await.expect("key exists check"),
            "new key must roll back when active selection fails"
        );
    }

    // ── Step 4: role-aware set_active tests ─────────────────────────────────

    fn set_active_request(provider_id: &str, role: Option<ProviderRole>) -> SetActiveLlmRequest {
        SetActiveLlmRequest {
            provider_id: provider_id.to_string(),
            model: None,
            role,
        }
    }

    #[tokio::test]
    async fn set_active_absent_role_defaults_to_kohai() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reborn_home = temp.path().join("reborn-home");
        let boot = boot_for_home(&reborn_home);
        let service = RebornLlmConfigService::new(boot.clone(), key_store());

        // Register a custom provider so set_provider_async can succeed.
        service
            .upsert_provider(caller(), upsert_request("acme", None, false))
            .await
            .expect("upsert");

        let snapshot = service
            .set_active(caller(), set_active_request("acme", None))
            .await
            .expect("set_active without role");

        assert_eq!(
            snapshot
                .kohai_active
                .as_ref()
                .map(|s| s.provider_id.as_str()),
            Some("acme"),
            "absent role must default to Kohai"
        );
        assert!(snapshot.sempai_active.is_none());
        assert_eq!(
            snapshot.active.as_ref().map(|s| s.provider_id.as_str()),
            Some("acme"),
            "legacy `active` field must stay in sync with Kohai"
        );
    }

    // Note: set_active_sempai tests require a postgres pool to observe DB writes.
    // Without a pool, save_role_to_db is a no-op, so sempai_active remains None
    // in non-postgres test builds. Integration-level tests cover the DB path.
    #[tokio::test]
    async fn set_active_sempai_role_succeeds_without_error() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reborn_home = temp.path().join("reborn-home");
        std::fs::create_dir_all(&reborn_home).expect("mkdir");
        let service = RebornLlmConfigService::new(boot_for_home(&reborn_home), key_store());

        // Without a postgres pool the DB write is a no-op, but the call must not error.
        service
            .set_active(
                caller(),
                set_active_request("ibm_bob_inference", Some(ProviderRole::Sempai)),
            )
            .await
            .expect("set_active Sempai must not error");
    }

    #[tokio::test]
    async fn set_active_sempai_conflict_with_kohai_is_rejected() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reborn_home = temp.path().join("reborn-home");
        let boot = boot_for_home(&reborn_home);
        let service = RebornLlmConfigService::new(boot.clone(), key_store());

        // Make acme the active Kohai provider.
        service
            .upsert_provider(caller(), upsert_request("acme", None, true))
            .await
            .expect("upsert as kohai");

        // Trying to assign the same provider as Sempai must fail.
        let err = service
            .set_active(
                caller(),
                set_active_request("acme", Some(ProviderRole::Sempai)),
            )
            .await
            .expect_err("conflict must be rejected");

        assert!(
            matches!(err, LlmConfigServiceError::Conflict { .. }),
            "expected Conflict, got {err:?}"
        );
    }

    // Note: Kohai+Sempai conflict detection requires a postgres pool.
    // Without a pool, read_sempai_sel_from_db returns None so no conflict is raised.
    // Integration-level tests cover the DB-backed conflict check.
    #[tokio::test]
    async fn set_active_kohai_after_sempai_without_pool_does_not_conflict() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reborn_home = temp.path().join("reborn-home");
        std::fs::create_dir_all(&reborn_home).expect("mkdir");
        let service = RebornLlmConfigService::new(boot_for_home(&reborn_home), key_store());

        // First assign ibm_bob_inference as Sempai (no-op DB write without pool).
        service
            .set_active(
                caller(),
                set_active_request("ibm_bob_inference", Some(ProviderRole::Sempai)),
            )
            .await
            .expect("set sempai");

        // Without a pool, read_sempai_sel_from_db returns None so no conflict is raised.
        // The call must succeed (not error).
        service
            .set_active(
                caller(),
                set_active_request("ibm_bob_inference", Some(ProviderRole::Kohai)),
            )
            .await
            .expect("set kohai without pool must not conflict");
    }

    #[tokio::test]
    async fn set_active_sempai_clear_succeeds_without_error() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reborn_home = temp.path().join("reborn-home");
        std::fs::create_dir_all(&reborn_home).expect("mkdir");
        let service = RebornLlmConfigService::new(boot_for_home(&reborn_home), key_store());

        // Set Sempai then clear it with empty provider_id. Both must succeed.
        service
            .set_active(
                caller(),
                set_active_request("ibm_bob_inference", Some(ProviderRole::Sempai)),
            )
            .await
            .expect("set sempai");

        service
            .set_active(
                caller(),
                SetActiveLlmRequest {
                    provider_id: String::new(),
                    model: None,
                    role: Some(ProviderRole::Sempai),
                },
            )
            .await
            .expect("clear sempai must not error");
    }

    #[tokio::test]
    async fn set_active_embedding_role_writes_file_and_updates_snapshot() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reborn_home = temp.path().join("reborn-home");
        std::fs::create_dir_all(&reborn_home).expect("mkdir");
        let boot = boot_for_home(&reborn_home);
        let service = RebornLlmConfigService::new(boot.clone(), key_store());

        let snapshot = service
            .set_active(
                caller(),
                set_active_request("ibm_bob_inference", Some(ProviderRole::Embedding)),
            )
            .await
            .expect("set_active Embedding");

        assert_eq!(
            snapshot
                .embedding_active
                .as_ref()
                .map(|s| s.provider_id.as_str()),
            Some("ibm_bob_inference"),
            "Embedding selection must appear in embedding_active"
        );
        // Embedding file must exist on disk.
        let path = boot.home().embedding_provider_file_path();
        assert!(path.exists(), "embedding_provider.json must be written");
        let sel = read_role_selection(path).expect("readable");
        assert_eq!(sel.provider_id, "ibm_bob_inference");
    }

    #[tokio::test]
    async fn set_active_embedding_clear_removes_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reborn_home = temp.path().join("reborn-home");
        std::fs::create_dir_all(&reborn_home).expect("mkdir");
        let boot = boot_for_home(&reborn_home);
        let service = RebornLlmConfigService::new(boot.clone(), key_store());

        // Set Embedding then clear it with empty provider_id.
        service
            .set_active(
                caller(),
                set_active_request("ibm_bob_inference", Some(ProviderRole::Embedding)),
            )
            .await
            .expect("set embedding");

        let snapshot = service
            .set_active(
                caller(),
                SetActiveLlmRequest {
                    provider_id: String::new(),
                    model: None,
                    role: Some(ProviderRole::Embedding),
                },
            )
            .await
            .expect("clear embedding");

        assert!(
            snapshot.embedding_active.is_none(),
            "clearing Embedding must remove embedding_active from snapshot"
        );
    }

    #[tokio::test]
    async fn set_active_embedding_may_coexist_with_kohai() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reborn_home = temp.path().join("reborn-home");
        std::fs::create_dir_all(&reborn_home).expect("mkdir");
        let boot = boot_for_home(&reborn_home);
        let service = RebornLlmConfigService::new(boot.clone(), key_store());

        // Make ibm_bob_inference both Kohai and Embedding — not a conflict.
        service
            .upsert_provider(caller(), upsert_request("ibm_bob_inference", None, true))
            .await
            .expect("upsert as kohai");

        let snapshot = service
            .set_active(
                caller(),
                set_active_request("ibm_bob_inference", Some(ProviderRole::Embedding)),
            )
            .await
            .expect("embedding must coexist with kohai");

        assert!(
            snapshot
                .embedding_active
                .as_ref()
                .is_some_and(|s| s.provider_id == "ibm_bob_inference"),
            "embedding selection must be set even when provider is also kohai"
        );
    }
}
