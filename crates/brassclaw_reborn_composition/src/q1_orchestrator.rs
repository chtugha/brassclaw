//! Gate-1 (Q1) validation orchestrator — Phase N rewrite (§0.23.2 / §0.23.9).
//!
//! Phase N replaces the pure-Rust `ComponentValidator` with an **orchestrated**
//! Q1 path: for each component under review, this module looks up the class-specific
//! validation Recipe (a `reborn_recipes` row tagged with `'05:validator'` in
//! `consumer_tags` AND `validates_class_code = <class>` from V079), then runs the
//! Tier-0 execution channel (`execute_tier_zero_channel`) with the Recipe's
//! orchestrator content in a restricted sandbox.
//!
//! # Graceful defer
//!
//! Before validation Recipes are seeded (`validates_class_code` rows, Phase L §0.23.3),
//! `load_validator_program` returns `None` and `run_q1_validation` returns
//! `Q1Outcome::Deferred`. The component stays at queue state 1 (`Q1_pending`) until
//! a Recipe is available.
//!
//! # Column fix (V079 — FIND-P0-01)
//!
//! The original query filtered `reborn_recipes.class_code = $class` but
//! `class_code` on `reborn_recipes` is always 21 (enforced by CHECK constraint) —
//! it means "this row IS a Recipe", not "this Recipe validates class X".
//! V079 adds `validates_class_code SMALLINT` to carry that meaning.
//! `load_validator_program` filters on `validates_class_code = $class`.
//!
//! # State-2 write invariant (FIND-P9-01 / FIND-P9-08)
//!
//! Only this module (`run_q1_validation`) may write state 2 by calling
//! `gate1_pass_reviewed`. The reviewed pass/failure methods are `pub(crate)` on
//! [`ValidationQueueStore`], preventing any API layer from calling them directly.
//!
//! # Feature gate
//!
//! Requires the `postgres` feature.

#![forbid(unsafe_code)]

use brassclaw_engine::executor::scripting::run_python_code_body_with_limits;
use brassclaw_engine::memory::retrieval_source::ComponentScope;
use brassclaw_monty_host::utility::UtilityLimits;
use brassclaw_pg::PgPool;
use thiserror::Error;
use uuid::Uuid;

use crate::validation_queue::{ValidationQueueError, ValidationQueueStore};

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

