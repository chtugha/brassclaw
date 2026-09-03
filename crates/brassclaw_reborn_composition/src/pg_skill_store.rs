//! Postgres-backed seed/CRUD store for `reborn_skills` (class 1 leaf Skills).
//!
//! This is the seed/CRUD-side store used by the Phase C.2 builtin-host
//! component seed ([`crate::seed_builtin_host_components`]). Retrieval-side
//! projection of class 1-3 skills lives in
//! [`brassclaw_engine::memory::retrieval_source::PostgresSource`].
//!
//! # Column set
//!
//! Only the V027-verified columns are written: scope-4, `name`, `description`,
//! `body`, `class_code`, `consumer_tags`, `intent_examples`, `source`,
//! `validation_status`. `reborn_skills` does **not** have
//! `prior_knowledge_content` / `override_prompt_creation` (V027 lacks them;
//! V046 added them to 8 other component tables but not `reborn_skills`) — the
//! leaf-skill text lives in `body`, and solution-override is N/A for skills.
//! `intent_examples` is `JSONB NOT NULL DEFAULT '[]'` — modelled as a non-null
//! [`Value`] here (the seed passes `json!([])` when there are no examples).
//!
//! # Scope
//!
//! All queries are scoped by `(tenant_id, user_id, agent_id, project_id)`.
//! Builtins are seeded with `source = 'system'` + `validation_status =
//! 'validated'` (V066 widened the `source` CHECK to allow `'system'`) and are
//! surfaced tenant-globally by the retrieval UNION (Phase C.2 slice 1b).
//!
//! # Feature gate
//!
//! Compiles behind the `postgres` feature (mirrors `pg_tool_store`).

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

/// Errors raised by `reborn_skills` store operations.
#[derive(Debug, Error)]
pub(crate) enum PgSkillStoreError {
    #[error("pool error: {reason}")]
    Pool { reason: String },
    #[error("database error: {reason}")]
    Db { reason: String },
    #[error("skill not found: {name}")]
    NotFound { name: String },
}

fn map_pool(e: deadpool_postgres::PoolError) -> PgSkillStoreError {
    PgSkillStoreError::Pool {
        reason: e.to_string(),
    }
}

fn map_pg(e: tokio_postgres::Error) -> PgSkillStoreError {
    PgSkillStoreError::Db {
        reason: e.to_string(),
    }
}

/// Minimal data required to insert a new `reborn_skills` row.
///
/// `class_code` is supplied by the caller (1 = Rusty leaf skill for the
/// `host.*` stack). `prompt_uid` (sequence default), `tier` (`'seedling'`),
/// the reward/scoring columns (0), the activation columns (defaults),
/// `validation_errors` (`'{}'`), the lineage/audit columns (NULL/0) and
/// timestamps are set by DDL defaults — the caller does not supply them.
/// Builtins set `source = "system"` + `validation_status = "validated"`.
#[derive(Debug, Clone)]
pub(crate) struct NewPgSkill {
    pub(crate) tenant_id: String,
    pub(crate) user_id: String,
    pub(crate) agent_id: String,
    pub(crate) project_id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) body: String,
    pub(crate) class_code: i16,
    pub(crate) consumer_tags: Vec<String>,
    /// `JSONB NOT NULL DEFAULT '[]'` — pass `json!([])` when there are no
    /// examples (never `Null`: the column is NOT NULL).
    pub(crate) intent_examples: Value,
    pub(crate) source: String,
    pub(crate) validation_status: String,
}

/// Postgres-backed store for `reborn_skills` (classes 1-3).
#[derive(Clone)]
pub(crate) struct PgSkillStore {
    pool: Arc<PgPool>,
}

impl PgSkillStore {
    pub(crate) fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    /// Insert a new skill row idempotently.
    ///
    /// On a `(tenant_id, user_id, agent_id, project_id, name)` conflict the row
    /// is left untouched and `Ok(None)` is returned; the caller resolves the
    /// existing id via [`Self::get_id_by_name`]. `prompt_uid` defaults to the
    /// sequence, `tier` to `'seedling'`, scoring to 0, `validation_errors` to
    /// `'{}'`.
    pub(crate) async fn insert(
        &self,
        row: NewPgSkill,
    ) -> Result<Option<Uuid>, PgSkillStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let row = client
            .query_opt(
                "INSERT INTO reborn_skills
                    (tenant_id, user_id, agent_id, project_id,
                     name, description, body,
                     class_code, consumer_tags, intent_examples,
                     source, validation_status)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
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
                    &row.body,
                    &row.class_code,
                    &row.consumer_tags,
                    &row.intent_examples,
                    &row.source,
                    &row.validation_status,
                ],
            )
            .await
            .map_err(map_pg)?;
        Ok(row.map(|r| r.get(0)))
    }

    /// Resolve a skill's id by `(scope, name)`. Used by the seed to recover the
    /// existing id when [`Self::insert`] returns `Ok(None)` (idempotent re-seed).
    pub(crate) async fn get_id_by_name(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        name: &str,
    ) -> Result<Option<Uuid>, PgSkillStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let row = client
            .query_opt(
                "SELECT id FROM reborn_skills
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
