//! Composition-side implementation of the interceptor configuration service.
//!
//! Implements [`InterceptorConfigService`] backed by:
//! - `reborn_basic_prompt_store` Postgres table for prefix-cache storage.
//! - [`SharedInterceptorMode`] for reading the current routing/rerouting mode.
//! - An optional Sempai gateway for the `regenerate_prefix` pre-warm endpoint.
//!
//! # Bundle storage model (corrected — v2)
//!
//! The bundle text is stored in `bundle_json` inside `PgBasicPromptStore`.
//! Per-turn Kohai and Sempai calls read the stored text via `get_system_bundle()` —
//! one cheap single-row DB fetch, no per-turn component-table re-assembly.
//!
//! Assembly only runs when the operator calls `regenerate_prefix` or on first use.
//!
//! # vLLM APC
//!
//! vLLM automatic prefix caching fires when the client sends the same token
//! sequence on consecutive turns.  Storing the bundle ensures every turn sends
//! the exact same bytes → KV-cache hit.  No client-side `cache_control`
//! breakpoints are needed or supported.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use brassclaw_interceptor::SharedInterceptorMode;
use brassclaw_pg::PgPool;
use brassclaw_product_workflow::{
    InterceptorConfigService, InterceptorConfigServiceError, InterceptorConfigSnapshot,
    PrefixEntry, PrefixListResponse, PrefixRegenerateResponse, UpdateInterceptorConfigRequest,
    WebUiAuthenticatedCaller,
};

use crate::db_config::{ConfigWriteContext, save_config_key};
#[cfg(feature = "postgres")]
use crate::pg_basic_prompt_store::PgBasicPromptStore;
use crate::runtime::{InternalTurnOptions, RebornRuntime};

/// Minimum interval between `regenerate_prefix` calls per caller.
const RATE_LIMIT_INTERVAL: Duration = Duration::from_secs(60);

/// Maximum allowed persona text size (64 KiB).
const PERSONA_MAX_BYTES: usize = 64 * 1024;

/// Config key for the Sempai persona text.
const KEY_PERSONA: &str = "interceptor.sempai_persona";

/// Well-known prefix name for the default base-prompt bundle.
const PREFIX_NAME_BASE_PROMPT: &str = "base-prompt";

/// Component tables that may hold `Validated` rows for bundle assembly.
/// Each entry is `(table_name, class_code, content_expr)`.
///
/// `content_expr` is a **static SQL expression** selecting the effective
/// prompt text for a row.  It mirrors `class_code_to_table` in
/// `brassclaw_engine::memory::retrieval_source` — keep both in sync.
///
/// SECURITY: `table_name` and `content_expr` must always be `&'static str`
/// literals — never user input.  They are interpolated into SQL via `format!()`.
#[cfg(test)]
pub(crate) const COMPONENT_TABLES: &[(&str, u16, &str)] = &[
    // skills / scaffolds use the `body` column
    (
        "reborn_skills",
        1,
        "COALESCE(NULLIF(prior_knowledge_content,''), body)",
    ),
    // tools have no prose body — use description
    (
        "reborn_tools",
        0,
        "COALESCE(prior_knowledge_content, description)",
    ),
    // actions — description-only (steps are JSONB, not human-readable prose)
    (
        "reborn_actions",
        16,
        "COALESCE(prior_knowledge_content, description)",
    ),
    // memory-class tables all use the `content` column
    (
        "reborn_specs",
        12,
        "COALESCE(NULLIF(prior_knowledge_content,''), content)",
    ),
    (
        "reborn_summaries",
        15,
        "COALESCE(NULLIF(prior_knowledge_content,''), content)",
    ),
    (
        "reborn_lessons",
        18,
        "COALESCE(NULLIF(prior_knowledge_content,''), content)",
    ),
    (
        "reborn_issues",
        19,
        "COALESCE(NULLIF(prior_knowledge_content,''), content)",
    ),
    (
        "reborn_notes",
        20,
        "COALESCE(NULLIF(prior_knowledge_content,''), content)",
    ),
    (
        "reborn_tool_skills",
        13,
        "COALESCE(NULLIF(prior_knowledge_content,''), content)",
    ),
    (
        "reborn_plans",
        14,
        "COALESCE(NULLIF(prior_knowledge_content,''), content)",
    ),
    (
        "reborn_docus",
        17,
        "COALESCE(NULLIF(prior_knowledge_content,''), content)",
    ), // Phase P
    (
        "reborn_python_code",
        22,
        "COALESCE(NULLIF(prior_knowledge_content,''), content)",
    ), // Phase B
    // recipes have no plain-text body; prior_knowledge_content or empty
    (
        "reborn_recipes",
        21,
        "COALESCE(NULLIF(prior_knowledge_content,''), '')",
    ),
    // extensions use description
    (
        "reborn_extensions_unified",
        9,
        "COALESCE(prior_knowledge_content, description)",
    ),
    // extension catalogues use the overview_doc column
    (
        "reborn_extension_catalogues",
        23,
        "COALESCE(NULLIF(prior_knowledge_content,''), overview_doc)",
    ), // Phase C
];