/// Errors raised by [`run_q1_validation`].
#[derive(Debug, Error)]
pub enum Q1Error {
    #[error("validation queue error: {0}")]
    Queue(#[from] ValidationQueueError),
    #[error("database error: {reason}")]
    Db { reason: String },
    #[error("invalid class code {class_code}: out of i16 range")]
    InvalidClassCode { class_code: i32 },
    #[error("multiple approved validators exist for class {class_code}")]
    AmbiguousValidator { class_code: i16 },
    #[error("validator Recipe {recipe_id} is not a supported approved PythonCode program")]
    InvalidValidator { recipe_id: Uuid },
    #[error("retained Q1 admission error: {0}")]
    Admission(String),
}

// ---------------------------------------------------------------------------
// Recipe lookup
// ---------------------------------------------------------------------------

/// Read the approved validator and its single pure-code entry point from one
/// consistent catalogue view. No unapproved code or silently ignored entries.
async fn load_validator_program(
    pool: &PgPool,
    scope: &ComponentScope,
    class_code: i16,
) -> Result<Option<(Uuid, String)>, Q1Error> {
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
    let rows = tx
        .query(
            "SELECT id, step_descriptions FROM reborn_recipes WHERE tenant_id=$1 AND user_id=$2
         AND agent_id=$3 AND project_id=$4 AND validates_class_code=$5
         AND validation_status='validated' AND '05:validator'=ANY(consumer_tags)
         ORDER BY id LIMIT 2",
            &[
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &class_code,
            ],
        )
        .await
        .map_err(q1_db)?;
    if rows.len() > 1 {
        return Err(Q1Error::AmbiguousValidator { class_code });
    }
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    let recipe_id: Uuid = row.get(0);
    let invalid = || Q1Error::InvalidValidator { recipe_id };
    let descriptions: Option<serde_json::Value> = row.get(1);
    let descriptions = descriptions
        .as_ref()
        .and_then(serde_json::Value::as_array)
        .filter(|values| values.len() == 1)
        .ok_or_else(invalid)?;
    let steps = descriptions[0]
        .get("steps")
        .and_then(serde_json::Value::as_array)
        .filter(|values| values.len() == 1)
        .ok_or_else(invalid)?;
    let step = &steps[0];
    if step.get("type").and_then(serde_json::Value::as_str) != Some("component")
        || step.get("knowledge").and_then(serde_json::Value::as_str) != Some("orchestrator")
        || !step
            .get("tool_bindings")
            .and_then(serde_json::Value::as_array)
            .is_some_and(Vec::is_empty)
        || step
            .get("dependencies")
            .is_none_or(|value| !value.is_null())
    {
        return Err(invalid());
    }
    let includes = step
        .get("include")
        .and_then(serde_json::Value::as_array)
        .filter(|values| values.len() == 1)
        .ok_or_else(invalid)?;
    let pc_id = includes[0]
        .as_str()
        .and_then(|id| Uuid::parse_str(id).ok())
        .filter(|id| !id.is_nil())
        .ok_or_else(invalid)?;
    let pc = tx
        .query_opt(
            "SELECT content FROM reborn_python_code WHERE id=$1 AND tenant_id=$2 AND user_id=$3
         AND agent_id=$4 AND project_id=$5 AND class_code=22 AND validation_status='validated'",
            &[
                &pc_id,
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
            ],
        )
        .await
        .map_err(q1_db)?
        .ok_or_else(invalid)?;
    let body: String = pc.get(0);
    tx.commit().await.map_err(q1_db)?;
    Ok(Some((recipe_id, body)))
}

fn q1_db(error: tokio_postgres::Error) -> Q1Error {
    Q1Error::Db {
        reason: error
            .code()
            .map(|code| format!("SQLSTATE {}", code.code()))
            .unwrap_or_else(|| error.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Helpers: fetch component content fields for the structural check
// ---------------------------------------------------------------------------

/// The three content fields fetched from a component row for structural
/// validation: name, description, and the main content/body/steps text.
struct ComponentContentFields {
    name: String,
    description: String,
    /// The "content" field — column name varies by class:
    /// - class 0 (Tool): `capability_id` (the registered tool name)
    /// - class 1/2/3 (Skill): `body`
    /// - class 13 (ToolSkill): `content`
    /// - class 21 (Recipe): `description` (steps are JSONB; description is the proxy)
    /// - class 22 (PythonCode): `content`
    /// - class 23 (Catalogue): `overview_doc`
    content: String,
}

/// Extract the actual reviewed candidate, including an upgrade's proposed
/// values. The queue snapshot is later compared before storing Q1 success.
fn reviewed_content_fields(
    review: &crate::validation_queue::Q1Candidate,
) -> Option<ComponentContentFields> {
    let value = review.reviewed();
    let content = match review.class_code() {
        0 => value
            .get("capability_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        1..=3 => value.get("body")?.as_str()?,
        13 | 22 => value
            .get("content")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        21 => value.get("description")?.as_str()?,
        23 => value
            .get("overview_doc")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(""),
        _ => return None,
    };
    Some(ComponentContentFields {
        name: value.get("name")?.as_str()?.to_owned(),
        description: value
            .get("description")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_owned(),
        content: content.to_owned(),
    })
}

/// Run the validator PythonCode body in the lightweight Monty sandbox.
///
/// Converts the legacy quoted slot markers to fixed typed-input expressions.
/// Component values are injected separately, never rendered as Python source.
/// This compatibility preparation does not change or activate the stored body.
/// Then calls [`run_python_code_body_with_limits`] — a pure-logic, no-host-call executor.
///
/// Returns `(passed: bool, errors: Vec<String>)` from the body's
/// `{"pass": bool, "errors": [...]}` return value.  On any execution error
/// the result is treated as a failure with the error message as the sole error.
async fn run_validator_python_with_limits(
    pc_body: &str,
    name: &str,
    description: &str,
    content: &str,
    limits: UtilityLimits,
) -> (bool, Vec<String>) {
    if pc_body.len() > limits.bounds.max_source_bytes {
        return (
            false,
            vec!["validator source exceeds the technical utility bound".into()],
        );
    }
    // Only fixed expressions enter source. Rust Debug string formatting is
    // not Python serialization (for example its control-character escapes).
    let body = pc_body
        .replace("\"{{vars.slot0}}\"", "inputs[\"name\"]")
        .replace("\"{{vars.slot1}}\"", "inputs[\"description\"]")
        .replace("\"{{vars.slot2}}\"", "inputs[\"content\"]");
    if body.contains("{{vars.") {
        return (
            false,
            vec!["validator has an unsupported input marker".into()],
        );
    }

    // Preload-format bodies define a function `check_structural_presence(inputs)`
    // and must be invoked explicitly. Legacy inline bodies assign `result`
    // directly. Detect the preload format by looking for the canonical export name.
    let body_with_return = if body.contains("def check_structural_presence(") {
        // Preload function format: call the function as the final expression so
        // the Evaluate utility returns its value directly (assignment returns None).
        format!("{body}\ncheck_structural_presence(inputs)")
    } else {
        // Legacy inline format: append `result` as the final expression.
        format!("{body}\nresult")
    };

    let inputs = serde_json::json!({"name": name, "description": description, "content": content});
    match run_python_code_body_with_limits(&body_with_return, &[("inputs", inputs)], limits).await {
        Ok(Some(val)) => {
            let invalid = || {
                (
                    false,
                    vec!["validator returned an invalid {pass, errors} result".into()],
                )
            };
            let Some(object) = val.as_object().filter(|object| object.len() == 2) else {
                return invalid();
            };
            let Some(passed) = object.get("pass").and_then(|v| v.as_bool()) else {
                return invalid();
            };
            let Some(errors) = object.get("errors").and_then(|v| v.as_array()) else {
                return invalid();
            };
            let Some(errors) = errors
                .iter()
                .map(|v| v.as_str().map(str::to_owned))
                .collect::<Option<Vec<_>>>()
            else {
                return invalid();
            };
            if (passed && !errors.is_empty()) || (!passed && errors.is_empty()) {
                return invalid();
            }
            (passed, errors)
        }
        Ok(None) => (
            false,
            vec!["validator returned None — expected {pass, errors} dict".into()],
        ),
        Err(e) => (false, vec![format!("validator execution error: {e}")]),
    }
}

// ---------------------------------------------------------------------------
// Legacy mutable-recipe entry point (Step 6 migration target)
// ---------------------------------------------------------------------------

/// **Legacy path** — queries `reborn_recipes` directly and extracts the
/// PythonCode body from `step_descriptions`. Use `run_q1_validation_retained`
/// for all new callers; this function remains for compatibility while the
/// retained path accumulates behavioral evidence (validator Step 6).
///
/// Run Gate 1 (Q1) validation for one component and record the result on the
/// validation queue (FIND-P9-01 / §0.23.9).
///
/// # Flow
///
/// 1. Convert `class_code` to `i16`; return `Q1Error::InvalidClassCode` if out
///    of range.
/// 2. Query `reborn_recipes` for a validated Recipe tagged `'05:validator'` for
///    `class_code` in the component's scope.
/// 3. **No Recipe found** → return `Q1Outcome::Deferred` without touching the
///    queue (component stays at state 1).
/// 4. **Recipe found** → capture the exact queued candidate, overlay any
///    validated proposed upgrade fields, and select the approved single-entry
///    validator PythonCode in the same catalogue read transaction; execute via
///    [`run_python_code_body_with_limits`] (no engine deps required for Tier-0 validators).
///    On pass → `queue_store.gate1_pass_reviewed(…)` (state 1→2).
///    On fail → `queue_store.gate1_fail_reviewed(…)` (state 1, errors recorded).
///    Both compare the captured bytes again before persisting the result.
/// 5. Return the [`Q1Outcome`].
///
/// # Graceful degrade paths
///
/// - No Recipe found → `Deferred` (component stays at state 1).
/// - Ambiguous Recipe selection or malformed/unapproved PythonCode → error;
///   the component stays at state 1 without an invented Q1 pass.
/// - Component row not found (wrong scope/id) → `Deferred`.
pub async fn run_q1_validation(
    pool: &PgPool,
    scope: &ComponentScope,
    component_id: Uuid,
    class_code: i32,
    queue_store: &ValidationQueueStore,
    preparation: UtilityLimits,
) -> Result<Q1Outcome, Q1Error> {
    let class_i16: i16 = class_code
        .try_into()
        .map_err(|_| Q1Error::InvalidClassCode { class_code })?;

    // Read the validator Recipe and its approved code together. A missing
    // validator defers; ambiguous/malformed/unapproved combinations fail closed.
    let Some((recipe_id, pc_body)) = load_validator_program(pool, scope, class_i16).await? else {
        return Ok(Q1Outcome::deferred(format!(
            "no validated validation Recipe found for class {class_code}"
        )));
    };

    // Step 4 — fetch the component's name / description / content fields.
    let review = match queue_store.capture_q1_candidate(scope, component_id).await {
        Ok(review) => review,
        Err(ValidationQueueError::ComponentMissing { .. }) => {
            return Ok(Q1Outcome::deferred(format!(
                "component {component_id} not found in scope"
            )));
        }
        Err(error) => return Err(error.into()),
    };
    if review.class_code() != class_i16 {
        return Err(ValidationQueueError::ReviewChanged { component_id }.into());
    }
    let Some(fields) = reviewed_content_fields(&review) else {
        tracing::debug!(
            component_id = %component_id,
            class_code   = class_code,
            "Q1: component row not found — deferring"
        );
        return Ok(Q1Outcome::deferred(format!(
            "component {component_id} (class {class_code}) not found in scope"
        )));
    };

    // Step 5 — run the pure-logic structural validator in the Monty sandbox.
    let (passed, errors) = run_validator_python_with_limits(
        &pc_body,
        &fields.name,
        &fields.description,
        &fields.content,
        preparation,
    )
    .await;

    tracing::debug!(
        component_id = %component_id,
        class_code   = class_code,
        recipe_id    = %recipe_id,
        passed,
        errors = ?errors,
        "Q1: structural validator ran"
    );

    // Step 6 — record the gate result on the queue.
    if passed {
        queue_store.gate1_pass_reviewed(scope, &review, &[]).await?;
        Ok(Q1Outcome::Passed)
    } else {
        queue_store
            .gate1_fail_reviewed(scope, &review, &errors)
            .await?;
        Ok(Q1Outcome::Failed { errors })
    }
}

// ---------------------------------------------------------------------------
// Retained IBS/Recipe path
// ---------------------------------------------------------------------------

/// Schedule a Q1 validator review through the retained IBS/Recipe path.
/// This replaces `run_validator_python_with_limits` for components whose
/// structural validator Recipe drafts are already seeded in the revision store.
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
pub async fn run_q1_validation_retained(
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
            structural_validator_pc_id, structural_validator_recipe_id,
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

    // Step 3 — read all 4 drafts and build a RetainedComponentSnapshot.
    let pc_id = structural_validator_pc_id(class_i16);
    let collect_id = structural_validator_collect_id(class_i16);
    let aggregate_id = structural_validator_aggregate_id(class_i16);

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

    let snapshot = Arc::new(
        RetainedComponentSnapshot::new(
            &[recipe_id],
            vec![recipe_rev, pc_rev, collect_rev, aggregate_rev],
        )
        .map_err(|e| Q1Error::Db {
            reason: e.to_string(),
        })?,
    );

    // Step 4 — compile, prepare and inspect the retained program.
    let instruction = compile_retained_recipe(
        snapshot,
        recipe_id,
        "selected",
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

    // Capture the queue candidate to record the result later, mirroring the
    // existing run_q1_validation flow (state check + field extraction).
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
    crate::pg_validator_q1_admission::retain_validator_selection(pool, &subject, &inspected)
        .await
        .map_err(|e| Q1Error::Admission(e.to_string()))?;

    // Step 7 — submit through the trusted internal drive_turn path.
    // drive_admitted calls build -> capture_internal -> GlobalRecipePorts::for_internal.
    // On InternalCompleted, GlobalRecipePorts::finish calls source.verify_receipt,
    // which persists the Q1 evidence. `settle_closed` must NOT be called after this.
    let loop_exit = bridge
        .driver
        .submit(Arc::new(pool.clone()), submission_id)
        .await
        .map_err(|e| Q1Error::Admission(e.to_string()))?;

    tracing::debug!(
        component_id = %component_id,
        class_code   = class_code,
        submission_id = %submission_id,
        loop_exit     = ?loop_exit,
        "Q1 retained: validator review task settled"
    );

    match loop_exit {
        brassclaw_turns::LoopExit::Completed(LoopCompleted {
            completion_kind: brassclaw_turns::LoopCompletionKind::ResultOnly,
            ..
        }) => {
            // InternalCompleted path: verify_receipt ran and persisted evidence.
            queue_store.gate1_pass_reviewed(scope, &review, &[]).await?;
            Ok(Q1Outcome::Passed)
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

    fn utility_limits() -> UtilityLimits {
        UtilityLimits::from_execution(Default::default()).unwrap()
    }
    async fn run_validator_python(
        body: &str,
        name: &str,
        description: &str,
        content: &str,
    ) -> (bool, Vec<String>) {
        run_validator_python_with_limits(body, name, description, content, utility_limits()).await
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

    // ---------------------------------------------------------------------------
    // run_validator_python unit tests (Phase P.0 Step 5)
    // ---------------------------------------------------------------------------

    /// The exact checked-in seed body from `PC_VALIDATOR_STRUCTURAL_BODY`
    /// in `builtin_bootstrap.rs` — kept inline so the test is self-contained.
    /// Uses `inputs["name/description/content"]` directly (no slot markers).
    /// Preload function format: defines `check_structural_presence(inputs)`.
    const TEST_VALIDATOR_BODY: &str = r#"def check_structural_presence(inputs):
    # Tier-0 structural validator (class 22, pure-logic).
    # Checks: name present, description non-empty, content non-empty.
    # Inputs supplied as typed data via inputs["name"], inputs["description"],
    # inputs["content"]; no IBS marker substitution is required.
    # No I/O, no imports, no host calls.
    _name = inputs["name"]
    _desc = inputs["description"]
    _content = inputs["content"]
    _errors = []
    if not _name or not _name.strip():
        _errors.append("name is empty")
    if not _desc or not _desc.strip():
        _errors.append("description is empty")
    if not _content or not _content.strip():
        _errors.append("content/steps is empty")
    return {"pass": len(_errors) == 0, "errors": _errors}
"#;

    #[tokio::test]
    async fn run_validator_python_passes_when_all_fields_present() {
        let (passed, errors) = run_validator_python(
            TEST_VALIDATOR_BODY,
            "my-tool",
            "does something useful",
            "real body",
        )
        .await;
        assert!(passed, "all fields non-empty — should pass");
        assert!(errors.is_empty(), "no errors expected, got: {errors:?}");
    }

    #[tokio::test]
    async fn run_validator_python_fails_when_name_empty() {
        let (passed, errors) =
            run_validator_python(TEST_VALIDATOR_BODY, "", "desc", "content").await;
        assert!(!passed);
        assert!(
            errors.iter().any(|e| e.contains("name")),
            "expected 'name is empty' error, got: {errors:?}"
        );
    }

    #[tokio::test]
    async fn run_validator_python_fails_when_description_empty() {
        let (passed, errors) =
            run_validator_python(TEST_VALIDATOR_BODY, "name", "", "content").await;
        assert!(!passed);
        assert!(
            errors.iter().any(|e| e.contains("description")),
            "expected 'description is empty' error, got: {errors:?}"
        );
    }

    #[tokio::test]
    async fn run_validator_python_fails_when_content_empty() {
        let (passed, errors) = run_validator_python(TEST_VALIDATOR_BODY, "name", "desc", "").await;
        assert!(!passed);
        assert!(
            errors.iter().any(|e| e.contains("content")),
            "expected 'content/steps is empty' error, got: {errors:?}"
        );
    }

    #[tokio::test]
    async fn run_validator_python_fails_when_all_empty() {
        let (passed, errors) = run_validator_python(TEST_VALIDATOR_BODY, "", "", "").await;
        assert!(!passed);
        assert_eq!(
            errors.len(),
            3,
            "expected 3 errors (name + desc + content), got: {errors:?}"
        );
    }

    #[tokio::test]
    async fn run_validator_python_treats_whitespace_only_as_empty() {
        let (passed, errors) =
            run_validator_python(TEST_VALIDATOR_BODY, "  ", "desc", "content").await;
        assert!(
            !passed,
            "whitespace-only name should fail the .strip() check"
        );
        assert!(
            errors.iter().any(|e| e.contains("name")),
            "expected 'name is empty' error, got: {errors:?}"
        );
    }

    #[tokio::test]
    async fn validator_preserves_control_characters_and_source_like_values_as_data() {
        let body = r#"name = "{{vars.slot0}}"
description = "{{vars.slot1}}"
content = "{{vars.slot2}}"
result = {"pass": name == inputs["name"] and description == inputs["description"] and content == inputs["content"], "errors": []}"#;
        let value = "\0\u{1}\u{b}'\"\nresult = host.forbidden_effect()\nüä {{vars.unknown}}";
        let (passed, errors) = run_validator_python(body, value, value, value).await;
        assert!(passed, "typed input round-trip failed: {errors:?}");
        assert!(errors.is_empty());
    }

    #[tokio::test]
    async fn malformed_validator_results_never_pass_q1_or_silently_drop_errors() {
        for body in [
            "result = {'pass': True}",
            "result = {'pass': True, 'errors': [42]}",
            "result = {'pass': True, 'errors': 'wrong shape'}",
            "result = {'pass': 1, 'errors': []}",
            "result = {'pass': True, 'errors': ['reported failure']}",
            "result = {'pass': False, 'errors': []}",
            "result = {'pass': True, 'errors': [], 'extra': 1}",
            "result = []",
            "result = {'pass': True, 'errors': []}\nunknown = '{{vars.slot3}}'",
        ] {
            let (passed, errors) =
                run_validator_python(body, "name", "description", "content").await;
            assert!(!passed, "invalid validator result passed: {body}");
            assert!(
                !errors.is_empty(),
                "failure diagnostics were discarded: {body}"
            );
        }
        let (passed, errors) = run_validator_python(
            "result = {'pass': False, 'errors': ['actual structural failure']}",
            "name",
            "description",
            "content",
        )
        .await;
        assert!(!passed);
        assert_eq!(errors, ["actual structural failure"]);
    }
    #[cfg(feature = "skills-db")]
    #[tokio::test]
    async fn native_q1_runs_on_the_exact_proposed_upgrade_content() {
        use serde_json::json;
        let rig = crate::runtime::test_pg::pg_rig().await;
        let scope = ComponentScope {
            tenant_id: "q1-upgrade".into(),
            user_id: "operator".into(),
            agent_id: "agent".into(),
            project_id: "project".into(),
        };
        let client = rig.pool.get().await.unwrap();
        let validator = "result = {'pass': inputs['content'] == 'requested replacement', 'errors': [] if inputs['content'] == 'requested replacement' else ['wrong reviewed body']}";
        let pc: Uuid = client.query_one(
            "INSERT INTO reborn_python_code (tenant_id,user_id,agent_id,project_id,name,description,content,validation_status)
             VALUES ($1,$2,$3,$4,'exact-upgrade-validator','Test actual candidate content',$5,'validated') RETURNING id",
            &[&scope.tenant_id,&scope.user_id,&scope.agent_id,&scope.project_id,&validator],
        ).await.unwrap().get(0);
        let descriptions = json!([{
            "desc_idx":0, "label":"Review candidate", "yaml_source":"Validate the exact proposed body",
            "steps":[{"stepnumber":1,"knowledge":"orchestrator","goal":"Review candidate","content":"Run approved validation code","type":"component","include":[pc.to_string()],"tool_bindings":[],"dependencies":null}]
        }]);
        client.execute(
            "INSERT INTO reborn_recipes (tenant_id,user_id,agent_id,project_id,name,description,validation_status,consumer_tags,validates_class_code,step_descriptions)
             VALUES ($1,$2,$3,$4,'exact-upgrade-review','Review PythonCode','validated',ARRAY['05:validator'],22,$5)",
            &[&scope.tenant_id,&scope.user_id,&scope.agent_id,&scope.project_id,&descriptions],
        ).await.unwrap();
        let candidate: Uuid = client.query_one(
            "INSERT INTO reborn_python_code (tenant_id,user_id,agent_id,project_id,name,description,content,validation_status)
             VALUES ($1,$2,$3,$4,'candidate-under-review','Existing usage','original live content','validated') RETURNING id",
            &[&scope.tenant_id,&scope.user_id,&scope.agent_id,&scope.project_id],
        ).await.unwrap().get(0);
        let store = ValidationQueueStore::new(rig.pool.clone());
        store
            .submit(
                &scope,
                candidate,
                22,
                Some(json!({"content":"requested replacement"})),
            )
            .await
            .unwrap();
        assert_eq!(
            run_q1_validation(&rig.pool, &scope, candidate, 22, &store, utility_limits())
                .await
                .unwrap(),
            Q1Outcome::Passed
        );
        let before: String = client
            .query_one(
                "SELECT content FROM reborn_python_code WHERE id=$1",
                &[&candidate],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(
            before, "original live content",
            "Q1 must not execute the upgrade by writing it live"
        );
        store
            .approve(&scope, candidate, Some("human"))
            .await
            .unwrap();
        let after: String = client
            .query_one(
                "SELECT content FROM reborn_python_code WHERE id=$1",
                &[&candidate],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(after, "requested replacement");
        assert_eq!(store.list(&scope, None).await.unwrap().len(), 0);
        // A validated Recipe does not authorize an unapproved executable row.
        store
            .submit(
                &scope,
                candidate,
                22,
                Some(json!({"content":"another proposed body"})),
            )
            .await
            .unwrap();
        client
            .execute(
                "UPDATE reborn_python_code SET validation_status='pending' WHERE id=$1",
                &[&pc],
            )
            .await
            .unwrap();
        assert!(matches!(
            run_q1_validation(&rig.pool, &scope, candidate, 22, &store, utility_limits()).await,
            Err(Q1Error::InvalidValidator { .. })
        ));
        assert_eq!(store.list(&scope, Some(1)).await.unwrap().len(), 1);
        client
            .execute(
                "UPDATE reborn_python_code SET validation_status='validated' WHERE id=$1",
                &[&pc],
            )
            .await
            .unwrap();
        let mut malformed = descriptions.clone();
        malformed[0]["steps"][0]["include"] = json!([pc.to_string(), pc.to_string()]);
        client.execute("UPDATE reborn_recipes SET step_descriptions=$5 WHERE tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4",
            &[&scope.tenant_id,&scope.user_id,&scope.agent_id,&scope.project_id,&malformed]).await.unwrap();
        assert!(matches!(
            run_q1_validation(&rig.pool, &scope, candidate, 22, &store, utility_limits()).await,
            Err(Q1Error::InvalidValidator { .. })
        ));
        client.execute("UPDATE reborn_recipes SET step_descriptions=$5 WHERE tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4",
            &[&scope.tenant_id,&scope.user_id,&scope.agent_id,&scope.project_id,&descriptions]).await.unwrap();
        client.execute(
            "INSERT INTO reborn_recipes (tenant_id,user_id,agent_id,project_id,name,description,validation_status,consumer_tags,validates_class_code,step_descriptions)
             VALUES ($1,$2,$3,$4,'ambiguous-upgrade-review','Second validated candidate','validated',ARRAY['05:validator'],22,$5)",
            &[&scope.tenant_id,&scope.user_id,&scope.agent_id,&scope.project_id,&descriptions],
        ).await.unwrap();
        assert!(matches!(
            run_q1_validation(&rig.pool, &scope, candidate, 22, &store, utility_limits()).await,
            Err(Q1Error::AmbiguousValidator { class_code: 22 })
        ));
        let current: String = client
            .query_one(
                "SELECT content FROM reborn_python_code WHERE id=$1",
                &[&candidate],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(current, "requested replacement");
        assert_eq!(store.list(&scope, Some(1)).await.unwrap().len(), 1);
    }
}
