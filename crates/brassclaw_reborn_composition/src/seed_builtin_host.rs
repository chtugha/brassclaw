//! Phase C.2 — idempotent boot seed of the built-in `host.*` component stack
//! (the Step 27 spec in `builtin_stuff_v3.md`).
//!
//! Runs on every service start (wired in `webui.rs` alongside
//! [`crate::webui::seed_builtin_providers`]). Idempotent: a re-seed leaves
//! existing rows untouched and only inserts what is missing.
//!
//! # Slices shipped in this file
//!
//! - 1d — skeleton + the class-23 `builtin-host` ExtensionCatalogue row.
//! - 2  — Step 27.1 `host.resolve_intent`: the 5-component stack (Tool +
//!   ToolSkill + PythonCode + leaf Skill + Recipe), minted ids appended
//!   to `builtin-host.child_component_ids`.
//! - 3  — Step 27.2 `host.compose_orchestrator`: the 5-component stack. The
//!   Recipe `orchestrator_steps` seeds only the real component
//!   (`pc-host-compose-orchestrator`); the composed program is run at runtime
//!   by the C.5 basic-mode script (not a static step — fork A).
//! - 4  — Step 27.3 `host.post_reply`: the 5-component stack (effect `write`);
//!   the single end-of-turn emit for both Matching- and Non-Matching-Mode.
//! - 5  — Step 27.7.2 `host.fetch_component`: the 4-component stack (Tool +
//!   ToolSkill + PythonCode + leaf Skill — no Recipe; a leaf tool called by
//!   other recipes). SEC-01 validated fetch by UUID + class code.
//!
//! `child_component_ids` is appended to incrementally across slices 2–12 as
//! each `host.*` tool / ToolSkill / PythonCode / leaf-Skill / Recipe id is
//! minted.
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
//! # Idempotency model
//!
//! `tool` / `tool_skill` / `skill` inserts use `ON CONFLICT (scope,name) DO
//! NOTHING` → `Option<Uuid>` (race-free); `None` is resolved via the matching
//! `get_id_by_name`. `python_code` / `recipe` inserts are not ON-CONFLICT, so
//! they use get-then-insert via `get_by_name` (the TOCTOU window is benign — a
//! concurrent insert would surface as a unique violation, retried next boot).
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
use serde_json::json;
use thiserror::Error;
use uuid::Uuid;

use crate::pg_extension_catalogue_store::{NewPgExtensionCatalogue, PgExtensionCatalogueStore};
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

/// The five seed-side stores + the catalogue store, all sharing one pool.
/// Built once per [`seed_builtin_host_components`] call; each slice's
/// `seed_host_*` helper borrows it.
struct HostStores {
    tenant: String,
    tool: PgToolStore,
    tool_skill: PgToolSkillStore,
    python_code: PgPythonCodeStore,
    skill: PgSkillStore,
    recipe: PgRecipeStore,
    catalogue: PgExtensionCatalogueStore,
}

impl HostStores {
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
    async fn upsert_tool(&self, row: NewPgTool, name: &str) -> Result<Uuid, SeedBuiltinHostError> {
        let map = |e: crate::pg_tool_store::PgToolStoreError| SeedBuiltinHostError::Db {
            reason: e.to_string(),
        };
        if let Some(id) = self.tool.insert(row).await.map_err(map)? {
            return Ok(id);
        }
        self.tool
            .get_id_by_name(&self.tenant, SEED_USER, SEED_AGENT, SEED_PROJECT, name)
            .await
            .map_err(map)?
            .ok_or_else(|| SeedBuiltinHostError::Db {
                reason: format!("tool `{name}` insert no-op but not found"),
            })
    }

    /// Insert-or-recover a ToolSkill id (class 13). Same ON-CONFLICT pattern.
    async fn upsert_tool_skill(
        &self,
        row: NewPgToolSkill,
        name: &str,
    ) -> Result<Uuid, SeedBuiltinHostError> {
        let map = |e: crate::pg_tool_skill_store::PgToolSkillStoreError| SeedBuiltinHostError::Db {
            reason: e.to_string(),
        };
        if let Some(id) = self.tool_skill.insert(row).await.map_err(map)? {
            return Ok(id);
        }
        self.tool_skill
            .get_id_by_name(&self.tenant, SEED_USER, SEED_AGENT, SEED_PROJECT, name)
            .await
            .map_err(map)?
            .ok_or_else(|| SeedBuiltinHostError::Db {
                reason: format!("tool_skill `{name}` insert no-op but not found"),
            })
    }

    /// Insert-or-recover a leaf Skill id (class 1). Same ON-CONFLICT pattern.
    async fn upsert_skill(&self, row: NewPgSkill, name: &str) -> Result<Uuid, SeedBuiltinHostError> {
        let map = |e: crate::pg_skill_store::PgSkillStoreError| SeedBuiltinHostError::Db {
            reason: e.to_string(),
        };
        if let Some(id) = self.skill.insert(row).await.map_err(map)? {
            return Ok(id);
        }
        self.skill
            .get_id_by_name(&self.tenant, SEED_USER, SEED_AGENT, SEED_PROJECT, name)
            .await
            .map_err(map)?
            .ok_or_else(|| SeedBuiltinHostError::Db {
                reason: format!("skill `{name}` insert no-op but not found"),
            })
    }

