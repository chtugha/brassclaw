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
use crate::pg_basic_prompt_store::{PgBasicPromptStore, compute_fingerprint};

/// Minimum interval between `regenerate_prefix` calls per caller.
const RATE_LIMIT_INTERVAL: Duration = Duration::from_secs(60);

/// Maximum allowed persona text size (64 KiB).
const PERSONA_MAX_BYTES: usize = 64 * 1024;

/// Config key for the Sempai persona text.
const KEY_PERSONA: &str = "interceptor.sempai_persona";

/// Well-known prefix name for the default base-prompt bundle.
const PREFIX_NAME_BASE_PROMPT: &str = "base-prompt";

/// Component tables that may hold `Validated` rows for bundle assembly.
/// Each entry is `(table_name, class_code)`.
const COMPONENT_TABLES: &[(&str, u16)] = &[
    ("reborn_skills", 1),
    ("reborn_tools", 0),
    ("reborn_actions", 16),
    ("reborn_specs", 12),
    ("reborn_summaries", 15),
    ("reborn_lessons", 18),
    ("reborn_issues", 19),
    ("reborn_notes", 20),
    ("reborn_recipes", 21),
    ("reborn_tool_skills", 13),
    ("reborn_plans", 14),
    ("reborn_extensions_unified", 9),
    ("reborn_orchestrators", 10), // future migration; skipped when absent
    ("reborn_scaffolds", 50),     // future migration; skipped when absent
];

