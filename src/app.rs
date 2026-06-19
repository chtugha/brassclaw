//! Application builder for initializing core BrassClaw components.
//!
//! Extracts the mechanical initialization phases from `main.rs` into a
//! reusable builder so that:
//!
//! - Tests can construct a full `AppComponents` without wiring channels
//! - Main stays focused on CLI dispatch and channel setup
//! - Each init phase is independently testable

use std::sync::Arc;

use crate::agent::SessionManager as AgentSessionManager;
use crate::logging::LogBroadcaster;
use crate::config::Config;
use crate::context::ContextManager;
use crate::db::{Database, UserStore};
use crate::extensions::ExtensionManager;
use crate::hooks::HookRegistry;
use crate::secrets::SecretsStore;
// TODO: MCP and WASM infrastructure removed - needs V2 reimplementation
use crate::workspace::Workspace;
use brassclaw_embeddings::{EmbeddingCacheConfig, EmbeddingProvider};
use brassclaw_llm::recording::HttpInterceptor;
use brassclaw_llm::{LlmProvider, LlmReloadHandle, RecordingLlm, SessionManager};
use brassclaw_safety::SafetyLayer;
use brassclaw_skills::SkillRegistry;
use brassclaw_skills::catalog::SkillCatalog;

/// Fully initialized application components, ready for channel wiring
/// and agent construction.
pub struct AppComponents {
    /// The (potentially mutated) config after DB reload and secret injection.
    pub config: Config,
    pub db: Option<Arc<dyn Database>>,
    pub secrets_store: Option<Arc<dyn SecretsStore + Send + Sync>>,
    pub llm: Arc<dyn LlmProvider>,
    pub cheap_llm: Option<Arc<dyn LlmProvider>>,
    /// Hot-reload controller for the LLM provider chain. `None` when the
    /// LLM was injected via `AppBuilder::with_llm` (test harnesses) so the
    /// chain was not built from config in the first place.
    pub llm_reload: Option<Arc<LlmReloadHandle>>,
    pub safety: Arc<SafetyLayer>,
    // TODO: V1 tools field removed - use effect_executor instead
    pub embeddings: Option<Arc<dyn EmbeddingProvider>>,
    pub workspace: Option<Arc<Workspace>>,
    /// Workspace-backed `SettingsStore` adapter that dual-writes settings to
    /// both the legacy `settings` table and `.system/settings/**` workspace
    /// documents. Populated when both `db` and `workspace` are available.
    /// Consumers that only need a `SettingsStore` (permission tools, the
    /// SIGHUP reload handler) should prefer this over the raw `db` so that
    /// runtime settings writes flow through the workspace and pick up schema
    /// validation.
    pub settings_store: Option<Arc<dyn crate::db::SettingsStore + Send + Sync>>,
    /// Concrete cache handle for `flush()` / `invalidate_user()`.
    /// Same instance backing `settings_store` when a cache is active.
    pub settings_cache: Option<Arc<crate::db::cached_settings::CachedSettingsStore>>,
    pub extension_manager: Option<Arc<ExtensionManager>>,
    // TODO: V1 MCP and WASM fields removed - needs V2 reimplementation
    pub log_broadcaster: Arc<LogBroadcaster>,
    pub context_manager: Arc<ContextManager>,
    pub hooks: Arc<HookRegistry>,
    /// Shared thread/session manager used by the standard agent runtime.
    pub agent_session_manager: Arc<AgentSessionManager>,
    pub skill_registry: Option<Arc<std::sync::RwLock<SkillRegistry>>>,
    pub skill_catalog: Option<Arc<SkillCatalog>>,
    pub cost_guard: Arc<crate::agent::cost_guard::CostGuard>,
    pub recording_handle: Option<Arc<RecordingLlm>>,
    pub http_interceptor: Option<Arc<dyn HttpInterceptor>>,
    pub session: Arc<SessionManager>,
    pub catalog_entries: Vec<crate::extensions::RegistryEntry>,
    // TODO: V1 builder field removed - needs V2 reimplementation
    /// In-process write-through cache: `(channel, external_id)` → `Identity`.
    /// Populated by the pairing flow (Task 8). Pre-allocated here so all
    /// subsystems can hold an `Arc` to the same cache instance.
    pub ownership_cache: Arc<crate::ownership::OwnershipCache>,
    /// V2 capability dispatcher for routing capability calls to built-in implementations.
    pub capability_dispatcher: Arc<crate::capabilities::dispatcher::BuiltinCapabilityDispatcher>,
    /// V2 effect executor for capability-based tool execution via EffectBridgeAdapter.
    pub effect_executor: Arc<dyn brassclaw_engine::EffectExecutor>,
    /// Routine engine slot that gets filled after RoutineEngine initialization to break circular dependency.
    pub routine_engine_slot: Arc<tokio::sync::RwLock<Option<Arc<crate::agent::routine_engine::RoutineEngine>>>>,
}

/// Options that control optional init phases.
#[derive(Default)]
pub struct AppBuilderFlags {
    pub no_db: bool,
}

/// Build an ephemeral in-memory secrets store backed by a freshly-generated
/// master key.
///
/// Returns `Err` only if the crypto routine fails to initialize — which
/// should not happen in practice, since the key is produced by the same
/// generator used throughout the test suite. Propagated (rather than
/// swallowed) so that a construction failure aborts startup at
/// `init_secrets` instead of surfacing later as an unactionable
/// "secrets store not initialized" error from `init_extensions`.
fn build_ephemeral_secrets_store()
-> Result<Arc<dyn SecretsStore + Send + Sync>, crate::secrets::SecretError> {
    use crate::secrets::{InMemorySecretsStore, SecretsCrypto};
    let ephemeral_key =
        secrecy::SecretString::from(crate::secrets::keychain::generate_master_key_hex());
    let crypto = SecretsCrypto::new(ephemeral_key)?;
    Ok(Arc::new(InMemorySecretsStore::new(Arc::new(crypto))))
}

/// Builder that orchestrates the 5 mechanical init phases.
pub struct AppBuilder {
    config: Config,
    flags: AppBuilderFlags,
    toml_path: Option<std::path::PathBuf>,
    session: Arc<SessionManager>,
    log_broadcaster: Arc<LogBroadcaster>,

    // Accumulated state
    db: Option<Arc<dyn Database>>,
    secrets_store: Option<Arc<dyn SecretsStore + Send + Sync>>,

    // Test overrides
    llm_override: Option<Arc<dyn LlmProvider>>,

    // Backend-specific handles needed by secrets store
    handles: Option<crate::db::DatabaseHandles>,
}

impl AppBuilder {
    /// Create a new builder.
    ///
    /// The `session` and `log_broadcaster` are created before the builder
    /// because tracing must be initialized before any init phase runs,
    /// and the log broadcaster is part of the tracing layer.
    pub fn new(
        config: Config,
        flags: AppBuilderFlags,
        toml_path: Option<std::path::PathBuf>,
        session: Arc<SessionManager>,
        log_broadcaster: Arc<LogBroadcaster>,
    ) -> Self {
        Self {
            config,
            flags,
            toml_path,
            session,
            log_broadcaster,
            db: None,
            secrets_store: None,
            llm_override: None,
            handles: None,
        }
    }

