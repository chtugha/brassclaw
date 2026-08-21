//! Composition-side implementation of the interceptor configuration service.
//!
//! Implements [`InterceptorConfigService`] backed by:
//! - `brassclaw_config` Postgres table for persisting base prompt + persona.
//! - [`SharedInterceptorMode`] for reading the current routing/rerouting mode.
//! - An optional Sempai gateway for the pre-warm endpoint.
//!
//! The `reassemble_base_prompt()` method queries component tables directly
//! (Q20 — NOT `reborn_component_catalog`) and is resilient to missing tables
//! (earlier-phase tables may not exist yet).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use brassclaw_interceptor::SharedInterceptorMode;
use brassclaw_pg::PgPool;
use brassclaw_product_workflow::{
    InterceptorConfigService, InterceptorConfigServiceError, InterceptorConfigSnapshot,
    UpdateInterceptorConfigRequest, WebUiAuthenticatedCaller,
};

use crate::db_config::{ConfigWriteContext, list_config_keys, save_config_key};

/// Minimum interval between `reassemble` or `prewarm` calls per caller.
const RATE_LIMIT_INTERVAL: Duration = Duration::from_secs(60);

/// Maximum allowed persona text size (64 KiB).  Prevents operators from
/// accidentally filling the `brassclaw_config` row with a multi-MB string.
const PERSONA_MAX_BYTES: usize = 64 * 1024;

/// Config keys used in `brassclaw_config`.
const KEY_BASE_PROMPT: &str = "interceptor.sempai_base_prompt";
const KEY_BASE_PROMPT_ASSEMBLED_AT: &str = "interceptor.sempai_base_prompt_assembled_at";
const KEY_PERSONA: &str = "interceptor.sempai_persona";
const KEY_PREWARM_LAST_AT: &str = "interceptor.sempai_prewarm_last_at";

/// Component tables that may hold `Validated` rows for Part A assembly.
/// Ordered tables that may exist in the schema depending on which phases
/// have been deployed.  Each entry is `(table_name, class_code)`.
///
/// Class codes must match the CHECK constraints in the corresponding
/// DDL migrations; the service uses the code from this table as the
/// section header in the assembled Sempai base prompt and does NOT
/// re-read `class_code` from the DB rows.
const COMPONENT_TABLES: &[(&str, u16)] = &[
    ("reborn_skills", 1), // V027  CHECK (class_code IN (1, 2, 3)) — primary label = Skill
    ("reborn_tools", 0),  // V030  CHECK (class_code = 0)
    ("reborn_actions", 16), // V029  CHECK (class_code = 16)
    ("reborn_specs", 12), // V036  CHECK (class_code = 12)
    ("reborn_summaries", 15), // V039  CHECK (class_code = 15)
    ("reborn_lessons", 18), // V041  CHECK (class_code = 18)
    ("reborn_issues", 19), // V042  CHECK (class_code = 19)
    ("reborn_notes", 20), // V043  CHECK (class_code = 20)
    ("reborn_recipes", 21), // V033  CHECK (class_code = 21)
    ("reborn_tool_skills", 13), // V037  CHECK (class_code = 13)
    ("reborn_plans", 14), // V038  CHECK (class_code = 14)
    ("reborn_extensions_unified", 9), // V032  CHECK (class_code IN (4–9)); 9 = Misc/Extension used as section label
    ("reborn_orchestrators", 10),     // future migration; gracefully skipped when absent
    ("reborn_scaffolds", 50),         // future migration; gracefully skipped when absent
];

/// Class code → human-readable type label for Part A headers.
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
    reassemble_rate_limit: RateLimitState,
    prewarm_rate_limit: RateLimitState,
}

