//! Pinned validator coverage declarations, checked against actual retained IBS.
//! This proves preparation consistency, not approval, semantic coverage or Q1.
use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use serde_json::{Value, json};

use super::{retained_inputs::RetainedRecipeInputs, typed_bindings::PreparedReference};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Declaration {
    format: String,
    rules: Vec<Rule>,
    aggregate_step: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rule {
    rule_id: String,
    step_id: String,
}

/// Exact route and rule-to-step selection from the checksummed Recipe revision.
/// Required IDs are not taken from a reviewed candidate or a runtime pass JSON.
pub struct RetainedValidatorCoverage {
    class_code: i32,
    rules: BTreeMap<String, String>,
    aggregate_step: String,
    collection_step: String,
    selection_checksum: [u8; 32],
}

/// Actual completed checking observations, never semantic approval, a Q1
/// qualification, a durable task receipt or permission to activate/dispatch.
/// Only comparison with executor-owned observations can construct this value.
pub struct ObservedValidatorChecks {
    report: Value,
}
impl ObservedValidatorChecks {
    /// Sensitive audit data for the review owner, not model-visible context.
    pub fn report(&self) -> &Value {
        &self.report
    }
}
impl RetainedValidatorCoverage {
    /// Rebuild the exact selected variant and typed bindings from verified
    /// retained revisions. Full byte comparison includes order and approval
    /// references; malformed/reordered manifests never become executable feeds.
    /// This still supplies no approval, loaded Tool artifact or dispatch grant.
    #[cfg(feature = "skills-db")]
    pub fn restore_preparation(
        saved: &brassclaw_skills::review_submission_store::selection::RetainedValidatorSelection,
    ) -> Result<super::retained_tools::RetainedToolProgram, &'static str> {
        use super::{
            retained_instruction::{WorkflowClass, compile_retained_recipe},
            retained_tools::prepare_retained_tool_program,
        };
        let selection = saved.selection();
        let root = selection["recipe"]["uuid"]
            .as_str()
            .and_then(|value| uuid::Uuid::parse_str(value).ok())
            .ok_or("retained validator root missing")?;
        let variant = selection["variant"]["variant_key"]
            .as_str()
            .ok_or("retained validator variant missing")?;
        let class = match selection["workflow_class"].as_str() {
            Some("deterministic") => WorkflowClass::Deterministic,
            Some("requires_model") => WorkflowClass::RequiresModel,
            _ => return Err("retained validator workflow class invalid"),
        };
        let mut instruction = compile_retained_recipe(saved.snapshot(), root, variant, class)
            .map_err(|_| "retained validator compilation failed")?;
        if let Some(pins) = selection.get("association_approvals") {
            instruction.association_approvals = serde_json::from_value(pins.clone())
                .map_err(|_| "retained validator approval references invalid")?;
        }
        let program = prepare_retained_tool_program(instruction)
            .map_err(|_| "retained validator bindings invalid")?;
        Self::from_inputs(program.inputs())?;
        if program
            .inputs()
            .instruction()
            .retained_selection()
            .map_err(|_| "restored validator selection cannot be retained")?
            .exact_bytes()
            != saved.validator_bytes()
        {
            return Err("retained validator selection differs from typed IBS");
        }
        Ok(program)
    }

    /// Exact durable preparation bytes from a sealed subject and the actual
    /// inspected selection. This is not evidence of activation or permission;
    /// the review owner must establish both before admitting execution.
    #[cfg(feature = "skills-db")]
    pub fn selection_bytes(
        subject: &brassclaw_skills::review_submission_store::RetainedValidatorSubject,
        inspected: &crate::executor::retained_source::InspectedRetainedProgram,
        limits: brassclaw_skills::value_contract::ContractLimits,
    ) -> Result<String, &'static str> {
        Self::from_inputs(inspected.program().inputs())?.require_subject(subject)?;
        let selection = inspected
            .program()
            .inputs()
            .instruction()
            .retained_selection()
            .map_err(|_| "validator selection cannot be retained")?;
        let checksum: String = selection
            .checksum()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let value = json!({"format":"component-validator-selection/1",
            "submission_id":subject.value()["submission_id"],
            "subject_checksum":subject.value()["subject_checksum"],
            "candidate":subject.value()["candidate"]["reference"],
            "validator_selection_bytes":selection.exact_bytes(),
            "validator_selection_checksum":checksum,"scope":"preparation-only"});
        brassclaw_skills::value_contract::validate_data_bounds(&value, limits)
            .map_err(|_| "validator selection exceeds owner policy")?;
        Ok(value.to_string())
    }

    /// Check the complete coherent generation, including validator metadata
    /// without its expected tag. This is selection consistency, not approval
    /// or dispatch. Each class has exactly one selected private entry point.
    pub fn validate_catalogue<'a>(
        inputs: impl IntoIterator<Item = &'a RetainedRecipeInputs>,
    ) -> Result<(), &'static str> {
        let mut classes = BTreeSet::new();
        for inputs in inputs {
            let instruction = inputs.instruction();
            let document = instruction.snapshot().revisions()[&instruction.recipe().uuid]
                .draft()
                .document();
            let declared = document.get("validates_class_code").is_some()
                || document.get("validator_coverage").is_some()
                || document["consumer_tags"]
                    .as_array()
                    .is_some_and(|tags| tags.contains(&json!("05:validator")));
            if declared {
                let coverage = Self::from_inputs(inputs)?;
                if !classes.insert(coverage.class_code) {
                    return Err("ambiguous validator class route");
                }
            }
        }
        Ok(())
    }

    /// `validator_coverage` is supported immutable revision-document metadata,
    /// not an added legacy Recipe INSERT field. This initial contract covers the
    /// runner's supported flat occurrence layout; it grants no execution access.
    pub fn from_inputs(inputs: &RetainedRecipeInputs) -> Result<Self, &'static str> {
        let instruction = inputs.instruction();
        let document = instruction.snapshot().revisions()[&instruction.recipe().uuid]
            .draft()
            .document();
        let class_code = document["validates_class_code"]
            .as_i64()
            .and_then(|value| i32::try_from(value).ok())
            .filter(|value| matches!(value, 0 | 1 | 2 | 3 | 13 | 21 | 22 | 23))
            .ok_or("supported validator class required")?;
        let tags = document["consumer_tags"]
            .as_array()
            .ok_or("private validator tags required")?;
        if tags.as_slice() != [json!("05:validator")]
            || !instruction.variant().intent_examples.is_empty()
        {
            return Err("validator entry must remain private");
        }
        let declaration: Declaration =
            serde_json::from_value(document["validator_coverage"].clone())
                .map_err(|_| "invalid validator coverage declaration")?;
        if declaration.format != "validator-coverage/1" || declaration.rules.is_empty() {
            return Err("nonempty validator coverage required");
        }
        let order: BTreeMap<_, _> = instruction
            .ordered()
            .step_order()
            .iter()
            .filter(|step| inputs.contracts().contains_key(*step))
            .enumerate()
            .map(|(position, step)| (step.as_str(), position))
            .collect();
        let aggregate_position = *order
            .get(declaration.aggregate_step.as_str())
            .ok_or("selected aggregation step required")?;
        let mut rules = BTreeMap::new();
        let mut seen_steps = BTreeSet::new();
        let mut required = Vec::new();
        for rule in declaration.rules {
            if !rule_identifier(&rule.rule_id)
                || rules
                    .insert(rule.rule_id.clone(), rule.step_id.clone())
                    .is_some()
                || !seen_steps.insert(rule.step_id.clone())
            {
                return Err("distinct validator rule and step identities required");
            }
            let position = *order
                .get(rule.step_id.as_str())
                .ok_or("required rule step is outside selected workflow")?;
            if position >= aggregate_position {
                return Err("rule must precede aggregation");
            }
            // A constant output label alone does not prove a rule actually ran.
            // These recursive contracts ensure its complete concrete result is
            // checked before the trusted execution owner counts it.
            let component = inputs.components()[&rule.step_id];
            let result = &instruction.snapshot().revisions()[&component.uuid]
                .draft()
                .document()["result_contract"];
            if !rule_result_shape(result) {
                return Err("rule result contract is incomplete");
            }
            required.push(json!(rule.rule_id));
        }
        let aggregate = inputs
            .layout()
            .steps()
            .get(&declaration.aggregate_step)
            .ok_or("aggregation input layout required")?;
        let Some(PreparedReference::Constant { value }) = aggregate.get("required_rule_ids") else {
            return Err("required rules must be pinned Recipe constants");
        };
        if value != &Value::Array(required) {
            return Err("aggregation coverage differs from declaration");
        }
        let Some(PreparedReference::Result { step_id, path }) = aggregate.get("checks") else {
            return Err("aggregation must consume an executed collection step");
        };
        if !path.is_empty()
            || order
                .get(step_id.as_str())
                .is_none_or(|position| *position >= aggregate_position)
        {
            return Err("aggregation collection must be a complete prior result");
        }
        let collection = inputs
            .layout()
            .steps()
            .get(step_id)
            .ok_or("rule collection layout required")?;
        let collected: BTreeSet<_> = collection
            .values()
            .filter_map(|reference| match reference {
                PreparedReference::Result { step_id, path } if path.is_empty() => {
                    Some(step_id.as_str())
                }
                _ => None,
            })
            .collect();
        if collection.len() != rules.len()
            || collected.len() != rules.len()
            || rules.iter().any(|(rule, step)| {
                !matches!(collection.get(rule), Some(PreparedReference::Result { step_id, path })
                    if step_id == step && path.is_empty())
            })
        {
            return Err("collection must bind every complete rule result under its pinned rule ID");
        }
        let collection_position = order[step_id.as_str()];
        if rules
            .values()
            .any(|rule_step| order[rule_step.as_str()] >= collection_position)
        {
            return Err("collection must follow every required rule step");
        }
        let component = inputs.components()[&declaration.aggregate_step];
        let result = &instruction.snapshot().revisions()[&component.uuid]
            .draft()
            .document()["result_contract"];
        if result["type"] != "object"
            || result["allow_extra_fields"] != false
            || result.get("nullable").is_some_and(|value| value != false)
            || result["fields"]
                .as_object()
                .is_none_or(|fields| fields.len() != 2)
            || result["fields"]["pass"]["type"] != "boolean"
            || result["fields"]["pass"]["required"] != true
            || result["fields"]["errors"]["type"] != "list"
            || result["fields"]["errors"]["required"] != true
            || ["pass", "errors"].iter().any(|name| {
                result["fields"][*name]
                    .get("nullable")
                    .is_some_and(|value| value != false)
            })
        {
            return Err("aggregation result contract is incomplete");
        }
        Ok(Self {
            class_code,
            rules,
            aggregate_step: declaration.aggregate_step,
            collection_step: step_id.clone(),
            selection_checksum: instruction
                .retained_selection()
                .map_err(|_| "validator selection cannot be retained")?
                .checksum(),
        })
    }

    /// Bind complete concrete results to the actual selected executor. Values
    /// alone, a success-shaped aggregate, or an incomplete/failed execution are
    /// insufficient. The owner still verifies approval, class-subject identity,
    /// attempt freshness and actual task settlement before any Q1 write.
    pub fn observe(
        &self,
        execution: &crate::executor::retained_recipe::RetainedRecipeExecution,
        results: &BTreeMap<String, Value>,
        limits: brassclaw_skills::value_contract::ContractLimits,
    ) -> Result<ObservedValidatorChecks, &'static str> {
        use crate::executor::retained_recipe::typed_value_checksum;
        use brassclaw_skills::value_contract::validate_data_bounds;
        let inspected = execution.inspected_program();
        let instruction = inspected.program().inputs().instruction();
        if instruction
            .retained_selection()
            .map_err(|_| "executed selection cannot be retained")?
            .checksum()
            != self.selection_checksum
            || !execution.is_complete()
            || execution.transport_failure().is_some()
        {
            return Err("validator execution is incomplete or differs from selection");
        }
        let expected: Vec<_> = inspected
            .program()
            .program()
            .steplist
            .iter()
            .map(|step| step.step_id.as_str())
            .collect();
        if execution.observation_order().is_none_or(|order| {
            order
                .iter()
                .map(String::as_str)
                .ne(expected.iter().copied())
        }) || results.len() != expected.len()
            || execution.observations().len() != expected.len()
        {
            return Err("complete ordered executor observations required");
        }
        for step in &expected {
            let value = results.get(*step).ok_or("executed result missing")?;
            let observed = execution
                .observations()
                .get(*step)
                .ok_or("executor observation missing")?;
            if !observed.settled()
                || observed.failure().is_some()
                || observed.observation_error().is_some()
                || observed.result_checksum()
                    != Some(
                        typed_value_checksum(value)
                            .map_err(|_| "result fingerprint cannot be verified")?,
                    )
            {
                return Err("result differs from successful executor observation");
            }
        }
        let collected = results[&self.collection_step]
            .as_array()
            .filter(|values| values.len() == self.rules.len())
            .ok_or("complete observed rule collection required")?;
        let mut seen = BTreeSet::new();
        let mut errors = Vec::new();
        for check in collected {
            let rule = check["rule_id"]
                .as_str()
                .ok_or("observed rule label required")?;
            let step = self.rules.get(rule).ok_or("unselected rule observed")?;
            if !seen.insert(rule) || check != &results[step] {
                return Err("rule result differs from its selected execution step");
            }
            let ok = check["ok"]
                .as_bool()
                .ok_or("observed rule boolean required")?;
            let diagnostics = check["errors"]
                .as_array()
                .ok_or("observed diagnostics required")?;
            if ok != diagnostics.is_empty()
                || diagnostics
                    .iter()
                    .any(|value| value.as_str().is_none_or(str::is_empty))
            {
                return Err("observed rule result is contradictory");
            }
            if !ok {
                errors.push(json!({"rule_id":rule,"errors":diagnostics}));
            }
        }
        let aggregation = json!({"pass":errors.is_empty(),"errors":errors});
        if results[&self.aggregate_step] != aggregation {
            return Err("aggregation differs from observed rule results");
        }
        let report = json!({"format":"observed-validator-checks/1",
            "selection_checksum":self.selection_checksum.iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
            "class_code":self.class_code,"executed_steps":expected,
            "rules":collected,"aggregation":aggregation,
            "scope":"selected-validator-execution","semantic_approval":false,
            "q1_qualification":false});
        validate_data_bounds(&report, limits)
            .map_err(|_| "observation report exceeds owner policy")?;
        Ok(ObservedValidatorChecks { report })
    }
    pub fn class_code(&self) -> i32 {
        self.class_code
    }
    /// Compare against the sealed full subject reader, never a runtime class
    /// label supplied by the candidate. Selection approval remains separate.
    #[cfg(feature = "skills-db")]
    pub fn require_subject(
        &self,
        subject: &brassclaw_skills::review_submission_store::RetainedValidatorSubject,
    ) -> Result<(), &'static str> {
        if subject.candidate().class_code != self.class_code {
            return Err("validator route differs from retained subject class");
        }
        Ok(())
    }
    pub fn rules(&self) -> &BTreeMap<String, String> {
        &self.rules
    }
    pub fn aggregate_step(&self) -> &str {
        &self.aggregate_step
    }
}

fn rule_identifier(value: &str) -> bool {
    let mut chars = value.bytes();
    chars.next().is_some_and(|byte| byte.is_ascii_lowercase())
        && chars.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}
fn rule_result_shape(value: &Value) -> bool {
    if value["type"] != "object"
        || value["allow_extra_fields"] != false
        || value.get("nullable").is_some_and(|value| value != false)
    {
        return false;
    }
    let Some(fields) = value["fields"].as_object() else {
        return false;
    };
    if fields.len() != 3 {
        return false;
    }
    for (name, kind) in [("rule_id", "string"), ("ok", "boolean"), ("errors", "list")] {
        let Some(field) = fields.get(name) else {
            return false;
        };
        if field["type"] != kind
            || field["required"] != true
            || field.get("nullable").is_some_and(|value| value != false)
        {
            return false;
        }
    }
    fields["errors"]["items"]["type"] == "string"
        && fields["errors"]["items"]
            .get("nullable")
            .is_none_or(|value| value == false)
}
