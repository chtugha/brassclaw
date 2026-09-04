//! Builtin Tool Bootstrap Seeder (Phase L).
//!
//! Seeds the full v3 component stack for the 23 first-party builtin tools at
//! first composition boot. This is a separate concern from
//! [`crate::seed_builtin_host`], which seeds the Step 27 `host.*` orchestrator
//! infrastructure. Phase L seeds the 23 first-party *capability* tools
//! (read_file, write_file, shell, …) and their ToolSkill / PythonCode / leaf
//! Skill / Domain Skill / Recipe / ExtensionCatalogue stacks.
//!
//! # Content source
//!
//! Every component body is transcribed from `builtin_stuff_v3.md` (Steps 1–26 +
//! Final section) into inline `const &str` / `serde_json::json!{}` literals —
//! no `include_str!()`, no on-disk files. The DB row is the live record from
//! first boot on.
//!
//! # Dispatch
//!
//! All PythonCode bodies dispatch via first-class `host.<tool>(kwarg=value)`
//! callables (the `__execute_action__` meta-primitive is retired). Slot
//! substitution uses `{{vars.slotN}}`.
//!
//! # Marker scope + idempotency
//!
//! Seeded builtins use the same marker scope as
//! [`crate::seed_builtin_host`]: `(tenant_id = runtime tenant,
//! user_id = SYSTEM_RESERVED_ID, agent_id = "default", project_id = "system")`
//! with `source = "system"` + `validation_status = "validated"` (bypassing Q1).
//! Every insert is idempotent (ON-CONFLICT / get-then-insert) and
//! `append_child_component_ids` deduplicates, so the whole seeder is safe to
//! call on every composition boot.
//!
//! # Feature gate
//!
//! Compiles behind the `postgres` feature (mirrors the `pg_*` stores). Built out
//! incrementally per domain group; the insert/lookup surface is exercised by
//! the boot wiring in `webui.rs`.
#![allow(dead_code)]
#![forbid(unsafe_code)]

use std::sync::Arc;

use brassclaw_host_api::SYSTEM_RESERVED_ID;
use brassclaw_pg::PgPool;
use serde_json::{json, Value};
use thiserror::Error;
use uuid::Uuid;

use crate::pg_extension_catalogue_store::{
    NewPgExtensionCatalogue, PgExtensionCatalogueStore,
};
use crate::pg_python_code_store::{NewPgPythonCode, PgPythonCodeStore};
use crate::pg_recipe_store::{NewPgRecipe, PgRecipeStore};
use crate::pg_skill_store::{NewPgSkill, PgSkillStore};
use crate::pg_tool_skill_store::{NewPgToolSkill, PgToolSkillStore};
use crate::pg_tool_store::{NewPgTool, PgToolStore};

/// Marker `user_id` for seeded builtins — the system sentinel.
const SEED_USER: &str = SYSTEM_RESERVED_ID;
/// Marker `agent_id` for seeded builtins — matches `build_component_scope`'s
/// agent fallback (`"default"`) so the seed key aligns with the live scope.
const SEED_AGENT: &str = "default";
/// Marker `project_id` for seeded builtins.
const SEED_PROJECT: &str = "system";

/// Errors raised by the builtin bootstrap seed.
#[derive(Debug, Error)]
pub enum SeedBuiltinBootstrapError {
    #[error("pool error: {reason}")]
    Pool { reason: String },
    #[error("database error: {reason}")]
    Db { reason: String },
}

/// The five seed-side stores + the catalogue store, all sharing one pool.
/// Built once per [`seed_builtin_components`] call; each group's seeder helper
/// borrows it. Mirrors [`crate::seed_builtin_host::HostStores`] (kept separate
/// to avoid touching the in-flight host seeder).
struct BootstrapStores {
    tenant: String,
    /// Held beside the per-table stores so the seeder can run targeted
    /// post-insert UPDATEs on columns the insert structs do not expose —
    /// notably `reborn_recipes.tier` / `wilson_lower` for the Tier-0 gate
    /// (Q2 decision A: composition-only post-insert UPDATE).
    pool: Arc<PgPool>,
    tool: PgToolStore,
    tool_skill: PgToolSkillStore,
    python_code: PgPythonCodeStore,
    skill: PgSkillStore,
    recipe: PgRecipeStore,
    catalogue: PgExtensionCatalogueStore,
}

impl BootstrapStores {
    fn new(pool: Arc<PgPool>, tenant_id: &str) -> Self {
        Self {
            tenant: tenant_id.to_string(),
            tool: PgToolStore::new(pool.clone()),
            tool_skill: PgToolSkillStore::new(pool.clone()),
            python_code: PgPythonCodeStore::new(pool.clone()),
            skill: PgSkillStore::new(pool.clone()),
            recipe: PgRecipeStore::new(pool.clone()),
            pool: pool.clone(),
            catalogue: PgExtensionCatalogueStore::new(pool),
        }
    }

    /// Insert-or-recover a Tool id (class 0). `insert` is ON-CONFLICT, so
    /// `None` is resolved via [`PgToolStore::get_id_by_name`].
    async fn upsert_tool(
        &self,
        row: NewPgTool,
        name: &str,
    ) -> Result<Uuid, SeedBuiltinBootstrapError> {
        let map = |e: crate::pg_tool_store::PgToolStoreError| SeedBuiltinBootstrapError::Db {
            reason: e.to_string(),
        };
        if let Some(id) = self.tool.insert(row).await.map_err(map)? {
            return Ok(id);
        }
        self.tool
            .get_id_by_name(&self.tenant, SEED_USER, SEED_AGENT, SEED_PROJECT, name)
            .await
            .map_err(map)?
            .ok_or_else(|| SeedBuiltinBootstrapError::Db {
                reason: format!("tool `{name}` insert no-op but not found"),
            })
    }

    /// Insert-or-recover a ToolSkill id (class 13). Same ON-CONFLICT pattern.
    async fn upsert_tool_skill(
        &self,
        row: NewPgToolSkill,
        name: &str,
    ) -> Result<Uuid, SeedBuiltinBootstrapError> {
        let map = |e: crate::pg_tool_skill_store::PgToolSkillStoreError| {
            SeedBuiltinBootstrapError::Db { reason: e.to_string() }
        };
        if let Some(id) = self.tool_skill.insert(row).await.map_err(map)? {
            return Ok(id);
        }
        self.tool_skill
            .get_id_by_name(&self.tenant, SEED_USER, SEED_AGENT, SEED_PROJECT, name)
            .await
            .map_err(map)?
            .ok_or_else(|| SeedBuiltinBootstrapError::Db {
                reason: format!("tool_skill `{name}` insert no-op but not found"),
            })
    }

    /// Insert-or-recover a leaf Skill id (class 1). Same ON-CONFLICT pattern.
    async fn upsert_skill(
        &self,
        row: NewPgSkill,
        name: &str,
    ) -> Result<Uuid, SeedBuiltinBootstrapError> {
        let map = |e: crate::pg_skill_store::PgSkillStoreError| SeedBuiltinBootstrapError::Db {
            reason: e.to_string(),
        };
        if let Some(id) = self.skill.insert(row).await.map_err(map)? {
            return Ok(id);
        }
        self.skill
            .get_id_by_name(&self.tenant, SEED_USER, SEED_AGENT, SEED_PROJECT, name)
            .await
            .map_err(map)?
            .ok_or_else(|| SeedBuiltinBootstrapError::Db {
                reason: format!("skill `{name}` insert no-op but not found"),
            })
    }

    /// Insert-or-recover a PythonCode id (class 22). `insert` is not
    /// ON-CONFLICT, so get-then-insert via [`PgPythonCodeStore::get_by_name`].
    /// Builtins bypass Q1, so the row is graduated to `validated` directly
    /// (the DDL default is `pending`, which the SEC-01 delivery filter hides).
    async fn upsert_python_code(
        &self,
        row: NewPgPythonCode,
        name: &str,
    ) -> Result<Uuid, SeedBuiltinBootstrapError> {
        let map = |e: crate::pg_python_code_store::PgPythonCodeStoreError| {
            SeedBuiltinBootstrapError::Db { reason: e.to_string() }
        };
        if let Some(existing) = self
            .python_code
            .get_by_name(&self.tenant, SEED_USER, SEED_AGENT, SEED_PROJECT, name)
            .await
            .map_err(map)?
        {
            return Ok(existing.id);
        }
        let id = self.python_code.insert(row).await.map_err(map)?;
        self.python_code
            .update_validation_status(
                &self.tenant,
                SEED_USER,
                SEED_AGENT,
                SEED_PROJECT,
                id,
                "validated",
            )
            .await
            .map_err(map)?;
        Ok(id)
    }

    /// Insert-or-recover a Recipe id (class 21). `insert` is not ON-CONFLICT,
    /// so get-then-insert via [`PgRecipeStore::get_by_name`].
    async fn upsert_recipe(
        &self,
        row: NewPgRecipe,
        name: &str,
    ) -> Result<Uuid, SeedBuiltinBootstrapError> {
        let map = |e: crate::pg_recipe_store::PgRecipeStoreError| SeedBuiltinBootstrapError::Db {
            reason: e.to_string(),
        };
        if let Some(existing) = self
            .recipe
            .get_by_name(&self.tenant, SEED_USER, SEED_AGENT, SEED_PROJECT, name)
            .await
            .map_err(map)?
        {
            return Ok(existing.id);
        }
        self.recipe.insert(row).await.map_err(map)
    }

    /// Get-or-insert an ExtensionCatalogue (class 23) row and graduate it to
    /// `validated` directly (builtins bypass Q1). Returns the catalogue id.
    async fn upsert_catalogue(
        &self,
        row: NewPgExtensionCatalogue,
        name: &str,
    ) -> Result<Uuid, SeedBuiltinBootstrapError> {
        let map = |e: crate::pg_extension_catalogue_store::PgExtensionCatalogueStoreError| {
            SeedBuiltinBootstrapError::Db { reason: e.to_string() }
        };
        if let Some(existing) = self
            .catalogue
            .get_by_name(&self.tenant, SEED_USER, SEED_AGENT, SEED_PROJECT, name)
            .await
            .map_err(map)?
        {
            return Ok(existing.id);
        }
        let id = self.catalogue.insert(row).await.map_err(map)?;
        // Bypass Q1 pending: graduate the builtin to `validated` directly.
        self.catalogue
            .update_validation_status(
                &self.tenant,
                SEED_USER,
                SEED_AGENT,
                SEED_PROJECT,
                id,
                "validated",
            )
            .await
            .map_err(map)?;
        Ok(id)
    }

    /// Append child component ids to a catalogue row. Deduplicates on re-run
    /// (the SQL uses `array_agg(DISTINCT x)`), so incremental appends are
    /// idempotent.
    async fn append_children(
        &self,
        cat_id: Uuid,
        child_ids: &[Uuid],
    ) -> Result<(), SeedBuiltinBootstrapError> {
        let map = |e: crate::pg_extension_catalogue_store::PgExtensionCatalogueStoreError| {
            SeedBuiltinBootstrapError::Db { reason: e.to_string() }
        };
        self.catalogue
            .append_child_component_ids(
                &self.tenant,
                SEED_USER,
                SEED_AGENT,
                SEED_PROJECT,
                cat_id,
                child_ids,
            )
            .await
            .map_err(map)
    }

    /// Mark a seeded recipe row as Tier-0 eligible. `NewPgRecipe` cannot set
    /// `reborn_recipes.tier` / `wilson_lower`, so a freshly inserted builtin
    /// defaults to `seedling` / `0.0` and `compose_with_pool`'s gate
    /// (`tier ∈ {mature, candidate} && validation_status='validated' &&
    /// wilson_lower >= 0.70`) computes `llm_call_required = true`. Setting
    /// `tier = 'mature'` + `wilson_lower = 1.0` flips that to `false` so the
    /// doc's Tier-0 builtins run without an LLM call. Tier-1 recipes are left
    /// at the insert defaults. Idempotent (plain UPDATE on the seeded row).
    async fn mark_recipe_tier0(
        &self,
        recipe_id: Uuid,
    ) -> Result<(), SeedBuiltinBootstrapError> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|e| SeedBuiltinBootstrapError::Pool {
                reason: e.to_string(),
            })?;
        client
            .execute(
                "UPDATE reborn_recipes \
                 SET tier = 'mature', wilson_lower = 1.0 \
                 WHERE id = $1 AND tenant_id = $2 AND user_id = $3 \
                 AND agent_id = $4 AND project_id = $5",
                &[
                    &recipe_id,
                    &self.tenant,
                    &SEED_USER,
                    &SEED_AGENT,
                    &SEED_PROJECT,
                ],
            )
            .await
            .map_err(|e| SeedBuiltinBootstrapError::Db {
                reason: e.to_string(),
            })?;
        Ok(())
    }

    /// Insert (or recover) a recipe transcribed from the doc's flat format into
    /// the IBS authoring model, then — for Tier-0 recipes — mark it
    /// Tier-0 eligible. `step_entries` are the pre-built IBS `StepEntry`
    /// objects (channel→knowledge, step_id→stepnumber, `llm`→`text`, include
    /// placeholders resolved to real seeded UUIDs); `yaml_source` is the
    /// verbatim doc `step_descriptions` block (WebUI renderer only — the IBS
    /// reads `steps`); `intent_examples` are the doc's `{input, class}`
    /// objects (preserved at the recipe top-level for Phase N graduation; the
    /// synthesized variant also carries the bare input strings).
    #[allow(clippy::too_many_arguments)]
    async fn seed_recipe(
        &self,
        tenant: &str,
        name: &str,
        description: &str,
        tier0: bool,
        yaml_source: &str,
        step_entries: &[Value],
        intent_examples: &[Value],
    ) -> Result<Uuid, SeedBuiltinBootstrapError> {
        let id = self
            .upsert_recipe(
                recipe_row(tenant, name, description, yaml_source, step_entries, intent_examples),
                name,
            )
            .await?;
        if tier0 {
            self.mark_recipe_tier0(id).await?;
        }
        Ok(id)
    }
}