    /// Insert-or-recover a PythonCode id (class 22). `insert` is not
    /// ON-CONFLICT, so get-then-insert via [`PgPythonCodeStore::get_by_name`].
    async fn upsert_python_code(
        &self,
        row: NewPgPythonCode,
        name: &str,
    ) -> Result<Uuid, SeedBuiltinHostError> {
        let map = |e: crate::pg_python_code_store::PgPythonCodeStoreError| SeedBuiltinHostError::Db {
            reason: e.to_string(),
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
    ) -> Result<Uuid, SeedBuiltinHostError> {
        let map = |e: crate::pg_recipe_store::PgRecipeStoreError| SeedBuiltinHostError::Db {
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
}

/// Seed the built-in `host.*` component stack for `tenant_id`.
///
/// Inserts the class-23 `builtin-host` ExtensionCatalogue row (idempotent) then
/// seeds each `host.*` tool's 5-component stack (slices 2–12) and appends the
/// minted ids to `builtin-host.child_component_ids`. All builtins are
/// `source = "system"` + `validation_status = "validated"` (bypassing Q1).
pub(crate) async fn seed_builtin_host_components(
    pool: Arc<PgPool>,
    tenant_id: &str,
) -> Result<(), SeedBuiltinHostError> {
    let stores = HostStores::new(pool, tenant_id);

    // get-or-insert the class-23 `builtin-host` catalogue row → cat_id.
    let cat_id = match stores
        .catalogue
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
    {
        Some(existing) => existing.id,
        None => {
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
                task_groups: json!([]),
                child_component_ids: Vec::new(),
                intent_index: None,
                prior_knowledge_content: None,
                override_prompt_creation: false,
                consumer_tags: vec!["02:orchestrator".into()],
                intent_examples: None,
                source: "system".into(),
                dependency_registry: None,
            };
            let id = stores
                .catalogue
                .insert(row)
                .await
                .map_err(|e| SeedBuiltinHostError::Db {
                    reason: e.to_string(),
                })?;
            // Bypass Q1 pending: graduate the builtin to `validated` directly.
            stores
                .catalogue
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
            id
        }
    };

    // Slice 2 — Step 27.1 host.resolve_intent (5 components).
    let mut child_ids = Vec::new();
    child_ids.extend(seed_host_resolve_intent(&stores).await?);

    // Slice 3 — Step 27.2 host.compose_orchestrator (5 components).
    child_ids.extend(seed_host_compose_orchestrator(&stores).await?);

    // Slice 4 — Step 27.3 host.post_reply (5 components).
    child_ids.extend(seed_host_post_reply(&stores).await?);

    // Slice 5 — Step 27.7.2 host.fetch_component (4 components, no Recipe).
    child_ids.extend(seed_host_fetch_component(&stores).await?);

    // Slice 6 — Step 27.7.3 host.resolve_component_by_name (4 components, no Recipe).
    child_ids.extend(seed_host_resolve_component_by_name(&stores).await?);

    // Slice 7 — Step 27.7.4 host.validate_component (4 components, no Recipe).
    child_ids.extend(seed_host_validate_component(&stores).await?);

    // Slice 8 — Step 27.9.1 host.check_signals (4 components, no Recipe).
    child_ids.extend(seed_host_check_signals(&stores).await?);

    // Slice 9 — Step 27.10.3 host.kohai_complete (4 components, no Recipe).
    // The 8th + last net-new host.* Tool — the Orchestrator→Kohai LLM handoff.
    child_ids.extend(seed_host_kohai_complete(&stores).await?);

    // Slice 10 — Step 27.4 host-save-history (Recipe over builtin.memory_write;
    // 3 components: pc-host-history-format + pc-memory-write + the Recipe).
    child_ids.extend(seed_host_save_history(&stores).await?);

    // Slice 11 — Step 27.10.1 host-assemble-prior-knowledge (fallback Recipe;
    // 2 components: pc-host-fallback-prior-knowledge + the Recipe).
    child_ids.extend(seed_host_assemble_prior_knowledge(&stores).await?);

    // Slice 12 — Step 27.10.2 host-non-match-llm-answer (Non-Matching-Mode Recipe;
    // 2 components: pc-host-assemble-non-match-prompt + the Recipe. Reuses
    // pc-host-kohai-complete seeded in slice 9 via orchestrator_steps + the
    // host.kohai_complete tool via rust_steps.)
    child_ids.extend(seed_host_non_match_llm_answer(&stores).await?);

    // Register the minted component ids on the `builtin-host` catalogue row.
    stores
        .catalogue
        .append_child_component_ids(
            tenant_id,
            SEED_USER,
            SEED_AGENT,
            SEED_PROJECT,
            cat_id,
            &child_ids,
        )
        .await
        .map_err(|e| SeedBuiltinHostError::Db {
            reason: e.to_string(),
        })?;

    tracing::debug!(
        catalogue_id = %cat_id,
        child_components = child_ids.len(),
        "seeded builtin-host stack"
    );
    Ok(())
}

/// Step 27.1 — `host.resolve_intent` (Phase 2 of the basic-mode main process).
///
/// Seeds the 5-component stack idempotently and returns the minted ids in
/// order: `[tool, tool_skill, python_code, skill, recipe]`. Per-component
/// `consumer_tags` (fork-locked): Tool + ToolSkill = `["00:rusty",
/// "02:orchestrator"]`; PythonCode = `["01:monty","02:orchestrator"]`; leaf
/// Skill = `["02:orchestrator","05:validation"]` (the fork-1 distinct tag);
/// Recipe = `["02:orchestrator"]`. None carry `05:validator` (builtins skip
/// Q1). The Recipe `steps` JSON is the fork-4 canonical shape:
/// `{llm_call_required, tier, rust_steps:[{tool,tool_skill}],
/// orchestrator_steps:[{python_code}]}`.
#[allow(clippy::too_many_lines)]
async fn seed_host_resolve_intent(
    stores: &HostStores,
) -> Result<Vec<Uuid>, SeedBuiltinHostError> {
    let tenant = stores.tenant.clone();

    let tool_id = stores
        .upsert_tool(
            NewPgTool {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "host.resolve_intent".to_string(),
                description: "Resolve a user input against the intent system. \
                              Returns a match descriptor {matched, component_id, \
                              intent_id, score, disambiguation}. Phase 2 of the \
                              basic-mode main process."
                    .to_string(),
                param_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "user_input": {"type": "string", "description": "The user's prompt text"},
                        "chat_history": {"type": "array", "description": "Recent turn messages (few tokens)"}
                    },
                    "required": ["user_input"]
                })),
                param_template: Some(json!({"user_input": ""})),
                effect_type: "read".to_string(),
                preconditions: None,
                error_handling: Some(
                    "No match → {matched: false}; never raises.".to_string(),
                ),
                consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
                source: "system".into(),
                validation_status: "validated".into(),
            },
            "host.resolve_intent",
        )
        .await?;

    let tool_skill_id = stores
        .upsert_tool_skill(
            NewPgToolSkill {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "ts-host-resolve-intent".to_string(),
                description: "Resolve user input to a component id via the intent system."
                    .to_string(),
                content: "Call `host.resolve_intent(user_input=<user text>)` at the start \
                          of every turn. Returns {matched, component_id, intent_id, score, \
                          disambiguation}. If matched is true, hand the component_id to \
                          `host.compose_orchestrator` (Matching-Mode). If false, fall through \
                          to the Non-Matching-Mode routine. `matched=false` is a normal result, \
                          not an error."
                    .to_string(),
                prior_knowledge_content: None,
                override_prompt_creation: false,
                tool_name: Some("host.resolve_intent".to_string()),
                param_schema: Some(json!([
                    {"name": "user_input", "param_type": "string", "required": true, "description": "User prompt text"},
                    {"name": "chat_history", "param_type": "array", "required": false, "description": "Recent messages for context"}
                ])),
                param_template: Some(json!({"user_input": "{{user_input}}"})),
                consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
                intent_examples: None,
                source: "system".into(),
                validation_status: "validated".into(),
            },
            "ts-host-resolve-intent",
        )
        .await?;

    let python_code_id = stores
        .upsert_python_code(
            NewPgPythonCode {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "pc-host-resolve-intent".to_string(),
                description: "Orchestrator step: resolve user input to a component id \
                              (Phase 2)."
                    .to_string(),
                content: "# Channel: orchestrator | Class: 22 | No I/O, no imports, no \
                          network, no DB.\n# IBS bakes in {{vars.slotN}} values before \
                          execution.\nresult = host.resolve_intent(user_input=\"{{vars.slot0}}\")\n"
                    .to_string(),
                prior_knowledge_content: None,
                override_prompt_creation: false,
                consumer_tags: vec!["01:monty".into(), "02:orchestrator".into()],
                intent_examples: None,
                source: "system".into(),
                dependency_registry: None,
            },
            "pc-host-resolve-intent",
        )
        .await?;

    let skill_id = stores
        .upsert_skill(
            NewPgSkill {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "skill-host-resolve-intent".to_string(),
                description: "Leaf skill: how to resolve user input to a component id \
                              (Phase 2)."
                    .to_string(),
                body: "Use `ts-host-resolve-intent` at the start of every turn to decide \
                       whether a recipe/instruction matches. Inspect `matched` in the \
                       result. If true, hand the `component_id` to `host.compose_orchestrator` \
                       (Matching-Mode). If false, fall through to the Non-Matching-Mode \
                       routine. Never treat `matched=false` as an error — it means the LLM \
                       path is required."
                    .to_string(),
                class_code: 1,
                consumer_tags: vec!["02:orchestrator".into(), "05:validation".into()],
                intent_examples: json!([]),
                source: "system".into(),
                validation_status: "validated".into(),
            },
            "skill-host-resolve-intent",
        )
        .await?;

    let recipe_id = stores
        .upsert_recipe(
            NewPgRecipe {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "host-resolve-intent".to_string(),
                description: "Resolve user input to a component id (Phase 2). Tier 0 — \
                              no LLM."
                    .to_string(),
                trigger: None,
                steps: json!({
                    "llm_call_required": false,
                    "tier": 0,
                    "rust_steps": [
                        {"tool": "host.resolve_intent", "tool_skill": "ts-host-resolve-intent"}
                    ],
                    "orchestrator_steps": [
                        {"python_code": "pc-host-resolve-intent"}
                    ]
                }),
                prior_knowledge_content: None,
                override_prompt_creation: false,
                consumer_tags: vec!["02:orchestrator".into()],
                intent_examples: Some(json!([
                    "what can you do", "run git status", "read the readme",
                    "search memory for x", "list files", "show me the plan",
                    "grep for foo", "write a file", "parse this json", "what time is it"
                ])),
                source: "system".into(),
                step_descriptions: Some(json!([
                    {"step": 0, "action": "resolve_intent", "desc": "Resolve user input to a component id or no-match."}
                ])),
                variants: None,
                dependency_registry: None,
            },
            "host-resolve-intent",
        )
        .await?;

    Ok(vec![
        tool_id,
        tool_skill_id,
        python_code_id,
        skill_id,
        recipe_id,
    ])
}

