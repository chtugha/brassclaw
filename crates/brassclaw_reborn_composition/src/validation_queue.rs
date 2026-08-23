//! Validation queue store — `reborn_validation_queue` (Phase A.5, Decision 2).
//!
//! The queue tracks every component through Q1 (deterministic structural
//! validation) and Q2 (human / Sempai review) before it graduates to
//! `validation_status = 'validated'`. It lands in Phase A.5 — ahead of Phase N
//! — so every component class (including the new class 22 from Phase B and
//! class 23 from Phase C) can enqueue from its very first WebUI-authored save.
//!
//! # State machine (§0.18)
//!
//! ```text
//!   1 = Q1_pending      2 = Q1_passed (awaiting Q2)   3 = rejected   4 = deletion_candidate
//! ```
//!
//! # State-2 write invariant (FIND-P9-08)
//!
//! Only [`ValidationQueueStore::gate1_pass`] (`pub(crate)`) writes state 2 —
//! the sole write path, enforced by Rust visibility. Any other writer of
//! state 2 is a security bug. The Q2 reviewer approves from state 2 →
//! [`ValidationQueueStore::approve`] deletes the row (graduation) in ONE
//! transaction with the component-table UPDATE (FIND-P9-05).
//!
//! # Upgrade model (§0.23.5)
//!
//! [`ValidationQueueStore::submit`] carries `proposed_payload: Option<Value>`
//! (set for upgrades, `None` for new-component submissions). The graduation
//! *apply* of `proposed_payload` (overwrite the live validated row) is wired
//! in Phase N (§0.23.9). Phase A.5 `approve` implements the new-component
//! graduation path only and **errors** on a non-null `proposed_payload` to
//! avoid silently dropping an upgrade (Q1 answer — defer per plan).
//!
//! # Feature gate
//!
//! Requires the `postgres` feature.

// Phase A.5 wiring into WebUI-save (Phase B/C) + boot-integrity (Phase N)
// lands later; the store API itself is complete and tested here.
#![allow(dead_code)]
#![forbid(unsafe_code)]

use std::sync::Arc;

use brassclaw_engine::memory::retrieval_source::ComponentScope;
use brassclaw_pg::PgPool;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

/// Default rejection threshold: after 3 rejections a row auto-promotes to
/// state 4 (deletion candidate) — §0.18. Configurable per-store via
/// [`ValidationQueueStore::with_reject_threshold`] (Q2 answer — construction-time
/// field; Phase K/N can later wire it from `reborn_monty_vm_settings`).
pub const DEFAULT_REJECT_THRESHOLD: u8 = 3;

/// State: just submitted, awaiting Gate 1.
pub const STATE_Q1_PENDING: i16 = 1;
/// State: Gate 1 clean — awaiting Q2 manual review.
pub const STATE_Q1_PASSED: i16 = 2;
/// State: Q2 rejected (author may fix + resubmit).
pub const STATE_REJECTED: i16 = 3;
/// State: deletion candidate (counter ≥ threshold or manually condemned).
pub const STATE_DELETION_CANDIDATE: i16 = 4;

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/// Errors raised by `reborn_validation_queue` store operations.
#[derive(Debug, Error)]
pub enum ValidationQueueError {
    #[error("pool error: {reason}")]
    Pool { reason: String },
    #[error("database error: {reason}")]
    Db { reason: String },
    #[error("component {component_id} is already in the validation queue")]
    AlreadyQueued { component_id: Uuid },
    #[error("unknown component class {class_code} — no target component table")]
    UnknownClass { class_code: i32 },
    #[error(
        "component {component_id} disappeared during approve (rolled back, queue row preserved)"
    )]
    ComponentMissing { component_id: Uuid },
    #[error(
        "upgrade-copy graduation for component {component_id} lands in Phase N \
         (proposed_payload is set); refusing to silently drop the upgrade"
    )]
    UpgradeGraduationNotImplemented { component_id: Uuid },
    #[error(
        "queue row for component {component_id} is not in state 2 (Q1_passed); current state {state}"
    )]
    NotQ1Passed { component_id: Uuid, state: i16 },
    #[error("queue row for component {component_id} not found (or not in the expected state)")]
    NotFound { component_id: Uuid },
}

fn map_pool(e: deadpool_postgres::PoolError) -> ValidationQueueError {
    ValidationQueueError::Pool {
        reason: e.to_string(),
    }
}

