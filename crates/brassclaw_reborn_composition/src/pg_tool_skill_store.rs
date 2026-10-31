//! Postgres-backed store for `reborn_tool_skills` (class 13 — ToolSkills).
//!
//! This is the seed/CRUD-side store used by the Phase C.2 builtin-host
//! component seed ([`crate::seed_builtin_host_components`]). Retrieval-side
//! projection of class-13 tool skills lives in
//! [`brassclaw_engine::memory::retrieval_source::PostgresSource`].
//!
//! # Scope
//!
//! All queries are scoped by `(tenant_id, user_id, agent_id, project_id)`.
//! Builtins are seeded with `source = 'system'` + `validation_status =
//! 'validated'` and are surfaced tenant-globally by the retrieval UNION
//! (Phase C.2 slice 1b).
//!
//! # Feature gate
//!
//! Compiles behind the `postgres` feature (mirrors `pg_recipe_store`).

#![forbid(unsafe_code)]

use std::sync::Arc;

use brassclaw_pg::PgPool;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

/// Errors raised by `reborn_tool_skills` store operations.
#[derive(Debug, Error)]
pub(crate) enum PgToolSkillStoreError {
    #[error("pool error: {reason}")]
    Pool { reason: String },
    #[error("database error: {reason}")]
    Db { reason: String },
}

fn map_pool(e: deadpool_postgres::PoolError) -> PgToolSkillStoreError {
    PgToolSkillStoreError::Pool {
        reason: e.to_string(),
    }
}

fn map_pg(e: tokio_postgres::Error) -> PgToolSkillStoreError {
    PgToolSkillStoreError::Db {
        reason: e.to_string(),
    }
}

/// Minimal data required to insert a new `reborn_tool_skills` row.
///
/// `class_code` (13), `prompt_uid` (sequence default), `tier` (`'seedling'`),
/// the reward/scoring columns (0), `validation_errors` (`'{}'`), the
/// lineage/audit columns (NULL/0) and timestamps are set by DDL defaults — the
/// caller does not supply them. Builtins set `source = "system"` +
/// `validation_status = "validated"`.
#[derive(Debug, Clone)]
pub(crate) struct NewPgToolSkill {
    pub(crate) tenant_id: String,
    pub(crate) user_id: String,
    pub(crate) agent_id: String,
    pub(crate) project_id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) content: String,
    pub(crate) prior_knowledge_content: Option<String>,
    pub(crate) override_prompt_creation: bool,
    pub(crate) tool_name: Option<String>,
    pub(crate) param_schema: Option<Value>,
    pub(crate) param_template: Option<Value>,
    pub(crate) consumer_tags: Vec<String>,
    pub(crate) intent_examples: Option<Value>,
    pub(crate) source: String,
    pub(crate) validation_status: String,
    /// C.4.5.3 — component UUIDs the composer inlines for `{{component_name}}`
    /// structural-include placeholders in this ToolSkill's description. Empty
    /// for leaf descriptions.
    pub(crate) includes: Vec<Uuid>,
    /// SHA-256 hex digest of `content`. Set to `Some(checksum)` for
    /// `source='system'` rows that participate in the boot integrity check.
    /// `None` for user-authored rows and rows not covered by the check.
    pub(crate) content_checksum: Option<String>,
}

/// Postgres-backed store for `reborn_tool_skills` (class 13).
#[derive(Clone)]
pub(crate) struct PgToolSkillStore {
    pool: Arc<PgPool>,
}

