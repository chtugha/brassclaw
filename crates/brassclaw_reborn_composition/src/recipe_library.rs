//! Recipe-Skill-Tool library adapter (Phase 7).
//!
//! Bridges between the persisted Recipe / ToolSkill MemoryDocs and the
//! `brassclaw_turns::run_profile::RecipeLookup` trait that the agent
//! loop's `RecipeStage` consults. The agent loop is intentionally
//! ignorant of how the library is persisted — the trait boundary
//! keeps `brassclaw_agent_loop` free of `brassclaw_engine` deps.
//!
//! Tier 0 selection (direct execution, no LLM round-trip) is deliberately
//! not implemented in this adapter — Phase 7 ships the lookup + planner
//! surface; Tier 0 dispatch arrives when the executor exposes a
//! `CapabilityCall`-shaped payload channel. Until then, looking up a
//! recipe only causes the prompt stage to inject the matched ToolSkill
//! summaries (Tier 1) — a soft signal that the LLM can choose to follow.

use async_trait::async_trait;
use brassclaw_turns::run_profile::{
    RecipeLookup, RecipeLookupError, RecipeMatchDto, ToolSkillMatchDto,
};

#[cfg(feature = "postgres")]
use std::sync::Arc;

#[cfg(feature = "postgres")]
use brassclaw_engine::memory::metric_outcome::MetricRecorder;
#[cfg(feature = "postgres")]
use brassclaw_engine::memory::recipe_matcher::{
    RECIPE_MIN_MATCH, RecipeMatch as EngineRecipeMatch, RecipeMatcher, ToolSkillMatch,
};
#[cfg(feature = "postgres")]
use brassclaw_engine::traits::store::Store;
#[cfg(feature = "postgres")]
use brassclaw_engine::types::project::ProjectId;
#[cfg(feature = "postgres")]
use brassclaw_turns::run_profile::RecipeStepDto;
#[cfg(feature = "postgres")]
use tracing::debug;

/// Recipe library backed by the engine `Store`.
///
/// Reads `DocType::Recipe` and `DocType::ToolSkill` memory docs from
/// `list_memory_docs_with_shared` (so admin-installed recipes are visible
/// alongside user-authored ones), filters to `ValidationStatus::Validated`
/// components via the engine's `RecipeMatcher`, and surfaces the result
/// as loop-layer DTOs.
///
/// Cheap to clone — the inner `Store` is `Arc`-shared with the rest of
/// the composition surface.
#[cfg(feature = "postgres")]
#[derive(Clone)]
pub(crate) struct RecipeLibrary {
    store: Arc<dyn Store>,
    recorder: MetricRecorder,
}

#[cfg(feature = "postgres")]
impl std::fmt::Debug for RecipeLibrary {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RecipeLibrary")
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "postgres")]
impl RecipeLibrary {
    pub(crate) fn new(store: Arc<dyn Store>) -> Self {
        let recorder = MetricRecorder::new(Arc::clone(&store));
        Self { store, recorder }
    }

    fn matcher(&self) -> RecipeMatcher {
        RecipeMatcher::new(Arc::clone(&self.store))
    }

    fn default_project_scope() -> ProjectId {
        ProjectId::from_slug("default", "local")
    }

    fn above_threshold(score: f64) -> bool {
        score >= RECIPE_MIN_MATCH
    }
}

#[cfg(feature = "postgres")]
#[async_trait]
impl RecipeLookup for RecipeLibrary {
    async fn find_recipe(
        &self,
        user_input: &str,
    ) -> Result<Option<RecipeMatchDto>, RecipeLookupError> {
        // Default-scope lookup: composition sites that need scoped lookups
        // wrap `RecipeLookup` themselves and forward to this adapter only
        // after scoping. The single-tenant v2 design treats `local` /
        // `default` as the canonical scopes.
        let project_id = Self::default_project_scope();
        let result = self
            .matcher()
            .find_recipe(project_id, "default", user_input)
            .await
            .map_err(|e| RecipeLookupError::Backend(e.to_string()))?;
        Ok(match result {
            Some((hit, score)) if Self::above_threshold(score) => Some(to_dto(hit, score)),
            _ => None,
        })
    }

    async fn find_skills(
        &self,
        user_input: &str,
    ) -> Result<Vec<ToolSkillMatchDto>, RecipeLookupError> {
        let project_id = Self::default_project_scope();
        let ranked = self
            .matcher()
            .find_skills(project_id, "default", user_input)
            .await
            .map_err(|e| RecipeLookupError::Backend(e.to_string()))?;
        Ok(ranked.into_iter().map(skill_to_dto).collect())
    }

    async fn record_recipe_outcome(
        &self,
        recipe_id: &str,
        success: bool,
    ) -> Result<(), RecipeLookupError> {
        let project_id = Self::default_project_scope();
        self.recorder
            .record_recipe(project_id, "default", recipe_id, success)
            .await
            .map_err(|e| RecipeLookupError::Backend(e.to_string()))?;
        debug!(
            recipe_id,
            success, "recipe_library: recipe outcome recorded"
        );
        Ok(())
    }

    async fn record_skill_outcome(
        &self,
        skill_id: &str,
        success: bool,
    ) -> Result<(), RecipeLookupError> {
        let project_id = Self::default_project_scope();
        self.recorder
            .record_tool_skill(project_id, "default", skill_id, success)
            .await
            .map_err(|e| RecipeLookupError::Backend(e.to_string()))?;
        debug!(skill_id, success, "recipe_library: skill outcome recorded");
        Ok(())
    }
}

#[cfg(feature = "postgres")]
fn to_dto(hit: EngineRecipeMatch, match_score: f64) -> RecipeMatchDto {
    RecipeMatchDto {
        id: hit.id,
        name: hit.name,
        tier: hit.tier,
        wilson_lower: hit.wilson_lower,
        tier0_eligible: hit.tier0_eligible,
        validation_kind: hit.validation_kind,
        steps: hit
            .steps
            .into_iter()
            .map(|step| RecipeStepDto {
                skill_name: step.skill_name,
                tool: step.tool,
                params: step.params,
                description: step.description,
            })
            .collect(),
        match_score,
    }
}

#[cfg(feature = "postgres")]
fn skill_to_dto(skill: ToolSkillMatch) -> ToolSkillMatchDto {
    ToolSkillMatchDto {
        id: skill.id,
        name: skill.name,
        tool_name: skill.tool_name,
        description: skill.description,
        param_template: skill.param_template,
        preconditions: skill.preconditions,
        estimated_tokens: skill.estimated_tokens,
    }
}

/// Placeholder for compositions that do not (yet) wire the persistent
/// library. Always returns "no match" / empty skills. Recording outcomes
/// is a no-op so test hosts can wire this in without wiring the store.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(crate) struct DisabledRecipeLookup;

#[async_trait]
impl RecipeLookup for DisabledRecipeLookup {
    async fn find_recipe(
        &self,
        _user_input: &str,
    ) -> Result<Option<RecipeMatchDto>, RecipeLookupError> {
        Ok(None)
    }

    async fn find_skills(
        &self,
        _user_input: &str,
    ) -> Result<Vec<ToolSkillMatchDto>, RecipeLookupError> {
        Ok(Vec::new())
    }

    async fn record_recipe_outcome(
        &self,
        _recipe_id: &str,
        _success: bool,
    ) -> Result<(), RecipeLookupError> {
        Ok(())
    }

    async fn record_skill_outcome(
        &self,
        _skill_id: &str,
        _success: bool,
    ) -> Result<(), RecipeLookupError> {
        Ok(())
    }
}