/// Class code → human-readable type label for bundle headers.
pub(crate) fn class_label(class_code: u16) -> &'static str {
    match class_code {
        0 => "Tool",
        1 => "Skill",
        2 => "DomainSkill",
        3 => "ScaffoldSkill",
        4 => "RustyExtension",
        5 => "MontyExtension",
        6 => "McpServer",
        7 => "McpClient",
        8 => "LlmExtension",
        9 => "Extension",
        10 => "Orchestrator",
        11 => "Component", // reserved; no table
        12 => "Spec",
        13 => "ToolSkill",
        14 => "Plan",
        15 => "Summary",
        16 => "Action",
        17 => "Docu",
        18 => "Lesson",
        19 => "Issue",
        20 => "Note",
        21 => "Recipe",
        22 => "PythonCode",
        23 => "Catalogue",
        50 => "Scaffold",
        _ => "Component",
    }
}

/// Per-caller rate-limit state.
type RateLimitState = Arc<tokio::sync::Mutex<HashMap<String, Instant>>>;

#[cfg(feature = "postgres")]
struct PrefixScopeLeaseGuard {
    store: crate::pg_prefix_scope_ticket::PgPrefixScopeTicketStore,
    permit: crate::pg_prefix_scope_ticket::PrefixScopeTicket,
    token: uuid::Uuid,
}

#[cfg(feature = "postgres")]
impl Drop for PrefixScopeLeaseGuard {
    fn drop(&mut self) {
        let store = self.store.clone();
        let permit = self.permit.clone();
        let token = self.token;
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                if let Err(error) = store.release_lease(&permit, token).await {
                    tracing::debug!(%error, "prefix regeneration lease release failed; lease will expire");
                }
            });
        }
    }
}

/// Composition-side [`InterceptorConfigService`] implementation.
pub struct RebornInterceptorConfigService {
    pool: Arc<PgPool>,
    tenant_id: String,
    interceptor_mode: Option<SharedInterceptorMode>,
    sempai_gateway: Option<Arc<dyn brassclaw_loop_support::HostManagedModelGateway>>,
    regenerate_rate_limit: RateLimitState,
    runtime: Option<std::sync::Weak<RebornRuntime>>,
    /// Pre-assembled bundle store (reads/writes `reborn_basic_prompt_store`).
    #[cfg(feature = "postgres")]
    pg_basic_prompt_store: Option<Arc<PgBasicPromptStore>>,
    #[cfg(feature = "postgres")]
    scope_tickets: crate::pg_prefix_scope_ticket::PgPrefixScopeTicketStore,
}

impl RebornInterceptorConfigService {
    pub fn new(pool: Arc<PgPool>, tenant_id: impl Into<String>) -> Self {
        let tenant_id = tenant_id.into();
        #[cfg(feature = "postgres")]
        let scope_tickets =
            crate::pg_prefix_scope_ticket::PgPrefixScopeTicketStore::new(Arc::clone(&pool));
        #[cfg(feature = "postgres")]
        let store = Some(Arc::new(PgBasicPromptStore::new(
            Arc::clone(&pool),
            tenant_id.clone(),
            "", // agent_id: empty string matches the default scope
        )));
        Self {
            pool,
            tenant_id,
            interceptor_mode: None,
            sempai_gateway: None,
            regenerate_rate_limit: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            runtime: None,
            #[cfg(feature = "postgres")]
            pg_basic_prompt_store: store,
            #[cfg(feature = "postgres")]
            scope_tickets,
        }
    }

