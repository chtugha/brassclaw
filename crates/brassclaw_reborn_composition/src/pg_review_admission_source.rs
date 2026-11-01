//! Sealed validator admission source for the `capture_internal` path.
//!
//! `ReviewAdmissionSource` implements [`InternalRecipeSource`] for a single
//! retained validator Recipe execution. The calling owner must:
//!
//! 1. Verify the submission exists and is active before constructing this source.
//! 2. Supply the exact `RetainedValidatorSelection` read from sealed storage.
//! 3. Pass the inspected program from `RetainedValidatorCoverage::restore_preparation`
//!    plus `InspectedRetainedProgram::inspect_with_limits`.
//! 4. Call `capture_internal` only on a `trusted_internal_turn` whose `user_input`
//!    equals `Self::reference_for(submission_id)`.
//!
//! # Receipt format
//!
//! The Python orchestrator for the structural validator must post its internal
//! receipt as a JSON string of the form:
//! ```json
//! {"format": "validator-structural-receipt/1", "results": {"step_id": <value>, …}}
//! ```
//! where each `step_id` key maps to the actual return value produced by that
//! step. `verify_receipt` then performs the following sequence, in order:
//!
//! 1. Verifies that the executed programme matches the one sealed at admission
//!    by comparing their `RetainedRecipeSelection` checksums.
//! 2. Parses the receipt JSON and extracts the per-step `results` map.
//! 3. Persists a `WorkflowReview::structural` record via `persist_workflow_review`,
//!    recording the selected-variant structure of the executed programme.
//! 4. Calls `RetainedValidatorCoverage::observe_subject` with the parsed
//!    per-step results map, the sealed subject, the sealed selection and the
//!    completed execution to produce `ObservedSubjectValidatorChecks`.
//! 5. Calls `persist_validator_q1_evidence` to write the durable Q1 evidence row.
//!
//! Steps 3 and 5 must both succeed before `Ok(())` is returned. A failed persist
//! leaves the task in an incomplete-evidence state; neither Q1 success nor a
//! fabricated pass is synthesised from a persistence error.
//!
//! # Reference string
//!
//! The canonical reference string for a validator review task is
//! `validator-review:<submission_id_hyphenated>` — lowercase canonical UUID.
//! The `capture_internal` implementation in `InstalledMontyCatalogue` must match
//! on this prefix before delegating here.
//!
//! # Feature gate
//!
//! Requires the `postgres` and `skills-db` features.

#![forbid(unsafe_code)]

use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use brassclaw_engine::{
    executor::{
        retained_recipe::{RetainedProgram, RetainedRecipeExecution},
        retained_source::InspectedRetainedProgram,
    },
    memory::retained_validator::RetainedValidatorCoverage,
};
use brassclaw_monty_host::service::PortFailure;
use brassclaw_pg::PgPool;
use brassclaw_skills::{
    component_revision::REVISION_LIMITS,
    review_submission_store::{RetainedValidatorSubject, selection::RetainedValidatorSelection},
    value_contract::ContractLimits,
};
use serde_json::Value;
use uuid::Uuid;

use crate::global_recipe_ports::{InternalRecipeSource, SelectedMontyRecipe};
use crate::pg_retained_q1::StructuralReviewFailure;
use crate::pg_workflow_review::{WorkflowReview, persist_workflow_review};

/// Canonical reference prefix for validator review internal tasks.
#[allow(dead_code)] // wired by validator Step 6 capture_internal routing
pub(crate) const VALIDATOR_REVIEW_PREFIX: &str = "validator-review:";

/// Build the canonical `user_input` reference for a validator review task.
///
/// The admitted `TaskInput::user_input` must equal this value for the task to
/// be routed through `ReviewAdmissionSource`.
#[allow(dead_code)] // wired by validator Step 6 capture_internal routing
pub(crate) fn validator_review_reference(submission_id: Uuid) -> String {
    format!("{VALIDATOR_REVIEW_PREFIX}{submission_id}")
}

