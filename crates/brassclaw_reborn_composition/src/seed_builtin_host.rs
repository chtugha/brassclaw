//! Phase C.2 — idempotent boot seed of the built-in `host.*` component stack
//! (the Step 27 spec in `builtin_stuff_v3.md`).
//!
//! Runs on every service start (wired in `webui.rs` alongside
//! [`crate::webui::seed_builtin_providers`]). Idempotent: a re-seed leaves
//! existing rows untouched and only inserts what is missing.
//!
//! # Slice 1d scope (this file)
//!
//! The skeleton + the class-23 `builtin-host` ExtensionCatalogue row only.
//! `child_component_ids` starts empty and is appended to incrementally across
//! slices 2–12 as the `host.*` tool / ToolSkill / PythonCode / leaf-Skill /
//! Recipe ids are minted.
//!
//! # Marker scope
//!
//! Seeded builtins use a fixed marker scope
//! `(tenant_id = runtime tenant, user_id = SYSTEM_RESERVED_ID,
//! agent_id = "default", project_id = "system")`. The retrieval UNION (slice
//! 1b) is tenant-anchored + `source = 'system'` agnostic on
//! user/agent/project, so this scope is just the row's stable storage key
//! across re-seeds — it does NOT gate visibility.
//!
//! # Feature gate
//!
//! Compiles behind the `postgres` feature (mirrors the `pg_*` stores).

// Built out incrementally across slices 2–12; the insert/lookup surface is
// exercised by the boot wiring in `webui.rs`. Mirrors the `pg_*` store
// allow(dead_code) pattern.
#![allow(dead_code)]
#![forbid(unsafe_code)]

use std::sync::Arc;

use brassclaw_host_api::SYSTEM_RESERVED_ID;
use brassclaw_pg::PgPool;
use thiserror::Error;

use crate::pg_extension_catalogue_store::{NewPgExtensionCatalogue, PgExtensionCatalogueStore};

/// Marker `user_id` for seeded builtins — the system sentinel.
const SEED_USER: &str = SYSTEM_RESERVED_ID;
/// Marker `agent_id` for seeded builtins — matches `build_component_scope`'s
/// agent fallback (`"default"`) so the seed key aligns with the live scope.
const SEED_AGENT: &str = "default";
/// Marker `project_id` for seeded builtins.
const SEED_PROJECT: &str = "system";

/// The class-23 ExtensionCatalogue row name that owns the `host.*` component
/// stack (Step 27.11).
const BUILTIN_HOST_CATALOGUE_NAME: &str = "builtin-host";

/// Errors raised by the builtin-host seed.
#[derive(Debug, Error)]
pub(crate) enum SeedBuiltinHostError {
    #[error("pool error: {reason}")]
    Pool { reason: String },
    #[error("database error: {reason}")]
    Db { reason: String },
}

/// Seed the built-in `host.*` component stack for `tenant_id`.
///
/// Slice 1d: inserts the class-23 `builtin-host` ExtensionCatalogue row with
/// empty `child_component_ids` (filled in slices 2–12), as
/// `source = "system"` + `validation_status = "validated"` (bypassing the Q1
/// pending queue). Idempotent: a no-op if the catalogue row already exists.
pub(crate) async fn seed_builtin_host_components(
    pool: Arc<PgPool>,
    tenant_id: &str,
) -> Result<(), SeedBuiltinHostError> {
    let cat_store = PgExtensionCatalogueStore::new(pool);

    // Idempotent: skip if the `builtin-host` catalogue row already exists.
    if cat_store
        .get_by_name(
            tenant_id,
            SEED_USER,
            SEED_AGENT,
            SEED_PROJECT,
            BUILTIN_HOST_CATALOGUE_NAME,
        )
        .await
        .map_err(|e| SeedBuiltinHostError::Db {
            reason: e.to_string(),
        })?
        .is_some()
    {
        tracing::debug!("builtin-host catalogue already seeded; skipping");
        return Ok(());
    }

    // Insert the class-23 `builtin-host` catalogue row. `child_component_ids`
    // starts empty; slices 2–12 append the minted host.* component ids.
    // No `05:validator` consumer tag — builtins skip Q1 and graduate directly
    // to `validated`, so the SEC-01 delivery filter surfaces them immediately.
    let row = NewPgExtensionCatalogue {
        tenant_id: tenant_id.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: BUILTIN_HOST_CATALOGUE_NAME.to_string(),
        description: "Built-in host.* capability stack (Step 27).".into(),
        version: "1.0".into(),
        overview_doc: "Container for the built-in host.* Tools, ToolSkills, \
                       PythonCode formatters, leaf Skills, and Recipes that \
                       back the Orchestrator↔Executioner surface."
            .into(),
        task_groups: serde_json::json!([]),
        child_component_ids: Vec::new(),
        intent_index: None,
        prior_knowledge_content: None,
        override_prompt_creation: false,
        consumer_tags: vec!["02:orchestrator".into()],
        intent_examples: None,
        source: "system".into(),
        dependency_registry: None,
    };
    let id = cat_store
        .insert(row)
        .await
        .map_err(|e| SeedBuiltinHostError::Db {
            reason: e.to_string(),
        })?;

    // Bypass Q1 pending: graduate the builtin to `validated` directly.
    cat_store
        .update_validation_status(
            tenant_id,
            SEED_USER,
            SEED_AGENT,
            SEED_PROJECT,
            id,
            "validated",
        )
        .await
        .map_err(|e| SeedBuiltinHostError::Db {
            reason: e.to_string(),
        })?;

    tracing::debug!(catalogue_id = %id, "seeded builtin-host catalogue");
    Ok(())
}