/// Seed the built-in first-party component stack for `tenant_id`.
///
/// Idempotent: safe to call on every composition boot. Each domain group is
/// seeded independently (filesystem → network → memory → process →
/// management). Only the filesystem group is implemented in this chunk;
/// subsequent chunks add the remaining groups.
pub async fn seed_builtin_components(
    pool: Arc<PgPool>,
    tenant_id: &str,
) -> Result<(), SeedBuiltinBootstrapError> {
    let stores = BootstrapStores::new(pool, tenant_id);

    // Pass 1 — filesystem group (read_file, write_file, list_dir, glob, grep,
    // apply_patch). Subsequent chunks add network / memory / process /
    // management groups here.
    seed_filesystem_group(&stores).await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Filesystem group (Pass 1)
// ---------------------------------------------------------------------------

/// Filesystem domain ExtensionCatalogue name (Step 22).
const CAT_FILESYSTEM: &str = "builtin-filesystem";

/// Seed the filesystem domain group: the primary `builtin-filesystem`
/// catalogue + the 6 per-tool catalogues + the 6 Tool rows + 6 ToolSkill rows.
///
/// PythonCode, leaf Skills, the Domain Skill, and Recipes are added in
/// subsequent chunks; their ids are appended to the catalogues'
/// `child_component_ids` as they are minted (dedup makes this idempotent).
async fn seed_filesystem_group(
    stores: &BootstrapStores,
) -> Result<(), SeedBuiltinBootstrapError> {
    let tenant = stores.tenant.clone();

    // 1. Primary domain catalogue + per-tool catalogues (empty child_ids;
    //    appended to as children are minted below and in later chunks).
    let cat_filesystem = stores
        .upsert_catalogue(filesystem_primary_catalogue_row(&tenant), CAT_FILESYSTEM)
        .await?;
    let cat_read_file = stores
        .upsert_catalogue(
            ext_catalogue_row(
                &tenant,
                "ext-read-file",
                "File read capability (builtin.read_file).",
                CAT_EXT_READ_FILE_OVERVIEW,
                json!([
                    {"group_name": "file-read-full", "description": "Read a complete file"},
                    {"group_name": "file-read-range", "description": "Read a specific line range"}
                ]),
            ),
            "ext-read-file",
        )
        .await?;
    let cat_write_file = stores
        .upsert_catalogue(
            ext_catalogue_row(
                &tenant,
                "ext-write-file",
                "File write capability (builtin.write_file).",
                CAT_EXT_WRITE_FILE_OVERVIEW,
                json!([
                    {"group_name": "file-write-new", "description": "Create a new file"},
                    {"group_name": "file-write-replace", "description": "Fully replace existing file content"}
                ]),
            ),
            "ext-write-file",
        )
        .await?;
    let cat_list_dir = stores
        .upsert_catalogue(
            ext_catalogue_row(
                &tenant,
                "ext-list-dir",
                "Directory listing capability (builtin.list_dir).",
                CAT_EXT_LIST_DIR_OVERVIEW,
                json!([
                    {"group_name": "dir-list-shallow", "description": "Single-level directory listing"},
                    {"group_name": "dir-list-recursive", "description": "Recursive directory tree scan"},
                    {"group_name": "dir-list-filtered", "description": "Type-filtered listing (files-only, dirs-only)"}
                ]),
            ),
            "ext-list-dir",
        )
        .await?;
    let cat_glob = stores
        .upsert_catalogue(
            ext_catalogue_row(
                &tenant,
                "ext-glob",
                "Glob file search capability (builtin.glob).",
                CAT_EXT_GLOB_OVERVIEW,
                json!([
                    {"group_name": "glob-by-extension", "description": "Find all files of a specific extension"},
                    {"group_name": "glob-by-name", "description": "Find files matching a name pattern"},
                    {"group_name": "glob-in-subdir", "description": "Restrict glob to a subdirectory"}
                ]),
            ),
            "ext-glob",
        )
        .await?;
    let cat_grep = stores
        .upsert_catalogue(
            ext_catalogue_row(
                &tenant,
                "ext-grep",
                "Grep content search capability (builtin.grep).",
                CAT_EXT_GREP_OVERVIEW,
                json!([
                    {"group_name": "grep-files", "description": "Find which files contain a pattern"},
                    {"group_name": "grep-content", "description": "Retrieve matching lines with context"},
                    {"group_name": "grep-count", "description": "Count occurrences without returning content"},
                    {"group_name": "grep-insensitive", "description": "Case-insensitive search"},
                    {"group_name": "grep-typed", "description": "Type-filtered search"}
                ]),
            ),
            "ext-grep",
        )
        .await?;
    let cat_apply_patch = stores
        .upsert_catalogue(
            ext_catalogue_row(
                &tenant,
                "ext-apply-patch",
                "Apply patch capability (builtin.apply_patch).",
                CAT_EXT_APPLY_PATCH_OVERVIEW,
                json!([
                    {"group_name": "patch-single", "description": "Replace one unique occurrence (Tier 1)"},
                    {"group_name": "patch-all", "description": "Replace all occurrences of a string (Tier 0 with explicit slots)"}
                ]),
            ),
            "ext-apply-patch",
        )
        .await?;

    // 2. Tool rows (class 0) — capability_id taken from the live
    //    `*_CAPABILITY_ID` constant in `first_party_tools/`.
    let tool_read_file = stores
        .upsert_tool(tool_read_file_row(&tenant), "read_file")
        .await?;
    let tool_write_file = stores
        .upsert_tool(tool_write_file_row(&tenant), "write_file")
        .await?;
    let tool_list_dir = stores
        .upsert_tool(tool_list_dir_row(&tenant), "list_dir")
        .await?;
    let tool_glob = stores.upsert_tool(tool_glob_row(&tenant), "glob").await?;
    let tool_grep = stores.upsert_tool(tool_grep_row(&tenant), "grep").await?;
    let tool_apply_patch = stores
        .upsert_tool(tool_apply_patch_row(&tenant), "apply_patch")
        .await?;

    // 3. ToolSkill rows (class 13).
    let ts_read_file = stores
        .upsert_tool_skill(ts_read_file_row(&tenant), "ts-read-file")
        .await?;
    let ts_write_file = stores
        .upsert_tool_skill(ts_write_file_row(&tenant), "ts-write-file")
        .await?;
    let ts_list_dir = stores
        .upsert_tool_skill(ts_list_dir_row(&tenant), "ts-list-dir")
        .await?;
    let ts_glob = stores
        .upsert_tool_skill(ts_glob_row(&tenant), "ts-glob")
        .await?;
    let ts_grep = stores
        .upsert_tool_skill(ts_grep_row(&tenant), "ts-grep")
        .await?;
    let ts_apply_patch = stores
        .upsert_tool_skill(ts_apply_patch_row(&tenant), "ts-apply-patch")
        .await?;

    // 4. PythonCode rows (class 22) — the orchestrator executor bodies that
    //    drive every Tier-0 recipe. Transcribed verbatim from
    //    builtin_stuff_v3.md Steps 2.3 / 3.3 / 4.3 / 5.3 / 6.3 / 7.x.1.
    //    consumer_tags omit `05:validator` (builtins bypass Q1; the SEC-01
    //    delivery filter would otherwise hide the row even when validated).
    let pc_read_file = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-read-file",
                "Orchestrator executor: calls host.<tool> to read a file via \
                 builtin.read_file. Input: path (string), range (optional string, \
                 e.g. '1-50'). Output: tool result dict {content, line_count, path}.",
                PC_EXEC_READ_FILE_CONTENT,
            ),
            "pc-exec-read-file",
        )
        .await?;
    let pc_write_file = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-write-file",
                "Orchestrator executor: calls host.<tool> to write a file via \
                 builtin.write_file. Input: path (string), content (string). Output: \
                 tool result dict {path, bytes_written}.",
                PC_EXEC_WRITE_FILE_CONTENT,
            ),
            "pc-exec-write-file",
        )
        .await?;
    let pc_list_dir = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-list-dir",
                "Orchestrator executor: calls host.<tool> to list a directory via \
                 builtin.list_dir. Input: path (string, omit for workspace root), \
                 recursive (bool, default false), max_depth (int, optional).",
                PC_EXEC_LIST_DIR_CONTENT,
            ),
            "pc-exec-list-dir",
        )
        .await?;
    let pc_glob = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-glob",
                "Orchestrator executor: calls host.<tool> to find files via \
                 builtin.glob. Input: pattern (string), path (optional string), \
                 max_results (optional int). Output: tool result with list of \
                 matching paths.",
                PC_EXEC_GLOB_CONTENT,
            ),
            "pc-exec-glob",
        )
        .await?;
    let pc_grep = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-grep",
                "Orchestrator executor: calls host.<tool> to search content via \
                 builtin.grep. Input: pattern (string), path (optional), output_mode \
                 (optional, default files_with_matches), glob (optional), \
                 case_insensitive (optional bool).",
                PC_EXEC_GREP_CONTENT,
            ),
            "pc-exec-grep",
        )
        .await?;
    let pc_apply_patch = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-apply-patch",
                "Orchestrator executor: calls host.<tool> to apply a targeted patch \
                 via builtin.apply_patch. Input: path (string), old_string (string), \
                 new_string (string), replace_all (optional bool). Output: {path, \
                 replacements_made}.",
                PC_EXEC_APPLY_PATCH_CONTENT,
            ),
            "pc-exec-apply-patch",
        )
        .await?;

    // 4b. Filesystem Tier-0 infrastructure + variant PythonCode (class 22).
    //     Grep variants (Step 6.x.3/4/7.1), list filter (Step 4.x.3),
    //     read variants head/tail/exists (Step 2.x.2), combined workflows
    //     read-then-grep / list-then-grep (Step 2.x.2), and pure-logic path
    //     helpers join/basename/dirname (Step 20.x.2). Transcribed verbatim.
    let pc_grep_case_insensitive = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-grep-case-insensitive",
                "Orchestrator executor: calls host.<tool> for a case-insensitive \
                 grep via builtin.grep. Input: pattern (string), path (optional), \
                 output_mode (optional, default 'files_with_matches'). Sets \
                 case_insensitive=true.",
                PC_EXEC_GREP_CASE_INSENSITIVE_CONTENT,
            ),
            "pc-exec-grep-case-insensitive",
        )
        .await?;
    let pc_grep_type_filtered = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-grep-type-filtered",
                "Orchestrator executor: calls host.<tool> for a type-filtered grep \
                 via builtin.grep. Input: pattern (string), glob_filter (string \
                 e.g. '*.rs'), path (optional), output_mode (optional, default \
                 'files_with_matches').",
                PC_EXEC_GREP_TYPE_FILTERED_CONTENT,
            ),
            "pc-exec-grep-type-filtered",
        )
        .await?;
    let pc_grep_invert = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-grep-invert",
                "Orchestrator executor: calls host.<tool> for an inverted grep via \
                 builtin.grep. Input: pattern (string), path (optional), \
                 output_mode (optional, default 'files_with_matches'). Sets \
                 invert_match=true.",
                PC_EXEC_GREP_INVERT_CONTENT,
            ),
            "pc-exec-grep-invert",
        )
        .await?;
    let pc_list_filter_by_type = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-list-filter-by-type",
                "Pure-logic helper: filters a list_dir result to only entries of a \
                 given type. Input: entries (list from list_dir result), entry_type \
                 ('file' | 'directory'). Output: {entries, entry_type, count} — only \
                 matching entries.",
                PC_EXEC_LIST_FILTER_BY_TYPE_CONTENT,
            ),
            "pc-exec-list-filter-by-type",
        )
        .await?;
    let pc_read_file_head = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-read-file-head",
                "Orchestrator executor: reads the first 50 lines of a file via \
                 builtin.read_file. Input: vars.slot0 = file path. Output: {content, \
                 line_count, path}.",
                PC_EXEC_READ_FILE_HEAD_CONTENT,
            ),
            "pc-exec-read-file-head",
        )
        .await?;
    let pc_read_file_tail = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-read-file-tail",
                "Orchestrator executor: reads the last 50 lines of a file (lines -50 \
                 onward). Input: vars.slot0 = file path. Output: {content, \
                 line_count, path}.",
                PC_EXEC_READ_FILE_TAIL_CONTENT,
            ),
            "pc-exec-read-file-tail",
        )
        .await?;
    let pc_file_exists = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-file-exists",
                "Orchestrator executor: checks whether a file exists by attempting to \
                 read line 1. Input: vars.slot0 = file path. Output: {exists: bool, \
                 path: string}.",
                PC_EXEC_FILE_EXISTS_CONTENT,
            ),
            "pc-exec-file-exists",
        )
        .await?;
    let pc_read_then_grep = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-read-then-grep",
                "Orchestrator executor: reads a file then greps the content for a \
                 pattern. Input: vars.slot0 = path, vars.slot1 = grep pattern. \
                 Output: matching lines list.",
                PC_EXEC_READ_THEN_GREP_CONTENT,
            ),
            "pc-exec-read-then-grep",
        )
        .await?;
    let pc_list_then_grep = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-list-then-grep",
                "Orchestrator executor: lists directory entries then filters by name \
                 substring. Input: vars.slot0 = directory path, vars.slot1 = name \
                 filter substring. Output: filtered entry names list.",
                PC_EXEC_LIST_THEN_GREP_CONTENT,
            ),
            "pc-exec-list-then-grep",
        )
        .await?;
    let pc_path_join = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-path-join",
                "PythonCode helper: join two path segments with a '/' separator. \
                 Input: vars.slot0 = base path, vars.slot1 = sub-path. Output: \
                 joined path string.",
                PC_PATH_JOIN_CONTENT,
            ),
            "pc-path-join",
        )
        .await?;
    let pc_path_basename = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-path-basename",
                "PythonCode helper: extract the filename (last path component) from a \
                 path. Input: vars.slot0 = path string. Output: basename string.",
                PC_PATH_BASENAME_CONTENT,
            ),
            "pc-path-basename",
        )
        .await?;
    let pc_path_dirname = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-path-dirname",
                "PythonCode helper: extract the directory part of a path. Input: \
                 vars.slot0 = path string. Output: directory path string.",
                PC_PATH_DIRNAME_CONTENT,
            ),
            "pc-path-dirname",
        )
        .await?;

    // 4c. Filesystem Leaf Skills (class 1). Prose bodies reference the
    //     ToolSkill + PythonCode above by name. consumer_tags carry
    //     05:validator (safe: the skill store has no SEC-01 hiding filter,
    //     unlike pg_python_code_store). No intent_examples in the doc source
    //     -> json!([]) (leaf skills are pulled in via recipe steps, not direct
    //     intent matching). Transcribed verbatim from builtin_stuff_v3.md
    //     Steps 2.4/2.5, 3.4/3.5/3.x.1, 4.4/4.5/4.x.1/4.x.2, 5.4/5.5/5.6,
    //     6.4/6.5/6.6/6.x.1/6.x.2/6.x.7.2, 7.3/7.4, plus the +5 additions
    //     (read-file-head/tail, file-exists, read-and-grep, list-and-filter).
    let skill_read_file = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-read-file",
                "Leaf skill: how to read a file from the workspace.",
                SKILL_READ_FILE_BODY,
            ),
            "skill-read-file",
        )
        .await?;
    let skill_read_file_range = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-read-file-range",
                "Leaf skill: how to read a specific line range from a large file.",
                SKILL_READ_FILE_RANGE_BODY,
            ),
            "skill-read-file-range",
        )
        .await?;
    let skill_read_file_head = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-read-file-head",
                "Leaf skill: how to read the first N lines of a file (head pattern).",
                SKILL_READ_FILE_HEAD_BODY,
            ),
            "skill-read-file-head",
        )
        .await?;
    let skill_read_file_tail = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-read-file-tail",
                "Leaf skill: how to read the last N lines of a file (tail pattern).",
                SKILL_READ_FILE_TAIL_BODY,
            ),
            "skill-read-file-tail",
        )
        .await?;
    let skill_file_exists = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-file-exists",
                "Leaf skill: how to check whether a file exists before reading or writing.",
                SKILL_FILE_EXISTS_BODY,
            ),
            "skill-file-exists",
        )
        .await?;
    let skill_write_file_new = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-write-file-new",
                "Leaf skill: how to create a new file in the workspace.",
                SKILL_WRITE_FILE_NEW_BODY,
            ),
            "skill-write-file-new",
        )
        .await?;
    let skill_write_file_replace = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-write-file-replace",
                "Leaf skill: how to fully replace an existing file's content.",
                SKILL_WRITE_FILE_REPLACE_BODY,
            ),
            "skill-write-file-replace",
        )
        .await?;
    let skill_write_file_template = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-write-file-template",
                "Leaf skill: how to write a file using a pre-baked template content from vars.",
                SKILL_WRITE_FILE_TEMPLATE_BODY,
            ),
            "skill-write-file-template",
        )
        .await?;
    let skill_list_dir = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-list-dir",
                "Leaf skill: how to list the contents of a single directory.",
                SKILL_LIST_DIR_BODY,
            ),
            "skill-list-dir",
        )
        .await?;
    let skill_list_dir_recursive = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-list-dir-recursive",
                "Leaf skill: how to recursively scan a directory tree.",
                SKILL_LIST_DIR_RECURSIVE_BODY,
            ),
            "skill-list-dir-recursive",
        )
        .await?;
    let skill_list_dir_files_only = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-list-dir-files-only",
                "Leaf skill: how to list only regular files (no subdirectories) in a directory.",
                SKILL_LIST_DIR_FILES_ONLY_BODY,
            ),
            "skill-list-dir-files-only",
        )
        .await?;
    let skill_list_dir_dirs_only = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-list-dir-dirs-only",
                "Leaf skill: how to list only subdirectories in a directory.",
                SKILL_LIST_DIR_DIRS_ONLY_BODY,
            ),
            "skill-list-dir-dirs-only",
        )
        .await?;
    let skill_glob_by_extension = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-glob-by-extension",
                "Leaf skill: how to find all files of a specific file extension.",
                SKILL_GLOB_BY_EXTENSION_BODY,
            ),
            "skill-glob-by-extension",
        )
        .await?;
    let skill_glob_by_name = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-glob-by-name",
                "Leaf skill: how to find files by name pattern (not extension).",
                SKILL_GLOB_BY_NAME_BODY,
            ),
            "skill-glob-by-name",
        )
        .await?;
    let skill_glob_in_subdir = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-glob-in-subdir",
                "Leaf skill: how to restrict a glob search to a specific subdirectory.",
                SKILL_GLOB_IN_SUBDIR_BODY,
            ),
            "skill-glob-in-subdir",
        )
        .await?;
    let skill_grep_files = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-grep-files",
                "Leaf skill: how to find which files contain a regex pattern.",
                SKILL_GREP_FILES_BODY,
            ),
            "skill-grep-files",
        )
        .await?;
    let skill_grep_content = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-grep-content",
                "Leaf skill: how to retrieve matching lines (with context) from files.",
                SKILL_GREP_CONTENT_BODY,
            ),
            "skill-grep-content",
        )
        .await?;
    let skill_grep_count = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-grep-count",
                "Leaf skill: how to count pattern occurrences without returning the matching lines.",
                SKILL_GREP_COUNT_BODY,
            ),
            "skill-grep-count",
        )
        .await?;
    let skill_grep_case_insensitive = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-grep-case-insensitive",
                "Leaf skill: how to perform a case-insensitive regex search across files.",
                SKILL_GREP_CASE_INSENSITIVE_BODY,
            ),
            "skill-grep-case-insensitive",
        )
        .await?;
    let skill_grep_type_filtered = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-grep-type-filtered",
                "Leaf skill: how to restrict a grep search to specific file types using the glob filter.",
                SKILL_GREP_TYPE_FILTERED_BODY,
            ),
            "skill-grep-type-filtered",
        )
        .await?;
    let skill_grep_invert = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-grep-invert",
                "Leaf skill: how to find files or lines that do NOT match a pattern.",
                SKILL_GREP_INVERT_BODY,
            ),
            "skill-grep-invert",
        )
        .await?;
    let skill_apply_patch_single = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-apply-patch-single",
                "Leaf skill: how to replace a single unique occurrence in a file.",
                SKILL_APPLY_PATCH_SINGLE_BODY,
            ),
            "skill-apply-patch-single",
        )
        .await?;
    let skill_apply_patch_all = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-apply-patch-all",
                "Leaf skill: how to replace every occurrence of a string in a file.",
                SKILL_APPLY_PATCH_ALL_BODY,
            ),
            "skill-apply-patch-all",
        )
        .await?;
    let skill_read_and_grep = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-read-and-grep",
                "Leaf skill: how to read a file and filter its content by a pattern in one step.",
                SKILL_READ_AND_GREP_BODY,
            ),
            "skill-read-and-grep",
        )
        .await?;
    let skill_list_and_filter = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-list-and-filter",
                "Leaf skill: how to list a directory and filter the entries by name in one step.",
                SKILL_LIST_AND_FILTER_BODY,
            ),
            "skill-list-and-filter",
        )
        .await?;

    // 4d. Filesystem Domain Skill (class 2) — `skill-filesystem`. References
    //     all 25 filesystem leaf skills by name (no duplicated content).
    //     NOTE: an earlier audit incorrectly reported this domain skill as
    //     undefined in the doc; it IS defined at `builtin_stuff_v3.md` Step 7.x
    //     (~line 3631). Seeded here per that definition (Q3 was answered "skip"
    //     under the since-corrected premise that no definition existed).
    let skill_filesystem = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-filesystem",
                "The filesystem domain provides six scoped tools for working with the workspace.",
                SKILL_FILESYSTEM_BODY,
                2,
                LEAF_SKILL_TAGS,
            ),
            "skill-filesystem",
        )
        .await?;

    // 4e. Filesystem Recipes (class 21) — transcribed from the doc's flat
    //     format into the IBS authoring model (Q1 decision A). Tier-0 recipes
    //     are marked tier0-eligible post-insert (Q2 decision A). Read/write/list
    //     here; glob/grep/patch land in subsequent chunks.
    let recipe_file_read = stores
        .seed_recipe(
            &tenant,
            "file-read",
            "Read a file from the workspace and return its contents.",
            true,
            RECIPE_FILE_READ_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-read-file ToolSkill binding", "component", &[ts_read_file]),
                step_entry(
                    2,
                    "orchestrator",
                    "PythonCode calls host.read_file(path, range) and returns result",
                    "component",
                    &[pc_read_file],
                ),
            ],
            &[
                json!({"input": "read a file", "class": 1}),
                json!({"input": "show me the contents of", "class": 1}),
                json!({"input": "open file", "class": 1}),
                json!({"input": "what is in config.toml", "class": 2}),
                json!({"input": "read the file at this path", "class": 1}),
                json!({"input": "show me this file", "class": 1}),
                json!({"input": "load file contents", "class": 1}),
                json!({"input": "display the file", "class": 1}),
                json!({"input": "cat this file", "class": 2}),
                json!({"input": "inspect the configuration file", "class": 2}),
                json!({"input": "file read", "class": 1}),
            ],
        )
        .await?;
    let recipe_file_read_range = stores
        .seed_recipe(
            &tenant,
            "file-read-range",
            "Read a specific line range from a file (for large files or targeted inspection).",
            true,
            RECIPE_FILE_READ_RANGE_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-read-file ToolSkill binding", "component", &[ts_read_file]),
                step_entry(
                    2,
                    "orchestrator",
                    "PythonCode calls host.read_file(path, range) — range slot is required",
                    "component",
                    &[pc_read_file],
                ),
            ],
            &[
                json!({"input": "read lines 10 to 50 of main.rs", "class": 1}),
                json!({"input": "show me line 100 to 200 of this file", "class": 1}),
                json!({"input": "read the first 30 lines", "class": 1}),
                json!({"input": "read lines 500 to 600 of the log", "class": 1}),
                json!({"input": "show only the top 20 lines", "class": 2}),
                json!({"input": "read the middle section of this file", "class": 2}),
                json!({"input": "show lines starting from 150", "class": 2}),
                json!({"input": "paginate through a large file", "class": 2}),
                json!({"input": "read just this specific section of the file", "class": 2}),
                json!({"input": "show me the function body starting at line 80", "class": 2}),
            ],
        )
        .await?;
    let recipe_file_write = stores
        .seed_recipe(
            &tenant,
            "file-write",
            "Read current file content (if it exists), then write new content authored by LLM.",
            false,
            RECIPE_FILE_WRITE_YAML,
            &[
                step_entry(
                    1,
                    "orchestrator",
                    "Load read + write leaf skill context",
                    "component",
                    &[skill_read_file, skill_write_file_replace, skill_write_file_new],
                ),
                step_entry(
                    2,
                    "rust",
                    "Pre-load ts-read-file binding (for optional pre-read)",
                    "component",
                    &[ts_read_file],
                ),
                step_entry(
                    3,
                    "orchestrator",
                    "LLM optionally reads current content, then composes new file content",
                    "text",
                    &[],
                ),
                step_entry(4, "rust", "Pre-load ts-write-file binding", "component", &[ts_write_file]),
            ],
            &[
                json!({"input": "write a file", "class": 1}),
                json!({"input": "create a file", "class": 1}),
                json!({"input": "save content to a file", "class": 1}),
                json!({"input": "write a README for this project", "class": 2}),
                json!({"input": "create config.toml with these values", "class": 2}),
                json!({"input": "make a new file with this content", "class": 1}),
                json!({"input": "create a new document", "class": 1}),
                json!({"input": "write this content to disk", "class": 1}),
                json!({"input": "overwrite this file with new content", "class": 2}),
                json!({"input": "file write", "class": 1}),
            ],
        )
        .await?;
    let recipe_file_write_template = stores
        .seed_recipe(
            &tenant,
            "file-write-template",
            "Write a file using a fully pre-baked template content from recipe vars (no LLM).",
            true,
            RECIPE_FILE_WRITE_TEMPLATE_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-write-file ToolSkill binding", "component", &[ts_write_file]),
                step_entry(
                    2,
                    "orchestrator",
                    "PythonCode calls host.write_file(path=slot0, content=slot1) — both pre-baked",
                    "component",
                    &[pc_write_file],
                ),
            ],
            &[
                json!({"input": "create an empty __init__.py", "class": 2}),
                json!({"input": "create a default .gitignore", "class": 2}),
                json!({"input": "write a minimal config file", "class": 2}),
                json!({"input": "initialize this file with a template", "class": 2}),
                json!({"input": "create a stub file", "class": 2}),
                json!({"input": "write a file from a template", "class": 1}),
                json!({"input": "scaffold a new config file", "class": 2}),
                json!({"input": "create a default settings file", "class": 2}),
                json!({"input": "file write from template vars", "class": 1}),
            ],
        )
        .await?;
    let recipe_file_list = stores
        .seed_recipe(
            &tenant,
            "file-list",
            "List the contents of a directory.",
            true,
            RECIPE_FILE_LIST_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-list-dir ToolSkill binding", "component", &[ts_list_dir]),
                step_entry(
                    2,
                    "orchestrator",
                    "PythonCode calls host.list_dir(path, recursive, max_depth)",
                    "component",
                    &[pc_list_dir],
                ),
            ],
            &[
                json!({"input": "list files in this directory", "class": 1}),
                json!({"input": "show directory contents", "class": 1}),
                json!({"input": "what files are in the project root", "class": 1}),
                json!({"input": "show me what is in this folder", "class": 1}),
                json!({"input": "ls", "class": 1}),
                json!({"input": "what is in the src directory", "class": 2}),
                json!({"input": "explore this folder", "class": 2}),
                json!({"input": "directory listing", "class": 1}),
            ],
        )
        .await?;
    let recipe_file_list_recursive = stores
        .seed_recipe(
            &tenant,
            "file-list-recursive",
            "Recursively list all files and directories under a path.",
            true,
            RECIPE_FILE_LIST_RECURSIVE_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-list-dir ToolSkill binding", "component", &[ts_list_dir]),
                step_entry(
                    2,
                    "orchestrator",
                    "PythonCode calls host.list_dir(path, recursive=true, max_depth=3)",
                    "component",
                    &[pc_list_dir],
                ),
            ],
            &[
                json!({"input": "list all files recursively", "class": 1}),
                json!({"input": "show me the full directory tree", "class": 1}),
                json!({"input": "list all files in this project", "class": 1}),
                json!({"input": "recursive directory listing", "class": 1}),
                json!({"input": "show all files and folders", "class": 1}),
                json!({"input": "tree view of this directory", "class": 2}),
                json!({"input": "list every file under this path", "class": 1}),
                json!({"input": "what files exist in this whole project", "class": 2}),
                json!({"input": "ls -r", "class": 1}),
                json!({"input": "recursive ls", "class": 1}),
            ],
        )
        .await?;

    // 4f. Filesystem Glob Recipes (class 21) — 5 recipes, all Tier-0. Each is
    //     a 2-step rust(preload ts-glob) + orchestrator(pc-exec-glob dispatch)
    //     program. Transcribed from the doc's flat format (Q1 decision A).
    let recipe_file_glob = stores
        .seed_recipe(
            &tenant,
            "file-glob",
            "Find files matching a glob pattern.",
            true,
            RECIPE_FILE_GLOB_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-glob ToolSkill binding", "component", &[ts_glob]),
                step_entry(
                    2,
                    "orchestrator",
                    "PythonCode calls host.glob(pattern, path, max_results)",
                    "component",
                    &[pc_glob],
                ),
            ],
            &[
                json!({"input": "find all TypeScript files", "class": 1}),
                json!({"input": "find files matching *.rs", "class": 1}),
                json!({"input": "search for test files in src", "class": 2}),
                json!({"input": "glob pattern **/*.json", "class": 1}),
                json!({"input": "find all config files in this repo", "class": 2}),
                json!({"input": "find all files with this extension", "class": 1}),
                json!({"input": "list all .py files in the project", "class": 1}),
                json!({"input": "find files by name pattern", "class": 2}),
                json!({"input": "glob search", "class": 1}),
            ],
        )
        .await?;
    let recipe_file_glob_by_extension = stores
        .seed_recipe(
            &tenant,
            "file-glob-by-extension",
            "Find all files of a specific file extension (e.g. all .rs or .ts files).",
            true,
            RECIPE_FILE_GLOB_BY_EXTENSION_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-glob ToolSkill binding", "component", &[ts_glob]),
                step_entry(
                    2,
                    "orchestrator",
                    "PythonCode calls host.glob(pattern='**/*.ext')",
                    "component",
                    &[pc_glob],
                ),
            ],
            &[
                json!({"input": "find all Rust files", "class": 1}),
                json!({"input": "list all .ts files", "class": 1}),
                json!({"input": "show me all Python files", "class": 1}),
                json!({"input": "find every .json config", "class": 1}),
                json!({"input": "find all TypeScript files in the project", "class": 1}),
                json!({"input": "list .rs files", "class": 1}),
                json!({"input": "which .md files exist", "class": 1}),
                json!({"input": "find all test files by extension", "class": 2}),
                json!({"input": "list every .toml file in the project", "class": 1}),
                json!({"input": "show me all YAML files", "class": 1}),
            ],
        )
        .await?;
    let recipe_file_glob_by_name = stores
        .seed_recipe(
            &tenant,
            "file-glob-by-name",
            "Find files whose names match a glob pattern (e.g. config*.toml, README*).",
            true,
            RECIPE_FILE_GLOB_BY_NAME_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-glob ToolSkill binding", "component", &[ts_glob]),
                step_entry(
                    2,
                    "orchestrator",
                    "PythonCode calls host.glob(pattern='**/name-pattern')",
                    "component",
                    &[pc_glob],
                ),
            ],
            &[
                json!({"input": "find the Makefile", "class": 1}),
                json!({"input": "find all README files", "class": 1}),
                json!({"input": "locate the config files", "class": 1}),
                json!({"input": "find files named settings*", "class": 1}),
                json!({"input": "where is the docker-compose file", "class": 2}),
                json!({"input": "find all files starting with test_", "class": 1}),
                json!({"input": "locate any .env files", "class": 2}),
                json!({"input": "find files by name pattern", "class": 1}),
                json!({"input": "where is the package.json", "class": 2}),
                json!({"input": "find all files that start with index", "class": 1}),
            ],
        )
        .await?;
    let recipe_file_glob_in_subdir = stores
        .seed_recipe(
            &tenant,
            "file-glob-in-subdir",
            "Find files matching a pattern within a specific subdirectory.",
            true,
            RECIPE_FILE_GLOB_IN_SUBDIR_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-glob ToolSkill binding", "component", &[ts_glob]),
                step_entry(
                    2,
                    "orchestrator",
                    "PythonCode calls host.glob(pattern, path=subdir)",
                    "component",
                    &[pc_glob],
                ),
            ],
            &[
                json!({"input": "find all test files in the src folder", "class": 1}),
                json!({"input": "list .ts files in the components directory", "class": 1}),
                json!({"input": "search for config files in crates/", "class": 2}),
                json!({"input": "find .rs files only in the migrations dir", "class": 2}),
                json!({"input": "glob in a subdirectory", "class": 1}),
                json!({"input": "show all Python files under the lib folder", "class": 2}),
                json!({"input": "find all markdown docs inside the docs directory", "class": 2}),
                json!({"input": "list all .json files under the config subfolder", "class": 1}),
                json!({"input": "restrict file search to the tests subdirectory", "class": 2}),
                json!({"input": "find every YAML file in the deployment directory", "class": 2}),
            ],
        )
        .await?;
    let recipe_file_glob_recent = stores
        .seed_recipe(
            &tenant,
            "file-glob-recent",
            "Find the 10 most recently modified files matching a glob pattern.",
            true,
            RECIPE_FILE_GLOB_RECENT_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-glob ToolSkill binding", "component", &[ts_glob]),
                step_entry(
                    2,
                    "orchestrator",
                    "PythonCode calls host.glob(pattern, max_results=10) — sorted by mtime",
                    "component",
                    &[pc_glob],
                ),
            ],
            &[
                json!({"input": "what files were recently modified", "class": 1}),
                json!({"input": "show the most recently changed files", "class": 1}),
                json!({"input": "what changed recently in this project", "class": 2}),
                json!({"input": "recently modified TypeScript files", "class": 2}),
                json!({"input": "last 10 modified files", "class": 1}),
                json!({"input": "what did I change recently", "class": 2}),
                json!({"input": "most recently touched files", "class": 2}),
                json!({"input": "find recently edited source files", "class": 2}),
                json!({"input": "show recently modified files in src", "class": 2}),
            ],
        )
        .await?;

    // 5. Append the minted tool + toolskill + python_code ids to each per-tool
    //    catalogue (leaf Skill / Recipe ids appended in later chunks).
    stores
        .append_children(
            cat_read_file,
            &[
                tool_read_file,
                ts_read_file,
                pc_read_file,
                pc_read_file_head,
                pc_read_file_tail,
                pc_file_exists,
                pc_read_then_grep,
                skill_read_file,
                skill_read_file_range,
                skill_read_file_head,
                skill_read_file_tail,
                skill_file_exists,
                skill_read_and_grep,
                recipe_file_read,
                recipe_file_read_range,
            ],
        )
        .await?;
    stores
        .append_children(
            cat_write_file,
            &[
                tool_write_file,
                ts_write_file,
                pc_write_file,
                skill_write_file_new,
                skill_write_file_replace,
                skill_write_file_template,
                recipe_file_write,
                recipe_file_write_template,
            ],
        )
        .await?;
    stores
        .append_children(
            cat_list_dir,
            &[
                tool_list_dir,
                ts_list_dir,
                pc_list_dir,
                pc_list_filter_by_type,
                pc_list_then_grep,
                skill_list_dir,
                skill_list_dir_recursive,
                skill_list_dir_files_only,
                skill_list_dir_dirs_only,
                skill_list_and_filter,
                recipe_file_list,
                recipe_file_list_recursive,
            ],
        )
        .await?;
    stores
        .append_children(
            cat_glob,
            &[
                tool_glob,
                ts_glob,
                pc_glob,
                skill_glob_by_extension,
                skill_glob_by_name,
                skill_glob_in_subdir,
                recipe_file_glob,
                recipe_file_glob_by_extension,
                recipe_file_glob_by_name,
                recipe_file_glob_in_subdir,
                recipe_file_glob_recent,
            ],
        )
        .await?;
    stores
        .append_children(
            cat_grep,
            &[
                tool_grep,
                ts_grep,
                pc_grep,
                pc_grep_case_insensitive,
                pc_grep_type_filtered,
                pc_grep_invert,
                skill_grep_files,
                skill_grep_content,
                skill_grep_count,
                skill_grep_case_insensitive,
                skill_grep_type_filtered,
                skill_grep_invert,
            ],
        )
        .await?;
    stores
        .append_children(
            cat_apply_patch,
            &[
                tool_apply_patch,
                ts_apply_patch,
                pc_apply_patch,
                skill_apply_patch_single,
                skill_apply_patch_all,
            ],
        )
        .await?;

    // 6. Append all filesystem tool + toolskill + python_code + leaf skill ids
    //    to the primary catalogue (path helpers are cross-capability → primary
    //    only), plus the filesystem domain skill + the 11 read/write/list/glob
    //    recipes. Grep/patch recipes land in later chunks.
    stores
        .append_children(
            cat_filesystem,
            &[
                tool_read_file,
                ts_read_file,
                pc_read_file,
                pc_read_file_head,
                pc_read_file_tail,
                pc_file_exists,
                pc_read_then_grep,
                skill_read_file,
                skill_read_file_range,
                skill_read_file_head,
                skill_read_file_tail,
                skill_file_exists,
                skill_read_and_grep,
                tool_write_file,
                ts_write_file,
                pc_write_file,
                skill_write_file_new,
                skill_write_file_replace,
                skill_write_file_template,
                tool_list_dir,
                ts_list_dir,
                pc_list_dir,
                pc_list_filter_by_type,
                pc_list_then_grep,
                skill_list_dir,
                skill_list_dir_recursive,
                skill_list_dir_files_only,
                skill_list_dir_dirs_only,
                skill_list_and_filter,
                tool_glob,
                ts_glob,
                pc_glob,
                skill_glob_by_extension,
                skill_glob_by_name,
                skill_glob_in_subdir,
                recipe_file_glob,
                recipe_file_glob_by_extension,
                recipe_file_glob_by_name,
                recipe_file_glob_in_subdir,
                recipe_file_glob_recent,
                tool_grep,
                ts_grep,
                pc_grep,
                pc_grep_case_insensitive,
                pc_grep_type_filtered,
                pc_grep_invert,
                skill_grep_files,
                skill_grep_content,
                skill_grep_count,
                skill_grep_case_insensitive,
                skill_grep_type_filtered,
                skill_grep_invert,
                tool_apply_patch,
                ts_apply_patch,
                pc_apply_patch,
                skill_apply_patch_single,
                skill_apply_patch_all,
                pc_path_join,
                pc_path_basename,
                pc_path_dirname,
                recipe_file_read,
                recipe_file_read_range,
                recipe_file_write,
                recipe_file_write_template,
                recipe_file_list,
                recipe_file_list_recursive,
                skill_filesystem,
            ],
        )
        .await?;

    tracing::debug!(catalogue_id = %cat_filesystem, "seeded filesystem group (chunk 3b: 6 base + 12 variant/helper PythonCode + 25 leaf skills + 1 domain skill + 11 read/write/list/glob recipes)");
    Ok(())
}