impl PgToolSkillStore {
    pub(crate) fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    /// Administrative inventory, with the same exact scope as the Recipe facade.
    /// Pending/rejected rows remain visible here; this is not execution admission.
    pub(crate) async fn list_summaries(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
    ) -> Result<Vec<brassclaw_product_workflow::ToolSkillSummary>, PgToolSkillStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let rows = client
            .query(
                "SELECT id, name, tool_name, description,
                        COALESCE(prior_knowledge_content, content) AS token_content,
                        usage_count, success_count, failure_count, wilson_lower,
                        tier, validation_status, source, created_at, updated_at
                 FROM reborn_tool_skills
                 WHERE tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4
                 ORDER BY prompt_uid ASC, id ASC",
                &[&tenant_id, &user_id, &agent_id, &project_id],
            )
            .await
            .map_err(map_pg)?;
        rows.iter()
            .map(|row| {
                let id: Uuid = row.try_get("id").map_err(map_pg)?;
                let tool_name: Option<String> = row.try_get("tool_name").map_err(map_pg)?;
                let content: String = row.try_get("token_content").map_err(map_pg)?;
                let usage: i32 = row.try_get("usage_count").map_err(map_pg)?;
                let success: i32 = row.try_get("success_count").map_err(map_pg)?;
                let failure: i32 = row.try_get("failure_count").map_err(map_pg)?;
                let created: chrono::DateTime<chrono::Utc> =
                    row.try_get("created_at").map_err(map_pg)?;
                let updated: chrono::DateTime<chrono::Utc> =
                    row.try_get("updated_at").map_err(map_pg)?;
                // V037 has no category/token-count columns. Use the component
                // kind and shared estimator, not invented persisted values or caps.
                let tokens = brassclaw_loop_support::estimate_tokens_from_chars(&content).as_u64();
                Ok(brassclaw_product_workflow::ToolSkillSummary {
                    id: id.to_string(),
                    name: row.try_get("name").map_err(map_pg)?,
                    tool_name: tool_name.unwrap_or_default(),
                    description: row.try_get("description").map_err(map_pg)?,
                    category: "tool_skill".into(),
                    estimated_tokens: u32::try_from(tokens).unwrap_or(u32::MAX),
                    usage_count: usage.max(0) as u64,
                    success_count: success.max(0) as u64,
                    failure_count: failure.max(0) as u64,
                    wilson_lower: row.try_get("wilson_lower").map_err(map_pg)?,
                    tier: row.try_get("tier").map_err(map_pg)?,
                    validation_status: row.try_get("validation_status").map_err(map_pg)?,
                    source: row.try_get("source").map_err(map_pg)?,
                    created_at: created.to_rfc3339(),
                    updated_at: updated.to_rfc3339(),
                })
            })
            .collect()
    }

    /// Return actual class-13 metadata, without treating text as executable code
    /// or manufacturing the legacy engine ToolSkill's absent schema fields.
    pub(crate) async fn get_detail(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        id: Uuid,
    ) -> Result<Option<brassclaw_product_workflow::ToolSkillDetail>, PgToolSkillStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let row = client
            .query_opt(
                "SELECT jsonb_build_object(
                    'id', id, 'name', name, 'description', description, 'content', content,
                    'prior_knowledge_content', prior_knowledge_content,
                    'override_prompt_creation', override_prompt_creation,
                    'tool_name', tool_name, 'param_schema', param_schema,
                    'param_template', param_template, 'class_code', class_code,
                    'prompt_uid', prompt_uid, 'consumer_tags', consumer_tags,
                    'intent_examples', intent_examples, 'includes', includes,
                    'dependency_registry', dependency_registry,
                    'formatted_content', formatted_content, 'tier', tier,
                    'usage_count', usage_count, 'success_count', success_count,
                    'failure_count', failure_count, 'wilson_lower', wilson_lower,
                    'validation_status', validation_status, 'source', source,
                    'content_hash', content_hash, 'content_checksum', content_checksum,
                    'created_at', created_at, 'updated_at', updated_at) AS tool_skill
                 FROM reborn_tool_skills
                 WHERE id=$1 AND tenant_id=$2 AND user_id=$3 AND agent_id=$4 AND project_id=$5",
                &[&id, &tenant_id, &user_id, &agent_id, &project_id],
            )
            .await
            .map_err(map_pg)?;
        row.map(|row| {
            Ok(brassclaw_product_workflow::ToolSkillDetail {
                id: id.to_string(),
                tool_skill: row.try_get("tool_skill").map_err(map_pg)?,
            })
        })
        .transpose()
    }

    /// Store the caller's requested review state exactly. A system source label
    /// never promotes pending metadata. This legacy projection is not v3 approval;
    /// installation bootstrap and authored qualification retain separate evidence.
    /// Insert a new tool_skill row idempotently.
    ///
    /// On a `(tenant_id, user_id, agent_id, project_id, name)` conflict the row
    /// is left untouched and `Ok(None)` is returned; the caller resolves the
    /// existing id via [`Self::get_id_by_name`]. `class_code` defaults to 13,
    /// `prompt_uid` to the sequence, `tier` to `'seedling'`, scoring to 0.
    pub(crate) async fn insert(
        &self,
        row: NewPgToolSkill,
    ) -> Result<Option<Uuid>, PgToolSkillStoreError> {
        let includes_json =
            serde_json::to_value(&row.includes).map_err(|e| PgToolSkillStoreError::Db {
                reason: format!("includes encode failed: {e}"),
            })?;
        let client = self.pool.get().await.map_err(map_pool)?;
        let row = client
            .query_opt(
                "INSERT INTO reborn_tool_skills
                    (tenant_id, user_id, agent_id, project_id,
                     name, description, content,
                     prior_knowledge_content, override_prompt_creation,
                     tool_name, param_schema, param_template,
                     consumer_tags, intent_examples, source, validation_status,
                     includes, content_checksum)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,
                         $16,$17,$18)
                 ON CONFLICT (tenant_id, user_id, agent_id, project_id, name)
                 DO NOTHING
                 RETURNING id",
                &[
                    &row.tenant_id,
                    &row.user_id,
                    &row.agent_id,
                    &row.project_id,
                    &row.name,
                    &row.description,
                    &row.content,
                    &row.prior_knowledge_content,
                    &row.override_prompt_creation,
                    &row.tool_name,
                    &row.param_schema,
                    &row.param_template,
                    &row.consumer_tags,
                    &row.intent_examples,
                    &row.source,
                    &row.validation_status,
                    &includes_json,
                    &row.content_checksum,
                ],
            )
            .await
            .map_err(map_pg)?;
        Ok(row.map(|r| r.get(0)))
    }

    /// Resolve a tool_skill's id by `(scope, name)`. Used by the seed to recover
    /// the existing id when [`Self::insert`] returns `Ok(None)` (idempotent
    /// re-seed).
    pub(crate) async fn get_id_by_name(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        name: &str,
    ) -> Result<Option<Uuid>, PgToolSkillStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let row = client
            .query_opt(
                "SELECT id FROM reborn_tool_skills
                 WHERE name = $1
                   AND tenant_id = $2 AND user_id = $3
                   AND agent_id  = $4 AND project_id = $5
                 LIMIT 1",
                &[&name, &tenant_id, &user_id, &agent_id, &project_id],
            )
            .await
            .map_err(map_pg)?;
        Ok(row.map(|r| r.get(0)))
    }
}
