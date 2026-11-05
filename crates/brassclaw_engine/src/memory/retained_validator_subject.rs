//! Subject/input custody for the existing selected validator observer. Recipes
//! still select and execute checks; this verifier neither schedules nor approves.
use std::collections::BTreeMap;

use brassclaw_skills::{
    review_submission_store::{RetainedValidatorSubject, selection::RetainedValidatorSelection},
    value_contract::{ContractLimits, validate_data_bounds},
};
use serde_json::{Value, json};

use super::{PreparedReference, RetainedValidatorCoverage};
use crate::executor::retained_recipe::{
    RetainedRecipeExecution, typed_object_checksum_with_limits,
};

use brassclaw_skills::review_submission_store::installation::{
    RetainedInstallationValidatorSelection, RetainedInstallationValidatorSubject,
};

#[derive(Clone, Copy)]
pub(super) enum SubjectView<'a> {
    Authored(&'a RetainedValidatorSubject),
    Installation(&'a RetainedInstallationValidatorSubject),
}
impl<'a> SubjectView<'a> {
    fn value(self) -> &'a Value {
        match self {
            Self::Authored(s) => s.value(),
            Self::Installation(s) => s.value(),
        }
    }
    pub(super) fn candidate(self) -> brassclaw_skills::association_contract::ComponentRevisionRef {
        match self {
            Self::Authored(s) => s.candidate(),
            Self::Installation(s) => s.candidate(),
        }
    }
    pub(super) fn is_function_review(self) -> bool {
        match self {
            Self::Authored(s) => s.is_function_review(),
            Self::Installation(_) => false,
        }
    }
    fn id(self) -> uuid::Uuid {
        match self {
            Self::Authored(s) => s.submission_id(),
            Self::Installation(s) => s.reference().id,
        }
    }
    fn subject_checksum(self) -> &'a str {
        match self {
            Self::Authored(s) => s.subject_checksum(),
            Self::Installation(s) => &s.reference().checksum,
        }
    }
}
#[derive(Clone, Copy)]
enum SelectionView<'a> {
    Authored(&'a RetainedValidatorSelection),
    Installation(&'a RetainedInstallationValidatorSelection),
}
impl<'a> SelectionView<'a> {
    fn id(self) -> uuid::Uuid {
        match self {
            Self::Authored(s) => s.submission_id(),
            Self::Installation(s) => s.subject().id,
        }
    }
    fn subject_checksum(self) -> &'a str {
        match self {
            Self::Authored(s) => s.subject_checksum(),
            Self::Installation(s) => &s.subject().checksum,
        }
    }
    fn validator_bytes(self) -> &'a str {
        match self {
            Self::Authored(s) => s.validator_bytes(),
            Self::Installation(s) => s.validator_bytes(),
        }
    }
}
/// Passive input evidence. Constructing it grants no execution/review authority.
pub struct ValidatorInputFingerprint {
    pub checksum: [u8; 32],
    pub limits: ContractLimits,
}
/// Installation-only factual observation; cannot be passed to authored Q1 writers.
pub struct ObservedInstallationValidatorChecks {
    report: Value,
}
impl ObservedInstallationValidatorChecks {
    pub fn report(&self) -> &Value {
        &self.report
    }
}

/// Complete actual observations bound to one sealed original submission and
/// stored validator selection. This is deliberately distinct from unbound
/// fixture observations. It still supplies no Q1, semantic or approval verdict.
pub struct ObservedSubjectValidatorChecks {
    report: Value,
}
impl ObservedSubjectValidatorChecks {
    /// Sensitive owner audit data; never a model-written success record.
    pub fn report(&self) -> &Value {
        &self.report
    }
}