/// Step 27.2 — `host.compose_orchestrator` (Phase 3 — Matching-Mode compose).
///
/// Fetch + split + assemble a matched recipe into a ready-to-run orchestrator
/// program (+ rust inputs + tier). Seeds the 5-component stack idempotently
/// and returns the minted ids in `[tool, tool_skill, python_code, skill,
/// recipe]` order. The Recipe `orchestrator_steps` seeds only the real
/// component (`pc-host-compose-orchestrator`) — the composed program is run at
/// runtime by the C.5 basic-mode script (fork A); `step_descriptions` keeps
/// both the compose + run steps as documentation.
#[allow(clippy::too_many_lines)]
async fn seed_host_compose_orchestrator(
    stores: &HostStores,
) -> Result<Vec<Uuid>, SeedBuiltinHostError> {
    let tenant = stores.tenant.clone();

    let tool_id = stores
        .upsert_tool(
            NewPgTool {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "host.compose_orchestrator".to_string(),
                description: "Fetch + split + assemble a recipe by component id. \
                              Returns the ready-to-run orchestrator program + rust \
                              inputs + tier. Monty runs the program; Rust does not \
                              sequence steps."
                    .to_string(),
                param_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "component_id": {"type": "string", "description": "UUID or name of the matched recipe/instruction"},
                        "class_code": {"type": "integer", "description": "Component class (21 recipe, 11 action, …)"}
                    },
                    "required": ["component_id"]
                })),
                param_template: Some(json!({"component_id": ""})),
                effect_type: "read".to_string(),
                preconditions: Some(
                    "Composition recipe store + rust/orchestrator splitter wired.".to_string(),
                ),
                error_handling: Some(
                    "Miss/parse failure → {orchestrator_program: null}; caller degrades to \
                     Non-Matching-Mode."
                        .to_string(),
                ),
                consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
                source: "system".into(),
                validation_status: "validated".into(),
            },
            "host.compose_orchestrator",
        )
        .await?;

    let tool_skill_id = stores
        .upsert_tool_skill(
            NewPgToolSkill {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "ts-host-compose-orchestrator".to_string(),
                description: "Compose the orchestrator program for a matched component id."
                    .to_string(),
                content: "After `host.resolve_intent` returns a match, call \
                          `host.compose_orchestrator(component_id=<matched id>)`. The host fetches \
                          the recipe, splits the rust vs orchestrator parts, loads rust bindings, \
                          and hands back `{orchestrator_program, rust_inputs, recipe_hint, tier}`. \
                          Run the returned `orchestrator_program` directly — do NOT re-sequence \
                          its steps from Rust. If `orchestrator_program` is null, degrade to the \
                          Non-Matching-Mode routine."
                    .to_string(),
                prior_knowledge_content: None,
                override_prompt_creation: false,
                tool_name: Some("host.compose_orchestrator".to_string()),
                param_schema: Some(json!([
                    {"name": "component_id", "param_type": "string", "required": true, "description": "Matched component UUID or name"},
                    {"name": "class_code", "param_type": "number", "required": false, "description": "Component class code (default 21)"}
                ])),
                param_template: Some(json!({"component_id": "{{component_id}}"})),
                consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
                intent_examples: None,
                source: "system".into(),
                validation_status: "validated".into(),
            },
            "ts-host-compose-orchestrator",
        )
        .await?;

    let python_code_id = stores
        .upsert_python_code(
            NewPgPythonCode {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "pc-host-compose-orchestrator".to_string(),
                description: "Orchestrator step: fetch+split+assemble the matched recipe \
                              into a runnable program."
                    .to_string(),
                content: "# Channel: orchestrator | Class: 22 | No I/O, no imports.\n\
                          # Fetch+split+assemble the matched recipe into a runnable program.\n\
                          composed = host.compose_orchestrator(component_id=\"{{vars.slot0}}\")\n\
                          # composed = {orchestrator_program, rust_inputs, recipe_hint, tier}\n"
                    .to_string(),
                prior_knowledge_content: None,
                override_prompt_creation: false,
                consumer_tags: vec!["01:monty".into(), "02:orchestrator".into()],
                intent_examples: None,
                source: "system".into(),
                dependency_registry: None,
            },
            "pc-host-compose-orchestrator",
        )
        .await?;

    let skill_id = stores
        .upsert_skill(
            NewPgSkill {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "skill-host-compose-orchestrator".to_string(),
                description: "Leaf skill: how to compose a matched recipe into a runnable \
                              program."
                    .to_string(),
                body: "After `host.resolve_intent` returns a match, call \
                       `ts-host-compose-orchestrator` with the component_id. The host fetches \
                       the recipe, splits the rust vs orchestrator parts, and hands back a \
                       ready-to-run orchestrator program plus rust inputs and a tier hint. Run \
                       the returned program directly — do NOT re-sequence its steps from Rust. \
                       If `orchestrator_program` is null, degrade to the Non-Matching-Mode \
                       routine."
                    .to_string(),
                class_code: 1,
                consumer_tags: vec!["02:orchestrator".into(), "05:validation".into()],
                intent_examples: json!([]),
                source: "system".into(),
                validation_status: "validated".into(),
            },
            "skill-host-compose-orchestrator",
        )
        .await?;

    let recipe_id = stores
        .upsert_recipe(
            NewPgRecipe {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "host-compose-and-run-orchestrator".to_string(),
                description: "Matching-Mode: compose the matched recipe then run its \
                              orchestrator program. Tier depends on the composed recipe \
                              (0 or 1)."
                    .to_string(),
                trigger: None,
                steps: json!({
                    "llm_call_required": false,
                    "tier": 0,
                    "rust_steps": [
                        {"tool": "host.compose_orchestrator", "tool_skill": "ts-host-compose-orchestrator"}
                    ],
                    "orchestrator_steps": [
                        {"python_code": "pc-host-compose-orchestrator"}
                    ]
                }),
                prior_knowledge_content: None,
                override_prompt_creation: false,
                consumer_tags: vec!["02:orchestrator".into()],
                intent_examples: Some(json!([
                    "(internal Matching-Mode driver — not user-routed)"
                ])),
                source: "system".into(),
                step_descriptions: Some(json!([
                    {"step": 0, "action": "compose", "desc": "Fetch+split+assemble the matched recipe."},
                    {"step": 1, "action": "run", "desc": "Run the assembled orchestrator program (Monty, one continuous program)."}
                ])),
                variants: None,
                dependency_registry: None,
            },
            "host-compose-and-run-orchestrator",
        )
        .await?;

    Ok(vec![
        tool_id,
        tool_skill_id,
        python_code_id,
        skill_id,
        recipe_id,
    ])
}