// ---------------------------------------------------------------------------
// Filesystem ExtensionCatalogue overview_doc constants (transcribed from
// builtin_stuff_v3.md Step 22 + Step 22.x.1–22.x.6).
// ---------------------------------------------------------------------------

const CAT_FILESYSTEM_OVERVIEW: &str = r#"# Filesystem Capabilities

The filesystem domain gives the agent structured, sandboxed access to the host
file system. All operations are scoped to the session's working directory or an
explicitly granted path scope — the agent cannot read or write outside its allowed
paths.

## Tools in this domain
- builtin.read_file — read a file or range of lines from a file
- builtin.write_file — create or overwrite a file
- builtin.list_dir — list directory contents (shallow or recursive)
- builtin.glob — find files by name/extension pattern
- builtin.grep — search file contents by regex or literal
- builtin.apply_patch — make targeted edits to a file (search-and-replace)

## When to use which tool
- Locate files: glob (by name/extension) or grep (by content)
- Read content: read_file (full file or line range)
- Create/replace: write_file
- Targeted edits: apply_patch (preferred over read+write for partial changes)
- Explore structure: list_dir

## Scope and safety
- All paths are resolved relative to the session root. Absolute paths outside
  the granted scope will be rejected.
- write_file and apply_patch require approval in restricted profiles.
- apply_patch uses exact string matching by default — provide exact whitespace.
"#;