    /// Inject a pre-created database, skipping `init_database()`.
    ///
    /// **Warning:** this leaves `self.handles` as `None`, which means
    /// `init_secrets()` cannot construct a real `SecretsStore` (the store
    /// needs a backend-specific handle, not the generic `Arc<dyn Database>`).
    /// Tests that need credentials/OAuth/encrypted secrets must use
    /// [`AppBuilder::with_database_and_handles`] instead so the secrets
    /// path stays wired.
    pub fn with_database(&mut self, db: Arc<dyn Database>) {
        self.db = Some(db);
    }

    /// Inject a pre-created database **and** the matching backend-specific
    /// handles, skipping `init_database()`.
    ///
    /// Use this whenever the test will exercise code paths that touch
    /// `SecretsStore` (OAuth, encrypted credentials, secrets-backed WASM
    /// tools). For libSQL backends the handles are constructed via
    /// `LibSqlBackend::shared_db()`; for PostgreSQL via `PgBackend::pool()`.
    pub fn with_database_and_handles(
        &mut self,
        db: Arc<dyn Database>,
        handles: crate::db::DatabaseHandles,
    ) {
        self.db = Some(db);
        self.handles = Some(handles);
    }

    /// Inject a pre-created LLM provider, skipping `init_llm()`.
    pub fn with_llm(&mut self, llm: Arc<dyn LlmProvider>) {
        self.llm_override = Some(llm);
    }

    /// Phase 1: Initialize database backend.
    ///
    /// Creates the database connection, runs migrations, reloads config
    /// from DB, attaches DB to session manager, and cleans up stale jobs.
    pub async fn init_database(&mut self) -> Result<(), anyhow::Error> {
        if self.db.is_some() {
            tracing::debug!("Database already provided, skipping init_database()");
            return Ok(());
        }

        if self.flags.no_db {
            tracing::warn!("Running without database connection");
            return Ok(());
        }

        let (db, handles) = crate::db::connect_with_handles(&self.config.database)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        self.handles = Some(handles);

        // Post-init: ensure owner user row exists and rewrite 'default' user_id rows.
        bootstrap_ownership(db.as_ref(), &self.config)
            .await
            .map_err(|e| anyhow::anyhow!("bootstrap_ownership failed: {e}"))?;

        // Post-init: migrate disk config, reload config from DB, attach session, cleanup
        if let Err(e) =
            crate::bootstrap::migrate_disk_to_db(db.as_ref(), &self.config.owner_id).await
        {
            tracing::warn!("Disk-to-DB settings migration failed: {}", e);
        }

        let toml_path = self.toml_path.as_deref();
        // is_operator=true: owner_id is the operator/admin scope.
        match Config::from_db_with_toml(db.as_ref(), &self.config.owner_id, toml_path, true).await {
            Ok(db_config) => {
                self.config = db_config;
                tracing::debug!("Configuration reloaded from database");
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to reload config from DB, keeping env-based config: {}",
                    e
                );
            }
        }

        let session_db: brassclaw_llm::host::SharedSessionDb =
            std::sync::Arc::new(crate::llm_host::DatabaseSessionDb::new(db.clone()));
        self.session
            .attach_store(session_db, &self.config.owner_id)
            .await;

        // Fire-and-forget housekeeping — no need to block startup.
        let db_cleanup = db.clone();
        tokio::spawn(async move {
            if let Err(e) = db_cleanup.cleanup_stale_sandbox_jobs().await {
                tracing::warn!("Failed to cleanup stale sandbox jobs: {}", e);
            }
        });