/// Step 27.3 — `host.post_reply` (Phase 3 — answer post; effect `write`).
///
/// The single end-of-turn emit for both Matching- and Non-Matching-Mode: posts
/// the final answer text into the user chat. Seeds the 5-component stack
/// idempotently and returns the minted ids in `[tool, tool_skill,
/// python_code, skill, recipe]` order.
#[allow(clippy::too_many_lines)]
async fn seed_host_post_reply(
    stores: &HostStores,
) -> Result<Vec<Uuid>, SeedBuiltinHostError> {
    let tenant = stores.tenant.clone();

    let tool_id = stores
        .upsert_tool(
            NewPgTool {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "host.post_reply".to_string(),
                description: "Post the final answer text into the user chat. End-of-turn \
                              emit for both Matching- and Non-Matching-Mode."
                    .to_string(),
                param_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "answer": {"type": "string", "description": "The final answer to post"}
                    },
                    "required": ["answer"]
                })),
                param_template: Some(json!({"answer": ""})),
                effect_type: "write".to_string(),
                preconditions: Some("Active chat session.".to_string()),
                error_handling: Some("Post failure → raise; caller retries.".to_string()),
                consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
                source: "system".into(),
                validation_status: "validated".into(),
            },
            "host.post_reply",
        )
        .await?;

    let tool_skill_id = stores
        .upsert_tool_skill(
            NewPgToolSkill {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "ts-host-post-reply".to_string(),
                description: "Post the final answer into the user chat.".to_string(),
                content: "Call `host.post_reply(answer=<final answer text>)` once after the \
                          turn's work is complete. This is the single end-of-turn emit for \
                          both Matching- and Non-Matching-Mode. After posting, call the \
                          `host-save-history` recipe so kohai/sempai can mint new components. \
                          Raises on post failure; the caller retries."
                    .to_string(),
                prior_knowledge_content: None,
                override_prompt_creation: false,
                tool_name: Some("host.post_reply".to_string()),
                param_schema: Some(json!([
                    {"name": "answer", "param_type": "string", "required": true, "description": "Final answer text"}
                ])),
                param_template: Some(json!({"answer": "{{answer}}"})),
                consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
                intent_examples: None,
                source: "system".into(),
                validation_status: "validated".into(),
            },
            "ts-host-post-reply",
        )
        .await?;

    let python_code_id = stores
        .upsert_python_code(
            NewPgPythonCode {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "pc-host-post-reply".to_string(),
                description: "Orchestrator step: post the final answer into the user chat."
                    .to_string(),
                content: "# Channel: orchestrator | Class: 22 | No I/O, no imports.\n\
                          host.post_reply(answer=\"{{vars.slot0}}\")\n"
                    .to_string(),
                prior_knowledge_content: None,
                override_prompt_creation: false,
                consumer_tags: vec!["01:monty".into(), "02:orchestrator".into()],
                intent_examples: None,
                source: "system".into(),
                dependency_registry: None,
            },
            "pc-host-post-reply",
        )
        .await?;

    let skill_id = stores
        .upsert_skill(
            NewPgSkill {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "skill-host-post-reply".to_string(),
                description: "Leaf skill: how to post the final answer into the chat."
                    .to_string(),
                body: "Call `ts-host-post-reply` once with the final answer text after the \
                       turn's work is complete. This is the single end-of-turn emit for both \
                       modes. After posting, call the `host-save-history` recipe so \
                       kohai/sempai can mint new components."
                    .to_string(),
                class_code: 1,
                consumer_tags: vec!["02:orchestrator".into(), "05:validation".into()],
                intent_examples: json!([]),
                source: "system".into(),
                validation_status: "validated".into(),
            },
            "skill-host-post-reply",
        )
        .await?;

    let recipe_id = stores
        .upsert_recipe(
            NewPgRecipe {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "host-post-reply".to_string(),
                description: "Post the final answer into the user chat. Tier 0 — no LLM."
                    .to_string(),
                trigger: None,
                steps: json!({
                    "llm_call_required": false,
                    "tier": 0,
                    "rust_steps": [
                        {"tool": "host.post_reply", "tool_skill": "ts-host-post-reply"}
                    ],
                    "orchestrator_steps": [
                        {"python_code": "pc-host-post-reply"}
                    ]
                }),
                prior_knowledge_content: None,
                override_prompt_creation: false,
                consumer_tags: vec!["02:orchestrator".into()],
                intent_examples: Some(json!([
                    "(internal end-of-turn emit — not user-routed)"
                ])),
                source: "system".into(),
                step_descriptions: Some(json!([
                    {"step": 0, "action": "post_reply", "desc": "Post the final answer."}
                ])),
                variants: None,
                dependency_registry: None,
            },
            "host-post-reply",
        )
        .await?;

    Ok(vec![
        tool_id,
        tool_skill_id,
        python_code_id,
        skill_id,
        recipe_id,
    ])
}

/// Step 27.7.2 — `host.fetch_component` (SEC-01 validated fetch by UUID).
///
/// A 4-component leaf-tool stack (Tool + ToolSkill + PythonCode + leaf Skill —
/// no Recipe; called by other recipes' `rust_steps` by name). Returns the
/// minted ids in `[tool, tool_skill, python_code, skill]` order. The Tool row
/// carries `["00:rusty","02:orchestrator"]` (discoverable); the leaf skill
/// carries the fork-1 `05:validation` tag (retrievable validated-builtin
/// marker).
#[allow(clippy::too_many_lines)]
async fn seed_host_fetch_component(
    stores: &HostStores,
) -> Result<Vec<Uuid>, SeedBuiltinHostError> {
    let tenant = stores.tenant.clone();

    let tool_id = stores
        .upsert_tool(
            NewPgTool {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "host.fetch_component".to_string(),
                description: "Fetch a single validated component by UUID + class code \
                              (SEC-01 gate). Returns {id,class_code,name,description,\
                              content,override_prompt_creation,steps?,allowed_tools?} \
                              or null."
                    .to_string(),
                param_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "uuid": {"type": "string", "description": "Component UUID"},
                        "class_code": {"type": "integer", "description": "Component class code"}
                    },
                    "required": ["uuid", "class_code"]
                })),
                param_template: Some(json!({"uuid": "", "class_code": 0})),
                effect_type: "read".to_string(),
                preconditions: Some(
                    "skills-db pool wired (returns null without it).".to_string(),
                ),
                error_handling: Some(
                    "Missing/invalid/absent → null; never raises.".to_string(),
                ),
                consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
                source: "system".into(),
                validation_status: "validated".into(),
            },
            "host.fetch_component",
        )
        .await?;

    let tool_skill_id = stores
        .upsert_tool_skill(
            NewPgToolSkill {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "ts-host-fetch-component".to_string(),
                description: "Fetch a validated component by UUID + class code (SEC-01 gate)."
                    .to_string(),
                content: "Call `host.fetch_component(uuid=<UUID>, class_code=<int>)` to fetch \
                          a single validated component. Returns {id,class_code,name,description,\
                          content,override_prompt_creation,steps?,allowed_tools?} or null. A \
                          null result means the component is absent or not validated — do not \
                          invent one."
                    .to_string(),
                prior_knowledge_content: None,
                override_prompt_creation: false,
                tool_name: Some("host.fetch_component".to_string()),
                param_schema: Some(json!([
                    {"name": "uuid", "param_type": "string", "required": true, "description": "Component UUID"},
                    {"name": "class_code", "param_type": "number", "required": true, "description": "Component class code"}
                ])),
                param_template: Some(json!({"uuid": "{{uuid}}", "class_code": "{{class_code}}"})),
                consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
                intent_examples: None,
                source: "system".into(),
                validation_status: "validated".into(),
            },
            "ts-host-fetch-component",
        )
        .await?;

    let python_code_id = stores
        .upsert_python_code(
            NewPgPythonCode {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "pc-host-fetch-component".to_string(),
                description: "Orchestrator step: fetch a validated component by UUID + \
                              class code (§0.9 Option A)."
                    .to_string(),
                content: "# Channel: orchestrator | Class: 22 | No I/O, no imports.\n\
                          comp = host.fetch_component(uuid=\"{{vars.slot0}}\", class_code={{vars.slot1}})\n"
                    .to_string(),
                prior_knowledge_content: None,
                override_prompt_creation: false,
                consumer_tags: vec!["01:monty".into(), "02:orchestrator".into()],
                intent_examples: None,
                source: "system".into(),
                dependency_registry: None,
            },
            "pc-host-fetch-component",
        )
        .await?;

    let skill_id = stores
        .upsert_skill(
            NewPgSkill {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "skill-host-fetch-component".to_string(),
                description: "Fetch a validated component by UUID for nested call_action \
                              lookups (§0.9 Option A)."
                    .to_string(),
                body: "Use `ts-host-fetch-component` when you hold a component UUID + class \
                       code. A null result means the component is absent or not validated — \
                       do not invent one."
                    .to_string(),
                class_code: 1,
                consumer_tags: vec!["02:orchestrator".into(), "05:validation".into()],
                intent_examples: json!([]),
                source: "system".into(),
                validation_status: "validated".into(),
            },
            "skill-host-fetch-component",
        )
        .await?;

    Ok(vec![tool_id, tool_skill_id, python_code_id, skill_id])
}

