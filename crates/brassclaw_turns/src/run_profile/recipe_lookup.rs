//! Recipe-Skill-Tool lookup port.
//!
//! Defined at the loop layer (`brassclaw_turns`) so that:
//! 1. `brassclaw_agent_loop::executor::recipe_stage` can compose it through
//!    the existing `AgentLoopDriverHost` without `brassclaw_agent_loop`
//!    gaining a direct dependency on `brassclaw_engine` types.
//! 2. The composition layer (`brassclaw_reborn_composition`) provides
//!    the implementation backed by the v2 `MemoryDoc`/`Store` graph.
//!
//! The trait surface is deliberately narrow — only the four methods the
//! recipe pipeline actually calls. Everything else (Wilson recomputation,
//! tier classification, content storage) lives in the implementation.

use std::fmt;

use async_trait::async_trait;

/// Lightweight DTO for a Recipe step — `RecipeStage` doesn't need the
/// full schema; it only invokes the step's `tool` with `params`.
#[derive(Debug, Clone)]
pub struct RecipeStepDto {
    pub skill_name: String,
    pub tool: String,
    pub params: serde_json::Value,
    pub description: String,
}

/// Best-match Recipe as surfaced to the agent loop.
///
/// `tier0_eligible` is pre-computed by the implementation so the hot
/// path doesn't repeat the Wilson + tier + validation check.
#[derive(Debug, Clone)]
pub struct RecipeMatchDto {
    pub id: String,
    pub name: String,
    pub tier: String,
    pub wilson_lower: f64,
    pub tier0_eligible: bool,
    pub validation_kind: String,
    pub steps: Vec<RecipeStepDto>,
    pub match_score: f64,
}

/// Compact ToolSkill entry for Tier 1 prompt injection.
///
/// `estimated_tokens` lets the agent loop budget the injection against
/// the conversation-history budget already wired by Phase 1.
#[derive(Debug, Clone)]
pub struct ToolSkillMatchDto {
    pub id: String,
    pub name: String,
    pub tool_name: String,
    pub description: String,
    pub param_template: serde_json::Value,
    pub preconditions: String,
    pub estimated_tokens: usize,
}

/// Async errors raised by lookups. Callers should treat every variant
/// as fatal — the lookup returns `None` on a soft miss.
#[derive(Debug)]
pub enum RecipeLookupError {
    StoreUnavailable(String),
    Decode(String),
    Backend(String),
}

impl fmt::Display for RecipeLookupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StoreUnavailable(reason) => write!(f, "recipe store unavailable: {reason}"),
            Self::Decode(reason) => write!(f, "recipe decode error: {reason}"),
            Self::Backend(reason) => write!(f, "recipe backend error: {reason}"),
        }
    }
}

impl std::error::Error for RecipeLookupError {}

/// Lookup contract for the Recipe-Skill-Tool pipeline.
///
/// All methods are `async` — the backing store is typically a DB and must
/// not be driven via `block_on()` inside a running Tokio runtime (deadlock
/// risk on single-threaded or work-stealing executors).
#[async_trait]
pub trait RecipeLookup: Send + Sync {
    /// Best matching validated Recipe for `user_input`, or `None` if no
    /// trigger fires above the matcher's threshold.
    async fn find_recipe(
        &self,
        user_input: &str,
    ) -> Result<Option<RecipeMatchDto>, RecipeLookupError>;

    /// Compact ToolSkill entries for Tier 1 prompt injection. Already
    /// ranked by the matcher; caller takes only as many as its budget allows.
    async fn find_skills(
        &self,
        user_input: &str,
    ) -> Result<Vec<ToolSkillMatchDto>, RecipeLookupError>;

    /// Atomically record an outcome on a Recipe — implementation MUST
    /// be a single SQL transaction so concurrent updates don't race the
    /// Wilson recomputation.
    async fn record_recipe_outcome(
        &self,
        recipe_id: &str,
        success: bool,
    ) -> Result<(), RecipeLookupError>;

    /// Same atomicity requirement as `record_recipe_outcome`.
    async fn record_skill_outcome(
        &self,
        skill_id: &str,
        success: bool,
    ) -> Result<(), RecipeLookupError>;
}
