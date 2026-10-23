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
        self.require_subject(subject)?;
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
        self.require_subject(subject)?;
        let inspected = execution.inspected_program();
        let inputs = inspected.program().inputs();
        if saved.submission_id() != subject.submission_id()
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
        let task_inputs = self.subject_task_inputs(subject, inputs, limits)?;
        // Verify dataflow, not workflow decisions: Monty already executed the
        // pinned flat order. A changed input producing the same output is still
        // a different observation and cannot become evidence for this subject.
        for step in &inspected.program().program().steplist {
            let step = step.step_id.as_str();
            let actual = execution
                .observations()
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
            if typed_object_checksum_with_limits(&expected, actual.data_limits())
                .map_err(|_| "validator expected input exceeds captured limits")?
                != actual.input_checksum()
            {
                return Err("validator input differs from sealed subject dataflow");
            }
        }
        let mut report = self.observe(execution, results, limits)?.report;
        let object = report
            .as_object_mut()
            .ok_or("validator observation report invalid")?;
        object.insert(
            "format".into(),
            json!("observed-subject-validator-checks/1"),
        );
        object.insert("scope".into(), json!("subject-bound-validator-execution"));
        object.insert("submission_id".into(), json!(subject.submission_id()));
        object.insert("subject_checksum".into(), json!(subject.subject_checksum()));
        object.insert("task_handle".into(), json!(execution.task_handle()));
        validate_data_bounds(&report, limits)
            .map_err(|_| "subject observation report exceeds owner policy")?;
        Ok(ObservedSubjectValidatorChecks { report })
    }
}
