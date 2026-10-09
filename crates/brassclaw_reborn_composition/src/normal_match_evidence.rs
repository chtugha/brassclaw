//! Observations from committed normal intent matching, not activation evidence.
//! Construction stays inside composition; no deserializer or client success flag.
use brassclaw_engine::memory::{
    intent_system::IntentResolution, retained_instruction::RetainedRecipeInstruction,
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Clone, Serialize)]
pub(crate) struct NormalMatchEvidence {
    format: &'static str,
    catalogue_generation: Uuid,
    command_checksum: String,
    matched_template_checksum: String,
    task_inputs_checksum: String,
    recipe_uuid: Uuid,
    recipe_version: u64,
    recipe_checksum: String,
    variant_key: String,
    step_link: String,
    selection_checksum: String,
}

impl NormalMatchEvidence {
    /// Called only after the real matcher/IBS transaction commits. The selected
    /// complete graph and typed inputs have already been checked by its owner.
    /// Named child lookup must never use this constructor to invent a match.
    pub(crate) fn from_committed_match(
        generation: Uuid,
        query: &str,
        matched: &IntentResolution,
        instruction: &RetainedRecipeInstruction,
        inputs: &Value,
    ) -> Option<Self> {
        let IntentResolution::Match {
            component_id,
            component_class_code,
            step_link,
            input_text,
            ..
        } = matched
        else {
            return None;
        };
        let recipe = instruction.recipe();
        if generation.is_nil()
            || *component_id != recipe.uuid
            || *component_class_code != 21
            || step_link != &instruction.variant().step_link
            || !instruction.variant().intent_examples.contains(input_text)
        {
            return None;
        }
        let selection = instruction.retained_selection().ok()?;
        Some(Self {
            format: "monty-normal-match/1",
            catalogue_generation: generation,
            command_checksum: hex::encode(Sha256::digest(query.as_bytes())),
            matched_template_checksum: hex::encode(Sha256::digest(input_text.as_bytes())),
            task_inputs_checksum: hex::encode(Sha256::digest(inputs.to_string().as_bytes())),
            recipe_uuid: recipe.uuid,
            recipe_version: recipe.version,
            recipe_checksum: hex::encode(recipe.checksum),
            variant_key: instruction.variant().variant_key.clone(),
            step_link: step_link.clone()?,
            selection_checksum: hex::encode(selection.checksum()),
        })
    }
}
