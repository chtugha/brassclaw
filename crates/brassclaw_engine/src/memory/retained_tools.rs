//! Tool binding preparation from the same retained graph as Recipe/IBS inputs.
//! Binding executes nothing. The host must separately retain the selected
//! implementation, resolve trusted approval evidence and enforce live policy.
//! A mutable artifact path or a parsed association cannot satisfy those gates.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use brassclaw_skills::{
    association_contract::{AssociationError, ComponentRevisionRef, SkillAssociation},
    component_revision::REVISION_LIMITS,
    value_contract::{ContractError, InputContract},
};
use serde_json::Value;
use uuid::Uuid;

use super::{
    composition::{ComposedProgram, compose_typed_program},
    retained_inputs::{
        RetainedComponentResolver, RetainedInputError, RetainedRecipeInputs,
        prepare_retained_inputs, require_leaf_python,
    },
    retained_instruction::RetainedRecipeInstruction,
};
use crate::types::ibs::ErrorPolicy;

#[derive(Debug, thiserror::Error)]
pub enum RetainedToolError {
    #[error("retained Tool preparation: {0}")]
    Invalid(&'static str),
    #[error(transparent)]
    Inputs(#[from] RetainedInputError),
    #[error(transparent)]
    Association(#[from] AssociationError),
    #[error(transparent)]
    Contract(#[from] ContractError),
}
fn invalid(reason: &'static str) -> RetainedToolError {
    RetainedToolError::Invalid(reason)
}

/// The selected usage, definition and descriptor stay together. The executable
/// implementation is deliberately absent: this is a prepared binding request,
/// not a registered callable, grant or approved task manifest.
pub struct RetainedToolBinding {
    tool: ComponentRevisionRef,
    tool_skill: ComponentRevisionRef,
    skill: ComponentRevisionRef,
    python: ComponentRevisionRef,
    capability_id: String,
    association: SkillAssociation,
    combination: Arc<[ComponentRevisionRef]>,
    tool_inputs: InputContract,
}
impl RetainedToolBinding {
    pub fn tool(&self) -> ComponentRevisionRef {
        self.tool
    }
    pub fn tool_skill(&self) -> ComponentRevisionRef {
        self.tool_skill
    }
    pub fn skill(&self) -> ComponentRevisionRef {
        self.skill
    }
    pub fn python(&self) -> ComponentRevisionRef {
        self.python
    }
    pub fn capability_id(&self) -> &str {
        &self.capability_id
    }
    pub fn association(&self) -> &SkillAssociation {
        &self.association
    }
    /// Complete selected dependency closure of this usage, including its Skill
    /// owner. Unrelated Recipe steps are not part of its combination approval.
    /// These references identify selection; trusted approval remains separate.
    pub fn combination(&self) -> &[ComponentRevisionRef] {
        &self.combination
    }

    /// Validate actual Python arguments against both the associated usage and
    /// retained primitive contract immediately before the kernel/implementation
    /// adapter. No source is interpreted, no effect performed or retry granted.
    pub fn bind_tool_arguments(
        &self,
        step_inputs: &Value,
        arguments: &Value,
    ) -> Result<Value, RetainedToolError> {
        self.association
            .validate_arguments(step_inputs, arguments)?;
        Ok(Value::Object(self.tool_inputs.bind(arguments)?))
    }
}

pub struct RetainedToolProgram {
    inputs: RetainedRecipeInputs,
    program: ComposedProgram,
    bindings: BTreeMap<String, RetainedToolBinding>,
}
impl RetainedToolProgram {
    pub fn inputs(&self) -> &RetainedRecipeInputs {
        &self.inputs
    }
    pub fn program(&self) -> &ComposedProgram {
        &self.program
    }
    /// Keyed by the following executable step, not the non-executing Rust step.
    pub fn bindings(&self) -> &BTreeMap<String, RetainedToolBinding> {
        &self.bindings
    }
}

/// New retention-document contract: ToolSkill.binding is exactly
/// {format:"tool-skill-binding/1",tool_uuid,callable,capability_id}; Tool documents
/// contain their capability_id, callable and recursive input_contract. This is
/// not implicit support in the legacy mutable insert/seeder APIs.
pub fn prepare_retained_tool_program(
    instruction: RetainedRecipeInstruction,
) -> Result<RetainedToolProgram, RetainedToolError> {
    let graph = instruction.snapshot().revisions();
    // Existing typed composition enforces adjacency and explicit binding
    // cardinality; it never interpolates runtime values or executes prose.
    let program = compose_typed_program(
        instruction.ordered(),
        &RetainedComponentResolver(&instruction),
    )
    .map_err(RetainedInputError::from)?;
    for step in &instruction.ordered().instruction().orchestrator_steps {
        let code = &graph[&step.include[0]];
        require_leaf_python(code).map_err(invalid)?;
    }
    let rust_steps: BTreeMap<_, _> = instruction
        .ordered()
        .instruction()
        .rust_steps
        .iter()
        .map(|s| (s.step_id.as_str(), s))
        .collect();
    let python_steps: BTreeMap<_, _> = instruction
        .ordered()
        .instruction()
        .orchestrator_steps
        .iter()
        .map(|s| (s.step_id.as_str(), s))
        .collect();
    // Index once; repeated uses of one Skill need neither a catalogue scan nor
    // a duplicate allocation of its complete transitive combination.
    let mut usages: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for revision in graph.values() {
        if let Some(a) = revision.draft().association() {
            usages
                .entry((
                    a.python_code_uuid(),
                    a.tool_skill_uuid(),
                    a.tool_uuid(),
                    a.callable(),
                ))
                .or_default()
                .push(revision);
        }
    }
    let mut combinations: BTreeMap<Uuid, Arc<[ComponentRevisionRef]>> = BTreeMap::new();
    let mut retained_references = 0usize;
    let mut bindings = BTreeMap::new();
    for pair in instruction.ordered().step_order().windows(2) {
        let Some(rust) = rust_steps.get(pair[0].as_str()) else {
            continue;
        };
        let python = python_steps
            .get(pair[1].as_str())
            .ok_or_else(|| invalid("binding has no following PythonCode"))?;
        let binding = &rust.tool_bindings[0]; // typed composer checked one
        // Legacy Ignore/Fallback carries silent-success/replay semantics.
        // Retry needs its durable effect/evidence adapter, not an implicit loop.
        if binding.error_policy != ErrorPolicy::Fail
            || !binding.params.as_object().is_some_and(|p| p.is_empty())
        {
            return Err(invalid(
                "legacy invocation parameters or error policy cannot prepare a v3 binding",
            ));
        }
        let descriptor = &graph[&rust.include[0]];
        if !descriptor.draft().dependencies().contains(&binding.tool_id) {
            return Err(invalid("ToolSkill must declare its Tool dependency"));
        }
        let tool = graph
            .get(&binding.tool_id)
            .filter(|tool| tool.reference().class_code == 0)
            .ok_or_else(|| invalid("retained Tool definition missing"))?;
        let raw = descriptor
            .draft()
            .document()
            .get("binding")
            .and_then(Value::as_object)
            .filter(|v| {
                v.len() == 4
                    && ["format", "tool_uuid", "callable", "capability_id"]
                        .iter()
                        .all(|key| v.contains_key(*key))
            })
            .ok_or_else(|| invalid("explicit retained ToolSkill binding required"))?;
        let callable = if binding.tool_name.starts_with("host.") {
            binding.tool_name.clone()
        } else {
            format!("host.{}", binding.tool_name)
        };
        let capability_id = tool
            .draft()
            .document()
            .get("capability_id")
            .and_then(Value::as_str)
            .filter(|id| !id.trim().is_empty())
            .ok_or_else(|| invalid("Tool capability identity required"))?;
        if raw["format"] != "tool-skill-binding/1"
            || raw["tool_uuid"]
                .as_str()
                .and_then(|s| uuid::Uuid::parse_str(s).ok())
                != Some(binding.tool_id)
            || raw["callable"] != callable
            || raw["capability_id"] != capability_id
            || tool
                .draft()
                .document()
                .get("callable")
                .and_then(Value::as_str)
                != Some(callable.as_str())
        {
            return Err(invalid("Tool/ToolSkill/Recipe identities disagree"));
        }
        let candidates = usages
            .get(&(
                python.include[0],
                rust.include[0],
                binding.tool_id,
                callable.as_str(),
            ))
            .ok_or_else(|| invalid("selected usage association missing"))?;
        if candidates.len() != 1 {
            return Err(invalid("selected usage association is ambiguous"));
        }
        let skill = candidates[0];
        let association = skill.draft().association().expect("filtered usage").clone();
        let code = &graph[&python.include[0]];
        let associated: Value = serde_json::from_str(association.exact_bytes())
            .map_err(|_| invalid("retained association integrity failed"))?;
        if code.draft().document().get("input_contract") != associated.get("inputs")
            || code.draft().document().get("result_contract") != associated.get("result")
        {
            return Err(invalid(
                "PythonCode contracts differ from its usage association",
            ));
        }
        let tool_inputs = InputContract::from_value(
            tool.draft()
                .document()
                .get("input_contract")
                .ok_or_else(|| invalid("retained Tool input contract required"))?,
            REVISION_LIMITS,
        )?;
        association.require_tool_input_contract(&tool_inputs)?;
        let combination = if let Some(retained) = combinations.get(&skill.reference().uuid) {
            retained.clone()
        } else {
            let mut reachable = BTreeSet::from([skill.reference().uuid]);
            let mut pending = vec![skill.reference().uuid];
            while let Some(id) = pending.pop() {
                let revision = graph
                    .get(&id)
                    .ok_or_else(|| invalid("usage dependency missing"))?;
                for dependency in revision.draft().dependencies() {
                    if reachable.insert(*dependency) {
                        pending.push(*dependency);
                    }
                }
            }
            retained_references += reachable.len();
            // Technical manifest capacity, independent of token budgets. Reject
            // the whole assembly; never omit dependency/approval references.
            if retained_references > 65_536 {
                return Err(invalid("usage combinations exceed manifest capacity"));
            }
            let retained: Arc<[ComponentRevisionRef]> = reachable
                .into_iter()
                .map(|id| graph[&id].reference())
                .collect::<Vec<_>>()
                .into();
            combinations.insert(skill.reference().uuid, retained.clone());
            retained
        };
        bindings.insert(
            python.step_id.clone(),
            RetainedToolBinding {
                tool: tool.reference(),
                tool_skill: descriptor.reference(),
                skill: skill.reference(),
                python: code.reference(),
                capability_id: capability_id.to_owned(),
                association,
                combination,
                tool_inputs,
            },
        );
    }
    if bindings.len() != instruction.ordered().instruction().rust_steps.len() {
        return Err(invalid("not every binding has a retained usage"));
    }
    // Do not expose path-based legacy dynamic load directives. The host resolves
    // these explicit exact bindings to retained implementation handles instead.
    let mut program = program;
    program.rust_directives.clear();
    let inputs = prepare_retained_inputs(instruction)?;
    Ok(RetainedToolProgram {
        inputs,
        program,
        bindings,
    })
}
