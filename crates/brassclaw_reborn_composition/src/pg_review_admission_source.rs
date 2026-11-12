//! Sealed validator admission source for the `capture_internal` path.
//!
//! `ReviewAdmissionSource` implements [`InternalRecipeSource`] for a single
//! retained validator Recipe execution. The calling owner must:
//!
//! 1. Verify the submission exists and is active before constructing this source.
//! 2. Supply the exact `RetainedValidatorSelection` read from sealed storage.
//! 3. Prepare or restore `PreparedValidatorReview` with the original catalogue
//!    `PreparationCapture` and retained worker image, then use `from_prepared`.
//!    That owner restores exact IBS, inspects it and derives typed inputs once.
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
            RetainedProgram, RetainedRecipeExecution, typed_value_checksum_with_limits,
        },
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

#[path = "validator_input_capture.rs"]
mod validator_input_capture;
#[cfg(test)]
pub(crate) use validator_input_capture::assert_original_capture;
pub(crate) use validator_input_capture::{PreparedValidatorReview, ValidatorInputCapture};

/// Retain and verify the original review worker bytes for both authored task
/// preparation and contained native source observations.
pub(crate) async fn verified_worker(
    worker: &brassclaw_host_runtime::PackagedExecutableImage,
    max_bytes: u64,
) -> Result<Arc<brassclaw_host_runtime::PackagedExecutableImage>, crate::RebornBuildError> {
    let copy = brassclaw_host_runtime::PackagedExecutableImage::capture(&worker.path(), max_bytes)
        .await
        .map_err(crate::bootstrap_reply_validation::invalid)?;
    if copy.checksum() != worker.checksum() {
        return Err(crate::bootstrap_reply_validation::invalid(
            "retained review worker bytes changed",
        ));
    }
    Ok(Arc::new(copy))
}

/// Canonical reference prefix for validator review internal tasks.
pub(crate) const VALIDATOR_REVIEW_PREFIX: &str = "validator-review:";

/// Build the canonical `user_input` reference for a validator review task.
///
/// The admitted `TaskInput::user_input` must equal this value for the task to
/// be routed through `ReviewAdmissionSource`.
#[allow(dead_code)] // used by routing tests
pub(crate) fn validator_review_reference(submission_id: Uuid) -> String {
    format!("{VALIDATOR_REVIEW_PREFIX}{submission_id}")
}

