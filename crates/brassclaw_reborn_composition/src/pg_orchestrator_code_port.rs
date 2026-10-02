//! Postgres-backed implementation of [`OrchestratorCodePort`].
//!
//! Queries `reborn_skills` (class 10) for the active orchestrator code body.
//!
//! # Feature gate
//!
//! This module is compiled only when both `postgres` and `skills-db` features
//! are active (the gate in `lib.rs`).  `skills-db` is required because the
//! consumer (`runtime.rs`) constructs the port inside a `#[cfg(feature =
//! "skills-db")]` block.

#![forbid(unsafe_code)]

// The items below are only compiled when both `postgres` and `skills-db` are
// active.  `lib.rs` gates this entire module the same way, so the #[cfg]
// attributes here are defence-in-depth only.

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_engine::executor::{OrchestratorCodeError, OrchestratorCodePort};
use brassclaw_pg::PgPool;

/// Postgres-backed [`OrchestratorCodePort`].
///
/// Queries `reborn_skills` for a `name = 'orchestrator:main'`,
/// `class_code = 10`, `validation_status = 'validated'` row.
///
/// When `allow_self_modify` is `false`: returns the `source='system'` row
/// only (safe default for production).
///
/// When `allow_self_modify` is `true`: prefers a validated non-system
/// (operator-customised) row if one exists; falls back to the system row.
pub(crate) struct PgOrchestratorCodePort {
    pool: Arc<PgPool>,
    tenant_id: String,
}

impl PgOrchestratorCodePort {
    pub(crate) fn new(pool: Arc<PgPool>, tenant_id: impl Into<String>) -> Self {
        Self {
            pool,
            tenant_id: tenant_id.into(),
        }
    }
}

#[async_trait]
impl OrchestratorCodePort for PgOrchestratorCodePort {
    async fn load_orchestrator_code(
        &self,
        allow_self_modify: bool,
    ) -> Result<String, OrchestratorCodeError> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|e| OrchestratorCodeError::Store {
                reason: e.to_string(),
            })?;

        let row = if allow_self_modify {
            // Prefer validated non-system (operator-customised) row; fall
            // back to the system row. ORDER BY (source = 'system') ASC puts
            // non-system rows first.
            client
                .query_opt(
                    "SELECT body FROM reborn_skills
                      WHERE tenant_id  = $1
                        AND name       = 'orchestrator:main'
                        AND class_code = 10
                        AND validation_status = 'validated'
                      ORDER BY (source = 'system') ASC,
                               updated_at DESC
                      LIMIT 1",
                    &[&self.tenant_id],
                )
                .await
        } else {
            client
                .query_opt(
                    "SELECT body FROM reborn_skills
                      WHERE tenant_id  = $1
                        AND name       = 'orchestrator:main'
                        AND class_code = 10
                        AND source     = 'system'
                        AND validation_status = 'validated'
                      LIMIT 1",
                    &[&self.tenant_id],
                )
                .await
        }
        .map_err(|e| OrchestratorCodeError::Store {
            reason: e.to_string(),
        })?;

        row.map(|r| r.get::<_, String>(0))
            .ok_or(OrchestratorCodeError::NotFound)
    }
}
