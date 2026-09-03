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

// Phase-C.2 store — the insert/lookup surface is exercised by the boot seed
// (`seed_builtin_host_components`). Mirrors the `pg_tool_store` lean-insert
// pattern; full CRUD lands later if a non-seed authoring path needs it.
#![allow(dead_code)]
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
    #[error("tool_skill not found: {name}")]
    NotFound { name: String },
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
        let includes_json = serde_json::to_value(&row.includes).map_err(|e| {
            PgToolSkillStoreError::Db {
                reason: format!("includes encode failed: {e}"),
            }
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
                     includes)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17)
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
