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
//! Only [`ValidationQueueStore::gate1_pass_reviewed`] (`pub(crate)`) writes state 2 —
//! the sole write path, enforced by Rust visibility. Any other writer of
//! state 2 is a security bug. The Q2 reviewer approves from state 2 →
//! [`ValidationQueueStore::approve`] deletes the row (graduation) in ONE
//! transaction with the component-table UPDATE (FIND-P9-05).
//!
//! # Upgrade model (§0.23.5)
//!
//! [`ValidationQueueStore::submit`] carries `proposed_payload: Option<Value>`
//! (set for upgrades, `None` for new-component submissions). The graduation
//! *apply* of `proposed_payload` still overwrites the live validated row. The
//! queue is locked through graduation and payload fields fail closed. This
//! legacy path does not implement immutable v3 revisions/association approval.
//!
//! # Feature gate
//!
//! Requires the `postgres` feature.

// Phase A.5 wiring is complete (Phase B/C + Phase N). Phase P.0 adds the
// q2_actor audit column and the builtin bootstrap audit path.
#![allow(dead_code)]
#![forbid(unsafe_code)]

use std::sync::Arc;

use brassclaw_engine::memory::retrieval_source::ComponentScope;
use brassclaw_pg::PgPool;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

#[path = "validation_review.rs"]
mod review;
pub(crate) use review::Q1Candidate;

#[cfg(feature = "postgres")]
use crate::pg_basic_prompt_store::PgBasicPromptStore;

/// Default rejection threshold: after 3 rejections a row auto-promotes to
/// state 4 (deletion candidate) — §0.18. Configurable per-store via
/// [`ValidationQueueStore::with_reject_threshold`] (Q2 answer — construction-time
/// field; Phase K/N can later wire it from `reborn_monty_vm_settings`).
pub const DEFAULT_REJECT_THRESHOLD: u8 = 3;

enum GraduationActor {
    Human,
    Builtin,
}

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
    #[error("component {component_id} has an invalid upgrade payload")]
    InvalidPayload { component_id: Uuid },
    #[error("component {component_id} cannot pass Q1 with reported errors")]
    InvalidGate1Result { component_id: Uuid },
    #[error("component {component_id} requires the human Q2 review path")]
    InvalidQ2Actor { component_id: Uuid },
    #[error("component {component_id} or its submission changed since Q1 review")]
    ReviewChanged { component_id: Uuid },
    #[error(
        "component {component_id} disappeared during approve (rolled back, queue row preserved)"
    )]
    ComponentMissing { component_id: Uuid },
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
        // PostgreSQL's detail/message can contain complete candidate rows.
        // Retain the stable SQLSTATE for diagnosis without exposing contents.
        reason: match e.code() {
            Some(code) => format!("SQLSTATE {}", code.code()),
            None => e.to_string(),
        },
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
    /// Phase P.0: who performed the Q2 graduation. `None` = pending (not yet
    /// approved). `Some("human")` = operator via WebUI. `Some("builtin")` =
    /// bootstrap seeder (exempt from human-Q2; audit label only).
    pub q2_actor: Option<String>,
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

/// Postgres-backed store for `reborn_validation_queue` (Phase A.5).
#[derive(Clone)]
pub struct ValidationQueueStore {
    pool: Arc<PgPool>,
    reject_threshold: u8,
    /// Optional: when set, `approve()` calls `mark_stale` on this store after
    /// each Q2 graduation so the Sempai prefix bundle is re-assembled on the
    /// next turn (§12 of prefix_V3.md).
    #[cfg(feature = "postgres")]
    basic_prompt_store: Option<Arc<PgBasicPromptStore>>,
}

