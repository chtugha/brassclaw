//! Postgres-backed store for `reborn_tools` (class 0 — Rusty Tools).
//!
//! This is the seed/CRUD-side store used by the Phase C.2 builtin-host
//! component seed ([`crate::seed_builtin_host_components`]). Retrieval-side
//! projection of class-0 tools lives in
//! [`brassclaw_engine::capability::db_tool_source::DbToolSource`] (the
//! `ToolRegistryStore` impl that lists validated tool names).
//!
//! # Scope
//!
//! All queries are scoped by `(tenant_id, user_id, agent_id, project_id)`.
//! Builtins are seeded with `source = 'system'` + `validation_status =
//! 'validated'` and are surfaced tenant-globally by the retrieval UNION
//! (Phase C.2 slice 1b) — the seed's marker scope is just the row's storage
//! key (part of the `reborn_tools_scope_name_unique` unique tuple).
//!
//! # Feature gate
//!
//! Compiles behind the `postgres` feature (mirrors `pg_recipe_store`).

// Phase-C.2 store — the insert/lookup surface is exercised by the boot seed
// (`seed_builtin_host_components`). Mirrors the `pg_recipe_store` lean-insert
// pattern; full CRUD lands later if a non-seed authoring path needs it.
#![allow(dead_code)]
#![forbid(unsafe_code)]

use std::sync::Arc;

use brassclaw_pg::PgPool;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

/// Errors raised by `reborn_tools` store operations.
#[derive(Debug, Error)]
pub(crate) enum PgToolStoreError {
    #[error("pool error: {reason}")]
    Pool { reason: String },
    #[error("database error: {reason}")]
    Db { reason: String },
    #[error("tool not found: {name}")]
    NotFound { name: String },
}

fn map_pool(e: deadpool_postgres::PoolError) -> PgToolStoreError {
    PgToolStoreError::Pool {
        reason: e.to_string(),
    }
}

fn map_pg(e: tokio_postgres::Error) -> PgToolStoreError {
    PgToolStoreError::Db {
        reason: e.to_string(),
    }
}

/// Minimal data required to insert a new `reborn_tools` row.
///
/// `class_code` (0), `prompt_uid` (sequence default), `validation_errors`
/// (`'{}'`), the lineage/audit columns (NULL/0) and timestamps are set by DDL
/// defaults — the caller does not supply them. Builtins set
/// `source = "system"` + `validation_status = "validated"`.
#[derive(Debug, Clone)]
pub(crate) struct NewPgTool {
    pub(crate) tenant_id: String,
    pub(crate) user_id: String,
    pub(crate) agent_id: String,
    pub(crate) project_id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) param_schema: Option<Value>,
    pub(crate) param_template: Option<Value>,
    pub(crate) effect_type: String,
    pub(crate) preconditions: Option<String>,
    pub(crate) error_handling: Option<String>,
    pub(crate) consumer_tags: Vec<String>,
    pub(crate) source: String,
    pub(crate) validation_status: String,
    /// C.4.5.4 — the Executioner's dispatch identifier (the rust-side common
    /// form of a Tool). For built-in tools the static `match call.function_name`
    /// key (e.g. "builtin.shell"); for host.* bridge tools the `host.X` name
    /// (e.g. "host.resolve_intent"). Validated non-empty by the Q1 gate
    /// (component_validator class-0 arm — `tool_name` carries the capability_id).
    pub(crate) capability_id: String,
}

/// Postgres-backed store for `reborn_tools` (class 0).
#[derive(Clone)]
pub(crate) struct PgToolStore {
    pool: Arc<PgPool>,
}

impl PgToolStore {
    pub(crate) fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    /// Insert a new tool row idempotently.
    ///
    /// On a `(tenant_id, user_id, agent_id, project_id, name)` conflict the row
    /// is left untouched and `Ok(None)` is returned; the caller resolves the
    /// existing id via [`Self::get_id_by_name`]. `class_code` defaults to 0,
    /// `prompt_uid` to the sequence, `validation_errors` to `'{}'`.
    pub(crate) async fn insert(
        &self,
        row: NewPgTool,
    ) -> Result<Option<Uuid>, PgToolStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let row = client
            .query_opt(
                "INSERT INTO reborn_tools
                    (tenant_id, user_id, agent_id, project_id,
                     name, description, param_schema, param_template,
                     effect_type, preconditions, error_handling,
                     consumer_tags, source, validation_status,
                     capability_id)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)
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
                    &row.param_schema,
                    &row.param_template,
                    &row.effect_type,
                    &row.preconditions,
                    &row.error_handling,
                    &row.consumer_tags,
                    &row.source,
                    &row.validation_status,
                    &row.capability_id,
                ],
            )
            .await
            .map_err(map_pg)?;
        Ok(row.map(|r| r.get(0)))
    }

    /// Resolve a tool's id by `(scope, name)`. Used by the seed to recover the
    /// existing id when [`Self::insert`] returns `Ok(None)` (idempotent re-seed).
    pub(crate) async fn get_id_by_name(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        name: &str,
    ) -> Result<Option<Uuid>, PgToolStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let row = client
            .query_opt(
                "SELECT id FROM reborn_tools
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