const CAT_EXT_READ_FILE_OVERVIEW: &str = r#"# File Read Capability
Tool: builtin.read_file
Effect: read (sandboxed to workspace mount)

Reads a file's content (full or line-range). Use for inspecting source files,
config files, logs, or any workspace file before editing or processing it.

Approaches:
- Full read: path only -> file-read recipe
- Ranged read: path + range -> file-read-range recipe
- Large file: paginate using range='N-M' in iterations
"#;

const CAT_EXT_WRITE_FILE_OVERVIEW: &str = r#"# File Write Capability
Tool: builtin.write_file
Effect: write (sandboxed to workspace mount)

Writes or replaces a file's full content. Use when creating a new file or
intentionally replacing an entire file. For partial edits, prefer ext-apply-patch.

Approaches:
- New file: path + new content -> file-write recipe
- Full replace: read first, then write with new content -> file-write recipe
"#;

const CAT_EXT_LIST_DIR_OVERVIEW: &str = r#"# Directory Listing Capability
Tool: builtin.list_dir
Effect: read_filesystem (sandboxed to workspace mount)

Lists directory contents: single level, recursive tree, or type-filtered.

Approaches:
- Shallow listing: path only -> file-list recipe
- Recursive scan: path + recursive:true -> file-list-recursive recipe
- Files only: list then filter -> file-list-files-only recipe
- Directories only: list then filter -> file-list-dirs-only recipe
"#;