/// Step 27.10.3 — `host.kohai_complete` (Orchestrator→Kohai LLM handoff Tool).
///
/// A 4-component leaf-tool stack (Tool + ToolSkill + PythonCode + leaf Skill —
/// no Recipe; the Recipe that composes it is 27.10.2 `host-non-match-llm-answer`,
/// seeded in slice 12). The 8th + LAST net-new `host.*` Tool. Wraps the existing
/// `brassclaw_interceptor` ingress — wiring only, no new logic. This is the LLM
/// surface: Rust↔LLM never communicate directly; the Orchestrator composes the
/// prompt + prefix-placeholder → Kohai saves → optional Sempai optimize → Kohai
/// adds the provider prefix → Kohai calls `first_party_tools/http` → Kohai saves
/// the answer → answer back to the Orchestrator.
#[allow(clippy::too_many_lines)]
async fn seed_host_kohai_complete(
    stores: &HostStores,
) -> Result<Vec<Uuid>, SeedBuiltinHostError> {
    let tenant = stores.tenant.clone();

    let tool_id = stores
        .upsert_tool(
            NewPgTool {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "host.kohai_complete".to_string(),
                description: "Hand an assembled LLM prompt (with a prefix-placeholder) to \
                              Kohai. Kohai saves the prompt; if a Sempai is connected, adds \
                              an optimization-prefix → Sempai optimizes → returns without \
                              prefix → Kohai saves the optimized prompt beside the original; \
                              Kohai adds the provider-LLM prefix for that placeholder and \
                              sends the prompt to the provider LLM by calling \
                              first_party_tools/http; receives the answer, saves it beside \
                              its prompt, and returns it. Wraps the existing \
                              brassclaw_interceptor ingress — no new logic, wiring only."
                    .to_string(),
                param_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "prompt": {"type": "object", "description": "Assembled prompt {chat_history, user_query, prefix_placeholder}"}
                    },
                    "required": ["prompt"]
                })),
                param_template: Some(json!({"prompt": {}})),
                effect_type: "write".to_string(),
                preconditions: Some(
                    "Interceptor (Kohai) ingress wired; provider-LLM prefix chunk \
                     precompiled for the placeholder."
                        .to_string(),
                ),
                error_handling: Some(
                    "Provider/HTTP failure → raises; Orchestrator catches and surfaces \
                     via post_reply."
                        .to_string(),
                ),
                consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
                source: "system".into(),
                validation_status: "validated".into(),
            },
            "host.kohai_complete",
        )
        .await?;

    let tool_skill_id = stores
        .upsert_tool_skill(
            NewPgToolSkill {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "ts-host-kohai-complete".to_string(),
                description: "Hand an assembled prompt to Kohai and await the provider-LLM \
                              answer."
                    .to_string(),
                content: "Call `host.kohai_complete(prompt=<assembled prompt>)` with the \
                          assembled prompt (chat history + user query + a prefix-placeholder). \
                          Kohai saves it, optionally Sempai-optimizes it, swaps the placeholder \
                          for the provider prefix, calls the provider LLM via \
                          first_party_tools/http, saves the answer, and returns it. Use this \
                          for every Orchestrator-side LLM call — Rust never talks to the LLM \
                          directly."
                    .to_string(),
                prior_knowledge_content: None,
                override_prompt_creation: false,
                tool_name: Some("host.kohai_complete".to_string()),
                param_schema: Some(json!([
                    {"name": "prompt", "param_type": "object", "required": true, "description": "Assembled prompt {chat_history, user_query, prefix_placeholder}"}
                ])),
                param_template: Some(json!({"prompt": "{{prompt}}"})),
                consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
                intent_examples: None,
                source: "system".into(),
                validation_status: "validated".into(),
            },
            "ts-host-kohai-complete",
        )
        .await?;

    let python_code_id = stores
        .upsert_python_code(
            NewPgPythonCode {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "pc-host-kohai-complete".to_string(),
                description: "Orchestrator→Kohai handoff: hand the assembled prompt to Kohai \
                              and await the provider-LLM answer."
                    .to_string(),
                // `prompt` is the in-scope variable holding the prior assembler
                // step's result (Option 2 — one continuous program).
                content: "# Channel: orchestrator | Class: 22 | No I/O, no imports.\n\
                          answer = host.kohai_complete(prompt=prompt)\n"
                    .to_string(),
                prior_knowledge_content: None,
                override_prompt_creation: false,
                consumer_tags: vec!["01:monty".into(), "02:orchestrator".into()],
                intent_examples: None,
                source: "system".into(),
                dependency_registry: None,
            },
            "pc-host-kohai-complete",
        )
        .await?;

    let skill_id = stores
        .upsert_skill(
            NewPgSkill {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "skill-host-kohai-complete".to_string(),
                description: "Hand an assembled prompt to Kohai and await the provider-LLM \
                              answer."
                    .to_string(),
                body: "Call `host.kohai_complete` with the assembled prompt (chat history + \
                       user query + a prefix-placeholder). Kohai saves it, optionally \
                       Sempai-optimizes it, swaps the placeholder for the provider prefix, \
                       calls the provider LLM via first_party_tools/http, saves the answer, \
                       and returns it. Use this for every Orchestrator-side LLM call — Rust \
                       never talks to the LLM directly."
                    .to_string(),
                class_code: 1,
                consumer_tags: vec!["02:orchestrator".into(), "05:validation".into()],
                intent_examples: json!([]),
                source: "system".into(),
                validation_status: "validated".into(),
            },
            "skill-host-kohai-complete",
        )
        .await?;

    Ok(vec![tool_id, tool_skill_id, python_code_id, skill_id])
}

/// Step 27.4 — `host-save-history` (Recipe over `builtin.memory_write`).
///
/// A Recipe-only component set (no new Tool): one PythonCode formatter
/// (`pc-host-history-format`) + a NEW `pc-memory-write` first-class-callable
/// step + the `host-save-history` Recipe (Tier 0). Reuses Step 11's
/// `builtin.memory_write` tool (`memory_write` + `ts-memory-write`) via the
/// Recipe's `rust_steps` binding — fork-B (user-locked): the 27.4 spec
/// referenced `pc-memory-write`, which was never defined (Step 11's PythonCode
/// is the stale `pc-exec-memory-write` that calls the RETIRED
/// `__execute_action__`), so a NEW `pc-memory-write` is seeded here as the
/// new-architecture first-class callable to the `memory_write` tool. Returns
/// the minted ids in `[pc-host-history-format, pc-memory-write, recipe]` order.
#[allow(clippy::too_many_lines)]
async fn seed_host_save_history(
    stores: &HostStores,
) -> Result<Vec<Uuid>, SeedBuiltinHostError> {
    let tenant = stores.tenant.clone();

    let pc_history_format_id = stores
        .upsert_python_code(
            NewPgPythonCode {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "pc-host-history-format".to_string(),
                description: "Orchestrator step: format a structured turn-summary \
                              doc body for the daily memory log."
                    .to_string(),
                content: r###"# Channel: orchestrator | Class: 22 | No I/O, no imports, no network, no DB.
# Compose a structured turn-summary doc body from slot vars.
summary = {
  "user_input": "{{vars.slot0}}",
  "answer": "{{vars.slot1}}",
  "mode": "{{vars.slot2}}",
  "matched_component": "{{vars.slot3}}",
  "timestamp": "{{vars.slot4}}"
}
body = "## Turn summary\n"
for k, v in summary.items():
    body += f"- **{k}**: {v}\n"
# handed to the following memory_write step
"###
                .to_string(),
                prior_knowledge_content: None,
                override_prompt_creation: false,
                consumer_tags: vec!["01:monty".into(), "02:orchestrator".into()],
                intent_examples: None,
                source: "system".into(),
                dependency_registry: None,
            },
            "pc-host-history-format",
        )
        .await?;

    let pc_memory_write_id = stores
        .upsert_python_code(
            NewPgPythonCode {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "pc-memory-write".to_string(),
                description: "Orchestrator step: append the formatted turn-summary \
                              body to the daily memory log via the memory_write tool \
                              (new-arch first-class callable — replaces the stale \
                              pc-exec-memory-write that called the RETIRED \
                              __execute_action__)."
                    .to_string(),
                // `body` is the in-scope variable holding the prior
                // pc-host-history-format step's result (Option 2 — one
                // continuous program). The memory_write tool is pre-bound into
                // the host namespace by the Recipe's rust_steps.
                content: "# Channel: orchestrator | Class: 22 | No I/O, no imports.\n\
                          # body = the formatted turn-summary from the prior step (in scope).\n\
                          host.memory_write(content=body, target=\"daily_log\")\n"
                    .to_string(),
                prior_knowledge_content: None,
                override_prompt_creation: false,
                consumer_tags: vec!["01:monty".into(), "02:orchestrator".into()],
                intent_examples: None,
                source: "system".into(),
                dependency_registry: None,
            },
            "pc-memory-write",
        )
        .await?;

    let recipe_id = stores
        .upsert_recipe(
            NewPgRecipe {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "host-save-history".to_string(),
                description: "Save a structured turn summary to the daily memory log \
                              for kohai/sempai. Tier 0 — no LLM. Reuses \
                              builtin.memory_write (Step 11)."
                    .to_string(),
                trigger: None,
                steps: json!({
                    "llm_call_required": false,
                    "tier": 0,
                    "rust_steps": [
                        {"tool": "memory_write", "tool_skill": "ts-memory-write"}
                    ],
                    "orchestrator_steps": [
                        {"python_code": "pc-host-history-format"},
                        {"python_code": "pc-memory-write"}
                    ]
                }),
                prior_knowledge_content: None,
                override_prompt_creation: false,
                consumer_tags: vec!["02:orchestrator".into()],
                intent_examples: Some(json!([
                    "(internal end-of-turn history save — not user-routed)"
                ])),
                source: "system".into(),
                step_descriptions: Some(json!([
                    {"step": 0, "action": "format", "desc": "Format the turn summary body."},
                    {"step": 1, "action": "memory_write", "desc": "Append the summary to the daily memory log."}
                ])),
                variants: None,
                dependency_registry: None,
            },
            "host-save-history",
        )
        .await?;

    Ok(vec![pc_history_format_id, pc_memory_write_id, recipe_id])
}