impl RebornInterceptorConfigService {
    pub fn new(pool: Arc<PgPool>, tenant_id: impl Into<String>) -> Self {
        Self {
            pool,
            tenant_id: tenant_id.into(),
            interceptor_mode: None,
            sempai_gateway: None,
            reassemble_rate_limit: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            prewarm_rate_limit: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
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

    /// Load interceptor config keys from the DB.
    ///
    /// Returns an empty map on DB error (resilience: the interceptor should still
    /// function with default config even if the DB is temporarily unavailable).
    /// The failure is logged at `debug!` so it remains observable.
    async fn load_config(&self) -> HashMap<String, String> {
        match list_config_keys(&self.pool, &self.tenant_id).await {
            Ok(kv) => kv
                .into_iter()
                .filter(|(k, _)| k.starts_with("interceptor."))
                .collect(),
            Err(e) => {
                tracing::debug!(
                    tenant_id = %self.tenant_id,
                    error = %e,
                    "interceptor load_config: DB unavailable, using empty config"
                );
                HashMap::new()
            }
        }
    }

    /// Build a snapshot from a loaded KV map.
    fn build_snapshot(&self, kv: &HashMap<String, String>) -> InterceptorConfigSnapshot {
        let base_prompt = kv.get(KEY_BASE_PROMPT).cloned();
        let base_prompt_size = base_prompt.as_deref().map(|s| s.len());
        let mode_str = if self
            .interceptor_mode
            .as_ref()
            .is_some_and(|m| m.get() == brassclaw_interceptor::InterceptorMode::Rerouting)
        {
            "rerouting".to_string()
        } else {
            "routing".to_string()
        };
        let sempai_connected = mode_str == "rerouting";
        InterceptorConfigSnapshot {
            sempai_connected,
            mode: mode_str,
            base_prompt_assembled_at: kv.get(KEY_BASE_PROMPT_ASSEMBLED_AT).cloned(),
            base_prompt_size_chars: base_prompt_size,
            persona: kv.get(KEY_PERSONA).cloned().unwrap_or_else(|| {
                brassclaw_reborn::loop_driver_host::DEFAULT_SEMPAI_PERSONA.to_string()
            }),
            prewarm_last_at: kv.get(KEY_PREWARM_LAST_AT).cloned(),
            components_since_rebuild: None,
        }
    }

    /// Check and update the rate limit for a caller.  Returns `Err` if the
    /// caller has already made a request within the rate-limit window.
    ///
    /// Also prunes stale entries that are older than one window to keep the
    /// map bounded (one entry per distinct caller who has used the endpoint).
    async fn check_rate_limit(
        &self,
        state: &RateLimitState,
        caller_id: &str,
    ) -> Result<(), InterceptorConfigServiceError> {
        let mut guard = state.lock().await;
        let now = Instant::now();
        // Prune entries that are beyond the window — they will no longer
        // trigger a rate-limit rejection on the next call.
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

    /// Reassemble Part A from individual component tables using direct SQL.
    ///
    /// Checks `information_schema.tables` before querying each table so
    /// tables from later phases (not yet deployed) are skipped gracefully.
    async fn do_reassemble(&self) -> Result<String, InterceptorConfigServiceError> {
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
                        "SELECT prompt_uid, name, COALESCE(content, '') AS content \
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
                    tracing::debug!(table, error = %e, "interceptor reassemble: skip table");
                    continue;
                }
            };
            for row in rows {
                let prompt_uid: i64 = match row.try_get("prompt_uid") {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::debug!(table, error = %e, "interceptor reassemble: skip row (prompt_uid)");
                        continue;
                    }
                };
                let name: String = match row.try_get("name") {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::debug!(table, error = %e, "interceptor reassemble: skip row (name)");
                        continue;
                    }
                };
                let content: String = match row.try_get("content") {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::debug!(table, error = %e, "interceptor reassemble: skip row (content)");
                        continue;
                    }
                };
                parts.push((class_code, prompt_uid as u32, name, content));
            }
        }

        // Sort by (class_code asc, prompt_uid asc).
        parts.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

        let mut buf = String::new();
        for (class_code, prompt_uid, name, content) in parts {
            buf.push_str(&format!(
                "\n\n## {class_code}:{prompt_uid}  {}  \"{name}\"\n\n{content}",
                class_label(class_code)
            ));
        }

        // Append the SempaiReviewOutcome JSON schema as a literal block so the
        // Sempai knows the expected output format even when no persona is set.
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

        Ok(buf)
    }
}