const CAT_EXT_GLOB_OVERVIEW: &str = r#"# Glob File Search Capability
Tool: builtin.glob
Effect: read_filesystem (sandboxed to workspace mount)

Finds files by name or extension pattern. Sorted by modification time.

Approaches:
- By extension: **/*.ext -> file-glob-by-extension recipe
- By name pattern: **/name* -> file-glob-by-name recipe
- In a subdirectory: path + pattern -> file-glob-in-subdir recipe
- Generic pattern: any pattern -> file-glob recipe
"#;

const CAT_EXT_GREP_OVERVIEW: &str = r#"# Grep Content Search Capability
Tool: builtin.grep
Effect: read_filesystem (sandboxed to workspace mount)

Searches file contents by regex or literal pattern. Three output modes:
files_with_matches (fast, compact), content (matching lines + context), count (frequency).

Approaches:
- Which files: output_mode=files_with_matches -> file-grep-files recipe
- Matching lines: output_mode=content -> file-grep-content recipe
- Count occurrences: output_mode=count -> file-grep-count recipe
- Case-insensitive: case_insensitive=true -> file-grep-case-insensitive recipe
- Type-filtered: glob='*.ext' -> file-grep-type-filtered recipe
"#;

const CAT_EXT_APPLY_PATCH_OVERVIEW: &str = r#"# Apply Patch Capability
Tool: builtin.apply_patch
Effect: mixed (reads + writes the file, sandboxed to workspace mount)
Permission: Ask (requires user confirmation in most profiles)

Applies a targeted search-replace edit to a file. Safer than full file replacement
because it requires exact matching of the old content.

Approaches:
- Single unique replacement: old_string + new_string -> file-patch recipe (Tier 1)
- Replace all occurrences: replace_all=true -> file-patch-replace-all recipe (Tier 0 if exact strings are slot-provided)

Most patch operations are Tier 1 because the LLM must read the file first and compose
exact old/new strings. file-patch-replace-all is Tier 0 when the caller supplies both
the old and new strings directly as recipe slots.
"#;

// ---------------------------------------------------------------------------
// Row builders — ExtensionCatalogue
// ---------------------------------------------------------------------------

/// Build the primary `builtin-filesystem` ExtensionCatalogue row.
fn filesystem_primary_catalogue_row(tenant: &str) -> NewPgExtensionCatalogue {
    NewPgExtensionCatalogue {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: CAT_FILESYSTEM.to_string(),
        description: "Filesystem domain capability catalogue (read_file, write_file, \
                       list_dir, glob, grep, apply_patch)."
            .to_string(),
        version: "1.0".into(),
        overview_doc: CAT_FILESYSTEM_OVERVIEW.into(),
        task_groups: json!([
            {"group_name": "file-read", "description": "Reading files: range reads, full reads, grep-then-read workflows"},
            {"group_name": "file-write", "description": "Writing and patching files: create, overwrite, targeted edit"},
            {"group_name": "file-search", "description": "Finding files and content: glob by pattern, grep by content"},
            {"group_name": "file-explore", "description": "Directory listing and workspace navigation"}
        ]),
        child_component_ids: Vec::new(),
        intent_index: None,
        prior_knowledge_content: None,
        override_prompt_creation: false,
        consumer_tags: vec!["02:orchestrator".into()],
        intent_examples: None,
        source: "system".into(),
        dependency_registry: None,
    }
}

/// Build a per-tool ExtensionCatalogue row from the given overview_doc +
/// task_groups JSON.
fn ext_catalogue_row(
    tenant: &str,
    name: &str,
    description: &str,
    overview_doc: &str,
    task_groups: Value,
) -> NewPgExtensionCatalogue {
    NewPgExtensionCatalogue {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        version: "1.0".into(),
        overview_doc: overview_doc.into(),
        task_groups,
        child_component_ids: Vec::new(),
        intent_index: None,
        prior_knowledge_content: None,
        override_prompt_creation: false,
        consumer_tags: vec!["02:orchestrator".into()],
        intent_examples: None,
        source: "system".into(),
        dependency_registry: None,
    }
}

// ---------------------------------------------------------------------------
// Row builders — Tool (class 0). capability_id values are the live
// `*_CAPABILITY_ID` constant strings from `first_party_tools/`.
// ---------------------------------------------------------------------------

fn tool_read_file_row(tenant: &str) -> NewPgTool {
    NewPgTool {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "read_file".to_string(),
        description: "Read the full contents of a scoped-workspace file. Supports an \
                       optional line-range selector (start-end, 1-based inclusive). \
                       Returns {content, line_count, path}."
            .to_string(),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Scoped workspace path to the file"},
                "range": {"type": "string", "description": "Optional line range, format: start-end (1-based)"}
            },
            "required": ["path"]
        })),
        param_template: Some(json!({"path": "{{path}}"})),
        effect_type: "read".to_string(),
        preconditions: Some("Path must resolve within a scoped mount with read permission.".into()),
        error_handling: Some(
            "FilesystemDenied: path outside mounts. File not found: surface to user.".into(),
        ),
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        source: "system".into(),
        validation_status: "validated".into(),
        capability_id: "builtin.read_file".into(),
    }
}

fn tool_write_file_row(tenant: &str) -> NewPgTool {
    NewPgTool {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "write_file".to_string(),
        description: "Write or overwrite a file in the scoped workspace. The entire \
                       content is replaced. Returns {path, bytes_written}. For targeted \
                       edits prefer apply_patch — it is safer and does not require a \
                       full read-back."
            .to_string(),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Scoped workspace path"},
                "content": {"type": "string", "description": "Full file content to write"}
            },
            "required": ["path", "content"]
        })),
        param_template: Some(json!({"path": "{{path}}", "content": "{{content}}"})),
        effect_type: "write".to_string(),
        preconditions: Some(
            "Path must resolve within a scoped mount with write permission. Content <= 6 MiB.".into(),
        ),
        error_handling: Some(
            "FilesystemDenied: path outside mounts. Resource limit: content too large.".into(),
        ),
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        source: "system".into(),
        validation_status: "validated".into(),
        capability_id: "builtin.write_file".into(),
    }
}

fn tool_list_dir_row(tenant: &str) -> NewPgTool {
    NewPgTool {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "list_dir".to_string(),
        description: "List the contents of a directory through scoped mounts. Returns \
                       entry names, types, and sizes. Supports optional recursive \
                       listing with a depth cap."
            .to_string(),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Scoped directory path. Defaults to workspace root."},
                "recursive": {"type": "boolean", "description": "Whether to list recursively"},
                "max_depth": {"type": "integer", "minimum": 0, "description": "Maximum recursive depth"}
            },
            "additionalProperties": false
        })),
        param_template: Some(json!({"path": "{{path}}"})),
        effect_type: "read_filesystem".to_string(),
        preconditions: Some("path must be within the active workspace mount".into()),
        error_handling: Some(
            "path-not-found or permission denied -> tool error; output capped at 1 MiB".into(),
        ),
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        source: "system".into(),
        validation_status: "validated".into(),
        capability_id: "builtin.list_dir".into(),
    }
}

fn tool_glob_row(tenant: &str) -> NewPgTool {
    NewPgTool {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "glob".to_string(),
        description: "Find files matching a glob pattern under a scoped root. Returns \
                       matching file paths sorted by modification time, capped at \
                       max_results."
            .to_string(),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "pattern": {"type": "string", "description": "Glob pattern relative to path"},
                "path": {"type": "string", "description": "Scoped root path. Defaults to workspace root."},
                "max_results": {"type": "integer", "minimum": 0, "description": "Maximum number of results"}
            },
            "required": ["pattern"],
            "additionalProperties": false
        })),
        param_template: Some(json!({"pattern": "{{pattern}}"})),
        effect_type: "read_filesystem".to_string(),
        preconditions: Some(
            "pattern required; path must be within the active workspace mount".into(),
        ),
        error_handling: Some(
            "invalid pattern or path outside mount -> tool error; empty match -> empty list".into(),
        ),
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        source: "system".into(),
        validation_status: "validated".into(),
        capability_id: "builtin.glob".into(),
    }
}

