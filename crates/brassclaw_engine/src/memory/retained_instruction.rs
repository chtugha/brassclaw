//! IBS over exact retained Recipe bytes. This compiler does not select latest,
//! resolve intent, approve a component, load a Tool artifact or execute a step.
//! The production catalogue owner must resolve trusted activation/combination
//! evidence before accepting the result; draft validation can use it separately.

use std::{collections::BTreeSet, sync::Arc};

use brassclaw_skills::{
    association_contract::ComponentRevisionRef, component_revision::RetainedComponentSnapshot,
};
use serde_json::Value;
use uuid::Uuid;

use super::instruction_builder::{
    IbsError, OrderedBuildInstruction, StepDescriptionEntry, StepOwner, build_ordered_instruction,
};
use crate::types::recipe::RecipeVariant;

/// Supplied by reviewed workflow classification, not inferred from mutable
/// usage statistics or an LLM at task startup. It grants no model/Tool access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowClass {
    Deterministic,
    RequiresModel,
}

/// Keeps the exact embedded variant/layout with the complete retained graph.
/// BuildInstruction remains ephemeral; persistence retains these references,
/// never a separate instruction table. This is not an approval capability.
pub struct RetainedRecipeInstruction {
    snapshot: Arc<RetainedComponentSnapshot>,
    recipe: ComponentRevisionRef,
    variant: RecipeVariant,
    class: WorkflowClass,
    ordered: OrderedBuildInstruction,
}
impl RetainedRecipeInstruction {
    pub fn snapshot(&self) -> &RetainedComponentSnapshot {
        &self.snapshot
    }
    pub fn recipe(&self) -> ComponentRevisionRef {
        self.recipe
    }
    pub fn variant(&self) -> &RecipeVariant {
        &self.variant
    }
    pub fn class(&self) -> WorkflowClass {
        self.class
    }
    pub fn ordered(&self) -> &OrderedBuildInstruction {
        &self.ordered
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RetainedInstructionError {
    #[error("retained Recipe instruction: {0}")]
    Invalid(&'static str),
    #[error(transparent)]
    Ibs(#[from] IbsError),
}
fn invalid(reason: &'static str) -> RetainedInstructionError {
    RetainedInstructionError::Invalid(reason)
}
fn fields(value: &Value, allowed: &[&str]) -> Result<(), RetainedInstructionError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("structured object required"))?;
    if object.keys().any(|name| !allowed.contains(&name.as_str())) {
        return Err(invalid("unsupported workflow field"));
    }
    Ok(())
}

pub fn compile_retained_recipe(
    snapshot: Arc<RetainedComponentSnapshot>,
    recipe_uuid: Uuid,
    variant_key: &str,
    class: WorkflowClass,
) -> Result<RetainedRecipeInstruction, RetainedInstructionError> {
    if !snapshot.roots().contains(&recipe_uuid) {
        return Err(invalid("Recipe is not a selected workflow root"));
    }
    let retained = snapshot
        .revisions()
        .get(&recipe_uuid)
        .ok_or_else(|| invalid("Recipe revision missing"))?;
    let recipe = retained.reference();
    if recipe.class_code != 21 {
        return Err(invalid("workflow root must be a Recipe"));
    }
    let document = retained.draft().document();
    let variants = document
        .get("variants")
        .and_then(Value::as_array)
        .filter(|variants| !variants.is_empty())
        .ok_or_else(|| invalid("Recipe variants required"))?;
    let mut names = BTreeSet::new();
    let mut selected = None;
    for raw in variants {
        fields(
            raw,
            &[
                "variant_key",
                "description",
                "step_link",
                "intent_examples",
                "variable_patterns",
            ],
        )?;
        let variant: RecipeVariant =
            serde_json::from_value(raw.clone()).map_err(|_| invalid("malformed Recipe variant"))?;
        if variant.variant_key.trim().is_empty() || !names.insert(variant.variant_key.clone()) {
            return Err(invalid("unique nonempty variant identity required"));
        }
        if let Some(patterns) = raw.get("variable_patterns").and_then(Value::as_array) {
            for pattern in patterns {
                fields(pattern, &["name", "pattern", "description"])?;
            }
        }
        if variant.variant_key == variant_key {
            selected = Some(variant);
        }
    }
    let variant =
        selected.ok_or_else(|| invalid("selected variant does not belong to Recipe revision"))?;
    let link = variant
        .step_link
        .as_deref()
        .filter(|link| !link.trim().is_empty())
        .ok_or_else(|| invalid("selected variant has no step_link"))?;
    let raw_descriptions = document
        .get("step_descriptions")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("structured step_descriptions required"))?;
    for raw in raw_descriptions {
        fields(raw, &["desc_idx", "label", "yaml_source", "steps"])?;
        if let Some(steps) = raw.get("steps").and_then(Value::as_array) {
            for step in steps {
                fields(
                    step,
                    &[
                        "stepnumber",
                        "knowledge",
                        "goal",
                        "content",
                        "type",
                        "info",
                        "include",
                        "tool_bindings",
                        "dependencies",
                    ],
                )?;
                if let Some(bindings) = step.get("tool_bindings").and_then(Value::as_array) {
                    for binding in bindings {
                        fields(binding, &["tool_id", "tool_name", "params", "error_policy"])?;
                        if let Some(policy) = binding.get("error_policy") {
                            fields(policy, &["policy", "max_attempts", "step_id"])?;
                        }
                    }
                }
            }
        }
    }
    let descriptions: Vec<StepDescriptionEntry> =
        serde_json::from_value(Value::Array(raw_descriptions.clone()))
            .map_err(|_| invalid("malformed step_descriptions"))?;
    let ordered = build_ordered_instruction(
        link,
        &descriptions,
        &variant.variable_patterns,
        class == WorkflowClass::RequiresModel,
    )?;
    if ordered.step_order().is_empty() {
        return Err(invalid("empty selected workflow"));
    }
    // Resolve class/identity from this retained graph, not mutable tables. Binding
    // preparation and immutable Tool implementation verification remain separate.
    for step in ordered
        .instruction()
        .rust_steps
        .iter()
        .chain(&ordered.instruction().orchestrator_steps)
    {
        if step.knowledge == StepOwner::Both {
            return Err(invalid("separate binding and executable steps required"));
        }
        let id = step.include[0]; // strict IBS established exactly one component
        if !retained.draft().dependencies().contains(&id) {
            return Err(invalid(
                "selected component missing from Recipe dependency declaration",
            ));
        }
        let component = snapshot
            .revisions()
            .get(&id)
            .ok_or_else(|| invalid("selected component revision missing"))?;
        let expected_class = if step.knowledge == StepOwner::Rust {
            13
        } else {
            22
        };
        if component.reference().class_code != expected_class {
            return Err(invalid("component class does not match execution channel"));
        }
    }
    Ok(RetainedRecipeInstruction {
        snapshot,
        recipe,
        variant,
        class,
        ordered,
    })
}