    pub fn with_runtime(mut self, runtime: Arc<RebornRuntime>) -> Self {
        #[cfg(feature = "postgres")]
        if self.pg_basic_prompt_store.is_some() {
            self.pg_basic_prompt_store = Some(Arc::new(PgBasicPromptStore::new(
                Arc::clone(&self.pool),
                self.tenant_id.clone(),
                runtime.webui_agent_id(),
            )));
        }
        self.runtime = Some(Arc::downgrade(&runtime));
        self
    }

    pub fn with_interceptor_mode(mut self, mode: SharedInterceptorMode) -> Self {
        self.interceptor_mode = Some(mode);
        self
    }

    pub fn with_sempai_gateway(
        mut self,
        gateway: Arc<dyn brassclaw_loop_support::HostManagedModelGateway>,
    ) -> Self {
        self.sempai_gateway = Some(gateway);
        self
    }

    /// Load the interceptor persona from the DB.
    async fn load_persona(&self) -> String {
        use crate::db_config::list_config_keys;
        match list_config_keys(&self.pool, &self.tenant_id).await {
            Ok(kv) => kv
                .into_iter()
                .find(|(k, _)| k == KEY_PERSONA)
                .map(|(_, v)| v)
                .unwrap_or_else(|| {
                    brassclaw_reborn::loop_driver_host::sempai_persona().to_string()
                }),
            Err(e) => {
                tracing::debug!(
                    tenant_id = %self.tenant_id,
                    error = %e,
                    "interceptor load_persona: DB unavailable, using boot-loaded default"
                );
                brassclaw_reborn::loop_driver_host::sempai_persona().to_string()
            }
        }
    }

    /// Check and update the rate limit for a caller.
    async fn check_rate_limit(
        &self,
        state: &RateLimitState,
        caller_id: &str,
    ) -> Result<(), InterceptorConfigServiceError> {
        let mut guard = state.lock().await;
        let now = Instant::now();
        guard.retain(|_, last| now.duration_since(*last) < RATE_LIMIT_INTERVAL);
        if let Some(&last) = guard.get(caller_id) {
            let elapsed = now.duration_since(last);
            if elapsed < RATE_LIMIT_INTERVAL {
                let retry_after = (RATE_LIMIT_INTERVAL - elapsed).as_secs().max(1);
                return Err(InterceptorConfigServiceError::RateLimitExceeded {
                    retry_after_seconds: retry_after,
                });
            }
        }
        guard.insert(caller_id.to_string(), now);
        Ok(())
    }

    /// Convert the sorted row set into the final bundle string.
    ///
    /// Pure-Rust formatter for Phase K.1. Swappable for a class-22 PythonCode
    /// component once it passes Q1+Q2 (Phase L bootstrap, §0.23.4).
    #[cfg(test)]
    fn do_format_bundle(parts: &[(u16, u32, String, String)]) -> String {
        let mut buf = String::new();
        for (class_code, prompt_uid, name, content) in parts {
            buf.push_str(&format!(
                "\n\n## {class_code}:{prompt_uid}  {}  \"{name}\"\n\n{content}",
                class_label(*class_code)
            ));
        }

        // Append the SempaiReviewOutcome JSON schema so the Sempai knows the
        // expected output format.
        buf.push_str(concat!(
            "\n\n## Sempai Response Schema\n\n",
            "```json\n",
            "{\n",
            "  \"adjusted_volatile_messages\": [[\"role\", \"content\"], ...],\n",
            "  \"bridge_messages\": [[\"role\", \"content\"], ...],\n",
            "  \"composition_summary\": \"string\",\n",
            "  \"proposed_recipe_updates\": [],\n",
            "  \"proposed_intent_examples\": [],\n",
            "  \"settings_adjustments\": []\n",
            "}\n",
            "```\n"
        ));

        buf
    }