        self.db = Some(db);
        Ok(())
    }

    /// Install an ephemeral in-memory secrets store so downstream WASM
    /// tool/channel wiring can always rely on `self.secrets_store` being
    /// `Some`.
    ///
    /// Used when persistent secrets construction fails (no master key, no DB
    /// handle, crypto init failure). Without this fallback, WASM tool
    /// credential injection silently does nothing on hosted TEE deployments
    /// because the loader only wires a store when `self.secrets_store` is
    /// `Some` — see #1537 ("WASM credential injection fails on hosted TEE").
    ///
    /// Tools that declare required credentials will then refuse to run via
    /// the fail-closed branch in `resolve_host_credentials`, surfacing a
    /// clear error instead of issuing unauthenticated HTTP requests.
    ///
    /// `reason` names the specific path that triggered the fallback — logged
    /// at warn so operators diagnosing a TEE deployment can distinguish
    /// "master key never resolved" from "master key resolved but no DB
    /// handle" from "crypto init failed" without turning on debug logging.
    ///
    /// Returns the error from `build_ephemeral_secrets_store` so that a
    /// genuinely broken crypto setup aborts startup here — otherwise a
    /// downstream phase (e.g. `init_extensions`) would later fail with a
    /// less actionable "secrets store not initialized" error.
    fn install_ephemeral_secrets_store(&mut self, reason: &str) -> Result<(), anyhow::Error> {
        let store = build_ephemeral_secrets_store().map_err(|e| {
            anyhow::anyhow!(
                "failed to initialize ephemeral secrets store ({reason}): {e}. \
                 This should not happen in practice; please report at \
                 https://github.com/chtugha/brassclaw/issues"
            )
        })?;
        tracing::warn!(
            reason = reason,
            "Persistent secrets store unavailable; installing ephemeral in-memory fallback. \
             Credentials saved via `brassclaw tool auth` will not persist across restarts. \
             Run `brassclaw doctor` for diagnostics (see #1537 for hosted-TEE specifics)."
        );
        self.secrets_store = Some(store);
        Ok(())
    }

    /// Phase 2: Create secrets store.
    ///
    /// Requires a master key and a backend-specific DB handle. After creating
    /// the store, injects any encrypted LLM API keys into the config overlay
    /// and re-resolves config.
    pub async fn init_secrets(&mut self) -> Result<(), anyhow::Error> {
        let master_key = match self.config.secrets.master_key() {
            Some(k) => k,
            None => {
                // No secrets DB available, but we can still load tokens from
                // OS credential stores (e.g., Anthropic OAuth via Claude Code's
                // macOS Keychain / Linux ~/.claude/.credentials.json).
                crate::config::inject_os_credentials();

                // Consume unused handles
                self.handles.take();

                // Re-resolve only the LLM config with OS credentials.
                let store: Option<&(dyn crate::db::SettingsStore + Sync)> =
                    self.db.as_ref().map(|db| db.as_ref() as _);
                let toml_path = self.toml_path.as_deref();
                let owner_id = self.config.owner_id.clone();
                if let Err(e) = self
                    .config
                    .re_resolve_llm(store, &owner_id, toml_path)
                    .await
                {
                    tracing::warn!(
                        "Failed to re-resolve LLM config after OS credential injection: {e}"
                    );
                }

                self.install_ephemeral_secrets_store("master key resolution produced no key")?;
                return Ok(());
            }
        };

        let crypto = match crate::secrets::SecretsCrypto::new(master_key.clone()) {
            Ok(c) => Arc::new(c),
            Err(e) => {
                tracing::warn!("Failed to initialize secrets crypto: {}", e);
                self.handles.take();
                self.install_ephemeral_secrets_store("secrets crypto initialization failed")?;
                return Ok(());
            }
        };

        // Fallback covers the no-database path where `init_database` returned
        // early before populating `self.handles`.
        let empty_handles = crate::db::DatabaseHandles::default();
        let handles = self.handles.as_ref().unwrap_or(&empty_handles);
        let store = crate::secrets::create_secrets_store(crypto, handles);

        // Safety gate: if we auto-generated a fresh master key this run
        // but the secrets table already carries rows from a prior key,
        // those rows are undecryptable and silently continuing would
        // shadow unrecoverable data. Fail loudly (and fail-closed on
        // probe error) so the user can restore the original key before
        // any new writes pile on top.
        //
        // Roll back the persistence `auto_generate_and_persist` already
        // committed: otherwise a subsequent restart would read the
        // newly-written key as `source = Env/Keychain, generated =
        // false`, skip this gate, and silently accept the wrong key.
        // Rollback keeps the gate re-firing on every start until the
        // user restores the real key or clears the stale rows.
        if let Some(ref secrets) = store
            && let Err(gate_err) = crate::secrets::verify_generated_key_safe(
                self.config.secrets.generated,
                secrets.as_ref(),
            )
            .await
        {
            if self.config.secrets.generated {
                crate::secrets::rollback_generated_key_persistence(
                    self.config.secrets.source,
                    &crate::bootstrap::brassclaw_env_path(),
                )
                .await;
            }
            return Err(gate_err.into());
        }

        if let Some(ref secrets) = store {
            // Migrate any plaintext API keys from the settings table to the
            // encrypted secrets store. Idempotent — safe to run on every startup.
            if let Some(ref db) = self.db {
                crate::config::migrate_plaintext_llm_keys(
                    db.as_ref(),
                    secrets.as_ref(),
                    &self.config.owner_id,
                )
                .await;

                // Migrate NEAR AI session token from plaintext settings to
                // encrypted secrets. Idempotent — safe to run on every startup.
                migrate_session_credential(db.as_ref(), secrets.as_ref(), &self.config.owner_id)
                    .await;
            }

            // Inject LLM API keys from encrypted storage
            crate::config::inject_llm_keys_from_secrets(secrets.as_ref(), &self.config.owner_id)
                .await;

            // Re-resolve only the LLM config with newly available keys,
            // including keys hydrated from the secrets store.
            let settings_store: Option<&(dyn crate::db::SettingsStore + Sync)> =
                self.db.as_ref().map(|db| db.as_ref() as _);
            let toml_path = self.toml_path.as_deref();
            let owner_id = self.config.owner_id.clone();
            // is_operator=true: owner_id is the operator/admin scope.
            if let Err(e) = self
                .config
                .re_resolve_llm_with_secrets(
                    settings_store,
                    &owner_id,
                    toml_path,
                    Some(secrets.as_ref()),
                    true,
                )
                .await
            {
                tracing::warn!("Failed to re-resolve LLM config after secret injection: {e}");
            }

            // Wire the secrets store into the session manager so future
            // token saves go to encrypted storage.
            let session_secrets: brassclaw_llm::host::SharedSessionSecrets = Arc::new(
                crate::llm_host::SecretsStoreSessionSecrets::new(Arc::clone(secrets)),
            );
            self.session.attach_secrets(session_secrets).await;
        }

        self.secrets_store = store;

        // If no persistent store was created (e.g. master key resolved but no
        // DB handle was available), fall back to an ephemeral in-memory store
        // so downstream WASM tool/channel wiring still goes through the
        // credential-injection code path. See `install_ephemeral_secrets_store`
        // for the rationale (#1537).
        if self.secrets_store.is_none() {
            let has_libsql_handle = self
                .handles
                .as_ref()
                .map(|h| {
                    #[cfg(feature = "libsql")]
                    {
                        h.libsql_db.is_some()
                    }
                    #[cfg(not(feature = "libsql"))]
                    {
                        let _ = h;
                        false
                    }
                })
                .unwrap_or(false);
            let has_pg_handle = self
                .handles
                .as_ref()
                .map(|h| {
                    #[cfg(feature = "postgres")]
                    {
                        h.pg_pool.is_some()
                    }
                    #[cfg(not(feature = "postgres"))]
                    {
                        let _ = h;
                        false
                    }
                })
                .unwrap_or(false);
            let reason = if self.handles.is_none() {
                "master key resolved but no database handles available (no_db mode or init_database did not run)"
            } else if !has_libsql_handle && !has_pg_handle {
                "master key resolved but neither libsql nor postgres handle is present (likely a feature-flag / backend mismatch)"
            } else {
                "master key resolved and DB handles present but create_secrets_store returned None (unexpected)"
            };
            self.install_ephemeral_secrets_store(reason)?;
        }

        Ok(())
    }

    /// Phase 3: Initialize LLM provider chain.
    ///
    /// Delegates to `build_provider_chain` which applies all decorators
    /// (retry, smart routing, failover, circuit breaker, response cache).
    #[allow(clippy::type_complexity)]
    pub async fn init_llm(
        &self,
    ) -> Result<
        (
            Arc<dyn LlmProvider>,
            Option<Arc<dyn LlmProvider>>,
            Option<Arc<RecordingLlm>>,
            Arc<LlmReloadHandle>,
        ),
        anyhow::Error,
    > {
        let (llm, cheap_llm, recording_handle, reload_handle) =
            brassclaw_llm::build_provider_chain(&self.config.llm, self.session.clone()).await?;
        Ok((llm, cheap_llm, recording_handle, reload_handle))
    }

    /// Phase 4: Initialize safety, tools, embeddings, and workspace.
    /// TODO: V1 tool system removed - this method is stubbed out
    /// Returns minimal values needed for V2 system initialization
    pub async fn init_tools(
        &self,
        _llm: &Arc<dyn LlmProvider>,
        _cheap_llm: Option<&Arc<dyn LlmProvider>>,
    ) -> Result<
        (
            Arc<SafetyLayer>,
            Option<Arc<dyn EmbeddingProvider>>,
            Option<Arc<Workspace>>,
            Option<Arc<dyn HttpInterceptor>>,
            Option<Arc<dyn crate::tools::builtin::memory::WorkspaceResolver>>,
        ),
        anyhow::Error,
    > {
        // Initialize safety layer
        let safety = Arc::new(SafetyLayer::new(&self.config.safety));
        tracing::debug!("Safety layer initialized");

        // Test-only HTTP host remapping
        let http_interceptor = if cfg!(any(test, debug_assertions)) {
            crate::http_intercept::remap_from_env()
        } else {
            None
        };

        // Create embeddings provider
        let bedrock_setup =
            self.config
                .llm
                .bedrock
                .as_ref()
                .map(|b| brassclaw_embeddings::BedrockEmbeddingSetup {
                    region: b.region.clone(),
                    profile: b.profile.clone(),
                });
        let embeddings = brassclaw_embeddings::create_provider(
            &self.config.embeddings,
            brassclaw_embeddings::ProviderDeps {
                session: self.session.clone(),
                bedrock_setup,
            },
        )
        .await;

        // Create workspace if database is available
        let workspace_user_id = self.config.owner_id.as_str();
        let (workspace, workspace_resolver) = if let Some(ref db) = self.db {
            let emb_cache_config = EmbeddingCacheConfig {
                max_entries: self.config.embeddings.cache_size,
            };
            let mut ws = Workspace::new_with_db(workspace_user_id, db.clone())
                .with_search_config(&self.config.search);

            if let Some(ref emb) = embeddings {
                ws = ws.with_embeddings_cached(emb.clone(), emb_cache_config.clone());
            }

            // Wire workspace-level settings (read scopes, memory layers)
            if !self.config.workspace.read_scopes.is_empty() {
                ws = ws.with_additional_read_scopes(self.config.workspace.read_scopes.clone());
                tracing::info!(
                    user_id = workspace_user_id,
                    read_scopes = ?ws.read_user_ids(),
                    "Workspace configured with multi-scope reads"
                );
            }
            ws = ws.with_memory_layers(self.config.workspace.memory_layers.clone());

            let is_multi_tenant = self.config.is_multi_tenant_deployment();
            if is_multi_tenant {
                ws = ws.with_admin_prompt();
            }

            let ws = Arc::new(ws);
            let pool: Arc<dyn crate::tools::builtin::memory::WorkspaceResolver> =
                Arc::new(crate::channels::web::platform::state::WorkspacePool::new(
                    Arc::clone(db),
                    embeddings.clone(),
                    emb_cache_config,
                    self.config.search.clone(),
                    self.config.workspace.clone(),
                ));
            
            tracing::debug!(
                multi_tenant = is_multi_tenant,
                "Workspace configured for V2 capability system"
            );

            (Some(ws), Some(pool))
        } else {
            (None, None)
        };

        Ok((
            safety,
            embeddings,
            workspace,
            http_interceptor,
            workspace_resolver,
        ))
    }

    /// TODO: V1 extension system removed - this method is stubbed out
    /// Returns minimal values needed for V2 system initialization
    pub async fn init_extensions(
        &self,
        _hooks: &Arc<HookRegistry>,
        _settings_store_override: Option<Arc<dyn crate::db::SettingsStore + Send + Sync>>,
        _ownership_cache: Arc<crate::ownership::OwnershipCache>,
    ) -> Result<
        (
            Option<Arc<ExtensionManager>>,
            Vec<crate::extensions::RegistryEntry>,
        ),
        anyhow::Error,
    > {
        // TODO: V1 WASM/MCP infrastructure removed - extension system needs V2 reimplementation
        
        // Load registry catalog entries for extension discovery
        let mut catalog_entries = match crate::registry::RegistryCatalog::load_or_embedded() {
            Ok(catalog) => {
                let entries = catalog.discovery_entries();
                tracing::debug!(
                    count = entries.len(),
                    "Loaded registry catalog entries for extension discovery"
                );
                entries
            }
            Err(e) => {
                tracing::warn!("Failed to load registry catalog: {}", e);
                Vec::new()
            }
        };

        // Append builtin entries
        let builtin = crate::extensions::registry::builtin_entries();
        for entry in builtin {
            if !catalog_entries.iter().any(|e| e.name == entry.name) {
                catalog_entries.push(entry);
            }
        }

        // Create minimal extension manager stub
        let extension_manager = if let Some(ref secrets) = self.secrets_store {
            let em = ExtensionManager::new_stub(
                Arc::clone(secrets),
                self.config.owner_id.clone(),
                self.db.clone(),
                catalog_entries.clone(),
            );
            Some(Arc::new(em))
        } else {
            None
        };

        tracing::debug!("Extension manager stub initialized for V2 capability system");


        Ok((
            mcp_session_manager,
            mcp_process_manager,
            wasm_tool_runtime,
            extension_manager,
            catalog_entries,
            dev_loaded_tool_names,
        ))
    }

    /// Run all init phases in order and return the assembled components.
    pub async fn build_all(mut self) -> Result<AppComponents, anyhow::Error> {
        self.init_database().await?;
        self.init_secrets().await?;

        // Post-init validation: backends with a dedicated config slot
        // (nearai/gemini_oauth/bedrock/openai_codex) read from their own
        // sub-struct and don't populate `LlmConfig.provider`. For
        // OpenAI-shape registry backends, fail early if no provider
        // config was resolved.
        let registry = brassclaw_llm::ProviderRegistry::load();
        let has_dedicated_config = registry
            .find(self.config.llm.backend.as_str())
            .is_some_and(|d| d.protocol.has_dedicated_config());
        if !has_dedicated_config && self.config.llm.provider.is_none() {
            let backend = &self.config.llm.backend;
            anyhow::bail!(
                "LLM_BACKEND={backend} is configured but no credentials were found. \
                 Set the appropriate API key environment variable or run the setup wizard."
            );
        }

        let (llm, cheap_llm, recording_handle, llm_reload) =
            if let Some(llm) = self.llm_override.take() {
                (llm, None, None, None)
            } else {
                let (llm, cheap, recording, reload) = self.init_llm().await?;
                (llm, cheap, recording, Some(reload))
            };
        let (
            safety,
            embeddings,
            workspace,
            http_interceptor,
            workspace_resolver,
        ) = self.init_tools(&llm, cheap_llm.as_ref()).await?;
        // TODO: V2 Reborn Capability System will be created after init_extensions()
        // where all required variables (tools, hooks, mcp managers, etc.) are available

        // Create hook registry early so runtime extension activation can register hooks.
        let hooks = Arc::new(HookRegistry::new());

        // Register session summary hook (writes conversation summary on session end).
        if let (Some(db), Some(ws_resolver)) = (&self.db, &workspace_resolver) {
            let summary_llm = cheap_llm
                .as_ref()
                .map(Arc::clone)
                .unwrap_or_else(|| Arc::clone(&llm));
            hooks
                .register(Arc::new(crate::hooks::SessionSummaryHook::new(
                    Arc::clone(db) as Arc<dyn crate::db::ConversationStore>,
                    Arc::clone(ws_resolver),
                    summary_llm,
                )))
                .await;
        }

        let agent_session_manager =
            Arc::new(AgentSessionManager::new().with_hooks(Arc::clone(&hooks)));

        // Build the workspace-backed `SettingsStore` BEFORE init_extensions so
        // tools registered there (`register_permission_tools`,
        // `upgrade_tool_list`) can be wired with the adapter from the start.
        // The same adapter instance is then exposed on `AppComponents.settings_store`
        // and reused by main.rs (e.g. for the SIGHUP reload handler).
        let (settings_store, settings_cache): (
            Option<Arc<dyn crate::db::SettingsStore + Send + Sync>>,
            Option<Arc<crate::db::cached_settings::CachedSettingsStore>>,
        ) = match (&workspace, &self.db) {
            (Some(ws), Some(db)) => {
                let adapter = Arc::new(crate::workspace::WorkspaceSettingsAdapter::new(
                    Arc::clone(ws),
                    Arc::clone(db),
                ));
                if let Err(e) = adapter.ensure_system_config().await {
                    tracing::debug!(
                        "WorkspaceSettingsAdapter eager seed failed (lazy seed will retry): {e}"
                    );
                }
                let cached = Arc::new(crate::db::cached_settings::CachedSettingsStore::new(
                    adapter as Arc<dyn crate::db::SettingsStore + Send + Sync>,
                ));
                (
                    Some(Arc::clone(&cached) as Arc<dyn crate::db::SettingsStore + Send + Sync>),
                    Some(cached),
                )
            }
            _ => (None, None),
        };

        let ownership_cache = Arc::new(crate::ownership::OwnershipCache::new());
        let (
            extension_manager,
            catalog_entries,
        ) = self
            .init_extensions(
                &hooks,
                settings_store.clone(),
                Arc::clone(&ownership_cache),
            )
            .await?;

        // Load bootstrap-completed flag from settings so that existing users
        // who already completed onboarding don't re-get bootstrap injection.
        if let Some(ref ws) = workspace {
            let toml_path = crate::settings::Settings::default_toml_path();
            if let Ok(Some(settings)) = crate::settings::Settings::load_toml(&toml_path)
                && settings.profile_onboarding_completed
            {
                ws.mark_bootstrap_completed();
            }
        }

        // Seed workspace and backfill embeddings
        if let Some(ref ws) = workspace {
            // Import workspace files from disk FIRST if WORKSPACE_IMPORT_DIR is set.
            // This lets Docker images / deployment scripts ship customized
            // workspace templates (e.g., AGENTS.md, TOOLS.md) that override
            // the generic seeds. Only imports files that don't already exist
            // in the database — never overwrites user edits.
            //
            // Runs before seed_if_empty() so that custom templates take priority
            // over generic seeds. seed_if_empty() then fills any remaining gaps.
            if let Ok(import_dir) = std::env::var("WORKSPACE_IMPORT_DIR") {
                let import_path = std::path::Path::new(&import_dir);
                match ws.import_from_directory(import_path).await {
                    Ok(count) if count > 0 => {
                        tracing::debug!("Imported {} workspace file(s) from {}", count, import_dir);
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!(
                            "Failed to import workspace files from {}: {}",
                            import_dir,
                            e
                        );
                    }
                }
            }

            match ws.seed_if_empty().await {
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!("Failed to seed workspace: {}", e);
                }
            }

            if embeddings.is_some() {
                let ws_bg = Arc::clone(ws);
                tokio::spawn(async move {
                    match ws_bg.backfill_embeddings().await {
                        Ok(count) if count > 0 => {
                            tracing::debug!("Backfilled embeddings for {} chunks", count);
                        }
                        Ok(_) => {}
                        Err(e) => {
                            tracing::warn!("Failed to backfill embeddings: {}", e);
                        }
                    }
                });
            }
        }

        // Skills system
        let (skill_registry, skill_catalog) = if self.config.skills.enabled {
            let mut registry = SkillRegistry::new(self.config.skills.local_dir.clone())
                .with_installed_dir(self.config.skills.installed_dir.clone())
                .with_bundled_content(crate::skills::bundled::load_bundled_skills())
                .with_max_scan_depth(self.config.skills.max_scan_depth);
            let loaded = registry.discover_all().await;
            if !loaded.is_empty() {
                tracing::debug!("Loaded {} skill(s): {}", loaded.len(), loaded.join(", "));
            }

            // Register credential mappings from skill frontmatter into the
            // shared registry so the HTTP tool can auto-inject credentials.
            crate::skills::register_skill_credentials(registry.skills(), &credential_registry);
            if let Some(db) = self.db.as_ref() {
                crate::skills::persist_skill_auth_descriptors(
                    registry.skills(),
                    Some(db.as_ref()),
                    &self.config.owner_id,
                )
                .await;
            }

            let registry = Arc::new(std::sync::RwLock::new(registry));
            let catalog = brassclaw_skills::catalog::shared_catalog();
            // TODO: V1 tools.register_skill_tools removed - needs V2 reimplementation
            (Some(registry), Some(catalog))
        } else {
            (None, None)
        };

        let context_manager = Arc::new(ContextManager::new(self.config.agent.max_parallel_jobs));
        let cost_guard = Arc::new(crate::agent::cost_guard::CostGuard::new(
            crate::agent::cost_guard::CostGuardConfig {
                max_cost_per_day_cents: self.config.agent.max_cost_per_day_cents,
                max_actions_per_hour: self.config.agent.max_actions_per_hour,
                max_cost_per_user_per_day_cents: self.config.agent.max_cost_per_user_per_day_cents,
            },
        ));

        tracing::debug!("V2 capability system initialization starting");

        // One-shot cleanup of ghost-seeded tool permission rows for the
        // owner. Pre-#3559, `seed_tool_permissions` wrote the code-level
        // defaults (e.g. `tool_install` → `AskEachTime`) into the DB so
        // the permissions panel could render them. Those rows were
        // indistinguishable from user-explicit overrides, so a user
        // could not be told from someone who never touched the setting,
        // and `AGENT_AUTO_APPROVE_TOOLS=true` ended up bypassing
        // user-explicit `AskEachTime` choices (#3559 security review).
        // The seeder is gone; this migration deletes ghost rows once,
        // after which any remaining row is user-explicit by
        // construction and `resolve_permission` can trust its value.
        cleanup_ghost_seeded_tool_permissions(self.db.as_ref(), &self.config.owner_id).await;

        // Initialize V2 Reborn Capability System
        // Create a workspace resolver for memory context
        let workspace_resolver: Arc<dyn crate::tools::builtin::memory::WorkspaceResolver> =
            if let Some(ws) = workspace.clone() {
                Arc::new(crate::tools::builtin::memory::FixedWorkspaceResolver::new(ws))
            } else {
                // Create a no-op resolver that returns a dummy workspace
                // This will be replaced with proper multi-tenant resolver in the future
                Arc::new(crate::tools::builtin::memory::FixedWorkspaceResolver::new(
                    Arc::new(crate::workspace::Workspace::new_with_db(
                        "default",
                        self.db.clone().expect("Database required for workspace"),
                    ))
                ))
            };

        // Initialize all 13 context structs for the capability dispatcher
        let filesystem_ctx = Arc::new(crate::capabilities::filesystem::FilesystemContext {
            base_dir: workspace.as_ref()
                .map(|_| std::path::PathBuf::from("."))
                .unwrap_or_else(|| std::path::PathBuf::from(".")),
            state: Default::default(),
        });

        let shell_ctx = Arc::new(crate::capabilities::shell::ShellContext {
            working_dir: workspace.as_ref().map(|_| std::path::PathBuf::from(".")),
            timeout: std::time::Duration::from_secs(120),
            allow_dangerous: false,
            sandbox: None, // Will be set by sandbox initialization if enabled
            sandbox_policy: crate::sandbox::SandboxPolicy::ReadOnly,
            extra_env: std::collections::HashMap::new(),
        });

        let network_ctx = Arc::new(crate::capabilities::network::NetworkContext {
            credential_registry: Some(Arc::clone(&credential_registry)),
            secrets_store: self.secrets_store.clone(),
            role_lookup: self.db.clone().map(|db| db as Arc<dyn crate::db::UserStore>),
            user_id: self.config.owner_id.clone(),
            http_interceptor: http_interceptor.clone(),
        });

        let memory_ctx = Arc::new(crate::capabilities::memory::MemoryContext {
            resolver: workspace_resolver,
            user_id: self.config.owner_id.clone(),
            user_timezone: "UTC".to_string(), // Default timezone, will be overridden per-session
            llm: Some(Arc::clone(&llm)),
            reasoning_enabled: false, // Default to false, can be enabled per-session
        });

        let messaging_ctx = Arc::new(crate::capabilities::messaging::MessagingContext {
            channel_manager: Arc::new(crate::channels::ChannelManager::new()),
            extension_manager: extension_manager.clone(),
            default_channel: Arc::new(std::sync::RwLock::new(None)),
            default_target: Arc::new(std::sync::RwLock::new(None)),
            base_dir: std::path::PathBuf::from("."),
            user_id: self.config.owner_id.clone(),
            metadata: serde_json::json!({}),
        });

        let jobs_ctx = Arc::new(crate::capabilities::jobs::JobsContext {
            context_manager: Arc::clone(&context_manager),
            scheduler_slot: None, // Will be set after Agent initialization
            job_manager: None, // Will be set by sandbox initialization if enabled
            store: self.db.clone(),
            event_tx: None, // Will be set by event system initialization
            inject_tx: None, // Will be set by channel initialization
            secrets_store: self.secrets_store.clone(),
            prompt_queue: None, // Will be set by orchestrator initialization
            user_id: self.config.owner_id.clone(),
            metadata: serde_json::json!({}),
        });

        let routine_engine_slot = Arc::new(tokio::sync::RwLock::new(None));
        let routines_ctx = Arc::new(crate::capabilities::routines::RoutinesContext {
            store: self.db.clone().expect("Database required for routines"),
            engine: routine_engine_slot.clone(), // Will be set after RoutineEngine initialization
            user_id: self.config.owner_id.clone(),
            metadata: serde_json::json!({}),
        });

        let skills_ctx = Arc::new(crate::capabilities::skills::SkillsContext {
            registry: skill_registry.clone().expect("Skill registry required"),
            catalog: skill_catalog.clone().expect("Skill catalog required"),
        });

        let extensions_ctx = Arc::new(crate::capabilities::extensions::ExtensionsContext {
            manager: extension_manager.clone().expect("Extension manager required"),
            user_id: self.config.owner_id.clone(),
        });

        let secrets_ctx = Arc::new(crate::capabilities::secrets::SecretsContext {
            store: self.secrets_store.clone().expect("Secrets store required"),
            user_id: self.config.owner_id.clone(),
        });

        let images_ctx = Arc::new(crate::capabilities::images::ImagesContext {
            api_base_url: "https://api.openai.com/v1".to_string(),
            api_key: secrecy::SecretString::new("".into()), // Will be populated from secrets store
            gen_model: "dall-e-3".to_string(),
            vision_model: "gpt-4-vision-preview".to_string(),
            client: reqwest::Client::new(),
            base_dir: None, // Will use current directory
        });

        let system_ctx = Arc::new(crate::capabilities::system::SystemContext {
            event_publisher: None, // Will be set by event system initialization
            tool_output_stash: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            user_timezone: "UTC".to_string(), // Default timezone
            conversation_id: None, // Will be set per-conversation
            registered_capability_names: Vec::new(), // Will be populated by capability registration
        });

        let pairing_ctx = Arc::new(crate::capabilities::pairing::PairingContext {
            store: Arc::new(crate::pairing::PairingStore::new(
                self.db.clone().expect("Database required for pairing"),
                Arc::clone(&ownership_cache),
            )),
            user_id: self.config.owner_id.clone(),
        });

        // Create the V2 capability dispatcher
        let capability_dispatcher = Arc::new(crate::capabilities::dispatcher::BuiltinCapabilityDispatcher::new(
            filesystem_ctx,
            shell_ctx,
            network_ctx,
            memory_ctx,
            messaging_ctx,
            jobs_ctx,
            routines_ctx,
            skills_ctx,
            extensions_ctx,
            secrets_ctx,
            images_ctx,
            system_ctx,
            pairing_ctx,
        ));

        // Create the V2 effect executor using EffectBridgeAdapter
        // 1. Create SharedExtensionRegistry (empty for now, will be populated by extension system)
        let extension_registry = Arc::new(brassclaw_extensions::SharedExtensionRegistry::new(
            brassclaw_extensions::ExtensionRegistry::new()
        ));

        // 2. Create TrustAwareCapabilityDispatchAuthorizer (using GrantAuthorizer for now)
        let authorizer = brassclaw_authorization::GrantAuthorizer::new();

        // 3. Create CapabilityHost with the dispatcher and authorizer
        // Note: We're using a 'static lifetime by leaking the references, which is acceptable
        // for application-lifetime components that live for the entire program duration.
        let capability_host = {
            // Leak the registry snapshot to get a 'static reference
            let registry_ref: &'static brassclaw_extensions::ExtensionRegistry =
                Box::leak(Box::new(extension_registry.snapshot()));
            
            // Leak the dispatcher to get a 'static reference
            // Safety: These components live for the entire application lifetime
            let dispatcher_ref: &'static crate::capabilities::dispatcher::BuiltinCapabilityDispatcher =
                unsafe { &*(Arc::as_ptr(&capability_dispatcher)) };
            
            // Leak the authorizer to get a 'static reference
            let authorizer_ref: &'static brassclaw_authorization::GrantAuthorizer =
                Box::leak(Box::new(authorizer));
            
            Arc::new(brassclaw_capabilities::CapabilityHost::new(
                registry_ref,
                dispatcher_ref,
                authorizer_ref,
            ))
        };

        // 4. Create EffectBridgeAdapter wrapping the CapabilityHost
        let effect_executor: Arc<dyn brassclaw_engine::EffectExecutor> = Arc::new(
            crate::bridge::EffectBridgeAdapterV2::new(
                capability_host,
                extension_registry,
                safety.clone(),
            )
        );

        Ok(AppComponents {
            config: self.config,
            db: self.db,
            secrets_store: self.secrets_store,
            llm,
            cheap_llm,
            llm_reload,
            safety,
            embeddings,
            workspace,
            settings_store,
            settings_cache,
            extension_manager,
            log_broadcaster: self.log_broadcaster,
            context_manager,
            hooks,
            agent_session_manager,
            skill_registry,
            skill_catalog,
            cost_guard,
            recording_handle,
            http_interceptor,
            session: self.session,
            catalog_entries,
            ownership_cache,
            capability_dispatcher,
            effect_executor,
            routine_engine_slot,
        })
    }
}