fn map_pg(e: tokio_postgres::Error) -> ValidationQueueError {
    ValidationQueueError::Db {
        reason: e.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Row type
// ---------------------------------------------------------------------------

/// A decoded `reborn_validation_queue` row (without scope — the caller knows
/// the scope it queried). Returned by [`ValidationQueueStore::list`].
#[derive(Debug, Clone)]
pub struct QueueRow {
    pub id: Uuid,
    pub component_id: Uuid,
    pub component_class: i16,
    pub state: i16,
    pub counter: i32,
    pub review_feedback: Option<String>,
    pub validation_errors: Vec<String>,
    pub proposed_payload: Option<Value>,
    pub submitted_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

/// Postgres-backed store for `reborn_validation_queue` (Phase A.5).
#[derive(Clone)]
pub struct ValidationQueueStore {
    pool: Arc<PgPool>,
    reject_threshold: u8,
}

impl ValidationQueueStore {
    /// Create a store with the default rejection threshold (3).
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self {
            pool,
            reject_threshold: DEFAULT_REJECT_THRESHOLD,
        }
    }

    /// Create a store with an explicit rejection threshold (§0.18: configurable,
    /// default 3). Used by tests and, later, by Phase K/N wiring that reads the
    /// threshold from `reborn_monty_vm_settings`.
    pub fn with_reject_threshold(pool: Arc<PgPool>, reject_threshold: u8) -> Self {
        Self {
            pool,
            reject_threshold,
        }
    }

    /// Submit a component to the Q1 queue (state 1).
    ///
    /// `proposed_payload` is `Some` for upgrades (edit of a validated
    /// component — the live validated row stays served while the copy is
    /// queued) and `None` for new-component submissions (§0.23.5).
    ///
    /// Returns [`ValidationQueueError::AlreadyQueued`] if a row already exists
    /// for `(scope, component_id)` — the queue's `UNIQUE(scope, component_id)`
    /// holds, so one pending upgrade per component at a time (concurrent edits
    /// are rejected while a copy is queued).
    pub async fn submit(
        &self,
        scope: &ComponentScope,
        component_id: Uuid,
        component_class: i32,
        proposed_payload: Option<Value>,
    ) -> Result<(), ValidationQueueError> {
        let class_i16: i16 =
            component_class
                .try_into()
                .map_err(|_| ValidationQueueError::UnknownClass {
                    class_code: component_class,
                })?;
        let client = self.pool.get().await.map_err(map_pool)?;
        let row = client
            .query_opt(
                "INSERT INTO reborn_validation_queue
                     (tenant_id, user_id, agent_id, project_id,
                      component_id, component_class, state, proposed_payload)
                 VALUES ($1, $2, $3, $4, $5, $6, 1, $7)
                 ON CONFLICT
                     (tenant_id, user_id, agent_id, project_id, component_id)
                 DO NOTHING
                 RETURNING id",
                &[
                    &scope.tenant_id,
                    &scope.user_id,
                    &scope.agent_id,
                    &scope.project_id,
                    &component_id,
                    &class_i16,
                    &proposed_payload,
                ],
            )
            .await
            .map_err(map_pg)?;
        if row.is_none() {
            return Err(ValidationQueueError::AlreadyQueued { component_id });
        }
        Ok(())
    }

    /// Gate 1 clean pass: transition `state 1 → state 2` and clear Q1 errors.
    ///
    /// `pub(crate)` — the ONLY write path for state 2 (FIND-P9-08). The Q1
    /// orchestration ([`crate::q1_orchestrator::run_q1_validation`]) lives in
    /// this crate and is the sole legitimate caller.
    pub(crate) async fn gate1_pass(
        &self,
        scope: &ComponentScope,
        component_id: Uuid,
        errors: &[String],
    ) -> Result<(), ValidationQueueError> {
        let cleared: Vec<String> = errors.to_vec();
        let client = self.pool.get().await.map_err(map_pool)?;
        let n = client
            .execute(
                "UPDATE reborn_validation_queue
                 SET state = 2,
                     validation_errors = $6,
                     updated_at = now()
                 WHERE tenant_id = $1 AND user_id = $2
                   AND agent_id = $3 AND project_id = $4
                   AND component_id = $5
                   AND state = 1",
                &[
                    &scope.tenant_id,
                    &scope.user_id,
                    &scope.agent_id,
                    &scope.project_id,
                    &component_id,
                    &cleared,
                ],
            )
            .await
            .map_err(map_pg)?;
        if n == 0 {
            return Err(ValidationQueueError::NotFound { component_id });
        }
        Ok(())
    }

    /// Record a Q1 failure: stays in `state 1`, populates `validation_errors`,
    /// increments nothing (the author must fix and resubmit — §0.18).
    ///
    /// `pub(crate)` — paired with [`Self::gate1_pass`]; only the Q1
    /// orchestration in this crate calls it.
    pub(crate) async fn gate1_fail(
        &self,
        scope: &ComponentScope,
        component_id: Uuid,
        errors: &[String],
    ) -> Result<(), ValidationQueueError> {
        let recorded: Vec<String> = errors.to_vec();
        let client = self.pool.get().await.map_err(map_pool)?;
        let n = client
            .execute(
                "UPDATE reborn_validation_queue
                 SET validation_errors = $6,
                     updated_at = now()
                 WHERE tenant_id = $1 AND user_id = $2
                   AND agent_id = $3 AND project_id = $4
                   AND component_id = $5
                   AND state = 1",
                &[
                    &scope.tenant_id,
                    &scope.user_id,
                    &scope.agent_id,
                    &scope.project_id,
                    &component_id,
                    &recorded,
                ],
            )
            .await
            .map_err(map_pg)?;
        if n == 0 {
            return Err(ValidationQueueError::NotFound { component_id });
        }
        Ok(())
    }

    /// Q2 rejection: `state 2 → state 3`, increment `counter`, store feedback.
    /// Auto-promotes to `state 4` (deletion candidate) when the incremented
    /// counter reaches this store's rejection threshold (§0.18).
    pub async fn reject(
        &self,
        scope: &ComponentScope,
        component_id: Uuid,
        feedback: &str,
    ) -> Result<(), ValidationQueueError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let row = client
            .query_opt(
                "UPDATE reborn_validation_queue
                 SET state = 3,
                     counter = counter + 1,
                     review_feedback = $6,
                     updated_at = now()
                 WHERE tenant_id = $1 AND user_id = $2
                   AND agent_id = $3 AND project_id = $4
                   AND component_id = $5
                   AND state = 2
                 RETURNING counter",
                &[
                    &scope.tenant_id,
                    &scope.user_id,
                    &scope.agent_id,
                    &scope.project_id,
                    &component_id,
                    &feedback,
                ],
            )
            .await
            .map_err(map_pg)?;
        let Some(row) = row else {
            // Not in state 2 — read the current state for a precise error.
            let cur = client
                .query_opt(
                    "SELECT state FROM reborn_validation_queue
                     WHERE tenant_id = $1 AND user_id = $2
                       AND agent_id = $3 AND project_id = $4
                       AND component_id = $5",
                    &[
                        &scope.tenant_id,
                        &scope.user_id,
                        &scope.agent_id,
                        &scope.project_id,
                        &component_id,
                    ],
                )
                .await
                .map_err(map_pg)?;
            return match cur {
                Some(r) => Err(ValidationQueueError::NotQ1Passed {
                    component_id,
                    state: r.get(0),
                }),
                None => Err(ValidationQueueError::NotFound { component_id }),
            };
        };
        let counter_after: i32 = row.get(0);
        if next_reject_state(counter_after, self.reject_threshold) == STATE_DELETION_CANDIDATE {
            let n = client
                .execute(
                    "UPDATE reborn_validation_queue
                     SET state = 4, updated_at = now()
                     WHERE tenant_id = $1 AND user_id = $2
                       AND agent_id = $3 AND project_id = $4
                       AND component_id = $5",
                    &[
                        &scope.tenant_id,
                        &scope.user_id,
                        &scope.agent_id,
                        &scope.project_id,
                        &component_id,
                    ],
                )
                .await
                .map_err(map_pg)?;
            if n == 0 {
                return Err(ValidationQueueError::NotFound { component_id });
            }
        }
        Ok(())
    }

    /// Q2 approval = graduation: flip the component row to
    /// `validation_status = 'validated'` and delete the queue row, in ONE
    /// transaction (FIND-P9-05).
    ///
    /// Phase A.5 implements the **new-component** graduation path only. A row
    /// carrying a non-null `proposed_payload` (an upgrade copy) is refused with
    /// [`ValidationQueueError::UpgradeGraduationNotImplemented`] — the upgrade
    /// *apply* logic lands in Phase N (§0.23.9), and silently graduating an
    /// upgrade row here would drop the edited payload (Q1 answer — defer per
    /// plan).
    ///
    /// Returns `Ok(component_id)` on success.
    pub async fn approve(
        &self,
        scope: &ComponentScope,
        component_id: Uuid,
    ) -> Result<Uuid, ValidationQueueError> {
        let mut client = self.pool.get().await.map_err(map_pool)?;

        // (0) Read the queue row before any transaction: state + upgrade
        // payload + class. These are plain reads, not part of the graduation tx.
        let row = client
            .query_opt(
                "SELECT state, proposed_payload, component_class
                 FROM reborn_validation_queue
                 WHERE tenant_id = $1 AND user_id = $2
                   AND agent_id = $3 AND project_id = $4
                   AND component_id = $5",
                &[
                    &scope.tenant_id,
                    &scope.user_id,
                    &scope.agent_id,
                    &scope.project_id,
                    &component_id,
                ],
            )
            .await
            .map_err(map_pg)?;
        let Some(row) = row else {
            return Err(ValidationQueueError::NotFound { component_id });
        };
        let state: i16 = row.get(0);
        if state != STATE_Q1_PASSED {
            return Err(ValidationQueueError::NotQ1Passed {
                component_id,
                state,
            });
        }
        let proposed_payload: Option<Value> = row.get(1);
        if proposed_payload.is_some() {
            return Err(ValidationQueueError::UpgradeGraduationNotImplemented { component_id });
        }
        let class_code: i16 = row.get(2);

        // (1) Resolve the target table BEFORE BEGIN — no wasted BEGIN on an
        // unknown class (FIND-P9-05). `resolve_component_table` returns a
        // static literal, so interpolating it into the UPDATE is safe (no
        // user-supplied identifier).
        let table = resolve_component_table(class_code as i32).ok_or(
            ValidationQueueError::UnknownClass {
                class_code: class_code as i32,
            },
        )?;

        // (2) BEGIN — UPDATE the component, then DELETE the queue row.
        // Ordering: UPDATE before DELETE so the graduation trigger (Phase N)
        // fires only after the component is already validated — no window where
        // the queue row is gone but the component is still `pending`.
        let tx = client.transaction().await.map_err(map_pg)?;

        let update_sql = format!(
            "UPDATE {table}
             SET validation_status = 'validated', updated_at = now()
             WHERE id = $1
               AND tenant_id = $2 AND user_id = $3
               AND agent_id = $4 AND project_id = $5"
        );
        let updated = tx
            .execute(
                update_sql.as_str(),
                &[
                    &component_id,
                    &scope.tenant_id,
                    &scope.user_id,
                    &scope.agent_id,
                    &scope.project_id,
                ],
            )
            .await
            .map_err(map_pg)?;
        if updated == 0 {
            // Component disappeared — ROLLBACK, queue row preserved (FIND-P9-05).
            tx.rollback().await.map_err(map_pg)?;
            return Err(ValidationQueueError::ComponentMissing { component_id });
        }

        let deleted = tx
            .execute(
                "DELETE FROM reborn_validation_queue
                 WHERE tenant_id = $1 AND user_id = $2
                   AND agent_id = $3 AND project_id = $4
                   AND component_id = $5",
                &[
                    &scope.tenant_id,
                    &scope.user_id,
                    &scope.agent_id,
                    &scope.project_id,
                    &component_id,
                ],
            )
            .await
            .map_err(map_pg)?;
        if deleted == 0 {
            // Queue row vanished between the read and the tx — ROLLBACK.
            tx.rollback().await.map_err(map_pg)?;
            return Err(ValidationQueueError::NotFound { component_id });
        }

        tx.commit().await.map_err(map_pg)?;
        Ok(component_id)
    }

    /// List queue rows for a scope, optionally filtered by state (WebUI
    /// validation view). Ordered by `submitted_at`.
    pub async fn list(
        &self,
        scope: &ComponentScope,
        state_filter: Option<u8>,
    ) -> Result<Vec<QueueRow>, ValidationQueueError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let rows = if let Some(state) = state_filter {
            let state_i16: i16 = state as i16;
            client
                .query(
                    "SELECT id, component_id, component_class, state, counter,
                            review_feedback, validation_errors, proposed_payload,
                            submitted_at, updated_at
                     FROM reborn_validation_queue
                     WHERE tenant_id = $1 AND user_id = $2
                       AND agent_id = $3 AND project_id = $4
                       AND state = $5
                     ORDER BY submitted_at",
                    &[
                        &scope.tenant_id,
                        &scope.user_id,
                        &scope.agent_id,
                        &scope.project_id,
                        &state_i16,
                    ],
                )
                .await
                .map_err(map_pg)?
        } else {
            client
                .query(
                    "SELECT id, component_id, component_class, state, counter,
                            review_feedback, validation_errors, proposed_payload,
                            submitted_at, updated_at
                     FROM reborn_validation_queue
                     WHERE tenant_id = $1 AND user_id = $2
                       AND agent_id = $3 AND project_id = $4
                     ORDER BY submitted_at",
                    &[
                        &scope.tenant_id,
                        &scope.user_id,
                        &scope.agent_id,
                        &scope.project_id,
                    ],
                )
                .await
                .map_err(map_pg)?
        };
        rows.into_iter().map(decode_queue_row).collect()
    }

    /// Deletion-candidate cleanup: delete `state = 4` rows and, for
    /// new-component deletion candidates (`proposed_payload IS NULL`), their
    /// component rows too. Upgrade deletion candidates (`proposed_payload IS
    /// NOT NULL`) only delete the queue row — the live validated row stays
    /// (§0.23.5). Each candidate is graduated in its own transaction. Returns
    /// the number of queue rows purged.
    pub async fn purge_deletion_candidates(
        &self,
        scope: &ComponentScope,
    ) -> Result<u64, ValidationQueueError> {
        let mut client = self.pool.get().await.map_err(map_pool)?;
        let rows = client
            .query(
                "SELECT component_id, component_class, proposed_payload
                 FROM reborn_validation_queue
                 WHERE tenant_id = $1 AND user_id = $2
                   AND agent_id = $3 AND project_id = $4
                   AND state = 4
                 ORDER BY submitted_at",
                &[
                    &scope.tenant_id,
                    &scope.user_id,
                    &scope.agent_id,
                    &scope.project_id,
                ],
            )
            .await
            .map_err(map_pg)?;

        let mut purged: u64 = 0;
        for row in rows {
            let component_id: Uuid = row.get(0);
            let class_code: i16 = row.get(1);
            let proposed_payload: Option<Value> = row.get(2);

            let tx = client.transaction().await.map_err(map_pg)?;

            // New-component deletion candidate: delete the (pending/rejected)
            // component row too. Upgrade deletion candidate: leave the live
            // validated row untouched (§0.23.5). Unknown class for a deletion
            // candidate skips the component delete (nothing to delete) and still
            // drops the queue row.
            if proposed_payload.is_none()
                && let Some(table) = resolve_component_table(class_code as i32)
            {
                let del_sql = format!(
                    "DELETE FROM {table}
                     WHERE id = $1
                       AND tenant_id = $2 AND user_id = $3
                       AND agent_id = $4 AND project_id = $5"
                );
                tx.execute(
                    del_sql.as_str(),
                    &[
                        &component_id,
                        &scope.tenant_id,
                        &scope.user_id,
                        &scope.agent_id,
                        &scope.project_id,
                    ],
                )
                .await
                .map_err(map_pg)?;
            }

            tx.execute(
                "DELETE FROM reborn_validation_queue
                 WHERE tenant_id = $1 AND user_id = $2
                   AND agent_id = $3 AND project_id = $4
                   AND component_id = $5",
                &[
                    &scope.tenant_id,
                    &scope.user_id,
                    &scope.agent_id,
                    &scope.project_id,
                    &component_id,
                ],
            )
            .await
            .map_err(map_pg)?;

            tx.commit().await.map_err(map_pg)?;
            purged += 1;
        }
        Ok(purged)
    }
}