impl RetainedValidatorCoverage {
    /// Materialize the full sealed subject as JSON string data for the supported
    /// private validator input profile. No source substitution or caller facts.
    pub fn subject_task_inputs(
        &self,
        subject: &RetainedValidatorSubject,
        inputs: &super::RetainedRecipeInputs,
        limits: ContractLimits,
    ) -> Result<Value, &'static str> {
        self.subject_task_inputs_view(SubjectView::Authored(subject), inputs, limits)
    }
    pub fn installation_task_inputs(
        &self,
        subject: &RetainedInstallationValidatorSubject,
        inputs: &super::RetainedRecipeInputs,
        limits: ContractLimits,
    ) -> Result<Value, &'static str> {
        self.subject_task_inputs_view(SubjectView::Installation(subject), inputs, limits)
    }
    fn subject_task_inputs_view(
        &self,
        subject: SubjectView<'_>,
        inputs: &super::RetainedRecipeInputs,
        limits: ContractLimits,
    ) -> Result<Value, &'static str> {
        self.require_subject_view(subject)?;
        if inputs
            .instruction()
            .retained_selection()
            .map_err(|_| "validator selection cannot be retained")?
            .checksum()
            != self.selection_checksum
            || !inputs.layout().steps().values().any(|bindings| {
                bindings.values().any(|reference| {
                    matches!(reference, PreparedReference::Input { name } if name == "subject_json")
                })
            })
        {
            return Err("selected validator must consume its complete subject input");
        }
        validate_data_bounds(subject.value(), limits)
            .map_err(|_| "validator subject exceeds owner policy")?;
        let length = brassclaw_common::json::count_json_bytes(
            subject.value(),
            Some(u64::try_from(limits.max_bytes).map_err(|_| "subject capacity overflow")?),
        )
        .map_err(|_| "encoded validator subject exceeds owner policy")?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(usize::try_from(length).map_err(|_| "subject capacity overflow")?)
            .map_err(|_| "validator subject capacity unavailable")?;
        brassclaw_common::json::write_json(&mut bytes, subject.value())
            .map_err(|_| "validator subject encoding failed")?;
        let value = json!({"subject_json":String::from_utf8(bytes)
            .map_err(|_| "validator subject encoding failed")?});
        inputs
            .bind_task_inputs_with_limits(&value, limits)
            .map_err(|_| "validator subject input contract differs")
    }

    /// Verify the original subject, stored preparation and every actual input
    /// edge before creating subject-bound observations. The existing observer
    /// additionally checks complete results/order under captured per-step limits.
    pub fn observe_subject(
        &self,
        subject: &RetainedValidatorSubject,
        saved: &RetainedValidatorSelection,
        execution: &RetainedRecipeExecution,
        results: &BTreeMap<String, Value>,
        limits: ContractLimits,
    ) -> Result<ObservedSubjectValidatorChecks, &'static str> {
        self.observe_subject_view(
            SubjectView::Authored(subject),
            SelectionView::Authored(saved),
            execution,
            results,
            limits,
        )
        .map(|report| ObservedSubjectValidatorChecks { report })
    }
    pub fn observe_installation_subject(
        &self,
        subject: &RetainedInstallationValidatorSubject,
        saved: &RetainedInstallationValidatorSelection,
        execution: &RetainedRecipeExecution,
        results: &BTreeMap<String, Value>,
        limits: ContractLimits,
    ) -> Result<ObservedInstallationValidatorChecks, &'static str> {
        self.observe_subject_view(
            SubjectView::Installation(subject),
            SelectionView::Installation(saved),
            execution,
            results,
            limits,
        )
        .map(|report| ObservedInstallationValidatorChecks { report })
    }
    /// Recheck immutable recorded inputs without manufacturing an execution or
    /// an Observed* seal. Both live observation and historical metadata use the
    /// same original subject serializer and exact dataflow checksum core.
    pub fn verify_installation_input_fingerprints(
        &self,
        subject: &RetainedInstallationValidatorSubject,
        saved: &RetainedInstallationValidatorSelection,
        program: &crate::memory::retained_tools::RetainedToolProgram,
        results: &BTreeMap<String, Value>,
        fingerprints: &BTreeMap<String, ValidatorInputFingerprint>,
        limits: ContractLimits,
    ) -> Result<(), &'static str> {
        if subject.reference() != saved.subject()
            || program
                .inputs()
                .instruction()
                .retained_selection()
                .map_err(|_| "recorded selection invalid")?
                .exact_bytes()
                != saved.validator_bytes()
        {
            return Err("recorded subject selection differs");
        }
        let order = program
            .program()
            .steplist
            .iter()
            .map(|s| s.step_id.as_str())
            .collect::<Vec<_>>();
        self.verify_subject_edges(
            SubjectView::Installation(subject),
            program.inputs(),
            &order,
            results,
            fingerprints,
            limits,
        )
    }
    fn verify_subject_edges(
        &self,
        subject: SubjectView<'_>,
        inputs: &super::RetainedRecipeInputs,
        order: &[&str],
        results: &BTreeMap<String, Value>,
        fingerprints: &BTreeMap<String, ValidatorInputFingerprint>,
        limits: ContractLimits,
    ) -> Result<(), &'static str> {
        if fingerprints.len() != order.len() {
            return Err("complete original input evidence required");
        }
        let task_inputs = self.subject_task_inputs_view(subject, inputs, limits)?;
        // Verify dataflow, not workflow decisions: Monty already executed the
        // pinned flat order. A changed input producing the same output is still
        // a different observation and cannot become evidence for this subject.
        for step in order {
            let step = *step;
            let actual = fingerprints
                .get(step)
                .ok_or("validator input observation missing")?;
            let bindings = inputs
                .layout()
                .steps()
                .get(step)
                .ok_or("validator input layout missing")?;
            let mut expected = BTreeMap::new();
            for (name, reference) in bindings {
                let value = match reference {
                    PreparedReference::Input { name } => task_inputs
                        .get(name)
                        .ok_or("validator task input missing")?,
                    PreparedReference::Constant { value } => value,
                    PreparedReference::Result { step_id, path } => {
                        let mut value = results
                            .get(step_id)
                            .ok_or("validator predecessor result missing")?;
                        for field in path {
                            value = value
                                .as_object()
                                .and_then(|object| object.get(field))
                                .ok_or("validator predecessor result field missing")?;
                        }
                        value
                    }
                };
                expected.insert(name.clone(), value);
            }
            if typed_object_checksum_with_limits(&expected, actual.limits)
                .map_err(|_| "validator expected input exceeds captured limits")?
                != actual.checksum
            {
                return Err("validator input differs from sealed subject dataflow");
            }
        }
        Ok(())
    }

    fn observe_subject_view(
        &self,
        subject: SubjectView<'_>,
        saved: SelectionView<'_>,
        execution: &RetainedRecipeExecution,
        results: &BTreeMap<String, Value>,
        limits: ContractLimits,
    ) -> Result<Value, &'static str> {
        if !matches!(
            (subject, saved),
            (SubjectView::Authored(_), SelectionView::Authored(_))
                | (SubjectView::Installation(_), SelectionView::Installation(_))
        ) {
            return Err("validator subject and selection origins differ");
        }
        self.require_subject_view(subject)?;
        let inspected = execution.inspected_program();
        let inputs = inspected.program().inputs();
        if saved.id() != subject.id()
            || saved.subject_checksum() != subject.subject_checksum()
            || inputs
                .instruction()
                .retained_selection()
                .map_err(|_| "executed selection cannot be retained")?
                .exact_bytes()
                != saved.validator_bytes()
        {
            return Err("validator execution differs from original subject selection");
        }
        let fingerprints = execution
            .observations()
            .iter()
            .map(|(step, actual)| {
                (
                    step.clone(),
                    ValidatorInputFingerprint {
                        checksum: actual.input_checksum(),
                        limits: actual.data_limits(),
                    },
                )
            })
            .collect();
        let order = inspected
            .program()
            .program()
            .steplist
            .iter()
            .map(|s| s.step_id.as_str())
            .collect::<Vec<_>>();
        self.verify_subject_edges(subject, inputs, &order, results, &fingerprints, limits)?;
        let mut report = self.observe(execution, results, limits)?.report;
        let object = report
            .as_object_mut()
            .ok_or("validator observation report invalid")?;
        match subject {
            SubjectView::Authored(_) => {
                object.insert(
                    "format".into(),
                    json!("observed-subject-validator-checks/1"),
                );
                object.insert("scope".into(), json!("subject-bound-validator-execution"));
                object.insert("submission_id".into(), json!(subject.id()));
            }
            SubjectView::Installation(_) => {
                object.insert(
                    "format".into(),
                    json!("observed-installation-validator-checks/1"),
                );
                object.insert(
                    "scope".into(),
                    json!("controlled-installation-source-review"),
                );
                object.insert("subject_id".into(), json!(subject.id()));
                object.insert("semantic_approval".into(), json!(false));
                object.insert("catalogue_activation".into(), json!(false));
            }
        }
        object.insert("subject_checksum".into(), json!(subject.subject_checksum()));
        object.insert("task_handle".into(), json!(execution.task_handle()));
        validate_data_bounds(&report, limits)
            .map_err(|_| "subject observation report exceeds owner policy")?;
        Ok(report)
    }
}
