//! Memory document system.
//!
//! - [`MemoryStore`] — project-scoped document CRUD
//! - [`RetrievalEngine`] — context building from project docs via keyword search
//! - [`SkillTracker`] — confidence tracking for auto-extracted skills
//! - [`RecipeMatcher`] — Recipe / ToolSkill match for tiered execution
//! - [`RecipeValidator`] — Step-1 structural validation
//! - [`SimilarityChecker`] — pre-validation deduplication gate

pub mod metric_outcome;
pub mod recipe_matcher;
pub mod recipe_validator;
pub mod retrieval;
pub mod similarity_checker;
pub mod skill_tracker;
pub mod store;

pub use metric_outcome::MetricRecorder;
pub use recipe_matcher::{RecipeMatch, RecipeMatcher, RecipeStepMatch, ToolSkillMatch};
pub use recipe_validator::{RecipeValidator, ValidationResult};
pub use retrieval::RetrievalEngine;
pub use similarity_checker::{SimilarityChecker, SimilarityMatch};
pub use skill_tracker::SkillTracker;
pub use store::MemoryStore;