fn decode_queue_row(row: tokio_postgres::Row) -> Result<QueueRow, ValidationQueueError> {
    Ok(QueueRow {
        id: row.get(0),
        component_id: row.get(1),
        component_class: row.get(2),
        state: row.get(3),
        counter: row.get(4),
        review_feedback: row.get(5),
        validation_errors: row.get(6),
        proposed_payload: row.get(7),
        submitted_at: row.get(8),
        updated_at: row.get(9),
    })
}

// ---------------------------------------------------------------------------
// Pure helpers (factored for unit testing without a live Postgres)
// ---------------------------------------------------------------------------

/// Map a component class code to its target component table — the same map
/// `fetch_component_by_id` uses (`retrieval_source.rs`). Returns `None` for
/// reserved (11) / unknown class codes. Classes 22 (Phase B) and 23 (Phase C)
/// are included now so `approve` is forward-compatible the moment those tables
/// land; until then a query against a not-yet-created table errors at runtime
/// (correct — the table arrives with its phase).
pub(crate) fn resolve_component_table(class_code: i32) -> Option<&'static str> {
    match class_code {
        0 => Some("reborn_tools"),
        1..=3 | 10 | 50 => Some("reborn_skills"),
        4..=9 => Some("reborn_extensions_unified"),
        12 => Some("reborn_specs"),
        13 => Some("reborn_tool_skills"),
        14 => Some("reborn_plans"),
        15 => Some("reborn_summaries"),
        16 => Some("reborn_actions"),
        17 => Some("reborn_docus"),
        18 => Some("reborn_lessons"),
        19 => Some("reborn_issues"),
        20 => Some("reborn_notes"),
        21 => Some("reborn_recipes"),
        // Phase B (V052) / Phase C (V053) — tables not yet created.
        22 => Some("reborn_python_code"),
        23 => Some("reborn_extension_catalogues"),
        _ => None, // 11 reserved, anything else unknown
    }
}

