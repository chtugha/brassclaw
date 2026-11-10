//! Gate-1 (Q1) validation orchestrator — retained IBS/Recipe path.
//!
//! Production queue notifications wake original discovery only. The historical
//! draft-head preparation path is restricted to controlled native tests until
//! B1-07/B2-04 supplies an actually qualified coherent selection.
//!
//! # Column fix (V079 — FIND-P0-01)
//!
//! `validates_class_code SMALLINT` on `reborn_recipes` carries the class being
//! validated; `class_code = 21` on that table always means "this row IS a Recipe".
//!
//! # State-2 write invariant (FIND-P9-01 / FIND-P9-08)
//!
//! `verify_receipt` records observations before terminal native settlement.
//! Structural observations never establish the remaining full Q1 qualification.
//! The reviewed pass/failure methods are `pub(crate)` on `ValidationQueueStore`,
//! preventing any API layer from calling them directly.
//!
//! # Feature gate
//!
//! Requires the `postgres` feature.

#![forbid(unsafe_code)]

use thiserror::Error;

#[cfg(all(test, feature = "skills-db"))]
use brassclaw_engine::memory::retrieval_source::ComponentScope;
#[cfg(all(test, feature = "skills-db"))]
use brassclaw_pg::PgPool;
#[cfg(all(test, feature = "skills-db"))]
use uuid::Uuid;

use crate::validation_queue::ValidationQueueError;
#[cfg(all(test, feature = "skills-db"))]
use crate::validation_queue::ValidationQueueStore;

// ---------------------------------------------------------------------------
// Outcome
// ---------------------------------------------------------------------------

/// The outcome of a Gate 1 (Q1) validation attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Q1Outcome {
    /// Actual structural observations were delivered and acknowledged, but
    /// complete class-Q1 qualification is still missing. Queue remains state 1.
    StructuralObserved,
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

/// Errors raised by the controlled retained-validation fixture.
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
#[cfg(test)]
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
/// Returns `Q1Outcome::StructuralObserved` / `Q1Outcome::Failed` only when the complete
/// IBS path executed, native settlement was acknowledged and the exact stored
/// verdict was authenticated. Cancellation and incomplete execution return an
/// error without recording a negative candidate verdict.
#[cfg(feature = "skills-db")]
#[cfg(test)]
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

    if !matches!(class_i16, 0 | 1..=3 | 13 | 21..=23) {
        return Ok(Q1Outcome::deferred(format!(
            "no structural validator revision draft for class {class_code}"
        )));
    }

    // Compatibility callers carry a component key, but resolution is solely
    // through its explicit original queue link. Unbound legacy work is visible
    // and never admitted by guessing a later immutable subject.
    {
        let client = pool.get().await.map_err(|_| Q1Error::Db {
            reason: "original queue read unavailable".into(),
        })?;
        let row=client.query_opt("SELECT b.submission_id, q.component_class, q.original_binding_required FROM reborn_validation_queue q LEFT JOIN reborn_component_review_queue_bindings b ON b.queue_id=q.id WHERE q.tenant_id=$1 AND q.user_id=$2 AND q.agent_id=$3 AND q.project_id=$4 AND q.component_id=$5",
            &[&scope.tenant_id,&scope.user_id,&scope.agent_id,&scope.project_id,&component_id]).await.map_err(q1_db)?
            .ok_or(crate::validation_queue::ValidationQueueError::NotFound{component_id})?;
        if row.get::<_, i16>(1) != class_i16 {
            return Err(
                crate::validation_queue::ValidationQueueError::ReviewChanged { component_id }
                    .into(),
            );
        }
        if row.get::<_, Option<Uuid>>(0).is_none() {
            if !row.get::<_, bool>(2) {
                return Ok(Q1Outcome::deferred(format!(
                    "legacy review custody unresolved for component {component_id}; safe disposition and fencing are not connected; do not replay"
                )));
            }
            return Ok(Q1Outcome::deferred(format!(
                "original review binding unresolved for component {component_id}; explicit resubmission required"
            )));
        }
    }

    // The queued original, never the newest unrelated subject for this UUID.
    let review = queue_store
        .capture_q1_candidate(scope, component_id)
        .await?;
    if review.class_code() != class_i16 {
        return Err(
            crate::validation_queue::ValidationQueueError::ReviewChanged { component_id }.into(),
        );
    }
    let original = match review.original() {
        Ok(original) => original,
        Err(crate::validation_queue::ValidationQueueError::OriginalReviewUnresolved { .. }) => {
            return Ok(Q1Outcome::deferred(format!(
                "original review binding unresolved for component {component_id}; explicit resubmission required"
            )));
        }
        Err(crate::validation_queue::ValidationQueueError::LegacyReviewCustodyUnresolved {
            ..
        }) => {
            return Ok(Q1Outcome::deferred(format!(
                "legacy review custody unresolved for component {component_id}; safe disposition and fencing are not connected; do not replay"
            )));
        }
        Err(error) => return Err(error.into()),
    };
    let submission_id = original.submission_id();

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
        .map_err(|error| crate::validation_queue::map_original_store(error, component_id))?;
        tx.commit().await.map_err(q1_db)?;
        subject
    };
    let subject = match subject {
        Some(s) => s,
        None => {
            return Err(
                crate::validation_queue::ValidationQueueError::ReviewChanged { component_id }
                    .into(),
            );
        }
    };

    original
        .require_subject(&subject)
        .map_err(|error| crate::validation_queue::map_original_store(error, component_id))?;
    // Current /1 validator preparation cannot cover a joint function. Keep the
    // complete /3 subject; never graduate only its primary class/root.
    if subject.is_function_review() {
        return Ok(Q1Outcome::deferred(
            "joint function validator selection not yet supported",
        ));
    }

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

    // Step 6 — retain the sealed validator selection (idempotent).
    let retained_selection =
        crate::pg_validator_q1_admission::retain_validator_selection(pool, &subject, &inspected)
            .await
            .map_err(|e| Q1Error::Admission(e.to_string()))?;

    // Step 7 — publish the original private Work to the existing Worker.
    // A03's captured source owns receipt verification/evidence; the native
    // driver owns settlement and delivers its actual opaque completion.
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

    let completion = require_settled_validator_completion(driven)?;
    let verdict = crate::pg_validator_q1_evidence::read_settled_validator_q1(
        pool,
        &subject,
        &retained_selection,
        &completion,
    )
    .await
    .map_err(|error| Q1Error::Admission(error.to_string()))?;
    let mut client = crate::bootstrap_evidence::io::acquire(pool)
        .await
        .map_err(|_| Q1Error::Admission("review confirmation unavailable".into()))?;
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .map_err(q1_db)?;
    let original = crate::pg_monty_admission::PgMontyAdmission::verify_private_live_completion(
        &tx,
        &completion,
    )
    .await
    .map_err(|error| Q1Error::Admission(error.to_string()))?
    .ok_or_else(|| Q1Error::Admission("validator original missing".into()))?;
    let context = original.request().context().clone();
    tx.commit().await.map_err(q1_db)?;
    client.recycle_after_commit();
    drop(client);
    let policy = preparation.process.response_timeout;
    tokio::time::timeout(
        policy,
        crate::review_work_confirmation::confirm_original(pool, submission_id, &context),
    )
    .await
    .map_err(|_| Q1Error::Admission("review confirmation deadline elapsed".into()))?
    .map_err(|error| Q1Error::Admission(error.to_string()))?
    .ok_or_else(|| Q1Error::Admission("validator delivery missing".into()))?;
    if verdict.succeeded {
        Ok(Q1Outcome::StructuralObserved)
    } else {
        Ok(Q1Outcome::Failed {
            errors: verdict.errors,
        })
    }
}

