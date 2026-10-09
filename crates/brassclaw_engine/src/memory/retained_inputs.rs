//! Typed input preparation from one exact retained workflow. This is new
//! retention-document metadata, not an extension of the legacy Recipe INSERT
//! schema. It supplies neither approval nor executable Tool artifacts.

use std::collections::{BTreeMap, BTreeSet};

use brassclaw_skills::{
    association_contract::ComponentRevisionRef,
    component_revision::{REVISION_LIMITS, RetainedComponentRevision},
    value_contract::{ContractError, InputContract, ValueContract, validate_data_bounds},
};
use serde_json::Value;
use uuid::Uuid;

use super::{
    composition::{
        ComponentResolver, ComposedProgram, ResolvedComponent, TypedCompositionError,
        compose_typed_program,
    },
    retained_instruction::RetainedRecipeInstruction,
    typed_bindings::{
        BindingSource, InputBinding, InputPreparationError, PreparedInputLayout, StepContracts,
        prepare_input_layout,
    },
};

// Technical parser capacity, independent of token budgets. Bound authoring
// regex compilation/cache memory before admitting a retained task context.
const MAX_CAPTURES: usize = 64;
const MAX_CAPTURE_REGEX_BYTES: usize = 32_768;
const MAX_CAPTURE_DFA_BYTES: usize = 8_192;

#[derive(Debug, thiserror::Error)]
pub enum RetainedInputError {
    #[error("retained input preparation: {0}")]
    Invalid(&'static str),
    #[error(transparent)]
    Contract(#[from] ContractError),
    #[error(transparent)]
    Binding(#[from] InputPreparationError),
    #[error(transparent)]
    Composition(#[from] TypedCompositionError),
}
fn invalid(reason: &'static str) -> RetainedInputError {
    RetainedInputError::Invalid(reason)
}
fn record<'a>(
    value: &'a Value,
    fields: &[&str],
) -> Result<&'a serde_json::Map<String, Value>, RetainedInputError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("object required"))?;
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err(invalid("missing or unsupported metadata field"));
    }
    Ok(object)
}

/// Privately constructed bindings retain their originating instruction and
/// exact component references. A caller cannot replace a contract with one
/// loaded from a newer mutable row. Monty still owns runtime values/results.
pub struct RetainedRecipeInputs {
    instruction: RetainedRecipeInstruction,
    task: InputContract,
    contracts: BTreeMap<String, StepContracts>,
    components: BTreeMap<String, ComponentRevisionRef>,
    layout: PreparedInputLayout,
    capture_patterns: Vec<Option<regex::Regex>>,
}

/// Retained assembly without Tool bindings uses the existing typed composer. Tool workflows
/// need a verified exact implementation adapter; paths from mutable documents
/// cannot fill that gap. This draft-validation result carries no activation.
pub struct RetainedUnboundProgram {
    inputs: RetainedRecipeInputs,
    program: ComposedProgram,
}
impl RetainedUnboundProgram {
    pub fn inputs(&self) -> &RetainedRecipeInputs {
        &self.inputs
    }
    pub fn program(&self) -> &ComposedProgram {
        &self.program
    }
}
pub(super) struct RetainedComponentResolver<'a>(pub(super) &'a RetainedRecipeInstruction);

/// The persisted PythonCode schema always carries `includes: []` for a leaf.
/// Accept that explicit empty list without confusing it with nested assembly.
/// Nonempty includes/dependencies still require the dedicated assembly path;
/// malformed metadata cannot silently erase the intended executable graph.
pub(super) fn require_leaf_python(
    component: &RetainedComponentRevision,
) -> Result<(), &'static str> {
    let draft = component.draft();
    let includes = match draft.document().get("includes") {
        None => false,
        Some(value) => !value
            .as_array()
            .ok_or("PythonCode includes must be an array")?
            .is_empty(),
    };
    if crate::executor::retained_preload::PreloadDeclaration::parse(
        draft.document(),
        draft.dependencies(),
    )
    .map_err(|_| "invalid retained preload dependency contract")?
    .is_some()
    {
        if includes
            || draft
                .document()
                .get("dependency_registry")
                .is_some_and(|v| !v.is_null())
        {
            return Err("legacy include assembly cannot be combined with a preload library");
        }
        return Ok(());
    }
    if !draft.dependencies().is_empty()
        || includes
        || draft
            .document()
            .get("dependency_registry")
            .is_some_and(|value| !value.is_null())
    {
        return Err("nested PythonCode requires explicit retained assembly");
    }
    Ok(())
}

