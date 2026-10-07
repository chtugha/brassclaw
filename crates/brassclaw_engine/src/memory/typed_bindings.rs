//! Pure IBS preparation of step-local typed inputs. Monty resolves the prepared
//! references against its task context and owns sequencing/results. No runtime
//! values are inserted into Python and this module executes no component/Tool.
//!
//! Contracts must come from the same approved, pinned catalogue snapshot as the
//! ordered instruction. This compiler proves data layout, not that snapshot's
//! approval. The persisted layout/association store still needs that enforcement.

use std::collections::{BTreeMap, BTreeSet};

use brassclaw_skills::value_contract::{ContractError, InputContract, ValueContract};
use serde::Serialize;
use serde_json::Value;

use super::instruction_builder::OrderedBuildInstruction;

/// Prepared complete component input/output contracts, selected by the caller
/// at the same exact revision as the step's PythonCode.
pub struct StepContracts {
    pub inputs: InputContract,
    pub result: ValueContract,
}

/// Authoring declarations are data. Constants are never reinterpreted as refs.
pub enum BindingSource {
    /// Exact whole-value `{{vars.NAME}}` grammar from recipe.md.
    TaskInputReference(String),
    Constant(Value),
    ConsumerDefault,
    PriorResult {
        step_id: String,
        path: Vec<String>,
    },
}

pub struct InputBinding {
    pub local_name: String,
    pub source: BindingSource,
}

/// Data handed to the global Monty context. There is no Python source field.
#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PreparedReference {
    Input { name: String },
    Constant { value: Value },
    Result { step_id: String, path: Vec<String> },
}

/// Private construction preserves exact selected step IDs and complete bindings.
pub struct PreparedInputLayout {
    steps: BTreeMap<String, BTreeMap<String, PreparedReference>>,
}

impl PreparedInputLayout {
    pub fn steps(&self) -> &BTreeMap<String, BTreeMap<String, PreparedReference>> {
        &self.steps
    }
}

#[derive(Debug, thiserror::Error)]
#[error("typed input preparation at step {step_id}: {reason}")]
pub struct InputPreparationError {
    pub step_id: String,
    pub reason: &'static str,
    #[source]
    pub contract: Option<ContractError>,
}

fn error(step_id: &str, reason: &'static str) -> InputPreparationError {
    InputPreparationError {
        step_id: step_id.into(),
        reason,
        contract: None,
    }
}

fn contract_error(step_id: &str, contract: ContractError) -> InputPreparationError {
    InputPreparationError {
        step_id: step_id.into(),
        reason: "incompatible input contract",
        contract: Some(contract),
    }
}

fn input_reference(expression: &str) -> Option<&str> {
    let name = expression.strip_prefix("{{vars.")?.strip_suffix("}}")?;
    let mut bytes = name.bytes();
    (bytes.next().is_some_and(|b| b.is_ascii_lowercase())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'))
    .then_some(name)
}

/// Prepare a straight ordered dataflow before the first effect. Guarded branches
/// and loop-item references require their separate control-tree proof; they must
/// not be flattened into this API or inferred from optional result fields.
pub fn prepare_input_layout(
    ordered: &OrderedBuildInstruction,
    task_inputs: &InputContract,
    contracts: &BTreeMap<String, StepContracts>,
    declarations: &BTreeMap<String, Vec<InputBinding>>,
) -> Result<PreparedInputLayout, InputPreparationError> {
    let selected: BTreeSet<_> = ordered
        .instruction()
        .orchestrator_steps
        .iter()
        .map(|step| step.step_id.as_str())
        .collect();
    if contracts.len() != selected.len()
        || declarations.len() != selected.len()
        || contracts.keys().any(|id| !selected.contains(id.as_str()))
        || declarations
            .keys()
            .any(|id| !selected.contains(id.as_str()))
    {
        return Err(error(
            "",
            "contracts and bindings must cover exactly the selected executable steps",
        ));
    }
    let mut prior = BTreeSet::new();
    let mut steps = BTreeMap::new();
    for step_id in ordered.step_order() {
        if !selected.contains(step_id.as_str()) {
            continue;
        }
        let consumer = &contracts[step_id].inputs;
        let names: BTreeSet<_> = consumer.names().collect();
        let mut bindings = BTreeMap::new();
        for binding in &declarations[step_id] {
            if !names.contains(binding.local_name.as_str())
                || bindings.contains_key(&binding.local_name)
            {
                return Err(error(step_id, "undeclared or duplicate local binding"));
            }
            let reference = match &binding.source {
                BindingSource::TaskInputReference(expression) => {
                    let name = input_reference(expression)
                        .ok_or_else(|| error(step_id, "invalid whole-value input reference"))?;
                    let producer = task_inputs
                        .bound_value_contract(name)
                        .map_err(|error| contract_error(step_id, error))?;
                    producer
                        .require_compatible_input(consumer, &binding.local_name)
                        .map_err(|error| contract_error(step_id, error))?;
                    PreparedReference::Input { name: name.into() }
                }
                BindingSource::PriorResult {
                    step_id: producer_id,
                    path,
                } => {
                    if !prior.contains(producer_id.as_str()) {
                        return Err(error(
                            step_id,
                            "result producer is not a completed predecessor",
                        ));
                    }
                    let producer = contracts[producer_id]
                        .result
                        .required_field_path(path)
                        .map_err(|error| contract_error(step_id, error))?;
                    producer
                        .require_compatible_input(consumer, &binding.local_name)
                        .map_err(|error| contract_error(step_id, error))?;
                    PreparedReference::Result {
                        step_id: producer_id.clone(),
                        path: path.clone(),
                    }
                }
                BindingSource::Constant(value) => {
                    // Validate just this consumer input using its exact schema.
                    // Cross-input checks are checked by full runtime binding.
                    consumer
                        .validate_supplied_input(&binding.local_name, value)
                        .map_err(|error| contract_error(step_id, error))?;
                    PreparedReference::Constant {
                        value: value.clone(),
                    }
                }
                BindingSource::ConsumerDefault => {
                    let value = consumer
                        .default_for_input(&binding.local_name)
                        .map_err(|error| contract_error(step_id, error))?;
                    PreparedReference::Constant { value }
                }
            };
            bindings.insert(binding.local_name.clone(), reference);
        }
        if bindings.len() != names.len() {
            return Err(error(
                step_id,
                "every declared local input needs an explicit binding",
            ));
        }
        if !prior.insert(step_id.as_str()) {
            return Err(error(step_id, "duplicate executable step"));
        }
        steps.insert(step_id.clone(), bindings);
    }
    if steps.len() != selected.len() {
        return Err(error(
            "",
            "selected executable step is absent from instruction order",
        ));
    }
    Ok(PreparedInputLayout { steps })
}