/// FK constraints applied after bootstrap_ownership rewrites 'default' rows.
/// NOT applied by the automatic refinery sweep — applied programmatically below.
///
/// PostgreSQL uses `ADD CONSTRAINT IF NOT EXISTS` to be idempotent.
/// libSQL (SQLite) does not support `ADD CONSTRAINT` at all — FK enforcement
/// there is handled by `PRAGMA foreign_keys = ON` in the schema declarations.
// TODO(ownership): Apply OWNERSHIP_FK_SQL on PostgreSQL after bootstrap completes.
// Requires detecting the database backend type from the Database trait object.
#[allow(dead_code)]
const OWNERSHIP_FK_SQL: &str = r#"
ALTER TABLE conversations    ADD CONSTRAINT IF NOT EXISTS fk_conversations_user
    FOREIGN KEY (user_id) REFERENCES users(id);
ALTER TABLE memory_documents ADD CONSTRAINT IF NOT EXISTS fk_memory_documents_user
    FOREIGN KEY (user_id) REFERENCES users(id);
ALTER TABLE heartbeat_state  ADD CONSTRAINT IF NOT EXISTS fk_heartbeat_user
    FOREIGN KEY (user_id) REFERENCES users(id);
ALTER TABLE secrets          ADD CONSTRAINT IF NOT EXISTS fk_secrets_user
    FOREIGN KEY (user_id) REFERENCES users(id);