    /// Return the stored bundle text for a scope.
    ///
    /// Fast path: non-stale, non-empty row → one cheap DB fetch.
    /// Slow path: stale or no row → minimal fallback (operator must click Regenerate).
    pub async fn get_system_bundle(&self, user_id: &str, project_id: &str) -> String {
        #[cfg(feature = "postgres")]
        if let Some(store) = &self.pg_basic_prompt_store {
            return crate::pg_basic_prompt_store::get_system_bundle(store, user_id, project_id)
                .await;
        }
        crate::pg_basic_prompt_store::minimal_base_prompt_fallback()
    }
}

#[async_trait]
impl InterceptorConfigService for RebornInterceptorConfigService {
    async fn snapshot(
        &self,
        _caller: WebUiAuthenticatedCaller,
    ) -> Result<InterceptorConfigSnapshot, InterceptorConfigServiceError> {
        let persona = self.load_persona().await;
        let mode_str = if self
            .interceptor_mode
            .as_ref()
            .is_some_and(|m| m.get() == brassclaw_interceptor::InterceptorMode::Rerouting)
        {
            "rerouting".to_string()
        } else {
            "routing".to_string()
        };
        Ok(InterceptorConfigSnapshot {
            sempai_connected: mode_str == "rerouting",
            mode: mode_str,
            persona,
        })
    }

    async fn update(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: UpdateInterceptorConfigRequest,
    ) -> Result<InterceptorConfigSnapshot, InterceptorConfigServiceError> {
        if let Some(persona) = request.persona {
            if persona.len() > PERSONA_MAX_BYTES {
                return Err(InterceptorConfigServiceError::InvalidRequest {
                    reason: format!(
                        "persona text exceeds maximum size ({} > {} bytes)",
                        persona.len(),
                        PERSONA_MAX_BYTES
                    ),
                });
            }
            save_config_key(
                &self.pool,
                &self.tenant_id,
                KEY_PERSONA,
                &persona,
                ConfigWriteContext::Operator,
            )
            .await
            .map_err(|e| {
                tracing::debug!(error = %e, "interceptor update: persona save failed");
                InterceptorConfigServiceError::Unavailable
            })?;
        }
        self.snapshot(caller).await
    }

    async fn list_prefix_entries(
        &self,
        caller: WebUiAuthenticatedCaller,
        user_id: &str,
        project_id: &str,
    ) -> Result<PrefixListResponse, InterceptorConfigServiceError> {
        if caller.tenant_id.as_str() != self.tenant_id || caller.user_id.as_str() != user_id {
            return Err(InterceptorConfigServiceError::InvalidRequest {
                reason: "prefix scope must match the authenticated caller".into(),
            });
        }
        #[cfg(feature = "postgres")]
        {
            let entry = if let Some(store) = &self.pg_basic_prompt_store {
                store
                    .get_for_scope(user_id, project_id)
                    .await
                    .map_err(|e| {
                        tracing::debug!(error = %e, "list_prefix_entries: db error");
                        InterceptorConfigServiceError::Unavailable
                    })?
            } else {
                None
            };
            let prefixes = vec![PrefixEntry {
                name: PREFIX_NAME_BASE_PROMPT.to_string(),
                fingerprint: entry.as_ref().map(|e| e.fingerprint.clone()),
                is_stale: entry.as_ref().map(|e| e.is_stale).unwrap_or(true),
                assembled_at: entry
                    .as_ref()
                    .and_then(|e| e.assembled_at)
                    .map(|t| t.to_rfc3339()),
                prewarm_last_at: entry
                    .as_ref()
                    .and_then(|e| e.prewarm_last_at)
                    .map(|t| t.to_rfc3339()),
                generation_ms: entry.as_ref().and_then(|e| e.generation_ms),
            }];
            return Ok(PrefixListResponse { prefixes });
        }
        #[cfg(not(feature = "postgres"))]
        Ok(PrefixListResponse {
            prefixes: vec![PrefixEntry {
                name: PREFIX_NAME_BASE_PROMPT.to_string(),
                fingerprint: None,
                is_stale: true,
                assembled_at: None,
                prewarm_last_at: None,
                generation_ms: None,
            }],
        })
    }

