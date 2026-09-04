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
    // apply_patch). Subsequent chunks add memory / process / management groups
    // here.
    seed_filesystem_group(&stores).await?;

    // Pass 2 — network group (http, http.save, web-search composition).
    seed_network_group(&stores).await?;

    // Pass 3 — memory group (memory_search, memory_write, memory_read,
    // memory_tree + search-and-read combined recipe).
    seed_memory_group(&stores).await?;

    // Pass 4 — process group (shell, spawn_subagent, trigger_create/list/remove).
    seed_process_group(&stores).await?;

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

    // 4g. Filesystem Grep Recipes (class 21) — 4 recipes, all Tier-0. One per
    //     output mode (default / files_with_matches / content / count). Each is
    //     a 2-step rust(preload ts-grep) + orchestrator(pc-exec-grep dispatch)
    //     program. Transcribed from the doc's flat format (Q1 decision A).
    let recipe_file_grep = stores
        .seed_recipe(
            &tenant,
            "file-grep",
            "Search file contents using a regular expression.",
            true,
            RECIPE_FILE_GREP_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-grep ToolSkill binding", "component", &[ts_grep]),
                step_entry(
                    2,
                    "orchestrator",
                    "PythonCode calls host.grep(pattern, path, output_mode, ...)",
                    "component",
                    &[pc_grep],
                ),
            ],
            &[
                json!({"input": "find all uses of function foo", "class": 1}),
                json!({"input": "search for TODO comments in src", "class": 1}),
                json!({"input": "which files import React", "class": 1}),
                json!({"input": "find all occurrences of FIXME", "class": 1}),
                json!({"input": "grep this pattern", "class": 1}),
                json!({"input": "search for this string in the codebase", "class": 1}),
                json!({"input": "find files containing this text", "class": 1}),
                json!({"input": "how many places use this function", "class": 2}),
            ],
        )
        .await?;
    let recipe_file_grep_files = stores
        .seed_recipe(
            &tenant,
            "file-grep-files",
            "Find which files contain a regex pattern (returns file paths only, no line content).",
            true,
            RECIPE_FILE_GREP_FILES_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-grep ToolSkill binding", "component", &[ts_grep]),
                step_entry(
                    2,
                    "orchestrator",
                    "PythonCode calls host.grep(pattern, output_mode='files_with_matches')",
                    "component",
                    &[pc_grep],
                ),
            ],
            &[
                json!({"input": "which files use this function", "class": 1}),
                json!({"input": "which files import this module", "class": 1}),
                json!({"input": "find files containing this string", "class": 1}),
                json!({"input": "which files have TODO", "class": 1}),
                json!({"input": "what files reference this constant", "class": 1}),
                json!({"input": "show me files with this error pattern", "class": 2}),
                json!({"input": "which .rs files contain async", "class": 2}),
                json!({"input": "find files matching this regex", "class": 1}),
                json!({"input": "list every file that has this keyword", "class": 1}),
                json!({"input": "show me all files that define this class", "class": 2}),
            ],
        )
        .await?;
    let recipe_file_grep_content = stores
        .seed_recipe(
            &tenant,
            "file-grep-content",
            "Search file contents and return matching lines with surrounding context.",
            true,
            RECIPE_FILE_GREP_CONTENT_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-grep ToolSkill binding", "component", &[ts_grep]),
                step_entry(
                    2,
                    "orchestrator",
                    "PythonCode calls host.grep(pattern, output_mode='content', context)",
                    "component",
                    &[pc_grep],
                ),
            ],
            &[
                json!({"input": "show me the lines that contain this error", "class": 1}),
                json!({"input": "find all uses of this function with context", "class": 1}),
                json!({"input": "search for this pattern and show surrounding code", "class": 1}),
                json!({"input": "grep with context lines", "class": 1}),
                json!({"input": "find this variable declaration", "class": 2}),
                json!({"input": "show matching lines in the source files", "class": 1}),
                json!({"input": "grep content mode", "class": 1}),
                json!({"input": "see the code around each match", "class": 2}),
                json!({"input": "show 3 lines before and after each match", "class": 2}),
                json!({"input": "grep and show surrounding context", "class": 1}),
            ],
        )
        .await?;
    let recipe_file_grep_count = stores
        .seed_recipe(
            &tenant,
            "file-grep-count",
            "Count occurrences of a pattern across files without returning the matching lines.",
            true,
            RECIPE_FILE_GREP_COUNT_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-grep ToolSkill binding", "component", &[ts_grep]),
                step_entry(
                    2,
                    "orchestrator",
                    "PythonCode calls host.grep(pattern, output_mode='count')",
                    "component",
                    &[pc_grep],
                ),
            ],
            &[
                json!({"input": "how many TODO comments are there", "class": 1}),
                json!({"input": "count occurrences of this pattern", "class": 1}),
                json!({"input": "how many times does this appear", "class": 1}),
                json!({"input": "count all uses of this function", "class": 2}),
                json!({"input": "how many errors in these log files", "class": 2}),
                json!({"input": "count grep matches", "class": 1}),
                json!({"input": "how many files contain this string", "class": 2}),
                json!({"input": "give me a count not the lines themselves", "class": 1}),
                json!({"input": "how many FIXME markers in the codebase", "class": 2}),
                json!({"input": "count how many times this import appears", "class": 2}),
            ],
        )
        .await?;

    // 4h. Filesystem Patch Recipes (class 21) — 2 recipes. file-patch is
    //     Tier-1 (LLM composes old_string/new_string; 4 steps with a `text`
    //     LLM-annotation step). file-patch-replace-all is Tier-0 (old/new
    //     strings pre-baked from vars; 2-step dispatch). Transcribed from the
    //     doc's flat format (Q1 decision A).
    let recipe_file_patch = stores
        .seed_recipe(
            &tenant,
            "file-patch",
            "Apply a targeted search-replace edit to a file.",
            false,
            RECIPE_FILE_PATCH_YAML,
            &[
                step_entry(
                    1,
                    "orchestrator",
                    "Load read + patch leaf skill context",
                    "component",
                    &[skill_read_file, skill_apply_patch_single],
                ),
                step_entry(2, "rust", "Pre-load ts-read-file binding", "component", &[ts_read_file]),
                step_entry(
                    3,
                    "orchestrator",
                    "LLM reads file, determines exact old_string and new_string for the change",
                    "text",
                    &[],
                ),
                step_entry(
                    4,
                    "rust",
                    "Pre-load ts-apply-patch binding",
                    "component",
                    &[ts_apply_patch],
                ),
            ],
            &[
                json!({"input": "fix this bug in the function", "class": 3}),
                json!({"input": "rename variable foo to bar in utils", "class": 3}),
                json!({"input": "update the default timeout value", "class": 2}),
                json!({"input": "replace the old error message", "class": 2}),
                json!({"input": "apply patch to file", "class": 2}),
                json!({"input": "edit this line in the file", "class": 2}),
                json!({"input": "change this string to something else", "class": 2}),
                json!({"input": "search and replace in this file", "class": 2}),
                json!({"input": "patch this specific section of the file", "class": 2}),
                json!({"input": "make a targeted edit to this file", "class": 2}),
            ],
        )
        .await?;
    let recipe_file_patch_replace_all = stores
        .seed_recipe(
            &tenant,
            "file-patch-replace-all",
            "Replace every occurrence of a pre-known string in a file (no LLM — vars pre-baked).",
            true,
            RECIPE_FILE_PATCH_REPLACE_ALL_YAML,
            &[
                step_entry(
                    1,
                    "rust",
                    "Pre-load ts-apply-patch ToolSkill binding",
                    "component",
                    &[ts_apply_patch],
                ),
                step_entry(
                    2,
                    "orchestrator",
                    "PythonCode calls host.apply_patch(path, old_string, new_string, replace_all=true)",
                    "component",
                    &[pc_apply_patch],
                ),
            ],
            &[
                json!({"input": "replace all occurrences of this string in the file", "class": 1}),
                json!({"input": "global find and replace in this file", "class": 1}),
                json!({"input": "replace every instance of this text", "class": 1}),
                json!({"input": "rename this symbol throughout the file", "class": 2}),
                json!({"input": "patch all occurrences", "class": 1}),
                json!({"input": "replace all matches in file", "class": 1}),
                json!({"input": "bulk replace in file", "class": 2}),
                json!({"input": "apply replace-all patch", "class": 1}),
                json!({"input": "change every occurrence of this value", "class": 2}),
            ],
        )
        .await?;

    // 4i. Gap-filler recipes (class 21) transcribed from the doc's Step 6.x /
    //     4.x / 2.x.2 sections. Their supporting PythonCode + leaf skills were
    //     minted in sections 3/4 above; this slot only adds the recipe rows.
    let recipe_file_grep_case_insensitive = stores
        .seed_recipe(
            &tenant,
            "file-grep-case-insensitive",
            "Search file contents case-insensitively using a regular expression.",
            true,
            RECIPE_FILE_GREP_CASE_INSENSITIVE_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-grep ToolSkill binding", "component", &[ts_grep]),
                step_entry(
                    2,
                    "orchestrator",
                    "PythonCode calls host.grep(pattern, case_insensitive=true, ...)",
                    "component",
                    &[pc_grep_case_insensitive],
                ),
            ],
            &[
                json!({"input": "find all uses of Error (any case)", "class": 1}),
                json!({"input": "case insensitive search for this pattern", "class": 1}),
                json!({"input": "find this word regardless of capitalisation", "class": 1}),
                json!({"input": "grep case insensitive", "class": 1}),
                json!({"input": "search for TODO ignoring case", "class": 2}),
                json!({"input": "case-insensitive regex search in the codebase", "class": 1}),
                json!({"input": "find 'config' in any capitalisation", "class": 2}),
                json!({"input": "grep -i for this pattern", "class": 1}),
                json!({"input": "search files case insensitively", "class": 1}),
            ],
        )
        .await?;
    let recipe_file_grep_type_filtered = stores
        .seed_recipe(
            &tenant,
            "file-grep-type-filtered",
            "Search only specific file types for a pattern using a glob file-type filter.",
            true,
            RECIPE_FILE_GREP_TYPE_FILTERED_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-grep ToolSkill binding", "component", &[ts_grep]),
                step_entry(
                    2,
                    "orchestrator",
                    "PythonCode calls host.grep(pattern, glob='*.ext', ...)",
                    "component",
                    &[pc_grep_type_filtered],
                ),
            ],
            &[
                json!({"input": "find this pattern only in .rs files", "class": 1}),
                json!({"input": "grep for this in TypeScript files only", "class": 1}),
                json!({"input": "search only Python files for this string", "class": 1}),
                json!({"input": "find this in .json config files", "class": 2}),
                json!({"input": "grep only Rust source files for this pattern", "class": 1}),
                json!({"input": "search in .ts and .tsx files", "class": 2}),
                json!({"input": "find this function only in test files", "class": 2}),
                json!({"input": "grep specific file extension for pattern", "class": 1}),
                json!({"input": "search only markdown files for this text", "class": 2}),
            ],
        )
        .await?;
    let recipe_file_grep_invert = stores
        .seed_recipe(
            &tenant,
            "file-grep-invert",
            "Find files or lines that do NOT contain a given pattern (inverted grep).",
            true,
            RECIPE_FILE_GREP_INVERT_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-grep ToolSkill binding", "component", &[ts_grep]),
                step_entry(
                    2,
                    "orchestrator",
                    "PythonCode calls host.grep(pattern, invert_match=true, ...)",
                    "component",
                    &[pc_grep_invert],
                ),
            ],
            &[
                json!({"input": "find files without this pattern", "class": 1}),
                json!({"input": "files missing this import", "class": 2}),
                json!({"input": "which files don't have a copyright header", "class": 2}),
                json!({"input": "invert grep — exclude matching lines", "class": 1}),
                json!({"input": "grep -v for this pattern", "class": 1}),
                json!({"input": "show lines that do not match", "class": 1}),
                json!({"input": "find files not containing this string", "class": 1}),
                json!({"input": "which source files lack this function", "class": 2}),
                json!({"input": "filter out lines matching this pattern", "class": 2}),
                json!({"input": "exclude files that have this keyword", "class": 2}),
            ],
        )
        .await?;
    let recipe_file_list_files_only = stores
        .seed_recipe(
            &tenant,
            "file-list-files-only",
            "List only regular files (no subdirectories) in a directory.",
            true,
            RECIPE_FILE_LIST_FILES_ONLY_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-list-dir ToolSkill binding", "component", &[ts_list_dir]),
                step_entry(
                    2,
                    "orchestrator",
                    "PythonCode: list_dir then filter entries to type='file'",
                    "component",
                    &[pc_list_dir, pc_list_filter_by_type],
                ),
            ],
            &[
                json!({"input": "list only the files in this directory", "class": 1}),
                json!({"input": "show me files without subdirectories", "class": 1}),
                json!({"input": "files only, no folders", "class": 1}),
                json!({"input": "list all files in this directory (no dirs)", "class": 1}),
                json!({"input": "what files are directly in the src folder", "class": 2}),
                json!({"input": "show only file entries not directories", "class": 1}),
                json!({"input": "list files in the project root", "class": 2}),
                json!({"input": "just the files please no subfolders", "class": 1}),
                json!({"input": "enumerate files in this folder", "class": 1}),
            ],
        )
        .await?;
    let recipe_file_list_dirs_only = stores
        .seed_recipe(
            &tenant,
            "file-list-dirs-only",
            "List only subdirectories (no regular files) in a directory.",
            true,
            RECIPE_FILE_LIST_DIRS_ONLY_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-list-dir ToolSkill binding", "component", &[ts_list_dir]),
                step_entry(
                    2,
                    "orchestrator",
                    "PythonCode: list_dir then filter entries to type='directory'",
                    "component",
                    &[pc_list_dir, pc_list_filter_by_type],
                ),
            ],
            &[
                json!({"input": "list only subdirectories", "class": 1}),
                json!({"input": "show me only the folders", "class": 1}),
                json!({"input": "directories only, no files", "class": 1}),
                json!({"input": "what subdirectories are in this folder", "class": 1}),
                json!({"input": "list only the immediate subdirs", "class": 1}),
                json!({"input": "show folder structure without files", "class": 2}),
                json!({"input": "list the top-level project directories", "class": 2}),
                json!({"input": "just folders no files", "class": 1}),
                json!({"input": "what are the child directories here", "class": 2}),
            ],
        )
        .await?;
    let recipe_file_read_head = stores
        .seed_recipe(
            &tenant,
            "file-read-head",
            "Read the first 50 lines of a file (head).",
            true,
            RECIPE_FILE_READ_HEAD_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-read-file ToolSkill binding", "component", &[ts_read_file]),
                step_entry(
                    2,
                    "orchestrator",
                    "PythonCode calls host.read_file(path, range='1-50')",
                    "component",
                    &[pc_read_file_head],
                ),
            ],
            &[
                json!({"input": "show me the top of this file", "class": 2}),
                json!({"input": "read the first few lines", "class": 1}),
                json!({"input": "show the beginning of the file", "class": 1}),
                json!({"input": "head of this file", "class": 1}),
                json!({"input": "first 50 lines", "class": 1}),
                json!({"input": "show me the file header", "class": 1}),
                json!({"input": "read the start of this file", "class": 2}),
                json!({"input": "show the top lines of this log", "class": 2}),
                json!({"input": "first lines of this config file", "class": 2}),
                json!({"input": "file head", "class": 1}),
            ],
        )
        .await?;
    let recipe_file_read_tail = stores
        .seed_recipe(
            &tenant,
            "file-read-tail",
            "Read the last 50 lines of a file (tail).",
            true,
            RECIPE_FILE_READ_TAIL_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-read-file ToolSkill binding", "component", &[ts_read_file]),
                step_entry(
                    2,
                    "orchestrator",
                    "PythonCode probes line_count then calls host.read_file(path, range=N-total)",
                    "component",
                    &[pc_read_file_tail],
                ),
            ],
            &[
                json!({"input": "show me the end of this file", "class": 2}),
                json!({"input": "tail of this file", "class": 1}),
                json!({"input": "read the last few lines", "class": 1}),
                json!({"input": "last 50 lines", "class": 1}),
                json!({"input": "show the bottom of the log", "class": 2}),
                json!({"input": "show recent log entries", "class": 2}),
                json!({"input": "read the end of this file", "class": 2}),
                json!({"input": "show latest lines in this log file", "class": 2}),
                json!({"input": "file tail", "class": 1}),
                json!({"input": "last lines of the file", "class": 1}),
            ],
        )
        .await?;
    let recipe_file_exists = stores
        .seed_recipe(
            &tenant,
            "file-exists",
            "Check whether a file exists at the given path.",
            true,
            RECIPE_FILE_EXISTS_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-read-file ToolSkill binding (used for existence probe)", "component", &[ts_read_file]),
                step_entry(
                    2,
                    "orchestrator",
                    "PythonCode tries reading line 1; returns {exists: bool, path}",
                    "component",
                    &[pc_file_exists],
                ),
            ],
            &[
                json!({"input": "does this file exist", "class": 1}),
                json!({"input": "check if a file exists", "class": 1}),
                json!({"input": "file exists check", "class": 1}),
                json!({"input": "does the path exist", "class": 1}),
                json!({"input": "is there a file at this path", "class": 2}),
                json!({"input": "check whether this path is valid", "class": 2}),
                json!({"input": "verify the file is present", "class": 2}),
                json!({"input": "file existence check", "class": 1}),
                json!({"input": "does config.toml exist", "class": 2}),
                json!({"input": "is this file present in the workspace", "class": 2}),
            ],
        )
        .await?;
    let recipe_file_read_and_grep = stores
        .seed_recipe(
            &tenant,
            "file-read-and-grep",
            "Read a file and return lines matching a pattern (combined read+filter, Tier 0).",
            true,
            RECIPE_FILE_READ_AND_GREP_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-read-file ToolSkill binding", "component", &[ts_read_file]),
                step_entry(
                    2,
                    "orchestrator",
                    "PythonCode reads file then filters lines matching vars.slot1 pattern",
                    "component",
                    &[pc_read_then_grep],
                ),
            ],
            &[
                json!({"input": "read this file and find lines with X", "class": 2}),
                json!({"input": "show me lines matching this pattern", "class": 2}),
                json!({"input": "read and filter this file", "class": 2}),
                json!({"input": "find matching lines in this file", "class": 2}),
                json!({"input": "search this file for a string", "class": 2}),
                json!({"input": "grep inside a specific file", "class": 2}),
                json!({"input": "file read and grep", "class": 1}),
                json!({"input": "read file and show matching lines only", "class": 2}),
                json!({"input": "filter lines in this log file", "class": 2}),
                json!({"input": "what lines in this file contain X", "class": 2}),
            ],
        )
        .await?;
    let recipe_file_list_and_filter = stores
        .seed_recipe(
            &tenant,
            "file-list-and-filter",
            "List a directory and return entries whose name contains a filter string.",
            true,
            RECIPE_FILE_LIST_AND_FILTER_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-list-dir ToolSkill binding", "component", &[ts_list_dir]),
                step_entry(
                    2,
                    "orchestrator",
                    "PythonCode lists directory then filters entries by name substring",
                    "component",
                    &[pc_list_then_grep],
                ),
            ],
            &[
                json!({"input": "list files containing test in the name", "class": 2}),
                json!({"input": "show only config files in this directory", "class": 2}),
                json!({"input": "find files with this name pattern", "class": 2}),
                json!({"input": "list and filter directory entries", "class": 2}),
                json!({"input": "show all test files", "class": 2}),
                json!({"input": "filter directory listing by name", "class": 2}),
                json!({"input": "file list filtered by name", "class": 1}),
                json!({"input": "list entries matching this substring", "class": 2}),
                json!({"input": "which files in this dir have this word", "class": 2}),
                json!({"input": "directory filtered listing", "class": 1}),
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
                recipe_file_read_head,
                recipe_file_read_tail,
                recipe_file_exists,
                recipe_file_read_and_grep,
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
                recipe_file_list_files_only,
                recipe_file_list_dirs_only,
                recipe_file_list_and_filter,
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
                recipe_file_grep,
                recipe_file_grep_files,
                recipe_file_grep_content,
                recipe_file_grep_count,
                recipe_file_grep_case_insensitive,
                recipe_file_grep_type_filtered,
                recipe_file_grep_invert,
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
                recipe_file_patch,
                recipe_file_patch_replace_all,
            ],
        )
        .await?;

    // 6. Append all filesystem tool + toolskill + python_code + leaf skill ids
    //    to the primary catalogue (path helpers are cross-capability → primary
    //    only), plus the filesystem domain skill + all 17 read/write/list/glob/
    //    grep/patch recipes. The filesystem group is now complete.
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
                recipe_file_grep,
                recipe_file_grep_files,
                recipe_file_grep_content,
                recipe_file_grep_count,
                recipe_file_grep_case_insensitive,
                recipe_file_grep_type_filtered,
                recipe_file_grep_invert,
                tool_apply_patch,
                ts_apply_patch,
                pc_apply_patch,
                skill_apply_patch_single,
                skill_apply_patch_all,
                recipe_file_patch,
                recipe_file_patch_replace_all,
                pc_path_join,
                pc_path_basename,
                pc_path_dirname,
                recipe_file_read,
                recipe_file_read_range,
                recipe_file_read_head,
                recipe_file_read_tail,
                recipe_file_exists,
                recipe_file_read_and_grep,
                recipe_file_write,
                recipe_file_write_template,
                recipe_file_list,
                recipe_file_list_recursive,
                recipe_file_list_files_only,
                recipe_file_list_dirs_only,
                recipe_file_list_and_filter,
                skill_filesystem,
            ],
        )
        .await?;

    tracing::debug!(catalogue_id = %cat_filesystem, "seeded filesystem group (chunk 3f: 6 base + 12 variant/helper PythonCode + 25 leaf skills + 1 domain skill + 27 read/write/list/glob/grep/patch recipes — complete)");
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

/// consumer_tags for spawn-subagent skills (class 1 + domain class 2) —
/// transcribed verbatim from the doc. Spawn/delegation skills are orchestrator-only
/// (the validator never delegates child runs), so `05:validator` is intentionally
/// absent, unlike `LEAF_SKILL_TAGS`.
const SPAWN_SKILL_TAGS: &[&str] = &["02:orchestrator"];

/// consumer_tags for the main trigger-management skills (list/create/remove leaf
/// skills + the skill-triggers domain) — transcribed verbatim from the doc. These
/// are orchestrator-only. Note the variant list skills (skill-trigger-list-active,
/// skill-trigger-list-scheduled) are an exception: the doc gives them the full
/// `LEAF_SKILL_TAGS` (`["02:orchestrator","05:validator"]`), so they use
/// `leaf_skill(...)` rather than this const.
const TRIGGER_SKILL_TAGS: &[&str] = &["02:orchestrator"];

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

const RECIPE_FILE_GREP_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-grep>"],
    "label":   "Pre-load ts-grep ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-grep>"],
    "label":   "PythonCode calls host.grep(pattern, path, output_mode, ...)"
  }
]
"#;

const RECIPE_FILE_GREP_FILES_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-grep>"],
    "label":   "Pre-load ts-grep ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-grep>"],
    "label":   "PythonCode calls host.grep(pattern, output_mode='files_with_matches')"
  }
]
"#;

const RECIPE_FILE_GREP_CONTENT_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-grep>"],
    "label":   "Pre-load ts-grep ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-grep>"],
    "label":   "PythonCode calls host.grep(pattern, output_mode='content', context)"
  }
]
"#;

const RECIPE_FILE_GREP_COUNT_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-grep>"],
    "label":   "Pre-load ts-grep ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-grep>"],
    "label":   "PythonCode calls host.grep(pattern, output_mode='count')"
  }
]
"#;

const RECIPE_FILE_PATCH_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-read-file>", "<uuid:skill-apply-patch-single>"],
    "label":   "Load read + patch leaf skill context"
  },
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-read-file>"],
    "label":   "Pre-load ts-read-file binding"
  },
  {
    "step_id": "step-2",
    "type":    "llm",
    "label":   "LLM reads file, determines exact old_string and new_string for the change"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-apply-patch>"],
    "label":   "Pre-load ts-apply-patch binding"
  }
]
"#;

const RECIPE_FILE_PATCH_REPLACE_ALL_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-apply-patch>"],
    "label":   "Pre-load ts-apply-patch ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-apply-patch>"],
    "label":   "PythonCode calls host.apply_patch(path, old_string, new_string, replace_all=true)"
  }
]
"#;

const RECIPE_FILE_GREP_CASE_INSENSITIVE_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-grep>"],
    "label":   "Pre-load ts-grep ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-grep-case-insensitive>"],
    "label":   "PythonCode calls host.grep(pattern, case_insensitive=true, ...)"
  }
]
"#;

const RECIPE_FILE_GREP_TYPE_FILTERED_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-grep>"],
    "label":   "Pre-load ts-grep ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-grep-type-filtered>"],
    "label":   "PythonCode calls host.grep(pattern, glob='*.ext', ...)"
  }
]
"#;

const RECIPE_FILE_GREP_INVERT_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-grep>"],
    "label":   "Pre-load ts-grep ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-grep-invert>"],
    "label":   "PythonCode calls host.grep(pattern, invert_match=true, ...)"
  }
]
"#;

const RECIPE_FILE_LIST_FILES_ONLY_YAML: &str = r#"step_descriptions: [
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
    "include": ["<uuid:pc-exec-list-dir>", "<uuid:pc-exec-list-filter-by-type>"],
    "label":   "PythonCode: list_dir then filter entries to type='file'"
  }
]
"#;

const RECIPE_FILE_LIST_DIRS_ONLY_YAML: &str = r#"step_descriptions: [
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
    "include": ["<uuid:pc-exec-list-dir>", "<uuid:pc-exec-list-filter-by-type>"],
    "label":   "PythonCode: list_dir then filter entries to type='directory'"
  }
]
"#;

const RECIPE_FILE_READ_HEAD_YAML: &str = r#"step_descriptions: [
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
    "include": ["<uuid:pc-exec-read-file-head>"],
    "label":   "PythonCode calls host.read_file(path, range='1-50')"
  }
]
"#;

const RECIPE_FILE_READ_TAIL_YAML: &str = r#"step_descriptions: [
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
    "include": ["<uuid:pc-exec-read-file-tail>"],
    "label":   "PythonCode probes line_count then calls host.read_file(path, range=N-total)"
  }
]
"#;

const RECIPE_FILE_EXISTS_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-read-file>"],
    "label":   "Pre-load ts-read-file ToolSkill binding (used for existence probe)"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-file-exists>"],
    "label":   "PythonCode tries reading line 1; returns {exists: bool, path}"
  }
]
"#;

const RECIPE_FILE_READ_AND_GREP_YAML: &str = r#"step_descriptions: [
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
    "include": ["<uuid:pc-exec-read-then-grep>"],
    "label":   "PythonCode reads file then filters lines matching vars.slot1 pattern"
  }
]
"#;

const RECIPE_FILE_LIST_AND_FILTER_YAML: &str = r#"step_descriptions: [
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
    "include": ["<uuid:pc-exec-list-then-grep>"],
    "label":   "PythonCode lists directory then filters entries by name substring"
  }
]
"#;

// ---------------------------------------------------------------------------
// Network group (Pass 2)
// ---------------------------------------------------------------------------

/// Network domain ExtensionCatalogue name (Step 23).
const CAT_NETWORK: &str = "builtin-network";

const CAT_EXT_HTTP_OVERVIEW: &str = r#"# HTTP Inline-Response Capability
Tool: builtin.http
Effect: network_egress
Permission: Ask

Issues HTTP requests (GET, POST, PUT, PATCH, DELETE, HEAD) and returns the response
inline (body capped at 256 KiB). For larger responses use ext-http-save.

Approaches:
- GET a URL: -> http-get recipe (Tier 0)
- GET JSON API: -> http-get-json recipe (Tier 0, Accept:application/json header)
- GET authenticated: -> http-authenticated-get recipe (Tier 0, Bearer token)
- HEAD (metadata only): -> http-head recipe (Tier 0)
- POST JSON body: -> http-post recipe (Tier 1, LLM composes body)
- POST webhook: -> http-post-json-webhook recipe (Tier 0, pre-structured body)
- PUT (replace resource): -> http-put recipe (Tier 1, LLM composes body)
- PATCH (partial update): -> http-patch recipe (Tier 1, LLM composes partial body)
- DELETE (remove resource): -> http-delete recipe (Tier 1, user confirmation required)
"#;

const CAT_EXT_HTTP_SAVE_OVERVIEW: &str = r#"# HTTP Save-to-File Capability
Tool: builtin.http.save
Effect: network_egress + write_filesystem
Permission: Ask

Issues an HTTP request and saves the response body to a scoped workspace file.
Use when the response exceeds 256 KiB or must be persisted for later processing.

Approaches:
- Download and save: url + save_to -> http-save recipe (Tier 0)
- Save large API response for parsing: url + save_to -> http-save recipe (Tier 0)
- Save with explicit large cap (5 MiB): -> http-save-large recipe (Tier 0)
"#;

const CAT_EXT_WEB_SEARCH_OVERVIEW: &str = r#"# Web Search Composition Capability
Tool: builtin.http (composed - no dedicated web_search capability)
Effect: network_egress (read)

Web search is a composed capability: builtin.http GET + JSON extraction.
A search API endpoint must be configured in the session scope first.

Approaches:
- Search the web: -> web-search recipe (Tier 1 - LLM formulates query, interprets results)
"#;

const CAT_NETWORK_OVERVIEW: &str = r#"# Network Capabilities

The network domain gives the agent structured HTTP access to external services.
All HTTP calls are subject to the session's outbound allowlist. Raw socket access
is not available - only HTTP(S) via the http and http.save tools.

## Tools in this domain
- builtin.http - issue an HTTP request and receive the response body inline
- builtin.http.save - issue an HTTP request and save the response body to a file

## Web search (composition)
Web search is not a separate tool - it is a composition of builtin.http + structured
JSON extraction (pc-web-search-extract). A search API endpoint must be configured
in the session scope before web search can be used.

## Constraints
- Response body cap: 15 MiB (builtin.http); same for http.save
- Default timeout: 10 s (connect) / 30 s (read)
- Redirect following: up to 5 hops
- Headers: set Accept and Content-Type explicitly for JSON APIs

## Scope and safety
- Outbound URLs are validated against the session's allowed-hosts list.
- POST requests with user-controlled bodies must be confirmed before sending.
- API keys in headers are resolved from the secrets layer - never hardcode them
  in recipe vars or PythonCode bodies.
"#;

/// Seed the network domain group (chunk 4a): the primary `builtin-network`
/// catalogue + 3 per-tool catalogues (`ext-http`, `ext-http-save`,
/// `ext-web-search`) + 2 Tool rows (`http`, `http.save`) + 3 ToolSkill rows
/// (`ts-http-fetch`, `ts-http-save`, `ts-web-search`).
///
/// PythonCode, leaf/domain Skills, and Recipes are added in chunks 4b–4d;
/// their ids are appended to the catalogues' `child_component_ids` as they
/// are minted (dedup makes this idempotent).
async fn seed_network_group(
    stores: &BootstrapStores,
) -> Result<(), SeedBuiltinBootstrapError> {
    let tenant = stores.tenant.clone();

    // 1. Primary domain catalogue + per-tool catalogues (empty child_ids;
    //    appended to as children are minted below and in later chunks).
    let cat_network = stores
        .upsert_catalogue(network_primary_catalogue_row(&tenant), CAT_NETWORK)
        .await?;
    let cat_http = stores
        .upsert_catalogue(
            ext_catalogue_row(
                &tenant,
                "ext-http",
                "HTTP inline-response capability (builtin.http).",
                CAT_EXT_HTTP_OVERVIEW,
                json!([
                    {"group_name": "http-get", "description": "GET requests (various auth/format variants)"},
                    {"group_name": "http-mutate", "description": "POST, PUT, DELETE requests"},
                    {"group_name": "http-head", "description": "HEAD requests for metadata/existence checks"}
                ]),
            ),
            "ext-http",
        )
        .await?;
    let cat_http_save = stores
        .upsert_catalogue(
            ext_catalogue_row(
                &tenant,
                "ext-http-save",
                "HTTP save-to-file capability (builtin.http.save).",
                CAT_EXT_HTTP_SAVE_OVERVIEW,
                json!([
                    {"group_name": "http-save-download", "description": "Download and save to workspace file"},
                    {"group_name": "http-save-api", "description": "Save large API response for later processing"}
                ]),
            ),
            "ext-http-save",
        )
        .await?;
    let cat_web_search = stores
        .upsert_catalogue(
            ext_catalogue_row(
                &tenant,
                "ext-web-search",
                "Web search composition capability (builtin.http + JSON extraction).",
                CAT_EXT_WEB_SEARCH_OVERVIEW,
                json!([
                    {"group_name": "web-search", "description": "Query a configured search API and extract results"}
                ]),
            ),
            "ext-web-search",
        )
        .await?;

    // 2. Tool rows (class 0) — capability_id taken from the live
    //    `*_CAPABILITY_ID` constant in `first_party_tools/http.rs`.
    let tool_http = stores.upsert_tool(tool_http_row(&tenant), "http").await?;
    let tool_http_save = stores
        .upsert_tool(tool_http_save_row(&tenant), "http.save")
        .await?;

    // 3. ToolSkill rows (class 13).
    let ts_http_fetch = stores
        .upsert_tool_skill(ts_http_fetch_row(&tenant), "ts-http-fetch")
        .await?;
    let ts_http_save = stores
        .upsert_tool_skill(ts_http_save_row(&tenant), "ts-http-save")
        .await?;
    let ts_web_search = stores
        .upsert_tool_skill(ts_web_search_row(&tenant), "ts-web-search")
        .await?;

    // 4. PythonCode rows (class 22) — orchestrator executors + pure-logic
    //    helpers. Transcribed verbatim from the doc (Q1 decision A); bodies
    //    use `host.<tool>(kwarg=value)` dispatch with `{{vars.slotN}}` vars.
    let pc_exec_http_get = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-http-get",
                "Orchestrator executor: calls host.<tool> for an HTTP GET request via \
                 builtin.http. Input: url (string), response_body_limit (optional int). \
                 Output: tool result with status, headers, body.",
                PC_EXEC_HTTP_GET_CONTENT,
            ),
            "pc-exec-http-get",
        )
        .await?;
    let pc_exec_http_post = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-http-post",
                "Orchestrator executor: calls host.<tool> for an HTTP POST request via \
                 builtin.http. Input: url (string), body (JSON value), headers (optional \
                 dict). Output: tool result with status, headers, body.",
                PC_EXEC_HTTP_POST_CONTENT,
            ),
            "pc-exec-http-post",
        )
        .await?;
    let pc_exec_http_save = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-http-save",
                "Orchestrator executor: calls host.<tool> for builtin.http.save. Input: \
                 url (string), save_to (string - scoped path). Output: metadata dict \
                 with status code and bytes_saved.",
                PC_EXEC_HTTP_SAVE_CONTENT,
            ),
            "pc-exec-http-save",
        )
        .await?;
    let pc_exec_http_patch = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-http-patch",
                "Orchestrator executor: calls host.<tool> for an HTTP PATCH request via \
                 builtin.http. Input: url (string), body (JSON value), headers (optional \
                 dict). Output: status + body.",
                PC_EXEC_HTTP_PATCH_CONTENT,
            ),
            "pc-exec-http-patch",
        )
        .await?;
    let pc_exec_http_head = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-http-head",
                "Orchestrator executor: calls host.<tool> for an HTTP HEAD request via \
                 builtin.http. Input: url (string). Output: status code and headers only.",
                PC_EXEC_HTTP_HEAD_CONTENT,
            ),
            "pc-exec-http-head",
        )
        .await?;
    let pc_exec_http_get_authenticated = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-http-get-authenticated",
                "Orchestrator executor: calls host.<tool> for an authenticated HTTP GET \
                 via builtin.http. Input: url (string), auth_header_value (string - full \
                 value for the Authorization header, e.g. 'Bearer <token>'). Output: \
                 status + body.",
                PC_EXEC_HTTP_GET_AUTHENTICATED_CONTENT,
            ),
            "pc-exec-http-get-authenticated",
        )
        .await?;
    let pc_exec_http_put = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-http-put",
                "Orchestrator executor: calls host.<tool> for an HTTP PUT request via \
                 builtin.http. Input: url (string), body (JSON value), headers (optional \
                 dict). Output: status + body.",
                PC_EXEC_HTTP_PUT_CONTENT,
            ),
            "pc-exec-http-put",
        )
        .await?;
    let pc_exec_http_delete = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-http-delete",
                "Orchestrator executor: calls host.<tool> for an HTTP DELETE request via \
                 builtin.http. Input: url (string), headers (optional dict with auth). \
                 Output: status + body.",
                PC_EXEC_HTTP_DELETE_CONTENT,
            ),
            "pc-exec-http-delete",
        )
        .await?;
    let pc_http_status_check = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-http-status-check",
                "Pure-logic helper: returns True when the HTTP status code indicates \
                 success (2xx range), False otherwise. Input: status_code (integer). \
                 Output: {is_success, status_code}.",
                PC_HTTP_STATUS_CHECK_CONTENT,
            ),
            "pc-http-status-check",
        )
        .await?;
    let pc_json_extract_field = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-json-extract-field",
                "Pure-logic helper: extracts a value from a JSON object by dot-separated \
                 path. Input: data (dict), path (dot-separated string e.g. \
                 'result.items.0'). Output: {value, path, found}.",
                PC_JSON_EXTRACT_FIELD_CONTENT,
            ),
            "pc-json-extract-field",
        )
        .await?;
    let pc_web_search_extract = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-web-search-extract",
                "PythonCode helper: extract title+url+snippet list from a search API JSON \
                 response. Input: response body string. Output: [{title, url, snippet}] \
                 or error.",
                PC_WEB_SEARCH_EXTRACT_CONTENT,
            ),
            "pc-web-search-extract",
        )
        .await?;
    let pc_web_search_query_build = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-web-search-query-build",
                "PythonCode helper: URL-encode a search query for embedding in an API URL. \
                 No imports - uses pure built-in percent-encoding (same as pc-url-encode). \
                 Input: raw query string. Output: {encoded, raw}.",
                PC_WEB_SEARCH_QUERY_BUILD_CONTENT,
            ),
            "pc-web-search-query-build",
        )
        .await?;
    let pc_url_encode = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-url-encode",
                "Pure-logic helper: URL-encodes a string (percent-encoding, spaces as \
                 %20). No imports - uses pure built-in character-by-character encoding. \
                 Input: raw string. Output: {encoded, raw}.",
                PC_URL_ENCODE_CONTENT,
            ),
            "pc-url-encode",
        )
        .await?;

    // 5. Leaf Skills (class 1) + Domain Skill (class 2). Transcribed verbatim
    //    from the doc. Leaf skills are loaded via recipe steps; the domain
    //    skill `skill-http` carries the HTTP method decision guide.
    let skill_http_get = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-http-get",
                "Leaf skill: how to fetch a URL via HTTP GET and receive the response \
                 inline.",
                SKILL_HTTP_GET_BODY,
                1,
                LEAF_SKILL_TAGS,
            ),
            "skill-http-get",
        )
        .await?;
    let skill_http_post = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-http-post",
                "Leaf skill: how to make an HTTP POST request with a body.",
                SKILL_HTTP_POST_BODY,
                1,
                LEAF_SKILL_TAGS,
            ),
            "skill-http-post",
        )
        .await?;
    let skill_http_authenticated = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-http-authenticated",
                "Leaf skill: how to make an authenticated HTTP request.",
                SKILL_HTTP_AUTHENTICATED_BODY,
                1,
                LEAF_SKILL_TAGS,
            ),
            "skill-http-authenticated",
        )
        .await?;
    let skill_http_save_download = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-http-save-download",
                "Leaf skill: how to download an HTTP response and save it to a file.",
                SKILL_HTTP_SAVE_DOWNLOAD_BODY,
                1,
                LEAF_SKILL_TAGS,
            ),
            "skill-http-save-download",
        )
        .await?;
    let skill_http_save_api = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-http-save-api",
                "Leaf skill: how to save a large API response for subsequent parsing.",
                SKILL_HTTP_SAVE_API_BODY,
                1,
                LEAF_SKILL_TAGS,
            ),
            "skill-http-save-api",
        )
        .await?;
    let skill_http_patch = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-http-patch",
                "Leaf skill: how to make an HTTP PATCH request for partial resource \
                 update.",
                SKILL_HTTP_PATCH_BODY,
                1,
                LEAF_SKILL_TAGS,
            ),
            "skill-http-patch",
        )
        .await?;
    let skill_http_head = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-http-head",
                "Leaf skill: how to make an HTTP HEAD request to check resource metadata.",
                SKILL_HTTP_HEAD_BODY,
                1,
                LEAF_SKILL_TAGS,
            ),
            "skill-http-head",
        )
        .await?;
    let skill_http_put = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-http-put",
                "Leaf skill: how to make an HTTP PUT request to replace a resource.",
                SKILL_HTTP_PUT_BODY,
                1,
                LEAF_SKILL_TAGS,
            ),
            "skill-http-put",
        )
        .await?;
    let skill_http_delete = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-http-delete",
                "Leaf skill: how to make an HTTP DELETE request to remove a resource.",
                SKILL_HTTP_DELETE_BODY,
                1,
                LEAF_SKILL_TAGS,
            ),
            "skill-http-delete",
        )
        .await?;
    let skill_web_search = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-web-search",
                "Leaf skill: how to perform a web search using builtin.http + JSON \
                 extraction.",
                SKILL_WEB_SEARCH_BODY,
                1,
                LEAF_SKILL_TAGS,
            ),
            "skill-web-search",
        )
        .await?;
    let skill_http = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-http",
                "The HTTP domain provides two tools for outbound HTTP requests (inline \
                 + save), plus a web-search composition.",
                SKILL_HTTP_BODY,
                2,
                LEAF_SKILL_TAGS,
            ),
            "skill-http",
        )
        .await?;

    // 6. Network Recipes (class 21) — transcribed from the doc's flat format
    //    into the IBS authoring model (Q1 decision A). Tier-0 recipes are
    //    deterministic 2-step dispatches (rust toolskill + orchestrator
    //    PythonCode); Tier-1 recipes add an LLM-annotation `text` step and load
    //    leaf-skill context first. step_link is synthesized as "0:1-0:E".
    let recipe_http_get = stores
        .seed_recipe(
            &tenant,
            "http-get",
            "Fetch a URL via HTTP GET and return the response.",
            true,
            RECIPE_HTTP_GET_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-http-fetch ToolSkill binding", "component", &[ts_http_fetch]),
                step_entry(2, "orchestrator", "PythonCode calls host.http(url, method=get)", "component", &[pc_exec_http_get]),
            ],
            &[
                json!({"input": "fetch this URL", "class": 1}),
                json!({"input": "GET https://api.example.com/data", "class": 1}),
                json!({"input": "download the JSON from this endpoint", "class": 1}),
                json!({"input": "make an HTTP GET request", "class": 1}),
                json!({"input": "check if this URL is reachable", "class": 1}),
                json!({"input": "fetch the contents of this page", "class": 1}),
                json!({"input": "HTTP GET this endpoint", "class": 1}),
                json!({"input": "call this REST API endpoint", "class": 2}),
                json!({"input": "retrieve data from this URL", "class": 1}),
                json!({"input": "ping this endpoint", "class": 2}),
            ],
        )
        .await?;
    let recipe_http_get_json = stores
        .seed_recipe(
            &tenant,
            "http-get-json",
            "Fetch a JSON API endpoint via HTTP GET with Accept: application/json header.",
            true,
            RECIPE_HTTP_GET_JSON_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-http-fetch ToolSkill binding", "component", &[ts_http_fetch]),
                step_entry(2, "orchestrator", "PythonCode calls host.http(url, method=get, headers={Accept:application/json})", "component", &[pc_exec_http_get]),
            ],
            &[
                json!({"input": "call this JSON API", "class": 1}),
                json!({"input": "fetch JSON from this endpoint", "class": 1}),
                json!({"input": "GET this REST API and parse JSON", "class": 1}),
                json!({"input": "retrieve JSON data from this URL", "class": 1}),
                json!({"input": "call the GitHub API", "class": 2}),
                json!({"input": "fetch the OpenAPI spec", "class": 2}),
                json!({"input": "GET this webhook URL and parse result", "class": 2}),
                json!({"input": "HTTP GET with JSON accept header", "class": 1}),
            ],
        )
        .await?;
    let recipe_http_post = stores
        .seed_recipe(
            &tenant,
            "http-post",
            "Send an HTTP POST request with a JSON body.",
            false,
            RECIPE_HTTP_POST_YAML,
            &[
                step_entry(1, "orchestrator", "Load http-post + auth leaf skill context", "component", &[skill_http_post, skill_http_authenticated]),
                step_entry(2, "orchestrator", "LLM constructs the POST URL, headers, and body from user instructions", "text", &[]),
                step_entry(3, "rust", "Pre-load ts-http-fetch binding", "component", &[ts_http_fetch]),
            ],
            &[
                json!({"input": "POST this data to the API", "class": 1}),
                json!({"input": "send a webhook notification", "class": 1}),
                json!({"input": "submit a form to this endpoint", "class": 2}),
                json!({"input": "call this API with a JSON body", "class": 2}),
                json!({"input": "create a GitHub issue via API", "class": 2}),
                json!({"input": "HTTP POST to this endpoint", "class": 1}),
                json!({"input": "send JSON payload to webhook", "class": 1}),
                json!({"input": "POST request with body", "class": 1}),
            ],
        )
        .await?;
    let recipe_http_save = stores
        .seed_recipe(
            &tenant,
            "http-save",
            "Fetch a URL and save the response body to a file.",
            true,
            RECIPE_HTTP_SAVE_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-http-save ToolSkill binding", "component", &[ts_http_save]),
                step_entry(2, "orchestrator", "PythonCode calls host.http.save(url, save_to)", "component", &[pc_exec_http_save]),
            ],
            &[
                json!({"input": "download this file and save it", "class": 1}),
                json!({"input": "fetch the API response and write to disk", "class": 1}),
                json!({"input": "save the download to workspace", "class": 1}),
                json!({"input": "GET this URL and save the result", "class": 1}),
                json!({"input": "download a large JSON response", "class": 1}),
                json!({"input": "save this URL response to a file", "class": 1}),
                json!({"input": "download the binary and store it", "class": 2}),
                json!({"input": "fetch and persist this large response", "class": 1}),
                json!({"input": "save API result to workspace file", "class": 1}),
            ],
        )
        .await?;
    let recipe_http_patch = stores
        .seed_recipe(
            &tenant,
            "http-patch",
            "Send an HTTP PATCH request to partially update a resource.",
            false,
            RECIPE_HTTP_PATCH_YAML,
            &[
                step_entry(1, "orchestrator", "Load http-patch + auth leaf skill context", "component", &[skill_http_patch, skill_http_authenticated]),
                step_entry(2, "orchestrator", "LLM constructs the PATCH URL, headers, and partial update body from user instructions", "text", &[]),
                step_entry(3, "rust", "Pre-load ts-http-fetch binding", "component", &[ts_http_fetch]),
            ],
            &[
                json!({"input": "partially update this resource via PATCH", "class": 1}),
                json!({"input": "PATCH request to update one field", "class": 1}),
                json!({"input": "send a PATCH to change the status field", "class": 2}),
                json!({"input": "HTTP PATCH this endpoint", "class": 1}),
                json!({"input": "update this resource partially via REST", "class": 2}),
                json!({"input": "patch this record with new values", "class": 2}),
                json!({"input": "PATCH the user email in the API", "class": 2}),
                json!({"input": "partial update via HTTP PATCH", "class": 1}),
            ],
        )
        .await?;
    let recipe_http_save_large = stores
        .seed_recipe(
            &tenant,
            "http-save-large",
            "Fetch a URL and save up to 5 MiB of the response body to a file.",
            true,
            RECIPE_HTTP_SAVE_LARGE_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-http-save ToolSkill binding", "component", &[ts_http_save]),
                step_entry(2, "orchestrator", "PythonCode calls host.http.save(url, save_to, response_body_limit=5242880)", "component", &[pc_exec_http_save]),
            ],
            &[
                json!({"input": "download a large file and save it", "class": 1}),
                json!({"input": "fetch a large API response and store it", "class": 1}),
                json!({"input": "download this dataset to workspace", "class": 2}),
                json!({"input": "save a large response up to 5MB", "class": 2}),
                json!({"input": "fetch and save this big response body", "class": 1}),
                json!({"input": "download and persist this large JSON dataset", "class": 2}),
                json!({"input": "http save large response", "class": 1}),
                json!({"input": "save 5mb response body to file", "class": 1}),
            ],
        )
        .await?;
    let recipe_http_head = stores
        .seed_recipe(
            &tenant,
            "http-head",
            "Send an HTTP HEAD request to check resource metadata (status + headers only).",
            true,
            RECIPE_HTTP_HEAD_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-http-fetch ToolSkill binding", "component", &[ts_http_fetch]),
                step_entry(2, "orchestrator", "PythonCode calls host.http(url, method='head')", "component", &[pc_exec_http_head]),
            ],
            &[
                json!({"input": "check if this URL exists", "class": 1}),
                json!({"input": "HEAD request to this endpoint", "class": 1}),
                json!({"input": "check if this resource is reachable", "class": 1}),
                json!({"input": "what content type does this URL return", "class": 2}),
                json!({"input": "check the headers of this URL without downloading", "class": 2}),
                json!({"input": "HTTP HEAD this URL", "class": 1}),
                json!({"input": "test if this API endpoint is up", "class": 2}),
                json!({"input": "check resource metadata without fetching body", "class": 1}),
                json!({"input": "is this URL reachable", "class": 1}),
            ],
        )
        .await?;
    let recipe_http_authenticated_get = stores
        .seed_recipe(
            &tenant,
            "http-authenticated-get",
            "Fetch a URL via HTTP GET with a Bearer token Authorization header.",
            true,
            RECIPE_HTTP_AUTHENTICATED_GET_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-http-fetch ToolSkill binding", "component", &[ts_http_fetch]),
                step_entry(2, "orchestrator", "PythonCode calls host.http(url, method=get, headers={Authorization:...})", "component", &[pc_exec_http_get_authenticated]),
            ],
            &[
                json!({"input": "call this API with my bearer token", "class": 1}),
                json!({"input": "authenticated GET request to this endpoint", "class": 1}),
                json!({"input": "fetch this URL with Authorization header", "class": 1}),
                json!({"input": "GET this private API endpoint", "class": 2}),
                json!({"input": "call this REST API using bearer auth", "class": 2}),
                json!({"input": "fetch the protected resource with my token", "class": 2}),
                json!({"input": "HTTP GET with bearer token", "class": 1}),
                json!({"input": "authenticated http get", "class": 1}),
                json!({"input": "call this endpoint with my API key as bearer", "class": 2}),
            ],
        )
        .await?;
    let recipe_http_put = stores
        .seed_recipe(
            &tenant,
            "http-put",
            "Send an HTTP PUT request to replace a resource at a URL.",
            false,
            RECIPE_HTTP_PUT_YAML,
            &[
                step_entry(1, "orchestrator", "Load http-put + auth leaf skill context", "component", &[skill_http_put, skill_http_authenticated]),
                step_entry(2, "orchestrator", "LLM constructs the PUT URL, headers, and replacement body from user instructions", "text", &[]),
                step_entry(3, "rust", "Pre-load ts-http-fetch binding", "component", &[ts_http_fetch]),
            ],
            &[
                json!({"input": "update this resource via PUT", "class": 1}),
                json!({"input": "PUT request to replace this record", "class": 1}),
                json!({"input": "replace this API resource via PUT", "class": 1}),
                json!({"input": "HTTP PUT to this endpoint", "class": 1}),
                json!({"input": "send a PUT request with this body", "class": 2}),
                json!({"input": "update this REST resource", "class": 2}),
                json!({"input": "PUT to update this configuration", "class": 2}),
                json!({"input": "replace the document via REST PUT", "class": 2}),
            ],
        )
        .await?;
    let recipe_http_delete = stores
        .seed_recipe(
            &tenant,
            "http-delete",
            "Send an HTTP DELETE request to remove a resource, with user confirmation.",
            false,
            RECIPE_HTTP_DELETE_YAML,
            &[
                step_entry(1, "orchestrator", "Load http-delete + auth leaf skill context", "component", &[skill_http_delete, skill_http_authenticated]),
                step_entry(2, "orchestrator", "LLM confirms target URL with user (ExternalWrite - irreversible), then calls ts-http-fetch", "text", &[]),
                step_entry(3, "rust", "Pre-load ts-http-fetch binding", "component", &[ts_http_fetch]),
            ],
            &[
                json!({"input": "delete this resource via REST API", "class": 1}),
                json!({"input": "send a DELETE request to this endpoint", "class": 1}),
                json!({"input": "HTTP DELETE this record", "class": 1}),
                json!({"input": "remove this resource via REST", "class": 2}),
                json!({"input": "delete this API entry", "class": 2}),
                json!({"input": "call DELETE on this endpoint", "class": 1}),
                json!({"input": "destroy this resource via HTTP", "class": 2}),
                json!({"input": "DELETE request to remove this item", "class": 1}),
            ],
        )
        .await?;
    let recipe_http_post_json_webhook = stores
        .seed_recipe(
            &tenant,
            "http-post-json-webhook",
            "Send a JSON webhook POST notification to a pre-configured URL.",
            true,
            RECIPE_HTTP_POST_JSON_WEBHOOK_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-http-fetch ToolSkill binding", "component", &[ts_http_fetch]),
                step_entry(2, "orchestrator", "PythonCode calls host.http(url, method=post, body={event, payload}, headers={Content-Type:application/json})", "component", &[pc_exec_http_post]),
            ],
            &[
                json!({"input": "send a webhook notification", "class": 1}),
                json!({"input": "post a JSON event to this webhook URL", "class": 1}),
                json!({"input": "fire a webhook with this payload", "class": 1}),
                json!({"input": "send a webhook alert", "class": 2}),
                json!({"input": "notify the webhook endpoint", "class": 1}),
                json!({"input": "trigger the webhook with a JSON body", "class": 1}),
                json!({"input": "post event to webhook", "class": 1}),
                json!({"input": "send a hook notification to this URL", "class": 2}),
                json!({"input": "call the webhook endpoint with JSON", "class": 1}),
            ],
        )
        .await?;
    let recipe_web_search = stores
        .seed_recipe(
            &tenant,
            "web-search",
            "Search the web via a configured HTTP search API.",
            false,
            RECIPE_WEB_SEARCH_YAML,
            &[
                step_entry(1, "orchestrator", "Load skill-web-search leaf skill body (composition pattern)", "component", &[skill_web_search]),
                step_entry(2, "orchestrator", "Load PythonCode helpers for query encoding and result extraction", "component", &[pc_web_search_query_build, pc_web_search_extract]),
                step_entry(3, "orchestrator", "LLM formulates query, calls ts-http-get, extracts results, presents to user", "text", &[]),
                step_entry(4, "rust", "Pre-load ts-http-fetch ToolSkill binding (used for both search and follow-up fetches)", "component", &[ts_http_fetch]),
            ],
            &[
                json!({"input": "search the web for X", "class": 1}),
                json!({"input": "look up X online", "class": 1}),
                json!({"input": "find information about X", "class": 1}),
                json!({"input": "what is the latest news on X", "class": 1}),
                json!({"input": "google X for me", "class": 2}),
                json!({"input": "web search", "class": 1}),
                json!({"input": "search online for this topic", "class": 1}),
                json!({"input": "find recent articles about X", "class": 2}),
                json!({"input": "internet search for X", "class": 1}),
                json!({"input": "find the official docs for X", "class": 2}),
            ],
        )
        .await?;

    // 7. Append children to the per-tool catalogues (dedup-idempotent).
    let ext_http_children: Vec<Uuid> = vec![
        tool_http, ts_http_fetch, pc_exec_http_get, pc_exec_http_get_authenticated,
        pc_exec_http_post, pc_exec_http_head, pc_exec_http_put, pc_exec_http_patch,
        pc_exec_http_delete, pc_http_status_check, pc_json_extract_field,
        skill_http_get, skill_http_post, skill_http_authenticated, skill_http_head,
        skill_http_put, skill_http_patch, skill_http_delete, recipe_http_get,
        recipe_http_get_json, recipe_http_authenticated_get, recipe_http_head,
        recipe_http_post, recipe_http_post_json_webhook, recipe_http_put,
        recipe_http_patch, recipe_http_delete, skill_http,
    ];
    let ext_http_save_children: Vec<Uuid> = vec![
        tool_http_save, ts_http_save, pc_exec_http_save, skill_http_save_download,
        skill_http_save_api, recipe_http_save, recipe_http_save_large,
    ];
    let ext_web_search_children: Vec<Uuid> = vec![
        ts_web_search, pc_web_search_extract, pc_web_search_query_build, pc_url_encode,
        skill_web_search, recipe_web_search,
    ];
    stores.append_children(cat_http, &ext_http_children).await?;
    stores.append_children(cat_http_save, &ext_http_save_children).await?;
    stores.append_children(cat_web_search, &ext_web_search_children).await?;
    // Primary catalogue owns the union of all three per-tool child sets.
    stores.append_children(cat_network, &ext_http_children).await?;
    stores.append_children(cat_network, &ext_http_save_children).await?;
    stores.append_children(cat_network, &ext_web_search_children).await?;

    tracing::debug!(
        catalogue_id = %cat_network,
        "seeded network group chunk 4d: 12 recipes (7 Tier-0 + 5 Tier-1) + catalogue appends - network group COMPLETE (2 tools + 3 toolskills + 13 PythonCode + 11 skills + 12 recipes + 4 catalogues)"
    );

    Ok(())
}

fn network_primary_catalogue_row(tenant: &str) -> NewPgExtensionCatalogue {
    NewPgExtensionCatalogue {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: CAT_NETWORK.to_string(),
        description: "Network domain capability catalogue (http, http.save, web-search \
                       composition)."
            .to_string(),
        version: "1.0".into(),
        overview_doc: CAT_NETWORK_OVERVIEW.into(),
        task_groups: json!([
            {"group_name": "http-fetch", "description": "GET and POST requests with inline response body"},
            {"group_name": "http-download", "description": "Requests that save the response body to a file"},
            {"group_name": "web-search", "description": "Search API composition (http + JSON extraction)"}
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

fn tool_http_row(tenant: &str) -> NewPgTool {
    NewPgTool {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "http".to_string(),
        description: "Perform an HTTP or HTTPS request and return the response inline. \
                       Supports GET, POST, PUT, PATCH, DELETE, HEAD. Response body capped \
                       at 256 KiB inline; larger responses should use builtin.http.save."
            .to_string(),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "url": {"type": "string", "description": "Absolute HTTP or HTTPS URL"},
                "method": {"type": "string", "enum": ["get","post","put","patch","delete","head"],
                           "description": "HTTP method. Defaults to get."},
                "headers": {"description": "HTTP headers as an object or [{name,value}] array"},
                "body": {"description": "String or JSON request body"},
                "body_base64": {"type": "string", "description": "Base64-encoded request body"},
                "response_body_limit": {"type": "integer", "minimum": 1, "maximum": 262144,
                            "description": "Max inline response bytes, capped at 256 KiB."},
                "timeout_ms": {"type": "integer", "minimum": 1, "maximum": 30000, "default": 10000}
            },
            "required": ["url"],
            "additionalProperties": false
        })),
        param_template: Some(json!({"url": "{{url}}"})),
        effect_type: "network_egress".to_string(),
        preconditions: Some(
            "url must be absolute http/https; network egress must be permitted by policy".into(),
        ),
        error_handling: Some(
            "connection failure -> tool error; body over limit -> truncated with guidance; \
             non-2xx -> in output (not a tool error)"
                .into(),
        ),
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        source: "system".into(),
        validation_status: "validated".into(),
        capability_id: "builtin.http".into(),
    }
}

fn tool_http_save_row(tenant: &str) -> NewPgTool {
    NewPgTool {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "http.save".to_string(),
        description: "Perform an HTTP or HTTPS request and save the sanitized response body \
                       to a scoped file path. Accepts up to 10 MiB of response body. Used when \
                       the response is too large for inline delivery or must be persisted."
            .to_string(),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "url": {"type": "string", "description": "Absolute HTTP or HTTPS URL"},
                "save_to": {"type": "string", "description": "Scoped path to save the response body"},
                "method": {"type": "string", "enum": ["get","post","put","patch","delete","head"]},
                "headers": {"description": "HTTP headers as an object or [{name,value}] array"},
                "body": {"description": "String or JSON request body"},
                "body_base64": {"type": "string"},
                "response_body_limit": {"type": "integer", "minimum": 1, "maximum": 10485760,
                            "description": "Max response body bytes to save. Default 10 MiB."},
                "timeout_ms": {"type": "integer", "minimum": 1, "maximum": 30000, "default": 10000}
            },
            "required": ["url", "save_to"],
            "additionalProperties": false
        })),
        param_template: Some(json!({"url": "{{url}}", "save_to": "{{save_to}}"})),
        effect_type: "mixed".to_string(),
        preconditions: Some(
            "url must be absolute http/https; save_to must be within workspace mount".into(),
        ),
        error_handling: Some(
            "connection failure -> tool error; save_to outside mount -> tool error".into(),
        ),
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        source: "system".into(),
        validation_status: "validated".into(),
        capability_id: "builtin.http.save".into(),
    }
}

fn ts_http_fetch_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-http-fetch".to_string(),
        description: "Executor binding for builtin.http. Required: url. Optional: method \
                       (default get), headers, body, body_base64, response_body_limit (max \
                       256 KiB), timeout_ms (max 30 000). Non-2xx status codes are returned \
                       in output - not errors."
            .to_string(),
        content: "Call `host.http(url=<absolute url>, method=<get|post|put|patch|delete|head>, \
                  headers=<optional>, body=<optional>, body_base64=<optional>, \
                  response_body_limit=<optional 1..262144>, timeout_ms=<optional 1..30000>)` \
                  to issue an outbound HTTP request and receive the response inline. The \
                  inline body is capped at 256 KiB; use ts-http-save for larger responses. \
                  Non-2xx status codes appear in the result's status field - they are not \
                  tool errors. Always inspect the status code after the call."
            .to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("http".to_string()),
        param_schema: Some(json!([
            {"name": "url", "param_type": "string", "required": true, "description": "Absolute http/https URL"},
            {"name": "method", "param_type": "string", "required": false, "description": "HTTP method (default get)"},
            {"name": "headers", "param_type": "object", "required": false, "description": "HTTP headers object or [{name,value}]"},
            {"name": "body", "param_type": "string", "required": false, "description": "String or JSON request body"},
            {"name": "body_base64", "param_type": "string", "required": false, "description": "Base64-encoded request body"},
            {"name": "response_body_limit", "param_type": "integer", "required": false, "description": "Max inline response bytes (1..262144)"},
            {"name": "timeout_ms", "param_type": "integer", "required": false, "description": "Request timeout ms (1..30000)"}
        ])),
        param_template: Some(json!({"url": "{{url}}"})),
        consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

fn ts_http_save_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-http-save".to_string(),
        description: "Executor binding for builtin.http.save. Required: url, save_to (scoped \
                       path). Optional: method, headers, body, body_base64, \
                       response_body_limit (default and max 10 MiB), timeout_ms (max 30 000). \
                       Returns metadata (status, bytes_saved)."
            .to_string(),
        content: "Call `host.http.save(url=<absolute url>, save_to=<scoped path>, \
                  method=<get|post|put|patch|delete|head>, headers=<optional>, \
                  body=<optional>, body_base64=<optional>, response_body_limit=<optional \
                  1..10485760>, timeout_ms=<optional 1..30000>)` to issue an outbound HTTP \
                  request and save the sanitized response body to a workspace file. Use this \
                  when the response exceeds 256 KiB or must be persisted. Returns metadata \
                  with the status code and bytes_saved - inspect the status field."
            .to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("http.save".to_string()),
        param_schema: Some(json!([
            {"name": "url", "param_type": "string", "required": true, "description": "Absolute http/https URL"},
            {"name": "save_to", "param_type": "string", "required": true, "description": "Scoped workspace path to save response body"},
            {"name": "method", "param_type": "string", "required": false, "description": "HTTP method (default get)"},
            {"name": "headers", "param_type": "object", "required": false, "description": "HTTP headers object or [{name,value}]"},
            {"name": "body", "param_type": "string", "required": false, "description": "String or JSON request body"},
            {"name": "body_base64", "param_type": "string", "required": false, "description": "Base64-encoded request body"},
            {"name": "response_body_limit", "param_type": "integer", "required": false, "description": "Max response body bytes to save (1..10485760)"},
            {"name": "timeout_ms", "param_type": "integer", "required": false, "description": "Request timeout ms (1..30000)"}
        ])),
        param_template: Some(json!({"url": "{{url}}", "save_to": "{{save_to}}"})),
        consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

fn ts_web_search_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-web-search".to_string(),
        description: "ToolSkill: web search via HTTP + structured extraction composition."
            .to_string(),
        content: "Tool used: builtin.http (no dedicated builtin.web_search capability \
                  exists). Effect: Read - issues an HTTP GET to a search API endpoint, \
                  extracts results.\n\nComposition pattern:\n\
                  1. Use builtin.http to GET a search API endpoint (e.g. DuckDuckGo Instant \
                  Answer API, SerpAPI, a configured search provider endpoint).\n\
                  2. The response body is JSON. Use pc-json-extract-field (or a local \
                  PythonCode step) to extract the relevant results array from the response.\n\
                  3. Filter, rank, or summarize the results as needed.\n\n\
                  Parameter guidance:\n\
                  - url: the search API endpoint, with the query embedded as a URL param.\n\
                  - headers: include 'Accept: application/json' and any required API key \
                  header.\n\
                  - method: always GET for search.\n\n\
                  Constraints:\n\
                  - The agent has no built-in search engine - it must use a configured \
                  search API. If no search API is configured in the current scope, inform \
                  the user.\n\
                  - Respect the 15 MiB response cap from builtin.http.\n\
                  - Do not embed raw user PII in search queries without consent."
            .to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("http".to_string()),
        param_schema: Some(json!([
            {"name": "url", "param_type": "string", "required": true, "description": "Search API endpoint URL with query embedded"},
            {"name": "headers", "param_type": "object", "required": false, "description": "Accept: application/json + API key header"},
            {"name": "method", "param_type": "string", "required": false, "description": "Always GET for search"}
        ])),
        param_template: Some(json!({"url": "{{url}}"})),
        consumer_tags: vec!["02:orchestrator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

// ---------------------------------------------------------------------------
// Network group PythonCode bodies (class 22) — transcribed verbatim from the
// doc. Bodies use `host.<tool>(kwarg=value)` dispatch with `{{vars.slotN}}`
// vars; no imports.
// ---------------------------------------------------------------------------

const PC_EXEC_HTTP_GET_CONTENT: &str = r#"# Orchestrator executor body.
_url = "{{vars.slot0}}"
_limit = {{vars.slot1}}
_params = {"url": _url, "method": "get"}
if _limit and _limit > 0:
    _params["response_body_limit"] = _limit
result = host.http(**_params)
"#;

const PC_EXEC_HTTP_POST_CONTENT: &str = r#"# Orchestrator executor body.
_url = "{{vars.slot0}}"
_body = {{vars.slot1}}
_headers = {{vars.slot2}}
_params = {"url": _url, "method": "post", "body": _body}
if _headers:
    _params["headers"] = _headers
result = host.http(**_params)
"#;

const PC_EXEC_HTTP_SAVE_CONTENT: &str = r#"# Orchestrator executor body.
_url = "{{vars.slot0}}"
_save_to = "{{vars.slot1}}"
result = host.http.save(url=_url, save_to=_save_to)
"#;

const PC_EXEC_HTTP_PATCH_CONTENT: &str = r#"# Orchestrator executor body.
_url = "{{vars.slot0}}"
_body = {{vars.slot1}}
_headers = {{vars.slot2}}
_params = {"url": _url, "method": "patch", "body": _body}
if _headers:
    _params["headers"] = _headers
result = host.http(**_params)
"#;

const PC_EXEC_HTTP_HEAD_CONTENT: &str = r#"# Orchestrator executor body.
_url = "{{vars.slot0}}"
result = host.http(url=_url, method="head")
"#;

const PC_EXEC_HTTP_GET_AUTHENTICATED_CONTENT: &str = r#"# Orchestrator executor body.
_url = "{{vars.slot0}}"
_auth = "{{vars.slot1}}"
_params = {"url": _url, "method": "get", "headers": {"Authorization": _auth}}
result = host.http(**_params)
"#;

const PC_EXEC_HTTP_PUT_CONTENT: &str = r#"# Orchestrator executor body.
_url = "{{vars.slot0}}"
_body = {{vars.slot1}}
_headers = {{vars.slot2}}
_params = {"url": _url, "method": "put", "body": _body}
if _headers:
    _params["headers"] = _headers
result = host.http(**_params)
"#;

const PC_EXEC_HTTP_DELETE_CONTENT: &str = r#"# Orchestrator executor body.
_url = "{{vars.slot0}}"
_headers = {{vars.slot1}}
_params = {"url": _url, "method": "delete"}
if _headers:
    _params["headers"] = _headers
result = host.http(**_params)
"#;

const PC_HTTP_STATUS_CHECK_CONTENT: &str = r#"# No I/O, no imports. IBS bakes in status_code as {{vars.slot0}} before execution.
status_code = {{vars.slot0}}
is_success = 200 <= status_code < 300
result = {"is_success": is_success, "status_code": status_code}
"#;

const PC_JSON_EXTRACT_FIELD_CONTENT: &str = r#"# No I/O, no imports. IBS bakes in 'data' and 'path' before execution.
data = {{vars.slot0}}
path = "{{vars.slot1}}"
parts = path.split(".")
current = data
for part in parts:
    if isinstance(current, dict) and part in current:
        current = current[part]
    elif isinstance(current, list):
        try:
            current = current[int(part)]
        except (ValueError, IndexError):
            current = None
            break
    else:
        current = None
        break
result = {"value": current, "path": path, "found": current is not None}
"#;

const PC_WEB_SEARCH_EXTRACT_CONTENT: &str = r#"# Pure orchestrator body - no imports. slot0 = pre-parsed response dict
# (the http tool's JSON response is already a dict in the execution context).
_data = "{{vars.slot0}}"
if isinstance(_data, dict):
    _results = (
        _data.get("results") or
        _data.get("organic_results") or
        _data.get("items") or
        _data.get("RelatedTopics") or
        []
    )
    result = [
        {
            "title":   r.get("title") or r.get("Text", ""),
            "url":     r.get("url") or r.get("link") or r.get("FirstURL", ""),
            "snippet": r.get("snippet") or r.get("description") or ""
        }
        for r in _results if isinstance(r, dict)
    ]
else:
    result = {"error": "expected parsed dict from http response", "raw": str(_data)[:500]}
"#;

const PC_WEB_SEARCH_QUERY_BUILD_CONTENT: &str = r#"# No imports - pure built-in percent-encoding (mirrors pc-url-encode logic).
_raw = "{{vars.slot0}}".strip()
_safe = set("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~")
_encoded = "".join(c if c in _safe else "%" + format(ord(c), "02X") for c in _raw)
result = {"encoded": _encoded, "raw": _raw}
"#;

const PC_URL_ENCODE_CONTENT: &str = r#"# No imports - pure built-in percent-encoding. Covers all non-unreserved chars.
_raw = "{{vars.slot0}}".strip()
_safe = set("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~")
_encoded = "".join(c if c in _safe else "%" + format(ord(c), "02X") for c in _raw)
result = {"encoded": _encoded, "raw": _raw}
"#;

// ---------------------------------------------------------------------------
// Network group skill bodies (class 1 leaf + class 2 domain) — transcribed
// verbatim from the doc.
// ---------------------------------------------------------------------------

const SKILL_HTTP_GET_BODY: &str = r#"Use `ts-http-fetch` with method='get' (via pc-exec-http-get) to fetch a URL and receive
the response body inline. The body is capped at 256 KiB. If a larger response is needed,
use skill-http-save instead. Non-2xx status codes appear in the result's status field -
they are not tool errors. Always inspect the status code after the call.
"#;

const SKILL_HTTP_POST_BODY: &str = r#"Use `ts-http-fetch` with method='post' and a `body` (string or JSON) to submit data
to an API or webhook. Add an `Authorization` or `Content-Type` header when required.
For JSON bodies the server typically expects `Content-Type: application/json`. Non-2xx
responses are not tool errors - check the status field and handle error responses.
"#;

const SKILL_HTTP_AUTHENTICATED_BODY: &str = r#"Use `ts-http-fetch` with a `headers` parameter to attach authentication. Common patterns:
- Bearer token: headers={'Authorization': 'Bearer <token>'}
- API key: headers={'X-Api-Key': '<key>'}
- Basic auth: headers={'Authorization': 'Basic <base64(user:pass)>'}
Never hardcode credentials in the skill body - always receive them from the session
context or memory. Use skill-http-get or skill-http-post as the base dispatch pattern.
"#;

const SKILL_HTTP_SAVE_DOWNLOAD_BODY: &str = r#"Use `ts-http-save` (via pc-exec-http-save) when the expected response exceeds 256 KiB
or when the content must be persisted to disk. Provide the url and a scoped save_to
path. After the call, use skill-read-file to inspect the saved content or report the
file path to the user. The response is saved without decoding binary content.
"#;

const SKILL_HTTP_SAVE_API_BODY: &str = r#"When an API returns more data than can be processed inline (>256 KiB), use
`ts-http-save` to write the full response to a temp file, then use skill-read-file
or pc-json-extract-field to extract the needed fields from the saved file. This is
the recommended pattern for paginated or bulk API responses.
"#;

const SKILL_HTTP_PATCH_BODY: &str = r#"Use `ts-http-fetch` with method='patch' and a `body` containing only the fields to
update (via a custom pc-exec-http-patch-like call). PATCH is idempotent partial update
- unlike PUT which replaces the full resource. Include Content-Type: application/json
and Authorization headers when required. Non-2xx responses are not tool errors.
"#;

const SKILL_HTTP_HEAD_BODY: &str = r#"Use `ts-http-fetch` with method='head' (via pc-exec-http-head) when you only need
the response headers and status code - not the response body. HEAD is cheaper than
GET for large resources and is the correct method for existence checks, content-type
inspection, and reachability tests. The response body will be empty; inspect the
status code (200 = exists, 404 = not found, etc.) and headers.
"#;

const SKILL_HTTP_PUT_BODY: &str = r#"Use `ts-http-fetch` with method='put' and a `body` (via pc-exec-http-put) when the
target API uses PUT semantics (idempotent full replacement of a resource). Include a
Content-Type header (typically 'application/json') and an Authorization header when
required. Non-2xx responses are not tool errors - always check the status field.
"#;

const SKILL_HTTP_DELETE_BODY: &str = r#"Use `ts-http-fetch` with method='delete' (via pc-exec-http-delete) to remove a
resource via REST API. DELETE has ExternalWrite semantics - always confirm with the
user before dispatching. Include Authorization headers when required. Check the
response status (204 No Content = success for many REST APIs; 404 = already gone).
"#;

const SKILL_WEB_SEARCH_BODY: &str = r#"Web search is a composition, not a single tool. The pattern:
1. Build the search URL: encode the user's query via pc-web-search-query-build and
   append it to the configured search API base URL.
2. Issue an HTTP GET via ts-http-get (or directly via builtin.http) with
   Accept: application/json header and any required API key header.
3. Parse and extract results from the response JSON using pc-web-search-extract.
4. Present the top N results (title, URL, snippet) to the user. Ask if they want
   to fetch any result's full page via builtin.http for deeper reading.

If no search API is configured, inform the user and ask them to configure one
(endpoint URL + API key) before proceeding.
"#;

const SKILL_HTTP_BODY: &str = r#"The HTTP domain provides two tools for outbound HTTP requests:

INLINE RESPONSE (<=256 KiB):
- skill-http-get: GET request, response inline.
- skill-http-post: POST request with body, response inline.
- skill-http-authenticated: Any method with auth headers.
- skill-http-head: HEAD request - metadata only, no body.
- skill-http-put: PUT request - full resource replacement.
- skill-http-patch: PATCH request - partial resource update.
- skill-http-delete: DELETE request - remove a resource.

SAVED RESPONSE (>256 KiB or must persist):
- skill-http-save-download: Download and save to a workspace file.
- skill-http-save-api: Save a large API response for later parsing.

Decision guide:
- Small response needed immediately -> skill-http-get
- POST with body -> skill-http-post
- Authenticated request -> skill-http-authenticated (combine with above)
- Existence/metadata check only -> skill-http-head (no body returned)
- Full resource replace -> skill-http-put
- Partial update -> skill-http-patch
- Delete a resource -> skill-http-delete
- Response >256 KiB or must be saved -> skill-http-save-download
- Large API response for later parsing -> skill-http-save-api

Non-2xx HTTP responses are NOT tool errors. Always inspect the status field.
Use pc-http-status-check to test success programmatically.
"#;

// ---------------------------------------------------------------------------
// Network group recipe step_descriptions (class 21) — verbatim doc
// step_descriptions blocks (Q1=A). `<uuid:...>` placeholders are resolved to
// live seeded UUIDs at seed_recipe time via step_entry's include lists.
// ---------------------------------------------------------------------------

const RECIPE_HTTP_GET_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-http-fetch>"],
    "label":   "Pre-load ts-http-fetch ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-http-get>"],
    "label":   "PythonCode calls host.http(url, method=get)"
  }
]
"#;

const RECIPE_HTTP_GET_JSON_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-http-fetch>"],
    "label":   "Pre-load ts-http-fetch ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-http-get>"],
    "label":   "PythonCode calls host.http(url, method=get, headers={Accept:application/json})"
  }
]
"#;

const RECIPE_HTTP_POST_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-http-post>", "<uuid:skill-http-authenticated>"],
    "label":   "Load http-post + auth leaf skill context"
  },
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM constructs the POST URL, headers, and body from user instructions"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-http-fetch>"],
    "label":   "Pre-load ts-http-fetch binding"
  }
]
"#;

const RECIPE_HTTP_SAVE_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-http-save>"],
    "label":   "Pre-load ts-http-save ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-http-save>"],
    "label":   "PythonCode calls host.http.save(url, save_to)"
  }
]
"#;

const RECIPE_HTTP_PATCH_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-http-patch>", "<uuid:skill-http-authenticated>"],
    "label":   "Load http-patch + auth leaf skill context"
  },
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM constructs the PATCH URL, headers, and partial update body from user instructions"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-http-fetch>"],
    "label":   "Pre-load ts-http-fetch binding"
  }
]
"#;

const RECIPE_HTTP_SAVE_LARGE_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-http-save>"],
    "label":   "Pre-load ts-http-save ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-http-save>"],
    "label":   "PythonCode calls host.http.save(url, save_to, response_body_limit=5242880)"
  }
]
"#;

const RECIPE_HTTP_HEAD_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-http-fetch>"],
    "label":   "Pre-load ts-http-fetch ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-http-head>"],
    "label":   "PythonCode calls host.http(url, method='head')"
  }
]
"#;

const RECIPE_HTTP_AUTHENTICATED_GET_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-http-fetch>"],
    "label":   "Pre-load ts-http-fetch ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-http-get-authenticated>"],
    "label":   "PythonCode calls host.http(url, method=get, headers={Authorization:...})"
  }
]
"#;

const RECIPE_HTTP_PUT_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-http-put>", "<uuid:skill-http-authenticated>"],
    "label":   "Load http-put + auth leaf skill context"
  },
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM constructs the PUT URL, headers, and replacement body from user instructions"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-http-fetch>"],
    "label":   "Pre-load ts-http-fetch binding"
  }
]
"#;

const RECIPE_HTTP_DELETE_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-http-delete>", "<uuid:skill-http-authenticated>"],
    "label":   "Load http-delete + auth leaf skill context"
  },
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM confirms target URL with user (ExternalWrite - irreversible), then calls ts-http-fetch"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-http-fetch>"],
    "label":   "Pre-load ts-http-fetch binding"
  }
]
"#;

const RECIPE_HTTP_POST_JSON_WEBHOOK_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-http-fetch>"],
    "label":   "Pre-load ts-http-fetch ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-http-post>"],
    "label":   "PythonCode calls host.http(url, method=post, body={event, payload}, headers={Content-Type:application/json})"
  }
]
"#;

const RECIPE_WEB_SEARCH_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-web-search>"],
    "label":   "Load skill-web-search leaf skill body (composition pattern)"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-web-search-query-build>", "<uuid:pc-web-search-extract>"],
    "label":   "Load PythonCode helpers for query encoding and result extraction"
  },
  {
    "step_id": "step-3",
    "type":    "llm",
    "label":   "LLM formulates query, calls ts-http-get, extracts results, presents to user"
  },
  {
    "step_id": "step-4",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-http-fetch>"],
    "label":   "Pre-load ts-http-fetch ToolSkill binding (used for both search and follow-up fetches)"
  }
]
"#;

// ---------------------------------------------------------------------------
// Memory group (Pass 3) — builtin.memory_search / memory_write / memory_read
// / memory_tree + the search-and-read combined Tier-1 recipe.
// ---------------------------------------------------------------------------

/// Memory domain ExtensionCatalogue name (Step 23).
const CAT_MEMORY: &str = "builtin-memory";

const CAT_EXT_MEMORY_SEARCH_OVERVIEW: &str = r#"# Memory Search Capability
Tool: builtin.memory_search
Effect: read_memory
Permission: Allow

Searches the agent's persistent memory store with a natural language query and
returns the most relevant documents ranked by semantic similarity. Limit defaults
to 5; maximum 20. Empty results are not an error.

Approaches:
- Focused recall: query -> memory-search recipe (Tier 0)
- Wide recall at session start: query + limit=20 -> memory-search-broad recipe (Tier 0)
- Search + read top result: -> memory-search-and-read recipe (Tier 1, LLM picks result)
"#;

const CAT_EXT_MEMORY_WRITE_OVERVIEW: &str = r#"# Memory Write Capability
Tool: builtin.memory_write
Effect: write_memory
Permission: Allow

Writes or appends content to the agent's persistent memory. Default target is
'daily_log' (today's dated log). Other targets: 'memory' (MEMORY.md),
'heartbeat' (HEARTBEAT.md), 'bootstrap' (clears BOOTSTRAP.md), or any relative
memory document path. Supports patch mode (old_string/new_string).

Approaches:
- Log a progress note (default daily_log): -> memory-write-log recipe (Tier 0)
- Update MEMORY.md: -> memory-write-main recipe (Tier 0, target='memory')
- Generic write: -> memory-write recipe (Tier 0)
- Targeted patch of a section: -> memory-write-patch recipe (Tier 0, patch mode)
"#;

const CAT_EXT_MEMORY_READ_OVERVIEW: &str = r#"# Memory Read Capability
Tool: builtin.memory_read
Effect: read_memory
Permission: Allow

Reads a specific memory document by its relative path and returns the full
content. Use memory_search for semantic discovery; use memory_read when the
exact path is known.

Approaches:
- Read a known path: -> memory-read recipe (Tier 0)
- Read MEMORY.md: -> memory-read-main recipe (Tier 0, path='MEMORY.md')
- Read HEARTBEAT.md: -> memory-read-heartbeat recipe (Tier 0, path='HEARTBEAT.md')
"#;

const CAT_EXT_MEMORY_TREE_OVERVIEW: &str = r#"# Memory Tree Capability
Tool: builtin.memory_tree
Effect: read_memory
Permission: Allow

Lists the directory tree of the agent's persistent memory up to a given depth.
Used to discover memory structure before targeted reads.

Approaches:
- Browse memory structure: -> memory-tree recipe (Tier 0)
"#;

const CAT_MEMORY_OVERVIEW: &str = r#"# Memory Capabilities

The memory domain gives the agent durable persistent storage for notes, decisions,
session context, and long-term project state. All memory operations go through
four tools; the orchestrator never touches the memory filesystem directly.

## Tools in this domain
- builtin.memory_search - semantic search across memory documents
- builtin.memory_write - write/append/patch a memory document
- builtin.memory_read  - read a memory document by exact path
- builtin.memory_tree  - browse the memory directory structure

## Common targets
- 'daily_log' (default) - today's dated log; lightest-weight, append-only
- 'memory'              - MEMORY.md, the primary durable context document
- 'heartbeat'           - HEARTBEAT.md, the rolling status/checklist file
- relative path         - any other memory document

## Constraints
- The orchestrator must NEVER use datetime.now() in PythonCode. Always call
  skill-time-now first to get a timestamp, then pass it to pc-memory-format-entry.
- Patch mode requires both old_string and new_string; old_string not found is a
  tool error.
- Empty search results are not an error.
"#;

/// Seed the memory domain group: the primary `builtin-memory` catalogue + 4
/// per-tool catalogues + 4 Tool rows + 4 ToolSkill rows.
///
/// PythonCode, leaf/domain Skills, and Recipes are added in chunks 5b-5d;
/// their ids are appended to the catalogues' `child_component_ids` as they
/// are minted (dedup makes this idempotent).
async fn seed_memory_group(
    stores: &BootstrapStores,
) -> Result<(), SeedBuiltinBootstrapError> {
    let tenant = stores.tenant.clone();

    // 1. Primary domain catalogue + per-tool catalogues (empty child_ids;
    //    appended to as children are minted below and in later chunks).
    let cat_memory = stores
        .upsert_catalogue(memory_primary_catalogue_row(&tenant), CAT_MEMORY)
        .await?;
    let cat_memory_search = stores
        .upsert_catalogue(
            ext_catalogue_row(
                &tenant,
                "ext-memory-search",
                "Memory search capability (builtin.memory_search).",
                CAT_EXT_MEMORY_SEARCH_OVERVIEW,
                json!([
                    {"group_name": "memory-search-focused", "description": "Focused semantic recall (default limit)"},
                    {"group_name": "memory-search-broad", "description": "Wide recall with limit=20"}
                ]),
            ),
            "ext-memory-search",
        )
        .await?;
    let cat_memory_write = stores
        .upsert_catalogue(
            ext_catalogue_row(
                &tenant,
                "ext-memory-write",
                "Memory write capability (builtin.memory_write).",
                CAT_EXT_MEMORY_WRITE_OVERVIEW,
                json!([
                    {"group_name": "memory-write-log", "description": "Append to today's daily_log"},
                    {"group_name": "memory-write-main", "description": "Update the main MEMORY.md document"},
                    {"group_name": "memory-write-patch", "description": "Targeted patch of an existing memory document"}
                ]),
            ),
            "ext-memory-write",
        )
        .await?;
    let cat_memory_read = stores
        .upsert_catalogue(
            ext_catalogue_row(
                &tenant,
                "ext-memory-read",
                "Memory read capability (builtin.memory_read).",
                CAT_EXT_MEMORY_READ_OVERVIEW,
                json!([
                    {"group_name": "memory-read-path", "description": "Read a memory document by exact path"},
                    {"group_name": "memory-read-main", "description": "Read MEMORY.md"},
                    {"group_name": "memory-read-heartbeat", "description": "Read HEARTBEAT.md"}
                ]),
            ),
            "ext-memory-read",
        )
        .await?;
    let cat_memory_tree = stores
        .upsert_catalogue(
            ext_catalogue_row(
                &tenant,
                "ext-memory-tree",
                "Memory directory tree capability (builtin.memory_tree).",
                CAT_EXT_MEMORY_TREE_OVERVIEW,
                json!([
                    {"group_name": "memory-tree", "description": "Browse the memory directory structure"}
                ]),
            ),
            "ext-memory-tree",
        )
        .await?;

    // 2. Tool rows (class 0) — capability_id taken from the live
    //    `*_CAPABILITY_ID` constant in `first_party_tools/memory.rs`.
    let tool_memory_search = stores
        .upsert_tool(tool_memory_search_row(&tenant), "memory_search")
        .await?;
    let tool_memory_write = stores
        .upsert_tool(tool_memory_write_row(&tenant), "memory_write")
        .await?;
    let tool_memory_read = stores
        .upsert_tool(tool_memory_read_row(&tenant), "memory_read")
        .await?;
    let tool_memory_tree = stores
        .upsert_tool(tool_memory_tree_row(&tenant), "memory_tree")
        .await?;

    // 3. ToolSkill rows (class 13).
    let ts_memory_search = stores
        .upsert_tool_skill(ts_memory_search_row(&tenant), "ts-memory-search")
        .await?;
    let ts_memory_write = stores
        .upsert_tool_skill(ts_memory_write_row(&tenant), "ts-memory-write")
        .await?;
    let ts_memory_read = stores
        .upsert_tool_skill(ts_memory_read_row(&tenant), "ts-memory-read")
        .await?;
    let ts_memory_tree = stores
        .upsert_tool_skill(ts_memory_tree_row(&tenant), "ts-memory-tree")
        .await?;

    // 4. PythonCode rows (class 22) — orchestrator executors + pure-logic
    //    helpers. Transcribed verbatim from the doc (Q1 decision A); bodies
    //    use `host.<tool>(kwarg=value)` dispatch with `{{vars.slotN}}` vars.
    let pc_exec_memory_search = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-memory-search",
                "Orchestrator executor: calls host.<tool> to search persistent memory \
                 via builtin.memory_search. Input: query (string), limit (optional int \
                 1-20). Output: ranked memory documents.",
                PC_EXEC_MEMORY_SEARCH_CONTENT,
            ),
            "pc-exec-memory-search",
        )
        .await?;
    let pc_exec_memory_write = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-memory-write",
                "Orchestrator executor: calls host.<tool> to write to persistent memory \
                 via builtin.memory_write. Input: content (string), target (optional \
                 string, default 'daily_log'), append (optional bool, default true).",
                PC_EXEC_MEMORY_WRITE_CONTENT,
            ),
            "pc-exec-memory-write",
        )
        .await?;
    let pc_exec_memory_patch = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-memory-patch",
                "Orchestrator executor: calls host.<tool> for a targeted patch to a \
                 memory document via builtin.memory_write patch mode. Input: target \
                 (string), old_string (string), new_string (string), replace_all \
                 (optional bool).",
                PC_EXEC_MEMORY_PATCH_CONTENT,
            ),
            "pc-exec-memory-patch",
        )
        .await?;
    let pc_exec_memory_read = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-memory-read",
                "Orchestrator executor: calls host.<tool> to read a memory document by \
                 path via builtin.memory_read. Input: path (string). Output: full \
                 document content.",
                PC_EXEC_MEMORY_READ_CONTENT,
            ),
            "pc-exec-memory-read",
        )
        .await?;
    let pc_exec_memory_tree = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-memory-tree",
                "Orchestrator executor: calls host.<tool> to list the memory directory \
                 tree via builtin.memory_tree. Input: path (optional string), depth \
                 (optional int).",
                PC_EXEC_MEMORY_TREE_CONTENT,
            ),
            "pc-exec-memory-tree",
        )
        .await?;
    let pc_memory_extract_section = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-memory-extract-section",
                "Pure-logic helper: extracts a named section from a Markdown document \
                 using heading matching. Input: content (string), heading (string - \
                 heading text without # prefix). Output: {section_content, heading, \
                 found}.",
                PC_MEMORY_EXTRACT_SECTION_CONTENT,
            ),
            "pc-memory-extract-section",
        )
        .await?;
    let pc_memory_format_entry = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-memory-format-entry",
                "Pure-logic helper: formats a memory entry string ready for appending \
                 to a memory document. Input: text (string), timestamp_str (string - \
                 caller supplies pre-fetched timestamp). Output: {formatted_entry}.",
                PC_MEMORY_FORMAT_ENTRY_CONTENT,
            ),
            "pc-memory-format-entry",
        )
        .await?;

    // 5. Leaf Skills (class 1) + Domain Skill (class 2). Transcribed verbatim
    //    from the doc (Q1 decision A). All carry LEAF_SKILL_TAGS
    //    [02:orchestrator, 05:validator] — the skill store has no SEC-01
    //    05:validator-hiding filter, so this is safe.
    let skill_memory_search = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-memory-search",
                "Leaf skill: how to retrieve relevant information from the agent's persistent \
                 memory.",
                SKILL_MEMORY_SEARCH_BODY,
                1,
                LEAF_SKILL_TAGS,
            ),
            "skill-memory-search",
        )
        .await?;
    let skill_memory_search_broad = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-memory-search-broad",
                "Leaf skill: how to perform a broad memory recall across many documents.",
                SKILL_MEMORY_SEARCH_BROAD_BODY,
                1,
                LEAF_SKILL_TAGS,
            ),
            "skill-memory-search-broad",
        )
        .await?;
    let skill_memory_write_log = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-memory-write-log",
                "Leaf skill: how to log a note or progress update to today's daily log.",
                SKILL_MEMORY_WRITE_LOG_BODY,
                1,
                LEAF_SKILL_TAGS,
            ),
            "skill-memory-write-log",
        )
        .await?;
    let skill_memory_write_main = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-memory-write-main",
                "Leaf skill: how to update the main MEMORY.md document.",
                SKILL_MEMORY_WRITE_MAIN_BODY,
                1,
                LEAF_SKILL_TAGS,
            ),
            "skill-memory-write-main",
        )
        .await?;
    let skill_memory_write_patch = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-memory-write-patch",
                "Leaf skill: how to make a targeted edit to an existing memory document.",
                SKILL_MEMORY_WRITE_PATCH_BODY,
                1,
                LEAF_SKILL_TAGS,
            ),
            "skill-memory-write-patch",
        )
        .await?;
    let skill_memory_read = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-memory-read",
                "Leaf skill: how to read a specific memory document by its exact path.",
                SKILL_MEMORY_READ_BODY,
                1,
                LEAF_SKILL_TAGS,
            ),
            "skill-memory-read",
        )
        .await?;
    let skill_memory_tree = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-memory-tree",
                "Leaf skill: how to browse the structure of the agent's persistent memory.",
                SKILL_MEMORY_TREE_BODY,
                1,
                LEAF_SKILL_TAGS,
            ),
            "skill-memory-tree",
        )
        .await?;
    let skill_memory_search_and_read = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-memory-search-and-read",
                "Leaf skill: how to search memory and immediately read the top result.",
                SKILL_MEMORY_SEARCH_AND_READ_BODY,
                1,
                LEAF_SKILL_TAGS,
            ),
            "skill-memory-search-and-read",
        )
        .await?;
    let skill_memory = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-memory",
                "The memory domain provides four tools for the agent's persistent memory \
                 store (search, write, read, tree) plus a search-and-read combined recipe.",
                SKILL_MEMORY_BODY,
                2,
                LEAF_SKILL_TAGS,
            ),
            "skill-memory",
        )
        .await?;

    // 5b. Step 11.x variant components: pc-exec-memory-append (class 22) +
    //      skill-memory-write-append (class 1). Minted here (chunk 5d) rather
    //      than in 5b/5c because the memory-write-append Tier-1 recipe is the
    //      only consumer and chunk 5d consumes every id via append_children.
    let pc_exec_memory_append = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-memory-append",
                "Orchestrator executor: appends text to an existing memory document. Reads the \
                 current content via memory_read, then writes combined content via memory_write. \
                 Input: vars.slot0 = path, vars.slot1 = text to append.",
                PC_EXEC_MEMORY_APPEND_CONTENT,
            ),
            "pc-exec-memory-append",
        )
        .await?;
    let skill_memory_write_append = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-memory-write-append",
                "Leaf skill: how to append new text to an existing memory document.",
                SKILL_MEMORY_WRITE_APPEND_BODY,
                1,
                LEAF_SKILL_TAGS,
            ),
            "skill-memory-write-append",
        )
        .await?;

    // 6. Memory Recipes (class 21) — transcribed from the doc's flat format
    //    into the IBS authoring model (Q1 decision A). Tier-0 recipes are
    //    deterministic 2-step dispatches (rust toolskill + orchestrator
    //    PythonCode); the two Tier-1 recipes (memory-write-append,
    //    memory-search-and-read) add an LLM-annotation `text` step and load
    //    leaf-skill context first. step_link is synthesized as "0:1-0:E".
    let recipe_memory_search = stores
        .seed_recipe(
            &tenant,
            "memory-search",
            "Search the agent's persistent memory.",
            true,
            RECIPE_MEMORY_SEARCH_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-memory-search ToolSkill binding", "component", &[ts_memory_search]),
                step_entry(2, "orchestrator", "PythonCode calls host.memory_search(query, limit)", "component", &[pc_exec_memory_search]),
            ],
            &[
                json!({"input": "what do you remember about this project", "class": 2}),
                json!({"input": "search memory for authentication notes", "class": 2}),
                json!({"input": "find any saved notes about this topic", "class": 2}),
                json!({"input": "recall what we discussed last time", "class": 2}),
                json!({"input": "memory search", "class": 1}),
                json!({"input": "do you have notes on this", "class": 2}),
                json!({"input": "search my memory for database setup", "class": 2}),
                json!({"input": "recall my earlier decisions about this module", "class": 2}),
                json!({"input": "find memory entries about this feature", "class": 2}),
                json!({"input": "memory recall", "class": 1}),
            ],
        )
        .await?;
    let recipe_memory_search_broad = stores
        .seed_recipe(
            &tenant,
            "memory-search-broad",
            "Search the agent's persistent memory with a wide recall (limit=20).",
            true,
            RECIPE_MEMORY_SEARCH_BROAD_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-memory-search ToolSkill binding", "component", &[ts_memory_search]),
                step_entry(2, "orchestrator", "PythonCode calls host.memory_search(query, limit=20)", "component", &[pc_exec_memory_search]),
            ],
            &[
                json!({"input": "recall everything you know about this project", "class": 2}),
                json!({"input": "broad memory recall for this topic", "class": 2}),
                json!({"input": "search all my memory about this feature", "class": 2}),
                json!({"input": "full memory recall at session start", "class": 2}),
                json!({"input": "memory broad search", "class": 1}),
                json!({"input": "find all notes I have on this", "class": 2}),
                json!({"input": "recall all prior decisions about this system", "class": 2}),
                json!({"input": "wide memory search for onboarding context", "class": 2}),
                json!({"input": "deep recall across all memory docs", "class": 2}),
                json!({"input": "start-of-session full memory restore", "class": 2}),
            ],
        )
        .await?;
    let recipe_memory_write = stores
        .seed_recipe(
            &tenant,
            "memory-write",
            "Write or append content to the agent's persistent memory.",
            true,
            RECIPE_MEMORY_WRITE_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-memory-write ToolSkill binding", "component", &[ts_memory_write]),
                step_entry(2, "orchestrator", "PythonCode calls host.memory_write(content, target, append)", "component", &[pc_exec_memory_write]),
            ],
            &[
                json!({"input": "save this to memory", "class": 2}),
                json!({"input": "remember this for later", "class": 2}),
                json!({"input": "log this progress note", "class": 2}),
                json!({"input": "update MEMORY.md with this decision", "class": 2}),
                json!({"input": "add this to my daily log", "class": 1}),
                json!({"input": "write a note to memory", "class": 2}),
                json!({"input": "store this for later", "class": 2}),
                json!({"input": "persist this outcome to memory", "class": 2}),
                json!({"input": "memory write", "class": 1}),
                json!({"input": "append this to the daily log", "class": 1}),
            ],
        )
        .await?;
    let recipe_memory_write_log = stores
        .seed_recipe(
            &tenant,
            "memory-write-log",
            "Append a note or progress entry to today's daily log in persistent memory.",
            true,
            RECIPE_MEMORY_WRITE_LOG_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-memory-write ToolSkill binding", "component", &[ts_memory_write]),
                step_entry(2, "orchestrator", "PythonCode calls host.memory_write(content, target='daily_log', append=true)", "component", &[pc_exec_memory_write]),
            ],
            &[
                json!({"input": "log this progress note", "class": 1}),
                json!({"input": "add to my daily log", "class": 1}),
                json!({"input": "append a note to today's log", "class": 1}),
                json!({"input": "write a progress update to the daily log", "class": 1}),
                json!({"input": "daily log entry", "class": 1}),
                json!({"input": "record this in the daily log", "class": 1}),
                json!({"input": "log what I did today", "class": 2}),
                json!({"input": "log session progress", "class": 1}),
                json!({"input": "note this down in my activity log", "class": 2}),
                json!({"input": "add this to today's memory log", "class": 2}),
            ],
        )
        .await?;
    let recipe_memory_write_main = stores
        .seed_recipe(
            &tenant,
            "memory-write-main",
            "Append content to the main MEMORY.md document in persistent memory.",
            true,
            RECIPE_MEMORY_WRITE_MAIN_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-memory-write ToolSkill binding", "component", &[ts_memory_write]),
                step_entry(2, "orchestrator", "PythonCode calls host.memory_write(content, target='memory', append=true)", "component", &[pc_exec_memory_write]),
            ],
            &[
                json!({"input": "update MEMORY.md with this", "class": 1}),
                json!({"input": "add this decision to MEMORY.md", "class": 1}),
                json!({"input": "write this to the main memory document", "class": 1}),
                json!({"input": "append to MEMORY.md", "class": 1}),
                json!({"input": "update my main memory", "class": 1}),
                json!({"input": "save this finding to MEMORY.md", "class": 1}),
                json!({"input": "add a permanent note to memory", "class": 2}),
                json!({"input": "write to the memory document", "class": 1}),
            ],
        )
        .await?;
    let recipe_memory_write_patch = stores
        .seed_recipe(
            &tenant,
            "memory-write-patch",
            "Patch a specific section of an existing memory document using search-replace.",
            true,
            RECIPE_MEMORY_WRITE_PATCH_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-memory-write ToolSkill binding", "component", &[ts_memory_write]),
                step_entry(2, "orchestrator", "PythonCode calls host.memory_write(target, old_string, new_string)", "component", &[pc_exec_memory_patch]),
            ],
            &[
                json!({"input": "patch a section in MEMORY.md", "class": 1}),
                json!({"input": "replace this text in my memory document", "class": 1}),
                json!({"input": "update a specific section of a memory file", "class": 1}),
                json!({"input": "memory write patch mode", "class": 1}),
                json!({"input": "fix a section in HEARTBEAT.md", "class": 2}),
                json!({"input": "search and replace in a memory document", "class": 2}),
                json!({"input": "targeted edit to a memory file", "class": 1}),
                json!({"input": "update one section without replacing the file", "class": 2}),
            ],
        )
        .await?;
    let recipe_memory_write_append = stores
        .seed_recipe(
            &tenant,
            "memory-write-append",
            "Append new content to an existing memory document (read-concat-write).",
            false,
            RECIPE_MEMORY_WRITE_APPEND_YAML,
            &[
                step_entry(1, "orchestrator", "Load append + read leaf skills", "component", &[skill_memory_write_append, skill_memory_read]),
                step_entry(2, "orchestrator", "LLM composes the new text to append based on current context", "text", &[]),
                step_entry(3, "rust", "Pre-load ts-memory-read and ts-memory-write ToolSkill bindings", "component", &[ts_memory_read, ts_memory_write]),
            ],
            &[
                json!({"input": "append to my memory document", "class": 2}),
                json!({"input": "add a note to my memory file", "class": 2}),
                json!({"input": "log this to my memory", "class": 2}),
                json!({"input": "add an entry to the log", "class": 2}),
                json!({"input": "append to CHANGELOG.md", "class": 1}),
                json!({"input": "add this to my running notes", "class": 2}),
                json!({"input": "update memory log with this entry", "class": 2}),
                json!({"input": "memory append", "class": 1}),
                json!({"input": "add a new session entry to memory", "class": 2}),
                json!({"input": "log this decision to memory", "class": 2}),
            ],
        )
        .await?;
    let recipe_memory_read = stores
        .seed_recipe(
            &tenant,
            "memory-read",
            "Read a specific memory document by path.",
            true,
            RECIPE_MEMORY_READ_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-memory-read ToolSkill binding", "component", &[ts_memory_read]),
                step_entry(2, "orchestrator", "PythonCode calls host.memory_read(path)", "component", &[pc_exec_memory_read]),
            ],
            &[
                json!({"input": "read MEMORY.md", "class": 1}),
                json!({"input": "show me the contents of HEARTBEAT.md", "class": 1}),
                json!({"input": "read my memory document", "class": 2}),
                json!({"input": "open this memory file", "class": 2}),
                json!({"input": "show memory at this path", "class": 1}),
                json!({"input": "read the file at this memory path", "class": 1}),
                json!({"input": "memory read", "class": 1}),
                json!({"input": "open the notes at this memory location", "class": 2}),
            ],
        )
        .await?;
    let recipe_memory_read_main = stores
        .seed_recipe(
            &tenant,
            "memory-read-main",
            "Read the main MEMORY.md document from persistent memory.",
            true,
            RECIPE_MEMORY_READ_MAIN_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-memory-read ToolSkill binding", "component", &[ts_memory_read]),
                step_entry(2, "orchestrator", "PythonCode calls host.memory_read(path='MEMORY.md')", "component", &[pc_exec_memory_read]),
            ],
            &[
                json!({"input": "read MEMORY.md", "class": 1}),
                json!({"input": "show me MEMORY.md", "class": 1}),
                json!({"input": "open the main memory document", "class": 1}),
                json!({"input": "read my persistent memory", "class": 2}),
                json!({"input": "what is in MEMORY.md", "class": 1}),
                json!({"input": "show me the contents of memory", "class": 2}),
                json!({"input": "display MEMORY.md", "class": 1}),
                json!({"input": "read main memory file", "class": 1}),
                json!({"input": "show me my durable context document", "class": 2}),
                json!({"input": "read the primary memory doc at session start", "class": 2}),
            ],
        )
        .await?;
    let recipe_memory_read_heartbeat = stores
        .seed_recipe(
            &tenant,
            "memory-read-heartbeat",
            "Read the HEARTBEAT.md status document from persistent memory.",
            true,
            RECIPE_MEMORY_READ_HEARTBEAT_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-memory-read ToolSkill binding", "component", &[ts_memory_read]),
                step_entry(2, "orchestrator", "PythonCode calls host.memory_read(path='HEARTBEAT.md')", "component", &[pc_exec_memory_read]),
            ],
            &[
                json!({"input": "read HEARTBEAT.md", "class": 1}),
                json!({"input": "show me the heartbeat document", "class": 1}),
                json!({"input": "what is in HEARTBEAT.md", "class": 1}),
                json!({"input": "read the agent heartbeat status", "class": 2}),
                json!({"input": "show me the current heartbeat", "class": 2}),
                json!({"input": "display HEARTBEAT.md", "class": 1}),
                json!({"input": "read heartbeat", "class": 1}),
                json!({"input": "open the heartbeat memory file", "class": 1}),
                json!({"input": "show the latest heartbeat checkpoint", "class": 2}),
                json!({"input": "what does my heartbeat status say", "class": 2}),
            ],
        )
        .await?;
    let recipe_memory_tree = stores
        .seed_recipe(
            &tenant,
            "memory-tree",
            "List the directory structure of the agent's persistent memory.",
            true,
            RECIPE_MEMORY_TREE_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-memory-tree ToolSkill binding", "component", &[ts_memory_tree]),
                step_entry(2, "orchestrator", "PythonCode calls host.memory_tree(path, depth)", "component", &[pc_exec_memory_tree]),
            ],
            &[
                json!({"input": "what files are in my memory", "class": 2}),
                json!({"input": "show me the memory directory structure", "class": 2}),
                json!({"input": "list all memory documents", "class": 1}),
                json!({"input": "browse my memory files", "class": 2}),
                json!({"input": "memory tree", "class": 1}),
                json!({"input": "what memory documents exist", "class": 2}),
                json!({"input": "show me the memory hierarchy", "class": 2}),
                json!({"input": "memory directory listing", "class": 1}),
                json!({"input": "what notes do I have stored", "class": 2}),
                json!({"input": "explore my memory structure", "class": 2}),
            ],
        )
        .await?;
    let recipe_memory_search_and_read = stores
        .seed_recipe(
            &tenant,
            "memory-search-and-read",
            "Search persistent memory by topic and read the top matching document in one flow.",
            false,
            RECIPE_MEMORY_SEARCH_AND_READ_YAML,
            &[
                step_entry(1, "orchestrator", "Load search-and-read combined leaf skill body", "component", &[skill_memory_search_and_read]),
                step_entry(2, "rust", "Pre-load both ToolSkill bindings", "component", &[ts_memory_search, ts_memory_read]),
                step_entry(3, "orchestrator", "PythonCode: search memory, take top result path, read document", "component", &[pc_exec_memory_search, pc_exec_memory_read]),
                step_entry(4, "orchestrator", "LLM interprets query intent, selects best result path, presents content", "text", &[]),
            ],
            &[
                json!({"input": "recall what I know about this topic", "class": 2}),
                json!({"input": "search memory and show me the full document", "class": 1}),
                json!({"input": "find and read the memory about X", "class": 1}),
                json!({"input": "look up this topic in my memory and show it", "class": 2}),
                json!({"input": "recall and display this memory doc", "class": 2}),
                json!({"input": "search then read the result", "class": 1}),
                json!({"input": "find this note and open it", "class": 2}),
                json!({"input": "memory search and read", "class": 1}),
                json!({"input": "recall the document about X", "class": 2}),
                json!({"input": "find this memory entry and show its contents", "class": 1}),
            ],
        )
        .await?;

    // 7. Append children to the per-tool catalogues (dedup-idempotent).
    let ext_memory_search_children: Vec<Uuid> = vec![
        tool_memory_search, ts_memory_search, pc_exec_memory_search,
        skill_memory_search, skill_memory_search_broad, skill_memory_search_and_read,
        recipe_memory_search, recipe_memory_search_broad, recipe_memory_search_and_read,
    ];
    let ext_memory_write_children: Vec<Uuid> = vec![
        tool_memory_write, ts_memory_write, pc_exec_memory_write, pc_exec_memory_patch,
        pc_exec_memory_append, pc_memory_format_entry,
        skill_memory_write_log, skill_memory_write_main, skill_memory_write_patch,
        skill_memory_write_append, skill_memory,
        recipe_memory_write, recipe_memory_write_log, recipe_memory_write_main,
        recipe_memory_write_patch, recipe_memory_write_append,
    ];
    let ext_memory_read_children: Vec<Uuid> = vec![
        tool_memory_read, ts_memory_read, pc_exec_memory_read, pc_memory_extract_section,
        skill_memory_read,
        recipe_memory_read, recipe_memory_read_main, recipe_memory_read_heartbeat,
    ];
    let ext_memory_tree_children: Vec<Uuid> = vec![
        tool_memory_tree, ts_memory_tree, pc_exec_memory_tree,
        skill_memory_tree, recipe_memory_tree,
    ];
    stores
        .append_children(cat_memory_search, &ext_memory_search_children)
        .await?;
    stores
        .append_children(cat_memory_write, &ext_memory_write_children)
        .await?;
    stores
        .append_children(cat_memory_read, &ext_memory_read_children)
        .await?;
    stores
        .append_children(cat_memory_tree, &ext_memory_tree_children)
        .await?;
    // Primary catalogue owns the union of all four per-tool child sets.
    stores
        .append_children(cat_memory, &ext_memory_search_children)
        .await?;
    stores
        .append_children(cat_memory, &ext_memory_write_children)
        .await?;
    stores
        .append_children(cat_memory, &ext_memory_read_children)
        .await?;
    stores
        .append_children(cat_memory, &ext_memory_tree_children)
        .await?;

    tracing::debug!(
        "seeded memory group chunk 5d: 12 recipes (10 Tier-0 + 2 Tier-1) + catalogue appends - memory group COMPLETE (4 tools + 4 toolskills + 8 PythonCode + 10 skills + 12 recipes + 5 catalogues)"
    );

    Ok(())
}

fn memory_primary_catalogue_row(tenant: &str) -> NewPgExtensionCatalogue {
    NewPgExtensionCatalogue {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: CAT_MEMORY.to_string(),
        description: "Memory domain capability catalogue (memory_search, memory_write, \
                       memory_read, memory_tree)."
            .to_string(),
        version: "1.0".into(),
        overview_doc: CAT_MEMORY_OVERVIEW.into(),
        task_groups: json!([
            {"group_name": "memory-search", "description": "Semantic search and recall"},
            {"group_name": "memory-write", "description": "Log, update, and patch memory documents"},
            {"group_name": "memory-read", "description": "Read memory documents by path"},
            {"group_name": "memory-tree", "description": "Browse the memory directory structure"}
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

fn tool_memory_search_row(tenant: &str) -> NewPgTool {
    NewPgTool {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "memory_search".to_string(),
        description: "Search the agent's persistent memory store using a natural language \
                       query. Returns the most relevant memory documents ranked by semantic \
                       similarity. Limit defaults to 5; maximum is 20."
            .to_string(),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Natural language search query"},
                "limit": {"type": "integer", "minimum": 1, "maximum": 20, "default": 5}
            },
            "required": ["query"],
            "additionalProperties": false
        })),
        param_template: Some(json!({"query": "{{query}}"})),
        effect_type: "read_memory".to_string(),
        preconditions: Some("query must not be empty".into()),
        error_handling: Some(
            "empty result is not an error; memory backend unavailable -> tool error".into(),
        ),
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        source: "system".into(),
        validation_status: "validated".into(),
        capability_id: "builtin.memory_search".into(),
    }
}

fn tool_memory_write_row(tenant: &str) -> NewPgTool {
    NewPgTool {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "memory_write".to_string(),
        description: "Write or append content to the agent's persistent memory. Default \
                       target is 'daily_log' (today's dated log). Other targets: 'memory' \
                       (MEMORY.md), 'heartbeat' (HEARTBEAT.md), 'bootstrap' (clears \
                       BOOTSTRAP.md), or any relative memory document path. Supports patch \
                       mode (old_string/new_string)."
            .to_string(),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "content":     {"type": "string",  "description": "Content to write or append"},
                "target":      {"type": "string",  "description": "Destination: 'memory', 'daily_log' (default), 'heartbeat', 'bootstrap', or relative path"},
                "append":      {"type": "boolean", "description": "Append when true; replace when false", "default": true},
                "metadata":    {"type": "object",  "description": "Optional document metadata"},
                "old_string":  {"type": "string",  "description": "Exact text to replace (patch mode)"},
                "new_string":  {"type": "string",  "description": "Replacement text (patch mode)"},
                "replace_all": {"type": "boolean", "description": "Replace every old_string occurrence"},
                "timezone":    {"type": "string",  "description": "IANA timezone for daily_log date resolution"}
            },
            "additionalProperties": false
        })),
        param_template: Some(json!({"content": "{{content}}"})),
        effect_type: "write_memory".to_string(),
        preconditions: Some("content required unless using bootstrap target".into()),
        error_handling: Some(
            "old_string not found in patch mode -> tool error; write failure -> tool error".into(),
        ),
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        source: "system".into(),
        validation_status: "validated".into(),
        capability_id: "builtin.memory_write".into(),
    }
}

fn tool_memory_read_row(tenant: &str) -> NewPgTool {
    NewPgTool {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "memory_read".to_string(),
        description: "Read a specific memory document by its relative path. Returns the full \
                       document content. Use memory_search for semantic discovery; use \
                       memory_read when you know the exact path."
            .to_string(),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Relative memory document path to read"}
            },
            "required": ["path"],
            "additionalProperties": false
        })),
        param_template: Some(json!({"path": "{{path}}"})),
        effect_type: "read_memory".to_string(),
        preconditions: Some("path must not be empty".into()),
        error_handling: Some("document not found -> tool error".into()),
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        source: "system".into(),
        validation_status: "validated".into(),
        capability_id: "builtin.memory_read".into(),
    }
}

fn tool_memory_tree_row(tenant: &str) -> NewPgTool {
    NewPgTool {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "memory_tree".to_string(),
        description: "List the directory tree of the agent's persistent memory. Returns \
                       entry names and types up to the specified depth. Used to discover \
                       memory structure before targeted reads."
            .to_string(),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "path":  {"type": "string",  "description": "Relative memory directory path (omit for root)"},
                "depth": {"type": "integer", "minimum": 1, "maximum": 10, "default": 1}
            },
            "additionalProperties": false
        })),
        param_template: Some(json!({})),
        effect_type: "read_memory".to_string(),
        preconditions: Some("path, if supplied, must resolve within the memory mount".into()),
        error_handling: Some("path not found in memory -> tool error".into()),
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        source: "system".into(),
        validation_status: "validated".into(),
        capability_id: "builtin.memory_tree".into(),
    }
}

fn ts_memory_search_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-memory-search".to_string(),
        description: "Executor binding for memory_search. Required: query (natural language). \
                       Optional: limit (1-20, default 5). Returns ranked memory documents with \
                       content and relevance scores."
            .to_string(),
        content: "Call `host.memory_search(query=<natural language query>, limit=<optional \
                  1..20>)` to search the agent's persistent memory. Results are ranked by \
                  semantic similarity. Empty results are not an error - check the returned \
                  list length before using a result."
            .to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("memory_search".to_string()),
        param_schema: Some(json!([
            {"name": "query", "param_type": "string", "required": true, "description": "Natural language search query"},
            {"name": "limit", "param_type": "integer", "required": false, "description": "Max results to return (1..20, default 5)"}
        ])),
        param_template: Some(json!({"query": "{{query}}"})),
        consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

fn ts_memory_write_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-memory-write".to_string(),
        description: "Executor binding for memory_write. Default writes to 'daily_log' (append \
                       mode). Use target='memory' for MEMORY.md. Patch mode: supply old_string \
                       + new_string. Setting append=false replaces the full document."
            .to_string(),
        content: "Call `host.memory_write(content=<text>, target=<optional 'memory'|'daily_log'|\
                  'heartbeat'|'bootstrap'|relative path>, append=<optional bool>, old_string=\
                  <optional>, new_string=<optional>, replace_all=<optional bool>, timezone=\
                  <optional IANA>)` to write to persistent memory. Default target is 'daily_log' \
                  with append=true. Patch mode requires both old_string and new_string."
            .to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("memory_write".to_string()),
        param_schema: Some(json!([
            {"name": "content", "param_type": "string", "required": false, "description": "Content to write or append"},
            {"name": "target", "param_type": "string", "required": false, "description": "Destination: memory|daily_log|heartbeat|bootstrap|relative path"},
            {"name": "append", "param_type": "boolean", "required": false, "description": "Append (true) or replace (false)"},
            {"name": "old_string", "param_type": "string", "required": false, "description": "Exact text to replace (patch mode)"},
            {"name": "new_string", "param_type": "string", "required": false, "description": "Replacement text (patch mode)"},
            {"name": "replace_all", "param_type": "boolean", "required": false, "description": "Replace every old_string occurrence"},
            {"name": "timezone", "param_type": "string", "required": false, "description": "IANA timezone for daily_log date resolution"}
        ])),
        param_template: Some(json!({"content": "{{content}}"})),
        consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

fn ts_memory_read_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-memory-read".to_string(),
        description: "Executor binding for memory_read. Required: path (relative memory \
                       document path). Returns the full document content. Use for known \
                       paths; use ts-memory-search for semantic discovery."
            .to_string(),
        content: "Call `host.memory_read(path=<relative memory document path>)` to read a \
                  memory document by exact path. Returns the full document content. Use \
                  ts-memory-search when the path is unknown."
            .to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("memory_read".to_string()),
        param_schema: Some(json!([
            {"name": "path", "param_type": "string", "required": true, "description": "Relative memory document path to read"}
        ])),
        param_template: Some(json!({"path": "{{path}}"})),
        consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

fn ts_memory_tree_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-memory-tree".to_string(),
        description: "Executor binding for memory_tree. Optional: path (relative memory dir, \
                       defaults to root), depth (1-10, default 1). Returns the directory tree \
                       of persistent memory."
            .to_string(),
        content: "Call `host.memory_tree(path=<optional relative dir>, depth=<optional 1..10>)` \
                  to list the memory directory tree. Omit both to get the root at depth=1. Use \
                  the result to decide which documents to read with ts-memory-read."
            .to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("memory_tree".to_string()),
        param_schema: Some(json!([
            {"name": "path", "param_type": "string", "required": false, "description": "Relative memory directory path (omit for root)"},
            {"name": "depth", "param_type": "integer", "required": false, "description": "Max directory depth (1..10, default 1)"}
        ])),
        param_template: Some(json!({})),
        consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

// ---------------------------------------------------------------------------
// Memory group PythonCode bodies (class 22) — transcribed verbatim from the
// doc. Bodies use `host.<tool>(kwarg=value)` dispatch with `{{vars.slotN}}`
// vars; no imports.
// ---------------------------------------------------------------------------

const PC_EXEC_MEMORY_SEARCH_CONTENT: &str = r#"# Orchestrator executor body.
_query = "{{vars.slot0}}"
_limit = {{vars.slot1}}
_params = {"query": _query}
if _limit and _limit > 0:
    _params["limit"] = _limit
result = host.memory_search(**_params)
"#;

const PC_EXEC_MEMORY_WRITE_CONTENT: &str = r#"# Orchestrator executor body.
_content = "{{vars.slot0}}"
_target = "{{vars.slot1}}"
_append = {{vars.slot2}}
_params = {"content": _content}
if _target and _target != "":
    _params["target"] = _target
if _append is not None:
    _params["append"] = _append
result = host.memory_write(**_params)
"#;

const PC_EXEC_MEMORY_PATCH_CONTENT: &str = r#"# Orchestrator executor body.
_target = "{{vars.slot0}}"
_old = "{{vars.slot1}}"
_new = "{{vars.slot2}}"
_replace_all = {{vars.slot3}}
_params = {"target": _target, "old_string": _old, "new_string": _new}
if _replace_all:
    _params["replace_all"] = True
result = host.memory_write(**_params)
"#;

const PC_EXEC_MEMORY_READ_CONTENT: &str = r#"# Orchestrator executor body.
_path = "{{vars.slot0}}"
result = host.memory_read(path=_path)
"#;

const PC_EXEC_MEMORY_TREE_CONTENT: &str = r#"# Orchestrator executor body.
_path = "{{vars.slot0}}"
_depth = {{vars.slot1}}
_params = {}
if _path and _path != "":
    _params["path"] = _path
if _depth and _depth > 0:
    _params["depth"] = _depth
result = host.memory_tree(**_params)
"#;

const PC_MEMORY_EXTRACT_SECTION_CONTENT: &str = r##"# No I/O, no imports. IBS bakes in content and heading before execution.
content = "{{vars.slot0}}"
heading = "{{vars.slot1}}"
lines = content.split("\n")
in_section = False
section_lines = []
for line in lines:
    stripped = line.lstrip("#").strip()
    if stripped == heading and line.startswith("#"):
        in_section = True
        continue
    if in_section:
        if line.startswith("#"):
            break
        section_lines.append(line)
section_content = "\n".join(section_lines).strip() if section_lines else None
result = {"section_content": section_content, "heading": heading, "found": section_content is not None}
"##;

const PC_MEMORY_FORMAT_ENTRY_CONTENT: &str = r####"# No I/O, no imports, no datetime. Caller must supply timestamp_str.
text = "{{vars.slot0}}"
timestamp_str = "{{vars.slot1}}"
formatted_entry = f"### {timestamp_str}\n\n{text}\n"
result = {"formatted_entry": formatted_entry}
"####;

// ---------------------------------------------------------------------------
// Memory group skill bodies — transcribed verbatim from the doc (Q1=A).
// Leaf skills are class 1; skill-memory is the class-2 domain skill. Em-dash
// list markers match the existing SKILL_FILESYSTEM_BODY convention.
// ---------------------------------------------------------------------------

const SKILL_MEMORY_SEARCH_BODY: &str = r#"Use `ts-memory-search` (via pc-exec-memory-search) when you need to recall past work, find saved notes, or check whether something was previously recorded. Provide a natural language query that describes what you are looking for. Set limit higher (up to 20) when broader recall coverage is needed. Review the returned documents and surface only those relevant to the current context.
"#;

const SKILL_MEMORY_SEARCH_BROAD_BODY: &str = r#"When a topic may span multiple memory documents, use `ts-memory-search` with `limit=20` to cast a wider net. Review all returned documents before deciding which are relevant. This is useful for session start — recovering full context about a project or topic before beginning work.
"#;

const SKILL_MEMORY_WRITE_LOG_BODY: &str = r#"Use `ts-memory-write` (via pc-exec-memory-write) with the default target='daily_log' and append=true to add timestamped progress notes, decisions, or session context to today's dated log. This is the lightest-weight memory write — use it frequently to maintain a running record of work within a session.
"#;

const SKILL_MEMORY_WRITE_MAIN_BODY: &str = r#"Use `ts-memory-write` with target='memory' to update the primary MEMORY.md document. With append=true, content is added to the end. With append=false, the entire document is replaced — use this only when intentionally rebuilding the memory from scratch. For targeted updates (patch a section), use skill-memory-write-patch instead.
"#;

const SKILL_MEMORY_WRITE_PATCH_BODY: &str = r#"Use `ts-memory-write` in patch mode (old_string + new_string) to replace a specific section of a memory document without rewriting the whole file. Read the document first with skill-memory-read to find the exact text to replace. Use replace_all=true when the same string appears multiple times and all occurrences should change.
"#;

const SKILL_MEMORY_READ_BODY: &str = r#"Use `ts-memory-read` (via pc-exec-memory-read) when you know the exact path of a memory document (e.g. MEMORY.md, HEARTBEAT.md, or a specific note file). Returns the full content of the document. If you do not know the exact path, use skill-memory-search to discover it first, or use skill-memory-tree to browse the directory structure.
"#;

const SKILL_MEMORY_TREE_BODY: &str = r#"Use `ts-memory-tree` (via pc-exec-memory-tree) to discover what memory documents exist. Call with no parameters to get the root structure at depth=1. Increase depth to see deeper levels. Use the returned structure to decide which documents to read with skill-memory-read or to inform a skill-memory-search query.
"#;

const SKILL_MEMORY_SEARCH_AND_READ_BODY: &str = r#"Use when the user wants to recall information and immediately see the full content — not just the search summary. The pattern:
1. Call ts-memory-search with the topic query (via pc-exec-memory-search).
2. Take the highest-scoring result's path from the search output.
3. Call ts-memory-read with that path (via pc-exec-memory-read).
4. Return the full document content.

If no results are found, report that no memory matches the topic. Do not fabricate a document path. Always check the search result before reading.
"#;

const SKILL_MEMORY_BODY: &str = r#"The memory domain provides four tools for the agent's persistent memory store:

READING / DISCOVERING:
— skill-memory-search: Semantic search by topic — use when path is unknown.
— skill-memory-search-broad: Wide recall with limit=20 for session start.
— skill-memory-search-and-read: Search + immediately read the top result.
— skill-memory-read: Read a specific document by exact path.
— skill-memory-tree: Browse the directory structure.

WRITING:
— skill-memory-write-log: Append a note to today's daily_log (default).
— skill-memory-write-main: Update the main MEMORY.md document.
— skill-memory-write-patch: Targeted patch of an existing memory document.

Decision guide:
— Recalling by topic (summary list) -> skill-memory-search
— Recalling + reading the top result in one step -> skill-memory-search-and-read
— Session start full recall -> skill-memory-search-broad
— Reading a known file -> skill-memory-read
— Logging progress -> skill-memory-write-log
— Updating permanent context -> skill-memory-write-main
— Patching a section -> skill-memory-write-patch
— Discovering what files exist -> skill-memory-tree

Orchestrator note: NEVER use datetime.now() in PythonCode. Always call skill-time-now first to get a timestamp, then pass it to pc-memory-format-entry.
"#;

// ---------------------------------------------------------------------------
// Memory group chunk 5d — Step 11.x variant component bodies.
// ---------------------------------------------------------------------------

const PC_EXEC_MEMORY_APPEND_CONTENT: &str = r#"# Append pattern: read existing, concat new content, write back.
_path    = "{{vars.slot0}}"
_new_txt = "{{vars.slot1}}"
_existing = host.memory_read(path=_path)
_current = _existing.get("content", "") if isinstance(_existing, dict) else ""
_combined = _current.rstrip("\n") + "\n\n" + _new_txt
result = host.memory_write(path=_path, content=_combined)
"#;

const SKILL_MEMORY_WRITE_APPEND_BODY: &str = r#"Use pc-exec-memory-append to add content to an existing memory document without overwriting
it. This pattern reads the current content, appends the new text (with blank line separation),
and writes back. Use for:
- Running logs (CHANGELOG.md, decision_log.md)
- Incremental session notes where each session adds an entry
- Any document that grows over time
If the document does not exist yet, use skill-memory-write-log to create it first.
"#;

// ---------------------------------------------------------------------------
// Memory group chunk 5d — recipe YAML sources (verbatim doc step_descriptions
// blocks; WebUI renderer reads these, the IBS reads the resolved `steps`).
// ---------------------------------------------------------------------------

const RECIPE_MEMORY_SEARCH_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-search>"],
    "label":   "Pre-load ts-memory-search ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-memory-search>"],
    "label":   "PythonCode calls host.memory_search(query, limit)"
  }
]
"#;

const RECIPE_MEMORY_SEARCH_BROAD_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-search>"],
    "label":   "Pre-load ts-memory-search ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-memory-search>"],
    "label":   "PythonCode calls host.memory_search(query, limit=20)"
  }
]
"#;

const RECIPE_MEMORY_WRITE_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-write>"],
    "label":   "Pre-load ts-memory-write ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-memory-write>"],
    "label":   "PythonCode calls host.memory_write(content, target, append)"
  }
]
"#;

const RECIPE_MEMORY_WRITE_LOG_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-write>"],
    "label":   "Pre-load ts-memory-write ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-memory-write>"],
    "label":   "PythonCode calls host.memory_write(content, target='daily_log', append=true)"
  }
]
"#;

const RECIPE_MEMORY_WRITE_MAIN_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-write>"],
    "label":   "Pre-load ts-memory-write ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-memory-write>"],
    "label":   "PythonCode calls host.memory_write(content, target='memory', append=true)"
  }
]
"#;

const RECIPE_MEMORY_WRITE_PATCH_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-write>"],
    "label":   "Pre-load ts-memory-write ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-memory-patch>"],
    "label":   "PythonCode calls host.memory_write(target, old_string, new_string)"
  }
]
"#;

const RECIPE_MEMORY_WRITE_APPEND_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-memory-write-append>", "<uuid:skill-memory-read>"],
    "label":   "Load append + read leaf skills"
  },
  {
    "step_id": "step-2",
    "type":    "llm",
    "label":   "LLM composes the new text to append based on current context"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-read>", "<uuid:ts-memory-write>"],
    "label":   "Pre-load ts-memory-read and ts-memory-write ToolSkill bindings"
  }
]
"#;

const RECIPE_MEMORY_READ_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-read>"],
    "label":   "Pre-load ts-memory-read ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-memory-read>"],
    "label":   "PythonCode calls host.memory_read(path)"
  }
]
"#;

const RECIPE_MEMORY_READ_MAIN_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-read>"],
    "label":   "Pre-load ts-memory-read ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-memory-read>"],
    "label":   "PythonCode calls host.memory_read(path='MEMORY.md')"
  }
]
"#;

const RECIPE_MEMORY_READ_HEARTBEAT_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-read>"],
    "label":   "Pre-load ts-memory-read ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-memory-read>"],
    "label":   "PythonCode calls host.memory_read(path='HEARTBEAT.md')"
  }
]
"#;

const RECIPE_MEMORY_TREE_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-tree>"],
    "label":   "Pre-load ts-memory-tree ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-memory-tree>"],
    "label":   "PythonCode calls host.memory_tree(path, depth)"
  }
]
"#;

const RECIPE_MEMORY_SEARCH_AND_READ_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-memory-search-and-read>"],
    "label":   "Load search-and-read combined leaf skill body"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-memory-search>", "<uuid:ts-memory-read>"],
    "label":   "Pre-load both ToolSkill bindings"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-memory-search>", "<uuid:pc-exec-memory-read>"],
    "label":   "PythonCode: search memory, take top result path, read document"
  },
  {
    "step_id": "step-4",
    "type":    "llm",
    "label":   "LLM interprets query intent, selects best result path, presents content"
  }
]
"#;

// ---------------------------------------------------------------------------
// Process group (Pass 4) — shell, spawn_subagent, trigger management.
// Catalogue names + overview docs (transcribed verbatim from the doc).
// ---------------------------------------------------------------------------

/// Process domain primary ExtensionCatalogue name (Step 25).
const CAT_PROCESS: &str = "builtin-process";

const CAT_EXT_SHELL_OVERVIEW: &str = r#"# Shell Execution Capability
Tool: builtin.shell
Effect: mixed (sandboxed subprocess)
Permission: Ask

TWO TIERS of shell execution in this catalogue:

§shell-safe-fixed (Tier 0): Fixed-literal pre-validated commands.
No user input enters the command string — zero injection surface.
- Git inspection: → shell-git-status, shell-git-log, shell-git-diff-stat,
  shell-git-branch, shell-git-stash-list, shell-git-remote, shell-git-show-stat,
  shell-git-tag-list recipes (all Tier 0)
- System info: → shell-pwd, shell-df, shell-ps, shell-env, shell-uname,
  shell-which, shell-date, shell-hostname, shell-whoami, shell-uptime,
  shell-free, shell-wc-l recipes (all Tier 0)

§shell-guard-custom (Tier 1): User-composed or user-supplied commands.
LLM must validate and compose the exact command before dispatch.
- Single custom command: → shell-run recipe (Tier 1)
- Multi-line script: → shell-script recipe (Tier 1)

Prefer structured filesystem, network, and memory tools over shell whenever possible.
Shell is the last resort when no structured tool covers the need.
"#;

const CAT_EXT_SPAWN_SUBAGENT_OVERVIEW: &str = r#"# Child Agent Delegation Capability
Tool: builtin.spawn_subagent
Effect: ExternalWrite

§spawn_subagent-guard: ALL recipes using this tool are Tier 1 (llm_call_required=true).
The LLM MUST frame the goal and confirm delegation. No Tier-0 spawn dispatch.

Approaches:
- Generic goal delegation: write clear goal + context → subagent-spawn recipe (Tier 1)
- Named procedure: specify recipe_name → subagent-spawn recipe (Tier 1)
- Research delegation: focused info-gathering → subagent-research recipe (Tier 1)
- Coding delegation: file read/write/patch task → subagent-coding recipe (Tier 1)
- Exploration delegation: deep read-only analysis → subagent-exploration recipe (Tier 1)
- Query delegation: focused single-question lookup → subagent-query recipe (Tier 1)

Choose the most specific recipe — the intent system routes the user's phrasing here
and the pre-loaded leaf skill body gives the LLM the right framing before it writes
the goal string.
"#;

const CAT_EXT_TRIGGER_MANAGEMENT_OVERVIEW: &str = r#"# Trigger Management Capability
Tools: builtin.trigger_list, builtin.trigger_create, builtin.trigger_remove
Effects: Read (list), ExternalWrite (create/remove)

Manages persistent scheduled triggers. List is Tier 0. Create and Remove are Tier 1
(ExternalWrite effect, user confirmation required).

Approaches:
- List all triggers: → trigger-list recipe (Tier 0)
- Create a trigger: → trigger-create recipe (Tier 1)
- Remove a trigger (generic): → trigger-remove recipe (Tier 1 — LLM resolves name)
- Remove a trigger by exact name: → trigger-remove-by-name recipe (Tier 1 — LLM confirms,
  PythonCode resolves and removes — no LLM disambiguation of the name)
"#;

const CAT_PROCESS_OVERVIEW: &str = r#"# Process & Scheduling Capabilities

The process domain covers: shell command execution (two tiers), child agent delegation,
and persistent trigger scheduling.

## Tools in this domain
- builtin.shell          — run a shell command in a sandboxed subprocess
- builtin.spawn_subagent — delegate a sub-goal to a child agent run
- builtin.trigger_create — create a scheduled or event-driven trigger
- builtin.trigger_list   — list configured triggers (read-only)
- builtin.trigger_remove — remove a trigger (irreversible)

## Shell safety — two tiers
§shell-safe-fixed (Tier 0): Fixed-literal pre-validated commands.
No user input enters the command string — the PythonCode hardcodes the command.
Git inspection (git status, log, diff --stat, branch) and system info
(pwd, df -h, ps aux, env, uname -a, which) are all Tier 0.

§shell-guard-custom (Tier 1): User-composed or user-supplied commands.
The LLM must compose and validate the exact command before dispatch.
Never pass unvalidated user input into a custom shell command.

## Subagent invariants (§spawn_subagent-guard)
- Any recipe using builtin.spawn_subagent MUST have llm_call_required=true. No Tier-0.
- Child cannot exceed parent scope or authority.
- Include all needed context explicitly — child has no parent conversation access.

## Trigger safety
- trigger_create and trigger_remove have ExternalWrite effect — require user confirmation.
- Triggers run with the creating session's authority and cannot escalate.
"#;

/// Seed the process domain group: the primary `builtin-process` catalogue, the
/// 3 per-tool catalogues (ext-shell, ext-spawn-subagent, ext-trigger-management),
/// the 5 Tool rows (shell, spawn_subagent, trigger_create/list/remove), and the 5
/// ToolSkill rows. PythonCode, leaf Skills, the 3 Domain Skills, and Recipes are
/// added in subsequent chunks (6b-6g); their ids are appended to the catalogues'
/// `child_component_ids` as they are minted (dedup makes this idempotent).
async fn seed_process_group(
    stores: &BootstrapStores,
) -> Result<(), SeedBuiltinBootstrapError> {
    let tenant = stores.tenant.clone();

    // 1. Primary domain catalogue + per-tool catalogues (empty child_ids;
    //    appended to as children are minted below and in later chunks).
    let cat_process = stores
        .upsert_catalogue(process_primary_catalogue_row(&tenant), CAT_PROCESS)
        .await?;
    let cat_shell = stores
        .upsert_catalogue(
            ext_catalogue_row(
                &tenant,
                "ext-shell",
                "Shell execution capability (builtin.shell).",
                CAT_EXT_SHELL_OVERVIEW,
                json!([
                    {"group_name": "shell-safe-fixed-git", "description": "Fixed-literal git commands (Tier 0, no LLM)"},
                    {"group_name": "shell-safe-fixed-sysinfo", "description": "Fixed-literal system info commands (Tier 0, no LLM)"},
                    {"group_name": "shell-custom", "description": "User-composed shell commands (Tier 1, LLM required)"}
                ]),
            ),
            "ext-shell",
        )
        .await?;
    let cat_spawn = stores
        .upsert_catalogue(
            ext_catalogue_row(
                &tenant,
                "ext-spawn-subagent",
                "Child agent delegation capability (builtin.spawn_subagent).",
                CAT_EXT_SPAWN_SUBAGENT_OVERVIEW,
                json!([
                    {"group_name": "subagent-goal", "description": "Delegate a self-contained sub-goal to a child agent"},
                    {"group_name": "subagent-procedure", "description": "Run a named recipe as a child agent procedure"},
                    {"group_name": "subagent-typed", "description": "Flavour-specific delegation: research, coding, exploration, query"}
                ]),
            ),
            "ext-spawn-subagent",
        )
        .await?;
    let cat_trigger = stores
        .upsert_catalogue(
            ext_catalogue_row(
                &tenant,
                "ext-trigger-management",
                "Trigger management capability (builtin.trigger_create/list/remove).",
                CAT_EXT_TRIGGER_MANAGEMENT_OVERVIEW,
                json!([
                    {"group_name": "trigger-list", "description": "Enumerate configured triggers"},
                    {"group_name": "trigger-create", "description": "Schedule a new trigger"},
                    {"group_name": "trigger-remove", "description": "Remove a scheduled trigger"},
                    {"group_name": "trigger-remove-by-name", "description": "Remove by exact name — PythonCode does list+resolve, LLM only confirms"}
                ]),
            ),
            "ext-trigger-management",
        )
        .await?;

    // 2. Tool rows (class 0) — capability_id is the literal `builtin.X`.
    let tool_shell = stores
        .upsert_tool(tool_shell_row(&tenant), "shell")
        .await?;
    let tool_spawn_subagent = stores
        .upsert_tool(tool_spawn_subagent_row(&tenant), "spawn_subagent")
        .await?;
    let tool_trigger_create = stores
        .upsert_tool(tool_trigger_create_row(&tenant), "trigger_create")
        .await?;
    let tool_trigger_list = stores
        .upsert_tool(tool_trigger_list_row(&tenant), "trigger_list")
        .await?;
    let tool_trigger_remove = stores
        .upsert_tool(tool_trigger_remove_row(&tenant), "trigger_remove")
        .await?;

    // 3. ToolSkill rows (class 13).
    let ts_shell_run = stores
        .upsert_tool_skill(ts_shell_run_row(&tenant), "ts-shell-run")
        .await?;
    let ts_spawn_subagent = stores
        .upsert_tool_skill(ts_spawn_subagent_row(&tenant), "ts-spawn-subagent")
        .await?;
    let ts_trigger_create = stores
        .upsert_tool_skill(ts_trigger_create_row(&tenant), "ts-trigger-create")
        .await?;
    let ts_trigger_list = stores
        .upsert_tool_skill(ts_trigger_list_row(&tenant), "ts-trigger-list")
        .await?;
    let ts_trigger_remove = stores
        .upsert_tool_skill(ts_trigger_remove_row(&tenant), "ts-trigger-remove")
        .await?;

    // 4. PythonCode rows (class 22) — shell executors. Transcribed verbatim from
    //    the doc (Q1 decision A); bodies use `host.shell(command=...)` dispatch.
    //    pc_row overrides the doc's `05:validator` tag with the SEC-01-safe
    //    `["01:monty","02:orchestrator"]` (pg_python_code_store hides
    //    `05:validator` rows even when validated).
    let pc_exec_shell_git_status = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-git-status",
                "Orchestrator executor (§shell-safe-fixed): runs 'git status' in the workspace \
                 root via builtin.shell. Command is a fixed literal. No user input enters the \
                 command string. Output: {output, exit_code, success}.",
                PC_EXEC_SHELL_GIT_STATUS_CONTENT,
            ),
            "pc-exec-shell-git-status",
        )
        .await?;
    let pc_exec_shell_git_log = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-git-log",
                "Orchestrator executor (§shell-safe-fixed): runs 'git log --oneline -20' to get \
                 the last 20 commits. Fixed literal command.",
                PC_EXEC_SHELL_GIT_LOG_CONTENT,
            ),
            "pc-exec-shell-git-log",
        )
        .await?;
    let pc_exec_shell_git_diff_stat = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-git-diff-stat",
                "Orchestrator executor (§shell-safe-fixed): runs 'git diff --stat' to show \
                 changed file summary. Fixed literal command.",
                PC_EXEC_SHELL_GIT_DIFF_STAT_CONTENT,
            ),
            "pc-exec-shell-git-diff-stat",
        )
        .await?;
    let pc_exec_shell_git_branch = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-git-branch",
                "Orchestrator executor (§shell-safe-fixed): runs 'git branch -a' to list all \
                 local and remote branches. Fixed literal command.",
                PC_EXEC_SHELL_GIT_BRANCH_CONTENT,
            ),
            "pc-exec-shell-git-branch",
        )
        .await?;
    let pc_exec_shell_git_stash_list = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-git-stash-list",
                "Orchestrator executor (§shell-safe-fixed): runs 'git stash list' to show the \
                 stash stack. Fixed literal command.",
                PC_EXEC_SHELL_GIT_STASH_LIST_CONTENT,
            ),
            "pc-exec-shell-git-stash-list",
        )
        .await?;
    let pc_exec_shell_git_log_n = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-git-log-n",
                "Orchestrator executor (§shell-safe-fixed variant): runs 'git log --oneline -N' \
                 where N is a validated integer (1–100). Input: count (int, 1–100).",
                PC_EXEC_SHELL_GIT_LOG_N_CONTENT,
            ),
            "pc-exec-shell-git-log-n",
        )
        .await?;
    let pc_exec_shell_git_remote = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-git-remote",
                "Orchestrator executor (§shell-safe-fixed): runs 'git remote -v' to list all \
                 configured remote repositories and their URLs. Fixed literal command.",
                PC_EXEC_SHELL_GIT_REMOTE_CONTENT,
            ),
            "pc-exec-shell-git-remote",
        )
        .await?;
    let pc_exec_shell_git_show_stat = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-git-show-stat",
                "Orchestrator executor (§shell-safe-fixed): runs 'git show --stat HEAD' to show \
                 the last commit's changed files and line counts. Fixed literal command.",
                PC_EXEC_SHELL_GIT_SHOW_STAT_CONTENT,
            ),
            "pc-exec-shell-git-show-stat",
        )
        .await?;
    let pc_exec_shell_git_tag_list = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-git-tag-list",
                "Orchestrator executor (§shell-safe-fixed): runs 'git tag --list' to enumerate \
                 all tags in the repository. Fixed literal command.",
                PC_EXEC_SHELL_GIT_TAG_LIST_CONTENT,
            ),
            "pc-exec-shell-git-tag-list",
        )
        .await?;
    let pc_exec_shell_git_diff_name_only = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-git-diff-name-only",
                "Orchestrator executor (§shell-safe-fixed): runs 'git diff --name-only HEAD' to \
                 list only the names of files changed since the last commit. No content shown. \
                 Fixed literal command — no slot interpolation.",
                PC_EXEC_SHELL_GIT_DIFF_NAME_ONLY_CONTENT,
            ),
            "pc-exec-shell-git-diff-name-only",
        )
        .await?;
    let pc_exec_shell_git_log_stat = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-git-log-stat",
                "Orchestrator executor (§shell-safe-fixed): runs 'git log --stat --oneline -5' \
                 to show the last 5 commits with file-change counts per commit. Fixed literal.",
                PC_EXEC_SHELL_GIT_LOG_STAT_CONTENT,
            ),
            "pc-exec-shell-git-log-stat",
        )
        .await?;
    let pc_exec_shell_git_stash_show = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-git-stash-show",
                "Orchestrator executor (§shell-safe-fixed): runs 'git stash show' to show the \
                 diff summary of the most recent stash entry. Fixed literal command.",
                PC_EXEC_SHELL_GIT_STASH_SHOW_CONTENT,
            ),
            "pc-exec-shell-git-stash-show",
        )
        .await?;
    let pc_exec_shell_git_config_list = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-git-config-list",
                "Orchestrator executor (§shell-safe-fixed): runs 'git config --list' to show all \
                 active git configuration values. Fixed literal command.",
                PC_EXEC_SHELL_GIT_CONFIG_LIST_CONTENT,
            ),
            "pc-exec-shell-git-config-list",
        )
        .await?;
    let pc_exec_shell_git_add = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-git-add",
                "Orchestrator executor: calls host.<tool> to run 'git add <path>'. Input: \
                 vars.slot0 = path(s) to stage. Use '.' to stage all changes. §shell-guard-custom \
                 — path is user/LLM-supplied. Tier 1 only.",
                PC_EXEC_SHELL_GIT_ADD_CONTENT,
            ),
            "pc-exec-shell-git-add",
        )
        .await?;
    let pc_exec_shell_git_commit = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-git-commit",
                "Orchestrator executor: calls host.<tool> to run 'git commit -m <msg>'. Input: \
                 vars.slot0 = commit message (user-supplied, LLM-validated). Tier 1 only.",
                PC_EXEC_SHELL_GIT_COMMIT_CONTENT,
            ),
            "pc-exec-shell-git-commit",
        )
        .await?;
    let pc_exec_shell_git_push = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-git-push",
                "Orchestrator executor: calls host.<tool> to run 'git push <remote> <branch>'. \
                 Input: vars.slot0 = remote (e.g. 'origin'), vars.slot1 = branch (e.g. 'main'). \
                 Tier 1 only.",
                PC_EXEC_SHELL_GIT_PUSH_CONTENT,
            ),
            "pc-exec-shell-git-push",
        )
        .await?;
    let pc_exec_shell_git_pull = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-git-pull",
                "Orchestrator executor: calls host.<tool> to run 'git pull <remote> <branch>'. \
                 Input: vars.slot0 = remote, vars.slot1 = branch. Tier 1 only.",
                PC_EXEC_SHELL_GIT_PULL_CONTENT,
            ),
            "pc-exec-shell-git-pull",
        )
        .await?;
    let pc_exec_shell_git_fetch = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-git-fetch",
                "Orchestrator executor: calls host.<tool> to run 'git fetch --all'. Tier 0 safe.",
                PC_EXEC_SHELL_GIT_FETCH_CONTENT,
            ),
            "pc-exec-shell-git-fetch",
        )
        .await?;
    let pc_exec_shell_pwd = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-pwd",
                "Orchestrator executor (§shell-safe-fixed): runs 'pwd' to show the current \
                 working directory. Fixed literal command.",
                PC_EXEC_SHELL_PWD_CONTENT,
            ),
            "pc-exec-shell-pwd",
        )
        .await?;
    let pc_exec_shell_df = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-df",
                "Orchestrator executor (§shell-safe-fixed): runs 'df -h' to show disk usage in \
                 human-readable format. Fixed literal command.",
                PC_EXEC_SHELL_DF_CONTENT,
            ),
            "pc-exec-shell-df",
        )
        .await?;
    let pc_exec_shell_ps = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-ps",
                "Orchestrator executor (§shell-safe-fixed): runs 'ps aux' to list running \
                 processes. Fixed literal command.",
                PC_EXEC_SHELL_PS_CONTENT,
            ),
            "pc-exec-shell-ps",
        )
        .await?;
    let pc_exec_shell_env = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-env",
                "Orchestrator executor (§shell-safe-fixed): runs 'env' to list all environment \
                 variables in the current session. Fixed literal command.",
                PC_EXEC_SHELL_ENV_CONTENT,
            ),
            "pc-exec-shell-env",
        )
        .await?;
    let pc_exec_shell_uname = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-uname",
                "Orchestrator executor (§shell-safe-fixed): runs 'uname -a' to show OS/kernel \
                 information. Fixed literal command.",
                PC_EXEC_SHELL_UNAME_CONTENT,
            ),
            "pc-exec-shell-uname",
        )
        .await?;
    let pc_exec_shell_which = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-which",
                "Orchestrator executor (§shell-safe-fixed variant): runs 'which <toolname>' to \
                 locate a binary. Input: tool_name (string, must be a safe identifier matching \
                 [a-zA-Z0-9_-]+). Validates before dispatch.",
                PC_EXEC_SHELL_WHICH_CONTENT,
            ),
            "pc-exec-shell-which",
        )
        .await?;
    let pc_exec_shell_date = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-date",
                "Orchestrator executor (§shell-safe-fixed): runs 'date -u +%Y-%m-%dT%H:%M:%SZ' \
                 to print the current UTC date/time as ISO-8601. Fixed literal command.",
                PC_EXEC_SHELL_DATE_CONTENT,
            ),
            "pc-exec-shell-date",
        )
        .await?;
    let pc_exec_shell_hostname = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-hostname",
                "Orchestrator executor (§shell-safe-fixed): runs 'hostname' to print the machine \
                 hostname. Fixed literal command.",
                PC_EXEC_SHELL_HOSTNAME_CONTENT,
            ),
            "pc-exec-shell-hostname",
        )
        .await?;
    let pc_exec_shell_whoami = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-whoami",
                "Orchestrator executor (§shell-safe-fixed): runs 'whoami' to print the current \
                 user account name. Fixed literal command.",
                PC_EXEC_SHELL_WHOAMI_CONTENT,
            ),
            "pc-exec-shell-whoami",
        )
        .await?;
    let pc_exec_shell_uptime = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-uptime",
                "Orchestrator executor (§shell-safe-fixed): runs 'uptime' to show system uptime, \
                 load average, and logged-in user count. Fixed literal command.",
                PC_EXEC_SHELL_UPTIME_CONTENT,
            ),
            "pc-exec-shell-uptime",
        )
        .await?;
    let pc_exec_shell_free = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-free",
                "Orchestrator executor (§shell-safe-fixed): runs 'free -h' to show memory usage \
                 in human-readable format. Fixed literal command.",
                PC_EXEC_SHELL_FREE_CONTENT,
            ),
            "pc-exec-shell-free",
        )
        .await?;
    let pc_exec_shell_wc_l = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-shell-wc-l",
                "Orchestrator executor (§shell-safe-fixed variant): runs 'wc -l <filepath>' to \
                 count lines in a file. Input: filepath (string — must be a safe scoped \
                 workspace path matching no special characters). Validates before dispatch.",
                PC_EXEC_SHELL_WC_L_CONTENT,
            ),
            "pc-exec-shell-wc-l",
        )
        .await?;

    // 5. Leaf Skills (class 1) — shell subgroup. Seven bodies are transcribed
    //    verbatim from the doc (run, safe-check, git-diff-name-only, git-log-stat,
    //    git-stash-show, git-config-list, git-add). The remaining 24 are
    //    synthesized following the doc's own leaf-skill pattern (Q1 decision A:
    //    each names its exact pc, command, tier, what it returns, and the Tier-1
    //    shell-run fallback) — content-specific, not generic.
    let skill_shell_run = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-run",
                "Leaf skill: how to drive the executor to run a single shell command.",
                SKILL_SHELL_RUN_BODY,
            ),
            "skill-shell-run",
        )
        .await?;
    let skill_shell_safe_check = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-safe-check",
                "Leaf skill: safety rules for shell command execution.",
                SKILL_SHELL_SAFE_CHECK_BODY,
            ),
            "skill-shell-safe-check",
        )
        .await?;
    let skill_shell_git_status = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-git-status",
                "Leaf skill: how to check the working-tree status (modified, staged, untracked files).",
                SKILL_SHELL_GIT_STATUS_BODY,
            ),
            "skill-shell-git-status",
        )
        .await?;
    let skill_shell_git_log = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-git-log",
                "Leaf skill: how to view the recent commit history as a compact one-line list.",
                SKILL_SHELL_GIT_LOG_BODY,
            ),
            "skill-shell-git-log",
        )
        .await?;
    let skill_shell_git_diff_stat = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-git-diff-stat",
                "Leaf skill: how to view a summary of changed files and line counts.",
                SKILL_SHELL_GIT_DIFF_STAT_BODY,
            ),
            "skill-shell-git-diff-stat",
        )
        .await?;
    let skill_shell_git_branch = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-git-branch",
                "Leaf skill: how to list all local and remote branches.",
                SKILL_SHELL_GIT_BRANCH_BODY,
            ),
            "skill-shell-git-branch",
        )
        .await?;
    let skill_shell_git_stash_list = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-git-stash-list",
                "Leaf skill: how to list the stash stack.",
                SKILL_SHELL_GIT_STASH_LIST_BODY,
            ),
            "skill-shell-git-stash-list",
        )
        .await?;
    let skill_shell_git_remote = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-git-remote",
                "Leaf skill: how to list configured remote repositories and their URLs.",
                SKILL_SHELL_GIT_REMOTE_BODY,
            ),
            "skill-shell-git-remote",
        )
        .await?;
    let skill_shell_git_show_stat = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-git-show-stat",
                "Leaf skill: how to inspect the last commit's changed files and line counts.",
                SKILL_SHELL_GIT_SHOW_STAT_BODY,
            ),
            "skill-shell-git-show-stat",
        )
        .await?;
    let skill_shell_git_tag_list = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-git-tag-list",
                "Leaf skill: how to enumerate all tags in the repository.",
                SKILL_SHELL_GIT_TAG_LIST_BODY,
            ),
            "skill-shell-git-tag-list",
        )
        .await?;
    let skill_shell_git_diff_name_only = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-git-diff-name-only",
                "Leaf skill: how to list only filenames changed since the last commit (no diff content).",
                SKILL_SHELL_GIT_DIFF_NAME_ONLY_BODY,
            ),
            "skill-shell-git-diff-name-only",
        )
        .await?;
    let skill_shell_git_log_stat = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-git-log-stat",
                "Leaf skill: how to view recent commit history with file-change statistics.",
                SKILL_SHELL_GIT_LOG_STAT_BODY,
            ),
            "skill-shell-git-log-stat",
        )
        .await?;
    let skill_shell_git_stash_show = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-git-stash-show",
                "Leaf skill: how to inspect the diff summary of the most recent stash entry.",
                SKILL_SHELL_GIT_STASH_SHOW_BODY,
            ),
            "skill-shell-git-stash-show",
        )
        .await?;
    let skill_shell_git_config_list = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-git-config-list",
                "Leaf skill: how to inspect all active git configuration values.",
                SKILL_SHELL_GIT_CONFIG_LIST_BODY,
            ),
            "skill-shell-git-config-list",
        )
        .await?;
    let skill_shell_pwd = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-pwd",
                "Leaf skill: how to print the current working directory.",
                SKILL_SHELL_PWD_BODY,
            ),
            "skill-shell-pwd",
        )
        .await?;
    let skill_shell_df = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-df",
                "Leaf skill: how to show disk usage in human-readable form.",
                SKILL_SHELL_DF_BODY,
            ),
            "skill-shell-df",
        )
        .await?;
    let skill_shell_ps = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-ps",
                "Leaf skill: how to list running processes.",
                SKILL_SHELL_PS_BODY,
            ),
            "skill-shell-ps",
        )
        .await?;
    let skill_shell_env = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-env",
                "Leaf skill: how to list environment variables in the current session.",
                SKILL_SHELL_ENV_BODY,
            ),
            "skill-shell-env",
        )
        .await?;
    let skill_shell_uname = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-uname",
                "Leaf skill: how to show OS and kernel information.",
                SKILL_SHELL_UNAME_BODY,
            ),
            "skill-shell-uname",
        )
        .await?;
    let skill_shell_which = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-which",
                "Leaf skill: how to locate a binary on PATH.",
                SKILL_SHELL_WHICH_BODY,
            ),
            "skill-shell-which",
        )
        .await?;
    let skill_shell_date = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-date",
                "Leaf skill: how to print the current UTC date and time as ISO-8601.",
                SKILL_SHELL_DATE_BODY,
            ),
            "skill-shell-date",
        )
        .await?;
    let skill_shell_hostname = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-hostname",
                "Leaf skill: how to print the machine hostname.",
                SKILL_SHELL_HOSTNAME_BODY,
            ),
            "skill-shell-hostname",
        )
        .await?;
    let skill_shell_whoami = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-whoami",
                "Leaf skill: how to print the current user account name.",
                SKILL_SHELL_WHOAMI_BODY,
            ),
            "skill-shell-whoami",
        )
        .await?;
    let skill_shell_uptime = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-uptime",
                "Leaf skill: how to show system uptime and load average.",
                SKILL_SHELL_UPTIME_BODY,
            ),
            "skill-shell-uptime",
        )
        .await?;
    let skill_shell_free = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-free",
                "Leaf skill: how to show memory usage in human-readable form.",
                SKILL_SHELL_FREE_BODY,
            ),
            "skill-shell-free",
        )
        .await?;
    let skill_shell_wc_l = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-wc-l",
                "Leaf skill: how to count lines in a file.",
                SKILL_SHELL_WC_L_BODY,
            ),
            "skill-shell-wc-l",
        )
        .await?;
    let skill_shell_git_add = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-git-add",
                "Leaf skill: how to stage files for commit with git add.",
                SKILL_SHELL_GIT_ADD_BODY,
            ),
            "skill-shell-git-add",
        )
        .await?;
    let skill_shell_git_commit = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-git-commit",
                "Leaf skill: how to create a commit with a message.",
                SKILL_SHELL_GIT_COMMIT_BODY,
            ),
            "skill-shell-git-commit",
        )
        .await?;
    let skill_shell_git_push = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-git-push",
                "Leaf skill: how to push local commits to a remote.",
                SKILL_SHELL_GIT_PUSH_BODY,
            ),
            "skill-shell-git-push",
        )
        .await?;
    let skill_shell_git_pull = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-git-pull",
                "Leaf skill: how to pull remote changes into the current branch.",
                SKILL_SHELL_GIT_PULL_BODY,
            ),
            "skill-shell-git-pull",
        )
        .await?;
    let skill_shell_git_fetch = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-shell-git-fetch",
                "Leaf skill: how to fetch remote refs without merging (Tier 0 safe).",
                SKILL_SHELL_GIT_FETCH_BODY,
            ),
            "skill-shell-git-fetch",
        )
        .await?;

    // 6. Domain Skill (class 2) — skill-shell. Body transcribed verbatim from the
    //    doc (references every leaf skill above; two-tier usage guide).
    let skill_shell = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-shell",
                "Domain skill: when and how to use shell execution — two tiers.",
                SKILL_SHELL_BODY,
                2,
                LEAF_SKILL_TAGS,
            ),
            "skill-shell",
        )
        .await?;

    // Append the shell tool + toolskill + 30 PythonCode ids to ext-shell and the
    // primary catalogue (dedup-idempotent). Recipes are appended in chunk 6d.
    let ext_shell_children: Vec<Uuid> = vec![
        tool_shell, ts_shell_run, pc_exec_shell_git_status, pc_exec_shell_git_log,
        pc_exec_shell_git_diff_stat, pc_exec_shell_git_branch, pc_exec_shell_git_stash_list,
        pc_exec_shell_git_log_n, pc_exec_shell_git_remote, pc_exec_shell_git_show_stat,
        pc_exec_shell_git_tag_list, pc_exec_shell_git_diff_name_only, pc_exec_shell_git_log_stat,
        pc_exec_shell_git_stash_show, pc_exec_shell_git_config_list, pc_exec_shell_git_add,
        pc_exec_shell_git_commit, pc_exec_shell_git_push, pc_exec_shell_git_pull,
        pc_exec_shell_git_fetch, pc_exec_shell_pwd, pc_exec_shell_df, pc_exec_shell_ps,
        pc_exec_shell_env, pc_exec_shell_uname, pc_exec_shell_which, pc_exec_shell_date,
        pc_exec_shell_hostname, pc_exec_shell_whoami, pc_exec_shell_uptime, pc_exec_shell_free,
        pc_exec_shell_wc_l,
    ];
    stores.append_children(cat_shell, &ext_shell_children).await?;
    stores.append_children(cat_process, &ext_shell_children).await?;

    // Append the 31 leaf skills + 1 domain skill to ext-shell and the primary
    // (recipes appended in chunk 6d).
    let ext_shell_skill_children: Vec<Uuid> = vec![
        skill_shell_git_status, skill_shell_git_log, skill_shell_git_diff_stat,
        skill_shell_git_branch, skill_shell_git_stash_list, skill_shell_git_remote,
        skill_shell_git_show_stat, skill_shell_git_tag_list, skill_shell_git_diff_name_only,
        skill_shell_git_log_stat, skill_shell_git_stash_show, skill_shell_git_config_list,
        skill_shell_pwd, skill_shell_df, skill_shell_ps, skill_shell_env, skill_shell_uname,
        skill_shell_which, skill_shell_date, skill_shell_hostname, skill_shell_whoami,
        skill_shell_uptime, skill_shell_free, skill_shell_wc_l, skill_shell_run,
        skill_shell_safe_check, skill_shell_git_add, skill_shell_git_commit,
        skill_shell_git_push, skill_shell_git_pull, skill_shell_git_fetch, skill_shell,
    ];
    stores
        .append_children(cat_shell, &ext_shell_skill_children)
        .await?;
    stores
        .append_children(cat_process, &ext_shell_skill_children)
        .await?;

    // 7. Shell Recipes (class 21) — transcribed verbatim from the doc's flat
    //    format into the IBS authoring model (Q1 decision A). 25 Tier-0
    //    recipes are deterministic 2-step dispatches (rust ts-shell-run +
    //    orchestrator pc-exec-shell-*); 6 Tier-1 recipes (shell-run,
    //    shell-script, git-add/commit/push/pull) load leaf-skill context,
    //    pre-load ts-shell-run, and add an LLM-annotation `text` step.
    //    step_link is synthesized as "0:1-0:E"; stepnumber is the 1-based
    //    position (the doc's step_id labels are preserved verbatim in the
    //    yaml_source constants appended at the end of this file).
    //
    // Tier-0 git inspect recipes (§shell-safe-fixed).
    let recipe_shell_git_status = stores
        .seed_recipe(
            &tenant,
            "shell-git-status",
            "Run 'git status' and return the working tree state.",
            true,
            RECIPE_SHELL_GIT_STATUS_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-shell-run ToolSkill binding", "component", &[ts_shell_run]),
                step_entry(2, "orchestrator", "PythonCode calls host.shell(command='git status') — fixed literal", "component", &[pc_exec_shell_git_status]),
            ],
            &[
                json!({"input": "git status", "class": 1}),
                json!({"input": "what is the current git status", "class": 1}),
                json!({"input": "show me uncommitted changes", "class": 1}),
                json!({"input": "what files have changed", "class": 1}),
                json!({"input": "check git working tree", "class": 1}),
                json!({"input": "are there any staged changes", "class": 2}),
                json!({"input": "what is modified in the repo", "class": 2}),
                json!({"input": "show me the repo status", "class": 1}),
                json!({"input": "any untracked files", "class": 2}),
                json!({"input": "is my working directory clean", "class": 2}),
            ],
        )
        .await?;
    let recipe_shell_git_log = stores
        .seed_recipe(
            &tenant,
            "shell-git-log",
            "Show the last 20 commits as one-line summaries ('git log --oneline -20').",
            true,
            RECIPE_SHELL_GIT_LOG_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-shell-run ToolSkill binding", "component", &[ts_shell_run]),
                step_entry(2, "orchestrator", "PythonCode calls host.shell(command='git log --oneline -20')", "component", &[pc_exec_shell_git_log]),
            ],
            &[
                json!({"input": "show me recent commits", "class": 1}),
                json!({"input": "git log", "class": 1}),
                json!({"input": "what were the last commits", "class": 1}),
                json!({"input": "show commit history", "class": 1}),
                json!({"input": "list recent git commits", "class": 1}),
                json!({"input": "what was the last change merged", "class": 2}),
                json!({"input": "show me the git log", "class": 1}),
                json!({"input": "recent commit hashes and messages", "class": 2}),
                json!({"input": "what commits have been made", "class": 2}),
                json!({"input": "git history last 20", "class": 1}),
            ],
        )
        .await?;
    let recipe_shell_git_diff_stat = stores
        .seed_recipe(
            &tenant,
            "shell-git-diff-stat",
            "Show a summary of which files have changed and how many lines ('git diff --stat').",
            true,
            RECIPE_SHELL_GIT_DIFF_STAT_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-shell-run ToolSkill binding", "component", &[ts_shell_run]),
                step_entry(2, "orchestrator", "PythonCode calls host.shell(command='git diff --stat')", "component", &[pc_exec_shell_git_diff_stat]),
            ],
            &[
                json!({"input": "what files changed", "class": 1}),
                json!({"input": "git diff stat", "class": 1}),
                json!({"input": "show me which files are modified", "class": 1}),
                json!({"input": "how many lines changed", "class": 2}),
                json!({"input": "diff summary", "class": 1}),
                json!({"input": "what is the scope of my changes", "class": 2}),
                json!({"input": "show file change counts", "class": 2}),
                json!({"input": "git diff summary", "class": 1}),
                json!({"input": "which files are dirty", "class": 2}),
            ],
        )
        .await?;
    let recipe_shell_git_branch = stores
        .seed_recipe(
            &tenant,
            "shell-git-branch",
            "List all local and remote git branches ('git branch -a').",
            true,
            RECIPE_SHELL_GIT_BRANCH_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-shell-run ToolSkill binding", "component", &[ts_shell_run]),
                step_entry(2, "orchestrator", "PythonCode calls host.shell(command='git branch -a')", "component", &[pc_exec_shell_git_branch]),
            ],
            &[
                json!({"input": "list git branches", "class": 1}),
                json!({"input": "what branch am I on", "class": 1}),
                json!({"input": "show all branches", "class": 1}),
                json!({"input": "git branch", "class": 1}),
                json!({"input": "what remote branches exist", "class": 2}),
                json!({"input": "list all local and remote branches", "class": 1}),
                json!({"input": "which branches are available", "class": 2}),
                json!({"input": "show me the branch list", "class": 1}),
                json!({"input": "what is the current branch", "class": 2}),
                json!({"input": "git branch listing", "class": 1}),
            ],
        )
        .await?;
    let recipe_shell_git_stash_list = stores
        .seed_recipe(
            &tenant,
            "shell-git-stash-list",
            "List the git stash stack ('git stash list').",
            true,
            RECIPE_SHELL_GIT_STASH_LIST_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-shell-run ToolSkill binding", "component", &[ts_shell_run]),
                step_entry(2, "orchestrator", "PythonCode calls host.shell(command='git stash list')", "component", &[pc_exec_shell_git_stash_list]),
            ],
            &[
                json!({"input": "list git stashes", "class": 1}),
                json!({"input": "show stash contents", "class": 1}),
                json!({"input": "what is in the stash", "class": 1}),
                json!({"input": "git stash list", "class": 1}),
                json!({"input": "how many stashes do I have", "class": 2}),
                json!({"input": "do I have any stashed changes", "class": 2}),
                json!({"input": "show me the stash", "class": 1}),
                json!({"input": "stash entries", "class": 2}),
            ],
        )
        .await?;
    // Tier-0 system-information recipes (§shell-safe-fixed).
    let recipe_shell_pwd = stores
        .seed_recipe(
            &tenant,
            "shell-pwd",
            "Print the current working directory ('pwd').",
            true,
            RECIPE_SHELL_PWD_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-shell-run ToolSkill binding", "component", &[ts_shell_run]),
                step_entry(2, "orchestrator", "PythonCode calls host.shell(command='pwd')", "component", &[pc_exec_shell_pwd]),
            ],
            &[
                json!({"input": "what is the current directory", "class": 1}),
                json!({"input": "pwd", "class": 1}),
                json!({"input": "show me the working directory", "class": 1}),
                json!({"input": "what is my cwd", "class": 1}),
                json!({"input": "what directory am I in", "class": 1}),
                json!({"input": "print working directory", "class": 1}),
                json!({"input": "where am I in the filesystem", "class": 2}),
                json!({"input": "show current path", "class": 2}),
            ],
        )
        .await?;
    let recipe_shell_df = stores
        .seed_recipe(
            &tenant,
            "shell-df",
            "Show disk usage for all mounted filesystems in human-readable format ('df -h').",
            true,
            RECIPE_SHELL_DF_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-shell-run ToolSkill binding", "component", &[ts_shell_run]),
                step_entry(2, "orchestrator", "PythonCode calls host.shell(command='df -h')", "component", &[pc_exec_shell_df]),
            ],
            &[
                json!({"input": "check disk space", "class": 1}),
                json!({"input": "how much disk is free", "class": 1}),
                json!({"input": "df -h", "class": 1}),
                json!({"input": "disk usage", "class": 1}),
                json!({"input": "is the disk full", "class": 2}),
                json!({"input": "show filesystem space", "class": 1}),
                json!({"input": "how much storage is available", "class": 2}),
                json!({"input": "storage status", "class": 2}),
                json!({"input": "show mounted disk space", "class": 2}),
            ],
        )
        .await?;
    let recipe_shell_ps = stores
        .seed_recipe(
            &tenant,
            "shell-ps",
            "List all running processes with CPU and memory usage ('ps aux').",
            true,
            RECIPE_SHELL_PS_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-shell-run ToolSkill binding", "component", &[ts_shell_run]),
                step_entry(2, "orchestrator", "PythonCode calls host.shell(command='ps aux')", "component", &[pc_exec_shell_ps]),
            ],
            &[
                json!({"input": "list running processes", "class": 1}),
                json!({"input": "what processes are running", "class": 1}),
                json!({"input": "ps aux", "class": 1}),
                json!({"input": "show all processes", "class": 1}),
                json!({"input": "is this service running", "class": 2}),
                json!({"input": "check process list", "class": 1}),
                json!({"input": "what is consuming CPU", "class": 2}),
                json!({"input": "show me the process table", "class": 2}),
                json!({"input": "list system processes", "class": 1}),
            ],
        )
        .await?;
    let recipe_shell_env = stores
        .seed_recipe(
            &tenant,
            "shell-env",
            "List all current environment variables ('env').",
            true,
            RECIPE_SHELL_ENV_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-shell-run ToolSkill binding", "component", &[ts_shell_run]),
                step_entry(2, "orchestrator", "PythonCode calls host.shell(command='env')", "component", &[pc_exec_shell_env]),
            ],
            &[
                json!({"input": "show environment variables", "class": 1}),
                json!({"input": "list env vars", "class": 1}),
                json!({"input": "what environment variables are set", "class": 1}),
                json!({"input": "env", "class": 1}),
                json!({"input": "show me the PATH", "class": 2}),
                json!({"input": "what is the current environment", "class": 1}),
                json!({"input": "check environment configuration", "class": 2}),
                json!({"input": "dump environment", "class": 1}),
                json!({"input": "what env vars does the session have", "class": 2}),
            ],
        )
        .await?;
    let recipe_shell_uname = stores
        .seed_recipe(
            &tenant,
            "shell-uname",
            "Show OS and kernel information ('uname -a').",
            true,
            RECIPE_SHELL_UNAME_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-shell-run ToolSkill binding", "component", &[ts_shell_run]),
                step_entry(2, "orchestrator", "PythonCode calls host.shell(command='uname -a')", "component", &[pc_exec_shell_uname]),
            ],
            &[
                json!({"input": "what OS is this", "class": 1}),
                json!({"input": "uname -a", "class": 1}),
                json!({"input": "show kernel version", "class": 1}),
                json!({"input": "what is the system architecture", "class": 2}),
                json!({"input": "show system info", "class": 1}),
                json!({"input": "is this Linux or macOS", "class": 2}),
                json!({"input": "show OS details", "class": 1}),
                json!({"input": "kernel info", "class": 1}),
                json!({"input": "what platform am I running on", "class": 2}),
            ],
        )
        .await?;
    let recipe_shell_which = stores
        .seed_recipe(
            &tenant,
            "shell-which",
            "Locate a binary on PATH ('which <toolname>') — toolname must be a safe identifier.",
            true,
            RECIPE_SHELL_WHICH_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-shell-run ToolSkill binding", "component", &[ts_shell_run]),
                step_entry(2, "orchestrator", "PythonCode validates tool name then calls host.shell(command='which <tool>')", "component", &[pc_exec_shell_which]),
            ],
            &[
                json!({"input": "where is git installed", "class": 2}),
                json!({"input": "which python", "class": 1}),
                json!({"input": "is docker installed", "class": 2}),
                json!({"input": "find the path to node", "class": 2}),
                json!({"input": "which cargo", "class": 1}),
                json!({"input": "is this tool on PATH", "class": 2}),
                json!({"input": "locate the binary", "class": 2}),
                json!({"input": "which command", "class": 1}),
                json!({"input": "find the tool path", "class": 2}),
            ],
        )
        .await?;
    // Tier-0 additional fixed shell recipes (§shell-safe-fixed continued).
    let recipe_shell_date = stores
        .seed_recipe(
            &tenant,
            "shell-date",
            "Print the current UTC date/time in ISO-8601 format ('date -u +...').",
            true,
            RECIPE_SHELL_DATE_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-shell-run ToolSkill binding", "component", &[ts_shell_run]),
                step_entry(2, "orchestrator", "PythonCode calls host.shell(command='date -u +...')", "component", &[pc_exec_shell_date]),
            ],
            &[
                json!({"input": "what is today's date", "class": 1}),
                json!({"input": "print the current date", "class": 1}),
                json!({"input": "date command", "class": 1}),
                json!({"input": "current date in ISO format", "class": 1}),
                json!({"input": "system date", "class": 1}),
                json!({"input": "what date is it", "class": 1}),
                json!({"input": "show the current date and time", "class": 1}),
                json!({"input": "get the date from the shell", "class": 2}),
            ],
        )
        .await?;
    let recipe_shell_hostname = stores
        .seed_recipe(
            &tenant,
            "shell-hostname",
            "Print the machine hostname.",
            true,
            RECIPE_SHELL_HOSTNAME_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-shell-run ToolSkill binding", "component", &[ts_shell_run]),
                step_entry(2, "orchestrator", "PythonCode calls host.shell(command='hostname')", "component", &[pc_exec_shell_hostname]),
            ],
            &[
                json!({"input": "what is this machine called", "class": 1}),
                json!({"input": "hostname", "class": 1}),
                json!({"input": "what is the server name", "class": 1}),
                json!({"input": "show me the machine hostname", "class": 1}),
                json!({"input": "what host is this", "class": 1}),
                json!({"input": "machine name", "class": 1}),
                json!({"input": "get the hostname", "class": 1}),
                json!({"input": "show hostname of this server", "class": 2}),
            ],
        )
        .await?;
    let recipe_shell_whoami = stores
        .seed_recipe(
            &tenant,
            "shell-whoami",
            "Print the current OS user account name ('whoami').",
            true,
            RECIPE_SHELL_WHOAMI_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-shell-run ToolSkill binding", "component", &[ts_shell_run]),
                step_entry(2, "orchestrator", "PythonCode calls host.shell(command='whoami')", "component", &[pc_exec_shell_whoami]),
            ],
            &[
                json!({"input": "who am I running as", "class": 1}),
                json!({"input": "what user is this", "class": 1}),
                json!({"input": "whoami", "class": 1}),
                json!({"input": "current user", "class": 1}),
                json!({"input": "what is my username", "class": 1}),
                json!({"input": "which user account is active", "class": 1}),
                json!({"input": "show the current user", "class": 1}),
                json!({"input": "am I running as root", "class": 2}),
            ],
        )
        .await?;
    let recipe_shell_uptime = stores
        .seed_recipe(
            &tenant,
            "shell-uptime",
            "Show system uptime and current load average ('uptime').",
            true,
            RECIPE_SHELL_UPTIME_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-shell-run ToolSkill binding", "component", &[ts_shell_run]),
                step_entry(2, "orchestrator", "PythonCode calls host.shell(command='uptime')", "component", &[pc_exec_shell_uptime]),
            ],
            &[
                json!({"input": "how long has this server been running", "class": 1}),
                json!({"input": "uptime", "class": 1}),
                json!({"input": "system uptime", "class": 1}),
                json!({"input": "when was this server last rebooted", "class": 2}),
                json!({"input": "what is the load average", "class": 1}),
                json!({"input": "is the system under load", "class": 2}),
                json!({"input": "check server uptime", "class": 1}),
                json!({"input": "show me uptime and load", "class": 1}),
            ],
        )
        .await?;
    let recipe_shell_free = stores
        .seed_recipe(
            &tenant,
            "shell-free",
            "Show memory usage in human-readable format ('free -h').",
            true,
            RECIPE_SHELL_FREE_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-shell-run ToolSkill binding", "component", &[ts_shell_run]),
                step_entry(2, "orchestrator", "PythonCode calls host.shell(command='free -h')", "component", &[pc_exec_shell_free]),
            ],
            &[
                json!({"input": "check memory usage", "class": 1}),
                json!({"input": "how much RAM is available", "class": 1}),
                json!({"input": "free -h", "class": 1}),
                json!({"input": "memory usage", "class": 1}),
                json!({"input": "how much memory does this process use", "class": 2}),
                json!({"input": "is RAM running low", "class": 2}),
                json!({"input": "show memory stats", "class": 1}),
                json!({"input": "available and used RAM", "class": 1}),
            ],
        )
        .await?;
    let recipe_shell_git_remote = stores
        .seed_recipe(
            &tenant,
            "shell-git-remote",
            "List all configured git remotes and their URLs ('git remote -v').",
            true,
            RECIPE_SHELL_GIT_REMOTE_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-shell-run ToolSkill binding", "component", &[ts_shell_run]),
                step_entry(2, "orchestrator", "PythonCode calls host.shell(command='git remote -v')", "component", &[pc_exec_shell_git_remote]),
            ],
            &[
                json!({"input": "list git remotes", "class": 1}),
                json!({"input": "what is the git remote URL", "class": 1}),
                json!({"input": "git remote -v", "class": 1}),
                json!({"input": "show remote repositories", "class": 1}),
                json!({"input": "what origin URL is configured", "class": 2}),
                json!({"input": "list all configured remotes", "class": 1}),
                json!({"input": "what remotes does this repo have", "class": 2}),
                json!({"input": "show git remote configuration", "class": 1}),
            ],
        )
        .await?;
    let recipe_shell_git_show_stat = stores
        .seed_recipe(
            &tenant,
            "shell-git-show-stat",
            "Show changed files and line counts for the last commit ('git show --stat HEAD').",
            true,
            RECIPE_SHELL_GIT_SHOW_STAT_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-shell-run ToolSkill binding", "component", &[ts_shell_run]),
                step_entry(2, "orchestrator", "PythonCode calls host.shell(command='git show --stat HEAD')", "component", &[pc_exec_shell_git_show_stat]),
            ],
            &[
                json!({"input": "what did the last commit change", "class": 1}),
                json!({"input": "git show --stat HEAD", "class": 1}),
                json!({"input": "show files changed in last commit", "class": 1}),
                json!({"input": "what was in the previous commit", "class": 2}),
                json!({"input": "show stat for most recent commit", "class": 1}),
                json!({"input": "what was the last thing committed", "class": 2}),
                json!({"input": "show HEAD commit diff summary", "class": 1}),
                json!({"input": "git show stat last commit", "class": 1}),
            ],
        )
        .await?;
    let recipe_shell_git_tag_list = stores
        .seed_recipe(
            &tenant,
            "shell-git-tag-list",
            "List all tags in the repository ('git tag --list').",
            true,
            RECIPE_SHELL_GIT_TAG_LIST_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-shell-run ToolSkill binding", "component", &[ts_shell_run]),
                step_entry(2, "orchestrator", "PythonCode calls host.shell(command='git tag --list')", "component", &[pc_exec_shell_git_tag_list]),
            ],
            &[
                json!({"input": "list git tags", "class": 1}),
                json!({"input": "what tags exist in this repo", "class": 1}),
                json!({"input": "show all release tags", "class": 1}),
                json!({"input": "git tag --list", "class": 1}),
                json!({"input": "what versions are tagged", "class": 2}),
                json!({"input": "list all git version tags", "class": 1}),
                json!({"input": "show me the tags in this repository", "class": 1}),
                json!({"input": "what is the latest git tag", "class": 2}),
            ],
        )
        .await?;
    let recipe_shell_wc_l = stores
        .seed_recipe(
            &tenant,
            "shell-wc-l",
            "Count the number of lines in a file ('wc -l <filepath>').",
            true,
            RECIPE_SHELL_WC_L_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-shell-run ToolSkill binding", "component", &[ts_shell_run]),
                step_entry(2, "orchestrator", "PythonCode validates path then calls host.shell(command='wc -l <file>')", "component", &[pc_exec_shell_wc_l]),
            ],
            &[
                json!({"input": "how many lines in this file", "class": 1}),
                json!({"input": "line count of this file", "class": 1}),
                json!({"input": "wc -l on this file", "class": 1}),
                json!({"input": "count lines in the log file", "class": 2}),
                json!({"input": "how long is this file", "class": 2}),
                json!({"input": "how many rows does this CSV have", "class": 2}),
                json!({"input": "get line count without reading file", "class": 2}),
                json!({"input": "count the lines in this source file", "class": 2}),
            ],
        )
        .await?;
    // Tier-0 extended git inspect recipes (§shell-safe-fixed continued).
    let recipe_shell_git_diff_name_only = stores
        .seed_recipe(
            &tenant,
            "shell-git-diff-name-only",
            "List only the names of files changed since the last commit (git diff --name-only HEAD).",
            true,
            RECIPE_SHELL_GIT_DIFF_NAME_ONLY_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-shell-run ToolSkill binding", "component", &[ts_shell_run]),
                step_entry(2, "orchestrator", "PythonCode calls host.shell(command='git diff --name-only HEAD')", "component", &[pc_exec_shell_git_diff_name_only]),
            ],
            &[
                json!({"input": "what files have changed since last commit", "class": 1}),
                json!({"input": "which files are modified", "class": 1}),
                json!({"input": "list changed files", "class": 1}),
                json!({"input": "show modified filenames only", "class": 1}),
                json!({"input": "git diff name only", "class": 1}),
                json!({"input": "what did I change in this working directory", "class": 2}),
                json!({"input": "files with uncommitted changes", "class": 1}),
                json!({"input": "what is dirty in the working tree", "class": 2}),
                json!({"input": "list unstaged or staged changed files", "class": 2}),
                json!({"input": "show only the names of changed files no content", "class": 1}),
            ],
        )
        .await?;
    let recipe_shell_git_log_stat = stores
        .seed_recipe(
            &tenant,
            "shell-git-log-stat",
            "Show the last 5 commits with file-change statistics (git log --stat --oneline -5).",
            true,
            RECIPE_SHELL_GIT_LOG_STAT_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-shell-run ToolSkill binding", "component", &[ts_shell_run]),
                step_entry(2, "orchestrator", "PythonCode calls host.shell(command='git log --stat --oneline -5')", "component", &[pc_exec_shell_git_log_stat]),
            ],
            &[
                json!({"input": "show recent commits with file changes", "class": 1}),
                json!({"input": "git log with stats", "class": 1}),
                json!({"input": "what files changed in the last few commits", "class": 2}),
                json!({"input": "show commit history with change counts", "class": 1}),
                json!({"input": "git log stat", "class": 1}),
                json!({"input": "which files were touched in recent commits", "class": 2}),
                json!({"input": "last 5 commits with file statistics", "class": 1}),
                json!({"input": "show me what changed in recent git history", "class": 2}),
                json!({"input": "recent commit file change summary", "class": 2}),
                json!({"input": "git history with additions deletions per file", "class": 2}),
            ],
        )
        .await?;
    let recipe_shell_git_stash_show = stores
        .seed_recipe(
            &tenant,
            "shell-git-stash-show",
            "Show the diff summary of the most recent git stash entry (git stash show).",
            true,
            RECIPE_SHELL_GIT_STASH_SHOW_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-shell-run ToolSkill binding", "component", &[ts_shell_run]),
                step_entry(2, "orchestrator", "PythonCode calls host.shell(command='git stash show')", "component", &[pc_exec_shell_git_stash_show]),
            ],
            &[
                json!({"input": "what is in my git stash", "class": 1}),
                json!({"input": "show the latest stash entry", "class": 1}),
                json!({"input": "git stash show", "class": 1}),
                json!({"input": "what changes did I stash", "class": 2}),
                json!({"input": "preview the stash without applying it", "class": 2}),
                json!({"input": "what files are in the current stash", "class": 2}),
                json!({"input": "inspect the stash summary", "class": 1}),
                json!({"input": "show stash diff summary", "class": 1}),
            ],
        )
        .await?;
    let recipe_shell_git_config_list = stores
        .seed_recipe(
            &tenant,
            "shell-git-config-list",
            "List all active git configuration values (git config --list).",
            true,
            RECIPE_SHELL_GIT_CONFIG_LIST_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-shell-run ToolSkill binding", "component", &[ts_shell_run]),
                step_entry(2, "orchestrator", "PythonCode calls host.shell(command='git config --list')", "component", &[pc_exec_shell_git_config_list]),
            ],
            &[
                json!({"input": "show git configuration", "class": 1}),
                json!({"input": "what is my git user.name", "class": 2}),
                json!({"input": "git config list", "class": 1}),
                json!({"input": "show all git settings", "class": 1}),
                json!({"input": "what email is configured for git commits", "class": 2}),
                json!({"input": "check git config", "class": 1}),
                json!({"input": "show current git identity settings", "class": 2}),
                json!({"input": "list active git configuration", "class": 1}),
            ],
        )
        .await?;
    let recipe_shell_git_fetch = stores
        .seed_recipe(
            &tenant,
            "shell-git-fetch",
            "Fetch all remote branches without merging (git fetch --all). Tier 0 — read-only.",
            true,
            RECIPE_SHELL_GIT_FETCH_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-shell-run ToolSkill binding", "component", &[ts_shell_run]),
                step_entry(2, "orchestrator", "PythonCode calls host.shell(command='git fetch --all')", "component", &[pc_exec_shell_git_fetch]),
            ],
            &[
                json!({"input": "git fetch", "class": 1}),
                json!({"input": "fetch all remote branches", "class": 1}),
                json!({"input": "git fetch all", "class": 1}),
                json!({"input": "update my remote tracking branches", "class": 2}),
                json!({"input": "fetch from origin", "class": 2}),
                json!({"input": "get latest remote refs", "class": 2}),
                json!({"input": "fetch without merging", "class": 1}),
                json!({"input": "update remote branch info", "class": 2}),
                json!({"input": "git fetch --all", "class": 1}),
                json!({"input": "pull down remote branch list", "class": 2}),
            ],
        )
        .await?;
    // Tier-1 shell recipes (§shell-guard-custom / §shell-guard). LLM step is
    // an annotation-only `text` step; llm_call_required stays at the seeded
    // default (true) because tier0=false leaves tier=wilson defaults.
    let recipe_shell_run = stores
        .seed_recipe(
            &tenant,
            "shell-run",
            "Run a single shell command and return its output.",
            false,
            RECIPE_SHELL_RUN_YAML,
            &[
                step_entry(1, "orchestrator", "Load shell domain + run + safety-check leaf skills", "component", &[skill_shell, skill_shell_run, skill_shell_safe_check]),
                step_entry(2, "orchestrator", "LLM validates safety, composes the exact command, gets user approval", "text", &[]),
                step_entry(3, "rust", "Executor pre-loads ts-shell-run binding", "component", &[ts_shell_run]),
            ],
            &[
                json!({"input": "run a command", "class": 2}),
                json!({"input": "execute a shell command", "class": 2}),
                json!({"input": "run ls in the project dir", "class": 3}),
                json!({"input": "check git status", "class": 3}),
                json!({"input": "shell", "class": 1}),
                json!({"input": "run this command in the project root", "class": 3}),
                json!({"input": "execute git pull", "class": 3}),
                json!({"input": "run a quick system command", "class": 2}),
                json!({"input": "shell execute this", "class": 1}),
                json!({"input": "run this CLI command", "class": 2}),
            ],
        )
        .await?;
    let recipe_shell_script = stores
        .seed_recipe(
            &tenant,
            "shell-script",
            "Execute a multi-line shell script authored by the LLM.",
            false,
            RECIPE_SHELL_SCRIPT_YAML,
            &[
                step_entry(1, "orchestrator", "Load shell domain + safety-check context", "component", &[skill_shell, skill_shell_safe_check]),
                step_entry(2, "orchestrator", "LLM writes the full script body, validates safety, gets user approval", "text", &[]),
                step_entry(3, "rust", "Executor pre-loads ts-shell-run binding", "component", &[ts_shell_run]),
            ],
            &[
                json!({"input": "run a bash script", "class": 2}),
                json!({"input": "execute a script", "class": 2}),
                json!({"input": "write and run a shell script that backs up my files", "class": 3}),
                json!({"input": "bash script", "class": 1}),
                json!({"input": "create and run a multi-step shell script", "class": 2}),
                json!({"input": "write a script to process these log files", "class": 3}),
                json!({"input": "run a shell script with these steps", "class": 2}),
                json!({"input": "execute a batch script", "class": 2}),
            ],
        )
        .await?;
    let recipe_shell_git_add = stores
        .seed_recipe(
            &tenant,
            "shell-git-add",
            "Stage files for commit with git add (LLM confirms paths with user).",
            false,
            RECIPE_SHELL_GIT_ADD_YAML,
            &[
                step_entry(1, "orchestrator", "Load git-add + git-status leaf skills", "component", &[skill_shell_git_add, skill_shell_git_status]),
                step_entry(2, "rust", "Pre-load ts-shell-run binding", "component", &[ts_shell_run]),
                step_entry(3, "orchestrator", "LLM checks git status, confirms which files to stage, dispatches git add", "text", &[]),
            ],
            &[
                json!({"input": "git add", "class": 1}),
                json!({"input": "stage my changes", "class": 1}),
                json!({"input": "add all files to git", "class": 1}),
                json!({"input": "stage this file for commit", "class": 2}),
                json!({"input": "git add .", "class": 1}),
                json!({"input": "add these changes to git staging", "class": 2}),
                json!({"input": "stage my modifications", "class": 2}),
                json!({"input": "mark these files for the next commit", "class": 2}),
                json!({"input": "git add specific file", "class": 2}),
                json!({"input": "track and stage these changes", "class": 2}),
            ],
        )
        .await?;
    let recipe_shell_git_commit = stores
        .seed_recipe(
            &tenant,
            "shell-git-commit",
            "Commit staged changes with a user-confirmed message.",
            false,
            RECIPE_SHELL_GIT_COMMIT_YAML,
            &[
                step_entry(1, "orchestrator", "Load git-commit + git-status leaf skills", "component", &[skill_shell_git_commit, skill_shell_git_status]),
                step_entry(2, "rust", "Pre-load ts-shell-run binding", "component", &[ts_shell_run]),
                step_entry(3, "orchestrator", "LLM checks git status, composes commit message, confirms with user, dispatches commit", "text", &[]),
            ],
            &[
                json!({"input": "git commit", "class": 1}),
                json!({"input": "commit my changes", "class": 1}),
                json!({"input": "commit staged files", "class": 1}),
                json!({"input": "commit with message", "class": 2}),
                json!({"input": "save my changes with a commit", "class": 2}),
                json!({"input": "create a git commit", "class": 2}),
                json!({"input": "commit everything I staged", "class": 2}),
                json!({"input": "make a commit with this message", "class": 2}),
                json!({"input": "git commit -m", "class": 1}),
                json!({"input": "finalize my changes with a git commit", "class": 3}),
            ],
        )
        .await?;
    let recipe_shell_git_push = stores
        .seed_recipe(
            &tenant,
            "shell-git-push",
            "Push local commits to a remote repository branch.",
            false,
            RECIPE_SHELL_GIT_PUSH_YAML,
            &[
                step_entry(1, "orchestrator", "Load git-push + git-log leaf skills", "component", &[skill_shell_git_push, skill_shell_git_log]),
                step_entry(2, "rust", "Pre-load ts-shell-run binding", "component", &[ts_shell_run]),
                step_entry(3, "orchestrator", "LLM shows recent commits, confirms remote/branch, dispatches push", "text", &[]),
            ],
            &[
                json!({"input": "git push", "class": 1}),
                json!({"input": "push my commits", "class": 1}),
                json!({"input": "push to origin", "class": 1}),
                json!({"input": "push to main", "class": 2}),
                json!({"input": "upload my changes to github", "class": 2}),
                json!({"input": "push local branch to remote", "class": 2}),
                json!({"input": "git push origin main", "class": 1}),
                json!({"input": "send my commits to the remote", "class": 2}),
                json!({"input": "push my work to the repository", "class": 2}),
                json!({"input": "deploy my commits to origin", "class": 2}),
            ],
        )
        .await?;
    let recipe_shell_git_pull = stores
        .seed_recipe(
            &tenant,
            "shell-git-pull",
            "Pull remote changes and merge into the current branch.",
            false,
            RECIPE_SHELL_GIT_PULL_YAML,
            &[
                step_entry(1, "orchestrator", "Load git-pull + git-status leaf skills", "component", &[skill_shell_git_pull, skill_shell_git_status]),
                step_entry(2, "rust", "Pre-load ts-shell-run binding", "component", &[ts_shell_run]),
                step_entry(3, "orchestrator", "LLM checks for local changes, confirms remote/branch, handles conflicts on failure", "text", &[]),
            ],
            &[
                json!({"input": "git pull", "class": 1}),
                json!({"input": "pull latest changes", "class": 1}),
                json!({"input": "update from remote", "class": 1}),
                json!({"input": "pull from origin", "class": 2}),
                json!({"input": "get latest code from github", "class": 2}),
                json!({"input": "sync with remote branch", "class": 2}),
                json!({"input": "git pull origin main", "class": 1}),
                json!({"input": "pull remote commits", "class": 2}),
                json!({"input": "update my local branch from remote", "class": 2}),
                json!({"input": "fetch and merge remote changes", "class": 2}),
            ],
        )
        .await?;

    // Append the 31 shell recipe ids to ext-shell and the primary catalogue
    // (dedup-idempotent). Completes the ext-shell child set: tool + ts + 30 pc
    // + 31 leaf skills + 1 domain + 31 recipes.
    let ext_shell_recipe_children: Vec<Uuid> = vec![
        recipe_shell_git_status, recipe_shell_git_log, recipe_shell_git_diff_stat,
        recipe_shell_git_branch, recipe_shell_git_stash_list, recipe_shell_pwd,
        recipe_shell_df, recipe_shell_ps, recipe_shell_env, recipe_shell_uname,
        recipe_shell_which, recipe_shell_date, recipe_shell_hostname,
        recipe_shell_whoami, recipe_shell_uptime, recipe_shell_free,
        recipe_shell_git_remote, recipe_shell_git_show_stat, recipe_shell_git_tag_list,
        recipe_shell_wc_l, recipe_shell_git_diff_name_only, recipe_shell_git_log_stat,
        recipe_shell_git_stash_show, recipe_shell_git_config_list, recipe_shell_git_fetch,
        recipe_shell_run, recipe_shell_script, recipe_shell_git_add,
        recipe_shell_git_commit, recipe_shell_git_push, recipe_shell_git_pull,
    ];
    stores
        .append_children(cat_shell, &ext_shell_recipe_children)
        .await?;
    stores
        .append_children(cat_process, &ext_shell_recipe_children)
        .await?;

    // 8. Spawn-subagent Leaf Skills (class 1) + Domain Skill (class 2) — chunk 6e.
    //    All spawn skills carry consumer_tags ["02:orchestrator"] (orchestrator-only;
    //    the validator never delegates child runs), transcribed verbatim from the doc.
    //    Bodies are transcribed verbatim from Step 18.3/18.4/18.x.1-18.x.4/18.5.
    let skill_spawn_subagent = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-spawn-subagent",
                "Leaf skill: how to spawn a child agent run for a delegated sub-task.",
                SKILL_SPAWN_SUBAGENT_BODY,
                1,
                SPAWN_SKILL_TAGS,
            ),
            "skill-spawn-subagent",
        )
        .await?;
    let skill_spawn_named_procedure = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-spawn-named-procedure",
                "Leaf skill: how to run a named recipe as a child agent procedure.",
                SKILL_SPAWN_NAMED_PROCEDURE_BODY,
                1,
                SPAWN_SKILL_TAGS,
            ),
            "skill-spawn-named-procedure",
        )
        .await?;
    let skill_spawn_research = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-spawn-research",
                "Leaf skill: how to delegate a research or information-gathering sub-task.",
                SKILL_SPAWN_RESEARCH_BODY,
                1,
                SPAWN_SKILL_TAGS,
            ),
            "skill-spawn-research",
        )
        .await?;
    let skill_spawn_coding = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-spawn-coding",
                "Leaf skill: how to delegate a focused coding sub-task to a child agent.",
                SKILL_SPAWN_CODING_BODY,
                1,
                SPAWN_SKILL_TAGS,
            ),
            "skill-spawn-coding",
        )
        .await?;
    let skill_spawn_exploration = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-spawn-exploration",
                "Leaf skill: how to delegate a deep read-only workspace exploration task.",
                SKILL_SPAWN_EXPLORATION_BODY,
                1,
                SPAWN_SKILL_TAGS,
            ),
            "skill-spawn-exploration",
        )
        .await?;
    let skill_spawn_query = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-spawn-query",
                "Leaf skill: how to delegate a focused lookup query to a child agent.",
                SKILL_SPAWN_QUERY_BODY,
                1,
                SPAWN_SKILL_TAGS,
            ),
            "skill-spawn-query",
        )
        .await?;
    let skill_subagent = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-subagent",
                "Domain skill: child agent delegation via spawn_subagent.",
                SKILL_SUBAGENT_BODY,
                2,
                SPAWN_SKILL_TAGS,
            ),
            "skill-subagent",
        )
        .await?;

    // 9. Spawn-subagent Recipes (class 21) — chunk 6e. ALL Tier 1
    //    (§spawn_subagent-guard: llm_call_required=true; no Tier-0 spawn dispatch).
    //    Each is a 3-step delegation: (1) orchestrator loads the flavour leaf skill,
    //    (2) LLM frames the goal (text annotation), (3) rust pre-loads ts-spawn-subagent.
    //    step_link synthesized "0:1-0:E"; stepnumber = 1-based position; the doc's flat
    //    step format is preserved verbatim in the RECIPE_SUBAGENT_*_YAML constants.
    let recipe_subagent_spawn = stores
        .seed_recipe(
            &tenant,
            "subagent-spawn",
            "Spawn a child agent for a delegated sub-task or named procedure.",
            false,
            RECIPE_SUBAGENT_SPAWN_YAML,
            &[
                step_entry(1, "orchestrator", "Load spawn leaf skills (goal delegation + named procedure patterns)", "component", &[skill_spawn_subagent, skill_spawn_named_procedure]),
                step_entry(2, "orchestrator", "LLM frames the goal, decides generic-vs-recipe delegation, confirms with user, calls ts-spawn-subagent", "text", &[]),
                step_entry(3, "rust", "Pre-load ts-spawn-subagent ToolSkill binding", "component", &[ts_spawn_subagent]),
            ],
            &[
                json!({"input": "spawn a child agent to do X", "class": 1}),
                json!({"input": "delegate this task to a subagent", "class": 1}),
                json!({"input": "run procedure Y in a child session", "class": 1}),
                json!({"input": "create a child agent for this sub-task", "class": 1}),
                json!({"input": "use a subagent for this long-running task", "class": 2}),
                json!({"input": "subagent spawn", "class": 1}),
                json!({"input": "hand off this work to a child agent", "class": 2}),
                json!({"input": "run this recipe in a child session", "class": 2}),
                json!({"input": "spawn subagent with this goal", "class": 1}),
                json!({"input": "delegate this to a parallel agent", "class": 2}),
            ],
        )
        .await?;
    let recipe_subagent_research = stores
        .seed_recipe(
            &tenant,
            "subagent-research",
            "Delegate a focused research or information-gathering task to a child agent.",
            false,
            RECIPE_SUBAGENT_RESEARCH_YAML,
            &[
                step_entry(1, "orchestrator", "Load research delegation leaf skill body", "component", &[skill_spawn_research]),
                step_entry(2, "orchestrator", "LLM frames focused research goal string, sets context, calls ts-spawn-subagent", "text", &[]),
                step_entry(3, "rust", "Pre-load ts-spawn-subagent ToolSkill binding", "component", &[ts_spawn_subagent]),
            ],
            &[
                json!({"input": "research this topic using a child agent", "class": 1}),
                json!({"input": "have a subagent look this up", "class": 1}),
                json!({"input": "delegate this research to a child", "class": 1}),
                json!({"input": "spawn a researcher subagent", "class": 1}),
                json!({"input": "research X in a child session", "class": 2}),
                json!({"input": "use a subagent to find information about X", "class": 2}),
                json!({"input": "gather information on X via child agent", "class": 2}),
                json!({"input": "let a subagent research this and report back", "class": 2}),
            ],
        )
        .await?;
    let recipe_subagent_coding = stores
        .seed_recipe(
            &tenant,
            "subagent-coding",
            "Delegate a focused code-reading, code-writing, or debugging task to a child agent.",
            false,
            RECIPE_SUBAGENT_CODING_YAML,
            &[
                step_entry(1, "orchestrator", "Load coding delegation leaf skill body", "component", &[skill_spawn_coding]),
                step_entry(2, "orchestrator", "LLM scopes the code task, includes file paths + constraints in context, calls ts-spawn-subagent", "text", &[]),
                step_entry(3, "rust", "Pre-load ts-spawn-subagent ToolSkill binding", "component", &[ts_spawn_subagent]),
            ],
            &[
                json!({"input": "have a child agent fix this bug", "class": 1}),
                json!({"input": "delegate this coding task to a subagent", "class": 1}),
                json!({"input": "spawn a coder subagent to handle this", "class": 1}),
                json!({"input": "let a child agent write this code", "class": 1}),
                json!({"input": "use a subagent to refactor this file", "class": 2}),
                json!({"input": "have a child agent apply this patch", "class": 2}),
                json!({"input": "delegate the code changes to a child session", "class": 2}),
                json!({"input": "subagent coding task", "class": 1}),
            ],
        )
        .await?;
    let recipe_subagent_exploration = stores
        .seed_recipe(
            &tenant,
            "subagent-exploration",
            "Delegate a deep read-only workspace or codebase exploration to a child agent.",
            false,
            RECIPE_SUBAGENT_EXPLORATION_YAML,
            &[
                step_entry(1, "orchestrator", "Load exploration delegation leaf skill body", "component", &[skill_spawn_exploration]),
                step_entry(2, "orchestrator", "LLM defines exploration scope and output format, calls ts-spawn-subagent", "text", &[]),
                step_entry(3, "rust", "Pre-load ts-spawn-subagent ToolSkill binding", "component", &[ts_spawn_subagent]),
            ],
            &[
                json!({"input": "have a subagent explore this codebase area", "class": 1}),
                json!({"input": "spawn an explorer to map out the structure", "class": 1}),
                json!({"input": "delegate a deep exploration to a child agent", "class": 1}),
                json!({"input": "have a child agent catalogue this directory", "class": 2}),
                json!({"input": "explore the codebase with a subagent", "class": 2}),
                json!({"input": "subagent explore", "class": 1}),
                json!({"input": "use a child agent to analyse code patterns", "class": 2}),
                json!({"input": "have a child agent trace this dependency", "class": 2}),
            ],
        )
        .await?;
    let recipe_subagent_query = stores
        .seed_recipe(
            &tenant,
            "subagent-query",
            "Delegate a focused single-question lookup to a child agent.",
            false,
            RECIPE_SUBAGENT_QUERY_YAML,
            &[
                step_entry(1, "orchestrator", "Load query delegation leaf skill body", "component", &[skill_spawn_query]),
                step_entry(2, "orchestrator", "LLM formulates focused single-question goal, expected output shape, calls ts-spawn-subagent", "text", &[]),
                step_entry(3, "rust", "Pre-load ts-spawn-subagent ToolSkill binding", "component", &[ts_spawn_subagent]),
            ],
            &[
                json!({"input": "ask a child agent to look this up", "class": 1}),
                json!({"input": "have a subagent answer this question", "class": 1}),
                json!({"input": "delegate this lookup to a child session", "class": 1}),
                json!({"input": "spawn a query subagent", "class": 1}),
                json!({"input": "use a child agent to find the answer to X", "class": 2}),
                json!({"input": "subagent query", "class": 1}),
                json!({"input": "let a child agent fetch this information", "class": 2}),
                json!({"input": "have a child agent check this value", "class": 2}),
            ],
        )
        .await?;

    // Append the spawn tool + toolskill to ext-spawn-subagent and the primary
    // (dedup-idempotent; seeded in chunk 6a).
    let ext_spawn_children: Vec<Uuid> = vec![tool_spawn_subagent, ts_spawn_subagent];
    stores.append_children(cat_spawn, &ext_spawn_children).await?;
    stores.append_children(cat_process, &ext_spawn_children).await?;

    // Append the 6 spawn leaf skills + 1 domain + 5 recipes to ext-spawn-subagent
    // and the primary (dedup-idempotent). Completes the ext-spawn-subagent child
    // set: tool + ts + 6 leaf skills + 1 domain + 5 recipes.
    let ext_spawn_skill_recipe_children: Vec<Uuid> = vec![
        skill_spawn_subagent, skill_spawn_named_procedure, skill_spawn_research,
        skill_spawn_coding, skill_spawn_exploration, skill_spawn_query, skill_subagent,
        recipe_subagent_spawn, recipe_subagent_research, recipe_subagent_coding,
        recipe_subagent_exploration, recipe_subagent_query,
    ];
    stores
        .append_children(cat_spawn, &ext_spawn_skill_recipe_children)
        .await?;
    stores
        .append_children(cat_process, &ext_spawn_skill_recipe_children)
        .await?;

    // 10. Trigger PythonCode (class 22) — chunk 6f. Orchestrator executors that
    //     call host.trigger_list / host.trigger_create / host.trigger_remove. All
    //     use pc_row (SEC-01-safe consumer_tags ["01:monty","02:orchestrator"]).
    //     Bodies transcribed verbatim from Step 17.7 / 17.x.1 / 17.x.3.1-2.
    let pc_exec_trigger_list = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-trigger-list",
                "Orchestrator executor: calls host.trigger_list to list configured \
                 triggers. Input: scope (string). Output: [{name, schedule, recipe_name, …}].",
                PC_EXEC_TRIGGER_LIST_CONTENT,
            ),
            "pc-exec-trigger-list",
        )
        .await?;
    let pc_exec_trigger_list_active = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-trigger-list-active",
                "Orchestrator executor: calls host.trigger_list(scope='active') to list \
                 only currently active (running/enabled) triggers. No LLM needed.",
                PC_EXEC_TRIGGER_LIST_ACTIVE_CONTENT,
            ),
            "pc-exec-trigger-list-active",
        )
        .await?;
    let pc_exec_trigger_list_scheduled = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-trigger-list-scheduled",
                "Orchestrator executor: calls host.trigger_list(scope='scheduled') to list \
                 only scheduled (cron/time-based) triggers. No LLM needed.",
                PC_EXEC_TRIGGER_LIST_SCHEDULED_CONTENT,
            ),
            "pc-exec-trigger-list-scheduled",
        )
        .await?;
    let pc_exec_trigger_resolve_and_remove = stores
        .upsert_python_code(
            pc_row(
                &tenant,
                "pc-exec-trigger-resolve-and-remove",
                "Orchestrator executor: lists all triggers, finds the one matching the \
                 given name exactly, and removes it. Input: trigger_name (string). Output: \
                 {removed: bool, trigger_name: string, error?: string}.",
                PC_EXEC_TRIGGER_RESOLVE_AND_REMOVE_CONTENT,
            ),
            "pc-exec-trigger-resolve-and-remove",
        )
        .await?;

    // 11. Trigger Leaf Skills (class 1) + Domain Skill (class 2) — chunk 6f.
    //     Main skills (list/create/remove + domain) carry TRIGGER_SKILL_TAGS
    //     (["02:orchestrator"]); the variant list skills (active/scheduled) carry
    //     the full LEAF_SKILL_TAGS per doc verbatim. Bodies verbatim from
    //     Step 17.8-17.11 / 17.x.3.3-4.
    let skill_trigger_list = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-trigger-list",
                "Leaf skill: how to list all configured triggers in the active scope.",
                SKILL_TRIGGER_LIST_BODY,
                1,
                TRIGGER_SKILL_TAGS,
            ),
            "skill-trigger-list",
        )
        .await?;
    let skill_trigger_create = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-trigger-create",
                "Leaf skill: how to create a scheduled trigger for a recipe.",
                SKILL_TRIGGER_CREATE_BODY,
                1,
                TRIGGER_SKILL_TAGS,
            ),
            "skill-trigger-create",
        )
        .await?;
    let skill_trigger_remove = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-trigger-remove",
                "Leaf skill: how to remove a configured trigger.",
                SKILL_TRIGGER_REMOVE_BODY,
                1,
                TRIGGER_SKILL_TAGS,
            ),
            "skill-trigger-remove",
        )
        .await?;
    let skill_trigger_list_active = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-trigger-list-active",
                "Leaf skill: how to list only currently active triggers.",
                SKILL_TRIGGER_LIST_ACTIVE_BODY,
            ),
            "skill-trigger-list-active",
        )
        .await?;
    let skill_trigger_list_scheduled = stores
        .upsert_skill(
            leaf_skill(
                &tenant,
                "skill-trigger-list-scheduled",
                "Leaf skill: how to list only scheduled (cron/time-based) triggers.",
                SKILL_TRIGGER_LIST_SCHEDULED_BODY,
            ),
            "skill-trigger-list-scheduled",
        )
        .await?;
    let skill_triggers = stores
        .upsert_skill(
            skill_row(
                &tenant,
                "skill-triggers",
                "Domain skill: trigger management — list, create, remove scheduled runs.",
                SKILL_TRIGGERS_BODY,
                2,
                TRIGGER_SKILL_TAGS,
            ),
            "skill-triggers",
        )
        .await?;

    // 12. Trigger Recipes (class 21) — chunk 6f. 3 Tier-0 list recipes
    //     (trigger-list/active/scheduled — deterministic, scope pre-baked) +
    //     3 Tier-1 recipes (trigger-create/remove/remove-by-name — ExternalWrite,
    //     LLM confirmation). step_link synthesized "0:1-0:E"; stepnumber = 1-based
    //     position; the doc's flat step format is preserved verbatim in the
    //     RECIPE_TRIGGER_*_YAML constants.
    let recipe_trigger_list = stores
        .seed_recipe(
            &tenant,
            "trigger-list",
            "List all configured triggers in the active scope.",
            true,
            RECIPE_TRIGGER_LIST_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-trigger-list ToolSkill binding", "component", &[ts_trigger_list]),
                step_entry(2, "orchestrator", "PythonCode calls host.trigger_list(scope)", "component", &[pc_exec_trigger_list]),
            ],
            &[
                json!({"input": "list my triggers", "class": 1}),
                json!({"input": "what triggers are configured", "class": 1}),
                json!({"input": "show scheduled tasks", "class": 1}),
                json!({"input": "what is scheduled", "class": 1}),
                json!({"input": "list system triggers", "class": 2}),
                json!({"input": "trigger list", "class": 1}),
                json!({"input": "show me all my scheduled runs", "class": 2}),
                json!({"input": "what recipes are scheduled to run", "class": 2}),
            ],
        )
        .await?;
    let recipe_trigger_list_active = stores
        .seed_recipe(
            &tenant,
            "trigger-list-active",
            "List only currently active triggers (scope='active').",
            true,
            RECIPE_TRIGGER_LIST_ACTIVE_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-trigger-list ToolSkill binding", "component", &[ts_trigger_list]),
                step_entry(2, "orchestrator", "PythonCode calls host.trigger_list(scope='active')", "component", &[pc_exec_trigger_list_active]),
            ],
            &[
                json!({"input": "show active triggers", "class": 1}),
                json!({"input": "what triggers are currently running", "class": 1}),
                json!({"input": "list enabled triggers", "class": 1}),
                json!({"input": "what is currently firing", "class": 2}),
                json!({"input": "show me the active automations", "class": 2}),
                json!({"input": "active trigger list", "class": 1}),
                json!({"input": "what triggers are live right now", "class": 1}),
                json!({"input": "show running triggers", "class": 1}),
                json!({"input": "which triggers are enabled", "class": 1}),
                json!({"input": "list triggers that are currently on", "class": 2}),
            ],
        )
        .await?;
    let recipe_trigger_list_scheduled = stores
        .seed_recipe(
            &tenant,
            "trigger-list-scheduled",
            "List only scheduled (cron/time-based) triggers (scope='scheduled').",
            true,
            RECIPE_TRIGGER_LIST_SCHEDULED_YAML,
            &[
                step_entry(1, "rust", "Pre-load ts-trigger-list ToolSkill binding", "component", &[ts_trigger_list]),
                step_entry(2, "orchestrator", "PythonCode calls host.trigger_list(scope='scheduled')", "component", &[pc_exec_trigger_list_scheduled]),
            ],
            &[
                json!({"input": "show scheduled triggers", "class": 1}),
                json!({"input": "what triggers run on a schedule", "class": 1}),
                json!({"input": "list cron triggers", "class": 1}),
                json!({"input": "what is scheduled to run", "class": 2}),
                json!({"input": "show me my scheduled automations", "class": 2}),
                json!({"input": "scheduled trigger list", "class": 1}),
                json!({"input": "what runs on a timer", "class": 2}),
                json!({"input": "list time-based triggers", "class": 1}),
                json!({"input": "show recurring triggers", "class": 2}),
                json!({"input": "what will run next based on schedule", "class": 2}),
            ],
        )
        .await?;
    let recipe_trigger_create = stores
        .seed_recipe(
            &tenant,
            "trigger-create",
            "Create a scheduled trigger for a recipe, with user confirmation.",
            false,
            RECIPE_TRIGGER_CREATE_YAML,
            &[
                step_entry(1, "orchestrator", "Load skill-trigger-create leaf skill body (creation procedure)", "component", &[skill_trigger_create]),
                step_entry(2, "orchestrator", "LLM translates schedule to cron, confirms with user, calls ts-trigger-create", "text", &[]),
                step_entry(3, "rust", "Pre-load ToolSkill bindings for list (pre-check) and create", "component", &[ts_trigger_list, ts_trigger_create]),
            ],
            &[
                json!({"input": "create a trigger to run X every morning", "class": 1}),
                json!({"input": "schedule recipe X every Monday", "class": 1}),
                json!({"input": "set up a daily trigger", "class": 1}),
                json!({"input": "run this recipe every hour", "class": 2}),
                json!({"input": "trigger create", "class": 1}),
                json!({"input": "schedule this recipe every 15 minutes", "class": 2}),
                json!({"input": "create a cron trigger for this recipe", "class": 1}),
                json!({"input": "set up an hourly trigger for X", "class": 1}),
                json!({"input": "automate this task to run weekly", "class": 2}),
                json!({"input": "schedule a recurring execution for this recipe", "class": 2}),
            ],
        )
        .await?;
    let recipe_trigger_remove = stores
        .seed_recipe(
            &tenant,
            "trigger-remove",
            "Remove a configured trigger by name, with user confirmation.",
            false,
            RECIPE_TRIGGER_REMOVE_YAML,
            &[
                step_entry(1, "orchestrator", "Load skill-trigger-remove leaf skill body (removal procedure)", "component", &[skill_trigger_remove]),
                step_entry(2, "orchestrator", "LLM confirms trigger name with user, warns about stoppage, calls ts-trigger-remove", "text", &[]),
                step_entry(3, "rust", "Pre-load ToolSkill bindings for list (pre-check) and remove", "component", &[ts_trigger_list, ts_trigger_remove]),
            ],
            &[
                json!({"input": "remove trigger X", "class": 1}),
                json!({"input": "delete this scheduled task", "class": 1}),
                json!({"input": "stop running recipe X", "class": 1}),
                json!({"input": "cancel the daily trigger", "class": 2}),
                json!({"input": "trigger remove", "class": 1}),
                json!({"input": "disable this scheduled trigger", "class": 2}),
                json!({"input": "stop the hourly trigger", "class": 1}),
                json!({"input": "delete the trigger named X", "class": 1}),
                json!({"input": "unschedule this recurring recipe", "class": 2}),
                json!({"input": "deactivate and remove this trigger", "class": 2}),
            ],
        )
        .await?;
    let recipe_trigger_remove_by_name = stores
        .seed_recipe(
            &tenant,
            "trigger-remove-by-name",
            "Remove a trigger by exact name — LLM confirms intent, then PythonCode resolves and removes.",
            false,
            RECIPE_TRIGGER_REMOVE_BY_NAME_YAML,
            &[
                step_entry(1, "orchestrator", "Load skill-trigger-remove leaf skill body (safety procedure)", "component", &[skill_trigger_remove]),
                step_entry(2, "orchestrator", "LLM confirms trigger name with user and warns about irreversibility", "text", &[]),
                step_entry(3, "rust", "Pre-load list + remove ToolSkill bindings", "component", &[ts_trigger_list, ts_trigger_remove]),
                step_entry(4, "orchestrator", "PythonCode: list triggers, find by exact name, remove — no LLM disambiguation", "component", &[pc_exec_trigger_resolve_and_remove]),
            ],
            &[
                json!({"input": "remove the trigger named X", "class": 1}),
                json!({"input": "delete trigger X", "class": 1}),
                json!({"input": "stop the trigger called X", "class": 1}),
                json!({"input": "cancel trigger by name", "class": 1}),
                json!({"input": "remove the scheduled trigger X", "class": 2}),
                json!({"input": "disable and remove trigger named X", "class": 2}),
                json!({"input": "delete this specific trigger by name", "class": 1}),
                json!({"input": "trigger remove by name", "class": 1}),
            ],
        )
        .await?;

    // Append the 3 trigger tools + 3 toolskills to ext-trigger-management and the
    // primary (dedup-idempotent; seeded in chunk 6a).
    let ext_trigger_children: Vec<Uuid> = vec![
        tool_trigger_create, ts_trigger_create, tool_trigger_list, ts_trigger_list,
        tool_trigger_remove, ts_trigger_remove,
    ];
    stores.append_children(cat_trigger, &ext_trigger_children).await?;
    stores.append_children(cat_process, &ext_trigger_children).await?;

    // Append the 4 trigger PythonCode + 5 leaf skills + 1 domain + 6 recipes to
    // ext-trigger-management and the primary (dedup-idempotent). Completes the
    // ext-trigger-management child set: 3 tools + 3 ts + 4 pc + 5 leaf skills
    // + 1 domain + 6 recipes.
    let ext_trigger_pc_skill_recipe_children: Vec<Uuid> = vec![
        pc_exec_trigger_list, pc_exec_trigger_list_active, pc_exec_trigger_list_scheduled,
        pc_exec_trigger_resolve_and_remove, skill_trigger_list, skill_trigger_create,
        skill_trigger_remove, skill_trigger_list_active, skill_trigger_list_scheduled,
        skill_triggers, recipe_trigger_list, recipe_trigger_list_active,
        recipe_trigger_list_scheduled, recipe_trigger_create, recipe_trigger_remove,
        recipe_trigger_remove_by_name,
    ];
    stores
        .append_children(cat_trigger, &ext_trigger_pc_skill_recipe_children)
        .await?;
    stores
        .append_children(cat_process, &ext_trigger_pc_skill_recipe_children)
        .await?;

    tracing::debug!(
        "seeded process group chunk 6f: trigger subgroup (4 pc + 5 leaf skills + 1 domain + 6 recipes) + catalogue appends (trigger subgroup COMPLETE: 3 tools + 3 ts + 4 pc + 5 leaf skills + 1 domain + 6 recipes; process group COMPLETE: shell + spawn + triggers)"
    );

    Ok(())
}

fn process_primary_catalogue_row(tenant: &str) -> NewPgExtensionCatalogue {
    NewPgExtensionCatalogue {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: CAT_PROCESS.to_string(),
        description: "Process domain capability catalogue (shell, spawn_subagent, \
                       trigger_create/list/remove)."
            .to_string(),
        version: "1.0".into(),
        overview_doc: CAT_PROCESS_OVERVIEW.into(),
        task_groups: json!([
            {"group_name": "shell-safe-fixed", "description": "Fixed-literal shell commands (Tier 0): git + system info"},
            {"group_name": "shell-custom", "description": "User-composed shell execution (Tier 1, LLM required)"},
            {"group_name": "agent-delegation", "description": "Child agent spawning and sub-task delegation"},
            {"group_name": "trigger-management", "description": "Scheduled trigger lifecycle: list, create, remove"}
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

fn tool_shell_row(tenant: &str) -> NewPgTool {
    NewPgTool {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "shell".to_string(),
        description: "Execute a shell command or script in the sandboxed process executor. \
                       Returns {output, exit_code, success, sandboxed}. When stdout+stderr \
                       exceeds the inline cap, the full output is saved to a scoped workspace \
                       file and the response body contains the saved path."
            .to_string(),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "command": {"type": "string", "description": "Shell command or multi-line script body"},
                "workdir": {"type": "string", "description": "Working directory (must be a backed scoped path)"},
                "timeout_secs": {"type": "number", "description": "Wall-clock timeout, max 120"},
                "extra_env": {"type": "object", "description": "Additional environment variables"}
            },
            "required": ["command"]
        })),
        param_template: Some(json!({"command": ""})),
        effect_type: "mixed".to_string(),
        preconditions: Some("".into()),
        error_handling: Some("".into()),
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        source: "system".into(),
        validation_status: "validated".into(),
        capability_id: "builtin.shell".into(),
    }
}

fn tool_spawn_subagent_row(tenant: &str) -> NewPgTool {
    NewPgTool {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "spawn_subagent".to_string(),
        description: "Spawn a child agent run to handle a sub-goal or delegated procedure."
            .to_string(),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "goal": {"type": "string", "description": "The task description or goal for the child agent."},
                "context": {"type": "string", "description": "Optional additional context to pass to the child. Plain text."},
                "recipe_name": {"type": "string", "description": "Optional: name of a recipe to seed the child's execution with."},
                "budget_tokens": {"type": "integer", "description": "Optional token budget cap for the child run. Inherits parent default if absent."}
            },
            "required": ["goal"]
        })),
        param_template: Some(json!({"goal": "{{goal}}"})),
        effect_type: "ExternalWrite".to_string(),
        preconditions: Some("".into()),
        error_handling: Some("".into()),
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        source: "system".into(),
        validation_status: "validated".into(),
        capability_id: "builtin.spawn_subagent".into(),
    }
}

fn tool_trigger_create_row(tenant: &str) -> NewPgTool {
    NewPgTool {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "trigger_create".to_string(),
        description: "Create a new scheduled or event-driven trigger for a recipe or task."
            .to_string(),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Human-readable trigger name."},
                "schedule": {"type": "string", "description": "Cron expression ('0 9 * * 1') or interval ('every 1h', 'every 30m')."},
                "recipe_name": {"type": "string", "description": "Name of the recipe to invoke on trigger."},
                "payload": {"type": "object", "description": "Optional input vars to pass to the recipe at trigger time."}
            },
            "required": ["name", "schedule", "recipe_name"]
        })),
        param_template: Some(json!({"name": "{{name}}"})),
        effect_type: "ExternalWrite".to_string(),
        preconditions: Some("".into()),
        error_handling: Some("".into()),
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        source: "system".into(),
        validation_status: "validated".into(),
        capability_id: "builtin.trigger_create".into(),
    }
}

fn tool_trigger_list_row(tenant: &str) -> NewPgTool {
    NewPgTool {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "trigger_list".to_string(),
        description: "List all configured triggers in the active scope.".to_string(),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "scope": {"type": "string", "description": "Scope filter: 'all' (default) | 'user' | 'system'."}
            },
            "required": []
        })),
        param_template: Some(json!({})),
        effect_type: "Read".to_string(),
        preconditions: Some("".into()),
        error_handling: Some("".into()),
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        source: "system".into(),
        validation_status: "validated".into(),
        capability_id: "builtin.trigger_list".into(),
    }
}

fn tool_trigger_remove_row(tenant: &str) -> NewPgTool {
    NewPgTool {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "trigger_remove".to_string(),
        description: "Remove a trigger by name. Irreversible — the scheduled task stops \
                       immediately."
            .to_string(),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "trigger_name": {"type": "string", "description": "Name of the trigger to remove."}
            },
            "required": ["trigger_name"]
        })),
        param_template: Some(json!({"trigger_name": "{{trigger_name}}"})),
        effect_type: "ExternalWrite".to_string(),
        preconditions: Some("".into()),
        error_handling: Some("".into()),
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        source: "system".into(),
        validation_status: "validated".into(),
        capability_id: "builtin.trigger_remove".into(),
    }
}

fn ts_shell_run_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-shell-run".to_string(),
        description: "Run a shell command via builtin.shell. Accepts command (required), optional \
                       workdir (must be a backed scoped path), optional timeout_secs (1–120). \
                       Returns {output, exit_code, success, sandboxed}. When output exceeds the \
                       inline cap, a saved_file path is returned — call read_file to retrieve it."
            .to_string(),
        content: "Call `host.shell(command=<command>, workdir=<optional backed scoped path>, \
                  timeout_secs=<optional 1..120>)` to run a shell command in the sandboxed process \
                  executor. Returns {output, exit_code, success, sandboxed}. When output exceeds \
                  the inline cap, a saved_file path is returned — call read_file on that path to \
                  retrieve the full content before proceeding."
            .to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("shell".to_string()),
        param_schema: Some(json!([
            {"name": "command", "param_type": "string", "required": true, "description": "Shell command or multi-line script"},
            {"name": "workdir", "param_type": "string", "required": false, "description": "Backed scoped working directory path"},
            {"name": "timeout_secs", "param_type": "number", "required": false, "description": "Timeout in seconds, max 120"}
        ])),
        param_template: Some(json!({"command": "{{command}}"})),
        consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

fn ts_spawn_subagent_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-spawn-subagent".to_string(),
        description: "ToolSkill binding for builtin.spawn_subagent — delegate a task to a child \
                       agent."
            .to_string(),
        content: r#"Tool: builtin.spawn_subagent
Effect: ExternalWrite — creates a child agent run.

Parameters:
- goal (string, required): the sub-goal for the child. Be precise — the child has no
  access to parent conversation history unless you include it in 'context'.
- context (string, optional): additional background text passed to the child. Include
  any file paths, decisions, or constraints the child needs.
- recipe_name (string, optional): if you want the child to start from a known recipe
  path, pass its name here. The recipe must be validation_status='validated'.
- budget_tokens (integer, optional): cap the child's token budget. Cannot exceed the
  parent's remaining budget.

Scope isolation invariants:
- The child runs in the same scope as the parent but cannot access parent-private
  session state or conversation history unless explicitly passed.
- The child cannot escalate authority beyond the parent's capability grants.
- Budget inheritance: if budget_tokens is omitted, the child inherits the parent
  session's default budget, not the parent's remaining balance.
- The child's tool approvals are independent — the user may need to re-approve the
  same tool in the child's context.

When to delegate:
- The sub-task is self-contained and would not benefit from the parent's ongoing context.
- The sub-task is long-running and you want to continue parent work in parallel.
- You are implementing a named procedure that has a stable recipe shape.

When NOT to delegate:
- When the task requires back-and-forth with the parent's current state.
- For trivial operations that take one or two tool calls.
"#
        .to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("spawn_subagent".to_string()),
        param_schema: None,
        param_template: None,
        consumer_tags: vec!["02:orchestrator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

fn ts_trigger_create_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-trigger-create".to_string(),
        description: "ToolSkill binding for builtin.trigger_create — schedule a recipe invocation."
            .to_string(),
        content: r#"Tool: builtin.trigger_create
Effect: ExternalWrite — registers a persistent scheduled run request.

Parameters:
- name (string, required): human-readable trigger name. Must be unique in scope.
- schedule (string, required): either a 5-field cron expression ('0 9 * * 1' = every
  Monday 9am) or a plain-English interval ('every 1h', 'every 30m', 'every day at 9am').
  The runtime normalizes interval syntax to cron internally.
- recipe_name (string, required): the Recipe to invoke. Must be installed and
  validation_status='validated'.
- payload (object, optional): key-value vars passed as input slots to the recipe.

Cron field order: minute hour day-of-month month day-of-week.
Examples:
  '0 9 * * 1'    → every Monday at 09:00
  '*/15 * * * *' → every 15 minutes
  'every 1h'     → every hour on the hour

Safety: triggers run with the authority of the creating session's scope. They cannot
escalate privilege beyond the scope in which they were created.
"#
        .to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("trigger_create".to_string()),
        param_schema: None,
        param_template: None,
        consumer_tags: vec!["02:orchestrator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

fn ts_trigger_list_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-trigger-list".to_string(),
        description: "ToolSkill binding for builtin.trigger_list — list all configured triggers."
            .to_string(),
        content: r#"Tool: builtin.trigger_list
Effect: Read — returns all triggers in scope as a JSON array.

Parameters:
- scope (string, optional): 'all' | 'user' | 'system'. Defaults to 'all'.

Output format:
  [{name, schedule, recipe_name, payload, created_at, last_fired_at, next_fire_at}]

Scope isolation: user-scope triggers are isolated from system-scope ones.
Always list before creating to avoid duplicate trigger names.
"#
        .to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("trigger_list".to_string()),
        param_schema: None,
        param_template: None,
        consumer_tags: vec!["02:orchestrator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

fn ts_trigger_remove_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-trigger-remove".to_string(),
        description: "ToolSkill binding for builtin.trigger_remove — remove a scheduled trigger."
            .to_string(),
        content: r#"Tool: builtin.trigger_remove
Effect: ExternalWrite — permanently removes the trigger. Stops immediately; any
pending next-fire for this trigger is discarded.

Parameters:
- trigger_name (string, required): exact name of the trigger to remove.

Safety:
- Always confirm with the user before removing a trigger — the scheduled task will
  stop and cannot be recovered (only re-created from scratch).
- Removing a trigger does not remove the recipe it pointed to.
"#
        .to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("trigger_remove".to_string()),
        param_schema: None,
        param_template: None,
        consumer_tags: vec!["02:orchestrator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

// ---------------------------------------------------------------------------
// Process group chunk 6b — shell PythonCode content bodies (verbatim from the
// doc; Q1 decision A). Bodies dispatch via `host.shell(command=...)`. Slot
// markers `{{vars.slotN}}` are stored verbatim (Rust raw strings do not
// interpret braces).
// ---------------------------------------------------------------------------

const PC_EXEC_SHELL_GIT_STATUS_CONTENT: &str = r#"# §shell-safe-fixed: command is a compile-time constant — no injection surface.
result = host.shell(command="git status")
"#;

const PC_EXEC_SHELL_GIT_LOG_CONTENT: &str = r#"# §shell-safe-fixed: fixed command, no user input, no injection surface.
result = host.shell(command="git log --oneline -20")
"#;

const PC_EXEC_SHELL_GIT_DIFF_STAT_CONTENT: &str = r#"# §shell-safe-fixed: fixed command, no user input, no injection surface.
result = host.shell(command="git diff --stat")
"#;

const PC_EXEC_SHELL_GIT_BRANCH_CONTENT: &str = r#"# §shell-safe-fixed: fixed command, no user input, no injection surface.
result = host.shell(command="git branch -a")
"#;

const PC_EXEC_SHELL_GIT_STASH_LIST_CONTENT: &str = r#"# §shell-safe-fixed: fixed command, no user input, no injection surface.
result = host.shell(command="git stash list")
"#;

const PC_EXEC_SHELL_GIT_LOG_N_CONTENT: &str = r#"_n = {{vars.slot0}}
# Validate: only safe integer in 1–100 range
if not isinstance(_n, int) or not (1 <= _n <= 100):
    _n = 20
result = host.shell(command=f"git log --oneline -{_n}")
"#;

const PC_EXEC_SHELL_GIT_REMOTE_CONTENT: &str = r#"# §shell-safe-fixed: fixed command, no user input.
result = host.shell(command="git remote -v")
"#;

const PC_EXEC_SHELL_GIT_SHOW_STAT_CONTENT: &str = r#"# §shell-safe-fixed: fixed command, no user input.
result = host.shell(command="git show --stat HEAD")
"#;

const PC_EXEC_SHELL_GIT_TAG_LIST_CONTENT: &str = r#"# §shell-safe-fixed: fixed command, no user input.
result = host.shell(command="git tag --list")
"#;

const PC_EXEC_SHELL_GIT_DIFF_NAME_ONLY_CONTENT: &str = r#"# §shell-safe-fixed: fixed command, no user input, no injection surface.
result = host.shell(command="git diff --name-only HEAD")
"#;

const PC_EXEC_SHELL_GIT_LOG_STAT_CONTENT: &str = r#"# §shell-safe-fixed: fixed command, no user input, no injection surface.
result = host.shell(command="git log --stat --oneline -5")
"#;

const PC_EXEC_SHELL_GIT_STASH_SHOW_CONTENT: &str = r#"# §shell-safe-fixed: fixed command, no user input, no injection surface.
result = host.shell(command="git stash show")
"#;

const PC_EXEC_SHELL_GIT_CONFIG_LIST_CONTENT: &str = r#"# §shell-safe-fixed: fixed command, no user input, no injection surface.
result = host.shell(command="git config --list")
"#;

const PC_EXEC_SHELL_GIT_ADD_CONTENT: &str = r#"# §shell-guard-custom — path is user/LLM-supplied. Tier 1 only.
_path = "{{vars.slot0}}" or "."
result = host.shell(command="git add " + _path)
"#;

const PC_EXEC_SHELL_GIT_COMMIT_CONTENT: &str = r#"# §shell-guard-custom — commit message is user/LLM-supplied. Tier 1 only.
_msg = "{{vars.slot0}}"
result = host.shell(command="git commit -m " + repr(_msg))
"#;

const PC_EXEC_SHELL_GIT_PUSH_CONTENT: &str = r#"# §shell-guard-custom — remote/branch are user-supplied. Tier 1 only.
_remote = "{{vars.slot0}}" or "origin"
_branch = "{{vars.slot1}}" or "main"
result = host.shell(command="git push " + _remote + " " + _branch)
"#;

const PC_EXEC_SHELL_GIT_PULL_CONTENT: &str = r#"# §shell-guard-custom — remote/branch are user-supplied. Tier 1 only.
_remote = "{{vars.slot0}}" or "origin"
_branch = "{{vars.slot1}}" or ""
_cmd = "git pull " + _remote
if _branch:
    _cmd = _cmd + " " + _branch
result = host.shell(command=_cmd)
"#;

const PC_EXEC_SHELL_GIT_FETCH_CONTENT: &str = r#"# §shell-safe-fixed — 'git fetch --all' is a fixed read-only remote query. No user input in command.
result = host.shell(command="git fetch --all")
"#;

const PC_EXEC_SHELL_PWD_CONTENT: &str = r#"# §shell-safe-fixed: fixed command, no user input.
result = host.shell(command="pwd")
"#;

const PC_EXEC_SHELL_DF_CONTENT: &str = r#"# §shell-safe-fixed: fixed command, no user input.
result = host.shell(command="df -h")
"#;

const PC_EXEC_SHELL_PS_CONTENT: &str = r#"# §shell-safe-fixed: fixed command, no user input.
result = host.shell(command="ps aux")
"#;

const PC_EXEC_SHELL_ENV_CONTENT: &str = r#"# §shell-safe-fixed: fixed command, no user input.
result = host.shell(command="env")
"#;

const PC_EXEC_SHELL_UNAME_CONTENT: &str = r#"# §shell-safe-fixed: fixed command, no user input.
result = host.shell(command="uname -a")
"#;

const PC_EXEC_SHELL_WHICH_CONTENT: &str = r#"import re as _re
_tool = "{{vars.slot0}}"
# Validate: only safe identifiers allowed (no injection surface)
if not _re.match(r'^[a-zA-Z0-9_\-]{1,64}$', _tool):
    result = {"error": "Invalid tool name — must be a safe identifier", "success": False}
else:
    result = host.shell(command=f"which {_tool}")
"#;

const PC_EXEC_SHELL_DATE_CONTENT: &str = r#"# §shell-safe-fixed: fixed command, no user input.
result = host.shell(command="date -u +%Y-%m-%dT%H:%M:%SZ")
"#;

const PC_EXEC_SHELL_HOSTNAME_CONTENT: &str = r#"# §shell-safe-fixed: fixed command, no user input.
result = host.shell(command="hostname")
"#;

const PC_EXEC_SHELL_WHOAMI_CONTENT: &str = r#"# §shell-safe-fixed: fixed command, no user input.
result = host.shell(command="whoami")
"#;

const PC_EXEC_SHELL_UPTIME_CONTENT: &str = r#"# §shell-safe-fixed: fixed command, no user input.
result = host.shell(command="uptime")
"#;

const PC_EXEC_SHELL_FREE_CONTENT: &str = r#"# §shell-safe-fixed: fixed command, no user input.
result = host.shell(command="free -h")
"#;

const PC_EXEC_SHELL_WC_L_CONTENT: &str = r#"import re as _re
_filepath = "{{vars.slot0}}"
# Validate: only allow safe relative paths (no shell metacharacters)
if not _re.match(r'^[a-zA-Z0-9_\-./]{1,256}$', _filepath):
    result = {"error": "Invalid filepath — must be a safe relative path", "success": False}
else:
    result = host.shell(command=f"wc -l {_filepath}")
"#;

// ---------------------------------------------------------------------------
// Process group chunk 6c — shell Leaf Skill + Domain Skill bodies.
// Seven leaf bodies + the domain body are transcribed verbatim from the doc.
// The remaining 24 leaf bodies are synthesized following the doc's own
// leaf-skill pattern (Q1 decision A): each names its exact pc, command, tier,
// what it returns, and the Tier-1 shell-run fallback.
// ---------------------------------------------------------------------------

const SKILL_SHELL_RUN_BODY: &str = r#"Use `ts-shell-run` when you need to execute one shell command. Pass the command
string verbatim; do NOT construct it from unvalidated user input without escaping.
Check `success` in the result; a false value means the command returned a non-zero
exit code — inspect `output` for details and decide whether to retry, report, or
continue. When the result contains a `saved_file` path (large output was saved),
call `skill-read-file` on that path to retrieve the full content before proceeding.
"#;

const SKILL_SHELL_SAFE_CHECK_BODY: &str = r#"Before dispatching any command via `ts-shell-run`, apply these rules:
- Never pass user-supplied strings directly into the command without escaping.
- Never run a command that modifies security-critical system files (/etc, /bin, etc.).
- Prefer scoped filesystem tools (skill-read-file, skill-list-dir, skill-grep) over
  shell equivalents (cat, ls, grep) when the structured tool covers the need.
- When output may exceed 1 MiB, add output-limiting flags (e.g. `head -n 200`).
- `builtin.shell` requires user approval (PermissionMode::Ask) — the LLM must present
  the command to the user and wait for confirmation before dispatch.
"#;

const SKILL_SHELL_GIT_STATUS_BODY: &str = r#"Use pc-exec-shell-git-status (§shell-safe-fixed) to run 'git status'.
Returns the working-tree state: staged, modified, and untracked files. Run this first
before any git write (add/commit) to know exactly what will be included. For a compact
one-line summary of history, use skill-shell-git-log instead.
"#;

const SKILL_SHELL_GIT_LOG_BODY: &str = r#"Use pc-exec-shell-git-log (§shell-safe-fixed) to run 'git log --oneline -20'.
Returns the last 20 commits as one-line summaries (hash + subject). Use to understand
recent project history before making changes. For per-file change counts, use
skill-shell-git-log-stat; for a custom count, use shell-run (Tier 1 — §shell-guard).
"#;

const SKILL_SHELL_GIT_DIFF_STAT_BODY: &str = r#"Use pc-exec-shell-git-diff-stat (§shell-safe-fixed) to run 'git diff --stat'.
Returns a per-file summary of insertions/deletions in the working tree. Use to gauge
the size of uncommitted changes before committing. For filenames only, use
skill-shell-git-diff-name-only; for full diff content, use shell-run (Tier 1).
"#;

const SKILL_SHELL_GIT_BRANCH_BODY: &str = r#"Use pc-exec-shell-git-branch (§shell-safe-fixed) to run 'git branch -a'.
Returns all local and remote-tracking branches with the current branch marked. Use
before checkout, merge, or push to confirm branch names. To create or switch branches,
use shell-run with a custom git command (Tier 1 — §shell-guard).
"#;

const SKILL_SHELL_GIT_STASH_LIST_BODY: &str = r#"Use pc-exec-shell-git-stash-list (§shell-safe-fixed) to run 'git stash list'.
Returns the stash stack (index@{n}: message). Use to review stashed work before popping.
For the diff summary of the top stash, use skill-shell-git-stash-show; to pop or apply,
use shell-run (Tier 1).
"#;

const SKILL_SHELL_GIT_REMOTE_BODY: &str = r#"Use pc-exec-shell-git-remote (§shell-safe-fixed) to run 'git remote -v'.
Returns each remote name with its fetch/push URLs. Use to confirm where a push or pull
will reach before dispatching a git-write command. To add or remove remotes, use
shell-run (Tier 1 — §shell-guard).
"#;

const SKILL_SHELL_GIT_SHOW_STAT_BODY: &str = r#"Use pc-exec-shell-git-show-stat (§shell-safe-fixed) to run 'git show --stat HEAD'.
Returns the last commit's message plus its changed-file summary and insertions/deletions.
Use to verify what the most recent commit actually contained. For older commits, use
shell-run with 'git show <hash>' (Tier 1).
"#;

const SKILL_SHELL_GIT_TAG_LIST_BODY: &str = r#"Use pc-exec-shell-git-tag-list (§shell-safe-fixed) to run 'git tag --list'.
Returns every tag in the repository. Use to discover release markers before checkout
or comparison. To create or delete tags, use shell-run (Tier 1 — §shell-guard).
"#;

const SKILL_SHELL_GIT_DIFF_NAME_ONLY_BODY: &str = r#"Use pc-exec-shell-git-diff-name-only (§shell-safe-fixed) to run 'git diff --name-only HEAD'.
Returns only the names of modified files — no content, no diff hunks. This is the fastest
way to discover which files are dirty before deciding which ones to inspect or read.
For full diff content, use shell-run with a custom git diff command (Tier 1 — §shell-guard).
"#;

const SKILL_SHELL_GIT_LOG_STAT_BODY: &str = r#"Use pc-exec-shell-git-log-stat (§shell-safe-fixed) to run 'git log --stat --oneline -5'.
Shows the last 5 commits with their changed-file counts and insertions/deletions summary.
For more commits or a different format, use shell-run (Tier 1 — §shell-guard).
"#;

const SKILL_SHELL_GIT_STASH_SHOW_BODY: &str = r#"Use pc-exec-shell-git-stash-show (§shell-safe-fixed) to run 'git stash show'.
Returns a short summary of files and line counts in the top stash entry. Does NOT pop
or apply the stash. To see the full patch or apply it, use shell-run (Tier 1).
"#;

const SKILL_SHELL_GIT_CONFIG_LIST_BODY: &str = r#"Use pc-exec-shell-git-config-list (§shell-safe-fixed) to run 'git config --list'.
Returns all effective git config key=value pairs (local, global, system). Useful for
checking user.email, user.name, remote settings, or merge strategy before making commits.
"#;

const SKILL_SHELL_PWD_BODY: &str = r#"Use pc-exec-shell-pwd (§shell-safe-fixed) to run 'pwd'.
Returns the absolute path of the current working directory. Use to confirm where
subsequent relative-path commands will resolve. To change directory, use shell-run
with a custom 'cd' command (Tier 1 — §shell-guard).
"#;

const SKILL_SHELL_DF_BODY: &str = r#"Use pc-exec-shell-df (§shell-safe-fixed) to run 'df -h'.
Returns per-mount disk usage with sizes in human-readable units. Use to check available
space before large writes or builds. For a specific path, use shell-run with
'df -h <path>' (Tier 1).
"#;

const SKILL_SHELL_PS_BODY: &str = r#"Use pc-exec-shell-ps (§shell-safe-fixed) to run 'ps aux'.
Returns all running processes with user, pid, cpu, memory, and command. Use to inspect
the process table or find a stray daemon. For a filtered view, use shell-run with
'ps aux | grep <pattern>' (Tier 1).
"#;

const SKILL_SHELL_ENV_BODY: &str = r#"Use pc-exec-shell-env (§shell-safe-fixed) to run 'env'.
Returns all environment variables exported in the current session as KEY=VALUE lines.
Use to confirm configuration before a command that depends on env vars. Never print
secrets-bearing vars to the chat without user consent.
"#;

const SKILL_SHELL_UNAME_BODY: &str = r#"Use pc-exec-shell-uname (§shell-safe-fixed) to run 'uname -a'.
Returns the kernel name, hostname, release, version, and architecture in one line. Use
to confirm the platform before issuing OS-specific commands.
"#;

const SKILL_SHELL_WHICH_BODY: &str = r#"Use pc-exec-shell-which (§shell-safe-fixed) to locate a binary. Pass the tool name via
vars.slot0 — it is validated against [a-zA-Z0-9_-]+ before dispatch, so there is no
injection surface. Returns the absolute path of the first matching binary on PATH, or
a not-found result. Use to confirm a tool is installed before invoking it.
"#;

const SKILL_SHELL_DATE_BODY: &str = r#"Use pc-exec-shell-date (§shell-safe-fixed) to run 'date -u +%Y-%m-%dT%H:%M:%SZ'.
Returns the current UTC date/time in ISO-8601. Use to timestamp logs, messages, or
memory entries consistently. For a local-time or custom format, use shell-run (Tier 1).
"#;

const SKILL_SHELL_HOSTNAME_BODY: &str = r#"Use pc-exec-shell-hostname (§shell-safe-fixed) to run 'hostname'.
Returns the machine's hostname. Use to identify the host in logs or diagnostics.
"#;

const SKILL_SHELL_WHOAMI_BODY: &str = r#"Use pc-exec-shell-whoami (§shell-safe-fixed) to run 'whoami'.
Returns the current effective user account name. Use to confirm which user a command
will run as before dispatching.
"#;

const SKILL_SHELL_UPTIME_BODY: &str = r#"Use pc-exec-shell-uptime (§shell-safe-fixed) to run 'uptime'.
Returns system uptime, the number of logged-in users, and the 1/5/15-minute load
averages. Use to gauge machine load before starting a heavy build or task.
"#;

const SKILL_SHELL_FREE_BODY: &str = r#"Use pc-exec-shell-free (§shell-safe-fixed) to run 'free -h'.
Returns total, used, free, and available memory in human-readable units (Linux only).
Use to check available RAM before a memory-intensive operation.
"#;

const SKILL_SHELL_WC_L_BODY: &str = r#"Use pc-exec-shell-wc-l (§shell-safe-fixed) to count lines in a file. Pass the filepath via
vars.slot0 — it is validated against [a-zA-Z0-9_-./]+ before dispatch, so there is no
shell-metacharacter injection surface. Returns the line count. Use to gauge file size
before reading it with skill-read-file.
"#;

const SKILL_SHELL_GIT_ADD_BODY: &str = r#"Use pc-exec-shell-git-add (via shell tool) to stage files.
§shell-guard-custom applies: always Tier 1.
Always run 'git status' (skill-shell-git-status) first to know which files are modified.
Pass '.' to stage all changes, or provide specific file paths. After staging, run
'git status' again to confirm the staged content before committing.
Never stage files the user has not confirmed — particularly .env, secrets, and
binary files should be explicitly confirmed before staging.
"#;

const SKILL_SHELL_GIT_COMMIT_BODY: &str = r#"Use pc-exec-shell-git-commit (via shell tool) to create a commit.
§shell-guard-custom applies: always Tier 1 — the commit message is user/LLM-supplied.
Always run 'git status' and 'git diff --staged' first to confirm what will be committed.
Pass the message via vars.slot0; it is repr()-escaped before dispatch. Never commit
without explicit user confirmation of both the staged content and the message.
Never commit secrets, .env files, or large binaries.
"#;

const SKILL_SHELL_GIT_PUSH_BODY: &str = r#"Use pc-exec-shell-git-push (via shell tool) to push commits.
§shell-guard-custom applies: always Tier 1 — remote and branch are user-supplied.
Always run 'git status' and 'git log <remote>/<branch>..HEAD' first to confirm what
will be pushed. Pass remote via vars.slot0 and branch via vars.slot1 (defaulting to
'origin' and 'main'). Never push without explicit user confirmation. Never force-push
without a separate explicit confirmation.
"#;

const SKILL_SHELL_GIT_PULL_BODY: &str = r#"Use pc-exec-shell-git-pull (via shell tool) to pull remote changes.
§shell-guard-custom applies: always Tier 1 — remote and branch are user-supplied.
Always run 'git status' first to ensure the working tree is clean (pull can fail on
conflicts). Pass remote via vars.slot0 and branch via vars.slot1. If conflicts arise,
the LLM must help resolve them — never overwrite local changes silently. Confirm the
pull with the user before dispatch.
"#;

const SKILL_SHELL_GIT_FETCH_BODY: &str = r#"Use pc-exec-shell-git-fetch (§shell-safe-fixed) to run 'git fetch --all'.
Updates all remote-tracking branches from all remotes without modifying the working
tree or current branch — a read-only remote query, Tier 0. Use before git-log or
git-diff to compare local state against the latest remote refs. To merge or rebase
after fetching, use git-pull or shell-run (Tier 1).
"#;

const SKILL_SHELL_BODY: &str = r#"Shell execution is the most powerful and most dangerous builtin. Use it only when no
higher-level tool covers the need (prefer filesystem domain tools for file operations;
prefer skill-http-fetch for network work).

TWO TIERS OF SHELL EXECUTION:

Tier 0 — Fixed pre-validated commands (§shell-safe-fixed):
Use when the command is a fixed literal with no user input. Zero injection surface.

Git inspection (status / diff / history):
— skill-shell-git-status:         'git status'
— skill-shell-git-log:            'git log --oneline -20'
— skill-shell-git-log-stat:       'git log --stat --oneline -5' (per-file change counts)
— skill-shell-git-diff-stat:      'git diff --stat'
— skill-shell-git-diff-name-only: 'git diff --name-only HEAD' (changed filenames only)
— skill-shell-git-branch:         'git branch -a'
— skill-shell-git-stash-list:     'git stash list'
— skill-shell-git-stash-show:     'git stash show' (diff summary of latest stash)
— skill-shell-git-remote:         'git remote -v'
— skill-shell-git-show-stat:      'git show --stat HEAD'
— skill-shell-git-tag-list:       'git tag --list'
— skill-shell-git-config-list:    'git config --list' (all active git config)

System information:
— skill-shell-pwd: run 'pwd'
— skill-shell-df: run 'df -h'
— skill-shell-ps: run 'ps aux'
— skill-shell-env: run 'env'
— skill-shell-uname: run 'uname -a'
— skill-shell-which: run 'which <tool>' (tool name is a fixed slot, not user-composed)
— skill-shell-date: run 'date -u' (UTC date/time)
— skill-shell-hostname: run 'hostname'
— skill-shell-whoami: run 'whoami'
— skill-shell-uptime: run 'uptime'
— skill-shell-free: run 'free -h' (Linux only)
— skill-shell-wc-l: run 'wc -l <file>' (line count, path validated)

Read-only git commands (fetch):
— skill-shell-git-fetch: 'git fetch --all' (Tier 0 — §shell-safe-fixed)

Decision guide for git work:
• What changed since last commit (names only) → skill-shell-git-diff-name-only (Tier 0)
• What changed in detail → shell-run 'git diff HEAD' (Tier 1 — custom)
• Recent commit history with stats → skill-shell-git-log-stat (Tier 0)
• What is in the stash → skill-shell-git-stash-show (Tier 0)
• Git identity/config check → skill-shell-git-config-list (Tier 0)
• Fetch latest remote refs without merging → skill-shell-git-fetch (Tier 0)

Tier 1 — Custom/user-composed commands (§shell-guard-custom):
Use when the command string involves user intent, user-supplied paths, or composition.
— skill-shell-run: run a single composed command (LLM validates and composes)
— skill-shell-safe-check: safety rules before composing any command

Git write operations (always Tier 1 — §shell-guard-custom):
— skill-shell-git-commit: 'git commit -m <msg>' (LLM composes message, user confirms)
— skill-shell-git-push: 'git push <remote> <branch>' (user confirms remote/branch)
— skill-shell-git-pull: 'git pull <remote> [branch]' (user confirms; LLM handles conflicts)

Safety rules before running any command → skill-shell-safe-check.
NEVER run a git commit/push/pull without explicit user confirmation.
NEVER run a command that the user supplied without LLM validation first.
"#;

// ---------------------------------------------------------------------------
// Process group chunk 6d — shell recipe YAML sources (verbatim doc
// step_descriptions blocks; WebUI renderer reads these, the IBS reads the
// resolved `steps` passed to seed_recipe). The `<uuid:*>` placeholders are
// preserved verbatim for the renderer; the real seeded UUIDs are bound in the
// step_entry calls above. 25 Tier-0 (2-step) + 6 Tier-1 (3-step with an
// annotation-only `type: "llm"` step preserved verbatim from the doc).
// ---------------------------------------------------------------------------

const RECIPE_SHELL_GIT_STATUS_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-git-status>"],
    "label":   "PythonCode calls host.shell(command='git status') — fixed literal"
  }
]
"#;

const RECIPE_SHELL_GIT_LOG_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-git-log>"],
    "label":   "PythonCode calls host.shell(command='git log --oneline -20')"
  }
]
"#;

const RECIPE_SHELL_GIT_DIFF_STAT_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-git-diff-stat>"],
    "label":   "PythonCode calls host.shell(command='git diff --stat')"
  }
]
"#;

const RECIPE_SHELL_GIT_BRANCH_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-git-branch>"],
    "label":   "PythonCode calls host.shell(command='git branch -a')"
  }
]
"#;

const RECIPE_SHELL_GIT_STASH_LIST_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-git-stash-list>"],
    "label":   "PythonCode calls host.shell(command='git stash list')"
  }
]
"#;

const RECIPE_SHELL_PWD_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-pwd>"],
    "label":   "PythonCode calls host.shell(command='pwd')"
  }
]
"#;

const RECIPE_SHELL_DF_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-df>"],
    "label":   "PythonCode calls host.shell(command='df -h')"
  }
]
"#;

const RECIPE_SHELL_PS_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-ps>"],
    "label":   "PythonCode calls host.shell(command='ps aux')"
  }
]
"#;

const RECIPE_SHELL_ENV_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-env>"],
    "label":   "PythonCode calls host.shell(command='env')"
  }
]
"#;

const RECIPE_SHELL_UNAME_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-uname>"],
    "label":   "PythonCode calls host.shell(command='uname -a')"
  }
]
"#;

const RECIPE_SHELL_WHICH_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-which>"],
    "label":   "PythonCode validates tool name then calls host.shell(command='which <tool>')"
  }
]
"#;

const RECIPE_SHELL_DATE_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-date>"],
    "label":   "PythonCode calls host.shell(command='date -u +...')"
  }
]
"#;

const RECIPE_SHELL_HOSTNAME_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-hostname>"],
    "label":   "PythonCode calls host.shell(command='hostname')"
  }
]
"#;

const RECIPE_SHELL_WHOAMI_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-whoami>"],
    "label":   "PythonCode calls host.shell(command='whoami')"
  }
]
"#;

const RECIPE_SHELL_UPTIME_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-uptime>"],
    "label":   "PythonCode calls host.shell(command='uptime')"
  }
]
"#;

const RECIPE_SHELL_FREE_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-free>"],
    "label":   "PythonCode calls host.shell(command='free -h')"
  }
]
"#;

const RECIPE_SHELL_GIT_REMOTE_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-git-remote>"],
    "label":   "PythonCode calls host.shell(command='git remote -v')"
  }
]
"#;

const RECIPE_SHELL_GIT_SHOW_STAT_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-git-show-stat>"],
    "label":   "PythonCode calls host.shell(command='git show --stat HEAD')"
  }
]
"#;

const RECIPE_SHELL_GIT_TAG_LIST_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-git-tag-list>"],
    "label":   "PythonCode calls host.shell(command='git tag --list')"
  }
]
"#;

const RECIPE_SHELL_WC_L_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-wc-l>"],
    "label":   "PythonCode validates path then calls host.shell(command='wc -l <file>')"
  }
]
"#;

const RECIPE_SHELL_GIT_DIFF_NAME_ONLY_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-git-diff-name-only>"],
    "label":   "PythonCode calls host.shell(command='git diff --name-only HEAD')"
  }
]
"#;

const RECIPE_SHELL_GIT_LOG_STAT_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-git-log-stat>"],
    "label":   "PythonCode calls host.shell(command='git log --stat --oneline -5')"
  }
]
"#;

const RECIPE_SHELL_GIT_STASH_SHOW_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-git-stash-show>"],
    "label":   "PythonCode calls host.shell(command='git stash show')"
  }
]
"#;

const RECIPE_SHELL_GIT_CONFIG_LIST_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-git-config-list>"],
    "label":   "PythonCode calls host.shell(command='git config --list')"
  }
]
"#;

const RECIPE_SHELL_GIT_FETCH_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-shell-git-fetch>"],
    "label":   "PythonCode calls host.shell(command='git fetch --all')"
  }
]
"#;

const RECIPE_SHELL_RUN_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-shell>", "<uuid:skill-shell-run>", "<uuid:skill-shell-safe-check>"],
    "label":   "Load shell domain + run + safety-check leaf skills"
  },
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM validates safety, composes the exact command, gets user approval"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Executor pre-loads ts-shell-run binding"
  }
]
"#;

const RECIPE_SHELL_SCRIPT_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-0",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-shell>", "<uuid:skill-shell-safe-check>"],
    "label":   "Load shell domain + safety-check context"
  },
  {
    "step_id": "step-1",
    "type":    "llm",
    "label":   "LLM writes the full script body, validates safety, gets user approval"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Executor pre-loads ts-shell-run binding"
  }
]
"#;

const RECIPE_SHELL_GIT_ADD_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-shell-git-add>", "<uuid:skill-shell-git-status>"],
    "label":   "Load git-add + git-status leaf skills"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run binding"
  },
  {
    "step_id": "step-3",
    "type":    "llm",
    "label":   "LLM checks git status, confirms which files to stage, dispatches git add"
  }
]
"#;

const RECIPE_SHELL_GIT_COMMIT_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-shell-git-commit>", "<uuid:skill-shell-git-status>"],
    "label":   "Load git-commit + git-status leaf skills"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run binding"
  },
  {
    "step_id": "step-3",
    "type":    "llm",
    "label":   "LLM checks git status, composes commit message, confirms with user, dispatches commit"
  }
]
"#;

const RECIPE_SHELL_GIT_PUSH_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-shell-git-push>", "<uuid:skill-shell-git-log>"],
    "label":   "Load git-push + git-log leaf skills"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run binding"
  },
  {
    "step_id": "step-3",
    "type":    "llm",
    "label":   "LLM shows recent commits, confirms remote/branch, dispatches push"
  }
]
"#;

const RECIPE_SHELL_GIT_PULL_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-shell-git-pull>", "<uuid:skill-shell-git-status>"],
    "label":   "Load git-pull + git-status leaf skills"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-shell-run>"],
    "label":   "Pre-load ts-shell-run binding"
  },
  {
    "step_id": "step-3",
    "type":    "llm",
    "label":   "LLM checks for local changes, confirms remote/branch, handles conflicts on failure"
  }
]
"#;

// ---------------------------------------------------------------------------
// Management group (Pass 5) — time, json, echo, skill_list/install/remove
// ---------------------------------------------------------------------------

/// Primary catalogue name for the management domain.
const CAT_MANAGEMENT: &str = "builtin-management";

const CAT_MANAGEMENT_OVERVIEW: &str = r#"# Management & Utility Capabilities

The management domain covers: skill lifecycle management, time operations, JSON
manipulation, and the diagnostic echo passthrough.

## Tools in this domain
- builtin.skill_list    — list installed skills
- builtin.skill_install — install a skill from URL/path (enters Q1/Q2)
- builtin.skill_remove  — remove an installed skill (irreversible)
- builtin.time          — time queries: now, parse, convert
- builtin.json          — JSON operations: query, stringify, parse, validate
- builtin.echo          — diagnostic passthrough (no user-facing recipe)

## Skill management
- Always list before installing (avoid duplicates).
- Always confirm with user before installing from external URLs or removing.
- After install, the skill is 'pending' — not usable until Q2 graduates it.
- System-scope skills cannot be modified from user-scope authority.

## Time utilities
- time/now: current UTC and local time in ISO 8601
- time/parse: parse a datetime string into components
- time/convert: convert between timezones or formats

## JSON utilities
- json/query: extract a value from a JSON structure via jq-style path
- json/stringify: serialize a value to a JSON string (pretty-printed)
- json/parse: parse a JSON string to a structured value
- json/validate: check whether a string is valid JSON

## Echo
Echo is a diagnostic-only passthrough. It has no user-facing recipe. Use it only
in tests and during recipe development.
"#;

const CAT_EXT_TIME_OVERVIEW: &str = r#"# Time Operations Capability
Tool: builtin.time
Effect: read_only

Provides five time operations via a single tool: now, parse, convert, diff, format.

Approaches:
- Get current time (UTC): → time-now recipe (Tier 0)
- Get current time in a timezone: → time-now-tz recipe (Tier 0)
- Parse a timestamp string: → time-parse recipe (Tier 0)
- Convert between timezones: → time-convert recipe (Tier 0)
- Compute duration between two timestamps: → time-diff recipe (Tier 0)
- Format a timestamp as a human-readable string: → time-format recipe (Tier 0)

PythonCode MUST NOT use datetime.now() — always use the time tool.
"#;

const CAT_EXT_JSON_OVERVIEW: &str = r#"# JSON Operations Capability
Tool: builtin.json
Effect: read_only

Provides four JSON operations via a single tool: query, stringify, parse, validate.

Approaches:
- Extract a field by path: → json-query recipe (Tier 0)
- Stringify / pretty-print: → json-stringify recipe (Tier 0)
- Parse JSON string: → json-parse recipe (Tier 0)
- Validate JSON syntax: → json-validate recipe (Tier 0)

Always validate before parsing when the source is external or user-provided.
"#;

const CAT_EXT_SKILL_MANAGEMENT_OVERVIEW: &str = r#"# Skill Management Capability
Tools: builtin.skill_list, builtin.skill_install, builtin.skill_remove
Effects: Read (list), Write (install/remove)

Manages the installed skill library. List is Tier 0. Install and Remove are Tier 1
(user confirmation required — both have side effects on the capability stack).

Approaches:
- List all skills: → skill-list recipe (Tier 0)
- List user skills only: → skill-list-user-only recipe (Tier 0)
- List system skills only: → skill-list-system-only recipe (Tier 0)
- Install a skill: → skill-install recipe (Tier 1)
- Remove a skill: → skill-remove recipe (Tier 1)
"#;

// ---------------------------------------------------------------------------
// Catalogue row builders — management group
// ---------------------------------------------------------------------------

fn management_primary_catalogue_row(tenant: &str) -> NewPgExtensionCatalogue {
    NewPgExtensionCatalogue {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: CAT_MANAGEMENT.to_string(),
        description: "Management & utility domain capability catalogue (skill_list, \
                       skill_install, skill_remove, time, json, echo)."
            .to_string(),
        version: "1.0".into(),
        overview_doc: CAT_MANAGEMENT_OVERVIEW.into(),
        task_groups: json!([
            {"group_name": "skill-management", "description": "Skill lifecycle: list, install, remove"},
            {"group_name": "time-utilities",   "description": "Time queries, parsing, and conversion"},
            {"group_name": "json-utilities",   "description": "JSON query, stringify, parse, validate"},
            {"group_name": "diagnostics",      "description": "Echo passthrough (development/testing only)"}
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

fn ext_time_catalogue_row(tenant: &str) -> NewPgExtensionCatalogue {
    ext_catalogue_row(
        tenant,
        "ext-time",
        "Per-tool extension catalogue for builtin.time (now, parse, convert, diff, format).",
        CAT_EXT_TIME_OVERVIEW,
        json!([
            {"group_name": "time-now",     "description": "Get current time"},
            {"group_name": "time-parse",   "description": "Parse timestamp strings"},
            {"group_name": "time-convert", "description": "Timezone conversion"},
            {"group_name": "time-diff",    "description": "Duration between timestamps"},
            {"group_name": "time-format",  "description": "Human-readable timestamp rendering"}
        ]),
    )
}

fn ext_json_catalogue_row(tenant: &str) -> NewPgExtensionCatalogue {
    ext_catalogue_row(
        tenant,
        "ext-json",
        "Per-tool extension catalogue for builtin.json (query, stringify, parse, validate).",
        CAT_EXT_JSON_OVERVIEW,
        json!([
            {"group_name": "json-query",           "description": "Extract values by path"},
            {"group_name": "json-stringify-parse", "description": "Serialize and deserialize"},
            {"group_name": "json-validate",        "description": "Syntax validation"}
        ]),
    )
}

fn ext_skill_management_catalogue_row(tenant: &str) -> NewPgExtensionCatalogue {
    ext_catalogue_row(
        tenant,
        "ext-skill-management",
        "Per-tool extension catalogue for the skill-management tools (skill_list, \
         skill_install, skill_remove).",
        CAT_EXT_SKILL_MANAGEMENT_OVERVIEW,
        json!([
            {"group_name": "skill-list",   "description": "Enumerate installed skills (scope-filtered)"},
            {"group_name": "skill-install", "description": "Install a new skill from URL/path"},
            {"group_name": "skill-remove",  "description": "Remove an installed skill"}
        ]),
    )
}

// ---------------------------------------------------------------------------
// Tool row builders — management group
// ---------------------------------------------------------------------------

fn tool_time_row(tenant: &str) -> NewPgTool {
    NewPgTool {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "time".to_string(),
        description: "Perform time and timezone operations: get the current time (now), parse a \
                       timestamp string (parse), convert between timezones (convert), compute the \
                       signed difference between two timestamps (diff), format a timestamp as a \
                       human-readable string (format)."
            .to_string(),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "operation":     {"type": "string", "enum": ["now","parse","convert","diff","format"]},
                "input":         {"type": "string"},
                "timezone":      {"type": "string"},
                "from_timezone": {"type": "string"},
                "to_timezone":   {"type": "string"},
                "timestamp2":    {"type": "string"},
                "format":        {"type": "string"},
                "format_string": {"type": "string"}
            },
            "additionalProperties": false
        })),
        param_template: Some(json!({"operation": "now"})),
        effect_type: "read_only".to_string(),
        preconditions: Some("invalid timezone → tool error; invalid timestamp → tool error".into()),
        error_handling: Some("invalid timezone → tool error; invalid timestamp → tool error".into()),
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        source: "system".into(),
        validation_status: "validated".into(),
        capability_id: "builtin.time".into(),
    }
}

fn tool_json_row(tenant: &str) -> NewPgTool {
    NewPgTool {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "json".to_string(),
        description: "Perform JSON operations: parse a JSON string (parse), serialize a value to \
                       a JSON string (stringify), extract a value by dot/bracket path (query), or \
                       validate whether a string is valid JSON (validate)."
            .to_string(),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "operation": {"type": "string", "enum": ["parse","stringify","query","validate"]},
                "data":      {},
                "path":      {"type": "string"}
            },
            "required": ["operation", "data"],
            "additionalProperties": false
        })),
        param_template: Some(json!({"operation": "{{operation}}", "data": "{{data}}"})),
        effect_type: "read_only".to_string(),
        preconditions: Some("operation required; data required".into()),
        error_handling: Some("invalid JSON for parse/query → tool error; path not found → null".into()),
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        source: "system".into(),
        validation_status: "validated".into(),
        capability_id: "builtin.json".into(),
    }
}

fn tool_echo_row(tenant: &str) -> NewPgTool {
    NewPgTool {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "echo".to_string(),
        description: "Diagnostic passthrough: returns input unchanged. For testing and stubs."
            .to_string(),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "message": {"type": "string", "description": "Any string. Returned verbatim."}
            },
            "required": ["message"]
        })),
        param_template: Some(json!({"message": "{{message}}"})),
        effect_type: "Read".to_string(),
        preconditions: None,
        error_handling: None,
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        source: "system".into(),
        validation_status: "validated".into(),
        capability_id: "builtin.echo".into(),
    }
}

fn tool_skill_list_row(tenant: &str) -> NewPgTool {
    NewPgTool {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "skill_list".to_string(),
        description: "List all skills currently installed in the active scope.".to_string(),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "scope": {"type": "string", "description": "Scope filter: 'all' | 'user' | 'system'. Defaults to 'all'."}
            },
            "required": []
        })),
        param_template: Some(json!({})),
        effect_type: "Read".to_string(),
        preconditions: None,
        error_handling: None,
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        source: "system".into(),
        validation_status: "validated".into(),
        capability_id: "builtin.skill_list".into(),
    }
}

fn tool_skill_install_row(tenant: &str) -> NewPgTool {
    NewPgTool {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "skill_install".to_string(),
        description: "Install a new skill from a URL or local path, entering the Q1/Q2 pipeline."
            .to_string(),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "source_url": {"type": "string", "description": "URL or local file path to skill manifest."},
                "scope": {"type": "string", "description": "Target scope: 'user' (default) or 'system'."}
            },
            "required": ["source_url"]
        })),
        param_template: Some(json!({"source_url": "{{source_url}}"})),
        effect_type: "Write".to_string(),
        preconditions: None,
        error_handling: None,
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        source: "system".into(),
        validation_status: "validated".into(),
        capability_id: "builtin.skill_install".into(),
    }
}

fn tool_skill_remove_row(tenant: &str) -> NewPgTool {
    NewPgTool {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "skill_remove".to_string(),
        description: "Remove an installed skill by name. Irreversible.".to_string(),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "skill_name": {"type": "string", "description": "Name of the skill to remove."},
                "scope": {"type": "string", "description": "Scope: 'user' | 'system'. Defaults to 'user'."}
            },
            "required": ["skill_name"]
        })),
        param_template: Some(json!({"skill_name": "{{skill_name}}"})),
        effect_type: "Write".to_string(),
        preconditions: None,
        error_handling: None,
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        source: "system".into(),
        validation_status: "validated".into(),
        capability_id: "builtin.skill_remove".into(),
    }
}

// ---------------------------------------------------------------------------
// ToolSkill row builders — management group
// ---------------------------------------------------------------------------
//
// `param_schema` uses the seeder-wide array format
// (`[{name, param_type, required, description}]`) for consistency with the
// filesystem/network ToolSkills, even though the doc renders the time/json
// ToolSkill schemas in JSON-Schema object form. The doc omits `consumer_tags`
// for ts-time-now/parse/convert and all ts-json-*; those inherit the parent
// tool's tags `["00:rusty","05:validator"]`, matching the explicitly-tagged
// siblings ts-time-diff/ts-time-format. The skill-management and echo
// ToolSkills carry the doc-verbatim `["02:orchestrator"]`.

fn ts_time_now_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-time-now".to_string(),
        description: "Executor binding: get the current UTC timestamp (operation='now'). \
                      Optional: timezone (IANA name) to return current time in a specific \
                      timezone."
            .to_string(),
        content: TS_TIME_NOW_CONTENT.to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("time".to_string()),
        param_schema: Some(json!([
            {"name": "operation", "param_type": "string", "required": true, "description": "Time operation (fixed 'now')"},
            {"name": "timezone",  "param_type": "string", "required": false, "description": "IANA timezone name (e.g. 'America/New_York')"}
        ])),
        param_template: Some(json!({"operation": "now"})),
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

fn ts_time_parse_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-time-parse".to_string(),
        description: "Executor binding: parse a timestamp string (operation='parse'). \
                      Required: input (timestamp string). Optional: timezone (IANA, for \
                      interpreting the input)."
            .to_string(),
        content: TS_TIME_PARSE_CONTENT.to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("time".to_string()),
        param_schema: Some(json!([
            {"name": "operation", "param_type": "string", "required": true, "description": "Time operation (fixed 'parse')"},
            {"name": "input",     "param_type": "string", "required": true, "description": "Timestamp string to parse"},
            {"name": "timezone",  "param_type": "string", "required": false, "description": "IANA timezone for interpreting naive input"}
        ])),
        param_template: Some(json!({"operation": "parse", "input": "{{input}}"})),
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

fn ts_time_convert_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-time-convert".to_string(),
        description: "Executor binding: convert a timestamp between timezones \
                      (operation='convert'). Required: input. Optional: from_timezone, \
                      to_timezone (IANA, default UTC)."
            .to_string(),
        content: TS_TIME_CONVERT_CONTENT.to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("time".to_string()),
        param_schema: Some(json!([
            {"name": "operation",     "param_type": "string", "required": true, "description": "Time operation (fixed 'convert')"},
            {"name": "input",         "param_type": "string", "required": true, "description": "Source timestamp string"},
            {"name": "from_timezone", "param_type": "string", "required": false, "description": "IANA timezone if input is naive (default UTC)"},
            {"name": "to_timezone",   "param_type": "string", "required": false, "description": "Target IANA timezone (default UTC)"}
        ])),
        param_template: Some(json!({"operation": "convert", "input": "{{input}}"})),
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

fn ts_time_diff_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-time-diff".to_string(),
        description: "Executor binding: compute the signed difference between two timestamps \
                      (operation='diff'). Required: input, timestamp2. Optional: timezone / \
                      from_timezone (IANA). Returns {seconds, minutes, hours, days} — signed."
            .to_string(),
        content: TS_TIME_DIFF_CONTENT.to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("time".to_string()),
        param_schema: Some(json!([
            {"name": "operation",     "param_type": "string", "required": true, "description": "Time operation (fixed 'diff')"},
            {"name": "input",         "param_type": "string", "required": true, "description": "First timestamp string"},
            {"name": "timestamp2",    "param_type": "string", "required": true, "description": "Second timestamp string"},
            {"name": "timezone",      "param_type": "string", "required": false, "description": "IANA timezone for both inputs"},
            {"name": "from_timezone", "param_type": "string", "required": false, "description": "Alias for timezone in diff context"}
        ])),
        param_template: Some(json!({"operation": "diff", "input": "{{input}}", "timestamp2": "{{timestamp2}}"})),
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

fn ts_time_format_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-time-format".to_string(),
        description: "Executor binding: format a timestamp as a human-readable string \
                      (operation='format'). Required: input. Optional: format_string (chrono), \
                      timezone, from_timezone (IANA). Returns {formatted, utc_iso, timezone?}."
            .to_string(),
        content: TS_TIME_FORMAT_CONTENT.to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("time".to_string()),
        param_schema: Some(json!([
            {"name": "operation",      "param_type": "string", "required": true, "description": "Time operation (fixed 'format')"},
            {"name": "input",          "param_type": "string", "required": true, "description": "Timestamp string to format"},
            {"name": "format_string",  "param_type": "string", "required": false, "description": "chrono format string, e.g. '%d %b %Y'"},
            {"name": "timezone",       "param_type": "string", "required": false, "description": "IANA timezone for the output"},
            {"name": "from_timezone",  "param_type": "string", "required": false, "description": "IANA timezone for interpreting a naive input"}
        ])),
        param_template: Some(json!({"operation": "format", "input": "{{input}}"})),
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

fn ts_json_query_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-json-query".to_string(),
        description: "Executor binding for json query operation. Required: operation='query', \
                      data (JSON string or value), path (dot/bracket path). Returns value at \
                      path or null."
            .to_string(),
        content: TS_JSON_QUERY_CONTENT.to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("json".to_string()),
        param_schema: Some(json!([
            {"name": "operation", "param_type": "string", "required": true, "description": "JSON operation (fixed 'query')"},
            {"name": "data",      "param_type": "any",    "required": true, "description": "JSON value or JSON string to query"},
            {"name": "path",      "param_type": "string", "required": true, "description": "Dot/bracket path, e.g. 'user.address.city'"}
        ])),
        param_template: Some(json!({"operation": "query", "data": "{{data}}", "path": "{{path}}"})),
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

fn ts_json_stringify_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-json-stringify".to_string(),
        description: "Executor binding for json stringify and parse operations. Required: \
                      operation ('stringify' or 'parse'), data. Stringify → formatted JSON \
                      string; parse → structured value from JSON string."
            .to_string(),
        content: TS_JSON_STRINGIFY_CONTENT.to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("json".to_string()),
        param_schema: Some(json!([
            {"name": "operation", "param_type": "string", "required": true, "description": "'stringify' (value → JSON string) or 'parse' (JSON string → value)"},
            {"name": "data",      "param_type": "any",    "required": true, "description": "Value to serialize, or JSON string to parse"}
        ])),
        param_template: Some(json!({"operation": "{{operation}}", "data": "{{data}}"})),
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

fn ts_json_validate_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-json-validate".to_string(),
        description: "Executor binding for json validate operation. Required: \
                      operation='validate', data (string to check). Returns {valid: bool, \
                      error: string|null}. Never a tool error."
            .to_string(),
        content: TS_JSON_VALIDATE_CONTENT.to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("json".to_string()),
        param_schema: Some(json!([
            {"name": "operation", "param_type": "string", "required": true, "description": "JSON operation (fixed 'validate')"},
            {"name": "data",      "param_type": "string", "required": true, "description": "String to check for JSON validity"}
        ])),
        param_template: Some(json!({"operation": "validate", "data": "{{data}}"})),
        consumer_tags: vec!["00:rusty".into(), "05:validator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

fn ts_skill_list_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-skill-list".to_string(),
        description: "ToolSkill binding for builtin.skill_list — deterministic scope-filtered \
                      listing."
            .to_string(),
        content: TS_SKILL_LIST_CONTENT.to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("skill_list".to_string()),
        param_schema: Some(json!([
            {"name": "scope", "param_type": "string", "required": false, "description": "Scope filter: 'all' | 'user' | 'system'. Defaults to 'all'."}
        ])),
        param_template: Some(json!({"scope": "{{scope}}"})),
        consumer_tags: vec!["02:orchestrator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

fn ts_skill_install_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-skill-install".to_string(),
        description: "ToolSkill binding for builtin.skill_install — installs a skill from \
                      URL/path."
            .to_string(),
        content: TS_SKILL_INSTALL_CONTENT.to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("skill_install".to_string()),
        param_schema: Some(json!([
            {"name": "source_url", "param_type": "string", "required": true,  "description": "URL or local file path to skill manifest"},
            {"name": "scope",      "param_type": "string", "required": false, "description": "Target scope: 'user' (default) or 'system'"}
        ])),
        param_template: Some(json!({"source_url": "{{source_url}}"})),
        consumer_tags: vec!["02:orchestrator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

fn ts_skill_remove_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-skill-remove".to_string(),
        description: "ToolSkill binding for builtin.skill_remove — removes a skill by name."
            .to_string(),
        content: TS_SKILL_REMOVE_CONTENT.to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("skill_remove".to_string()),
        param_schema: Some(json!([
            {"name": "skill_name", "param_type": "string", "required": true,  "description": "Name of the skill to remove"},
            {"name": "scope",      "param_type": "string", "required": false, "description": "Scope: 'user' | 'system'. Defaults to 'user'."}
        ])),
        param_template: Some(json!({"skill_name": "{{skill_name}}"})),
        consumer_tags: vec!["02:orchestrator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

fn ts_echo_row(tenant: &str) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "ts-echo".to_string(),
        description: "ToolSkill binding for builtin.echo — diagnostic passthrough, no \
                      user-facing recipe."
            .to_string(),
        content: TS_ECHO_CONTENT.to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("echo".to_string()),
        param_schema: Some(json!([
            {"name": "message", "param_type": "string", "required": true, "description": "Any string. Returned verbatim."}
        ])),
        param_template: Some(json!({"message": "{{message}}"})),
        consumer_tags: vec!["02:orchestrator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

// ---------------------------------------------------------------------------
// ToolSkill content constants — management group
// ---------------------------------------------------------------------------

const TS_TIME_NOW_CONTENT: &str = r#"Tool: builtin.time (operation='now')
Effect: read_only — returns the current UTC timestamp and optionally local time.

Parameters:
- operation (string, fixed): "now"
- timezone (string, optional): IANA timezone name (e.g. 'America/New_York'). When provided,
  the response includes both UTC and the local time in that zone.

Output: {utc_iso, local_iso?, timezone?}

Use for:
- Any PythonCode that needs the current time — NEVER use datetime.now() directly.
- Timestamping memory entries, log records, or display fields.
- The starting point for time-convert and time-format operations.
"#;

const TS_TIME_PARSE_CONTENT: &str = r#"Tool: builtin.time (operation='parse')
Effect: read_only — parses a timestamp string into a structured time value.

Parameters:
- operation (string, fixed): "parse"
- input (string, required): the timestamp to parse. Supports ISO 8601, RFC 2822, and
  common human-readable formats (e.g. '2024-01-15 10:30', 'Jan 15 2024', '15/01/2024').
- timezone (string, optional): IANA timezone for interpreting naive (timezone-less) input.

Output: {utc_iso, year, month, day, hour, minute, second, timezone?}
Error: unrecognised format → tool error.
"#;

const TS_TIME_CONVERT_CONTENT: &str = r#"Tool: builtin.time (operation='convert')
Effect: read_only — converts a timestamp between timezones.

Parameters:
- operation (string, fixed): "convert"
- input (string, required): source timestamp (ISO 8601 or any recognised format).
- from_timezone (string, optional): IANA timezone if input is naive (default: UTC).
- to_timezone (string, optional): target IANA timezone (default: UTC).

Output: {utc_iso, converted_iso, from_timezone, to_timezone}
Error: invalid timezone or unrecognised timestamp → tool error.
"#;

const TS_TIME_DIFF_CONTENT: &str = r#"Tool: builtin.time (operation='diff')
Effect: read_only — computes the signed difference between two timestamps.

Parameters:
- operation (string, fixed): "diff"
- input (string, required): first timestamp.
- timestamp2 (string, required): second timestamp.
- timezone / from_timezone (string, optional): IANA timezone for both inputs if naive.

Output: {seconds, minutes, hours, days} — all signed (positive when timestamp2 is after input).
Error: unrecognised format or ambiguous local time → tool error.
"#;

const TS_TIME_FORMAT_CONTENT: &str = r#"Tool: builtin.time (operation='format')
Effect: read_only — formats a timestamp as a human-readable string.

Parameters:
- operation (string, fixed): "format"
- input (string, required): source timestamp.
- format / format_string (string, optional): chrono format string.
  Default: '%Y-%m-%d %H:%M:%S %Z'. Examples: '%d %b %Y', '%I:%M %p', '%A, %B %-d, %Y'.
- timezone (string, optional): IANA timezone to express the output in.
- from_timezone (string, optional): IANA timezone for interpreting a naive input.

Output: {formatted, utc_iso, timezone?}
Error: unrecognised timestamp or invalid timezone → tool error.
"#;

const TS_JSON_QUERY_CONTENT: &str = r#"Tool: builtin.json (operation='query')
Effect: read_only — extracts a value from a JSON structure by dot/bracket path.

Parameters:
- operation (string, fixed): "query"
- data (any, required): JSON value or JSON string to query against.
- path (string, required): dot-separated or bracket-notation path (e.g. 'user.address.city',
  'items[0].name', 'response.data.results').

Output: the value at path, or null if not found.
Error: invalid JSON data → tool error. Path not found → null (not a tool error).
"#;

const TS_JSON_STRINGIFY_CONTENT: &str = r#"Tool: builtin.json (operation='stringify' or 'parse')
Effect: read_only — serializes a value to a JSON string or parses a JSON string.

Parameters:
- operation (string, required): 'stringify' (value → JSON string) or 'parse' (JSON string → value).
- data (any, required): the value to serialize, or the JSON string to parse.

Output (stringify): {result} — pretty-printed JSON string.
Output (parse): {result} — the parsed structured value.
Error (parse): invalid JSON string → tool error.
"#;

const TS_JSON_VALIDATE_CONTENT: &str = r#"Tool: builtin.json (operation='validate')
Effect: read_only — checks whether a string is syntactically valid JSON.

Parameters:
- operation (string, fixed): "validate"
- data (string, required): the string to check.

Output: {valid: bool, error: string|null}
Never a tool error — invalid JSON returns {valid: false, error: "..."}.
Use as a guard before json-parse when the source is external or user-provided.
"#;

const TS_SKILL_LIST_CONTENT: &str = r#"Tool: builtin.skill_list
Effect: Read — returns a JSON array of installed skills.

Parameters:
- scope (string, optional): 'all' (default) | 'user' | 'system'. Use 'user' when the user
  wants to see what they have installed. Use 'system' to inspect system-provided builtins.

Output format:
  [{name, class_code, description, source, validation_status, installed_at}]

Scope isolation: a 'user' scope call never returns system-only components. The agent
cannot modify system-scope skills without elevated authority.

When to use:
- Before installing a skill, list first to check whether it already exists.
- When the user asks "what skills do I have?"
- As the first step in any skill management recipe.
"#;

const TS_SKILL_INSTALL_CONTENT: &str = r#"Tool: builtin.skill_install
Effect: Write — installs a skill, creating a pending component that enters Q1 → Q2.

Parameters:
- source_url (string, required): URL (https://) or absolute local path to a skill manifest
  YAML/JSON. Remote URLs are fetched; the response must be a valid component manifest.
- scope (string, optional): 'user' (default) | 'system'.

Post-install state: the skill enters validation_status='pending' and goes through Q1.
If Q1 fails, the install is rejected and logged. Q2 graduation is required before the
skill is usable by the agent.

Safety note: always confirm with the user before installing from an unknown source URL.
Skills can contain PythonCode bodies that will execute in the orchestrator sandbox.
"#;

const TS_SKILL_REMOVE_CONTENT: &str = r#"Tool: builtin.skill_remove
Effect: Write — permanently removes a skill from the scope. Irreversible.

Parameters:
- skill_name (string, required): exact name of the skill to remove.
- scope (string, optional): 'user' (default) | 'system'.

Safety invariants:
- System-scope skills cannot be removed by user-scope calls.
- Removal of a skill that is referenced by an active recipe will fail with an error
  listing the dependent recipes. Resolve dependencies first.
- Always confirm with the user before removal — this cannot be undone without
  reinstalling.
"#;

const TS_ECHO_CONTENT: &str = r#"Tool: builtin.echo
Effect: Read — returns the input message unchanged.

Parameters:
- message (string, required): any string value.

Use cases (diagnostic / development only):
- Confirm that the orchestrator's tool dispatch pipeline is functional.
- Stub out a tool call during recipe development before the real tool is wired.
- Verify that slot variable interpolation is working in a PythonCode executor.

No user-facing recipe is defined for echo. Do not use echo in production recipe flows.
If you find yourself routing user requests through echo, use the correct tool instead.
"#;

// ---------------------------------------------------------------------------
// PythonCode body constants — management group
// ---------------------------------------------------------------------------

const PC_EXEC_TIME_NOW_CONTENT: &str = r#"# Orchestrator executor body.
_timezone = "{{vars.slot0}}"
_params = {"operation": "now"}
if _timezone and _timezone != "":
    _params["timezone"] = _timezone
result = host.time(**_params)
"#;

const PC_EXEC_TIME_PARSE_CONTENT: &str = r#"# Orchestrator executor body.
_input = "{{vars.slot0}}"
_timezone = "{{vars.slot1}}"
_params = {"operation": "parse", "input": _input}
if _timezone and _timezone != "":
    _params["timezone"] = _timezone
result = host.time(**_params)
"#;

const PC_EXEC_TIME_CONVERT_CONTENT: &str = r#"# Orchestrator executor body.
_input = "{{vars.slot0}}"
_from_tz = "{{vars.slot1}}"
_to_tz = "{{vars.slot2}}"
_params = {"operation": "convert", "input": _input}
if _from_tz and _from_tz != "":
    _params["from_timezone"] = _from_tz
if _to_tz and _to_tz != "":
    _params["to_timezone"] = _to_tz
result = host.time(**_params)
"#;

const PC_EXEC_TIME_DIFF_CONTENT: &str = r#"# Orchestrator executor body. No I/O, no imports, no network.
# IBS bakes in slot values before execution.
_input = "{{vars.slot0}}"
_ts2   = "{{vars.slot1}}"
result = host.time(operation="diff", input=_input, timestamp2=_ts2)
"#;

const PC_EXEC_TIME_FORMAT_CONTENT: &str = r#"# Orchestrator executor body. No I/O, no imports, no network.
# IBS bakes in slot values before execution.
_input  = "{{vars.slot0}}"
_fmt    = "{{vars.slot1}}"
_tz     = "{{vars.slot2}}"
_params = {"operation": "format", "input": _input}
if _fmt:
    _params["format_string"] = _fmt
if _tz:
    _params["timezone"] = _tz
result = host.time(**_params)
"#;

const PC_EXEC_JSON_QUERY_CONTENT: &str = r#"# Orchestrator executor body.
_data = {{vars.slot0}}
_path = "{{vars.slot1}}"
result = host.json(operation="query", data=_data, path=_path)
"#;

const PC_EXEC_JSON_STRINGIFY_CONTENT: &str = r#"# Orchestrator executor body.
_operation = "{{vars.slot0}}"
_data = {{vars.slot1}}
result = host.json(operation=_operation, data=_data)
"#;

const PC_EXEC_JSON_VALIDATE_CONTENT: &str = r#"# Orchestrator executor body.
_data = "{{vars.slot0}}"
result = host.json(operation="validate", data=_data)
"#;

const PC_EXEC_SKILL_LIST_CONTENT: &str = r#"# Orchestrator executor body.
_scope = "{{vars.slot0}}" if "{{vars.slot0}}" else "all"
result = host.skill_list(scope=_scope)
"#;

const PC_EXEC_ECHO_CONTENT: &str = r#"# Diagnostic executor body. host.<tool> provided by runtime sandbox.
_message = "{{vars.slot0}}"
result = host.echo(message=_message)
"#;

// ---------------------------------------------------------------------------
// Process group chunk 6e — spawn-subagent skill bodies (verbatim doc source)
// + spawn recipe YAML sources (verbatim doc flat format).
// ---------------------------------------------------------------------------

const SKILL_SPAWN_SUBAGENT_BODY: &str = r#"Use `ts-spawn-subagent` to create a child agent run for a self-contained sub-goal.

Before spawning:
1. Ensure the goal is truly self-contained — include all necessary context in the
   'context' field since the child cannot see the parent conversation.
2. Confirm with the user if the sub-task has any destructive or external effects.
3. Set budget_tokens if the sub-task should be bounded.

After spawning:
- The child result is returned as a structured object. Check result.status for
  'completed' | 'failed' | 'budget_exceeded'.
- If the child fails, report the failure reason to the user and decide whether to
  retry, rephrase the goal, or handle the sub-task in the parent context instead.
"#;

const SKILL_SPAWN_NAMED_PROCEDURE_BODY: &str = r#"Use `ts-spawn-subagent` with recipe_name set to invoke a known, stable procedure.

Use this when:
- You have a validated Recipe that encodes a complete procedure (e.g. 'file-patch',
  'memory-write', a user-installed skill recipe).
- You want the child to follow that procedure's recipe structure exactly rather than
  improvise from a goal description.

Pass relevant slot variables in 'context' as a structured key-value description:
  "vars: {slot0: '/path/to/file', slot1: 'search term'}"
The child's recipe loader will extract these into its vars map.
"#;

const SKILL_SPAWN_RESEARCH_BODY: &str = r#"Use `ts-spawn-subagent` with a goal written as a focused research question. Research
delegation works best when:
- The question is self-contained and answerable from memory, files, or web search.
- You want the answer returned as a structured summary (not inline back-and-forth).
- The child will need to call multiple tools (memory_search, glob, grep, or http).

Frame the goal as a question: "Research and summarise X, focusing on Y. Return a
structured summary with: key findings, relevant files/sources, open questions."
Include all constraints in the context field — the child has no access to parent state.
"#;

const SKILL_SPAWN_CODING_BODY: &str = r#"Use `ts-spawn-subagent` with a goal written as a concrete code task. Coding delegation
works best when:
- The task is scoped to a specific file, function, or module.
- The child needs to read files, apply patches, and report a result.
- The task is too long to inline and benefits from isolated execution.

Frame the goal concretely: "Read /path/to/file, fix the bug described by X, write the
corrected version back. Return the diff of changes made."
Include all file paths and error descriptions in the context field.
Set budget_tokens appropriately — coding tasks can be token-heavy.
"#;

const SKILL_SPAWN_EXPLORATION_BODY: &str = r#"Use `ts-spawn-subagent` with a goal written as a deep-analysis question. Exploration
delegation works best when:
- The task is read-only (no file writes, no shell execution with side effects).
- You want the child to map out a codebase area, trace a dependency, or catalogue
  patterns across many files.
- The result is a structured report or inventory.

Frame the goal as an analysis assignment: "Explore all Rust files under crates/X/,
identify all public trait definitions, and return a structured inventory with trait
names, file paths, and method signatures."
Explicitly state "read-only — do not modify any files" in the goal if needed.
"#;

const SKILL_SPAWN_QUERY_BODY: &str = r#"Use `ts-spawn-subagent` when the user asks a specific factual question that requires
one or two tool lookups (memory_search, grep, glob, or a quick http fetch) to answer.
Query delegation avoids cluttering the parent context with intermediate tool results.

Frame the goal as a direct question with expected output shape: "Find the current
version of X in Cargo.toml and return it. Return only the version string."
Keep goals short and unambiguous — the child will return a text result, not continue
a conversation.
"#;

const SKILL_SUBAGENT_BODY: &str = r#"Delegation gives the parent agent a way to hand off a well-scoped sub-task to a
child run with full tool access and its own budget.

Choosing a delegation grain:
- skill-spawn-subagent: general goal delegation — write a clear, self-contained goal
- skill-spawn-named-procedure: procedure delegation — use an existing validated Recipe
- skill-spawn-research: research/info-gathering delegation — returns structured summary
- skill-spawn-coding: coding task delegation — file reads, patches, reports changes
- skill-spawn-exploration: read-only deep analysis — returns catalogue or report
- skill-spawn-query: focused single-question lookup — returns direct answer

Decision guide:
• Generic open-ended sub-goal → skill-spawn-subagent
• Run a known recipe in a child → skill-spawn-named-procedure
• Research / web lookups / memory searches → skill-spawn-research
• File editing, debugging, writing code → skill-spawn-coding
• Mapping codebase structure, tracing deps → skill-spawn-exploration
• Single factual question needing 1-2 tool calls → skill-spawn-query

Critical safety invariants (§spawn_subagent-guard):
- Any Recipe binding spawn_subagent MUST have llm_call_required=true (hard Q1 rule).
  There is NO Tier-0 spawn recipe — the LLM must always be in the loop to frame
  the goal and confirm delegation.
- Child cannot exceed parent scope or authority.
- Budget inheritance is from the session default, not parent remaining balance.
- Include all needed context explicitly — child has no parent conversation access.
"#;

const RECIPE_SUBAGENT_SPAWN_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-spawn-subagent>", "<uuid:skill-spawn-named-procedure>"],
    "label":   "Load spawn leaf skills (goal delegation + named procedure patterns)"
  },
  {
    "step_id": "step-2",
    "type":    "llm",
    "label":   "LLM frames the goal, decides generic-vs-recipe delegation, confirms with user, calls ts-spawn-subagent"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-spawn-subagent>"],
    "label":   "Pre-load ts-spawn-subagent ToolSkill binding"
  }
]
"#;

const RECIPE_SUBAGENT_RESEARCH_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-spawn-research>"],
    "label":   "Load research delegation leaf skill body"
  },
  {
    "step_id": "step-2",
    "type":    "llm",
    "label":   "LLM frames focused research goal string, sets context, calls ts-spawn-subagent"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-spawn-subagent>"],
    "label":   "Pre-load ts-spawn-subagent ToolSkill binding"
  }
]
"#;

const RECIPE_SUBAGENT_CODING_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-spawn-coding>"],
    "label":   "Load coding delegation leaf skill body"
  },
  {
    "step_id": "step-2",
    "type":    "llm",
    "label":   "LLM scopes the code task, includes file paths + constraints in context, calls ts-spawn-subagent"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-spawn-subagent>"],
    "label":   "Pre-load ts-spawn-subagent ToolSkill binding"
  }
]
"#;

const RECIPE_SUBAGENT_EXPLORATION_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-spawn-exploration>"],
    "label":   "Load exploration delegation leaf skill body"
  },
  {
    "step_id": "step-2",
    "type":    "llm",
    "label":   "LLM defines exploration scope and output format, calls ts-spawn-subagent"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-spawn-subagent>"],
    "label":   "Pre-load ts-spawn-subagent ToolSkill binding"
  }
]
"#;

const RECIPE_SUBAGENT_QUERY_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-spawn-query>"],
    "label":   "Load query delegation leaf skill body"
  },
  {
    "step_id": "step-2",
    "type":    "llm",
    "label":   "LLM formulates focused single-question goal, expected output shape, calls ts-spawn-subagent"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-spawn-subagent>"],
    "label":   "Pre-load ts-spawn-subagent ToolSkill binding"
  }
]
"#;

// ---------------------------------------------------------------------------
// Process group chunk 6f — trigger PythonCode bodies + trigger skill bodies
// (verbatim doc source) + trigger recipe YAML sources (verbatim doc flat format).
// ---------------------------------------------------------------------------

const PC_EXEC_TRIGGER_LIST_CONTENT: &str = r#"# Orchestrator executor body.
_scope = "{{vars.slot0}}" if "{{vars.slot0}}" else "all"
result = host.trigger_list(scope=_scope)
"#;

const PC_EXEC_TRIGGER_LIST_ACTIVE_CONTENT: &str = r#"# Orchestrator executor body.
result = host.trigger_list(scope="active")
"#;

const PC_EXEC_TRIGGER_LIST_SCHEDULED_CONTENT: &str = r#"# Orchestrator executor body.
result = host.trigger_list(scope="scheduled")
"#;

const PC_EXEC_TRIGGER_RESOLVE_AND_REMOVE_CONTENT: &str = r#"# Orchestrator executor body. No I/O, no imports, no network.
# IBS bakes in slot values before execution.
_trigger_name = "{{vars.slot0}}"
_list_result  = host.trigger_list()
_triggers     = _list_result.get("triggers", []) if isinstance(_list_result, dict) else []
_found        = next((t for t in _triggers if t.get("name") == _trigger_name), None)
if _found is None:
    result = {"removed": False, "trigger_name": _trigger_name,
              "error": f"No trigger named '{_trigger_name}' found"}
else:
    _remove_result = host.trigger_remove(trigger_name=_trigger_name)
    result = {"removed": True, "trigger_name": _trigger_name, "remove_result": _remove_result}
"#;

const SKILL_TRIGGER_LIST_BODY: &str = r#"Use `ts-trigger-list` (via pc-exec-trigger-list) to retrieve a JSON array of all
configured triggers. Inspect schedule, recipe_name, last_fired_at, and next_fire_at
to give the user a clear picture of what is scheduled. Always list before creating
to avoid name collisions.
"#;

const SKILL_TRIGGER_CREATE_BODY: &str = r#"Use `ts-trigger-create` to register a recurring or one-off trigger. Always:
1. List existing triggers first (ts-trigger-list) to check for name conflicts.
2. Confirm the schedule with the user — translate their natural-language request
   ('every Monday morning') into a cron expression ('0 9 * * 1') and confirm it.
3. Confirm the recipe_name exists and is validation_status='validated'.
4. Optionally confirm any payload vars with the user before creating.
Triggers have ExternalWrite effect — the user should explicitly approve creation.
"#;

const SKILL_TRIGGER_REMOVE_BODY: &str = r#"Use `ts-trigger-remove` to permanently remove a scheduled trigger by name. Always:
1. List triggers first to confirm the trigger exists and show the user its schedule.
2. Confirm with the user that removal is intended — the trigger stops immediately
   and cannot be recovered without re-creating it from scratch.
Triggers have ExternalWrite effect — explicit user approval is required.
"#;

const SKILL_TRIGGER_LIST_ACTIVE_BODY: &str = r#"Use pc-exec-trigger-list-active to call trigger_list with scope='active'. Returns only
triggers that are currently running or enabled. Use this when the user wants to know
what is actively firing right now — not scheduled-but-paused entries.
Compare with skill-trigger-list (all triggers) and skill-trigger-list-scheduled (cron).
"#;

const SKILL_TRIGGER_LIST_SCHEDULED_BODY: &str = r#"Use pc-exec-trigger-list-scheduled to call trigger_list with scope='scheduled'. Returns
only triggers that run on a cron or time-interval basis. Use when the user is asking
about what is set up to run on a schedule, not manual/event triggers.
"#;

const SKILL_TRIGGERS_BODY: &str = r#"Triggers are persistent scheduled invocations of recipes. Use the right grain:

LISTING TRIGGERS:
- skill-trigger-list: list ALL triggers (all scopes) — always start here.
- skill-trigger-list-active: list ONLY currently active/enabled triggers (Tier-0).
- skill-trigger-list-scheduled: list ONLY scheduled (cron/time-based) triggers (Tier-0).

Decision: if user says "what triggers do I have" → trigger-list.
If user says "what is currently active/running" → trigger-list-active.
If user says "what is scheduled/recurring" → trigger-list-scheduled.

CREATING A TRIGGER:
- skill-trigger-create: confirm schedule (cron) + recipe_name + payload with user.
  Translate natural language schedule to cron and verify before committing.

REMOVING A TRIGGER:
- skill-trigger-remove: confirm name, warn about immediate stoppage, then remove.
- skill-trigger-list + skill-trigger-remove-by-name: when name is known exactly
  (use pc-exec-trigger-resolve-and-remove — find by exact name then remove).

Safety rules:
- trigger_create and trigger_remove both have ExternalWrite effect — require explicit
  user confirmation for each.
- Triggers run with the creating session's authority; they cannot escalate privilege.
- A trigger referencing a recipe that is later removed will fail at fire time —
  inform the user of this risk when removing recipes.
"#;

const RECIPE_TRIGGER_LIST_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-trigger-list>"],
    "label":   "Pre-load ts-trigger-list ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-trigger-list>"],
    "label":   "PythonCode calls host.trigger_list(scope)"
  }
]
"#;

const RECIPE_TRIGGER_LIST_ACTIVE_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-trigger-list>"],
    "label":   "Pre-load ts-trigger-list ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-trigger-list-active>"],
    "label":   "PythonCode calls host.trigger_list(scope='active')"
  }
]
"#;

const RECIPE_TRIGGER_LIST_SCHEDULED_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-trigger-list>"],
    "label":   "Pre-load ts-trigger-list ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-trigger-list-scheduled>"],
    "label":   "PythonCode calls host.trigger_list(scope='scheduled')"
  }
]
"#;

const RECIPE_TRIGGER_CREATE_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-trigger-create>"],
    "label":   "Load skill-trigger-create leaf skill body (creation procedure)"
  },
  {
    "step_id": "step-2",
    "type":    "llm",
    "label":   "LLM translates schedule to cron, confirms with user, calls ts-trigger-create"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-trigger-list>", "<uuid:ts-trigger-create>"],
    "label":   "Pre-load ToolSkill bindings for list (pre-check) and create"
  }
]
"#;

const RECIPE_TRIGGER_REMOVE_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-trigger-remove>"],
    "label":   "Load skill-trigger-remove leaf skill body (removal procedure)"
  },
  {
    "step_id": "step-2",
    "type":    "llm",
    "label":   "LLM confirms trigger name with user, warns about stoppage, calls ts-trigger-remove"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-trigger-list>", "<uuid:ts-trigger-remove>"],
    "label":   "Pre-load ToolSkill bindings for list (pre-check) and remove"
  }
]
"#;

const RECIPE_TRIGGER_REMOVE_BY_NAME_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-trigger-remove>"],
    "label":   "Load skill-trigger-remove leaf skill body (safety procedure)"
  },
  {
    "step_id": "step-2",
    "type":    "llm",
    "label":   "LLM confirms trigger name with user and warns about irreversibility"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-trigger-list>", "<uuid:ts-trigger-remove>"],
    "label":   "Pre-load list + remove ToolSkill bindings"
  },
  {
    "step_id": "step-4",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-trigger-resolve-and-remove>"],
    "label":   "PythonCode: list triggers, find by exact name, remove — no LLM disambiguation"
  }
]
"#;

// ---------------------------------------------------------------------------
// Leaf Skill + Domain Skill body constants — management group (chunk 7b)
// Transcribed verbatim from builtin_stuff_v3.md Steps 14/15/16/14.x/15.x.
// ---------------------------------------------------------------------------

const SKILL_TIME_NOW_BODY: &str = r#"Use `ts-time-now` (via pc-exec-time-now) to get the current UTC timestamp. Provide a
timezone parameter if the user specified a locale (e.g. 'America/New_York'). The returned
timestamp can be used as input to other time operations or to stamp memory entries.
PythonCode that needs the current time must always call this first — never use datetime.
"#;

const SKILL_TIME_PARSE_BODY: &str = r#"Use `ts-time-parse` (via pc-exec-time-parse) to interpret a date or time in text form.
Supports ISO 8601, RFC 2822, and common human-readable formats. Provide timezone when
the input is ambiguous about its timezone context.
"#;

const SKILL_TIME_CONVERT_BODY: &str = r#"Use `ts-time-convert` (via pc-exec-time-convert) to express a timestamp in a different
timezone. Provide the input timestamp and the target `to_timezone` (IANA name, e.g.
'America/New_York', 'Europe/Berlin', 'Asia/Tokyo'). Optionally specify `from_timezone`
if the input's timezone is ambiguous.
"#;

const SKILL_TIME_DIFF_BODY: &str = r#"Use `ts-time-diff` (via pc-exec-time-diff) to compute the signed duration between
two timestamps. Provide both `input` (first timestamp) and `timestamp2` (second
timestamp) as ISO 8601 strings or any recognised format. The result contains
`seconds`, `minutes`, `hours`, and `days` — all signed (positive when timestamp2
is after input). If the inputs are in local time without timezone info, supply
`from_timezone` (IANA name). Use this when the user asks 'how long ago', 'how
many days between', or 'what is the duration between these dates'.
"#;

const SKILL_TIME_FORMAT_BODY: &str = r#"Use `ts-time-format` (via pc-exec-time-format) to render a timestamp in a custom
or human-readable format. Provide `input` as the source timestamp and optionally
`format_string` using chrono format codes (e.g. `'%d %b %Y'`, `'%I:%M %p'`,
`'%A, %B %-d, %Y'`). Optionally supply `timezone` (IANA) to localise the output.
Default format is `'%Y-%m-%d %H:%M:%S %Z'`. Use when the user asks to display a
date in a particular style, or when building a human-readable timestamp label for
memory entries, reports, or logs.
"#;

const SKILL_JSON_QUERY_BODY: &str = r#"Use `ts-json-query` (via pc-exec-json-query) to extract a specific field from a JSON
structure. Provide the data and a dot-separated path (e.g. 'user.address.city' or
'items.0.name'). Returns null if the path does not exist. For multi-field extraction,
use pc-json-extract-field PythonCode instead.
"#;

const SKILL_JSON_STRINGIFY_BODY: &str = r#"Use `ts-json-stringify` with operation='stringify' (via pc-exec-json-stringify) to
format a structured value as a human-readable JSON string for display or for writing
to a file. The result is pretty-printed.
"#;

const SKILL_JSON_PARSE_BODY: &str = r#"Use `ts-json-stringify` with operation='parse' (via pc-exec-json-stringify) when you
have a raw JSON string (e.g. from a tool response body) and need to work with it as
a structured value. The result can then be queried with ts-json-query or
pc-json-extract-field.
"#;

const SKILL_JSON_VALIDATE_BODY: &str = r#"Use `ts-json-validate` (via pc-exec-json-validate) to check whether a string is
syntactically valid JSON before attempting to parse or process it. Returns {valid: bool,
error: string|null}. Useful as a guard before running json-query or json-parse.
"#;

const SKILL_JSON_PARSE_AND_QUERY_BODY: &str = r#"Use the two-step parse + query pattern when you receive a raw JSON string (e.g. from
an HTTP response body) and immediately need a specific field. The pattern:
1. Call ts-json (operation='parse') to get the structured object (via pc-exec-json-stringify).
2. Call ts-json (operation='query') with a dot-path to extract the field.

Alternatively, use pc-json-extract-field (pure Python) if the json tool is not bound.
Always validate with json-validate before parse if the source is external or user-supplied.
"#;

const SKILL_SKILL_LIST_BODY: &str = r#"Use `ts-skill-list` (via pc-exec-skill-list) to retrieve a JSON array of all installed
skills. Pass scope='user' to see only user-installed skills. Pass scope='system' to
inspect system builtins. Omit scope (or pass 'all') to see everything.
Check the returned array before deciding to install a skill — avoid duplicates.
"#;

const SKILL_SKILL_INSTALL_BODY: &str = r#"Use `ts-skill-install` to fetch and register a skill manifest. Always:
1. Run `ts-skill-list` first to confirm the skill does not already exist.
2. Confirm the source URL with the user before proceeding.
3. After install, inform the user the skill enters validation_status='pending' and
   cannot be used until Q1 and Q2 pass. Do not promise immediate availability.
"#;

const SKILL_SKILL_REMOVE_BODY: &str = r#"Use `ts-skill-remove` to permanently remove a skill by name. Always:
1. Run `ts-skill-list` first to confirm the skill exists and note its scope.
2. Confirm with the user that removal is intended and irreversible.
3. If the tool returns a dependency error (recipes reference this skill), resolve those
   first or inform the user of the blocker.
"#;

// Domain skills (class 2). skill-time / skill-json carry the long domain text as
// their body (the doc places it in the `description` field); a short one-line
// description is supplied at the seed call site. skill-skills follows the doc's
// short-description + long-body split verbatim.

const SKILL_TIME_BODY: &str = r#"The time domain provides one tool for all time operations:

GETTING CURRENT TIME:
— skill-time-now: Get the current UTC timestamp (and optionally in a timezone).

PARSING:
— skill-time-parse: Parse a timestamp string into a structured time value.

CONVERTING:
— skill-time-convert: Convert a timestamp to a different timezone.

DIFFING:
— skill-time-diff: Compute the signed duration between two timestamps.
  Returns {seconds, minutes, hours, days}. Positive = timestamp2 is after input.

FORMATTING:
— skill-time-format: Render a timestamp as a human-readable string.
  Uses chrono format codes. Default: '%Y-%m-%d %H:%M:%S %Z'.

Decision guide:
• What time is it now → skill-time-now
• Time in a specific timezone → skill-time-now (with timezone parameter)
• Parse a date/time string → skill-time-parse
• Convert between timezones → skill-time-convert
• How long between two timestamps → skill-time-diff
• Display a date in a human-readable style → skill-time-format

PythonCode in the orchestrator must NEVER use datetime.now() or any date
library directly — always call skill-time-now first to get the current time
from the runtime clock.
"#;

const SKILL_JSON_BODY: &str = r#"The JSON domain provides one tool for four JSON operations:

EXTRACTING:
— skill-json-query: Extract a value by dot/bracket path from a JSON structure.
— skill-json-parse-and-query: Parse a JSON string AND immediately extract a field
  (combined two-step pattern — Tier-0, both pre-baked in vars).

SERIALIZING:
— skill-json-stringify: Convert a structured value to a pretty-printed JSON string.

PARSING:
— skill-json-parse: Parse a JSON string into a structured value.

VALIDATING:
— skill-json-validate: Check whether a string is valid JSON (returns {valid, error}).

Decision guide:
• Have a structured value, need a field → skill-json-query
• Have a JSON string, need a specific field → skill-json-parse-and-query (Tier-0)
• Need to write JSON to a file or display it → skill-json-stringify
• Have a raw JSON string, need a dict/list → skill-json-parse
• Unsure if a string is valid JSON before parsing → skill-json-validate first

Always validate before parsing when the source is external or user-provided.
pc-json-extract-field is an alternative pure-Python extractor for multi-hop
path resolution when the json tool is not available in the current context.
"#;

const SKILL_SKILLS_BODY: &str = r#"Skill management gives the agent and user visibility and control over the installed
skill library. Use the right grain for each task:

Listing skills:
- skill-skill-list: enumerate the installed skill library (always start here)

Installing a skill:
- skill-skill-install: fetch a manifest from URL/path, confirm with user, enter Q1/Q2

Removing a skill:
- skill-skill-remove: confirm with user, check for dependent recipes, then remove

Safety rules:
- Never install from an untrusted URL without explicit user confirmation.
- Never remove without explicit user confirmation — removal is irreversible.
- System-scope skills cannot be modified from user-scope authority.
- After install, the skill is 'pending' — not usable until Q2 graduates it.
"#;

// ---------------------------------------------------------------------------
// Recipe YAML sources — management group (chunk 7c)
// Transcribed verbatim from builtin_stuff_v3.md (the doc's flat step format).
// 15 Tier-0 (llm_call_required=false) + 2 Tier-1 (skill-install, skill-remove).
// ---------------------------------------------------------------------------

const RECIPE_TIME_NOW_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-time-now>"],
    "label":   "Pre-load ts-time-now ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-time-now>"],
    "label":   "PythonCode calls host.time(operation=now)"
  }
]
"#;

const RECIPE_TIME_NOW_TZ_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-time-now>"],
    "label":   "Pre-load ts-time-now ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-time-now>"],
    "label":   "PythonCode calls host.time(operation=now, timezone=<tz>)"
  }
]
"#;

const RECIPE_TIME_PARSE_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-time-parse>"],
    "label":   "Pre-load ts-time-parse ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-time-parse>"],
    "label":   "PythonCode calls host.time(operation=parse, input)"
  }
]
"#;

const RECIPE_TIME_CONVERT_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-time-convert>"],
    "label":   "Pre-load ts-time-convert ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-time-convert>"],
    "label":   "PythonCode calls host.time(operation=convert, input, to_timezone)"
  }
]
"#;

const RECIPE_TIME_DIFF_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-time-diff>"],
    "label":   "Pre-load ts-time-diff ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-time-diff>"],
    "label":   "PythonCode calls host.time(operation=diff, input, timestamp2)"
  }
]
"#;

const RECIPE_TIME_FORMAT_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-time-format>"],
    "label":   "Pre-load ts-time-format ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-time-format>"],
    "label":   "PythonCode calls host.time(operation=format, input, format_string?)"
  }
]
"#;

const RECIPE_JSON_QUERY_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-json-query>"],
    "label":   "Pre-load ts-json-query ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-json-query>"],
    "label":   "PythonCode calls host.json(operation=query, data, path)"
  }
]
"#;

const RECIPE_JSON_STRINGIFY_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-json-stringify>"],
    "label":   "Pre-load ts-json-stringify ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-json-stringify>"],
    "label":   "PythonCode calls host.json(operation, data)"
  }
]
"#;

const RECIPE_JSON_PARSE_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-json-stringify>"],
    "label":   "Pre-load ts-json-stringify ToolSkill binding (handles parse operation)"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-json-stringify>"],
    "label":   "PythonCode calls host.json(operation='parse', data)"
  }
]
"#;

const RECIPE_JSON_VALIDATE_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-json-validate>"],
    "label":   "Pre-load ts-json-validate ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-json-validate>"],
    "label":   "PythonCode calls host.json(operation='validate', data)"
  }
]
"#;

const RECIPE_JSON_PARSE_AND_QUERY_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-json-validate>"],
    "label":   "Pre-load ts-json-validate ToolSkill binding (validate first)"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-json-validate>"],
    "label":   "PythonCode validates the JSON string before proceeding"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-json-query>"],
    "label":   "Pre-load ts-json-query ToolSkill binding"
  },
  {
    "step_id": "step-4",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-json-query>"],
    "label":   "PythonCode calls host.json(operation='query', path=slot1) on parsed data"
  }
]
"#;

const RECIPE_SKILL_LIST_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-skill-list>"],
    "label":   "Pre-load ts-skill-list ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-skill-list>"],
    "label":   "PythonCode calls host.skill_list(scope)"
  }
]
"#;

const RECIPE_SKILL_LIST_USER_ONLY_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-skill-list>"],
    "label":   "Pre-load ts-skill-list ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-skill-list>"],
    "label":   "PythonCode calls host.skill_list(scope='user')"
  }
]
"#;

const RECIPE_SKILL_LIST_SYSTEM_ONLY_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-skill-list>"],
    "label":   "Pre-load ts-skill-list ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-skill-list>"],
    "label":   "PythonCode calls host.skill_list(scope='system')"
  }
]
"#;

const RECIPE_SKILL_INSTALL_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-skill-install>"],
    "label":   "Load skill-skill-install leaf skill body (install procedure)"
  },
  {
    "step_id": "step-2",
    "type":    "llm",
    "label":   "LLM confirms URL with user, explains pending state, calls ts-skill-install"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-skill-list>", "<uuid:ts-skill-install>"],
    "label":   "Pre-load ToolSkill bindings for list (pre-check) and install"
  }
]
"#;

const RECIPE_SKILL_REMOVE_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:skill-skill-remove>"],
    "label":   "Load skill-skill-remove leaf skill body (removal procedure)"
  },
  {
    "step_id": "step-2",
    "type":    "llm",
    "label":   "LLM confirms skill name, warns about irreversibility, calls ts-skill-remove"
  },
  {
    "step_id": "step-3",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-skill-list>", "<uuid:ts-skill-remove>"],
    "label":   "Pre-load ToolSkill bindings for list (pre-check) and remove"
  }
]
"#;

const RECIPE_ECHO_PING_YAML: &str = r#"step_descriptions: [
  {
    "step_id": "step-1",
    "type":    "component",
    "channel": "rust",
    "include": ["<uuid:ts-echo>"],
    "label":   "Pre-load ts-echo ToolSkill binding"
  },
  {
    "step_id": "step-2",
    "type":    "component",
    "channel": "orchestrator",
    "include": ["<uuid:pc-exec-echo>"],
    "label":   "PythonCode calls host.echo(message) — returned verbatim"
  }
]
"#;