/// Step 27.10.1 — `host-assemble-prior-knowledge` (fallback Recipe, Tier 1).
///
/// A Recipe-only component set (no new Tool): one PythonCode formatter
/// (`pc-host-fallback-prior-knowledge`) + the `host-assemble-prior-knowledge`
/// Recipe (Tier 1). Used ONLY when no prefix is present — adds basic "what is
/// going on" context so the LLM understands the run. Calls NO retrieval verbs
/// (`retrieve_docs` / `get_reduction_rules` are dropped). The spec's
/// `rust_steps: []` is widened to bind the existing `builtin.time` tool
/// (`time` + `ts-time-now`, Step 14) so the formatter can stamp `assembled_at`
/// — the spec's `__now()` was a phantom (exists nowhere); per user direction the
/// existing time tool/skill is reused (the stale `pc-exec-time-now` that calls
/// the RETIRED `__execute_action__` is NOT referenced — the formatter calls
/// `host.time(operation="now")` as a new-arch first-class callable, consistent
/// with slice 10's `host.memory_write`). Returns the minted ids in
/// `[pc-host-fallback-prior-knowledge, recipe]` order.
#[allow(clippy::too_many_lines)]
async fn seed_host_assemble_prior_knowledge(
    stores: &HostStores,
) -> Result<Vec<Uuid>, SeedBuiltinHostError> {
    let tenant = stores.tenant.clone();

    let pc_fallback_id = stores
        .upsert_python_code(
            NewPgPythonCode {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "pc-host-fallback-prior-knowledge".to_string(),
                description: "Orchestrator step: FALLBACK prior-knowledge bundle, used \
                              ONLY when no prefix is present. Adds basic 'what is going \
                              on' context. Calls NO retrieval verbs."
                    .to_string(),
                content: r###"# Channel: orchestrator | Class: 22 | No I/O, no imports.
# FALLBACK prior-knowledge bundle (used ONLY when no prefix is present).
# Calls NO retrieval verbs (retrieve_docs / get_reduction_rules are dropped).
user_query = "{{vars.slot0}}"
_now = host.time(operation="now")
bundle = {
  "context": "You are running inside BrassClaw's orchestrator. Answer the user's request.",
  "user_query": user_query,
  "assembled_at": _now
}
"###
                .to_string(),
                prior_knowledge_content: None,
                override_prompt_creation: false,
                consumer_tags: vec!["01:monty".into(), "02:orchestrator".into()],
                intent_examples: None,
                source: "system".into(),
                dependency_registry: None,
            },
            "pc-host-fallback-prior-knowledge",
        )
        .await?;

    let recipe_id = stores
        .upsert_recipe(
            NewPgRecipe {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "host-assemble-prior-knowledge".to_string(),
                description: "FALLBACK prior-knowledge bundle, used ONLY when no prefix \
                              is present. Adds basic 'what is going on' context so the \
                              LLM understands the run. Calls NO retrieval verbs. \
                              Recipe-only — one PythonCode formatter; binds builtin.time \
                              to stamp assembled_at."
                    .to_string(),
                trigger: None,
                steps: json!({
                    "llm_call_required": false,
                    "tier": 1,
                    "rust_steps": [
                        {"tool": "time", "tool_skill": "ts-time-now"}
                    ],
                    "orchestrator_steps": [
                        {"python_code": "pc-host-fallback-prior-knowledge"}
                    ]
                }),
                prior_knowledge_content: None,
                override_prompt_creation: false,
                consumer_tags: vec!["02:orchestrator".into()],
                intent_examples: Some(json!([
                    "(internal Tier-1 prior-knowledge fallback — not user-routed)"
                ])),
                source: "system".into(),
                step_descriptions: Some(json!([
                    {"step": 0, "action": "fallback_context", "desc": "Add basic 'what is going on' context so the LLM understands (no retrieval)."}
                ])),
                variants: None,
                dependency_registry: None,
            },
            "host-assemble-prior-knowledge",
        )
        .await?;

    Ok(vec![pc_fallback_id, recipe_id])
}