    async fn regenerate_prefix(
        &self,
        caller: WebUiAuthenticatedCaller,
        name: &str,
        user_id: &str,
        project_id: &str,
    ) -> Result<PrefixRegenerateResponse, InterceptorConfigServiceError> {
        if name != PREFIX_NAME_BASE_PROMPT {
            return Err(InterceptorConfigServiceError::PrefixNotFound {
                name: name.to_string(),
            });
        }

        if caller.tenant_id.as_str() != self.tenant_id || caller.user_id.as_str() != user_id {
            return Err(InterceptorConfigServiceError::InvalidRequest {
                reason: "prefix scope must match the authenticated caller".into(),
            });
        }
        let caller_id = caller.user_id.to_string();
        self.check_rate_limit(&self.regenerate_rate_limit, &caller_id)
            .await?;

        #[cfg(not(feature = "postgres"))]
        return Err(InterceptorConfigServiceError::Unavailable);

        #[cfg(feature = "postgres")]
        {
            let runtime = self
                .runtime
                .as_ref()
                .and_then(std::sync::Weak::upgrade)
                .ok_or(InterceptorConfigServiceError::Unavailable)?;
            let agent_id = runtime.webui_agent_id();
            let conversation = runtime
                .new_internal_conversation(&self.tenant_id, user_id, project_id)
                .await
                .map_err(|_| InterceptorConfigServiceError::Unavailable)?;
            let request_id = uuid::Uuid::new_v4();
            let scope_ticket = uuid::Uuid::new_v4().to_string();
            let permit = crate::pg_prefix_scope_ticket::PrefixScopeTicket {
                tenant_id: self.tenant_id.clone(),
                user_id: user_id.to_string(),
                agent_id: agent_id.to_string(),
                project_id: project_id.to_string(),
                conversation_id: conversation.0.as_str().to_string(),
                request_id,
            };
            let lease_token = self
                .scope_tickets
                .acquire_lease(&permit)
                .await
                .map_err(|_| InterceptorConfigServiceError::Unavailable)?;
            let _lease_guard = PrefixScopeLeaseGuard {
                store: self.scope_tickets.clone(),
                permit: permit.clone(),
                token: lease_token,
            };
            self.scope_tickets
                .issue(&scope_ticket, &permit)
                .await
                .map_err(|_| InterceptorConfigServiceError::Unavailable)?;
            let reply = runtime
                .send_internal_user_message(
                    &conversation,
                    &format!("regenerate prefix bundle ticket={scope_ticket}"),
                    InternalTurnOptions {
                        hard_fail_on_recipe_miss: true,
                        allow_tier_two: false,
                    },
                )
                .await
                .map_err(|_| InterceptorConfigServiceError::Unavailable)?;
            if !reply.is_successful_final_reply() {
                return Err(InterceptorConfigServiceError::Unavailable);
            }

            let store = self
                .pg_basic_prompt_store
                .as_ref()
                .ok_or(InterceptorConfigServiceError::Unavailable)?;
            let entry = store
                .get_for_scope(user_id, project_id)
                .await
                .map_err(|_| InterceptorConfigServiceError::Unavailable)?
                .filter(|entry| entry.generation_id == Some(request_id))
                .ok_or(InterceptorConfigServiceError::Unavailable)?;

            let mut with_prewarm = false;
            if let Some(gateway) = &self.sempai_gateway {
                use brassclaw_loop_support::{
                    HostManagedModelMessage, HostManagedModelMessageRole, HostManagedModelRequest,
                };
                use brassclaw_turns::{
                    LoopMessageRef, TurnId, TurnRunId, run_profile::ModelProfileId,
                };
                let profile_id = ModelProfileId::new("sempai_model")
                    .map_err(|_| InterceptorConfigServiceError::Unavailable)?;
                let content_ref = LoopMessageRef::new("msg:interceptor.regenerate-prefix")
                    .map_err(|_| InterceptorConfigServiceError::Unavailable)?;
                let request = HostManagedModelRequest {
                    model_profile_id: profile_id,
                    messages: vec![HostManagedModelMessage {
                        role: HostManagedModelMessageRole::System,
                        content: entry.bundle.clone(),
                        content_ref,
                        tool_result_provider_call: None,
                        tool_result_content: None,
                    }],
                    surface_version: None,
                    resolved_model_route: None,
                    run_id: TurnRunId::new(),
                    turn_id: TurnId::new(),
                };
                if gateway.stream_model(request).await.is_ok() {
                    with_prewarm = true;
                    store
                        .store(
                            user_id,
                            project_id,
                            &entry.bundle,
                            true,
                            entry.generation_ms,
                            request_id,
                        )
                        .await
                        .map_err(|_| InterceptorConfigServiceError::Unavailable)?;
                }
            }
            let final_entry = store
                .get_for_scope(user_id, project_id)
                .await
                .map_err(|_| InterceptorConfigServiceError::Unavailable)?
                .filter(|entry| entry.generation_id == Some(request_id))
                .ok_or(InterceptorConfigServiceError::Unavailable)?;
            let assembled_at = final_entry
                .assembled_at
                .map(|t| t.to_rfc3339())
                .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
            let prewarm_last_at = final_entry.prewarm_last_at.map(|t| t.to_rfc3339());
            let _ = with_prewarm;
            Ok(PrefixRegenerateResponse {
                name: PREFIX_NAME_BASE_PROMPT.to_string(),
                fingerprint: final_entry.fingerprint,
                assembled_at,
                prewarm_last_at,
                generation_ms: final_entry.generation_ms,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_label_17_is_docu() {
        assert_eq!(class_label(17), "Docu");
    }

    #[test]
    fn class_label_22_is_python_code() {
        assert_eq!(class_label(22), "PythonCode");
    }

    #[test]
    fn class_label_23_is_catalogue() {
        assert_eq!(class_label(23), "Catalogue");
    }

    #[test]
    fn class_label_previously_missing_arms() {
        assert_eq!(class_label(2), "DomainSkill");
        assert_eq!(class_label(3), "ScaffoldSkill");
        assert_eq!(class_label(4), "RustyExtension");
        assert_eq!(class_label(5), "MontyExtension");
        assert_eq!(class_label(6), "McpServer");
        assert_eq!(class_label(7), "McpClient");
        assert_eq!(class_label(8), "LlmExtension");
        assert_eq!(class_label(11), "Component"); // reserved
    }

    #[test]
    fn component_tables_contains_reborn_docus() {
        assert!(
            COMPONENT_TABLES
                .iter()
                .any(|(t, c, _)| *t == "reborn_docus" && *c == 17),
            "COMPONENT_TABLES must contain (\"reborn_docus\", 17) for Phase P doc-conversion"
        );
    }

    #[test]
    fn component_tables_contains_reborn_python_code() {
        assert!(
            COMPONENT_TABLES
                .iter()
                .any(|(t, c, _)| *t == "reborn_python_code" && *c == 22),
            "COMPONENT_TABLES must contain (\"reborn_python_code\", 22)"
        );
    }

    #[test]
    fn component_tables_contains_reborn_extension_catalogues() {
        assert!(
            COMPONENT_TABLES
                .iter()
                .any(|(t, c, _)| *t == "reborn_extension_catalogues" && *c == 23),
            "COMPONENT_TABLES must contain (\"reborn_extension_catalogues\", 23)"
        );
    }

    #[test]
    fn do_format_bundle_empty_parts_contains_schema() {
        let bundle = RebornInterceptorConfigService::do_format_bundle(&[]);
        assert!(bundle.contains("Sempai Response Schema"));
        assert!(bundle.contains("adjusted_volatile_messages"));
    }

    #[test]
    fn do_format_bundle_includes_class_label() {
        let parts = vec![(1u16, 1u32, "test-skill".to_string(), "content".to_string())];
        let bundle = RebornInterceptorConfigService::do_format_bundle(&parts);
        assert!(bundle.contains("Skill"));
        assert!(bundle.contains("test-skill"));
        assert!(bundle.contains("content"));
    }

    #[test]
    fn prewarm_content_ref_literal_is_a_valid_loop_message_ref() {
        // Regression: the literal used for the Sempai pre-warm HostManagedModelRequest
        // must satisfy LoopMessageRef validation (prefix "msg:" + alphanumeric/._-).
        // Previously "interceptor:regenerate_prefix" was used which fails with
        // `loop_message_ref must start with msg:`, producing the 400 invalid_request
        // error shown in the WebUI prefix tab.
        use brassclaw_turns::LoopMessageRef;
        assert!(
            LoopMessageRef::new("msg:interceptor.regenerate-prefix").is_ok(),
            "prewarm content_ref literal must be a valid LoopMessageRef"
        );
    }
}