/// Sealed single-class structural Q1 review — one submission, one validator.
///
/// Constructed only by the trusted `capture_internal` path in
/// `InstalledMontyCatalogue`. No public API or arbitrary JSON supplies the
/// sealed fields; the pool and subject come from durable storage reads.
#[allow(dead_code)] // wired by validator Step 6 capture_internal routing
pub(crate) struct ReviewAdmissionSource {
    /// Canonical `user_input` reference for this review task.
    reference: String,
    /// Sealed submission subject (candidate + base + dependencies).
    subject: Arc<RetainedValidatorSubject>,
    /// Sealed validator programme selection (recipe + snapshot + variant).
    selection: Arc<RetainedValidatorSelection>,
    /// Inspected retained programme ready for execution. Pure-logic only;
    /// Tool bindings are rejected at construction via validator coverage check.
    inspected: Arc<InspectedRetainedProgram>,
    /// Retained coverage declaration, derived once from the sealed programme at
    /// construction. Storing it here ensures construction-time errors surface
    /// before any task execution begins.
    coverage: RetainedValidatorCoverage,
    /// Typed inputs derived from the sealed subject, matching the validator's
    /// declared `input_layout`. Constructed once at admission time.
    inputs: brassclaw_host_api::JsonValue,
    /// Database pool for `verify_receipt` evidence persistence.
    pool: Arc<PgPool>,
    /// Contract limits for data-bounds checks in `verify_receipt`.
    limits: ContractLimits,
}

impl ReviewAdmissionSource {
    /// Construct a sealed source from an already-read subject and selection.
    ///
    /// The caller must have verified that:
    /// - `subject.submission_id()` matches `selection.submission_id()`
    /// - `selection` was read from `reborn_component_validator_selections`
    /// - `inspected` was built from `RetainedValidatorCoverage::restore_preparation`
    ///   followed by `InspectedRetainedProgram::inspect_with_limits`
    ///
    /// Returns `Err` if the inspected program does not satisfy the validator
    /// coverage contract (e.g. has unexpected Tool bindings for a pure validator).
    #[allow(dead_code)] // wired by validator Step 6 capture_internal routing
    pub(crate) fn new(
        subject: Arc<RetainedValidatorSubject>,
        selection: Arc<RetainedValidatorSelection>,
        inspected: Arc<InspectedRetainedProgram>,
        inputs: brassclaw_host_api::JsonValue,
        pool: Arc<PgPool>,
        limits: ContractLimits,
    ) -> Result<Self, &'static str> {
        // Verify subject/selection agreement.
        if selection.submission_id() != subject.submission_id()
            || selection.subject_checksum() != subject.subject_checksum()
        {
            return Err("validator selection subject mismatch");
        }
        // Verify the inspected programme satisfies the validator coverage contract
        // and retain the result — derive once, not on every verify_receipt call.
        let coverage = RetainedValidatorCoverage::from_inputs(inspected.program().inputs())
            .map_err(|_| "validator programme does not satisfy coverage declaration")?;
        // Verify pure-logic (no Tool bindings) — structural validators must not
        // dispatch any Tools during execution.
        let RetainedProgram::Tools(program) = inspected.program() else {
            return Err("validator programme must be a Tools programme");
        };
        if !program.bindings().is_empty() {
            return Err("structural validator must have zero Tool bindings");
        }
        let reference = validator_review_reference(subject.submission_id());
        Ok(Self {
            reference,
            subject,
            selection,
            inspected,
            coverage,
            inputs,
            pool,
            limits,
        })
    }

    /// The sealed submission subject (read-only for callers needing class code).
    #[allow(dead_code)] // wired by validator Step 6 capture_internal routing
    pub(crate) fn subject(&self) -> &RetainedValidatorSubject {
        &self.subject
    }
}

#[async_trait]
impl InternalRecipeSource for ReviewAdmissionSource {
    fn name(&self) -> &str {
        "validator-structural-review"
    }

    fn reference(&self) -> &str {
        &self.reference
    }

    fn captured_program(&self) -> Option<Arc<InspectedRetainedProgram>> {
        Some(self.inspected.clone())
    }