fn tool_grep_row(tenant: &str) -> NewPgTool {
    NewPgTool {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "grep".to_string(),
        description: "Search file contents using a regular expression within scoped \
                       mounts. Supports content, files_with_matches, and count output \
                       modes. Optional glob filter, context lines, case-insensitive \
                       matching, and result pagination."
            .to_string(),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "pattern": {"type": "string", "description": "Regular expression to search for"},
                "path": {"type": "string", "description": "Scoped file or directory path. Defaults to workspace root."},
                "glob": {"type": "string", "description": "Optional glob filter relative to path"},
                "output_mode": {"type": "string", "enum": ["content","files_with_matches","count"], "description": "Output mode. Defaults to files_with_matches."},
                "case_insensitive": {"type": "boolean"},
                "multiline": {"type": "boolean"},
                "context": {"type": "integer", "minimum": 0},
                "before_context": {"type": "integer", "minimum": 0},
                "after_context": {"type": "integer", "minimum": 0},
                "head_limit": {"type": "integer", "minimum": 0},
                "offset": {"type": "integer", "minimum": 0}
            },
            "required": ["pattern"],
            "additionalProperties": false
        })),
        param_template: Some(json!({"pattern": "{{pattern}}"})),
        effect_type: "read_filesystem".to_string(),
        preconditions: Some(
            "pattern required; path must be within the active workspace mount".into(),
        ),
        error_handling: Some(
            "invalid regex -> tool error; empty results -> empty list; output truncated at 1 MiB".into(),
        ),
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        source: "system".into(),
        validation_status: "validated".into(),
        capability_id: "builtin.grep".into(),
    }
}

fn tool_apply_patch_row(tenant: &str) -> NewPgTool {
    NewPgTool {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "apply_patch".to_string(),
        description: "Apply a targeted search-replace edit to a scoped file. Finds \
                       old_string in the file and replaces it with new_string. Exact \
                       match required by default; replace_all replaces every \
                       occurrence. Reads and writes through scoped mounts."
            .to_string(),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Scoped file path to patch"},
                "old_string": {"type": "string", "description": "Exact text to replace"},
                "new_string": {"type": "string", "description": "Replacement text"},
                "replace_all": {"type": "boolean", "description": "Replace every match instead of exactly one"}
            },
            "required": ["path", "old_string", "new_string"],
            "additionalProperties": false
        })),
        param_template: Some(json!({
            "path": "{{path}}", "old_string": "{{old_string}}", "new_string": "{{new_string}}"
        })),
        effect_type: "mixed".to_string(),
        preconditions: Some(
            "path within workspace mount scope; old_string must appear exactly once unless replace_all".into(),
        ),
        error_handling: Some(
            "old_string not found -> tool error; multiple matches without replace_all -> tool error".into(),
        ),
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        source: "system".into(),
        validation_status: "validated".into(),
        capability_id: "builtin.apply_patch".into(),
    }
}

// ---------------------------------------------------------------------------
// Row builders — ToolSkill (class 13). `content` is the canonical ToolSkill
// body ("Call `host.<tool>(...)` …"); the doc's Step X.2 blocks specify
// description / param_schema / preconditions / error_handling verbatim.
// ---------------------------------------------------------------------------

fn ts_read_file_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-read-file".to_string(),
        description: "Read a file from the scoped workspace via builtin.read_file. \
                       Optional range narrows to specific lines (format: start-end, \
                       e.g. '10-50')."
            .to_string(),
        content: "Call `host.read_file(path=<workspace path>, range=<optional 'start-end'>)` \
                  to read a file from the scoped workspace. Returns {content, line_count, path}. \
                  Omit `range` for a full read; supply a 1-based inclusive range (e.g. '10-50') \
                  to read a line span. Absolute host paths and traversal sequences (..) are \
                  rejected."
            .to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("read_file".to_string()),
        param_schema: Some(json!([
            {"name": "path", "param_type": "string", "required": true, "description": "Workspace-relative scoped path"},
            {"name": "range", "param_type": "string", "required": false, "description": "Line range start-end, e.g. '10-50'"}
        ])),
        param_template: Some(json!({"path": "{{path}}"})),
        consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

fn ts_write_file_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-write-file".to_string(),
        description: "Write or overwrite a file via builtin.write_file. Replaces the \
                       entire file content. Content limit: 6 MiB. Returns {path, \
                       bytes_written}."
            .to_string(),
        content: "Call `host.write_file(path=<workspace path>, content=<full file content>)` \
                  to create or overwrite a file. The entire file content is replaced (limit \
                  6 MiB). Returns {path, bytes_written}. Read the file first when replacing \
                  existing content; for partial edits prefer ts-apply-patch."
            .to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("write_file".to_string()),
        param_schema: Some(json!([
            {"name": "path", "param_type": "string", "required": true, "description": "Workspace-relative scoped path"},
            {"name": "content", "param_type": "string", "required": true, "description": "Complete new file content"}
        ])),
        param_template: Some(json!({"path": "{{path}}", "content": "{{content}}"})),
        consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

fn ts_list_dir_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-list-dir".to_string(),
        description: "Executor binding for list_dir. Lists directory contents through \
                       scoped mounts. Optional recursive flag and max_depth limit. path \
                       defaults to workspace root."
            .to_string(),
        content: "Call `host.list_dir(path=<optional scoped dir>, recursive=<bool>, \
                  max_depth=<int>)` to list directory contents through scoped mounts. \
                  Returns entry names, types (file/directory), and sizes. Omit `path` to \
                  default to the workspace root. Output is capped at 1 MiB."
            .to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("list_dir".to_string()),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Scoped directory path (omit for workspace root)"},
                "recursive": {"type": "boolean", "description": "Recurse into subdirectories"},
                "max_depth": {"type": "integer", "minimum": 0, "description": "Depth cap for recursive listing"}
            },
            "additionalProperties": false
        })),
        param_template: Some(json!({"path": "{{path}}"})),
        consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

fn ts_glob_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-glob".to_string(),
        description: "Executor binding for glob. Required: pattern (glob expression \
                       e.g. '**/*.rs'). Optional: path (scoped root, defaults to workspace \
                       root), max_results (cap on returned paths). Returns a list of \
                       matching paths sorted by modification time."
            .to_string(),
        content: "Call `host.glob(pattern=<glob expression>, path=<optional scoped root>, \
                  max_results=<optional int>)` to find files matching a glob pattern. \
                  Returns matching paths sorted by modification time (most recent first). \
                  Use `**` to search recursively; use `path` to restrict to a subdirectory."
            .to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("glob".to_string()),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "pattern": {"type": "string"},
                "path": {"type": "string"},
                "max_results": {"type": "integer", "minimum": 0}
            },
            "required": ["pattern"],
            "additionalProperties": false
        })),
        param_template: Some(json!({"pattern": "{{pattern}}"})),
        consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

fn ts_grep_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-grep".to_string(),
        description: "Executor binding for grep. Required: pattern (regex). Optional: \
                       path (scoped file or directory, defaults to workspace root), glob \
                       (file filter), output_mode (content | files_with_matches | count, \
                       default files_with_matches), case_insensitive, \
                       context/before_context/after_context (lines of context), head_limit \
                       (cap results), offset (pagination start)."
            .to_string(),
        content: "Call `host.grep(pattern=<regex>, path=<optional>, output_mode=<content|\
                  files_with_matches|count>, glob=<optional>, case_insensitive=<optional bool>, \
                  context=<optional int>, head_limit=<optional int>, offset=<optional int>)` \
                  to search file contents. Defaults to files_with_matches. Invalid regex is a \
                  tool error; no matches return an empty result (not an error)."
            .to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("grep".to_string()),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "pattern": {"type": "string"},
                "path": {"type": "string"},
                "glob": {"type": "string"},
                "output_mode": {"type": "string", "enum": ["content","files_with_matches","count"]},
                "case_insensitive": {"type": "boolean"},
                "multiline": {"type": "boolean"},
                "context": {"type": "integer", "minimum": 0},
                "before_context": {"type": "integer", "minimum": 0},
                "after_context": {"type": "integer", "minimum": 0},
                "head_limit": {"type": "integer", "minimum": 0},
                "offset": {"type": "integer", "minimum": 0}
            },
            "required": ["pattern"],
            "additionalProperties": false
        })),
        param_template: Some(json!({"pattern": "{{pattern}}"})),
        consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

fn ts_apply_patch_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-apply-patch".to_string(),
        description: "Executor binding for apply_patch. Required: path (scoped file), \
                       old_string (exact text to replace), new_string (replacement). \
                       Optional: replace_all (replaces every occurrence; default: exactly \
                       one match, error if multiple). old_string must include enough \
                       surrounding context to be unique in the file."
            .to_string(),
        content: "Call `host.apply_patch(path=<scoped file>, old_string=<exact text>, \
                  new_string=<replacement>, replace_all=<optional bool>)` to make a \
                  targeted search-replace edit. old_string must match exactly (including \
                  whitespace) and be unique unless replace_all is set. Read the file first \
                  with ts-read-file when uncertain of the exact current text."
            .to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("apply_patch".to_string()),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"},
                "old_string": {"type": "string"},
                "new_string": {"type": "string"},
                "replace_all": {"type": "boolean"}
            },
            "required": ["path", "old_string", "new_string"],
            "additionalProperties": false
        })),
        param_template: Some(json!({
            "path": "{{path}}", "old_string": "{{old_string}}", "new_string": "{{new_string}}"
        })),
        consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

// ---------------------------------------------------------------------------
// Row builders — PythonCode (class 22). The orchestrator executor bodies
// that drive every Tier-0 recipe. Bodies use `{{vars.slotN}}` (substituted
// by IBS before execution) and `host.<tool>(...)` dispatch. No imports, no
// I/O — pure sandbox dispatch. Transcribed verbatim from builtin_stuff_v3.md.
// ---------------------------------------------------------------------------

/// Build a `NewPgPythonCode` builtin row. `consumer_tags` omit `05:validator`
/// (builtins bypass Q1; the SEC-01 delivery filter hides `05:validator` rows
/// even when `validated`). The row is graduated to `validated` by
/// [`BootstrapStores::upsert_python_code`].
fn pc_row(tenant: &str, name: &str, description: &str, content: &str) -> NewPgPythonCode {
    NewPgPythonCode {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        content: content.to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        consumer_tags: vec!["01:monty".into(), "02:orchestrator".into()],
        intent_examples: None,
        source: "system".into(),
        dependency_registry: None,
        includes: vec![],
    }
}

/// consumer_tags shared by every filesystem leaf skill (class 1) — transcribed
/// verbatim from the doc. The skill store has no SEC-01 `05:validator`-hiding
/// filter (unlike `pg_python_code_store`), so carrying `05:validator` here is
/// safe and matches the doc source.
const LEAF_SKILL_TAGS: &[&str] = &["02:orchestrator", "05:validator"];

/// Build a `NewPgSkill` row from the variable parts. `intent_examples` is
/// `json!([])` because the doc's leaf/domain skill definitions carry no
/// intent examples (leaf skills are loaded via recipe steps, not direct intent
/// matching). `source`/`validation_status` are fixed to the builtin values.
fn skill_row(
    tenant: &str,
    name: &str,
    description: &str,
    body: &str,
    class_code: i16,
    consumer_tags: &[&str],
) -> NewPgSkill {
    NewPgSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        body: body.to_string(),
        class_code,
        consumer_tags: consumer_tags.iter().map(|s| s.to_string()).collect(),
        intent_examples: json!([]),
        source: "system".into(),
        validation_status: "validated".into(),
    }
}

/// Convenience: a class-1 leaf skill with the shared `LEAF_SKILL_TAGS`.
fn leaf_skill(tenant: &str, name: &str, description: &str, body: &str) -> NewPgSkill {
    skill_row(tenant, name, description, body, 1, LEAF_SKILL_TAGS)
}

/// Build one IBS `StepEntry` from a transcribed doc step. `knowledge` is the
/// doc's `channel` (`rust` | `orchestrator` | `both`); the doc's `llm` step
/// type (no channel) is passed as `knowledge = "orchestrator"`. `ty` is the
/// doc's `type` (`component` | `text` | `snippet`); the doc's `llm` type maps
/// to `"text"` (the IBS `RecipeStepType` has no `llm` variant — LLM invocation
/// is the recipe-level `llm_call_required` flag). `include` carries the real
/// seeded UUIDs that the doc's `<uuid:name>` placeholders resolved to.
/// `tool_bindings` is empty (the rust step pre-loads a ToolSkill via
/// `include`; the orchestrator step's `host.<tool>()` call lives in the
/// PythonCode body). `dependencies` is null.
fn step_entry(stepnumber: u32, knowledge: &str, goal: &str, ty: &str, include: &[Uuid]) -> Value {
    json!({
        "stepnumber": stepnumber,
        "knowledge": knowledge,
        "goal": goal,
        "content": goal,
        "type": ty,
        "include": include,
        "tool_bindings": [],
        "dependencies": null,
    })
}