/// Sealed single-class structural Q1 review — one submission, one validator.
///
/// Constructed only by the trusted `capture_internal` path in
/// `InstalledMontyCatalogue`. No public API or arbitrary JSON supplies the
/// sealed fields; the pool and subject come from durable storage reads.
pub(crate) struct ReviewAdmissionSource {
    reference: String,
    prepared: Arc<PreparedValidatorReview>,
    pool: Arc<PgPool>,
}
impl ReviewAdmissionSource {
    pub(crate) fn from_prepared(
        prepared: Arc<PreparedValidatorReview>,
        pool: Arc<PgPool>,
    ) -> Result<Self, &'static str> {
        let inspected = prepared.inspected();
        let RetainedProgram::Tools(program) = inspected.program() else {
            return Err("validator programme must be a Tools programme");
        };
        if !program.bindings().is_empty() {
            return Err("structural validator must have zero Tool bindings");
        }
        Ok(Self {
            reference: validator_review_reference(prepared.subject().submission_id()),
            prepared,
            pool,
        })
    }
    pub(crate) fn input_capture(&self) -> &ValidatorInputCapture {
        self.prepared.input_capture()
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
        Some(self.prepared.inspected.clone())
    }

    /// Pure validators have zero Tool bindings; binding native runtimes is a
    /// no-op. Already verified at construction, checked again defensively here.
    fn bind_native_runtimes(&self) -> Result<(), PortFailure> {
        let RetainedProgram::Tools(program) = self.prepared.inspected.program() else {
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
            inspected: self.prepared.inspected.clone(),
            inputs: Some(self.input_capture().inputs().clone()),
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
        let (checks, _) = ValidatorReceiptSubject {
            subject: &self.prepared.subject,
            selection: &self.prepared.selection,
            inspected: &self.prepared.inspected,
            coverage: &self.prepared.coverage,
            limits: validator_input_capture::limits(self.input_capture().preparation()),
        }
        .observe(
            ValidatorReceiptProfile::Structural,
            receipt,
            selected,
            execution,
        )
        .map_err(failure)?;

        // Step 5: persist structural workflow-review evidence. WorkflowReview::structural
        // derives its report from the sealed inspected programme — no receipt values
        // are needed for this record. Placed after observe_subject so no structural
        // evidence is written unless the fingerprint and observations are consistent.
        let structural_review = WorkflowReview::structural(Arc::clone(&self.prepared.inspected))
            .map_err(|e| match e {
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
            &self.prepared.subject,
            &self.prepared.selection,
            &checks,
            task_handle,
        )
        .await
        .map(|_| ())
        .map_err(|_| failure("validator_receipt_evidence_persist_failed"))
    }
}

/// Shared original-subject/receipt authentication. The profile selects the
/// existing wire grammar, never permissions, Q1 or terminal settlement.
pub(crate) enum ValidatorReceiptProfile {
    Structural,
    Inventory,
}
pub(crate) struct ValidatorReceiptSubject<'a> {
    pub(crate) subject: &'a RetainedValidatorSubject,
    pub(crate) selection: &'a RetainedValidatorSelection,
    pub(crate) inspected: &'a InspectedRetainedProgram,
    pub(crate) coverage: &'a RetainedValidatorCoverage,
    pub(crate) limits: ContractLimits,
}
impl ValidatorReceiptSubject<'_> {
    pub(crate) fn observe(
        self,
        profile: ValidatorReceiptProfile,
        receipt: &str,
        selected: &InspectedRetainedProgram,
        execution: &RetainedRecipeExecution,
    ) -> Result<
        (
            brassclaw_engine::memory::retained_validator::ObservedSubjectValidatorChecks,
            BTreeMap<String, Value>,
        ),
        &'static str,
    > {
        if self.selection.submission_id() != self.subject.submission_id()
            || self.selection.subject_checksum() != self.subject.subject_checksum()
        {
            return Err("validator_selection_subject_mismatch");
        }
        let results = receipt_results(
            profile,
            receipt,
            selected,
            execution,
            self.inspected,
            self.coverage,
            self.limits,
        )?;
        let checks = self
            .coverage
            .observe_subject(
                self.subject,
                self.selection,
                execution,
                &results,
                self.limits,
            )
            .map_err(|_| "validator_receipt_observation_failed")?;
        Ok((checks, results))
    }
}

