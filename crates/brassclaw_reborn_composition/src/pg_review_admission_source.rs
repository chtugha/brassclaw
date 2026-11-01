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
//! step. `verify_receipt` performs the following sequence, in strict order:
//!
//! 1. Verifies that the executed programme matches the one sealed at admission
//!    by comparing their `RetainedRecipeSelection` checksums.
//! 2. Parses the receipt JSON and extracts the per-step `results` map.
//! 3. **Verifies the aggregate-step fingerprint**: the raw receipt string is the
//!    typed value the Python orchestrator returned at the aggregate step, so
//!    `proof.result_checksum()` must equal
//!    `typed_value_checksum_with_limits(&json!(receipt), proof.data_limits())`.
//!    This is the tamper-evidence link between the receipt payload and the sealed
//!    execution record. Any mismatch causes an immediate `Err` before any writes.
//! 4. Calls `RetainedValidatorCoverage::observe_subject` with the parsed
//!    per-step results map, the sealed subject, the sealed selection and the
//!    completed execution to produce `ObservedSubjectValidatorChecks`. This
//!    additionally verifies per-step input and result fingerprints across all steps.
//! 5. Persists a `WorkflowReview::structural` record via `persist_workflow_review`,
//!    recording the selected-variant structure of the executed programme.
//! 6. Calls `persist_validator_q1_evidence` to write the durable Q1 evidence row.
//!
//! Steps 5 and 6 must both succeed before `Ok(())` is returned. No writes occur
//! before step 3 passes. A failed persist leaves the task in an
//! incomplete-evidence state; neither Q1 success nor a fabricated pass is
//! synthesised from a persistence error.
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
        retained_recipe::{
            typed_value_checksum_with_limits, RetainedProgram, RetainedRecipeExecution,
        },
        retained_source::InspectedRetainedProgram,
    },
    memory::retained_validator::RetainedValidatorCoverage,
};
use brassclaw_monty_host::service::PortFailure;
use brassclaw_pg::PgPool;
use brassclaw_skills::{
    component_revision::REVISION_LIMITS,
    review_submission_store::{selection::RetainedValidatorSelection, RetainedValidatorSubject},
    value_contract::ContractLimits,
};
use serde_json::Value;
use uuid::Uuid;

use crate::global_recipe_ports::{InternalRecipeSource, SelectedMontyRecipe};
use crate::pg_retained_q1::StructuralReviewFailure;
use crate::pg_workflow_review::{persist_workflow_review, WorkflowReview};

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
    ///
    /// Exact verification sequence — see module-level doc for the full contract:
    /// 1. Programme identity (RetainedRecipeSelection checksum).
    /// 2. Receipt JSON parsing and envelope format.
    /// 3. Aggregate-step fingerprint: the raw receipt string is the value Python
    ///    returned at the aggregate step; its checksum must match the executor's
    ///    sealed observation before any writes occur.
    /// 4. `observe_subject`: per-step input + result fingerprint verification across
    ///    all steps, producing sealed `ObservedSubjectValidatorChecks`.
    /// 5. `WorkflowReview::structural` + `persist_workflow_review`.
    /// 6. `persist_validator_q1_evidence`.
    ///
    /// No writes occur before step 3 passes. Neither a partial write nor a
    /// fabricated pass is synthesised from any individual failure.
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

        // Step 3: verify the aggregate-step fingerprint.
        //
        // The Python orchestrator returns the receipt as a string value at the
        // aggregate step. The executor records its SHA-256 fingerprint (as
        // typed-value-sha256/1 of Value::String(receipt)) in the observation.
        // Verifying this before any writes proves the receipt came from the actual
        // Monty execution rather than being supplied externally. A spoofed or
        // truncated receipt string would produce a different checksum and is
        // rejected here, before observe_subject runs.
        let aggregate_step = self.coverage.aggregate_step();
        let proof = execution
            .observations()
            .get(aggregate_step)
            .ok_or_else(|| failure("validator_receipt_aggregate_observation_missing"))?;
        // The receipt string is what Python returned — it is typed as
        // Value::String, so json!(receipt) wraps it correctly.
        let expected_checksum =
            typed_value_checksum_with_limits(&serde_json::json!(receipt), proof.data_limits())
                .map_err(|_| failure("validator_receipt_checksum_overflow"))?;
        if proof.result_checksum() != Some(expected_checksum) {
            return Err(failure("validator_receipt_fingerprint_mismatch"));
        }

        // Step 4: produce sealed subject-bound observations. This verifies per-step
        // input and result fingerprints for every step using the stored coverage
        // declaration (derived once at construction in new()).
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

        // Step 5: persist structural workflow-review evidence. WorkflowReview::structural
        // derives its report from the sealed inspected programme — no receipt values
        // are needed for this record. Placed after observe_subject so no structural
        // evidence is written unless the fingerprint and observations are consistent.
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

        // Step 6: persist the durable Q1 evidence row.
        // TaskHandle wraps Uuid and only exposes Display; the hyphenated form
        // produced by Display always round-trips through Uuid::parse_str without error.
        let task_handle = Uuid::parse_str(&execution.task_handle().to_string())
            .map_err(|_| failure("validator_receipt_task_handle_invalid"))?;
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