/// Build a `NewPgRecipe` row from the doc's flat recipe format, adapted to the
/// IBS authoring model (Q1 decision A — composition-only):
/// - `step_descriptions` = one `StepDescriptionEntry` (`desc_idx: 0`,
///   `label` = recipe description, `yaml_source` = verbatim doc block, `steps`
///   = the supplied `StepEntry` array). The IBS reads `steps`; the WebUI
///   renderer reads `yaml_source`.
/// - `variants` = one synthesized default `RecipeVariant` whose `step_link`
///   is the canonical "run every step" formula `0:1-0:E` (desc_idx 0,
///   stepnumber 1 → End). This is the value `parse_step_link` expects — the
///   doc's flat recipes carry no variants/step_link, so a single whole-recipe
///   variant is synthesized. `match_variant` matches by exact string equality,
///   and Phase N graduates `intent_examples` into `reborn_intent_inputs`
///   carrying this same `step_link`, so the value round-trips intent → Monty →
///   compose → `build_instruction`.
/// - `intent_examples` (recipe top-level) preserves the doc's `{input, class}`
///   objects verbatim.
///
/// `consumer_tags` includes `05:validator` per the `NewPgRecipe` contract;
/// recipes are not subject to the SEC-01 delivery filter (only `pg_python_code`
/// is), so this does not hide the row.
fn recipe_row(
    tenant: &str,
    name: &str,
    description: &str,
    yaml_source: &str,
    step_entries: &[Value],
    intent_examples: &[Value],
) -> NewPgRecipe {
    let intent_input_strings: Vec<String> = intent_examples
        .iter()
        .filter_map(|e| e.get("input").and_then(|v| v.as_str()).map(str::to_string))
        .collect();
    NewPgRecipe {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        trigger: None,
        steps: json!([]),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        consumer_tags: vec!["02:orchestrator".into(), "05:validator".into()],
        intent_examples: Some(json!(intent_examples)),
        source: "system".into(),
        step_descriptions: Some(json!([{
            "desc_idx": 0,
            "label": description,
            "yaml_source": yaml_source,
            "steps": step_entries,
        }])),
        variants: Some(json!([{
            "variant_key": name,
            "step_link": "0:1-0:E",
            "description": description,
            "intent_examples": intent_input_strings,
            "variable_patterns": [],
        }])),
        dependency_registry: None,
    }
}

const PC_EXEC_READ_FILE_CONTENT: &str = r#"# Orchestrator executor body. host.<tool> is provided by the runtime sandbox.
# IBS bakes in path and range values as {{vars.slot0}} / {{vars.slot1}} before execution.
# No I/O, no imports — pure orchestrator dispatch.
_path = "{{vars.slot0}}"
_range = "{{vars.slot1}}"
_params = {"path": _path}
if _range and _range != "":
    _params["range"] = _range
result = host.read_file(**_params)
"#;

const PC_EXEC_WRITE_FILE_CONTENT: &str = r#"# Orchestrator executor body. host.<tool> is provided by the runtime sandbox.
# IBS bakes in path and content as {{vars.slot0}} / {{vars.slot1}} before execution.
# No I/O, no imports — pure orchestrator dispatch.
_path = "{{vars.slot0}}"
_content = "{{vars.slot1}}"
result = host.write_file(path=_path, content=_content)
"#;

const PC_EXEC_LIST_DIR_CONTENT: &str = r#"# Orchestrator executor body. host.<tool> provided by runtime sandbox.
# IBS bakes in path/recursive/max_depth as slot0/slot1/slot2.
_path = "{{vars.slot0}}"
_recursive = {{vars.slot1}}
_max_depth = {{vars.slot2}}
_params = {}
if _path and _path != "":
    _params["path"] = _path
if _recursive:
    _params["recursive"] = True
if _max_depth and _max_depth > 0:
    _params["max_depth"] = _max_depth
result = host.list_dir(**_params)
"#;

const PC_EXEC_GLOB_CONTENT: &str = r#"# Orchestrator executor body.
_pattern = "{{vars.slot0}}"
_path = "{{vars.slot1}}"
_max_results = {{vars.slot2}}
_params = {"pattern": _pattern}
if _path and _path != "":
    _params["path"] = _path
if _max_results and _max_results > 0:
    _params["max_results"] = _max_results
result = host.glob(**_params)
"#;

const PC_EXEC_GREP_CONTENT: &str = r#"# Orchestrator executor body.
_pattern = "{{vars.slot0}}"
_path = "{{vars.slot1}}"
_output_mode = "{{vars.slot2}}"
_glob = "{{vars.slot3}}"
_case_insensitive = {{vars.slot4}}
_params = {"pattern": _pattern}
if _path and _path != "":
    _params["path"] = _path
if _output_mode and _output_mode != "":
    _params["output_mode"] = _output_mode
if _glob and _glob != "":
    _params["glob"] = _glob
if _case_insensitive:
    _params["case_insensitive"] = True
result = host.grep(**_params)
"#;

const PC_EXEC_APPLY_PATCH_CONTENT: &str = r#"# Orchestrator executor body.
_path = "{{vars.slot0}}"
_old = "{{vars.slot1}}"
_new = "{{vars.slot2}}"
_replace_all = {{vars.slot3}}
_params = {"path": _path, "old_string": _old, "new_string": _new}
if _replace_all:
    _params["replace_all"] = True
result = host.apply_patch(**_params)
"#;

const PC_EXEC_GREP_CASE_INSENSITIVE_CONTENT: &str = r#"# Orchestrator executor body.
_pattern = "{{vars.slot0}}"
_path = "{{vars.slot1}}"
_output_mode = "{{vars.slot2}}"
_params = {"pattern": _pattern, "case_insensitive": True}
if _path and _path != "":
    _params["path"] = _path
if _output_mode and _output_mode != "":
    _params["output_mode"] = _output_mode
else:
    _params["output_mode"] = "files_with_matches"
result = host.grep(**_params)
"#;

const PC_EXEC_GREP_TYPE_FILTERED_CONTENT: &str = r#"# Orchestrator executor body.
_pattern = "{{vars.slot0}}"
_glob_filter = "{{vars.slot1}}"
_path = "{{vars.slot2}}"
_output_mode = "{{vars.slot3}}"
_params = {"pattern": _pattern, "glob": _glob_filter}
if _path and _path != "":
    _params["path"] = _path
if _output_mode and _output_mode != "":
    _params["output_mode"] = _output_mode
else:
    _params["output_mode"] = "files_with_matches"
result = host.grep(**_params)
"#;

const PC_EXEC_GREP_INVERT_CONTENT: &str = r#"# Orchestrator executor body.
_pattern = "{{vars.slot0}}"
_path = "{{vars.slot1}}"
_output_mode = "{{vars.slot2}}"
_params = {"pattern": _pattern, "invert_match": True}
if _path and _path != "":
    _params["path"] = _path
if _output_mode and _output_mode != "":
    _params["output_mode"] = _output_mode
else:
    _params["output_mode"] = "files_with_matches"
result = host.grep(**_params)
"#;

const PC_EXEC_LIST_FILTER_BY_TYPE_CONTENT: &str = r#"# No I/O, no imports. IBS bakes in entries and entry_type before execution.
# host.<tool> is NOT called here — this is a post-processing step.
_entries = {{vars.slot0}}
_entry_type = "{{vars.slot1}}"
if not isinstance(_entries, list):
    _entries = []
filtered = [e for e in _entries if isinstance(e, dict) and e.get("type") == _entry_type]
result = {"entries": filtered, "entry_type": _entry_type, "count": len(filtered)}
"#;

const PC_EXEC_READ_FILE_HEAD_CONTENT: &str = r#"# Pre-baked head variant: reads lines 1-50. No user input in range -> Tier 0 safe.
_path = "{{vars.slot0}}"
result = host.read_file(path=_path, range="1-50")
"#;

const PC_EXEC_READ_FILE_TAIL_CONTENT: &str = r#"# Tail variant: reads from line (total - 50) onward. First get line_count, then slice.
_path = "{{vars.slot0}}"
_info = host.read_file(path=_path, range="1-1")
_total = _info.get("line_count", 1) if isinstance(_info, dict) else 1
_start = max(1, _total - 49)
result = host.read_file(path=_path, range=str(_start) + "-" + str(_total))
"#;

const PC_EXEC_FILE_EXISTS_CONTENT: &str = r#"# Check existence by reading line 1. If tool returns an error, file doesn't exist.
_path = "{{vars.slot0}}"
try:
    _r = host.read_file(path=_path, range="1-1")
    result = {"exists": True, "path": _path, "line_count": _r.get("line_count", 0) if isinstance(_r, dict) else 0}
except Exception:
    result = {"exists": False, "path": _path}
"#;

const PC_EXEC_READ_THEN_GREP_CONTENT: &str = r#"# Read file content, then filter lines matching the pattern.
_path    = "{{vars.slot0}}"
_pattern = "{{vars.slot1}}"
_file_result = host.read_file(path=_path)
_content = _file_result.get("content", "") if isinstance(_file_result, dict) else str(_file_result)
_lines = _content.split("\n")
result = [_l for _l in _lines if _pattern in _l]
"#;

const PC_EXEC_LIST_THEN_GREP_CONTENT: &str = r#"# List directory, then filter entries by name substring.
_dir    = "{{vars.slot0}}"
_filter = "{{vars.slot1}}"
_list_result = host.list_dir(path=_dir)
_entries = _list_result if isinstance(_list_result, list) else (
    _list_result.get("entries", []) if isinstance(_list_result, dict) else []
)
result = [_e for _e in _entries if _filter in str(_e)]
"#;

const PC_PATH_JOIN_CONTENT: &str = r#"# Pure path join — no os.path import needed. Uses string concat with normalization.
_base = "{{vars.slot0}}".rstrip("/")
_sub  = "{{vars.slot1}}".lstrip("/")
result = (_base + "/" + _sub) if _sub else _base
"#;

const PC_PATH_BASENAME_CONTENT: &str = r#"# Pure basename — split on '/' and take the last non-empty component.
_path = "{{vars.slot0}}"
_parts = [p for p in _path.split("/") if p]
result = _parts[-1] if _parts else ""
"#;

const PC_PATH_DIRNAME_CONTENT: &str = r#"# Pure dirname — split on '/' and drop the last component.
_path = "{{vars.slot0}}"
_parts = [p for p in _path.split("/") if p]
result = "/" + "/".join(_parts[:-1]) if len(_parts) > 1 else "/"
"#;

// ---------------------------------------------------------------------------
// Filesystem Leaf Skill bodies (class 1) — transcribed verbatim from
// builtin_stuff_v3.md (the 2-space YAML `|` indent is stripped; trailing
// newline preserved). No body contains a `"#` sequence -> r#"..."# is safe.
// ---------------------------------------------------------------------------

const SKILL_READ_FILE_BODY: &str = r#"Use `ts-read-file` (via pc-exec-read-file) when you need to inspect a file's content.
Always read a file before editing it — never overwrite blindly.
For large files, use the `range` parameter (e.g. '1-100') to read specific line spans
rather than loading the entire file at once.
If the path is unknown, call skill-list-dir or skill-glob first to discover valid paths.
"#;

const SKILL_READ_FILE_RANGE_BODY: &str = r#"When a file is too large to read in full, use the `range` parameter of `ts-read-file`
(e.g. range='100-200') to read only the needed lines. Check `line_count` in the first
read result to know the file length, then paginate through sections. Each range call
returns only those lines. Use this pattern to avoid the 1 MiB output cap.
"#;

const SKILL_READ_FILE_HEAD_BODY: &str = r#"Use pc-exec-read-file-head to read the first 50 lines of a file without loading the
whole file. Useful for inspecting file headers, licence blocks, or configuration prefixes.
For a custom line count (other than 50), use skill-read-file-range with an explicit range.
"#;

const SKILL_READ_FILE_TAIL_BODY: &str = r#"Use pc-exec-read-file-tail to read the last 50 lines of a file without loading the whole file.
Useful for reading logs, recent entries, or the end of an append-only file.
The helper first probes line_count via a range='1-1' read, then fetches the tail window.
"#;

const SKILL_FILE_EXISTS_BODY: &str = r#"Use pc-exec-file-exists to probe whether a file exists before attempting a full read or
write. Returns {exists: bool, path}. Use this before skill-read-file to avoid surfacing
a 'file not found' error to the user when existence is uncertain. Also use before
skill-write-file-replace to confirm whether to overwrite or create-new.
"#;