ALTER TABLE wasm_tools       ADD CONSTRAINT IF NOT EXISTS fk_wasm_tools_user
    FOREIGN KEY (user_id) REFERENCES users(id);
ALTER TABLE routines         ADD CONSTRAINT IF NOT EXISTS fk_routines_user
    FOREIGN KEY (user_id) REFERENCES users(id);
ALTER TABLE settings         ADD CONSTRAINT IF NOT EXISTS fk_settings_user
    FOREIGN KEY (user_id) REFERENCES users(id);
ALTER TABLE agent_jobs       ADD CONSTRAINT IF NOT EXISTS fk_agent_jobs_user
    FOREIGN KEY (user_id) REFERENCES users(id);
"#;

/// Runs on every startup after migrations V1–V20.
/// Idempotent — safe to call multiple times.
///
/// 1. Ensures the owner user row exists in `users`.
/// 2. Rewrites all `user_id = 'default'` rows to the real owner_id.
pub async fn bootstrap_ownership(
    db: &dyn crate::db::Database,
    config: &crate::config::Config,
) -> Result<(), anyhow::Error> {
    let owner_id = &config.owner_id;

    // 1. Ensure owner user exists
    db.get_or_create_user(crate::db::UserRecord {
        id: owner_id.clone(),
        role: "admin".to_string(),
        display_name: "Owner".to_string(),
        status: "active".to_string(),
        email: None,
        last_login_at: None,
        created_by: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        metadata: serde_json::Value::Object(Default::default()),
    })
    .await?;

    // 2. Rewrite 'default' rows to the real owner_id
    db.migrate_default_owner(owner_id).await?;

    tracing::info!(
        owner_id = %owner_id,
        "bootstrap_ownership: owner user ensured, default rows migrated"
    );
    Ok(())
}

