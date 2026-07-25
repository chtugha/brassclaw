//! Memory document system.
//!
//! - [`MemoryStore`] — project-scoped document CRUD
//! - [`RetrievalEngine`] — context building from project docs via keyword search
//! - [`SkillTracker`] — confidence tracking for auto-extracted skills
//! - [`RecipeMatcher`] — Recipe / ToolSkill match for tiered execution
//! - [`RecipeValidator`] — Step-1 structural validation
//! - [`ComponentValidator`] — Phase 3 generalized class-dispatch validator
//! - [`SimilarityChecker`] — pre-validation deduplication gate
//! - [`intent_system`] — unified intent resolution (§3.12, V028)

pub mod component_validator;
pub mod intent_system;
pub mod metric_outcome;
pub mod recipe_matcher;
pub mod recipe_validator;
pub mod retrieval;
pub mod retrieval_dbless;
pub mod retrieval_source;
pub mod similarity_checker;
pub mod skill_tracker;
pub mod store;

pub use component_validator::{ComponentPayload, ComponentValidator, GenericComponent, ValidationConfig};
pub use metric_outcome::MetricRecorder;
pub use recipe_matcher::{RecipeMatch, RecipeMatcher, RecipeStepMatch, ToolSkillMatch};
pub use recipe_validator::{RecipeValidator, ValidationResult};
pub use retrieval::RetrievalEngine;
pub use retrieval_source::{ComponentItem, ComponentScope, RamSource, RetrievalSource, RetrievalSourceError};
#[cfg(feature = "skills-db")]
pub use retrieval_source::PostgresSource;
pub use similarity_checker::{SimilarityChecker, SimilarityMatch};
pub use skill_tracker::SkillTracker;
pub use store::MemoryStore;