/// Step 27.10.2 — `host-non-match-llm-answer` (Non-Matching-Mode Recipe, Tier 2).
///
/// A Recipe-only component set (no new Tool): one NEW PythonCode assembler
/// (`pc-host-assemble-non-match-prompt`) + the `host-non-match-llm-answer`
/// Recipe (Tier 2). Reuses `pc-host-kohai-complete` (seeded in slice 9) via
/// `orchestrator_steps` and binds the existing `host.kohai_complete` tool
/// (`host.kohai_complete` + `ts-host-kohai-complete`) via `rust_steps`. NO
/// `host.llm_complete` — Rust↔LLM never talk directly; the Orchestrator
/// assembles the prompt (chat history + user question + a prefix-PLACEHOLDER)
/// and hands it to Kohai, which does the save / optional-Sempai /
/// provider-prefix / `first_party_tools/http` / save-answer dance and returns
/// the answer. Recipe-driven so prompt additions / prefixes / query-type
/// routing evolve with no code changes. Returns the minted ids in
/// `[pc-host-assemble-non-match-prompt, recipe]` order.
#[allow(clippy::too_many_lines)]
async fn seed_host_non_match_llm_answer(
    stores: &HostStores,
) -> Result<Vec<Uuid>, SeedBuiltinHostError> {
    let tenant = stores.tenant.clone();

    let pc_assemble_id = stores
        .upsert_python_code(
            NewPgPythonCode {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "pc-host-assemble-non-match-prompt".to_string(),
                description: "Orchestrator step: assemble the Non-Matching-Mode prompt \
                              (chat history + user question + a prefix-PLACEHOLDER). \
                              Pure-logic assembler — no host call; Kohai swaps the \
                              placeholder for the provider prefix last."
                    .to_string(),
                // Multi-line indented Python — raw string preserves the `\n`
                // and the literal dict indentation (not `\`-continuation, which
                // would strip leading whitespace). IBS binds chat_history /
                // user_query / placeholder into slots 0 / 1 / 2.
                content: r###"# Channel: orchestrator | Class: 22 | No I/O, no imports.
# Non-Matching-Mode prompt assembly (Kohai swaps the placeholder last).
chat_history = "{{vars.slot0}}"
user_query   = "{{vars.slot1}}"
placeholder  = "{{vars.slot2}}"
prompt = {
  "chat_history": chat_history,
  "user_query": user_query,
  "prefix_placeholder": placeholder
}
"###
                .to_string(),
                prior_knowledge_content: None,
                override_prompt_creation: false,
                consumer_tags: vec!["01:monty".into(), "02:orchestrator".into()],
                intent_examples: None,
                source: "system".into(),
                dependency_registry: None,
            },
            "pc-host-assemble-non-match-prompt",
        )
        .await?;

    let recipe_id = stores
        .upsert_recipe(
            NewPgRecipe {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "host-non-match-llm-answer".to_string(),
                description: "Non-Matching-Mode (Tier 2): no intent matched. The \
                              Orchestrator assembles the standard prompt (chat history \
                              + user question + a prefix-PLACEHOLDER) and hands it to \
                              Kohai. Kohai saves the prompt; if a Sempai is connected, \
                              adds an optimization-prefix → Sempai optimizes → returns \
                              without prefix → Kohai saves the optimized prompt beside \
                              the original; Kohai adds the provider-LLM prefix for that \
                              placeholder and sends the prompt to the provider LLM by \
                              calling first_party_tools/http; Kohai receives the answer, \
                              saves it beside its prompt, and returns it. NO \
                              host.llm_complete — Rust↔LLM never talk directly."
                    .to_string(),
                trigger: None,
                steps: json!({
                    "llm_call_required": true,
                    "tier": 2,
                    "rust_steps": [
                        {"tool": "host.kohai_complete", "tool_skill": "ts-host-kohai-complete"}
                    ],
                    "orchestrator_steps": [
                        {"python_code": "pc-host-assemble-non-match-prompt"},
                        {"python_code": "pc-host-kohai-complete"}
                    ]
                }),
                prior_knowledge_content: None,
                override_prompt_creation: false,
                consumer_tags: vec!["02:orchestrator".into()],
                intent_examples: Some(json!([
                    "(internal Non-Matching-Mode fallback — not user-routed)"
                ])),
                source: "system".into(),
                step_descriptions: Some(json!([
                    {"step": 0, "action": "assemble_prompt", "desc": "Assemble chat history + user question + a prefix-PLACEHOLDER into the prompt (kohai swaps the placeholder for the provider prefix last)."},
                    {"step": 1, "action": "kohai_complete", "desc": "Hand the assembled prompt to Kohai via host.kohai_complete; Kohai saves, optional Sempai optimize, adds provider prefix, calls first_party_tools/http, saves the answer, and returns it."}
                ])),
                variants: None,
                dependency_registry: None,
            },
            "host-non-match-llm-answer",
        )
        .await?;

    Ok(vec![pc_assemble_id, recipe_id])
}

/// Step 27.9.1 — `host.check_signals` (stop/suspend/inject poll).
///
/// A 4-component leaf-tool stack (Tool + ToolSkill + PythonCode + leaf Skill —
/// no Recipe). The ONLY surviving VM-control verb: external signals
/// (stop/suspend/inject) arrive asynchronously from outside the Orchestrator's
/// own step sequence, so a poll verb is needed. The five other VM-control verbs
/// (emit_event, save_checkpoint, transition_to, check_budget, log_budget_warning)
/// are RETIRED (Q-D) — the Orchestrator owns its own thread/run state.
#[allow(clippy::too_many_lines)]
async fn seed_host_check_signals(
    stores: &HostStores,
) -> Result<Vec<Uuid>, SeedBuiltinHostError> {
    let tenant = stores.tenant.clone();

    let tool_id = stores
        .upsert_tool(
            NewPgTool {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "host.check_signals".to_string(),
                description: "Poll the thread signal channel. Returns 'stop' on \
                              stop/suspend, {inject: msg} on an injected message, or \
                              None when clear."
                    .to_string(),
                param_schema: Some(json!({"type": "object", "properties": {}, "required": []})),
                param_template: Some(json!({})),
                effect_type: "read".to_string(),
                preconditions: Some("Signal receiver wired.".to_string()),
                error_handling: Some("No signal → None; never raises.".to_string()),
                consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
                source: "system".into(),
                validation_status: "validated".into(),
            },
            "host.check_signals",
        )
        .await?;

    let tool_skill_id = stores
        .upsert_tool_skill(
            NewPgToolSkill {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "ts-host-check-signals".to_string(),
                description: "Poll the thread signal channel for stop/suspend/inject."
                    .to_string(),
                content: "Call `host.check_signals()` between orchestrator steps. On \
                          'stop', halt cleanly. On {inject: msg}, fold the message in and \
                          continue. On None, proceed."
                    .to_string(),
                prior_knowledge_content: None,
                override_prompt_creation: false,
                tool_name: Some("host.check_signals".to_string()),
                param_schema: Some(json!([])),
                param_template: Some(json!({})),
                consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
                intent_examples: None,
                source: "system".into(),
                validation_status: "validated".into(),
            },
            "ts-host-check-signals",
        )
        .await?;

    let python_code_id = stores
        .upsert_python_code(
            NewPgPythonCode {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "pc-host-check-signals".to_string(),
                description: "Orchestrator step: poll the thread signal channel for \
                              stop/suspend/inject."
                    .to_string(),
                content: "# Channel: orchestrator | Class: 22 | No I/O, no imports.\n\
                          sig = host.check_signals()\n"
                    .to_string(),
                prior_knowledge_content: None,
                override_prompt_creation: false,
                consumer_tags: vec!["01:monty".into(), "02:orchestrator".into()],
                intent_examples: None,
                source: "system".into(),
                dependency_registry: None,
            },
            "pc-host-check-signals",
        )
        .await?;

    let skill_id = stores
        .upsert_skill(
            NewPgSkill {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "skill-host-check-signals".to_string(),
                description: "Poll for stop/suspend/inject signals between steps."
                    .to_string(),
                body: "Call `ts-host-check-signals` between orchestrator steps. On 'stop', \
                       halt cleanly. On {inject: msg}, fold the message in and continue. \
                       On None, proceed."
                    .to_string(),
                class_code: 1,
                consumer_tags: vec!["02:orchestrator".into(), "05:validation".into()],
                intent_examples: json!([]),
                source: "system".into(),
                validation_status: "validated".into(),
            },
            "skill-host-check-signals",
        )
        .await?;

    Ok(vec![tool_id, tool_skill_id, python_code_id, skill_id])
}