const SKILL_WRITE_FILE_NEW_BODY: &str = r#"Use `ts-write-file` (via pc-exec-write-file) when creating a file that does not yet
exist. Provide the full intended content. The path must be within the scoped workspace
mount. The file is created immediately — there is no confirmation step unless the
orchestrator adds one.
"#;

const SKILL_WRITE_FILE_REPLACE_BODY: &str = r#"Use `ts-write-file` to completely replace a file's content when the entire file must
be rewritten. IMPORTANT: read the file first with skill-read-file before overwriting —
never discard existing content without seeing it. For small, targeted edits (a few lines),
prefer skill-apply-patch — it is safer because it requires matching the current content.
Use write_file only when you genuinely intend to replace the full content.
"#;

const SKILL_WRITE_FILE_TEMPLATE_BODY: &str = r#"Use `ts-write-file` (via pc-exec-write-file) when the content to write is fully
pre-determined and baked into the recipe vars by IBS — no LLM authorship needed.
Examples: creating an empty __init__.py, writing a fixed .gitignore stub, creating
a minimal config file with pre-set default values. The path and content both come
from vars, not from user input that needs interpretation.
"#;

const SKILL_LIST_DIR_BODY: &str = r#"Use `ts-list-dir` (via pc-exec-list-dir) to enumerate the files and folders in a
directory. Provide the scoped path; omit it to default to the workspace root. The
result includes entry names, types (file/directory), and sizes. Interpret and present
the entries relevant to the task. If the listing is large, summarise by grouping.
"#;

const SKILL_LIST_DIR_RECURSIVE_BODY: &str = r#"Use `ts-list-dir` with `recursive=true` and a `max_depth` limit when you need to see
the full subtree of a directory. Keep max_depth at 3 or less for large projects to
avoid output truncation. If the root listing is too large, narrow the path first.
For pattern-based searching, skill-glob is more precise.
"#;

const SKILL_LIST_DIR_FILES_ONLY_BODY: &str = r#"Use `ts-list-dir` and then filter the result with `pc-exec-list-filter-by-type` to
return only entries of type 'file'. This is useful when you want to process every file
in a directory without recursing, and you want to skip subdirectory entries. The
filter is applied in the PythonCode step after the list call returns.
"#;

const SKILL_LIST_DIR_DIRS_ONLY_BODY: &str = r#"Use `ts-list-dir` and then filter the result with `pc-exec-list-filter-by-type` to
return only entries of type 'directory'. This is useful when exploring the top-level
structure of a project (e.g. list only the immediate subdirectories of the repo root).
The filter is applied in the PythonCode step after the list call returns.
"#;

const SKILL_GLOB_BY_EXTENSION_BODY: &str = r#"Use `ts-glob` with a pattern like `**/*.rs` or `**/*.ts` to find all files of a given
extension across the workspace. The `**` prefix searches recursively into all
subdirectories. Use `path` to restrict the search to a specific subdirectory. Use
`max_results` when you only need a sample.
"#;

const SKILL_GLOB_BY_NAME_BODY: &str = r#"Use `ts-glob` with a pattern like `**/config*.toml` or `**/README*` to find files
whose names match a specific pattern. Combine `*` (any chars in one directory level)
and `**` (any number of directory levels) to build the right pattern. The results
are sorted by modification time — most recently changed first.
"#;

const SKILL_GLOB_IN_SUBDIR_BODY: &str = r#"Use `ts-glob` with the `path` parameter set to a specific subdirectory to restrict the
search scope (e.g. path='src/', pattern='**/*.test.ts'). This is faster and more
precise than a workspace-root glob when the files of interest are in a known subtree.
"#;

const SKILL_GREP_FILES_BODY: &str = r#"Use `ts-grep` with `output_mode='files_with_matches'` when you only need to know
WHICH files contain the pattern — not the matching lines. This is the fastest mode
and produces compact output. Use `glob` to restrict the file types searched (e.g.
glob='*.rs' to search only Rust files). Use `case_insensitive=true` when the match
should be case-independent.
"#;

const SKILL_GREP_CONTENT_BODY: &str = r#"Use `ts-grep` with `output_mode='content'` when you need the actual matching lines,
not just which files match. Add `context` (symmetric) or `before_context`/`after_context`
(asymmetric) to include surrounding lines — useful when the surrounding code helps
understand the match. Use `head_limit` to cap the number of results when the pattern
appears frequently. Use `offset` to paginate through large result sets.
"#;

const SKILL_GREP_COUNT_BODY: &str = r#"Use `ts-grep` with `output_mode='count'` when you only need to know how many times
a pattern appears, not the actual lines. This is efficient for large codebases where
you want a frequency signal (e.g. how many TODO comments exist) without reading all
the matching content. The result contains per-file counts.
"#;

const SKILL_GREP_CASE_INSENSITIVE_BODY: &str = r#"Use `ts-grep` with `case_insensitive=true` when the match should be case-independent
(e.g. searching for 'error' should also match 'Error', 'ERROR'). Combine with any
output_mode (files_with_matches, content, count). This is a distinct approach from
the default case-sensitive search — prefer this skill when the user says 'any case',
'case insensitive', or when the pattern contains mixed-case user input.
"#;

const SKILL_GREP_TYPE_FILTERED_BODY: &str = r#"Use `ts-grep` with the `glob` parameter to restrict the search to a specific file type
(e.g. glob='*.rs' to search only Rust files, glob='*.{ts,tsx}' for TypeScript). This
is more precise than a workspace-root grep and avoids noise from unrelated file types.
Combine with any output_mode. When the user specifies a file type in their search
intent, always use the glob filter — it reduces result noise significantly.
"#;

const SKILL_GREP_INVERT_BODY: &str = r#"Use `ts-grep` with `invert_match=true` when you need to find content that EXCLUDES a
pattern (e.g. source files without a copyright header, lines that are not comments,
configs missing a required key). The output returns non-matching entries. Combine with
output_mode='files_with_matches' to get the list of files without that pattern, or
'content' to get non-matching lines. Use pc-exec-grep-invert for execution.
"#;

const SKILL_APPLY_PATCH_SINGLE_BODY: &str = r#"Use `ts-apply-patch` with a unique `old_string` to replace exactly one occurrence of
text in a file. old_string must include enough surrounding lines (3–5) to be unambiguous.
If the string appears more than once, the tool will error — use skill-apply-patch-all
instead, or narrow old_string to include unique context. Always read the file first with
skill-read-file when uncertain of the exact current text.
"#;

const SKILL_APPLY_PATCH_ALL_BODY: &str = r#"Use `ts-apply-patch` with `replace_all=true` when the same string appears multiple
times and ALL occurrences should be changed (e.g. renaming a symbol throughout a file).
Verify the replacement is correct for ALL occurrences before dispatching — this is
irreversible without re-reading and re-patching.
"#;

const SKILL_READ_AND_GREP_BODY: &str = r#"Use pc-exec-read-then-grep when you need to find specific lines in a known file without
running a separate grep tool call. This is more efficient than read_file + grep as a
separate step for small-to-medium files. For large files or multi-file searches, prefer
skill-grep-content instead. Returns a list of matching lines.
"#;

const SKILL_LIST_AND_FILTER_BODY: &str = r#"Use pc-exec-list-then-grep when you need to enumerate a directory and immediately
narrow results by a name substring (e.g. "show me all Python files in src/"). This
avoids a separate glob call for simple substring name filters. For extension-based
filtering, prefer skill-glob-by-extension for exact extension matching.
"#;

/// `skill-filesystem` (class 2) domain-skill body — transcribed verbatim from
/// `builtin_stuff_v3.md` Step 7.x (the doc's `description:` field; the doc
/// omits a separate `body:` for this domain skill, so the full prose lives
/// here and a one-line summary is used as the row `description`).
const SKILL_FILESYSTEM_BODY: &str = r#"The filesystem domain provides six scoped tools for working with the workspace.
Decision guide — use the right skill for each approach:

READING:
— skill-read-file: Read a file's full content.
— skill-read-file-range: Read a specific line range from a large file.

LISTING / FINDING:
— skill-list-dir: List contents of a single directory level.
— skill-list-dir-recursive: Recursively scan a directory tree.
— skill-list-dir-files-only: List only regular files (no subdirs).
— skill-list-dir-dirs-only: List only subdirectories.
— skill-glob-by-extension: Find all files of a given extension.
— skill-glob-by-name: Find files whose names match a pattern.
— skill-glob-in-subdir: Restrict a glob to a specific subdirectory.

SEARCHING CONTENT:
— skill-grep-files: Find which files contain a pattern (fast, compact output).
— skill-grep-content: Retrieve matching lines with surrounding context.
— skill-grep-count: Count occurrences without returning content.
— skill-grep-case-insensitive: Case-insensitive grep (add case_insensitive=true).
— skill-grep-type-filtered: Grep only specific file types via glob filter.
— skill-grep-invert: Find files/lines that do NOT match (invert_match=true).

Decision for grep approach:
• Which files contain pattern → skill-grep-files
• What exactly matches with context → skill-grep-content
• How many occurrences → skill-grep-count
• Pattern in any case → skill-grep-case-insensitive
• Only in .rs / .ts / etc. files → skill-grep-type-filtered
• Files MISSING a pattern → skill-grep-invert

WRITING / EDITING:
— skill-write-file-new: Create a new file with full content.
— skill-write-file-replace: Replace an existing file's entire content.
— skill-write-file-template: Write a file from pre-baked template vars (no LLM).
— skill-apply-patch-single: Replace one unique occurrence in a file.
— skill-apply-patch-all: Replace every occurrence of a string in a file.

All paths are scoped to the workspace mount. Output is capped at 1 MiB per call.
"#;

// ---------------------------------------------------------------------------
// Filesystem recipe `yaml_source` — verbatim `step_descriptions` blocks from
// `builtin_stuff_v3.md`. The IBS never parses `yaml_source` (it reads the
// structured `steps` array built by `step_entry`); this is preserved for the
// WebUI authoring renderer.
// ---------------------------------------------------------------------------

const RECIPE_FILE_READ_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-read-file>"],
    "label":   "Pre-load ts-read-file ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-read-file>"],
    "label":   "PythonCode calls host.read_file(path, range) and returns result"
  }
]
"#;

const RECIPE_FILE_READ_RANGE_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-read-file>"],
    "label":   "Pre-load ts-read-file ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-read-file>"],
    "label":   "PythonCode calls host.read_file(path, range) — range slot is required"
  }
]
"#;

const RECIPE_FILE_WRITE_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-read-file>", "<uuid:skill-write-file-replace>", "<uuid:skill-write-file-new>"],
    "label":   "Load read + write leaf skill context"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-read-file>"],
    "label":   "Pre-load ts-read-file binding (for optional pre-read)"
  },
  {
    "step_id": "step-2",
    "type":    "llm",
    "label":   "LLM optionally reads current content, then composes new file content"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-write-file>"],
    "label":   "Pre-load ts-write-file binding"
  }
]
"#;

const RECIPE_FILE_WRITE_TEMPLATE_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-write-file>"],
    "label":   "Pre-load ts-write-file ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-write-file>"],
    "label":   "PythonCode calls host.write_file(path=slot0, content=slot1) — both pre-baked"
  }
]
"#;

const RECIPE_FILE_LIST_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-list-dir>"],
    "label":   "Pre-load ts-list-dir ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-list-dir>"],
    "label":   "PythonCode calls host.list_dir(path, recursive, max_depth)"
  }
]
"#;

const RECIPE_FILE_LIST_RECURSIVE_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-list-dir>"],
    "label":   "Pre-load ts-list-dir ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-list-dir>"],
    "label":   "PythonCode calls host.list_dir(path, recursive=true, max_depth=3)"
  }
]
"#;

const RECIPE_FILE_GLOB_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-glob>"],
    "label":   "Pre-load ts-glob ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-glob>"],
    "label":   "PythonCode calls host.glob(pattern, path, max_results)"
  }
]
"#;

const RECIPE_FILE_GLOB_BY_EXTENSION_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-glob>"],
    "label":   "Pre-load ts-glob ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-glob>"],
    "label":   "PythonCode calls host.glob(pattern='**/*.ext')"
  }
]
"#;

const RECIPE_FILE_GLOB_BY_NAME_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-glob>"],
    "label":   "Pre-load ts-glob ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-glob>"],
    "label":   "PythonCode calls host.glob(pattern='**/name-pattern')"
  }
]
"#;

const RECIPE_FILE_GLOB_IN_SUBDIR_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-glob>"],
    "label":   "Pre-load ts-glob ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-glob>"],
    "label":   "PythonCode calls host.glob(pattern, path=subdir)"
  }
]
"#;

const RECIPE_FILE_GLOB_RECENT_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-glob>"],
    "label":   "Pre-load ts-glob ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-glob>"],
    "label":   "PythonCode calls host.glob(pattern, max_results=10) — sorted by mtime"
  }
]
"#;
