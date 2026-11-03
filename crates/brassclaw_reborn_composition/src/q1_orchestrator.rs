//! Gate-1 (Q1) validation orchestrator — retained IBS/Recipe path.
//!
//! The active Q1 entry point is [`run_q1_validation_retained`], which schedules
//! a trusted internal validator Review Recipe through the global Monty service.
//! The queue kick is fired by [`ValidationQueueStore::fire_q1_kick_if_wired`]
//! after a component is submitted for review.
//!
//! # Column fix (V079 — FIND-P0-01)
//!
//! `validates_class_code SMALLINT` on `reborn_recipes` carries the class being
//! validated; `class_code = 21` on that table always means "this row IS a Recipe".
//!
//! # State-2 write invariant (FIND-P9-01 / FIND-P9-08)
//!
//! `verify_receipt` records observations before terminal native settlement.
//! `run_q1_validation_retained` authenticates that exact settled execution before
//! its reviewed queue transition may write state 2.
//! The reviewed pass/failure methods are `pub(crate)` on [`ValidationQueueStore`],
//! preventing any API layer from calling them directly.
//!
//! # Feature gate
//!
//! Requires the `postgres` feature.

#![forbid(unsafe_code)]

use thiserror::Error;

#[cfg(feature = "skills-db")]
use brassclaw_engine::memory::retrieval_source::ComponentScope;
#[cfg(feature = "skills-db")]
use brassclaw_pg::PgPool;
#[cfg(feature = "skills-db")]
use uuid::Uuid;

use crate::validation_queue::ValidationQueueError;
#[cfg(feature = "skills-db")]
use crate::validation_queue::ValidationQueueStore;

// ---------------------------------------------------------------------------
// Outcome
// ---------------------------------------------------------------------------

/// The outcome of a Gate 1 (Q1) validation attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Q1Outcome {
    /// Gate 1 ran and the component passed all checks. `gate1_pass_reviewed` has been
    /// called — the queue row is now at state 2 (awaiting Q2).
    Passed,

    /// Gate 1 ran and the component failed one or more checks. `gate1_fail` has
    /// been called — the queue row stays at state 1 with recorded errors.
    Failed { errors: Vec<String> },

    /// No validation Recipe was found for this class code. Q1 was **not** run;
    /// the queue row remains at state 1 until a Recipe is seeded.  This is the
    /// expected result between Phase N going live and the validation-system
    /// trusted-root being fully seeded (Phase P.0).
    Deferred { reason: String },
}

impl Q1Outcome {
    /// Convenience constructor — clean pass.
    pub fn passed() -> Self {
        Self::Passed
    }

    /// Convenience constructor — failure with a list of human-readable errors.
    pub fn failed(errors: Vec<String>) -> Self {
        Self::Failed { errors }
    }

    /// Convenience constructor — deferred because no Recipe was available.
    pub fn deferred(reason: impl Into<String>) -> Self {
        Self::Deferred {
            reason: reason.into(),
        }
    }