#[async_trait]
impl InterceptorConfigService for RebornInterceptorConfigService {
    async fn snapshot(
        &self,
        _caller: WebUiAuthenticatedCaller,
    ) -> Result<InterceptorConfigSnapshot, InterceptorConfigServiceError> {
        let kv = self.load_config().await;
        Ok(self.build_snapshot(&kv))
    }

    async fn update(
        &self,
        _caller: WebUiAuthenticatedCaller,
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
        let kv = self.load_config().await;
        Ok(self.build_snapshot(&kv))
    }

    async fn reassemble_base_prompt(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<InterceptorConfigSnapshot, InterceptorConfigServiceError> {
        let caller_id = caller.user_id.to_string();
        self.check_rate_limit(&self.reassemble_rate_limit, &caller_id)
            .await?;

        let assembled = self.do_reassemble().await?;
        let assembled_at = chrono::Utc::now().to_rfc3339();

        save_config_key(
            &self.pool,
            &self.tenant_id,
            KEY_BASE_PROMPT,
            &assembled,
            ConfigWriteContext::Operator,
        )
        .await
        .map_err(|e| InterceptorConfigServiceError::InvalidRequest {
            reason: format!("base prompt save: {e}"),
        })?;

        save_config_key(
            &self.pool,
            &self.tenant_id,
            KEY_BASE_PROMPT_ASSEMBLED_AT,
            &assembled_at,
            ConfigWriteContext::Operator,
        )
        .await
        .map_err(|e| InterceptorConfigServiceError::InvalidRequest {
            reason: format!("assembled_at save: {e}"),
        })?;

        let kv = self.load_config().await;
        Ok(self.build_snapshot(&kv))
    }

    async fn prewarm(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<InterceptorConfigSnapshot, InterceptorConfigServiceError> {
        let caller_id = caller.user_id.to_string();
        self.check_rate_limit(&self.prewarm_rate_limit, &caller_id)
            .await?;

        let kv = self.load_config().await;
        let base_prompt = kv
            .get(KEY_BASE_PROMPT)
            .cloned()
            .filter(|s| !s.is_empty())
            .ok_or(InterceptorConfigServiceError::BasePromptNotAssembled)?;

        let gateway = self
            .sempai_gateway
            .as_ref()
            .ok_or(InterceptorConfigServiceError::Unavailable)?;

        use brassclaw_loop_support::{
            HostManagedModelMessage, HostManagedModelMessageRole, HostManagedModelRequest,
        };
        use brassclaw_turns::{LoopMessageRef, TurnId, TurnRunId, run_profile::ModelProfileId};

        let profile_id = ModelProfileId::new("sempai_model").map_err(|e| {
            InterceptorConfigServiceError::InvalidRequest {
                reason: format!("model profile id: {e}"),
            }
        })?;
        let content_ref = LoopMessageRef::new("interceptor:prewarm".to_string()).map_err(|e| {
            InterceptorConfigServiceError::InvalidRequest {
                reason: format!("message ref: {e}"),
            }
        })?;
        let request = HostManagedModelRequest {
            model_profile_id: profile_id,
            messages: vec![HostManagedModelMessage {
                role: HostManagedModelMessageRole::System,
                content: base_prompt,
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
                reason: format!("prewarm gateway call: {e}"),
            }
        })?;

        let prewarm_at = chrono::Utc::now().to_rfc3339();
        save_config_key(
            &self.pool,
            &self.tenant_id,
            KEY_PREWARM_LAST_AT,
            &prewarm_at,
            ConfigWriteContext::Operator,
        )
        .await
        .map_err(|e| InterceptorConfigServiceError::InvalidRequest {
            reason: format!("prewarm_at save: {e}"),
        })?;

        let kv = self.load_config().await;
        Ok(self.build_snapshot(&kv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Phase B: class 22 (PythonCode) gets a title-case display label, matching
    /// the single-word entries (`"Tool"`, `"Action"`, `"Recipe"`) in this
    /// function. FIND-P6-07 / FIND-P7-06. `class_label` is private, so the
    /// assertion lives inside the module via `use super::*`.
    #[test]
    fn class_label_22_is_python_code() {
        assert_eq!(class_label(22), "PythonCode");
    }
}