/// Migrate the NEAR AI session token from the plaintext settings table to the
/// encrypted secrets store.
///
/// The `nearai.session_token` settings key stores a JSON-serialized `SessionData`
/// object. This migration re-serializes it as a JSON string and stores it under
/// the `nearai_session_token` secret name.
///
/// Idempotent: if the secret already exists, the settings key is removed (cleanup).
/// If the settings key is absent, nothing happens.
async fn migrate_session_credential(
    db: &dyn crate::db::Database,
    secrets: &(dyn crate::secrets::SecretsStore + Send + Sync),
    user_id: &str,
) {
    // If already migrated and the secret decrypts to valid JSON, clean up the
    // plaintext copy and return. If the secret exists but is corrupt, fall
    // through to re-migrate from the plaintext settings value.
    match secrets.get_decrypted(user_id, "nearai_session_token").await {
        Ok(decrypted) => {
            if let Ok(secret_value) = serde_json::from_str::<serde_json::Value>(decrypted.expose())
            {
                // Verify the decrypted secret matches the plaintext setting (round-trip check).
                match db.get_setting(user_id, "nearai.session_token").await {
                    Ok(Some(settings_value)) if secret_value == settings_value => {
                        // Round-trip verified — safe to clean up plaintext copy.
                        let _ = db.delete_setting(user_id, "nearai.session_token").await;
                        return;
                    }
                    Ok(Some(_)) => {
                        // Secret doesn't match plaintext — fall through to re-migrate.
                        tracing::warn!(
                            "nearai_session_token secret doesn't match plaintext setting; re-migrating"
                        );
                    }
                    Ok(None) => {
                        // No plaintext left — treat as already migrated.
                        return;
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to read nearai.session_token setting for round-trip check: {e}"
                        );
                        return;
                    }
                }
            } else {
                // Secret exists but failed JSON parsing — fall through to re-migrate.
                tracing::warn!(
                    "nearai_session_token secret exists but failed JSON validation; re-migrating"
                );
            }
        }
        Err(crate::secrets::SecretError::NotFound(_)) => {
            // Not yet migrated — continue.
        }
        Err(e) => {
            tracing::warn!("Failed to check secrets store for nearai_session_token: {e}");
            return;
        }
    }

    // Read the JSON value from settings.
    let value = match db.get_setting(user_id, "nearai.session_token").await {
        Ok(Some(v)) => v,
        Ok(None) => return, // Nothing to migrate.
        Err(e) => {
            tracing::warn!("Failed to read nearai.session_token from settings: {e}");
            return;
        }
    };

    // Re-serialize the JSON value to a string for secrets storage.
    let value_str = match &value {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    };

    let params = crate::secrets::CreateSecretParams::new("nearai_session_token", value_str)
        .with_provider("nearai");

    match secrets.create(user_id, params).await {
        Ok(_) => {
            tracing::info!("Migrated nearai.session_token from settings to encrypted secrets");
            let _ = db.delete_setting(user_id, "nearai.session_token").await;
        }
        Err(e) => {
            tracing::warn!("Failed to migrate nearai.session_token to secrets: {e}");
        }
    }
}

