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
//!
//! # Feature gates
//!
//! - `v1-types` — gates `types`, `component_type`, and their re-exports.
//!   These belong to the v1 SKILL.md filesystem subsystem; only enable for
//!   the v1→v3 migration importer and legacy SKILL.md parsing paths.
//! - `v2-compat` — gates `v2` and its re-exports.
//!   Bridge types for the MemoryDoc-backed v2 skill_tracker path; do not add
//!   new consumers.
//! - `db-store` — gates `db_store` and its re-exports.
//!   The `reborn_skills` CRUD store; required when the engine uses the DB
//!   fast path (`skills-db` feature on `brassclaw_engine`).

/// v3 content escaping and name/credential validation — always compiled.
pub mod validation;

/// `reborn_skills` DB reader/writer — gated on `db-store` feature.
#[cfg(feature = "db-store")]
pub mod db_store;

// ── v1-types: SKILL.md filesystem-era types ──────────────────────────────────
// Gated so that crates that do not need v1 parsing cannot accidentally pull in
// keyword-scoring, setup-marker, and credential-spec infrastructure.

/// v1 SKILL.md manifest, activation criteria, credential specs, and loaded-skill
/// types.  Gated on `v1-types`; do not add new consumers outside the migration
/// importer.
#[cfg(feature = "v1-types")]
pub mod types;

/// v2-era role-intersection component-type dispatch (ComponentType /
/// ComponentTypeSet).  Superseded in v3 by `consumer_tags`; gated on
/// `v1-types` because it is only needed when parsing SKILL.md manifests.
#[cfg(feature = "v1-types")]
pub mod component_type;

// ── v2-compat: MemoryDoc-backed v2 skill metadata ────────────────────────────
// Bridge types used exclusively by brassclaw_engine::memory::skill_tracker.
// Do not add new consumers; migrate telemetry to reborn_skills columns instead.

/// v2 MemoryDoc skill metadata bridge types.  Gated on `v2-compat`; do not
/// add new consumers.
#[cfg(feature = "v2-compat")]
pub mod v2;

// ── Re-exports (v3 core — always available) ───────────────────────────────────

pub use validation::{
    SafeRelativePathError, escape_skill_content, escape_xml_attr, normalize_line_endings,
    normalize_safe_relative_path, validate_credential_name, validate_credential_spec,
    validate_path_pattern, validate_skill_name,
};

// ── Re-exports (v1-types feature) ─────────────────────────────────────────────

#[cfg(feature = "v1-types")]
pub use component_type::{ComponentType, ComponentTypeSet};

#[cfg(feature = "v1-types")]
pub use types::{
    ActivationCriteria, GatingRequirements, LoadedSkill, MAX_PROMPT_FILE_SIZE,
    ProviderRefreshStrategy, SkillCredentialLocation, SkillCredentialSpec, SkillManifest,
    SkillOAuthConfig, SkillSource,
};