/// Class code → human-readable type label for bundle headers.
fn class_label(class_code: u16) -> &'static str {
    match class_code {
        0 => "Tool",
        1 => "Skill",
        9 => "Extension",
        10 => "Orchestrator",
        12 => "Spec",
        13 => "ToolSkill",
        14 => "Plan",
        15 => "Summary",
        16 => "Action",
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

/// Composition-side [`InterceptorConfigService`] implementation.
pub struct RebornInterceptorConfigService {
    pool: Arc<PgPool>,
    tenant_id: String,
    interceptor_mode: Option<SharedInterceptorMode>,
    sempai_gateway: Option<Arc<dyn brassclaw_loop_support::HostManagedModelGateway>>,
    regenerate_rate_limit: RateLimitState,
    /// Pre-assembled bundle store (reads/writes `reborn_basic_prompt_store`).
    #[cfg(feature = "postgres")]
    pg_basic_prompt_store: Option<Arc<PgBasicPromptStore>>,
}

impl RebornInterceptorConfigService {
    pub fn new(pool: Arc<PgPool>, tenant_id: impl Into<String>) -> Self {
        let tenant_id = tenant_id.into();
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
            #[cfg(feature = "postgres")]
            pg_basic_prompt_store: store,
        }
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
                    brassclaw_reborn::loop_driver_host::DEFAULT_SEMPAI_PERSONA.to_string()
                }),
            Err(e) => {
                tracing::debug!(
                    tenant_id = %self.tenant_id,
                    error = %e,
                    "interceptor load_persona: DB unavailable, using default"
                );
                brassclaw_reborn::loop_driver_host::DEFAULT_SEMPAI_PERSONA.to_string()
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

    /// Assemble the bundle from component tables, store it in `PgBasicPromptStore`,
    /// and return `(bundle_text, fingerprint)`.
    ///
    /// Checks `information_schema.tables` before querying each table so
    /// future-phase tables (not yet deployed) are skipped gracefully.
    ///
    /// The bundle text is stored — this is called only on operator demand (not per-turn).
    async fn do_assemble_bundle(
        &self,
        user_id: &str,
        project_id: &str,
        with_prewarm: bool,
    ) -> Result<(String, String), InterceptorConfigServiceError> {
        let client =
            self.pool
                .get()
                .await
                .map_err(|e| InterceptorConfigServiceError::InvalidRequest {
                    reason: format!("db pool: {e}"),
                })?;

        // Discover which component tables actually exist.
        let table_rows = client
            .query(
                "SELECT table_name FROM information_schema.tables \
                 WHERE table_schema = 'public' \
                   AND table_type = 'BASE TABLE' \
                   AND table_name = ANY($1)",
                &[&COMPONENT_TABLES.iter().map(|(t, _)| *t).collect::<Vec<_>>()],
            )
            .await
            .map_err(|e| InterceptorConfigServiceError::InvalidRequest {
                reason: format!("information_schema query: {e}"),
            })?;

        let existing_tables: std::collections::HashSet<String> = table_rows
            .iter()
            .filter_map(|r| r.try_get::<_, String>("table_name").ok())
            .collect();

        let mut parts: Vec<(u16, u32, String, String)> = Vec::new(); // (class_code, prompt_uid, name, content)

        for &(table, class_code) in COMPONENT_TABLES {
            if !existing_tables.contains(table) {
                continue;
            }
            let rows = client
                .query(
                    &format!(
                        "SELECT prompt_uid, name, \
                                COALESCE(content, '') AS content \
                         FROM {table} \
                         WHERE validation_status = 'validated' \
                           AND NOT ('05:validator' = ANY(COALESCE(consumer_tags, ARRAY[]::text[]))) \
                         ORDER BY prompt_uid ASC \
                         LIMIT 1000"
                    ),
                    &[],
                )
                .await;
            let rows = match rows {
                Ok(r) => r,
                Err(e) => {
                    tracing::debug!(table, error = %e, "interceptor assemble: skip table");
                    continue;
                }
            };
            for row in rows {
                let prompt_uid: i64 = match row.try_get("prompt_uid") {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::debug!(table, error = %e, "interceptor assemble: skip row (prompt_uid)");
                        continue;
                    }
                };
                let name: String = match row.try_get("name") {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::debug!(table, error = %e, "interceptor assemble: skip row (name)");
                        continue;
                    }
                };
                let content: String = match row.try_get("content") {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::debug!(table, error = %e, "interceptor assemble: skip row (content)");
                        continue;
                    }
                };
                parts.push((class_code, prompt_uid as u32, name, content));
            }
        }

        // Sort by (class_code ASC, prompt_uid ASC) — deterministic token order.
        parts.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

        let bundle = Self::do_format_bundle(&parts);
        let fingerprint = compute_fingerprint(&bundle);

        // Store the bundle text so per-turn calls can read it cheaply.
        #[cfg(feature = "postgres")]
        if let Some(store) = &self.pg_basic_prompt_store
            && let Err(e) = store
                .store(user_id, project_id, &bundle, with_prewarm)
                .await
        {
            tracing::debug!(error = %e, "do_assemble_bundle: store() failed (non-fatal)");
        }

        Ok((bundle, fingerprint))
    }

    /// Convert the sorted row set into the final bundle string.
    ///
    /// Pure-Rust formatter for Phase K.1. Swappable for a class-22 PythonCode
    /// component once it passes Q1+Q2 (Phase L bootstrap, §0.23.4).
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
            .map_err(|e| InterceptorConfigServiceError::InvalidRequest {
                reason: format!("persona save: {e}"),
            })?;
        }
        self.snapshot(caller).await
    }

    async fn list_prefix_entries(
        &self,
        _caller: WebUiAuthenticatedCaller,
        user_id: &str,
        project_id: &str,
    ) -> Result<PrefixListResponse, InterceptorConfigServiceError> {
        #[cfg(feature = "postgres")]
        {
            let entry = if let Some(store) = &self.pg_basic_prompt_store {
                store
                    .get_for_scope(user_id, project_id)
                    .await
                    .map_err(|e| InterceptorConfigServiceError::InvalidRequest {
                        reason: format!("list_prefix_entries db: {e}"),
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

        let caller_id = caller.user_id.to_string();
        self.check_rate_limit(&self.regenerate_rate_limit, &caller_id)
            .await?;

        // Assemble and store the bundle (with_prewarm=false initially; updated below if gateway succeeds).
        let (bundle, fingerprint) = self.do_assemble_bundle(user_id, project_id, false).await?;

        // Pre-warm the Sempai gateway so vLLM allocates KV blocks.
        let mut with_prewarm = false;
        if let Some(gateway) = &self.sempai_gateway {
            use brassclaw_loop_support::{
                HostManagedModelMessage, HostManagedModelMessageRole, HostManagedModelRequest,
            };
            use brassclaw_turns::{LoopMessageRef, TurnId, TurnRunId, run_profile::ModelProfileId};

            let profile_id = ModelProfileId::new("sempai_model").map_err(|e| {
                InterceptorConfigServiceError::InvalidRequest {
                    reason: format!("model profile id: {e}"),
                }
            })?;
            let content_ref = LoopMessageRef::new("interceptor:regenerate_prefix".to_string())
                .map_err(|e| InterceptorConfigServiceError::InvalidRequest {
                    reason: format!("message ref: {e}"),
                })?;
            let request = HostManagedModelRequest {
                model_profile_id: profile_id,
                messages: vec![HostManagedModelMessage {
                    role: HostManagedModelMessageRole::System,
                    content: bundle,
                    content_ref,
                    tool_result_provider_call: None,
                    tool_result_content: None,
                }],
                surface_version: None,
                resolved_model_route: None,
                run_id: TurnRunId::new(),
                turn_id: TurnId::new(),
            };

            gateway.stream_model(request).await.map_err(|e| {
                InterceptorConfigServiceError::InvalidRequest {
                    reason: format!("regenerate_prefix gateway call: {e}"),
                }
            })?;
            with_prewarm = true;

            // Re-store with prewarm=true to update prewarm_last_at.
            #[cfg(feature = "postgres")]
            if let Some(store) = &self.pg_basic_prompt_store
                && let Ok(Some(entry)) = store.get_for_scope(user_id, project_id).await
                && let Err(e) = store.store(user_id, project_id, &entry.bundle, true).await
            {
                // Re-assemble is not needed; re-read the already-stored bundle and call store() with prewarm=true.
                tracing::debug!(error = %e, "regenerate_prefix: re-store with prewarm failed");
            }
        }

        // Read the final row timestamps for the response.
        #[cfg(feature = "postgres")]
        let (assembled_at, prewarm_last_at_str) = if let Some(store) = &self.pg_basic_prompt_store {
            match store.get_for_scope(user_id, project_id).await {
                Ok(Some(entry)) => (
                    entry
                        .assembled_at
                        .map(|t| t.to_rfc3339())
                        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
                    entry.prewarm_last_at.map(|t| t.to_rfc3339()),
                ),
                _ => (chrono::Utc::now().to_rfc3339(), None),
            }
        } else {
            (chrono::Utc::now().to_rfc3339(), None)
        };
        #[cfg(not(feature = "postgres"))]
        let (assembled_at, prewarm_last_at_str): (String, Option<String>) = (
            chrono::Utc::now().to_rfc3339(),
            if with_prewarm {
                Some(chrono::Utc::now().to_rfc3339())
            } else {
                None
            },
        );

        let _ = with_prewarm; // suppress unused warning on non-postgres builds

        Ok(PrefixRegenerateResponse {
            name: PREFIX_NAME_BASE_PROMPT.to_string(),
            fingerprint,
            assembled_at,
            prewarm_last_at: prewarm_last_at_str,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_label_22_is_python_code() {
        assert_eq!(class_label(22), "PythonCode");
    }

    #[test]
    fn class_label_23_is_catalogue() {
        assert_eq!(class_label(23), "Catalogue");
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
}