    /// Pure validators have zero Tool bindings; binding native runtimes is a
    /// no-op. Already verified at construction, checked again defensively here.
    fn bind_native_runtimes(&self) -> Result<(), PortFailure> {
        let RetainedProgram::Tools(program) = self.inspected.program() else {
            return Err(failure("validator_binding_program_type"));
        };
        if !program.bindings().is_empty() {
            return Err(failure("validator_binding_unexpected_tools"));
        }
        Ok(())
    }

    /// Step observations are required so that `verify_receipt` can verify the
    /// actual executor observation fingerprints against the parsed receipt results.
    fn requires_step_observations(&self) -> bool {
        true
    }

    /// Return the sealed inspected programme and typed inputs. No new DB read.
    async fn select(&self) -> Result<SelectedMontyRecipe, PortFailure> {
        Ok(SelectedMontyRecipe {
            normal_match: None,
            inspected: self.inspected.clone(),
            inputs: Some(self.inputs.clone()),
            // Pure structural validators have no Tool port.
            tools: None,
        })
    }

    /// Persist the sealed Q1 evidence after verifying the receipt.
    ///
    /// The Python orchestrator posts a receipt string of the form:
    /// `{"format": "validator-structural-receipt/1", "results": {"step_id": <value>, …}}`
    /// This method verifies programme identity, persists structural workflow evidence,
    /// produces sealed observations, and persists durable Q1 evidence — in that order.
    async fn verify_receipt(
        &self,
        receipt: &str,
        selected: &InspectedRetainedProgram,
        execution: &RetainedRecipeExecution,
    ) -> Result<(), PortFailure> {
        // Step 1: verify the executed programme matches the admission-time selection
        // by comparing RetainedRecipeSelection checksums — the v3 identity contract.
        // ptr equality is an implementation accident that does not survive refactoring.
        let executed_checksum = selected
            .program()
            .inputs()
            .instruction()
            .retained_selection()
            .map_err(|_| failure("validator_receipt_selection_invalid"))?
            .checksum();
        let sealed_checksum = self
            .inspected
            .program()
            .inputs()
            .instruction()
            .retained_selection()
            .map_err(|_| failure("validator_receipt_sealed_selection_invalid"))?
            .checksum();
        if executed_checksum != sealed_checksum {
            return Err(failure("validator_receipt_programme_mismatch"));
        }

        // Step 2: parse the receipt JSON. The orchestrator must post the exact envelope.
        let receipt_value: Value =
            serde_json::from_str(receipt).map_err(|_| failure("validator_receipt_parse_failed"))?;
        if receipt_value["format"] != "validator-structural-receipt/1" {
            return Err(failure("validator_receipt_format_invalid"));
        }
        let results_value = receipt_value
            .get("results")
            .and_then(Value::as_object)
            .ok_or_else(|| failure("validator_receipt_results_missing"))?;
        let results: BTreeMap<String, Value> = results_value
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        // Step 3: persist structural workflow-review evidence before Q1 evidence.
        // WorkflowReview::structural derives its report from the sealed inspected
        // programme — no receipt values are needed. Both persists must succeed;
        // neither a partial nor a fabricated pass is acceptable on failure.
        let structural_review =
            WorkflowReview::structural(Arc::clone(&self.inspected)).map_err(|e| match e {
                StructuralReviewFailure::Preparation(reason) => {
                    tracing::warn!(reason, "validator structural review preparation failed");
                    failure("validator_receipt_structural_review_failed")
                }
                StructuralReviewFailure::Integrity | StructuralReviewFailure::Observation => {
                    failure("validator_receipt_structural_review_integrity")
                }
                StructuralReviewFailure::Connection(_) | StructuralReviewFailure::Database(_) => {
                    failure("validator_receipt_structural_review_failed")
                }
            })?;
        persist_workflow_review(&self.pool, Arc::new(structural_review))
            .await
            .map_err(|_| failure("validator_receipt_structural_review_persist_failed"))?;

        // Step 4: produce sealed subject-bound observations using the stored
        // coverage declaration (derived once at construction in new()).
        let checks = self
            .coverage
            .observe_subject(
                &self.subject,
                &self.selection,
                execution,
                &results,
                self.limits,
            )
            .map_err(|_| failure("validator_receipt_observation_failed"))?;

        // Extract the task handle UUID. TaskHandle wraps Uuid and only exposes
        // Display; the hyphenated form produced by Display always round-trips
        // through Uuid::parse_str without error.
        let task_handle = Uuid::parse_str(&execution.task_handle().to_string())
            .map_err(|_| failure("validator_receipt_task_handle_invalid"))?;

        // Step 5: persist the durable Q1 evidence row.
        crate::pg_validator_q1_evidence::persist_validator_q1_evidence(
            &self.pool,
            &self.subject,
            &self.selection,
            &checks,
            task_handle,
        )
        .await
        .map(|_| ())
        .map_err(|_| failure("validator_receipt_evidence_persist_failed"))
    }
}