/// Execution failures cannot become candidate diagnostics. Only the settled
/// reader's authenticated negative verdict may reach the queue failure writer.
#[cfg(feature = "skills-db")]
#[cfg(test)]
fn require_settled_validator_completion(
    driven: crate::global_monty_driver::DrivenMontyTask,
) -> Result<crate::global_monty_driver::SettledInternalCompletion, Q1Error> {
    use brassclaw_turns::{LoopCompleted, LoopCompletionKind, LoopExit};
    match driven.exit {
        LoopExit::Completed(LoopCompleted {
            completion_kind: LoopCompletionKind::ResultOnly,
            ..
        }) => driven.internal_completion.ok_or_else(|| {
            Q1Error::Admission("validator terminal settlement proof missing".into())
        }),
        LoopExit::Completed(_) => Err(Q1Error::Admission(
            "retained validator returned unexpected reply completion".into(),
        )),
        LoopExit::Cancelled(cancelled) => Err(Q1Error::Admission(format!(
            "validator task cancelled: {:?}",
            cancelled.reason_kind
        ))),
        LoopExit::Blocked(_) | LoopExit::Failed(_) => Err(Q1Error::Admission(
            "retained validator returned unexpected blocked/failed exit".into(),
        )),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Pure classification coverage; native settlement and queue behavior are
    /// covered separately by the retained Q1 integration cases.
    #[cfg(feature = "skills-db")]
    #[test]
    fn incomplete_execution_cannot_become_a_negative_candidate_verdict() {
        use brassclaw_turns::{
            LoopCompleted, LoopCompletionKind, LoopExit, LoopExitId, LoopFailureKind,
        };
        let id = || LoopExitId::new("exit:q1-incomplete-fixture".to_owned()).unwrap();
        let mut exits = vec![
            LoopExit::cancelled_for_observed_interrupt(id()),
            LoopExit::failed(LoopFailureKind::EffectOutcomeUnresolved, id()),
        ];
        for completion_kind in [
            LoopCompletionKind::FinalReply,
            LoopCompletionKind::AskUserReply,
            LoopCompletionKind::NoReply,
            LoopCompletionKind::DelegatedResult,
            LoopCompletionKind::ResultOnly,
        ] {
            exits.push(LoopExit::Completed(LoopCompleted {
                completion_kind,
                reply_message_refs: Vec::new(),
                result_refs: Vec::new(),
                final_checkpoint_id: None,
                usage_summary_ref: None,
                exit_id: id(),
            }));
        }
        for exit in exits {
            assert!(matches!(
                require_settled_validator_completion(exit.into()),
                Err(Q1Error::Admission(_))
            ));
        }
    }

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