impl ValidationQueueStore {
    /// Create a store with the default rejection threshold (3).
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self {
            pool,
            reject_threshold: DEFAULT_REJECT_THRESHOLD,
            #[cfg(feature = "postgres")]
            basic_prompt_store: None,
        }
    }

    /// Create a store with an explicit rejection threshold (§0.18: configurable,
    /// default 3). Used by tests and, later, by Phase K/N wiring that reads the
    /// threshold from `reborn_monty_vm_settings`.
    pub fn with_reject_threshold(pool: Arc<PgPool>, reject_threshold: u8) -> Self {
        Self {
            pool,
            reject_threshold,
            #[cfg(feature = "postgres")]
            basic_prompt_store: None,
        }
    }

    /// Attach a [`PgBasicPromptStore`] so that `approve()` calls `mark_stale`
    /// after each Q2 graduation (§12 — side-effect 4).
    #[cfg(feature = "postgres")]
    pub(crate) fn with_basic_prompt_store(mut self, store: Arc<PgBasicPromptStore>) -> Self {
        self.basic_prompt_store = Some(store);
        self
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
        if let Some(payload) = &proposed_payload {
            validate_upgrade_payload(component_class, component_id, payload)?;
        }
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

    /// Record a Q1 failure: stays in `state 1`, populates `validation_errors`,
    /// increments nothing (the author must fix and resubmit — §0.18).
    ///
    /// `pub(crate)` — paired with [`Self::gate1_pass_reviewed`]; only the Q1
    /// orchestration in this crate calls it.
    #[cfg(test)]
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
        let threshold = i32::from(self.reject_threshold);
        let updated = client
            .execute(
                "UPDATE reborn_validation_queue
                 SET state = CASE WHEN counter + 1 >= $7 THEN 4 ELSE 3 END,
                     counter = counter + 1,
                     review_feedback = $6,
                     updated_at = now()
                 WHERE tenant_id = $1 AND user_id = $2
                   AND agent_id = $3 AND project_id = $4
                   AND component_id = $5
                   AND state = 2",
                &[
                    &scope.tenant_id,
                    &scope.user_id,
                    &scope.agent_id,
                    &scope.project_id,
                    &component_id,
                    &feedback,
                    &threshold,
                ],
            )
            .await
            .map_err(map_pg)?;
        if updated == 0 {
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
        }
        // Promotion and rejection commit in one row update, so an invalidation
        // cannot be overwritten by a later unguarded state-4 update.
        Ok(())
    }

    /// Q2 approval = graduation.
    ///
    /// **New-component path** (`proposed_payload IS NULL`): flip the component
    /// row to `validation_status = 'validated'` and delete the queue row in
    /// ONE transaction (FIND-P9-05).
    ///
    /// **Upgrade-copy path** (`proposed_payload IS NOT NULL`, §0.23.5 / Phase N):
    /// the live component row stays `'validated'` and keeps serving retrieval
    /// throughout. On approval, the `proposed_payload` JSONB is merged into
    /// the live row's writable columns (`name`, `description`, plus the
    /// class-specific primary content column) while leaving `validation_status =
    /// 'validated'`. The queue row is then deleted (graduation trigger fires).
    /// The live row is NEVER taken out of retrieval during the upgrade window —
    /// only its content changes at approval.
    ///
    /// On rejection: the queue copy moves to state 3/4 (via [`Self::reject`]);
    /// the live row is untouched.
    ///
    /// Returns `Ok(component_id)` on success.
    /// `q2_actor` must be `Some("human")` on this public operator path.
    /// Compiled-in bootstrap audits use the separate crate-private method.
    /// This marker is legacy audit metadata, not authenticated identity or
    /// v3 exact-combination approval evidence.
    pub async fn approve(
        &self,
        scope: &ComponentScope,
        component_id: Uuid,
        q2_actor: Option<&str>,
    ) -> Result<Uuid, ValidationQueueError> {
        if q2_actor != Some("human") {
            return Err(ValidationQueueError::InvalidQ2Actor { component_id });
        }
        self.approve_with_actor(scope, component_id, GraduationActor::Human)
            .await
    }

    /// Internal checked-in bootstrap audit only. This legacy receipt is not a
    /// v3 system-seed combination approval, Q1/behavior evidence or Tool grant.
    /// Ordinary operator/component authoring cannot select this audit path.
    pub(crate) async fn approve_builtin_seed(
        &self,
        scope: &ComponentScope,
        component_id: Uuid,
    ) -> Result<Uuid, ValidationQueueError> {
        self.approve_with_actor(scope, component_id, GraduationActor::Builtin)
            .await
    }

    async fn approve_with_actor(
        &self,
        scope: &ComponentScope,
        component_id: Uuid,
        actor: GraduationActor,
    ) -> Result<Uuid, ValidationQueueError> {
        let q2_actor = Some(match actor {
            GraduationActor::Human => "human",
            GraduationActor::Builtin => "builtin",
        });
        let mut client = self.pool.get().await.map_err(map_pool)?;

        // Queue-before-component lock order is shared with invalidation and
        // purge. Read the reviewed state/payload under the graduation lock; a
        // rejection or concurrent approval cannot invalidate an earlier read.
        let tx = client.transaction().await.map_err(map_pg)?;
        let row = tx
            .query_opt(
                &format!(
                    "SELECT state, proposed_payload, component_class, id, to_jsonb(q)::text,
                         q1_component_bytes, q1_queue_bytes, (to_jsonb(q)-{})::text
                 FROM reborn_validation_queue q
                 WHERE tenant_id = $1 AND user_id = $2
                   AND agent_id = $3 AND project_id = $4
                   AND component_id = $5
                 FOR UPDATE",
                    review::QUEUE_VOLATILE
                ),
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
        let class_code: i16 = row.get(2);
        let queue_id: Uuid = row.get(3);
        let queue_bytes: String = row.get(4);

        // Resolve only a trusted table literal; unknown classes roll back
        // without mutating the queue or component.
        let table = resolve_component_table(class_code as i32).ok_or(
            ValidationQueueError::UnknownClass {
                class_code: class_code as i32,
            },
        )?;

        // Recheck all protected authoring fields and the exact queued proposal
        // under queue-before-component locks. Changes after Q1 require review
        // again; a hash of today's row is not evidence it was reviewed.
        if let Some(payload) = &proposed_payload {
            validate_upgrade_payload(i32::from(class_code), component_id, payload)?;
        }
        let current_component =
            review::component_bytes(&tx, scope, component_id, class_code).await?;
        let reviewed_component: Option<String> = row.get(5);
        let reviewed_queue: Option<String> = row.get(6);
        if reviewed_component.as_deref() != Some(current_component.as_str())
            || reviewed_queue.as_deref() != Some(row.get::<_, String>(7).as_str())
        {
            return Err(ValidationQueueError::ReviewChanged { component_id });
        }

        // Update the component, then delete the locked queue row.
        // Ordering: UPDATE before DELETE so the graduation trigger (V077)
        // fires only after the component is already updated — no window where
        // the queue row is gone but the component change is not yet committed.
        let updated = if let Some(payload) = proposed_payload {
            // Upgrade-copy graduation (§0.23.5 / Phase N): apply proposed
            // payload to the live validated row. Only validated writable fields
            // are updated; unknown fields/types fail before mutation.
            apply_upgrade_payload(&tx, table, class_code, component_id, scope, &payload).await?
        } else {
            // New-component graduation: flip pending → validated.
            let update_sql = format!(
                "UPDATE {table}
                 SET validation_status = 'validated', updated_at = now()
                 WHERE id = $1
                   AND tenant_id = $2 AND user_id = $3
                   AND agent_id = $4 AND project_id = $5 AND class_code = $6"
            );
            tx.execute(
                update_sql.as_str(),
                &[
                    &component_id,
                    &scope.tenant_id,
                    &scope.user_id,
                    &scope.agent_id,
                    &scope.project_id,
                    &class_code,
                ],
            )
            .await
            .map_err(map_pg)?
        };

        if updated == 0 {
            // Component disappeared — ROLLBACK, queue row preserved (FIND-P9-05).
            tx.rollback().await.map_err(map_pg)?;
            return Err(ValidationQueueError::ComponentMissing { component_id });
        }

        // Ordinary Recipe graduation removes its Q1 workflow marker in this same
        // transaction, before recording the exact resulting component bytes.
        // A cleanup failure must roll back graduation, not leave an approved
        // component with missing cleanup evidence or silently bypass Q1.
        // Validator Recipes intentionally retain this routing tag: Q1 selects
        // them by validates_class_code plus 05:validator, even after graduation.
        if class_code == 21 {
            tx.execute(
                "UPDATE reborn_recipes
                 SET consumer_tags=array_remove(consumer_tags, '05:validator')
                 WHERE id=$1 AND tenant_id=$2 AND user_id=$3 AND agent_id=$4
                   AND project_id=$5 AND class_code=21 AND validates_class_code IS NULL",
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

        // Capture the actual updated row while its write lock is held. This
        // immutable legacy receipt is evidence retention, not an exact-version
        // Skill association approval or permission to invoke a Tool.
        let component_sql = format!(
            "SELECT to_jsonb(c)::text FROM {table} c WHERE id=$1
             AND tenant_id=$2 AND user_id=$3 AND agent_id=$4 AND project_id=$5
             AND class_code=$6"
        );
        let component_bytes: String = tx
            .query_one(
                &component_sql,
                &[
                    &component_id,
                    &scope.tenant_id,
                    &scope.user_id,
                    &scope.agent_id,
                    &scope.project_id,
                    &class_code,
                ],
            )
            .await
            .map_err(map_pg)?
            .get(0);
        tx.execute(
            "INSERT INTO reborn_component_graduation_receipts
             (id, component_id, component_class, tenant_id, user_id, agent_id, project_id,
              queue_id, q2_actor, component_bytes, component_checksum, queue_bytes, queue_checksum)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,
                encode(sha256(convert_to($10::text,'UTF8')),'hex'),$11,
                encode(sha256(convert_to($11::text,'UTF8')),'hex'))",
            &[
                &Uuid::new_v4(),
                &component_id,
                &class_code,
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &queue_id,
                &q2_actor,
                &component_bytes,
                &queue_bytes,
            ],
        )
        .await
        .map_err(map_pg)?;

        // The immutable receipt retains the actor after queue deletion. Keep
        // the queue stamp coherent too; this row is locked and must still exist.
        if let Some(actor) = q2_actor {
            let stamped = tx
                .execute(
                    "UPDATE reborn_validation_queue
                 SET q2_actor = $6
                 WHERE tenant_id = $1 AND user_id = $2
                   AND agent_id = $3 AND project_id = $4
                   AND component_id = $5",
                    &[
                        &scope.tenant_id,
                        &scope.user_id,
                        &scope.agent_id,
                        &scope.project_id,
                        &component_id,
                        &actor,
                    ],
                )
                .await
                .map_err(map_pg)?;
            if stamped != 1 {
                tx.rollback().await.map_err(map_pg)?;
                return Err(ValidationQueueError::NotFound { component_id });
            }
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

        // The attached prefix store shares this graduation transaction. A
        // failed invalidation preserves the queue/review and rolls back the
        // component and receipt; no post-commit failure can invite replay.
        #[cfg(feature = "postgres")]
        if let Some(store) = &self.basic_prompt_store {
            store
                .mark_stale_in_transaction(&tx, &scope.user_id, &scope.project_id)
                .await
                .map_err(|error| ValidationQueueError::Db {
                    reason: error.to_string(),
                })?;
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
                            submitted_at, updated_at, q2_actor
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
                            submitted_at, updated_at, q2_actor
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
                "SELECT component_id
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

            let tx = client.transaction().await.map_err(map_pg)?;

            // Re-read under the same queue-before-component lock order as
            // approval. The discovery query is not permission to delete a row
            // whose state/payload changed while waiting for this transaction.
            let current = tx
                .query_opt(
                    "SELECT component_class, proposed_payload FROM reborn_validation_queue
                 WHERE tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4
                   AND component_id=$5 AND state=4 FOR UPDATE",
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
            let Some(current) = current else {
                tx.rollback().await.map_err(map_pg)?;
                continue;
            };
            let class_code: i16 = current.get(0);
            let proposed_payload: Option<Value> = current.get(1);

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
                       AND agent_id = $4 AND project_id = $5 AND class_code = $6"
                );
                tx.execute(
                    del_sql.as_str(),
                    &[
                        &component_id,
                        &scope.tenant_id,
                        &scope.user_id,
                        &scope.agent_id,
                        &scope.project_id,
                        &class_code,
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

    /// Composition-time referential integrity failure (HI.1 / Gap 2): set the
    /// component's `validation_status = 'pending'` and re-insert it into the
    /// validation queue in one transaction. This is called by
    /// `handle_compose_orchestrator` when `PgCompositionPort::compose` returns
    /// `ComponentPortError::IncludeNotResolved` — an include UUID the component
    /// declared no longer resolves to a validated component.
    ///
    /// The component is re-queued at state 1 (`Q1_pending`) with a new
    /// `validation_errors` entry describing the broken include. The queue
    /// `submit` path uses `ON CONFLICT DO NOTHING` so calling `invalidate` when
    /// a queue row already exists (e.g. concurrent invalidation) is idempotent.
    #[cfg(feature = "postgres")]
    pub async fn invalidate(
        &self,
        scope: &ComponentScope,
        component_id: Uuid,
        class_code: i32,
        reason: &str,
    ) -> Result<(), ValidationQueueError> {
        let class_i16: i16 = class_code
            .try_into()
            .map_err(|_| ValidationQueueError::UnknownClass { class_code })?;
        let table = resolve_component_table(class_code)
            .ok_or(ValidationQueueError::UnknownClass { class_code })?;

        let set_pending_sql = format!(
            "UPDATE {table}
             SET validation_status = 'pending', updated_at = now()
             WHERE id = $1
               AND tenant_id = $2 AND user_id = $3
               AND agent_id = $4 AND project_id = $5 AND class_code = $6"
        );

        let mut client = self.pool.get().await.map_err(map_pool)?;
        let tx = client.transaction().await.map_err(map_pg)?;

        // Approval/purge take the queue lock before the component lock. A
        // consistent order prevents a queue/component inversion deadlock.
        tx.query_opt(
            "SELECT id FROM reborn_validation_queue
             WHERE tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4
               AND component_id=$5 FOR UPDATE",
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

        // 1. Flip the component row back to 'pending'.
        tx.execute(
            set_pending_sql.as_str(),
            &[
                &component_id,
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &class_i16,
            ],
        )
        .await
        .map_err(map_pg)?;

        // 2. Re-insert into the validation queue (state 1, no proposed_payload).
        //    ON CONFLICT DO NOTHING: idempotent if a queue row already exists.
        tx.execute(
            "INSERT INTO reborn_validation_queue
                 (tenant_id, user_id, agent_id, project_id,
                  component_id, component_class, state, proposed_payload,
                  validation_errors)
             VALUES ($1, $2, $3, $4, $5, $6, 1, NULL, ARRAY[$7::text])
             ON CONFLICT
                 (tenant_id, user_id, agent_id, project_id, component_id)
             DO NOTHING",
            &[
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &component_id,
                &class_i16,
                &reason,
            ],
        )
        .await
        .map_err(map_pg)?;

        tx.commit().await.map_err(map_pg)?;
        Ok(())
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
        q2_actor: row.get(10),
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

/// Resolve the primary writable content column name for a component class.
///
/// Returns `(column_name, is_jsonb)` — `is_jsonb = true` means the column is
/// JSONB and the payload value must be cast with `::jsonb` in the UPDATE.
/// Used by [`apply_upgrade_payload`] to update the live row on upgrade-copy
/// graduation (§0.23.5 / Phase N).
///
/// Column map verified against migration DDL:
/// - V027 skills:                 `body TEXT`
/// - V029 actions:                `steps JSONB`
/// - V032 extensions_unified:     `payload JSONB`
/// - V033 recipes:                `steps JSONB`
/// - V036 specs:                  `content TEXT`
/// - V037 tool_skills:            `content TEXT`
/// - V038–V043 plans/summaries/docus/lessons/issues/notes: `content TEXT`
/// - V052 python_code:            `content TEXT`
/// - V053 extension_catalogues:   `overview_doc TEXT`
/// - V030 tools (class 0):        no single body column → `None`
fn resolve_content_column(class_code: i16) -> Option<(&'static str, bool)> {
    match class_code {
        // Skills (1–3, 10, 50): body TEXT
        1..=3 | 10 | 50 => Some(("body", false)),
        // Extensions unified (4–9): payload JSONB
        4..=9 => Some(("payload", true)),
        // ToolSkills (13) + doc-type classes (12, 14–20) + PythonCode (22): content TEXT
        12..=15 | 17..=20 | 22 => Some(("content", false)),
        // Actions (16): steps JSONB
        16 => Some(("steps", true)),
        // Recipes (21): steps JSONB
        21 => Some(("steps", true)),
        // ExtensionCatalogues (23): overview_doc TEXT
        23 => Some(("overview_doc", false)),
        // Tools (0): no single content column; name+description suffice.
        _ => None,
    }
}

fn validate_upgrade_payload(
    class_code: i32,
    component_id: Uuid,
    payload: &Value,
) -> Result<(), ValidationQueueError> {
    resolve_component_table(class_code).ok_or(ValidationQueueError::UnknownClass { class_code })?;
    let invalid = || ValidationQueueError::InvalidPayload { component_id };
    let record = payload
        .as_object()
        .filter(|value| !value.is_empty())
        .ok_or_else(invalid)?;
    if record
        .keys()
        .any(|key| !["name", "description", "content"].contains(&key.as_str()))
    {
        return Err(invalid());
    }
    for key in ["name", "description"] {
        if record.get(key).is_some_and(|value| !value.is_string()) {
            return Err(invalid());
        }
    }
    if let Some(value) = record.get("content") {
        let (_, json) = resolve_content_column(class_code as i16).ok_or_else(invalid)?;
        if (json && value.is_null()) || (!json && !value.is_string()) {
            return Err(invalid());
        }
    }
    Ok(())
}

/// Apply an upgrade `proposed_payload` to the live component row inside a
/// transaction. Updates `name`, `description`, and the primary content column
/// (class-specific — see [`resolve_content_column`]) from the JSONB payload.
/// Leaves `validation_status = 'validated'` and all other columns untouched.
/// Missing writable keys leave their columns unchanged. Unknown keys and invalid
/// types fail; an approval must never silently approve different content.
///
/// Returns the row count (0 = component not found, 1 = updated).
async fn apply_upgrade_payload(
    tx: &tokio_postgres::Transaction<'_>,
    table: &str,
    class_code: i16,
    component_id: uuid::Uuid,
    scope: &ComponentScope,
    payload: &Value,
) -> Result<u64, ValidationQueueError> {
    validate_upgrade_payload(i32::from(class_code), component_id, payload)?;
    let name: Option<String> = payload
        .get("name")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let description: Option<String> = payload
        .get("description")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let content_col = resolve_content_column(class_code);

    // Resolve the primary content value from the payload.
    // Callers encode the content under the well-known key "content" regardless of
    // which DB column it maps to (the class→column dispatch is done here).
    let content_info: Option<((&'static str, bool), String)> =
        if let Some((col, is_jsonb)) = content_col {
            payload.get("content").map(|v| {
                let s = if is_jsonb {
                    v.to_string()
                } else {
                    v.as_str().expect("validated text content").to_owned()
                };
                ((col, is_jsonb), s)
            })
        } else {
            None
        };

    // Build a dynamic SET clause from the present payload fields.
    // Only fields present in the payload are overwritten; others remain unchanged.
    let has_name = name.is_some();
    let has_description = description.is_some();
    let has_content = content_info.is_some();

    let mut set_parts: Vec<String> = vec!["updated_at = now()".to_string()];
    let mut next_param: usize = 7;

    if has_name {
        set_parts.push(format!("name = ${next_param}"));
        next_param += 1;
    }
    if has_description {
        set_parts.push(format!("description = ${next_param}"));
        next_param += 1;
    }
    if let Some(((col, is_jsonb), _)) = &content_info {
        if *is_jsonb {
            set_parts.push(format!("{col} = ${next_param}::jsonb"));
        } else {
            set_parts.push(format!("{col} = ${next_param}"));
        }
    }

    let set_clause = set_parts.join(", ");
    let update_sql = format!(
        "UPDATE {table}
         SET {set_clause}
         WHERE id = $1
           AND tenant_id = $2 AND user_id = $3
           AND agent_id = $4 AND project_id = $5 AND class_code = $6"
    );

    // Consume Options into owned Strings before building the params slice.
    let name_str: String = name.unwrap_or_default();
    let description_str: String = description.unwrap_or_default();

    let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = vec![
        &component_id,
        &scope.tenant_id,
        &scope.user_id,
        &scope.agent_id,
        &scope.project_id,
        &class_code,
    ];
    // Append extras in the same order the SET placeholders were pushed.
    if has_name {
        params.push(&name_str);
    }
    if has_description {
        params.push(&description_str);
    }
    if has_content && let Some((_, ref content_val)) = content_info {
        params.push(content_val);
    }

    tx.execute(update_sql.as_str(), &params)
        .await
        .map_err(map_pg)
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
        // No data migration / DROPs (those are V077 / Phase N).
        assert!(!sql.contains("DROP COLUMN"), "V051 must not drop columns");
        assert!(
            !sql.contains("INSERT INTO reborn_validation_queue"),
            "V051 must not populate rows"
        );
    }

    #[test]
    fn v077_migration_populates_queue_and_drops_legacy_columns() {
        let sql = include_str!(
            "../../brassclaw_pg/migrations/V077__reborn_validation_queue_populate.sql"
        );
        // Must NOT re-create the queue table (that's V051).
        // Use a newline-anchored check so the comment "-- Step 1: CREATE TABLE is NOT here"
        // (which contains the substring but is not DDL) does not trigger the assertion.
        let has_create_table_ddl = sql
            .lines()
            .any(|l| l.trim_start().starts_with("CREATE TABLE"));
        assert!(
            !has_create_table_ddl,
            "V077 must not CREATE TABLE — table is in V051"
        );
        // Step 2: populate arms present for all 15 component tables.
        assert!(
            sql.contains("FROM reborn_recipes"),
            "V077 must populate from reborn_recipes"
        );
        assert!(
            sql.contains("FROM reborn_skills"),
            "V077 must populate from reborn_skills"
        );
        assert!(
            sql.contains("FROM reborn_tools"),
            "V077 must populate from reborn_tools"
        );
        assert!(
            sql.contains("FROM reborn_tool_skills"),
            "V077 must populate from reborn_tool_skills"
        );
        assert!(
            sql.contains("FROM reborn_actions"),
            "V077 must populate from reborn_actions"
        );
        assert!(
            sql.contains("FROM reborn_python_code"),
            "V077 must populate from reborn_python_code"
        );
        assert!(
            sql.contains("FROM reborn_extension_catalogues"),
            "V077 must populate from reborn_extension_catalogues"
        );
        // ON CONFLICT DO NOTHING — idempotent.
        assert!(
            sql.contains("ON CONFLICT") && sql.contains("DO NOTHING"),
            "V077 populate must be idempotent"
        );
        // Step 3: last_graduation_at column.
        assert!(
            sql.contains("last_graduation_at"),
            "V077 must add last_graduation_at"
        );
        assert!(
            sql.contains("ADD COLUMN IF NOT EXISTS last_graduation_at"),
            "last_graduation_at must be IF NOT EXISTS"
        );
        // Step 4: graduation trigger.
        assert!(
            sql.contains("reborn_validation_queue_graduation"),
            "V077 must create graduation trigger function"
        );
        assert!(
            sql.contains("reborn_validation_queue_on_delete"),
            "V077 must create the AFTER DELETE trigger"
        );
        // Step 5: the only real DROP is on reborn_recipes (FIND-N-03).
        assert!(
            sql.contains("ALTER TABLE reborn_recipes"),
            "V077 must drop columns from reborn_recipes"
        );
        assert!(
            sql.contains("DROP COLUMN IF EXISTS queue_code"),
            "V077 must drop queue_code"
        );
        assert!(
            sql.contains("DROP COLUMN IF EXISTS review_attempts"),
            "V077 must drop review_attempts"
        );
        assert!(
            sql.contains("DROP COLUMN IF EXISTS review_feedback"),
            "V077 must drop review_feedback"
        );
        assert!(
            sql.contains("DROP COLUMN IF EXISTS rejected_at"),
            "V077 must drop rejected_at"
        );
        assert!(
            sql.contains("DROP COLUMN IF EXISTS validation_errors"),
            "V077 must drop validation_errors"
        );
        // validation_status is NOT dropped.
        assert!(
            !sql.contains("DROP COLUMN IF EXISTS validation_status"),
            "V077 must NOT drop validation_status"
        );
    }

    // Native PostgreSQL tests apply the complete schema and fail on setup errors.

    mod pg {
        use super::*;
        use brassclaw_engine::memory::retrieval_source::ComponentScope;
        use brassclaw_pg::PgPool;

        async fn pg_rig() -> crate::runtime::test_pg::native_pg::NativePostgres {
            crate::runtime::test_pg::native_pg::NativePostgres::start().await
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
            let rig = pg_rig().await;
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
            let rig = pg_rig().await;
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
                .submit(
                    &scope,
                    cid,
                    20,
                    Some(serde_json::json!({"content": "edited"})),
                )
                .await
                .expect_err("duplicate submit must error");
            assert!(matches!(err, ValidationQueueError::AlreadyQueued { .. }));
        }

        #[tokio::test]
        async fn gate1_pass_and_gate1_fail_transition_correctly() {
            let rig = pg_rig().await;
            let scope = test_scope();
            let store = ValidationQueueStore::new(rig.pool.clone());

            // gate1_pass: 1 → 2, errors cleared.
            let a = insert_pending_note(&rig.pool, &scope).await;
            store.submit(&scope, a, 20, None).await.unwrap();
            assert!(matches!(
                store
                    .gate1_pass(&scope, a, &["unresolved validation failure".into()])
                    .await,
                Err(ValidationQueueError::InvalidGate1Result { .. })
            ));
            assert_eq!(
                store.list(&scope, Some(1)).await.unwrap()[0].state,
                STATE_Q1_PENDING
            );
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
            let rig = pg_rig().await;
            let scope = test_scope();
            let store = ValidationQueueStore::new(rig.pool.clone()); // threshold 3
            let cid = insert_pending_note(&rig.pool, &scope).await;
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
            let rig = pg_rig().await;
            let scope = test_scope();
            // threshold 1 → first rejection promotes to state 4.
            let store = ValidationQueueStore::with_reject_threshold(rig.pool.clone(), 1);
            let cid = insert_pending_note(&rig.pool, &scope).await;
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
        async fn q1_result_cannot_certify_a_changed_candidate_or_submission() {
            let rig = pg_rig().await;
            let scope = test_scope();
            let store = ValidationQueueStore::new(rig.pool.clone());
            for change_submission in [false, true] {
                let cid = insert_pending_note(&rig.pool, &scope).await;
                store.submit(&scope, cid, 20, None).await.unwrap();
                let review = store.capture_q1_candidate(&scope, cid).await.unwrap();
                let client = rig.pool.get().await.unwrap();
                if change_submission {
                    client.execute("UPDATE reborn_validation_queue SET proposed_payload=$2 WHERE component_id=$1",
                        &[&cid, &serde_json::json!({"content":"changed while Q1 runs"})]).await.unwrap();
                } else {
                    client
                        .execute(
                            "UPDATE reborn_notes SET content='changed while Q1 runs' WHERE id=$1",
                            &[&cid],
                        )
                        .await
                        .unwrap();
                }
                assert!(matches!(
                    store.gate1_pass_reviewed(&scope, &review, &[]).await,
                    Err(ValidationQueueError::ReviewChanged { .. })
                ));
                assert!(matches!(
                    store
                        .gate1_fail_reviewed(&scope, &review, &["stale failure".into()])
                        .await,
                    Err(ValidationQueueError::ReviewChanged { .. })
                ));
                let rows = store.list(&scope, Some(1)).await.unwrap();
                let queued = rows.iter().find(|row| row.component_id == cid).unwrap();
                assert!(queued.validation_errors.is_empty());
            }
        }

        #[tokio::test]
        async fn q2_never_approves_content_or_proposals_changed_after_q1() {
            let rig = pg_rig().await;
            let scope = test_scope();
            let store = ValidationQueueStore::new(rig.pool.clone());
            for change_submission in [false, true] {
                let cid = insert_pending_note(&rig.pool, &scope).await;
                store
                    .submit(
                        &scope,
                        cid,
                        20,
                        Some(serde_json::json!({"content":"reviewed replacement"})),
                    )
                    .await
                    .unwrap();
                store.gate1_pass(&scope, cid, &[]).await.unwrap();
                let client = rig.pool.get().await.unwrap();
                if change_submission {
                    client.execute("UPDATE reborn_validation_queue SET proposed_payload=$2 WHERE component_id=$1",
                        &[&cid, &serde_json::json!({"content":"unreviewed replacement"})]).await.unwrap();
                } else {
                    client
                        .execute(
                            "UPDATE reborn_notes SET description='unreviewed metadata' WHERE id=$1",
                            &[&cid],
                        )
                        .await
                        .unwrap();
                }
                assert!(matches!(
                    store.approve(&scope, cid, Some("human")).await,
                    Err(ValidationQueueError::ReviewChanged { .. })
                ));
                assert_eq!(read_note_status(&rig.pool, &scope, cid).await, "pending");
                assert!(
                    store
                        .list(&scope, Some(2))
                        .await
                        .unwrap()
                        .iter()
                        .any(|row| row.component_id == cid)
                );
                let receipts: i64 = client.query_one("SELECT count(*) FROM reborn_component_graduation_receipts WHERE component_id=$1", &[&cid]).await.unwrap().get(0);
                assert_eq!(receipts, 0);
            }
        }

        #[tokio::test]
        async fn accounting_changes_between_q1_and_q2_do_not_change_authoring_review() {
            let rig = pg_rig().await;
            let scope = test_scope();
            let store = ValidationQueueStore::new(rig.pool.clone());
            let cid = insert_pending_note(&rig.pool, &scope).await;
            store.submit(&scope, cid, 20, None).await.unwrap();
            store.gate1_pass(&scope, cid, &[]).await.unwrap();
            rig.pool
                .get()
                .await
                .unwrap()
                .execute(
                    "UPDATE reborn_notes SET last_audit_at=now(), audit_failure_count=audit_failure_count+1 WHERE id=$1",
                    &[&cid],
                )
                .await
                .unwrap();
            store.approve(&scope, cid, Some("human")).await.unwrap();
            assert_eq!(read_note_status(&rig.pool, &scope, cid).await, "validated");
        }

        /// Real queue/receipt persistence fixture. Its sealed Q1 state is test
        /// setup, not a claim that a behavioral validator or v3 approval ran.
        #[tokio::test]
        async fn public_q2_rejects_missing_or_bootstrap_markers_without_mutating_review() {
            let rig = pg_rig().await;
            let scope = test_scope();
            let store = ValidationQueueStore::new(rig.pool.clone());
            let cid = insert_pending_note(&rig.pool, &scope).await;
            store.submit(&scope, cid, 20, None).await.unwrap();
            store.gate1_pass(&scope, cid, &[]).await.unwrap();
            for marker in [None, Some("builtin"), Some("sempai"), Some("")] {
                assert!(matches!(store.approve(&scope, cid, marker).await,
                    Err(ValidationQueueError::InvalidQ2Actor { component_id }) if component_id == cid));
                assert_eq!(read_note_status(&rig.pool, &scope, cid).await, "pending");
                let rows = store.list(&scope, Some(2)).await.unwrap();
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].component_id, cid);
                let count: i64 = rig.pool.get().await.unwrap().query_one(
                    "SELECT count(*) FROM reborn_component_graduation_receipts WHERE component_id=$1", &[&cid]
                ).await.unwrap().get(0);
                assert_eq!(count, 0);
            }
        }

        #[tokio::test]
        async fn approve_graduates_component_and_deletes_queue_row() {
            let rig = pg_rig().await;
            let scope = test_scope();
            let store = ValidationQueueStore::new(rig.pool.clone());
            let cid = insert_pending_note(&rig.pool, &scope).await;
            store.submit(&scope, cid, 20, None).await.expect("submit");
            store.gate1_pass(&scope, cid, &[]).await.expect("pass");
            let returned = store
                .approve(&scope, cid, Some("human"))
                .await
                .expect("approve");
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
        async fn approval_rechecks_review_state_after_a_concurrent_rejection_commits() {
            let rig = pg_rig().await;
            let scope = test_scope();
            let store = Arc::new(ValidationQueueStore::new(rig.pool.clone()));
            let cid = insert_pending_note(&rig.pool, &scope).await;
            store.submit(&scope, cid, 20, None).await.unwrap();
            store.gate1_pass(&scope, cid, &[]).await.unwrap();

            // Keep an actual rejection write uncommitted. Approval's SELECT
            // must wait for this row and then observe state 3; an unlocked read
            // of state 2 followed by a later DELETE would graduate stale review.
            let mut rejecting = rig.pool.get().await.unwrap();
            let tx = rejecting.transaction().await.unwrap();
            tx.execute(
                "UPDATE reborn_validation_queue SET state=3, counter=counter+1,
                 review_feedback='concurrent rejection' WHERE component_id=$1",
                &[&cid],
            )
            .await
            .unwrap();
            let approving = Arc::clone(&store);
            let approval_scope = scope.clone();
            let approval = tokio::spawn(async move {
                approving.approve(&approval_scope, cid, Some("human")).await
            });
            let observer = rig.pool.get().await.unwrap();
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                loop {
                    let blocked: i64 = observer
                        .query_one(
                            "SELECT count(*) FROM pg_stat_activity WHERE datname=current_database()
                         AND pid<>pg_backend_pid() AND state='active' AND wait_event_type='Lock'
                         AND query LIKE '%reborn_validation_queue%'",
                            &[],
                        )
                        .await
                        .unwrap()
                        .get(0);
                    if blocked > 0 {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("approval reaches the actual database row lock");
            tx.commit().await.unwrap();
            let failure = tokio::time::timeout(std::time::Duration::from_secs(5), approval)
                .await
                .unwrap()
                .unwrap()
                .unwrap_err();
            assert!(matches!(
                failure,
                ValidationQueueError::NotQ1Passed {
                    state: STATE_REJECTED,
                    ..
                }
            ));
            assert_eq!(read_note_status(&rig.pool, &scope, cid).await, "pending");
            let queued = store.list(&scope, Some(3)).await.unwrap();
            assert_eq!(queued.len(), 1);
            assert_eq!(queued[0].component_id, cid);
            assert_eq!(
                queued[0].review_feedback.as_deref(),
                Some("concurrent rejection")
            );
        }

        #[tokio::test]
        async fn malformed_upgrade_payloads_never_change_or_graduate_a_component() {
            let rig = pg_rig().await;
            let scope = test_scope();
            let store = ValidationQueueStore::new(rig.pool.clone());
            let cid = insert_pending_note(&rig.pool, &scope).await;
            for payload in [
                serde_json::json!({"unknown":"ignored before"}),
                serde_json::json!({"content":7}),
                serde_json::json!({"name":null}),
                serde_json::json!({}),
                serde_json::json!([]),
            ] {
                assert!(matches!(
                    store.submit(&scope, cid, 20, Some(payload)).await,
                    Err(ValidationQueueError::InvalidPayload { .. })
                ));
            }
            assert!(store.list(&scope, None).await.unwrap().is_empty());
            store.submit(&scope, cid, 20, None).await.unwrap();
            store.gate1_pass(&scope, cid, &[]).await.unwrap();
            // A corrupt persisted/older-writer payload must also fail at the
            // final application boundary, preserving the queue for diagnosis.
            let client = rig.pool.get().await.unwrap();
            client
                .execute(
                    "UPDATE reborn_validation_queue SET proposed_payload=$2 WHERE component_id=$1",
                    &[&cid, &serde_json::json!({"content":false})],
                )
                .await
                .unwrap();
            assert!(matches!(
                store.approve(&scope, cid, Some("human")).await,
                Err(ValidationQueueError::InvalidPayload { .. })
            ));
            assert_eq!(read_note_status(&rig.pool, &scope, cid).await, "pending");
            assert_eq!(store.list(&scope, Some(2)).await.unwrap().len(), 1);
        }

        #[tokio::test]
        async fn approve_unknown_class_preserves_queue_without_mutation() {
            let rig = pg_rig().await;
            let scope = test_scope();
            let store = ValidationQueueStore::new(rig.pool.clone());
            let cid = insert_pending_note(&rig.pool, &scope).await;
            // Corrupt a reviewed ticket to an unknown class; graduation must refuse it.
            store.submit(&scope, cid, 20, None).await.expect("submit");
            store.gate1_pass(&scope, cid, &[]).await.unwrap();
            rig.pool
                .get()
                .await
                .unwrap()
                .execute(
                    "UPDATE reborn_validation_queue SET component_class=11 WHERE component_id=$1",
                    &[&cid],
                )
                .await
                .unwrap();
            let err = store
                .approve(&scope, cid, Some("human"))
                .await
                .expect_err("unknown class must error without mutation");
            assert!(
                matches!(err, ValidationQueueError::UnknownClass { class_code: 11 }),
                "wrong error: {err:?}"
            );
            // Queue row preserved after the read-only transaction rolls back.
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
            let rig = pg_rig().await;
            let scope = test_scope();
            let store = ValidationQueueStore::new(rig.pool.clone());
            // The reviewed component disappears before Q2.
            let cid = insert_pending_note(&rig.pool, &scope).await;
            store.submit(&scope, cid, 20, None).await.unwrap();
            store.gate1_pass(&scope, cid, &[]).await.unwrap();
            rig.pool
                .get()
                .await
                .unwrap()
                .execute("DELETE FROM reborn_notes WHERE id=$1", &[&cid])
                .await
                .unwrap();
            let err = store
                .approve(&scope, cid, Some("human"))
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
        async fn approve_upgrade_copy_applies_payload_phase_n() {
            // Phase N: upgrade-copy graduation applies proposed_payload to the
            // live component row and removes the queue row (§0.23.5).
            let rig = pg_rig().await;
            let scope = test_scope();
            let store = ValidationQueueStore::new(rig.pool.clone());
            let client = rig.pool.get().await.expect("pool");

            // Insert a validated reborn_notes (class 20) row as the "live" row.
            let note_id = Uuid::new_v4();
            let note_name = format!("note-{}", Uuid::new_v4());
            client
                .execute(
                    "INSERT INTO reborn_notes
                         (id, tenant_id, user_id, agent_id, project_id, name, validation_status)
                     VALUES ($1,$2,$3,$4,$5,$6,'validated')",
                    &[
                        &note_id,
                        &scope.tenant_id,
                        &scope.user_id,
                        &scope.agent_id,
                        &scope.project_id,
                        &note_name,
                    ],
                )
                .await
                .expect("insert live note");

            // Submit an upgrade copy: proposed_payload carries the edited content.
            // The well-known key "content" is used by callers regardless of which
            // DB column the class maps to (class 20 notes → `content TEXT`).
            let payload = serde_json::json!({"content": "upgraded content text"});
            store
                .submit(&scope, note_id, 20, Some(payload))
                .await
                .expect("submit upgrade copy");
            store.gate1_pass(&scope, note_id, &[]).await.unwrap();

            // Approve — should succeed and apply the upgrade (Phase N).
            let result = store.approve(&scope, note_id, Some("human")).await;
            assert!(
                result.is_ok(),
                "upgrade graduation must succeed: {result:?}"
            );

            // Queue row deleted.
            let queue_rows = store.list(&scope, None).await.unwrap();
            assert!(
                !queue_rows.iter().any(|r| r.component_id == note_id),
                "queue row must be deleted after graduation"
            );

            // Live row still validated; content updated.
            let row = client
                .query_one(
                    "SELECT validation_status, content FROM reborn_notes WHERE id = $1",
                    &[&note_id],
                )
                .await
                .expect("live note still exists");
            let vs: String = row.get(0);
            let content: String = row.get(1);
            assert_eq!(vs, "validated", "validation_status must stay 'validated'");
            assert_eq!(
                content, "upgraded content text",
                "content must be updated from payload"
            );
        }

        #[tokio::test]
        async fn immutable_graduation_receipts_retain_exact_rows_across_replacement_and_deletion() {
            use sha2::{Digest, Sha256};
            let rig = pg_rig().await;
            let scope = test_scope();
            let store = ValidationQueueStore::new(rig.pool.clone());
            let cid = insert_pending_note(&rig.pool, &scope).await;
            let client = rig.pool.get().await.unwrap();
            client
                .execute(
                    "INSERT INTO reborn_monty_vm_settings
                 (tenant_id,user_id,agent_id,project_id,max_duration_secs,revision)
                 VALUES ($1,$2,$3,$4,900,1)",
                    &[
                        &scope.tenant_id,
                        &scope.user_id,
                        &scope.agent_id,
                        &scope.project_id,
                    ],
                )
                .await
                .unwrap();
            store.submit(&scope, cid, 20, None).await.unwrap();
            store.gate1_pass(&scope, cid, &[]).await.unwrap();
            store.approve(&scope, cid, Some("human")).await.unwrap();
            let original = client.query_one(
                "SELECT id, component_bytes, component_checksum, queue_bytes, queue_checksum, q2_actor
                 FROM reborn_component_graduation_receipts WHERE component_id=$1", &[&cid],
            ).await.unwrap();
            let receipt: Uuid = original.get(0);
            let component_bytes: String = original.get(1);
            let queue_bytes: String = original.get(3);
            assert_eq!(
                original.get::<_, String>(2),
                format!("{:x}", Sha256::digest(component_bytes.as_bytes()))
            );
            assert_eq!(
                original.get::<_, String>(4),
                format!("{:x}", Sha256::digest(queue_bytes.as_bytes()))
            );
            assert_eq!(
                original.get::<_, Option<String>>(5).as_deref(),
                Some("human")
            );
            assert_eq!(
                serde_json::from_str::<Value>(&queue_bytes).unwrap()["state"],
                2
            );
            assert_eq!(
                serde_json::from_str::<Value>(&component_bytes).unwrap()["validation_status"],
                "validated"
            );

            store
                .submit(
                    &scope,
                    cid,
                    20,
                    Some(serde_json::json!({"content":"replacement"})),
                )
                .await
                .unwrap();
            store.gate1_pass(&scope, cid, &[]).await.unwrap();
            store.approve(&scope, cid, Some("human")).await.unwrap();
            let settings = client
                .query_one(
                    "SELECT revision,max_duration_secs,last_graduation_at IS NOT NULL
                 FROM reborn_monty_vm_settings
                 WHERE tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4",
                    &[
                        &scope.tenant_id,
                        &scope.user_id,
                        &scope.agent_id,
                        &scope.project_id,
                    ],
                )
                .await
                .unwrap();
            assert_eq!(settings.get::<_, i64>(0), 1);
            assert_eq!(settings.get::<_, i32>(1), 900);
            assert!(settings.get::<_, bool>(2));
            let newer = client.query_one("SELECT component_bytes FROM reborn_component_graduation_receipts WHERE component_id=$1 AND id<>$2", &[&cid,&receipt]).await.unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(&newer.get::<_, String>(0)).unwrap()["content"],
                "replacement"
            );
            client
                .execute("DELETE FROM reborn_notes WHERE id=$1", &[&cid])
                .await
                .unwrap();
            let retained = client
                .query_one(
                    "SELECT component_bytes FROM reborn_component_graduation_receipts WHERE id=$1",
                    &[&receipt],
                )
                .await
                .unwrap();
            assert_eq!(retained.get::<_, String>(0), component_bytes);
            for sql in [
                "UPDATE reborn_component_graduation_receipts SET q2_actor='altered' WHERE id=$1",
                "DELETE FROM reborn_component_graduation_receipts WHERE id=$1",
            ] {
                let failure = client.execute(sql, &[&receipt]).await.unwrap_err();
                assert_eq!(
                    failure.code(),
                    Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
                );
            }
            assert_eq!(
                client
                    .batch_execute("TRUNCATE reborn_component_graduation_receipts")
                    .await
                    .unwrap_err()
                    .code(),
                Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
            );
            // A forged checksum cannot satisfy the stored exact-byte contract.
            let invalid_checksum = client.execute(
                "INSERT INTO reborn_component_graduation_receipts
                 SELECT $1,component_id,component_class,tenant_id,user_id,agent_id,project_id,$2,
                 q2_actor,component_bytes,'wrong',
                 jsonb_set(queue_bytes::jsonb,'{id}',to_jsonb($2::uuid))::text,
                 encode(sha256(convert_to(jsonb_set(queue_bytes::jsonb,'{id}',to_jsonb($2::uuid))::text,'UTF8')),'hex'),now()
                 FROM reborn_component_graduation_receipts WHERE id=$3",
                &[&Uuid::new_v4(),&Uuid::new_v4(),&receipt],
            ).await.unwrap_err();
            assert_eq!(
                invalid_checksum.as_db_error().unwrap().constraint(),
                Some("component_graduation_exact_component_checksum")
            );
        }

        #[tokio::test]
        async fn approval_cannot_reclassify_an_orchestrator_as_a_usage_skill() {
            let rig = pg_rig().await;
            let scope = test_scope();
            let store = ValidationQueueStore::with_reject_threshold(rig.pool.clone(), 1);
            let client = rig.pool.get().await.unwrap();
            let cid: Uuid = client.query_one(
                "INSERT INTO reborn_skills (tenant_id,user_id,agent_id,project_id,name,description,body,class_code,validation_status)
                 VALUES ($1,$2,$3,$4,'protected-orchestrator','test orchestration component','result = 1',10,'pending') RETURNING id",
                &[&scope.tenant_id,&scope.user_id,&scope.agent_id,&scope.project_id],
            ).await.unwrap().get(0);
            // Corrupt the reviewed class; final dispatch remains class-checked.
            store.submit(&scope, cid, 10, None).await.unwrap();
            store.gate1_pass(&scope, cid, &[]).await.unwrap();
            client
                .execute(
                    "UPDATE reborn_validation_queue SET component_class=1 WHERE component_id=$1",
                    &[&cid],
                )
                .await
                .unwrap();
            assert!(matches!(
                store.approve(&scope, cid, Some("human")).await,
                Err(ValidationQueueError::ComponentMissing { .. })
            ));
            let status: String = client
                .query_one(
                    "SELECT validation_status FROM reborn_skills WHERE id=$1",
                    &[&cid],
                )
                .await
                .unwrap()
                .get(0);
            assert_eq!(status, "pending");
            let count: i64 = client.query_one("SELECT count(*) FROM reborn_component_graduation_receipts WHERE component_id=$1",&[&cid]).await.unwrap().get(0);
            assert_eq!(count, 0);
            assert_eq!(store.list(&scope, Some(2)).await.unwrap().len(), 1);
            store
                .reject(&scope, cid, "wrong class ticket")
                .await
                .unwrap();
            assert_eq!(store.purge_deletion_candidates(&scope).await.unwrap(), 1);
            let retained: i64 = client
                .query_one(
                    "SELECT count(*) FROM reborn_skills WHERE id=$1 AND class_code=10",
                    &[&cid],
                )
                .await
                .unwrap()
                .get(0);
            assert_eq!(
                retained, 1,
                "purging a usage-Skill ticket cannot delete an Orchestrator row"
            );
        }

        /// Phase N: run_q1_validation defers when no validation Recipe is seeded.
        ///
        /// In a test environment `reborn_recipes` has no `05:validator`-tagged rows,
        /// so every Q1 attempt returns `Q1Outcome::Deferred` and leaves the queue
        /// row at state 1 (Q1_pending) — the graceful-defer path documented in §0.23.9.
        #[tokio::test]
        async fn run_q1_validation_defers_when_no_recipe_seeded() {
            use crate::q1_orchestrator::{Q1Outcome, run_q1_validation};
            let rig = pg_rig().await;
            let scope = test_scope();
            let store = ValidationQueueStore::new(rig.pool.clone());

            let cid = Uuid::new_v4();
            store.submit(&scope, cid, 20, None).await.unwrap();

            let outcome = run_q1_validation(&rig.pool, &scope, cid, 20, &store)
                .await
                .expect("run q1");

            // No validator Recipe in the test DB → Deferred, not Passed or Failed.
            assert!(
                matches!(outcome, Q1Outcome::Deferred { .. }),
                "expected Deferred when no Recipe is seeded; got {outcome:?}"
            );

            // Queue row must still be at state 1 (unchanged).
            let row = store
                .list(&scope, Some(STATE_Q1_PENDING as u8))
                .await
                .unwrap()
                .into_iter()
                .find(|r| r.component_id == cid)
                .expect("queue row at state 1");
            assert_eq!(row.state, STATE_Q1_PENDING);
        }

        /// submit → gate1_pass (pub(crate)) → approve: full graduation round-trip.
        ///
        /// Since orchestrated Q1 defers until validator Recipes are seeded, the
        /// round-trip test drives `gate1_pass` directly (the only path that writes
        /// state 2) so the Q2 approval path can be verified independently of Recipe
        /// availability.
        #[tokio::test]
        async fn integration_submit_gate1pass_approve_graduates() {
            let rig = pg_rig().await;
            let scope = test_scope();
            let store = ValidationQueueStore::new(rig.pool.clone());
            let cid = insert_pending_note(&rig.pool, &scope).await;

            store.submit(&scope, cid, 20, None).await.expect("submit");
            // Advance to state 2 via pub(crate) gate1_pass (simulates Q1 pass).
            store
                .gate1_pass(&scope, cid, &[])
                .await
                .expect("gate1_pass");
            store
                .approve(&scope, cid, Some("human"))
                .await
                .expect("approve");
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
            let rig = pg_rig().await;
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