#[allow(dead_code)] // wired by validator Step 6 capture_internal routing
fn failure(reason: &'static str) -> PortFailure {
    PortFailure::new(reason).expect("static classified reason")
}

/// Build the typed input object for the structural validator Recipe.
///
/// The structural validator Recipe's `input_layout` expects a `subject_json`
/// field containing the complete serialised subject as a JSON string. This
/// is produced by `RetainedValidatorCoverage::subject_task_inputs`.
///
/// All values come from the sealed subject's candidate revision document.
/// No source substitution occurs; values are typed data.
#[allow(dead_code)] // wired by validator Step 6 capture_internal routing
pub(crate) fn subject_validator_task_inputs(
    coverage: &RetainedValidatorCoverage,
    subject: &RetainedValidatorSubject,
    inspected: &InspectedRetainedProgram,
    limits: ContractLimits,
) -> Result<brassclaw_host_api::JsonValue, &'static str> {
    coverage
        .subject_task_inputs(subject, inspected.program().inputs(), limits)
        .map(|v| v.into())
}

/// Check that the given `receipt` string can be decoded and that its step
/// results can be matched against the `execution` observations. This is the
/// same verification path as `verify_receipt` but returns the parsed results
/// map for callers that need it (e.g. test helpers).
///
/// Does NOT write any evidence row; that is done by `verify_receipt`.
#[allow(dead_code)] // wired by validator Step 6 capture_internal routing
pub(crate) fn parse_validator_receipt(
    receipt: &str,
    limits: ContractLimits,
) -> Result<BTreeMap<String, Value>, &'static str> {
    let receipt_value: Value =
        serde_json::from_str(receipt).map_err(|_| "validator receipt parse failed")?;
    if receipt_value["format"] != "validator-structural-receipt/1" {
        return Err("validator receipt format invalid");
    }
    let results_value = receipt_value
        .get("results")
        .and_then(Value::as_object)
        .ok_or("validator receipt results missing")?;
    let results: BTreeMap<String, Value> = results_value
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    // Validate the total size of the parsed results against limits.
    let as_value = Value::Object(
        results
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
    );
    brassclaw_skills::value_contract::validate_data_bounds(&as_value, limits)
        .map_err(|_| "validator receipt results exceed data limits")?;
    Ok(results)
}

/// Format a canonical validator receipt string for the Python orchestrator.
///
/// The structural validator orchestrator posts the receipt by calling
/// `host.post_internal_receipt(make_validator_receipt(results))`.
/// Each step_id maps to the Value that `host.run_program(step_id)` returned.
#[allow(dead_code)] // wired by validator Step 6 capture_internal routing
pub(crate) fn make_validator_receipt(results: &BTreeMap<String, Value>) -> String {
    serde_json::json!({
        "format": "validator-structural-receipt/1",
        "results": results,
    })
    .to_string()
}

/// Build the subject input that is passed into the validator's `subject_json`
/// input field for pure structural checks. Uses REVISION_LIMITS by default.
#[allow(dead_code)] // wired by validator Step 6 capture_internal routing
pub(crate) fn subject_inputs_with_default_limits(
    coverage: &RetainedValidatorCoverage,
    subject: &RetainedValidatorSubject,
    inspected: &InspectedRetainedProgram,
) -> Result<brassclaw_host_api::JsonValue, &'static str> {
    subject_validator_task_inputs(coverage, subject, inspected, REVISION_LIMITS)
}
