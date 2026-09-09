//! Memory document system.
//!
//! - [`MemoryStore`] — project-scoped document CRUD
//! - [`SkillTracker`] — confidence tracking for auto-extracted skills
//! - [`RecipeMatcher`] — Recipe / ToolSkill match for tiered execution
//! - [`RecipeValidator`] — Step-1 structural validation
//! - [`SimilarityChecker`] — pre-validation deduplication gate
//! - [`intent_system`] — unified intent resolution (§3.12, V028)
//!
//! `ComponentValidator` was retired in Phase N (§0.23.2): Q1 is now orchestrated
//! via `brassclaw_reborn_composition::q1_orchestrator::run_q1_validation`.

pub mod composition;
pub mod instruction_builder;
pub mod intent_system;
pub mod metric_outcome;
pub mod recipe_matcher;
pub mod recipe_validator;
pub mod retrieval_source;
pub mod similarity_checker;
pub mod skill_tracker;
pub mod store;
pub mod template_extractor;

pub use composition::{
    ComponentResolver, ComposedProgram, ComposedStep, ResolvedComponent, RustDirective, SkillRef,
};
pub use metric_outcome::MetricRecorder;
pub use recipe_matcher::{RecipeMatch, RecipeMatcher, RecipeStepMatch, ToolSkillMatch};
pub use recipe_validator::{RecipeValidator, ValidationResult};
pub use retrieval_source::{
    ComponentItem, ComponentScope, FetchForTurnResult, RetrievalSource,
    RetrievalSourceError, TurnRoutingSignals,
};
#[cfg(feature = "skills-db")]
pub use retrieval_source::{
    DependencyEntry, PostgresSource, lookup_component_class, resolve_dependencies,
};
pub use similarity_checker::{SimilarityChecker, SimilarityMatch};
pub use skill_tracker::SkillTracker;
pub use store::MemoryStore;