/// The post-reject state after `counter` has been incremented: `4` (deletion
/// candidate) when `counter >= threshold`, else `3` (rejected) — §0.18.
pub(crate) fn next_reject_state(counter_after_increment: i32, threshold: u8) -> i16 {
    if counter_after_increment >= threshold as i32 {
        STATE_DELETION_CANDIDATE
    } else {
        STATE_REJECTED
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Pure-logic unit tests (no Postgres) ──────────────────────────────

    #[test]
    fn resolve_component_table_covers_every_class() {
        // Fixed-class tables.
        assert_eq!(resolve_component_table(0), Some("reborn_tools"));
        assert_eq!(resolve_component_table(21), Some("reborn_recipes"));
        assert_eq!(resolve_component_table(12), Some("reborn_specs"));
        assert_eq!(resolve_component_table(13), Some("reborn_tool_skills"));
        assert_eq!(resolve_component_table(14), Some("reborn_plans"));
        assert_eq!(resolve_component_table(15), Some("reborn_summaries"));
        assert_eq!(resolve_component_table(16), Some("reborn_actions"));
        assert_eq!(resolve_component_table(17), Some("reborn_docus"));
        assert_eq!(resolve_component_table(18), Some("reborn_lessons"));
        assert_eq!(resolve_component_table(19), Some("reborn_issues"));
        assert_eq!(resolve_component_table(20), Some("reborn_notes"));
        // Shared reborn_skills table.
        assert_eq!(resolve_component_table(1), Some("reborn_skills"));
        assert_eq!(resolve_component_table(2), Some("reborn_skills"));
        assert_eq!(resolve_component_table(3), Some("reborn_skills"));
        assert_eq!(resolve_component_table(10), Some("reborn_skills"));
        assert_eq!(resolve_component_table(50), Some("reborn_skills"));
        // Shared reborn_extensions_unified table.
        assert_eq!(
            resolve_component_table(4),
            Some("reborn_extensions_unified")
        );
        assert_eq!(
            resolve_component_table(9),
            Some("reborn_extensions_unified")
        );
        // Phase B / C tables (forward-compatible — arrive with their phase).
        assert_eq!(resolve_component_table(22), Some("reborn_python_code"));
        assert_eq!(
            resolve_component_table(23),
            Some("reborn_extension_catalogues")
        );
        // Reserved / unknown.
        assert_eq!(resolve_component_table(11), None);
        assert_eq!(resolve_component_table(99), None);
        assert_eq!(resolve_component_table(-1), None);
    }

    #[test]
    fn next_reject_state_promotes_at_threshold() {
        // Below threshold → state 3 (rejected).
        assert_eq!(next_reject_state(1, 3), STATE_REJECTED);
        assert_eq!(next_reject_state(2, 3), STATE_REJECTED);
        // At/above threshold → state 4 (deletion candidate).
        assert_eq!(next_reject_state(3, 3), STATE_DELETION_CANDIDATE);
        assert_eq!(next_reject_state(4, 3), STATE_DELETION_CANDIDATE);
        // Threshold 1 promotes on the first rejection.
        assert_eq!(next_reject_state(1, 1), STATE_DELETION_CANDIDATE);
        // Counter 0 (no rejection yet) is below any non-zero threshold.
        assert_eq!(next_reject_state(0, 3), STATE_REJECTED);
    }

    #[test]
    fn store_defaults_to_threshold_three() {
        // `new` uses DEFAULT_REJECT_THRESHOLD; `with_reject_threshold` overrides.
        // The field is private, so observe it indirectly through the const and
        // the constructor contract.
        assert_eq!(DEFAULT_REJECT_THRESHOLD, 3);
        // Construct both (pool unused here — only the threshold contract matters).
        let pool = test_pool_stub();
        let s = ValidationQueueStore::new(pool.clone());
        assert_eq!(s.reject_threshold, 3);
        let s2 = ValidationQueueStore::with_reject_threshold(pool, 1);
        assert_eq!(s2.reject_threshold, 1);
    }

    /// Build a pool stub for constructor-only tests (no DB calls made).
    ///
    /// A zero-capacity deadpool is fine — these tests never await a checkout.
    fn test_pool_stub() -> Arc<PgPool> {
        use deadpool_postgres::{Manager, Pool};
        let mut cfg = tokio_postgres::Config::new();
        cfg.dbname("stub").user("stub").host("127.0.0.1").port(1);
        let manager = Manager::new(cfg, tokio_postgres::NoTls);
        Arc::new(Pool::builder(manager).build().expect("pool builder"))
    }

    // ── Migration shape (guards the V051 DDL without a live Postgres) ─────

    #[test]
    fn v051_migration_creates_table_indexes_and_proposed_payload() {
        let sql = include_str!("../../brassclaw_pg/migrations/V051__reborn_validation_queue.sql");
        // Table + idempotent creation.
        assert!(
            sql.contains("CREATE TABLE IF NOT EXISTS reborn_validation_queue"),
            "V051 must create reborn_validation_queue"
        );
        // §0.23.5 fold-in: upgrade-copy payload column.
        assert!(
            sql.contains("proposed_payload") && sql.contains("JSONB"),
            "V051 must add proposed_payload JSONB (§0.23.5)"
        );
        // State CHECK constraint (§0.18 authoritative).
        assert!(
            sql.contains("CHECK (state IN (1, 2, 3, 4))"),
            "V051 must carry the §0.18 state CHECK"
        );
        // Three indexes (scope-state, scope-class, deletion partial).
        assert!(
            sql.contains("reborn_validation_queue_scope_state_idx"),
            "scope_state index missing"
        );
        assert!(
            sql.contains("reborn_validation_queue_scope_class_idx"),
            "scope_class index missing"
        );
        assert!(
            sql.contains("reborn_validation_queue_deletion_idx") && sql.contains("WHERE state = 4"),
            "deletion partial index (state = 4) missing"
        );
        // Scope-first UNIQUE (one queue row per component).
        assert!(
            sql.contains("UNIQUE (tenant_id, user_id, agent_id, project_id, component_id)"),
            "scope-first UNIQUE missing"
        );
        // No data migration / DROPs (those are V059 / Phase N).
        assert!(!sql.contains("DROP COLUMN"), "V051 must not drop columns");
        assert!(
            !sql.contains("INSERT INTO reborn_validation_queue"),
            "V051 must not populate rows"
        );
    }

    // ── Postgres integration tests (skip when docker is unavailable) ──────
    //
    // These mirror the `postgres_substrate.rs` harness: each test starts an
    // isolated Postgres-16 testcontainer, runs the full migration set (so
    // `reborn_validation_queue` and the component tables all exist), and
    // returns early (pass) when docker/testcontainers is unavailable. They
    // run under the default `postgres` feature and add no failures in a
    // docker-less `cargo test -p brassclaw_reborn_composition` run.

    mod pg {
        use super::*;
        use brassclaw_engine::memory::component_validator::{
            ComponentPayload, GenericComponent, ValidationConfig,
        };
        use brassclaw_engine::memory::retrieval_source::ComponentScope;
        use brassclaw_pg::PgPool;

        struct PgRig {
            // Held for the test's lifetime so the container stays up.
            _container: testcontainers_modules::testcontainers::ContainerAsync<
                testcontainers_modules::postgres::Postgres,
            >,
            pool: Arc<PgPool>,
        }

        /// Start an isolated Postgres-16 testcontainer, build a pool, and run
        /// every migration (V000–V051). Returns `None` (skip) when docker is
        /// unavailable.
        async fn pg_rig_or_skip() -> Option<PgRig> {
            use deadpool_postgres::{Manager, Pool};
            use testcontainers_modules::testcontainers::{ImageExt, runners::AsyncRunner};

            let image = testcontainers_modules::postgres::Postgres::default()
                .with_db_name("brassclaw_test")
                .with_user("postgres")
                .with_password("postgres")
                .with_tag("16-alpine");
            let container = match image.start().await {
                Ok(c) => c,
                Err(error) => {
                    eprintln!(
                        "skipping validation_queue pg tests: docker/testcontainers unavailable ({error})"
                    );
                    return None;
                }
            };
            let host = match container.get_host().await {
                Ok(h) => h,
                Err(error) => {
                    eprintln!("skipping validation_queue pg tests: no host ({error})");
                    return None;
                }
            };
            let port = match container.get_host_port_ipv4(5432).await {
                Ok(p) => p,
                Err(error) => {
                    eprintln!("skipping validation_queue pg tests: no port ({error})");
                    return None;
                }
            };
            let url = format!("postgres://postgres:postgres@{host}:{port}/brassclaw_test");
            let cfg: tokio_postgres::Config = url.parse().expect("testcontainer url parses");
            let manager = Manager::new(cfg, tokio_postgres::NoTls);
            let pool = Pool::builder(manager)
                .max_size(4)
                .build()
                .expect("Postgres pool must build");
            brassclaw_pg::migrations::run_migrations(&pool)
                .await
                .expect("migrations must apply");
            Some(PgRig {
                _container: container,
                pool: Arc::new(pool),
            })
        }

        fn test_scope() -> ComponentScope {
            ComponentScope {
                tenant_id: "t".into(),
                user_id: "u".into(),
                agent_id: "a".into(),
                project_id: "p".into(),
            }
        }

        /// Insert a `reborn_notes` (class 20) row at `validation_status =
        /// 'pending'` and return its id. Uses a UUID-derived name so parallel
        /// tests never hit the `UNIQUE(scope, name)` constraint.
        async fn insert_pending_note(pool: &PgPool, scope: &ComponentScope) -> Uuid {
            let name = format!("note-{}", Uuid::new_v4());
            let client = pool.get().await.expect("pool client");
            let row = client
                .query_one(
                    "INSERT INTO reborn_notes
                         (tenant_id, user_id, agent_id, project_id, name, validation_status)
                     VALUES ($1, $2, $3, $4, $5, 'pending')
                     RETURNING id",
                    &[
                        &scope.tenant_id,
                        &scope.user_id,
                        &scope.agent_id,
                        &scope.project_id,
                        &name,
                    ],
                )
                .await
                .expect("insert pending note");
            row.get(0)
        }

        async fn read_note_status(pool: &PgPool, scope: &ComponentScope, id: Uuid) -> String {
            let client = pool.get().await.expect("pool client");
            let row = client
                .query_one(
                    "SELECT validation_status FROM reborn_notes
                     WHERE id = $1 AND tenant_id = $2 AND user_id = $3
                       AND agent_id = $4 AND project_id = $5",
                    &[
                        &id,
                        &scope.tenant_id,
                        &scope.user_id,
                        &scope.agent_id,
                        &scope.project_id,
                    ],
                )
                .await
                .expect("read note");
            row.get(0)
        }

        #[tokio::test]
        async fn submit_inserts_state_one_row() {
            let Some(rig) = pg_rig_or_skip().await else {
                return;
            };
            let scope = test_scope();
            let store = ValidationQueueStore::new(rig.pool.clone());
            let cid = Uuid::new_v4();
            store.submit(&scope, cid, 20, None).await.expect("submit");
            let rows = store.list(&scope, None).await.expect("list");
            let row = rows
                .iter()
                .find(|r| r.component_id == cid)
                .expect("queue row exists");
            assert_eq!(row.state, STATE_Q1_PENDING);
            assert_eq!(row.component_class, 20);
            assert!(row.proposed_payload.is_none());
        }

        #[tokio::test]
        async fn submit_rejects_duplicate_component() {
            let Some(rig) = pg_rig_or_skip().await else {
                return;
            };
            let scope = test_scope();
            let store = ValidationQueueStore::new(rig.pool.clone());
            let cid = Uuid::new_v4();
            store
                .submit(&scope, cid, 20, None)
                .await
                .expect("first submit");
            // A second submit for the same (scope, component_id) is rejected —
            // one pending upgrade per component at a time (§0.23.5).
            let err = store
                .submit(&scope, cid, 20, Some(serde_json::json!({"x": 1})))
                .await
                .expect_err("duplicate submit must error");
            assert!(matches!(err, ValidationQueueError::AlreadyQueued { .. }));
        }

        #[tokio::test]
        async fn gate1_pass_and_gate1_fail_transition_correctly() {
            let Some(rig) = pg_rig_or_skip().await else {
                return;
            };
            let scope = test_scope();
            let store = ValidationQueueStore::new(rig.pool.clone());

            // gate1_pass: 1 → 2, errors cleared.
            let a = Uuid::new_v4();
            store.submit(&scope, a, 20, None).await.unwrap();
            store.gate1_pass(&scope, a, &[]).await.expect("pass");
            let passed = store.list(&scope, Some(2)).await.unwrap();
            assert!(passed.iter().any(|r| r.component_id == a));
            let ra = passed.iter().find(|r| r.component_id == a).unwrap();
            assert!(ra.validation_errors.is_empty());

            // gate1_fail: stays 1, errors recorded.
            let b = Uuid::new_v4();
            store.submit(&scope, b, 20, None).await.unwrap();
            store
                .gate1_fail(&scope, b, &["bad name".to_string()])
                .await
                .expect("fail");
            let pending = store.list(&scope, Some(1)).await.unwrap();
            let rb = pending
                .iter()
                .find(|r| r.component_id == b)
                .expect("still pending");
            assert_eq!(rb.state, STATE_Q1_PENDING);
            assert_eq!(rb.validation_errors, vec!["bad name".to_string()]);
        }

        #[tokio::test]
        async fn reject_transitions_two_to_three_and_increments_counter() {
            let Some(rig) = pg_rig_or_skip().await else {
                return;
            };
            let scope = test_scope();
            let store = ValidationQueueStore::new(rig.pool.clone()); // threshold 3
            let cid = Uuid::new_v4();
            store.submit(&scope, cid, 20, None).await.unwrap();
            store.gate1_pass(&scope, cid, &[]).await.unwrap();
            store
                .reject(&scope, cid, "needs work")
                .await
                .expect("reject");
            let row = store
                .list(&scope, Some(3))
                .await
                .unwrap()
                .into_iter()
                .find(|r| r.component_id == cid)
                .expect("rejected");
            assert_eq!(row.state, STATE_REJECTED);
            assert_eq!(row.counter, 1);
            assert_eq!(row.review_feedback.as_deref(), Some("needs work"));
        }

        #[tokio::test]
        async fn reject_auto_promotes_to_deletion_candidate_at_threshold() {
            let Some(rig) = pg_rig_or_skip().await else {
                return;
            };
            let scope = test_scope();
            // threshold 1 → first rejection promotes to state 4.
            let store = ValidationQueueStore::with_reject_threshold(rig.pool.clone(), 1);
            let cid = Uuid::new_v4();
            store.submit(&scope, cid, 20, None).await.unwrap();
            store.gate1_pass(&scope, cid, &[]).await.unwrap();
            store.reject(&scope, cid, "first").await.expect("reject");
            let row = store
                .list(&scope, Some(4))
                .await
                .unwrap()
                .into_iter()
                .find(|r| r.component_id == cid)
                .expect("promoted to deletion candidate");
            assert_eq!(row.state, STATE_DELETION_CANDIDATE);
            assert_eq!(row.counter, 1);
        }

        #[tokio::test]
        async fn approve_graduates_component_and_deletes_queue_row() {
            let Some(rig) = pg_rig_or_skip().await else {
                return;
            };
            let scope = test_scope();
            let store = ValidationQueueStore::new(rig.pool.clone());
            let cid = insert_pending_note(&rig.pool, &scope).await;
            store.submit(&scope, cid, 20, None).await.expect("submit");
            store.gate1_pass(&scope, cid, &[]).await.expect("pass");
            let returned = store.approve(&scope, cid).await.expect("approve");
            assert_eq!(returned, cid, "approve returns the component id");
            // Queue row deleted.
            let rows = store.list(&scope, None).await.unwrap();
            assert!(
                !rows.iter().any(|r| r.component_id == cid),
                "queue row must be deleted on graduation"
            );
            // Component now validated.
            assert_eq!(read_note_status(&rig.pool, &scope, cid).await, "validated");
        }

        #[tokio::test]
        async fn approve_unknown_class_errors_before_transaction() {
            let Some(rig) = pg_rig_or_skip().await else {
                return;
            };
            let scope = test_scope();
            let store = ValidationQueueStore::new(rig.pool.clone());
            let cid = Uuid::new_v4();
            // Class 11 is reserved (no target table) — submit accepts it; approve must not.
            store.submit(&scope, cid, 11, None).await.expect("submit");
            store.gate1_pass(&scope, cid, &[]).await.unwrap();
            let err = store
                .approve(&scope, cid)
                .await
                .expect_err("unknown class must error before tx");
            assert!(
                matches!(err, ValidationQueueError::UnknownClass { class_code: 11 }),
                "wrong error: {err:?}"
            );
            // Queue row preserved (no transaction touched it).
            let row = store
                .list(&scope, Some(2))
                .await
                .unwrap()
                .into_iter()
                .find(|r| r.component_id == cid)
                .expect("queue row preserved");
            assert_eq!(row.state, STATE_Q1_PASSED);
        }

        #[tokio::test]
        async fn approve_missing_component_rolls_back_and_preserves_queue_row() {
            let Some(rig) = pg_rig_or_skip().await else {
                return;
            };
            let scope = test_scope();
            let store = ValidationQueueStore::new(rig.pool.clone());
            // No component row exists for this id.
            let cid = Uuid::new_v4();
            store.submit(&scope, cid, 20, None).await.unwrap();
            store.gate1_pass(&scope, cid, &[]).await.unwrap();
            let err = store
                .approve(&scope, cid)
                .await
                .expect_err("missing component must error");
            assert!(
                matches!(err, ValidationQueueError::ComponentMissing { .. }),
                "wrong error: {err:?}"
            );
            // Queue row preserved at state 2 (the UPDATE 0-row path rolled back).
            let row = store
                .list(&scope, Some(2))
                .await
                .unwrap()
                .into_iter()
                .find(|r| r.component_id == cid)
                .expect("queue row preserved after rollback");
            assert_eq!(row.state, STATE_Q1_PASSED);
        }

        #[tokio::test]
        async fn approve_refuses_upgrade_copy_until_phase_n() {
            let Some(rig) = pg_rig_or_skip().await else {
                return;
            };
            let scope = test_scope();
            let store = ValidationQueueStore::new(rig.pool.clone());
            let cid = Uuid::new_v4();
            let payload = serde_json::json!({"content": "edited version"});
            store
                .submit(&scope, cid, 20, Some(payload))
                .await
                .expect("submit upgrade copy");
            store.gate1_pass(&scope, cid, &[]).await.unwrap();
            let err = store
                .approve(&scope, cid)
                .await
                .expect_err("upgrade graduation not implemented in Phase A.5");
            assert!(
                matches!(
                    err,
                    ValidationQueueError::UpgradeGraduationNotImplemented { .. }
                ),
                "wrong error: {err:?}"
            );
            // Queue row preserved at state 2 with the payload intact.
            let row = store
                .list(&scope, Some(2))
                .await
                .unwrap()
                .into_iter()
                .find(|r| r.component_id == cid)
                .expect("queue row preserved");
            assert_eq!(row.state, STATE_Q1_PASSED);
            assert!(
                row.proposed_payload.is_some(),
                "upgrade payload must survive"
            );
        }

        #[tokio::test]
        async fn run_q1_validation_valid_passes_and_invalid_fails_queue() {
            use crate::q1_orchestrator::run_q1_validation;
            let Some(rig) = pg_rig_or_skip().await else {
                return;
            };
            let scope = test_scope();
            let store = ValidationQueueStore::new(rig.pool.clone());

            // Valid payload → gate1_pass (state 1 → 2, errors empty).
            let good = Uuid::new_v4();
            store.submit(&scope, good, 20, None).await.unwrap();
            let outcome = run_q1_validation(
                &rig.pool,
                &scope,
                good,
                20,
                ComponentPayload::Generic(GenericComponent {
                    name: "good-note",
                    description: "d",
                    content: "c",
                    extra: None,
                }),
                &ValidationConfig::default(),
                &store,
            )
            .await
            .expect("run q1");
            assert!(outcome.passed, "valid payload should pass: {outcome:?}");
            let row = store
                .list(&scope, Some(2))
                .await
                .unwrap()
                .into_iter()
                .find(|r| r.component_id == good)
                .expect("passed row");
            assert_eq!(row.state, STATE_Q1_PASSED);
            assert!(row.validation_errors.is_empty());

            // Invalid payload (empty name) → gate1_fail (stays 1, errors recorded).
            let bad = Uuid::new_v4();
            store.submit(&scope, bad, 20, None).await.unwrap();
            let outcome = run_q1_validation(
                &rig.pool,
                &scope,
                bad,
                20,
                ComponentPayload::Generic(GenericComponent {
                    name: "",
                    description: "d",
                    content: "c",
                    extra: None,
                }),
                &ValidationConfig::default(),
                &store,
            )
            .await
            .expect("run q1");
            assert!(!outcome.passed, "invalid payload should fail: {outcome:?}");
            assert!(!outcome.errors.is_empty());
            let row = store
                .list(&scope, Some(1))
                .await
                .unwrap()
                .into_iter()
                .find(|r| r.component_id == bad)
                .expect("failed row");
            assert_eq!(row.state, STATE_Q1_PENDING);
            assert!(!row.validation_errors.is_empty());
        }

        #[tokio::test]
        async fn integration_submit_q1_approve_graduates() {
            use crate::q1_orchestrator::run_q1_validation;
            let Some(rig) = pg_rig_or_skip().await else {
                return;
            };
            let scope = test_scope();
            let store = ValidationQueueStore::new(rig.pool.clone());
            let cid = insert_pending_note(&rig.pool, &scope).await;

            store.submit(&scope, cid, 20, None).await.expect("submit");
            let outcome = run_q1_validation(
                &rig.pool,
                &scope,
                cid,
                20,
                ComponentPayload::Generic(GenericComponent {
                    name: "roundtrip",
                    description: "d",
                    content: "c",
                    extra: None,
                }),
                &ValidationConfig::default(),
                &store,
            )
            .await
            .expect("q1");
            assert!(outcome.passed);
            store.approve(&scope, cid).await.expect("approve");
            assert!(
                store
                    .list(&scope, None)
                    .await
                    .unwrap()
                    .iter()
                    .all(|r| r.component_id != cid),
                "queue row deleted after graduation"
            );
            assert_eq!(
                read_note_status(&rig.pool, &scope, cid).await,
                "validated",
                "component graduated to validated"
            );
        }

        #[tokio::test]
        async fn purge_deletion_candidates_drops_queue_and_component_rows() {
            let Some(rig) = pg_rig_or_skip().await else {
                return;
            };
            let scope = test_scope();
            // threshold 1 so a single reject promotes to state 4.
            let store = ValidationQueueStore::with_reject_threshold(rig.pool.clone(), 1);
            let cid = insert_pending_note(&rig.pool, &scope).await;
            store.submit(&scope, cid, 20, None).await.unwrap();
            store.gate1_pass(&scope, cid, &[]).await.unwrap();
            store.reject(&scope, cid, "condemned").await.unwrap();
            // confirm state 4
            assert!(
                store
                    .list(&scope, Some(4))
                    .await
                    .unwrap()
                    .iter()
                    .any(|r| r.component_id == cid)
            );

            let purged = store
                .purge_deletion_candidates(&scope)
                .await
                .expect("purge");
            assert_eq!(purged, 1);
            // queue row gone
            assert!(
                !store
                    .list(&scope, None)
                    .await
                    .unwrap()
                    .iter()
                    .any(|r| r.component_id == cid)
            );
            // component row gone (new-component deletion candidate)
            let client = rig.pool.get().await.expect("pool");
            let row = client
                .query_opt("SELECT id FROM reborn_notes WHERE id = $1", &[&cid])
                .await
                .expect("read");
            assert!(row.is_none(), "component row must be purged");
        }
    }
}
