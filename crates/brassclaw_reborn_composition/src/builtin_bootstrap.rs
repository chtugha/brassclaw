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
        self.python_code.insert(row).await.map_err(map)
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

    // 4. Append the minted tool + toolskill ids to each per-tool catalogue
    //    (PythonCode / Skill / Recipe ids appended in later chunks).
    stores
        .append_children(cat_read_file, &[tool_read_file, ts_read_file])
        .await?;
    stores
        .append_children(cat_write_file, &[tool_write_file, ts_write_file])
        .await?;
    stores
        .append_children(cat_list_dir, &[tool_list_dir, ts_list_dir])
        .await?;
    stores
        .append_children(cat_glob, &[tool_glob, ts_glob])
        .await?;
    stores
        .append_children(cat_grep, &[tool_grep, ts_grep])
        .await?;
    stores
        .append_children(cat_apply_patch, &[tool_apply_patch, ts_apply_patch])
        .await?;

    // 5. Append all filesystem tool + toolskill ids to the primary catalogue.
    stores
        .append_children(
            cat_filesystem,
            &[
                tool_read_file,
                ts_read_file,
                tool_write_file,
                ts_write_file,
                tool_list_dir,
                ts_list_dir,
                tool_glob,
                ts_glob,
                tool_grep,
                ts_grep,
                tool_apply_patch,
                ts_apply_patch,
            ],
        )
        .await?;

    tracing::debug!(catalogue_id = %cat_filesystem, "seeded filesystem group (chunk 1: catalogues + tools + toolskills)");
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
