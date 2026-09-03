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