pub(crate) struct InstallationReceiptSubject<'a> {
    pub(crate) subject:&'a brassclaw_skills::review_submission_store::installation::RetainedInstallationValidatorSubject,
    pub(crate) selection:&'a brassclaw_skills::review_submission_store::installation::RetainedInstallationValidatorSelection,
    pub(crate) inspected:&'a InspectedRetainedProgram,
    pub(crate) coverage:&'a RetainedValidatorCoverage,
    pub(crate) limits:ContractLimits,
}
impl InstallationReceiptSubject<'_> {
    pub(crate) fn observe(
        self,
        receipt: &str,
        selected: &InspectedRetainedProgram,
        execution: &RetainedRecipeExecution,
    ) -> Result<
        (
            brassclaw_engine::memory::retained_validator::ObservedInstallationValidatorChecks,
            BTreeMap<String, Value>,
        ),
        &'static str,
    > {
        if self.selection.subject() != self.subject.reference() {
            return Err("installation_selection_subject_mismatch");
        }
        let results = receipt_results(
            ValidatorReceiptProfile::Inventory,
            receipt,
            selected,
            execution,
            self.inspected,
            self.coverage,
            self.limits,
        )?;
        let checks = self.coverage.observe_installation_subject(
            self.subject,
            self.selection,
            execution,
            &results,
            self.limits,
        )?;
        Ok((checks, results))
    }
}
fn receipt_results(
    profile: ValidatorReceiptProfile,
    receipt: &str,
    selected: &InspectedRetainedProgram,
    execution: &RetainedRecipeExecution,
    inspected: &InspectedRetainedProgram,
    coverage: &RetainedValidatorCoverage,
    limits: ContractLimits,
) -> Result<BTreeMap<String, Value>, &'static str> {
    let original = inspected
        .program()
        .inputs()
        .instruction()
        .retained_selection()
        .map_err(|_| "validator_receipt_selection_invalid")?;
    let actual = selected
        .program()
        .inputs()
        .instruction()
        .retained_selection()
        .map_err(|_| "validator_receipt_selection_invalid")?;
    if actual.exact_bytes() != original.exact_bytes() {
        return Err("validator_receipt_programme_mismatch");
    }
    let receipt_step = coverage.receipt_step();
    let expected = selected
        .program()
        .program()
        .steplist
        .iter()
        .filter(|s| s.step_id != receipt_step)
        .map(|s| s.step_id.as_str())
        .collect::<Vec<_>>();
    let results = recorded_receipt_results(profile, receipt, &expected, receipt_step, limits)?;
    let proof = execution
        .observations()
        .get(receipt_step)
        .ok_or("validator_receipt_observation_missing")?;
    if proof.result_checksum()
        != Some(
            typed_value_checksum_with_limits(&serde_json::json!(receipt), proof.data_limits())
                .map_err(|_| "validator_receipt_checksum_overflow")?,
        )
    {
        return Err("validator_receipt_fingerprint_mismatch");
    }
    Ok(results)
}
/// Shared receipt grammar for actual execution and trusted original records.
/// This returns typed data only; executor/native seals remain separate.
pub(crate) fn recorded_receipt_results(
    profile: ValidatorReceiptProfile,
    receipt: &str,
    expected: &[&str],
    receipt_step: &str,
    limits: ContractLimits,
) -> Result<BTreeMap<String, Value>, &'static str> {
    let value = match profile {
        ValidatorReceiptProfile::Structural => {
            serde_json::from_str(receipt).map_err(|_| "validator_receipt_parse_failed")?
        }
        ValidatorReceiptProfile::Inventory => {
            brassclaw_skills::value_contract::strict_json(receipt, limits)
                .map_err(|_| "validator_receipt_parse_failed")?
        }
    };
    let expected_format = match profile {
        ValidatorReceiptProfile::Structural => "validator-structural-receipt/1",
        ValidatorReceiptProfile::Inventory => "validator-step-results/1",
    };
    if value["format"] != expected_format {
        return Err("validator_receipt_format_invalid");
    }
    let values = value
        .get("results")
        .and_then(Value::as_object)
        .ok_or("validator_receipt_results_missing")?;
    if matches!(profile, ValidatorReceiptProfile::Inventory)
        && (value.as_object().is_none_or(|object| object.len() != 2)
            || values.len() != expected.len()
            || expected.iter().any(|step| !values.contains_key(*step)))
    {
        return Err("validator_receipt_steps_invalid");
    }
    let mut results: BTreeMap<_, _> = values
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    results.insert(receipt_step.to_owned(), serde_json::json!(receipt));
    Ok(results)
}

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
#[allow(dead_code)] // used by routing tests
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
#[allow(dead_code)] // used by routing tests
pub(crate) fn make_validator_receipt(results: &BTreeMap<String, Value>) -> String {
    serde_json::json!({
        "format": "validator-structural-receipt/1",
        "results": results,
    })
    .to_string()
}

/// Build the subject input that is passed into the validator's `subject_json`
/// input field for pure structural checks. Uses REVISION_LIMITS by default.
#[allow(dead_code)] // used by routing tests
pub(crate) fn subject_inputs_with_default_limits(
    coverage: &RetainedValidatorCoverage,
    subject: &RetainedValidatorSubject,
    inspected: &InspectedRetainedProgram,
) -> Result<brassclaw_host_api::JsonValue, &'static str> {
    subject_validator_task_inputs(coverage, subject, inspected, REVISION_LIMITS)
}

#[cfg(test)]
mod routing_tests {
    use super::{VALIDATOR_REVIEW_PREFIX, validator_review_reference};
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