impl ComponentResolver for RetainedComponentResolver<'_> {
    fn resolve(&self, id: Uuid) -> Option<ResolvedComponent> {
        let component = self.0.snapshot().revisions().get(&id)?;
        let class_code = component.reference().class_code;
        if !matches!(class_code, 0 | 13 | 22) {
            return None;
        }
        let document = component.draft().document();
        Some(ResolvedComponent {
            class_code: class_code as i16,
            name: document
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            content: if class_code == 22 {
                document.get("content")?.as_str()?.to_owned()
            } else {
                document
                    .get("content")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned()
            },
            description: document
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            cdylib_artifact_path: None,
        })
    }
}

pub fn prepare_retained_unbound_program(
    instruction: RetainedRecipeInstruction,
) -> Result<RetainedUnboundProgram, RetainedInputError> {
    if !instruction.ordered().instruction().rust_steps.is_empty() {
        return Err(invalid(
            "Tool workflow requires an exact implementation adapter",
        ));
    }
    for step in &instruction.ordered().instruction().orchestrator_steps {
        let component = &instruction.snapshot().revisions()[&step.include[0]];
        require_leaf_python(component).map_err(invalid)?;
    }
    let program = compose_typed_program(
        instruction.ordered(),
        &RetainedComponentResolver(&instruction),
    )?;
    let inputs = prepare_retained_inputs(instruction)?;
    Ok(RetainedUnboundProgram { inputs, program })
}
impl RetainedRecipeInputs {
    pub fn instruction(&self) -> &RetainedRecipeInstruction {
        &self.instruction
    }
    pub fn layout(&self) -> &PreparedInputLayout {
        &self.layout
    }
    pub fn components(&self) -> &BTreeMap<String, ComponentRevisionRef> {
        &self.components
    }
    pub fn contracts(&self) -> &BTreeMap<String, StepContracts> {
        &self.contracts
    }
    /// Emit data for the existing Monty recipe-flow/1 interpreter. Rust compiles
    /// the layout; Python selects each next step and owns concrete result state.
    /// No source, Tool grant or new program/attempt identity is issued here.
    pub fn monty_flow(&self) -> Result<Value, RetainedInputError> {
        // Monty's validator retains both node and source-step identities and a
        // final return under its 512-entry technical bound.
        if self.layout.steps().len() > 255 {
            return Err(invalid("prepared flow exceeds technical node capacity"));
        }
        let mut body = Vec::new();
        let mut last = None;
        for step_id in self.instruction.ordered().step_order() {
            let Some(bindings) = self.layout.steps().get(step_id) else {
                continue; // Rust-channel availability is prepared separately.
            };
            for source in bindings.values() {
                if let super::typed_bindings::PreparedReference::Result { path, .. } = source
                    && (path.len() > 16 || path.iter().any(String::is_empty))
                {
                    return Err(invalid("prepared field path exceeds Monty flow capacity"));
                }
            }
            body.push(serde_json::json!({
                "kind":"step", "node_id":format!("execute_{}", body.len()),
                "step_id":step_id, "inputs":bindings,
            }));
            last = Some(step_id);
        }
        let last = last.ok_or_else(|| invalid("prepared flow has no executable step"))?;
        body.push(
            serde_json::json!({"kind":"return", "node_id":"recipe_return",
            "source":{"kind":"result", "step_id":last, "path":[]}}),
        );
        Ok(serde_json::json!({"format":"recipe-flow/1", "body":body}))
    }
    /// Call after exact capture/refinement, before starting executable code.
    /// Missing defaults are materialized as independent data. Invalid or null
    /// supplied values never fall back to defaults or positional raw captures.
    pub fn bind_task_inputs(&self, supplied: &Value) -> Result<Value, RetainedInputError> {
        Ok(Value::Object(self.task.bind(supplied)?))
    }
    /// Capture an example from this exact variant. Strings remain
    /// strings; numeric/boolean conversion requires an explicit reviewed logic
    /// component. This does not select an intent or read mutable matcher rows.
    pub fn bind_variant_example(
        &self,
        template: &str,
        user_text: &str,
        additional: &Value,
    ) -> Result<Value, RetainedInputError> {
        if user_text.len() > REVISION_LIMITS.max_bytes || template.len() > REVISION_LIMITS.max_bytes
        {
            return Err(invalid("capture exceeds technical input capacity"));
        }
        let variant = self.instruction.variant();
        if !variant
            .intent_examples
            .iter()
            .any(|example| example == template)
        {
            return Err(invalid("template does not belong to the retained variant"));
        }
        let count = template.bytes().filter(|byte| *byte == b'%').count();
        if count != variant.variable_patterns.len() {
            return Err(invalid(
                "every capture needs one semantic input declaration",
            ));
        }
        let segments: Vec<_> = template.split('%').collect();
        validate_data_bounds(additional, REVISION_LIMITS)?;
        let mut values = additional
            .as_object()
            .ok_or_else(|| invalid("additional inputs object required"))?
            .clone();
        if count == 0 {
            if user_text != template {
                return Err(invalid("input does not match the retained template"));
            }
            return self.bind_task_inputs(&Value::Object(values));
        }
        if (segments[0].is_empty() && segments[count].is_empty())
            || segments[1..count].iter().any(|segment| segment.is_empty())
        {
            return Err(invalid(
                "anchored template with separated captures required",
            ));
        }
        let mut remainder = user_text
            .strip_prefix(segments[0])
            .and_then(|text| text.strip_suffix(segments[count]))
            .ok_or_else(|| invalid("input does not match the retained template"))?;
        let mut names = BTreeSet::new();
        let declared: BTreeSet<_> = self.task.names().collect();
        for (index, pattern) in variant.variable_patterns.iter().enumerate() {
            if !declared.contains(pattern.name.as_str())
                || !names.insert(pattern.name.as_str())
                || values.contains_key(&pattern.name)
            {
                return Err(invalid(
                    "capture name is undeclared, duplicate or already supplied",
                ));
            }
            let raw = if index + 1 == count {
                remainder
            } else {
                let separator = segments[index + 1];
                let offset = remainder
                    .find(separator)
                    .ok_or_else(|| invalid("capture separator missing"))?;
                // The authoring guide's positional convention consumes the
                // first literal separator left-to-right. Q1/behavioral review
                // must cover separator text inside values for this layout.
                let value = &remainder[..offset];
                remainder = &remainder[offset + separator.len()..];
                value
            };
            let refined = if let Some(regex) = &self.capture_patterns[index] {
                let captures = regex
                    .captures(raw)
                    .ok_or_else(|| invalid("capture refinement failed"))?;
                let complete = captures
                    .get(0)
                    .ok_or_else(|| invalid("capture refinement failed"))?;
                if complete.start() != 0 || complete.end() != raw.len() {
                    return Err(invalid(
                        "capture refinement must validate the complete slot",
                    ));
                }
                if regex.capture_names().flatten().next().is_none() {
                    raw.to_owned()
                } else {
                    captures
                        .name(&pattern.name)
                        .ok_or_else(|| invalid("declared capture group did not participate"))?
                        .as_str()
                        .to_owned()
                }
            } else {
                raw.to_owned()
            };
            values.insert(pattern.name.clone(), Value::String(refined));
        }
        self.bind_task_inputs(&Value::Object(values))
    }
    /// Monty resolves the prepared references in its own task context. The host
    /// checks that concrete data against the pinned consumer before execution.
    pub fn bind_step_inputs(
        &self,
        step_id: &str,
        supplied: &Value,
    ) -> Result<Value, RetainedInputError> {
        let contract = self
            .contracts
            .get(step_id)
            .ok_or_else(|| invalid("step is not in the selected workflow"))?;
        Ok(Value::Object(contract.inputs.bind(supplied)?))
    }
    /// Validation never creates an output or changes completed effect evidence.
    pub fn validate_step_result(
        &self,
        step_id: &str,
        result: &Value,
    ) -> Result<(), RetainedInputError> {
        self.contracts
            .get(step_id)
            .ok_or_else(|| invalid("step is not in the selected workflow"))?
            .result
            .validate(result)?;
        Ok(())
    }
}