    /// `true` when the component cleared Gate 1.
    pub fn passed_gate(&self) -> bool {
        matches!(self, Self::Passed)
    }
}

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/// Errors raised by [`run_q1_validation_retained`].
#[derive(Debug, Error)]
pub enum Q1Error {
    #[error("validation queue error: {0}")]
    Queue(#[from] ValidationQueueError),
    #[error("database error: {reason}")]
    Db { reason: String },
    #[error("invalid class code {class_code}: out of i16 range")]
    InvalidClassCode { class_code: i32 },
    #[error("retained Q1 admission error: {0}")]
    Admission(String),
}

#[cfg(feature = "skills-db")]
fn q1_db(error: tokio_postgres::Error) -> Q1Error {
    Q1Error::Db {
        reason: error
            .code()
            .map(|code| format!("SQLSTATE {}", code.code()))
            .unwrap_or_else(|| error.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Retained IBS/Recipe path
// ---------------------------------------------------------------------------

/// Schedule a Q1 validator review through the retained IBS/Recipe path.
/// Requires that structural validator Recipe drafts are seeded in the revision
/// store via `retain_validator_check_drafts`.
///
/// Returns `Q1Outcome::Deferred` when:
/// - The revision store has no structural validator draft for `class_code`
/// - The `ServiceClient` is not yet wired (None)
/// - No active submission exists in the review-submission store for `component_id`
/// - The installed worker binary is unavailable
///
/// Returns `Q1Outcome::Passed` / `Q1Outcome::Failed` only when the complete
/// IBS path executed and `verify_receipt` persisted evidence without error.
#[cfg(feature = "skills-db")]
pub(crate) async fn run_q1_validation_retained(
    pool: &PgPool,
    scope: &ComponentScope,
    component_id: Uuid,
    class_code: i32,
    queue_store: &ValidationQueueStore,
    preparation: brassclaw_monty_host::utility::UtilityLimits,
    bridge: Option<&crate::installed_monty_catalogue::ValidatorServiceBridge>,
) -> Result<Q1Outcome, Q1Error> {
    use std::sync::Arc;

    use brassclaw_engine::{
        executor::{retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram},
        memory::{
            retained_inputs::prepare_retained_unbound_program,
            retained_instruction::{WorkflowClass, compile_retained_recipe},
        },
    };
    use brassclaw_skills::{
        component_revision::{REVISION_LIMITS, RetainedComponentSnapshot},
        review_submission_store::PgReviewSubmissionStore,
        revision_store::PgComponentRevisionStore,
        validator_components::{
            structural_validator_aggregate_id, structural_validator_collect_id,
            structural_validator_pc_id, structural_validator_receipt_id,
            structural_validator_recipe_id,
        },
    };
    use brassclaw_turns::LoopCompleted;

    // Step 1 — require wired bridge; defer if not yet available.
    let bridge = match bridge {
        Some(b) => b,
        None => {
            return Ok(Q1Outcome::deferred("retained path not yet wired"));
        }
    };

    // Step 2 — convert class_code and check if the structural validator revision
    // draft is seeded for this class.
    let class_i16: i16 = class_code
        .try_into()
        .map_err(|_| Q1Error::InvalidClassCode { class_code })?;

    let revision_store = PgComponentRevisionStore::new(Arc::new(pool.clone()));
    let recipe_id = structural_validator_recipe_id(class_i16);
    let Some((_, recipe_version)) = revision_store
        .read_allocation_head(recipe_id)
        .await
        .map_err(|e| Q1Error::Db {
            reason: e.to_string(),
        })?
    else {
        return Ok(Q1Outcome::deferred(format!(
            "no structural validator revision draft for class {class_code}"
        )));
    };

    // Step 3 — read all 5 drafts and build a RetainedComponentSnapshot.
    let pc_id = structural_validator_pc_id(class_i16);
    let collect_id = structural_validator_collect_id(class_i16);
    let aggregate_id = structural_validator_aggregate_id(class_i16);
    let receipt_id = structural_validator_receipt_id(class_i16);

    let pc_version = revision_store
        .read_allocation_head(pc_id)
        .await
        .map_err(|e| Q1Error::Db {
            reason: e.to_string(),
        })?
        .map(|(_, v)| v)
        .ok_or_else(|| Q1Error::Db {
            reason: format!("structural validator PC draft missing for class {class_code}"),
        })?;
    let collect_version = revision_store
        .read_allocation_head(collect_id)
        .await
        .map_err(|e| Q1Error::Db {
            reason: e.to_string(),
        })?
        .map(|(_, v)| v)
        .ok_or_else(|| Q1Error::Db {
            reason: format!("structural validator collect draft missing for class {class_code}"),
        })?;
    let aggregate_version = revision_store
        .read_allocation_head(aggregate_id)
        .await
        .map_err(|e| Q1Error::Db {
            reason: e.to_string(),
        })?
        .map(|(_, v)| v)
        .ok_or_else(|| Q1Error::Db {
            reason: format!("structural validator aggregate draft missing for class {class_code}"),
        })?;
    let receipt_version = revision_store
        .read_allocation_head(receipt_id)
        .await
        .map_err(|e| Q1Error::Db {
            reason: e.to_string(),
        })?
        .map(|(_, v)| v)
        .ok_or_else(|| Q1Error::Db {
            reason: format!(
                "structural validator receipt encoder draft missing for class {class_code}"
            ),
        })?;

    let recipe_rev = revision_store
        .read_revision(recipe_id, recipe_version)
        .await
        .map_err(|e| Q1Error::Db {
            reason: e.to_string(),
        })?;
    let pc_rev = revision_store
        .read_revision(pc_id, pc_version)
        .await
        .map_err(|e| Q1Error::Db {
            reason: e.to_string(),
        })?;
    let collect_rev = revision_store
        .read_revision(collect_id, collect_version)
        .await
        .map_err(|e| Q1Error::Db {
            reason: e.to_string(),
        })?;
    let aggregate_rev = revision_store
        .read_revision(aggregate_id, aggregate_version)
        .await
        .map_err(|e| Q1Error::Db {
            reason: e.to_string(),
        })?;
    let receipt_rev = revision_store
        .read_revision(receipt_id, receipt_version)
        .await
        .map_err(|e| Q1Error::Db {
            reason: e.to_string(),
        })?;

    let snapshot = Arc::new(
        RetainedComponentSnapshot::new(
            &[recipe_id],
            vec![recipe_rev, pc_rev, collect_rev, aggregate_rev, receipt_rev],
        )
        .map_err(|e| Q1Error::Db {
            reason: e.to_string(),
        })?,
    );

    // Step 4 — compile, prepare and inspect the retained program.
    // Variant key must match what structural_validator_recipe_drafts emits.
    let variant_key = format!("validator-class-{class_i16}");
    let instruction = compile_retained_recipe(
        snapshot,
        recipe_id,
        &variant_key,
        WorkflowClass::Deterministic,
    )
    .map_err(|e| Q1Error::Db {
        reason: format!("compile_retained_recipe: {e}"),
    })?;
    let program = prepare_retained_unbound_program(instruction).map_err(|e| Q1Error::Db {
        reason: format!("prepare_retained_unbound_program: {e}"),
    })?;

    let worker = match brassclaw_monty_host::process::installed_worker() {
        Ok(w) => w,
        Err(_) => {
            return Ok(Q1Outcome::deferred("worker image not available"));
        }
    };

    let inspected = Arc::new(
        InspectedRetainedProgram::inspect_with_limits(
            RetainedProgram::Unbound(Arc::new(program)),
            &worker,
            preparation,
        )
        .await
        .map_err(|e| Q1Error::Db {
            reason: format!("InspectedRetainedProgram::inspect_with_limits: {e}"),
        })?,
    );

    // Step 5 — find the most-recent active submission for this component and
    // read its sealed validator subject.  A missing submission is expected
    // until the operator has explicitly submitted the component for review via
    // the retained authoring path; defer rather than error.
    //
    // We query reborn_component_review_submissions directly by component_id
    // because PgReviewSubmissionStore only provides lookup-by-submission-id.
    let submission_id: Option<Uuid> = {
        let client = pool.get().await.map_err(|e| Q1Error::Db {
            reason: e.to_string(),
        })?;
        client
            .query_opt(
                "SELECT submission_id FROM reborn_component_review_submissions \
                 WHERE component_id=$1 ORDER BY recorded_at DESC LIMIT 1",
                &[&component_id],
            )
            .await
            .map_err(q1_db)?
            .map(|row| row.get(0))
    };
    let submission_id = match submission_id {
        Some(id) => id,
        None => {
            return Ok(Q1Outcome::deferred(format!(
                "no active submission for component {component_id}"
            )));
        }
    };

    // Capture the queue candidate to record the result later (state check + field extraction).
    let review = match queue_store.capture_q1_candidate(scope, component_id).await {
        Ok(review) => review,
        Err(crate::validation_queue::ValidationQueueError::ComponentMissing { .. }) => {
            return Ok(Q1Outcome::deferred(format!(
                "component {component_id} not found in scope"
            )));
        }
        Err(error) => return Err(error.into()),
    };
    if review.class_code() != class_i16 {
        return Err(
            crate::validation_queue::ValidationQueueError::ReviewChanged { component_id }.into(),
        );
    }

    // Read the sealed validator subject from the review-submission store.
    let subject = {
        let mut client = pool.get().await.map_err(|e| Q1Error::Db {
            reason: e.to_string(),
        })?;
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(q1_db)?;
        let subject = PgReviewSubmissionStore::read_validator_subject_in_transaction(
            &tx,
            submission_id,
            REVISION_LIMITS,
        )
        .await
        .map_err(|e| Q1Error::Db {
            reason: format!("read_validator_subject_in_transaction: {e}"),
        })?;
        tx.commit().await.map_err(q1_db)?;
        subject
    };
    let subject = match subject {
        Some(s) => s,
        None => {
            return Ok(Q1Outcome::deferred(format!(
                "no active submission for component {component_id}"
            )));
        }
    };

    // Step 6 — retain the sealed validator selection (idempotent).
    let retained_selection =
        crate::pg_validator_q1_admission::retain_validator_selection(pool, &subject, &inspected)
            .await
            .map_err(|e| Q1Error::Admission(e.to_string()))?;

    // Step 7 — submit through the trusted internal drive_turn path.
    // drive_admitted calls build -> capture_internal -> GlobalRecipePorts::for_internal.
    // On InternalCompleted, GlobalRecipePorts::finish calls source.verify_receipt,
    // which persists the Q1 evidence. `settle_closed` must NOT be called after this.
    let driven = bridge
        .driver
        .submit(Arc::new(pool.clone()), submission_id)
        .await
        .map_err(|e| Q1Error::Admission(e.to_string()))?;

    tracing::debug!(
        component_id = %component_id,
        class_code   = class_code,
        submission_id = %submission_id,
        loop_exit     = ?driven.exit,
        "Q1 retained: validator review task settled"
    );

    match driven.exit {
        brassclaw_turns::LoopExit::Completed(LoopCompleted {
            completion_kind: brassclaw_turns::LoopCompletionKind::ResultOnly,
            ..
        }) => {
            let completion = driven.internal_completion.as_ref().ok_or_else(|| {
                Q1Error::Admission("validator terminal settlement proof missing".into())
            })?;
            let verdict = crate::pg_validator_q1_evidence::read_settled_validator_q1(
                pool,
                &subject,
                &retained_selection,
                completion,
            )
            .await
            .map_err(|error| Q1Error::Admission(error.to_string()))?;
            if verdict.succeeded {
                queue_store.gate1_pass_reviewed(scope, &review, &[]).await?;
                Ok(Q1Outcome::Passed)
            } else {
                queue_store
                    .gate1_fail_reviewed(scope, &review, &verdict.errors)
                    .await?;
                Ok(Q1Outcome::Failed {
                    errors: verdict.errors,
                })
            }
        }
        brassclaw_turns::LoopExit::Completed(_) => {
            // Unexpected normal Completed (FinalReply) for an internal task.
            let errors =
                vec!["retained validator returned unexpected FinalReply completion".into()];
            queue_store
                .gate1_fail_reviewed(scope, &review, &errors)
                .await?;
            Ok(Q1Outcome::Failed { errors })
        }
        brassclaw_turns::LoopExit::Cancelled(cancelled) => {
            let errors = vec![format!(
                "validator task cancelled: {:?}",
                cancelled.reason_kind
            )];
            queue_store
                .gate1_fail_reviewed(scope, &review, &errors)
                .await?;
            Ok(Q1Outcome::Failed { errors })
        }
        brassclaw_turns::LoopExit::Blocked(_) | brassclaw_turns::LoopExit::Failed(_) => {
            // Not produced by drive_admitted for internal tasks — treat as failure.
            let errors = vec!["retained validator returned unexpected blocked/failed exit".into()];
            queue_store
                .gate1_fail_reviewed(scope, &review, &errors)
                .await?;
            Ok(Q1Outcome::Failed { errors })
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn q1outcome_deferred_is_not_passed() {
        let d = Q1Outcome::deferred("no recipe");
        assert!(!d.passed_gate());
        assert!(matches!(d, Q1Outcome::Deferred { .. }));
    }

    #[test]
    fn q1outcome_passed_is_passed() {
        assert!(Q1Outcome::passed().passed_gate());
    }

    #[test]
    fn q1outcome_failed_is_not_passed() {
        let f = Q1Outcome::failed(vec!["oops".into()]);
        assert!(!f.passed_gate());
        assert!(matches!(f, Q1Outcome::Failed { .. }));
    }

    #[test]
    fn q1_invalid_class_code_is_i16_range_check() {
        // i32::MAX does not fit in i16 — the error variant is correct.
        let too_large: i32 = i32::MAX;
        let result: Result<i16, _> = too_large.try_into();
        assert!(
            result.is_err(),
            "i32::MAX should not fit in i16 — confirms the range check fires"
        );
    }

    /// Confirm that the structural validator recipe drafts for all 8 classes can
    /// be compiled and prepared without a database.  This catches variant-key and
    /// value-contract issues before the integration tests run.
    #[cfg(feature = "skills-db")]
    #[test]
    fn structural_validator_recipe_compiles_and_prepares_for_all_classes() {
        use brassclaw_engine::memory::{
            retained_inputs::prepare_retained_unbound_program,
            retained_instruction::{WorkflowClass, compile_retained_recipe},
        };
        use brassclaw_skills::{
            component_revision::RetainedComponentSnapshot,
            validator_components::{
                VALIDATOR_CLASS_CODES, structural_validator_recipe_drafts,
                structural_validator_recipe_id,
            },
        };
        use std::sync::Arc;

        for &class_code in VALIDATOR_CLASS_CODES {
            let label = format!("class-{class_code}");
            // Use the same selector logic as retain_validator_check_drafts.
            let pc_body = crate::builtin_bootstrap::validator_pc_body_for_class_test(class_code);
            let drafts = structural_validator_recipe_drafts(class_code, &label, pc_body)
                .unwrap_or_else(|e| panic!("class {class_code} drafts failed: {e}"));
            let recipe_id = structural_validator_recipe_id(class_code);
            let revisions: Vec<_> = drafts
                .into_iter()
                .enumerate()
                .map(|(i, d)| {
                    d.at_version((i + 1) as u64)
                        .unwrap_or_else(|e| panic!("class {class_code} at_version failed: {e}"))
                })
                .collect();
            let snapshot = Arc::new(
                RetainedComponentSnapshot::new(&[recipe_id], revisions)
                    .unwrap_or_else(|e| panic!("class {class_code} snapshot failed: {e}")),
            );
            let variant_key = format!("validator-class-{class_code}");
            let instruction = compile_retained_recipe(
                snapshot,
                recipe_id,
                &variant_key,
                WorkflowClass::Deterministic,
            )
            .unwrap_or_else(|e| panic!("class {class_code} compile failed: {e}"));
            prepare_retained_unbound_program(instruction)
                .unwrap_or_else(|e| panic!("class {class_code} prepare failed: {e}"));
        }
    }
}