/// Step 27.7.4 — `host.validate_component` (kohai/sempai → Q1 pending queue).
///
/// A 4-component leaf-tool stack (Tool + ToolSkill + PythonCode + leaf Skill —
/// no Recipe). The Tool row deliberately KEEPS the `05:validator` consumer tag:
/// it is greyed OUT of the LLM tool list (`NOT ('05:validator' = ANY(
/// consumer_tags))`) because `validate_component` is a kohai/sempai-path tool
/// that must not be advertised to the LLM — yet it remains bindable BY NAME via
/// a recipe's `rust_steps` (by-name compose keys on `validation_status=
/// 'validated'`, NOT the consumer-tag grey-out). The leaf skill uses the fork-1
/// `05:validation` tag (the distinct retrievable validated-builtin marker).
#[allow(clippy::too_many_lines)]
async fn seed_host_validate_component(
    stores: &HostStores,
) -> Result<Vec<Uuid>, SeedBuiltinHostError> {
    let tenant = stores.tenant.clone();

    let tool_id = stores
        .upsert_tool(
            NewPgTool {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "host.validate_component".to_string(),
                description: "Intercept a self-improvement component write. Protected \
                              titles (orchestrator:main, prompt:codeact_preamble) become a \
                              Q1 pending update-candidate (llm_audit_required for class \
                              10/50) instead of a direct write. Returns {queued, reason?, \
                              candidate_id?}."
                    .to_string(),
                param_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "title": {"type": "string", "description": "Component title"},
                        "content": {"type": "string", "description": "Proposed component content"},
                        "doc_type": {"type": "string", "description": "skill|recipe|tool_skill|lesson|spec|plan|note"},
                        "metadata": {"type": "object", "description": "Extra metadata (non-overriding on validation fields)"}
                    },
                    "required": ["title", "content"]
                })),
                param_template: Some(json!({"title": "", "content": "", "doc_type": "note", "metadata": {}})),
                effect_type: "write".to_string(),
                preconditions: Some("Store wired.".to_string()),
                error_handling: Some(
                    "Empty payload → {queued:false,reason:'empty payload'}; no store → \
                     {queued:false,reason:'no_store'}."
                        .to_string(),
                ),
                // KEEPS `05:validator` — greyed out of the LLM tool list, bindable by
                // name via a recipe's rust_steps.
                consumer_tags: vec![
                    "00:rusty".into(),
                    "02:orchestrator".into(),
                    "05:validator".into(),
                ],
                source: "system".into(),
                validation_status: "validated".into(),
            },
            "host.validate_component",
        )
        .await?;

    let tool_skill_id = stores
        .upsert_tool_skill(
            NewPgToolSkill {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "ts-host-validate-component".to_string(),
                description: "Route a self-improvement component proposal into the \
                              validation queue."
                    .to_string(),
                content: "Call `host.validate_component(title=<title>, content=<content>, \
                          doc_type=<type>, metadata=<obj>)` when the kohai/sempai system \
                          proposes a new/updated component. Inspect `queued`; protected \
                          components go to Q1 pending with an LLM-audit gate before Q2 \
                          manual validation."
                    .to_string(),
                prior_knowledge_content: None,
                override_prompt_creation: false,
                tool_name: Some("host.validate_component".to_string()),
                param_schema: Some(json!([
                    {"name": "title", "param_type": "string", "required": true, "description": "Component title"},
                    {"name": "content", "param_type": "string", "required": true, "description": "Proposed component content"},
                    {"name": "doc_type", "param_type": "string", "required": false, "description": "skill|recipe|tool_skill|lesson|spec|plan|note"},
                    {"name": "metadata", "param_type": "object", "required": false, "description": "Extra metadata"}
                ])),
                param_template: Some(json!({"title": "{{title}}", "content": "{{content}}", "doc_type": "{{doc_type}}", "metadata": "{{metadata}}"})),
                consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
                intent_examples: None,
                source: "system".into(),
                validation_status: "validated".into(),
            },
            "ts-host-validate-component",
        )
        .await?;

    let python_code_id = stores
        .upsert_python_code(
            NewPgPythonCode {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "pc-host-validate-component".to_string(),
                description: "Orchestrator step: route a self-improvement component \
                              proposal into the validation queue."
                    .to_string(),
                content: "# Channel: orchestrator | Class: 22 | No I/O, no imports.\n\
                          res = host.validate_component(title=\"{{vars.slot0}}\", content=\"{{vars.slot1}}\", doc_type=\"{{vars.slot2}}\", metadata={{vars.slot3}})\n"
                    .to_string(),
                prior_knowledge_content: None,
                override_prompt_creation: false,
                consumer_tags: vec!["01:monty".into(), "02:orchestrator".into()],
                intent_examples: None,
                source: "system".into(),
                dependency_registry: None,
            },
            "pc-host-validate-component",
        )
        .await?;

    let skill_id = stores
        .upsert_skill(
            NewPgSkill {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "skill-host-validate-component".to_string(),
                description: "Route a self-improvement component proposal into the \
                              validation queue."
                    .to_string(),
                body: "When the kohai/sempai system proposes a new/updated component, call \
                       `ts-host-validate-component` with title, content, doc_type, and any \
                       extra metadata. Inspect `queued`; protected components go to Q1 \
                       pending with an LLM-audit gate before Q2 manual validation."
                    .to_string(),
                class_code: 1,
                consumer_tags: vec!["02:orchestrator".into(), "05:validation".into()],
                intent_examples: json!([]),
                source: "system".into(),
                validation_status: "validated".into(),
            },
            "skill-host-validate-component",
        )
        .await?;

    Ok(vec![tool_id, tool_skill_id, python_code_id, skill_id])
}

/// Step 27.7.3 — `host.resolve_component_by_name` (SEC-01 validated fetch by
/// name — §0.9 Option B).
///
/// A 4-component leaf-tool stack (Tool + ToolSkill + PythonCode + leaf Skill —
/// no Recipe) — the `call_action` Option B fallback when only a step name is
/// held. Returns the minted ids in `[tool, tool_skill, python_code, skill]`
/// order. Same null-means-absent contract as `host.fetch_component`.
#[allow(clippy::too_many_lines)]
async fn seed_host_resolve_component_by_name(
    stores: &HostStores,
) -> Result<Vec<Uuid>, SeedBuiltinHostError> {
    let tenant = stores.tenant.clone();

    let tool_id = stores
        .upsert_tool(
            NewPgTool {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "host.resolve_component_by_name".to_string(),
                description: "Fetch a single validated component by NAME + class code \
                              (SEC-01 gate) — the §0.9 Option B fallback when only a step \
                              name is held. Same dict shape as host.fetch_component, or null."
                    .to_string(),
                param_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "name": {"type": "string", "description": "Component name"},
                        "class_code": {"type": "integer", "description": "Component class code"}
                    },
                    "required": ["name", "class_code"]
                })),
                param_template: Some(json!({"name": "", "class_code": 0})),
                effect_type: "read".to_string(),
                preconditions: Some(
                    "skills-db pool wired (returns null without it).".to_string(),
                ),
                error_handling: Some(
                    "Missing/invalid/absent → null; never raises.".to_string(),
                ),
                consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
                source: "system".into(),
                validation_status: "validated".into(),
            },
            "host.resolve_component_by_name",
        )
        .await?;

    let tool_skill_id = stores
        .upsert_tool_skill(
            NewPgToolSkill {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "ts-host-resolve-component-by-name".to_string(),
                description: "Resolve a validated component by name + class code \
                              (SEC-01 gate, §0.9 Option B)."
                    .to_string(),
                content: "Call `host.resolve_component_by_name(name=<name>, class_code=<int>)` \
                          when call_action holds a step name, not a UUID. Returns the component \
                          projection or null. A null result means the component is absent or not \
                          validated — do not invent one."
                    .to_string(),
                prior_knowledge_content: None,
                override_prompt_creation: false,
                tool_name: Some("host.resolve_component_by_name".to_string()),
                param_schema: Some(json!([
                    {"name": "name", "param_type": "string", "required": true, "description": "Component name"},
                    {"name": "class_code", "param_type": "number", "required": true, "description": "Component class code"}
                ])),
                param_template: Some(json!({"name": "{{name}}", "class_code": "{{class_code}}"})),
                consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
                intent_examples: None,
                source: "system".into(),
                validation_status: "validated".into(),
            },
            "ts-host-resolve-component-by-name",
        )
        .await?;

    let python_code_id = stores
        .upsert_python_code(
            NewPgPythonCode {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "pc-host-resolve-component-by-name".to_string(),
                description: "Orchestrator step: resolve a validated component by name + \
                              class code (§0.9 Option B)."
                    .to_string(),
                content: "# Channel: orchestrator | Class: 22 | No I/O, no imports.\n\
                          comp = host.resolve_component_by_name(name=\"{{vars.slot0}}\", class_code={{vars.slot1}})\n"
                    .to_string(),
                prior_knowledge_content: None,
                override_prompt_creation: false,
                consumer_tags: vec!["01:monty".into(), "02:orchestrator".into()],
                intent_examples: None,
                source: "system".into(),
                dependency_registry: None,
            },
            "pc-host-resolve-component-by-name",
        )
        .await?;

    let skill_id = stores
        .upsert_skill(
            NewPgSkill {
                tenant_id: tenant.clone(),
                user_id: SEED_USER.to_string(),
                agent_id: SEED_AGENT.to_string(),
                project_id: SEED_PROJECT.to_string(),
                name: "skill-host-resolve-component-by-name".to_string(),
                description: "Resolve a component by name (§0.9 Option B) when only a \
                              step name is held."
                    .to_string(),
                body: "Use `ts-host-resolve-component-by-name` when call_action holds a \
                       step name, not a UUID. Same null-means-absent contract as \
                       fetch_component."
                    .to_string(),
                class_code: 1,
                consumer_tags: vec!["02:orchestrator".into(), "05:validation".into()],
                intent_examples: json!([]),
                source: "system".into(),
                validation_status: "validated".into(),
            },
            "skill-host-resolve-component-by-name",
        )
        .await?;

    Ok(vec![tool_id, tool_skill_id, python_code_id, skill_id])
}