/// New immutable metadata contract:
/// Recipe document.input_layouts[variant_key] holds recipe-input-layout/1;
/// PythonCode documents hold input_contract and result_contract. All are inside
/// the retained checksummed bytes. Legacy rows without these fail explicitly.
pub fn prepare_retained_inputs(
    instruction: RetainedRecipeInstruction,
) -> Result<RetainedRecipeInputs, RetainedInputError> {
    let recipe = &instruction.snapshot().revisions()[&instruction.recipe().uuid];
    let layouts = recipe
        .draft()
        .document()
        .get("input_layouts")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("retained Recipe input layouts required"))?;
    let raw = layouts
        .get(&instruction.variant().variant_key)
        .ok_or_else(|| invalid("selected variant input layout missing"))?;
    let layout = record(raw, &["format", "task_inputs", "steps"])?;
    if layout["format"] != "recipe-input-layout/1" {
        return Err(invalid("unsupported input layout format"));
    }
    let task = InputContract::from_value(&layout["task_inputs"], REVISION_LIMITS)?;
    if instruction.variant().variable_patterns.len() > MAX_CAPTURES {
        return Err(invalid("capture layout exceeds technical parser capacity"));
    }
    let declared: BTreeSet<_> = task.names().collect();
    let mut capture_names = BTreeSet::new();
    let mut capture_patterns = Vec::new();
    for pattern in &instruction.variant().variable_patterns {
        if !declared.contains(pattern.name.as_str()) || !capture_names.insert(pattern.name.as_str())
        {
            return Err(invalid("capture name is undeclared or duplicate"));
        }
        let compiled = pattern
            .pattern
            .as_ref()
            .map(|source| {
                let compiled = regex::RegexBuilder::new(source)
                    .size_limit(MAX_CAPTURE_REGEX_BYTES)
                    .dfa_size_limit(MAX_CAPTURE_DFA_BYTES)
                    .build()
                    .map_err(|_| invalid("invalid retained capture pattern"))?;
                let names: Vec<_> = compiled.capture_names().flatten().collect();
                if !names.is_empty() && (names.len() != 1 || names[0] != pattern.name.as_str()) {
                    return Err(invalid("capture group must name its declared input"));
                }
                Ok(compiled)
            })
            .transpose()?;
        capture_patterns.push(compiled);
    }
    let raw_steps = layout["steps"]
        .as_object()
        .ok_or_else(|| invalid("step-local bindings object required"))?;
    let mut declarations = BTreeMap::new();
    for (step_id, value) in raw_steps {
        let locals = value
            .as_object()
            .ok_or_else(|| invalid("local bindings object required"))?;
        let mut bindings = Vec::with_capacity(locals.len());
        for (local_name, value) in locals {
            let source = match value.get("kind").and_then(Value::as_str) {
                Some("task_input") => {
                    let data = record(value, &["kind", "reference"])?;
                    BindingSource::TaskInputReference(
                        data["reference"]
                            .as_str()
                            .ok_or_else(|| invalid("input reference string required"))?
                            .to_owned(),
                    )
                }
                Some("constant") => {
                    let data = record(value, &["kind", "value"])?;
                    BindingSource::Constant(data["value"].clone())
                }
                Some("consumer_default") => {
                    record(value, &["kind"])?;
                    BindingSource::ConsumerDefault
                }
                Some("result") => {
                    let data = record(value, &["kind", "step_id", "path"])?;
                    let step_id = data["step_id"]
                        .as_str()
                        .filter(|id| !id.is_empty())
                        .ok_or_else(|| invalid("result producer identity required"))?
                        .to_owned();
                    let path = data["path"]
                        .as_array()
                        .ok_or_else(|| invalid("result field path required"))?
                        .iter()
                        .map(|field| {
                            field
                                .as_str()
                                .map(str::to_owned)
                                .ok_or_else(|| invalid("result field name required"))
                        })
                        .collect::<Result<_, _>>()?;
                    BindingSource::PriorResult { step_id, path }
                }
                _ => return Err(invalid("unsupported binding source")),
            };
            bindings.push(InputBinding {
                local_name: local_name.clone(),
                source,
            });
        }
        declarations.insert(step_id.clone(), bindings);
    }
    let mut contracts = BTreeMap::new();
    let mut components = BTreeMap::new();
    for step in &instruction.ordered().instruction().orchestrator_steps {
        let component = &instruction.snapshot().revisions()[&step.include[0]];
        let document = component.draft().document();
        let inputs = InputContract::from_value(
            document
                .get("input_contract")
                .ok_or_else(|| invalid("retained PythonCode input contract missing"))?,
            REVISION_LIMITS,
        )?;
        let result = ValueContract::result_from_value(
            document
                .get("result_contract")
                .ok_or_else(|| invalid("retained PythonCode result contract missing"))?,
            REVISION_LIMITS,
        )?;
        components.insert(step.step_id.clone(), component.reference());
        contracts.insert(step.step_id.clone(), StepContracts { inputs, result });
    }
    let layout = prepare_input_layout(instruction.ordered(), &task, &contracts, &declarations)?;
    Ok(RetainedRecipeInputs {
        instruction,
        task,
        contracts,
        components,
        layout,
        capture_patterns,
    })
}