/// Sentinel settings key marking that ghost-seeded tool permission rows
/// have been cleaned up for this owner. Reads/writes are idempotent and
/// scoped per-user, so the migration is safe to re-run.
const TOOL_PERMISSION_CLEANUP_SENTINEL: &str = "_internal.tool_permissions_seed_cleanup_v1";

/// One-shot migration that removes ghost-seeded `tool_permissions.<name>`
/// rows whose value matches `seeded_default_permission(name)` from the
/// owner's settings. After this runs, any surviving DB row is a
/// user-explicit choice — which lets `ToolPermissionSnapshot` treat all
/// DB rows as explicit again. See `cleanup_ghost_seeded_tool_permissions`
/// call site for context and the #3559 security review.
async fn cleanup_ghost_seeded_tool_permissions(db: Option<&Arc<dyn Database>>, owner_id: &str) {
    let db = match db {
        Some(db) => db,
        None => {
            tracing::debug!(
                "cleanup_ghost_seeded_tool_permissions: no database available, skipping"
            );
            return;
        }
    };

    // Skip if migration already ran for this owner.
    match db
        .get_setting(owner_id, TOOL_PERMISSION_CLEANUP_SENTINEL)
        .await
    {
        Ok(Some(_)) => {
            tracing::debug!("cleanup_ghost_seeded_tool_permissions: sentinel present, skipping");
            return;
        }
        Ok(None) => {}
        Err(e) => {
            tracing::warn!(
                "cleanup_ghost_seeded_tool_permissions: failed to read sentinel: {}",
                e
            );
            return;
        }
    }

    let db_map = match db.get_all_settings(owner_id).await {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(
                "cleanup_ghost_seeded_tool_permissions: failed to load settings: {}",
                e
            );
            return;
        }
    };
    let existing = crate::settings::Settings::from_db_map(&db_map).tool_permissions;

    let mut deleted = 0u32;
    for (tool_name, state) in &existing {
        let Some(seeded) = crate::tools::permissions::seeded_default_permission(tool_name) else {
            continue;
        };
        if *state != seeded {
            continue;
        }
        match db
            .delete_setting(owner_id, &format!("tool_permissions.{}", tool_name))
            .await
        {
            Ok(_) => deleted += 1,
            Err(e) => {
                tracing::warn!(
                    "cleanup_ghost_seeded_tool_permissions: failed to delete '{}': {}",
                    tool_name,
                    e
                );
            }
        }
    }

    // Record the sentinel even on partial failures so we don't re-scan
    // every startup. The deletes are idempotent if a future run does
    // re-process the same row.
    if let Err(e) = db
        .set_setting(
            owner_id,
            TOOL_PERMISSION_CLEANUP_SENTINEL,
            &serde_json::json!(true),
        )
        .await
    {
        tracing::warn!(
            "cleanup_ghost_seeded_tool_permissions: failed to write sentinel: {}",
            e
        );
    }

    if deleted > 0 {
        tracing::info!(
            count = deleted,
            "Cleaned up ghost-seeded tool permission rows for owner"
        );
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use tokio::sync::mpsc;

    use crate::agent::SessionManager as AgentSessionManager;
    use crate::hooks::{
        Hook, HookContext, HookError, HookEvent, HookOutcome, HookPoint, HookRegistry,
    };

    /// Regression for #1537 — WASM credential injection silently failed on
    /// hosted TEE deployments because the ephemeral-store fallback was only
    /// wired for `ExtensionManager`, not for `WasmToolLoader` or
    /// `setup_wasm_channels`. `build_ephemeral_secrets_store` is the shared
    /// construction path that `install_ephemeral_secrets_store` uses to
    /// guarantee `AppBuilder::secrets_store` is always `Some` after
    /// `init_secrets` — so every downstream consumer sees the same store.
    #[tokio::test]
    async fn ephemeral_secrets_store_is_constructible_and_usable() {
        use crate::secrets::CreateSecretParams;

        let store = super::build_ephemeral_secrets_store()
            .expect("ephemeral store construction must not fail with a freshly generated key");

        store
            .create(
                "user-1",
                CreateSecretParams::new("matrix_access_token", "tok-abc"),
            )
            .await
            .expect("storing a credential in the ephemeral store must succeed");

        let decrypted = store
            .get_decrypted("user-1", "matrix_access_token")
            .await
            .expect("reading the credential back from the ephemeral store must succeed");
        assert_eq!(decrypted.expose(), "tok-abc");
    }

    struct SessionStartHook {
        tx: mpsc::UnboundedSender<(String, String)>,
    }

    #[async_trait]
    impl Hook for SessionStartHook {
        fn name(&self) -> &str {
            "session-start-test"
        }

        fn hook_points(&self) -> &[HookPoint] {
            &[HookPoint::OnSessionStart]
        }

        async fn execute(
            &self,
            event: &HookEvent,
            _ctx: &HookContext,
        ) -> Result<HookOutcome, HookError> {
            if let HookEvent::SessionStart {
                user_id,
                session_id,
            } = event
            {
                self.tx
                    .send((user_id.clone(), session_id.clone()))
                    .expect("test channel receiver should be alive");
            } else {
                panic!("SessionStartHook received an unexpected event: {event:?}");
            }
            Ok(HookOutcome::ok())
        }
    }

    #[tokio::test]
    async fn agent_session_manager_runs_session_start_hooks() {
        let hooks = Arc::new(HookRegistry::new());
        let (tx, mut rx) = mpsc::unbounded_channel();
        hooks.register(Arc::new(SessionStartHook { tx })).await;

        let manager = AgentSessionManager::new().with_hooks(Arc::clone(&hooks));
        manager.get_or_create_session("user-123").await;

        let (user_id, session_id) =
            tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
                .await
                .expect("session start hook should fire")
                .expect("session start payload should be present");

        assert_eq!(user_id, "user-123");
        assert!(!session_id.is_empty());
    }

    /// #3559 security review: ghost-seeded rows whose value matches the
    /// code-level seeded default are deleted on first run. After cleanup,
    /// the row no longer exists in DB and `effective_permission` falls
    /// back to the code-level default at read time. Genuine user
    /// overrides (value != seeded default) survive untouched. The
    /// migration is idempotent — re-running after the sentinel is
    /// written is a no-op.
    #[cfg(feature = "libsql")]
    #[tokio::test]
    async fn cleanup_ghost_seeded_tool_permissions_removes_seed_matching_rows() {
        use crate::db::Database;
        use crate::db::libsql::LibSqlBackend;
        use crate::tools::permissions::PermissionState;

        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test_cleanup.db");
        let backend = LibSqlBackend::new_local(&db_path).await.unwrap();
        backend.run_migrations().await.unwrap();
        let db: Arc<dyn Database> = Arc::new(backend);

        let owner = "test-user";

        // 1. Simulate the old seeder's effect: write seeded-default rows
        //    for `tool_install` (AskEachTime) and `echo` (AlwaysAllow),
        //    plus a real user override for `shell` (AlwaysAllow, diverges
        //    from the seeded AskEachTime).
        let install_seed = serde_json::to_value(PermissionState::AskEachTime).unwrap();
        let echo_seed = serde_json::to_value(PermissionState::AlwaysAllow).unwrap();
        let shell_override = serde_json::to_value(PermissionState::AlwaysAllow).unwrap();
        db.set_setting(owner, "tool_permissions.tool_install", &install_seed)
            .await
            .unwrap();
        db.set_setting(owner, "tool_permissions.echo", &echo_seed)
            .await
            .unwrap();
        db.set_setting(owner, "tool_permissions.shell", &shell_override)
            .await
            .unwrap();

        // 2. Run the cleanup migration.
        super::cleanup_ghost_seeded_tool_permissions(Some(&db), owner).await;

        let map = db.get_all_settings(owner).await.unwrap();
        let settings = crate::settings::Settings::from_db_map(&map);

        // Ghost-seeded rows are gone.
        assert!(
            !settings.tool_permissions.contains_key("tool_install"),
            "tool_install row matching the seeded default must be removed"
        );
        assert!(
            !settings.tool_permissions.contains_key("echo"),
            "echo row matching the seeded default must be removed"
        );

        // Genuine user override survives.
        assert_eq!(
            settings.tool_permissions.get("shell"),
            Some(&PermissionState::AlwaysAllow),
            "shell override diverging from the seeded default must survive cleanup"
        );

        // Sentinel is set so subsequent runs are no-ops.
        let sentinel = db
            .get_setting(owner, super::TOOL_PERMISSION_CLEANUP_SENTINEL)
            .await
            .unwrap();
        assert!(sentinel.is_some(), "cleanup sentinel must be written");

        // 3. Re-running the migration after the sentinel is a no-op:
        //    re-seed a ghost row and assert it survives the second pass.
        db.set_setting(owner, "tool_permissions.tool_install", &install_seed)
            .await
            .unwrap();
        super::cleanup_ghost_seeded_tool_permissions(Some(&db), owner).await;
        let map = db.get_all_settings(owner).await.unwrap();
        let settings = crate::settings::Settings::from_db_map(&map);
        assert_eq!(
            settings.tool_permissions.get("tool_install"),
            Some(&PermissionState::AskEachTime),
            "after sentinel is written, a manually re-inserted row must NOT be cleaned up; \
             the migration is one-shot per owner"
        );
    }
}