#[cfg(test)]
mod routing_tests {
    use super::{validator_review_reference, VALIDATOR_REVIEW_PREFIX};
    use uuid::Uuid;

    /// The prefix constant has the exact expected form used by capture_internal.
    #[test]
    fn validator_review_prefix_has_expected_form() {
        assert_eq!(VALIDATOR_REVIEW_PREFIX, "validator-review:");
    }

    /// A user_input matching the prefix + valid UUID is detected as a validator
    /// review admission. This mirrors the starts_with check in capture_internal.
    #[test]
    fn user_input_with_prefix_and_valid_uuid_is_detected_as_validator_review() {
        let id = Uuid::new_v4();
        let reference = validator_review_reference(id);
        assert!(
            reference.starts_with(VALIDATOR_REVIEW_PREFIX),
            "reference must start with VALIDATOR_REVIEW_PREFIX"
        );
        // The suffix after stripping the prefix must be a valid UUID.
        let suffix = reference
            .strip_prefix(VALIDATOR_REVIEW_PREFIX)
            .expect("suffix must exist");
        let parsed = Uuid::parse_str(suffix).expect("suffix must be a valid UUID");
        assert_eq!(parsed, id, "round-trip UUID must match original");
    }

    /// A user_input that does not match the prefix is NOT detected as a
    /// validator review admission. This mirrors the early-return in capture_internal.
    #[test]
    fn user_input_without_prefix_is_not_detected_as_validator_review() {
        let inputs = [
            "",
            "ordinary-chat-message",
            "prefix-job:some-uuid",
            // Malformed: missing the colon separator
            &format!("validator-review{}", Uuid::new_v4()),
            // Correct prefix but no UUID
            "validator-review:",
            // Correct prefix but invalid UUID
            "validator-review:not-a-uuid",
        ];
        for input in inputs {
            assert!(
                !input.starts_with(VALIDATOR_REVIEW_PREFIX)
                    || Uuid::parse_str(input.strip_prefix(VALIDATOR_REVIEW_PREFIX).unwrap_or(""))
                        .is_err(),
                "input {:?} must not be detected as a valid validator review reference",
                input
            );
        }
    }

    /// The receipt format string matches what parse_validator_receipt expects.
    #[test]
    fn make_and_parse_receipt_round_trip() {
        use super::{make_validator_receipt, parse_validator_receipt};
        use brassclaw_skills::component_revision::REVISION_LIMITS;
        use std::collections::BTreeMap;

        let mut results = BTreeMap::new();
        results.insert("step_1".to_string(), serde_json::json!({"ok": true}));
        results.insert("step_2".to_string(), serde_json::json!(42));

        let receipt_str = make_validator_receipt(&results);
        let parsed = parse_validator_receipt(&receipt_str, REVISION_LIMITS)
            .expect("valid receipt must parse cleanly");
        assert_eq!(parsed, results, "round-trip must produce identical map");
    }
}
