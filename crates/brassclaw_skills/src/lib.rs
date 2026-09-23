// Allow deprecated DocType during the migration to class-code component tables.
// v2.rs uses DocType as a bridge type; retained until the v2 module is retired.
#![allow(deprecated)]
//! Skill component types, validation, and DB-backed storage for BrassClaw.
//!
//! The v1 SKILL.md subsystem (filesystem install/remove, host-FS registry
//! discovery, and the remote catalog) has been removed. Skills are v3
//! components (class codes 1/2/3) stored in `reborn_skills` and injected into
//! the base prompt by the runtime.
//!
//! - **`types`** + **`v2`** — Data structures (`SkillManifest`, `V2SkillMetadata`, etc.)
//! - **`validation`** — Name/content escaping, credential spec validation
//! - **`db_store`** — `reborn_skills` reader/writer (feature `db-store`)
//! - **`component_type`** — Class-code component taxonomy
pub mod component_type;
#[cfg(feature = "db-store")]
pub mod db_store;
pub mod types;
pub mod v2;
pub mod validation;

// Re-export core types at crate root for convenience.
pub use component_type::{ComponentType, ComponentTypeSet};
pub use types::{
    ActivationCriteria, GatingRequirements, LoadedSkill, MAX_PROMPT_FILE_SIZE,
    ProviderRefreshStrategy, SkillCredentialLocation, SkillCredentialSpec, SkillManifest,
    SkillOAuthConfig, SkillSource,
};

pub use validation::{
    SafeRelativePathError, escape_skill_content, escape_xml_attr, normalize_line_endings,
    normalize_safe_relative_path, validate_credential_name, validate_credential_spec,
    validate_path_pattern, validate_skill_name,
};
