//! Zencoder Extension Bootstrap Seeder (Pass 16).
//!
//! Seeds the full v3 component stack for the Zencoder/Zenflow AI coding agent
//! REST API extension at composition boot. Called from
//! [`crate::builtin_bootstrap::seed_builtin_components`] as Pass 16.
//!
//! # Component stack
//!
//! - 1 Tool (class 0): `zencoder-api` — wraps `builtin.http` with Zencoder-
//!   specific config (Bearer JWT from secret `zencoder_access_token`, base URL
//!   `https://api.zencoder.ai/api/v1/`, 30 s timeout)
//! - 8 ToolSkills (class 13): one per URL template × HTTP method
//! - 11 PythonCode (class 22): 8 executors (one host call each) + 3 pure-logic
//!   helpers (UUID guard, task-summary merge, auth-instructions post)
//! - 10 Leaf Skills (class 1): 8 per-executor + 2 shared (auth-error,
//!   resilience)
//! - 1 Domain Skill (class 2): `skill-zencoder`
//! - 10 Recipes (class 21): 7 Tier-0 + 3 Tier-1
//! - 1 ExtensionCatalogue (class 23): `ext-zencoder`
//!
//! # Idempotency
//!
//! Uses the same get-or-insert pattern as `builtin_bootstrap.rs`. Safe to call
//! on every composition boot.
//!
//! # Plan reference
//!
//! `docs/plans/zencoder-extension-plan.md` (v3) is the authoritative source for
//! all component bodies, step_descriptions, and intent_examples transcribed here.

#![forbid(unsafe_code)]

use std::sync::Arc;

use brassclaw_pg::PgPool;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::builtin_bootstrap::SeedBuiltinBootstrapError;
use crate::pg_extension_catalogue_store::{NewPgExtensionCatalogue, PgExtensionCatalogueStore};
use crate::pg_python_code_store::{NewPgPythonCode, PgPythonCodeStore};
use crate::pg_recipe_store::{NewPgRecipe, PgRecipeStore};
use crate::pg_skill_store::{NewPgSkill, PgSkillStore};
use crate::pg_tool_skill_store::{NewPgToolSkill, PgToolSkillStore};
use crate::pg_tool_store::{NewPgTool, PgToolStore};
use crate::validation_queue::ValidationQueueStore;
use brassclaw_host_api::SYSTEM_RESERVED_ID;

const SEED_USER: &str = SYSTEM_RESERVED_ID;
const SEED_AGENT: &str = "default";
const SEED_PROJECT: &str = "system";

// ---------------------------------------------------------------------------
// Local store bundle (mirrors BootstrapStores in builtin_bootstrap.rs)
// ---------------------------------------------------------------------------

struct ZencoderStores {
    tenant: String,
    pool: Arc<PgPool>,
    tool: PgToolStore,
    tool_skill: PgToolSkillStore,
    python_code: PgPythonCodeStore,
    skill: PgSkillStore,
    recipe: PgRecipeStore,
    catalogue: PgExtensionCatalogueStore,
    queue: ValidationQueueStore,
}

impl ZencoderStores {
    fn new(pool: Arc<PgPool>, tenant_id: &str) -> Self {
        Self {
            tenant: tenant_id.to_string(),
            tool: PgToolStore::new(pool.clone()),
            tool_skill: PgToolSkillStore::new(pool.clone()),
            python_code: PgPythonCodeStore::new(pool.clone()),
            skill: PgSkillStore::new(pool.clone()),
            recipe: PgRecipeStore::new(pool.clone()),
            pool: pool.clone(),
            queue: ValidationQueueStore::new(pool.clone()),
            catalogue: PgExtensionCatalogueStore::new(pool),
        }
    }

    async fn audit(&self, id: Uuid, class_code: i32, name: &str) {
        use crate::validation_queue::ValidationQueueError;
        use brassclaw_engine::memory::retrieval_source::ComponentScope;
        let scope = ComponentScope {
            tenant_id: self.tenant.clone(),
            user_id: SEED_USER.to_string(),
            agent_id: SEED_AGENT.to_string(),
            project_id: SEED_PROJECT.to_string(),
        };
        match self.queue.submit(&scope, id, class_code, None).await {
            Ok(()) => {}
            Err(ValidationQueueError::AlreadyQueued { .. }) => return,
            Err(e) => {
                tracing::debug!(component_id=%id, class_code, name, error=%e,
                    "zencoder audit: submit failed (non-fatal)");
                return;
            }
        }
        if let Err(e) = self.queue.gate1_pass(&scope, id, &[]).await {
            tracing::debug!(component_id=%id, class_code, name, error=%e,
                "zencoder audit: gate1_pass failed (non-fatal)");
            return;
        }
        if let Err(e) = self.queue.approve(&scope, id, Some("builtin")).await {
            tracing::debug!(component_id=%id, class_code, name, error=%e,
                "zencoder audit: approve failed (non-fatal)");
        }
    }

    async fn upsert_tool(&self, row: NewPgTool, name: &str) -> Result<Uuid, SeedBuiltinBootstrapError> {
        let map = |e: crate::pg_tool_store::PgToolStoreError| SeedBuiltinBootstrapError::Db { reason: e.to_string() };
        if let Some(id) = self.tool.insert(row).await.map_err(map)? {
            self.audit(id, 0, name).await;
            return Ok(id);
        }
        self.tool.get_id_by_name(&self.tenant, SEED_USER, SEED_AGENT, SEED_PROJECT, name)
            .await.map_err(map)?
            .ok_or_else(|| SeedBuiltinBootstrapError::Db { reason: format!("tool `{name}` not found") })
    }

    async fn upsert_tool_skill(&self, row: NewPgToolSkill, name: &str) -> Result<Uuid, SeedBuiltinBootstrapError> {
        let map = |e: crate::pg_tool_skill_store::PgToolSkillStoreError| SeedBuiltinBootstrapError::Db { reason: e.to_string() };
        if let Some(id) = self.tool_skill.insert(row).await.map_err(map)? {
            self.audit(id, 13, name).await;
            return Ok(id);
        }
        self.tool_skill.get_id_by_name(&self.tenant, SEED_USER, SEED_AGENT, SEED_PROJECT, name)
            .await.map_err(map)?
            .ok_or_else(|| SeedBuiltinBootstrapError::Db { reason: format!("tool_skill `{name}` not found") })
    }

    async fn upsert_skill(&self, row: NewPgSkill, name: &str) -> Result<Uuid, SeedBuiltinBootstrapError> {
        let map = |e: crate::pg_skill_store::PgSkillStoreError| SeedBuiltinBootstrapError::Db { reason: e.to_string() };
        if let Some(id) = self.skill.insert(row).await.map_err(map)? {
            self.audit(id, 1, name).await;
            return Ok(id);
        }
        self.skill.get_id_by_name(&self.tenant, SEED_USER, SEED_AGENT, SEED_PROJECT, name)
            .await.map_err(map)?
            .ok_or_else(|| SeedBuiltinBootstrapError::Db { reason: format!("skill `{name}` not found") })
    }

    async fn upsert_python_code(&self, row: NewPgPythonCode, name: &str) -> Result<Uuid, SeedBuiltinBootstrapError> {
        let map = |e: crate::pg_python_code_store::PgPythonCodeStoreError| SeedBuiltinBootstrapError::Db { reason: e.to_string() };
        if let Some(existing) = self.python_code.get_by_name(&self.tenant, SEED_USER, SEED_AGENT, SEED_PROJECT, name)
            .await.map_err(map)? {
            return Ok(existing.id);
        }
        let id = self.python_code.insert(row).await.map_err(map)?;
        self.python_code.update_validation_status(&self.tenant, SEED_USER, SEED_AGENT, SEED_PROJECT, id, "validated")
            .await.map_err(map)?;
        self.audit(id, 22, name).await;
        Ok(id)
    }

    async fn upsert_recipe(&self, row: NewPgRecipe, name: &str) -> Result<Uuid, SeedBuiltinBootstrapError> {
        let map = |e: crate::pg_recipe_store::PgRecipeStoreError| SeedBuiltinBootstrapError::Db { reason: e.to_string() };
        if let Some(existing) = self.recipe.get_by_name(&self.tenant, SEED_USER, SEED_AGENT, SEED_PROJECT, name)
            .await.map_err(map)? {
            return Ok(existing.id);
        }
        let id = self.recipe.insert(row).await.map_err(map)?;
        self.audit(id, 21, name).await;
        Ok(id)
    }

    async fn mark_recipe_tier0(&self, recipe_id: Uuid) -> Result<(), SeedBuiltinBootstrapError> {
        let client = self.pool.get().await.map_err(|e| SeedBuiltinBootstrapError::Pool { reason: e.to_string() })?;
        client.execute(
            "UPDATE reborn_recipes SET tier = 'mature', wilson_lower = 1.0 \
             WHERE id = $1 AND tenant_id = $2 AND user_id = $3 AND agent_id = $4 AND project_id = $5",
            &[&recipe_id, &self.tenant, &SEED_USER, &SEED_AGENT, &SEED_PROJECT],
        ).await.map_err(|e| SeedBuiltinBootstrapError::Db { reason: e.to_string() })?;
        Ok(())
    }

    async fn upsert_catalogue(&self, row: NewPgExtensionCatalogue, name: &str) -> Result<Uuid, SeedBuiltinBootstrapError> {
        let map = |e: crate::pg_extension_catalogue_store::PgExtensionCatalogueStoreError| SeedBuiltinBootstrapError::Db { reason: e.to_string() };
        if let Some(existing) = self.catalogue.get_by_name(&self.tenant, SEED_USER, SEED_AGENT, SEED_PROJECT, name)
            .await.map_err(map)? {
            return Ok(existing.id);
        }
        let id = self.catalogue.insert(row).await.map_err(map)?;
        self.catalogue.update_validation_status(&self.tenant, SEED_USER, SEED_AGENT, SEED_PROJECT, id, "validated")
            .await.map_err(map)?;
        self.audit(id, 23, name).await;
        Ok(id)
    }

    async fn append_children(&self, cat_id: Uuid, child_ids: &[Uuid]) -> Result<(), SeedBuiltinBootstrapError> {
        let map = |e: crate::pg_extension_catalogue_store::PgExtensionCatalogueStoreError| SeedBuiltinBootstrapError::Db { reason: e.to_string() };
        self.catalogue.append_child_component_ids(&self.tenant, SEED_USER, SEED_AGENT, SEED_PROJECT, cat_id, child_ids)
            .await.map_err(map)
    }

    async fn seed_recipe(
        &self,
        name: &str,
        description: &str,
        tier0: bool,
        yaml_source: &str,
        step_entries: &[Value],
        intent_examples: &[Value],
    ) -> Result<Uuid, SeedBuiltinBootstrapError> {
        let id = self.upsert_recipe(recipe_row(&self.tenant, name, description, yaml_source, step_entries, intent_examples), name).await?;
        if tier0 {
            self.mark_recipe_tier0(id).await?;
        }
        Ok(id)
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Seed the Zencoder extension component stack for `tenant_id`.
///
/// Called from `seed_builtin_components` as Pass 16. Idempotent.
pub async fn seed_zencoder_extension(
    pool: Arc<PgPool>,
    tenant_id: &str,
) -> Result<(), SeedBuiltinBootstrapError> {
    let s = ZencoderStores::new(pool, tenant_id);
    let t = &s.tenant.clone();

    // ---- Step 1: Tool (class 0) ----
    let tool_zencoder = s.upsert_tool(tool_zencoder_api_row(t), "zencoder-api").await?;

    // ---- Step 2: ToolSkills (class 13) ----
    let ts_projects_list   = s.upsert_tool_skill(ts_projects_list_row(t),   "ts-zencoder-projects-list").await?;
    let ts_tasks_list      = s.upsert_tool_skill(ts_tasks_list_row(t),      "ts-zencoder-tasks-list").await?;
    let ts_tasks_get       = s.upsert_tool_skill(ts_tasks_get_row(t),       "ts-zencoder-tasks-get").await?;
    let ts_tasks_post      = s.upsert_tool_skill(ts_tasks_post_row(t),      "ts-zencoder-tasks-post").await?;
    let ts_tasks_patch     = s.upsert_tool_skill(ts_tasks_patch_row(t),     "ts-zencoder-tasks-patch").await?;
    let ts_plan_get        = s.upsert_tool_skill(ts_plan_get_row(t),        "ts-zencoder-plan-get").await?;
    let ts_automations_list = s.upsert_tool_skill(ts_automations_list_row(t), "ts-zencoder-automations-list").await?;
    let ts_automations_post = s.upsert_tool_skill(ts_automations_post_row(t), "ts-zencoder-automations-post").await?;

    // Also need ts-host-post-reply for the auth-setup recipe — look it up from
    // the already-seeded host group rather than re-inserting it.
    let ts_host_post_reply = s.tool_skill
        .get_id_by_name(t, SEED_USER, SEED_AGENT, SEED_PROJECT, "ts-host-post-reply")
        .await
        .map_err(|e| SeedBuiltinBootstrapError::Db { reason: e.to_string() })?
        .ok_or_else(|| SeedBuiltinBootstrapError::Db {
            reason: "ts-host-post-reply not found — host group must be seeded first".into(),
        })?;

    // ---- Step 3: PythonCode (class 22) ----
    let pc_validate_uuid   = s.upsert_python_code(pc_row(t, "pc-zencoder-validate-uuid",
        "Pure-logic: validates slot0 as a UUID. Returns {valid, error?}. No host call.",
        PC_VALIDATE_UUID), "pc-zencoder-validate-uuid").await?;
    let pc_build_summary   = s.upsert_python_code(pc_row(t, "pc-zencoder-build-task-summary",
        "Pure-logic: merges task JSON (slot0) and plan JSON (slot1) into a summary dict. No host call.",
        PC_BUILD_TASK_SUMMARY), "pc-zencoder-build-task-summary").await?;
    let pc_list_projects   = s.upsert_python_code(pc_row(t, "pc-zencoder-list-projects",
        "Executor: GET /projects via host.zencoder_api. No parameters.",
        PC_LIST_PROJECTS), "pc-zencoder-list-projects").await?;
    let pc_list_tasks      = s.upsert_python_code(pc_row(t, "pc-zencoder-list-tasks",
        "Executor: GET /projects/{pid}/tasks[?status&limit] via host.zencoder_api. slot0=pid, slot1=status, slot2=limit.",
        PC_LIST_TASKS), "pc-zencoder-list-tasks").await?;
    let pc_get_task        = s.upsert_python_code(pc_row(t, "pc-zencoder-get-task",
        "Executor: GET /projects/{pid}/tasks/{tid} via host.zencoder_api. slot0=pid, slot1=tid.",
        PC_GET_TASK), "pc-zencoder-get-task").await?;
    let pc_get_plan        = s.upsert_python_code(pc_row(t, "pc-zencoder-get-plan",
        "Executor: GET /projects/{pid}/tasks/{tid}/plan via host.zencoder_api. slot0=pid, slot1=tid.",
        PC_GET_PLAN), "pc-zencoder-get-plan").await?;
    let pc_create_task     = s.upsert_python_code(pc_row(t, "pc-zencoder-create-task",
        "Executor: POST /projects/{pid}/tasks via host.zencoder_api. slot0=pid, slot1=JSON body.",
        PC_CREATE_TASK), "pc-zencoder-create-task").await?;
    let pc_patch_task      = s.upsert_python_code(pc_row(t, "pc-zencoder-patch-task",
        "Executor: PATCH /projects/{pid}/tasks/{tid} via host.zencoder_api. slot0=pid, slot1=tid, slot2=JSON body.",
        PC_PATCH_TASK), "pc-zencoder-patch-task").await?;
    let pc_list_automations = s.upsert_python_code(pc_row(t, "pc-zencoder-list-automations",
        "Executor: GET /automations[?enabled] via host.zencoder_api. slot0=enabled filter.",
        PC_LIST_AUTOMATIONS), "pc-zencoder-list-automations").await?;
    let pc_create_automation = s.upsert_python_code(pc_row(t, "pc-zencoder-create-automation",
        "Executor: POST /automations via host.zencoder_api. slot0=JSON body (LLM-composed, user-confirmed).",
        PC_CREATE_AUTOMATION), "pc-zencoder-create-automation").await?;
    let pc_post_auth       = s.upsert_python_code(pc_row(t, "pc-zencoder-post-auth-instructions",
        "Executor: posts fixed Zencoder auth setup instructions to chat via host.post_reply. No parameters.",
        PC_POST_AUTH_INSTRUCTIONS), "pc-zencoder-post-auth-instructions").await?;

    // ---- Step 4: Leaf Skills (class 1) ----
    let sk_auth_error      = s.upsert_skill(leaf_skill(t, "skill-zencoder-auth-error",
        "What Zencoder HTTP error codes mean and how to recover (401/402/429/5xx).",
        SKILL_AUTH_ERROR), "skill-zencoder-auth-error").await?;
    let sk_resilience      = s.upsert_skill(leaf_skill(t, "skill-zencoder-resilience",
        "Three-state resilience model: healthy / degraded / unavailable. Skip Zencoder calls when degraded.",
        SKILL_RESILIENCE), "skill-zencoder-resilience").await?;
    let sk_list_projects   = s.upsert_skill(leaf_skill(t, "skill-zencoder-list-projects",
        "How to list accessible Zencoder projects via pc-zencoder-list-projects.",
        SKILL_LIST_PROJECTS), "skill-zencoder-list-projects").await?;
    let sk_list_tasks      = s.upsert_skill(leaf_skill(t, "skill-zencoder-list-tasks",
        "How to list tasks in a Zencoder project with optional status/limit filters.",
        SKILL_LIST_TASKS), "skill-zencoder-list-tasks").await?;
    let sk_get_task        = s.upsert_skill(leaf_skill(t, "skill-zencoder-get-task",
        "How to fetch a single Zencoder task. Always call before PATCH — description is fully replaced.",
        SKILL_GET_TASK), "skill-zencoder-get-task").await?;
    let sk_get_plan        = s.upsert_skill(leaf_skill(t, "skill-zencoder-get-plan",
        "How to fetch a Zencoder task execution plan. 404 = plan not yet created, not an error.",
        SKILL_GET_PLAN), "skill-zencoder-get-plan").await?;
    let sk_create_task     = s.upsert_skill(leaf_skill(t, "skill-zencoder-create-task",
        "How to create a Zencoder task. For delegation use workflow_id:default-auto-workflow + start:true.",
        SKILL_CREATE_TASK), "skill-zencoder-create-task").await?;
    let sk_patch_task      = s.upsert_skill(leaf_skill(t, "skill-zencoder-patch-task",
        "How to patch a Zencoder task. PATCH fully replaces description — read first.",
        SKILL_PATCH_TASK), "skill-zencoder-patch-task").await?;
    let sk_list_automations = s.upsert_skill(leaf_skill(t, "skill-zencoder-list-automations",
        "How to list Zencoder automations with optional enabled filter.",
        SKILL_LIST_AUTOMATIONS), "skill-zencoder-list-automations").await?;
    let sk_create_automation = s.upsert_skill(leaf_skill(t, "skill-zencoder-create-automation",
        "How to create a Zencoder automation. Always confirm full spec with user before dispatching.",
        SKILL_CREATE_AUTOMATION), "skill-zencoder-create-automation").await?;

    // ---- Step 5: Domain Skill (class 2) ----
    let sk_domain = s.upsert_skill(
        skill_row(t, "skill-zencoder",
            "Zencoder/Zenflow AI coding agent delegation — routing rules, resilience, leaf skill index.",
            SKILL_DOMAIN, 2, &["02:orchestrator"]),
        "skill-zencoder").await?;

    // ---- Step 7: Recipes (class 21) ----

    // Tier-0: list-projects
    let r_list_projects = s.seed_recipe(
        "zencoder-list-projects",
        "List all accessible Zencoder projects.",
        true,
        YAML_LIST_PROJECTS,
        &[
            step_entry(1, "rust",         "Pre-load ts-zencoder-projects-list ToolSkill binding", "component", &[ts_projects_list]),
            step_entry(2, "orchestrator", "Execute: host.zencoder_api(GET /projects)",            "component", &[pc_list_projects]),
        ],
        &intents(&[
            ("list my zencoder projects",          1), ("what zencoder projects do I have",  1),
            ("show all zenflow projects",          1), ("zencoder project list",              1),
            ("which project should I pick",        2), ("zenflow projects",                  1),
            ("what projects are in zencoder",      1), ("list projects",                     2),
            ("show zenflow project list",          1), ("zencoder projects",                 1),
        ]),
    ).await?;

    // Tier-0: list-tasks (no filter)
    let r_list_tasks = s.seed_recipe(
        "zencoder-list-tasks",
        "List all tasks in a Zencoder project (no status filter).",
        true,
        YAML_LIST_TASKS,
        &[
            step_entry(1, "rust",         "Pre-load ts-zencoder-tasks-list ToolSkill binding",             "component", &[ts_tasks_list]),
            step_entry(2, "orchestrator", "Execute: host.zencoder_api(GET /projects/{pid}/tasks)",          "component", &[pc_list_tasks]),
        ],
        &intents(&[
            ("list zencoder tasks",               1), ("show all zenflow tasks",              1),
            ("what tasks are in this project",    1), ("zencoder task list",                  1),
            ("list all tasks",                    2), ("show zenflow tasks",                  1),
            ("what tasks exist in zencoder",      1), ("list my zencoder tasks",              1),
            ("show tasks in project X",           2), ("zenflow task list",                   1),
        ]),
    ).await?;

    // Tier-0: list-tasks-filtered (with status filter)
    let r_list_tasks_filtered = s.seed_recipe(
        "zencoder-list-tasks-filtered",
        "List tasks in a Zencoder project filtered by status (todo|inprogress|inreview|done|cancelled).",
        true,
        YAML_LIST_TASKS_FILTERED,
        &[
            step_entry(1, "rust",         "Pre-load ts-zencoder-tasks-list ToolSkill binding",                          "component", &[ts_tasks_list]),
            step_entry(2, "orchestrator", "Execute: host.zencoder_api(GET /projects/{pid}/tasks?status={slot1})",        "component", &[pc_list_tasks]),
        ],
        &intents(&[
            ("show inprogress zencoder tasks",    1), ("list tasks in review",                1),
            ("what tasks are done",               1), ("show cancelled zencoder tasks",       1),
            ("list todo tasks in zenflow",        1), ("which tasks are in progress",         1),
            ("show tasks with status done",       1), ("zenflow inprogress tasks",            1),
            ("list tasks that are in review",     1), ("show open zencoder tasks",            1),
        ]),
    ).await?;

    // Tier-0: get-task
    let r_get_task = s.seed_recipe(
        "zencoder-get-task",
        "Fetch a single Zencoder task by project_id and task_id.",
        true,
        YAML_GET_TASK,
        &[
            step_entry(1, "rust",         "Pre-load ts-zencoder-tasks-get ToolSkill binding",                   "component", &[ts_tasks_get]),
            step_entry(2, "orchestrator", "Execute: host.zencoder_api(GET /projects/{pid}/tasks/{tid})",         "component", &[pc_get_task]),
        ],
        &intents(&[
            ("get zencoder task",                 1), ("show task details",                  2),
            ("what is the status of task X",      1), ("fetch zencoder task",                1),
            ("look up zenflow task",              1), ("show me that task",                  2),
            ("get task by id",                    1), ("zencoder task details",              1),
            ("what does task X say",              1), ("read zenflow task",                  1),
        ]),
    ).await?;

    // Tier-0: get-plan
    let r_get_plan = s.seed_recipe(
        "zencoder-get-plan",
        "Fetch the execution plan for a Zencoder task.",
        true,
        YAML_GET_PLAN,
        &[
            step_entry(1, "rust",         "Pre-load ts-zencoder-plan-get ToolSkill binding",                          "component", &[ts_plan_get]),
            step_entry(2, "orchestrator", "Execute: host.zencoder_api(GET /projects/{pid}/tasks/{tid}/plan)",          "component", &[pc_get_plan]),
        ],
        &intents(&[
            ("show the zencoder plan",            1), ("what steps has zenflow planned",     1),
            ("get plan for this task",            1), ("zencoder execution plan",             1),
            ("show plan steps",                   1), ("what is zenflow going to do",        1),
            ("list the plan",                     2), ("show task plan",                     2),
            ("get plan from zencoder",            1), ("zenflow plan steps",                 1),
        ]),
    ).await?;

    // Tier-0: check-solution-status (get-task + get-plan + summary merge)
    let r_check_status = s.seed_recipe(
        "zencoder-check-solution-status",
        "Check a Zencoder task status and execution plan progress in one combined read.",
        true,
        YAML_CHECK_STATUS,
        &[
            step_entry(1, "rust",         "Pre-load task + plan ToolSkill bindings",
                "component", &[ts_tasks_get, ts_plan_get]),
            step_entry(2, "orchestrator", "Execute: GET /projects/{pid}/tasks/{tid}",
                "component", &[pc_get_task]),
            step_entry(3, "orchestrator", "Execute: GET /projects/{pid}/tasks/{tid}/plan",
                "component", &[pc_get_plan]),
            step_entry(4, "orchestrator", "Pure-logic: merge task + plan results into summary dict",
                "component", &[pc_build_summary]),
        ],
        &intents(&[
            ("how is that zencoder task going",   1), ("check zencoder status",              1),
            ("is the zenflow agent done",         1), ("check solution status",              1),
            ("how far along is zencoder",         1), ("what has zenflow done so far",       1),
            ("status of the delegated task",      1), ("is the coding task finished",        1),
            ("how many steps completed",          1), ("zencoder progress",                  1),
            ("check if zencoder is done",         1), ("what is the progress",               2),
        ]),
    ).await?;

    // Tier-0: auth-setup (fixed reply via host.post_reply — no LLM)
    let r_auth_setup = s.seed_recipe(
        "zencoder-auth-setup",
        "Post fixed Zencoder authentication setup instructions to the user. Tier 0.",
        true,
        YAML_AUTH_SETUP,
        &[
            step_entry(1, "rust",         "Pre-load ts-host-post-reply ToolSkill binding",            "component", &[ts_host_post_reply]),
            step_entry(2, "orchestrator", "Execute: host.post_reply(fixed auth instructions)",         "component", &[pc_post_auth]),
        ],
        &intents(&[
            ("set up zencoder",                   1), ("authenticate with zencoder",         1),
            ("configure zencoder token",          1), ("zencoder auth",                      1),
            ("how do I connect to zencoder",      1), ("set zencoder api key",               1),
            ("zencoder login",                    1), ("configure zenflow access",            1),
            ("set up zenflow credentials",        1), ("zencoder token setup",               1),
        ]),
    ).await?;

    // Tier-1: solve-coding-problem (LLM composes task body)
    let r_solve = s.seed_recipe(
        "zencoder-solve-coding-problem",
        "Delegate a coding problem to Zencoder — LLM composes and confirms task body, then creates + starts a task.",
        false,
        YAML_SOLVE,
        &[
            step_entry(1, "orchestrator", "Load Zencoder domain + create-task + auth-error skills as LLM context",
                "component", &[sk_domain, sk_create_task, sk_list_projects, sk_auth_error]),
            step_entry(2, "orchestrator", "LLM: compose task body, confirm project_id, present to user",
                "text", &[]),
            step_entry(3, "rust",         "Pre-load ts-zencoder-tasks-post ToolSkill binding",
                "component", &[ts_tasks_post]),
            step_entry(4, "orchestrator", "Execute: host.zencoder_api(POST /projects/{pid}/tasks, body)",
                "component", &[pc_create_task]),
        ],
        &intents(&[
            ("delegate this to zencoder",                    1), ("have zenflow fix this",              1),
            ("let zencoder handle the bug",                  1), ("solve coding problem with zencoder", 1),
            ("send this to zenflow",                         1), ("zencoder solve this",                1),
            ("have zencoder implement this",                 1), ("push this to zenflow",               1),
            ("ask zencoder to fix this",                     1), ("delegate to zenflow agents",         1),
            ("zenflow take over",                            1), ("delegate the implementation to zencoder", 1),
        ]),
    ).await?;

    // Tier-1: update-task (LLM reads current task, composes PATCH body)
    let r_update = s.seed_recipe(
        "zencoder-update-task",
        "Update a Zencoder task — LLM reads current state, composes PATCH body, user confirms.",
        false,
        YAML_UPDATE_TASK,
        &[
            step_entry(1, "orchestrator", "Load get-task + patch-task + auth-error skills as LLM context",
                "component", &[sk_get_task, sk_patch_task, sk_auth_error]),
            step_entry(2, "orchestrator", "LLM: read current task in context, compose PATCH body, confirm with user",
                "text", &[]),
            step_entry(3, "rust",         "Pre-load ts-zencoder-tasks-patch ToolSkill binding",
                "component", &[ts_tasks_patch]),
            step_entry(4, "orchestrator", "Execute: host.zencoder_api(PATCH /projects/{pid}/tasks/{tid}, body)",
                "component", &[pc_patch_task]),
        ],
        &intents(&[
            ("update the zencoder task",          1), ("mark task as done",                  1),
            ("change task status to done",        1), ("close this zencoder task",           1),
            ("update task description",           1), ("set status to cancelled",            1),
            ("add notes to the task",             1), ("mark zenflow task complete",         1),
            ("finish the zencoder task",          1), ("update zenflow task status",         1),
        ]),
    ).await?;

    // Tier-1: create-automation (LLM collects details, user confirms)
    let r_automation = s.seed_recipe(
        "zencoder-create-automation",
        "Create a Zencoder scheduled automation — LLM collects details and user confirms before dispatch.",
        false,
        YAML_CREATE_AUTOMATION,
        &[
            step_entry(1, "orchestrator", "Load create-automation + auth-error skills as LLM context",
                "component", &[sk_create_automation, sk_auth_error]),
            step_entry(2, "orchestrator", "LLM: collect automation details, validate schedule_time, present full spec for confirmation",
                "text", &[]),
            step_entry(3, "rust",         "Pre-load ts-zencoder-automations-post ToolSkill binding",
                "component", &[ts_automations_post]),
            step_entry(4, "orchestrator", "Execute: host.zencoder_api(POST /automations, body)",
                "component", &[pc_create_automation]),
        ],
        &intents(&[
            ("create a zencoder automation",      1), ("schedule zenflow daily",             1),
            ("run zencoder every weekday",        1), ("automate zencoder",                  1),
            ("set up scheduled zencoder task",    1), ("create zenflow automation",          1),
            ("schedule zencoder at 9am",          1), ("add a zencoder cron",                1),
            ("daily zenflow run",                 1), ("automate the coding task weekly",    1),
        ]),
    ).await?;

    // ---- Step 8: ExtensionCatalogue (class 23) ----
    let cat = s.upsert_catalogue(
        NewPgExtensionCatalogue {
            tenant_id: t.to_string(),
            user_id:   SEED_USER.to_string(),
            agent_id:  SEED_AGENT.to_string(),
            project_id: SEED_PROJECT.to_string(),
            name:       "ext-zencoder".to_string(),
            description: "Zencoder/Zenflow AI coding agent: delegate tasks, track progress, manage automations.".to_string(),
            version:    "1.0".into(),
            overview_doc: CAT_OVERVIEW.into(),
            task_groups: json!([
                {"group_name": "delegation",      "description": "Delegate and track coding tasks"},
                {"group_name": "task-management", "description": "List, read, and update tasks"},
                {"group_name": "plans",           "description": "Read task execution plans"},
                {"group_name": "automations",     "description": "List and create scheduled automations"},
                {"group_name": "setup",           "description": "Authenticate with Zencoder"},
            ]),
            child_component_ids: Vec::new(),
            intent_index: None,
            prior_knowledge_content: None,
            override_prompt_creation: false,
            consumer_tags: vec!["02:orchestrator".into()],
            intent_examples: None,
            source: "system".into(),
            dependency_registry: None,
        },
        "ext-zencoder",
    ).await?;

    s.append_children(cat, &[
        tool_zencoder,
        ts_projects_list, ts_tasks_list, ts_tasks_get, ts_tasks_post, ts_tasks_patch,
        ts_plan_get, ts_automations_list, ts_automations_post,
        pc_validate_uuid, pc_build_summary, pc_list_projects, pc_list_tasks,
        pc_get_task, pc_get_plan, pc_create_task, pc_patch_task,
        pc_list_automations, pc_create_automation, pc_post_auth,
        sk_auth_error, sk_resilience, sk_list_projects, sk_list_tasks,
        sk_get_task, sk_get_plan, sk_create_task, sk_patch_task,
        sk_list_automations, sk_create_automation, sk_domain,
        r_list_projects, r_list_tasks, r_list_tasks_filtered, r_get_task,
        r_get_plan, r_check_status, r_auth_setup, r_solve, r_update, r_automation,
    ]).await?;

    // ---- Step 6 (plan): Skill Amendments ----
    // Prepend Zencoder routing preambles to three existing builtin skills.
    // Idempotent: the UPDATE is guarded by `body NOT LIKE '%Zencoder Routing%'`.
    amend_builtin_skills(&s).await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Skill amendments (Step 6)
// ---------------------------------------------------------------------------

/// Prepend a one-paragraph Zencoder routing preamble to three existing
/// builtin skills:
///
/// - `skill-coding`         — check Zencoder task status before local edits
/// - `skill-commit-workflow` — block commits while a task is inprogress/inreview
/// - `skill-spawn-coding`   — route coding delegation to Zencoder
///
/// The preamble only references `skill-zencoder` by name; it does not duplicate
/// content. The guard `body NOT LIKE '%Zencoder Routing%'` makes the UPDATE
/// idempotent so re-seeding does not double-prepend.
async fn amend_builtin_skills(s: &ZencoderStores) -> Result<(), SeedBuiltinBootstrapError> {
    let client = s.pool.get().await.map_err(|e| SeedBuiltinBootstrapError::Pool { reason: e.to_string() })?;

    // Each amendment is a (skill_name, preamble) pair.
    let amendments: &[(&str, &str)] = &[
        (
            "skill-coding",
            AMEND_CODING,
        ),
        (
            "skill-commit-workflow",
            AMEND_COMMIT,
        ),
        (
            "skill-spawn-coding",
            AMEND_SPAWN_CODING,
        ),
    ];

    for (name, preamble) in amendments {
        client
            .execute(
                "UPDATE reborn_skills \
                 SET body = $1 || body \
                 WHERE name         = $2 \
                   AND tenant_id    = $3 \
                   AND user_id      = $4 \
                   AND agent_id     = $5 \
                   AND project_id   = $6 \
                   AND body NOT LIKE '%Zencoder Routing%'",
                &[
                    preamble,
                    name,
                    &s.tenant,
                    &SEED_USER,
                    &SEED_AGENT,
                    &SEED_PROJECT,
                ],
            )
            .await
            .map_err(|e| SeedBuiltinBootstrapError::Db { reason: format!("amend {name}: {e}") })?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Row builders
// ---------------------------------------------------------------------------

fn tool_zencoder_api_row(tenant: &str) -> NewPgTool {
    NewPgTool {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: "zencoder-api".to_string(),
        description: "Authenticated REST call to https://api.zencoder.ai/api/v1/. \
                       Bearer JWT injected from secret 'zencoder_access_token'. \
                       Methods: GET, POST, PATCH. Timeout: 30 000 ms. \
                       Returns raw JSON response body. \
                       Rate limit: 60 req/min, 1 000 req/hour (Zencoder-enforced)."
            .to_string(),
        param_schema: Some(json!({
            "type": "object",
            "properties": {
                "method": {"type": "string", "enum": ["GET","POST","PATCH"], "description": "HTTP method"},
                "path":   {"type": "string", "description": "API path under /api/v1/ (e.g. /projects)"},
                "body":   {"type": "string", "description": "JSON body string for POST/PATCH requests"}
            },
            "required": ["method", "path"]
        })),
        param_template: Some(json!({"method": "GET", "path": ""})),
        effect_type: "mixed".to_string(),
        preconditions: Some("Secret 'zencoder_access_token' must be set. 401 = token expired.".into()),
        error_handling: Some(
            "401→re-auth (brassclaw secret set zencoder_access_token <jwt>). \
             402→quota exceeded. 429→Retry-After header. 5xx→retry GETs ×3."
                .into(),
        ),
        consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into(), "05:validator".into()],
        source: "system".into(),
        validation_status: "validated".into(),
        capability_id: "builtin.http".into(),
    }
}

fn ts_row(
    tenant: &str,
    name: &str,
    description: &str,
    content: &str,
    path_template: &str,
    method: &str,
    params: Value,
) -> NewPgToolSkill {
    NewPgToolSkill {
        tenant_id: tenant.to_string(),
        user_id: SEED_USER.to_string(),
        agent_id: SEED_AGENT.to_string(),
        project_id: SEED_PROJECT.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        content: content.to_string(),
        prior_knowledge_content: None,
        override_prompt_creation: false,
        tool_name: Some("zencoder-api".to_string()),
        param_schema: Some(params),
        param_template: Some(json!({"method": method, "path": path_template})),
        consumer_tags: vec!["00:rusty".into(), "02:orchestrator".into()],
        intent_examples: None,
        source: "system".into(),
        validation_status: "validated".into(),
        includes: vec![],
    }
}

fn ts_projects_list_row(tenant: &str) -> NewPgToolSkill {
    ts_row(tenant, "ts-zencoder-projects-list",
        "GET /projects — list all accessible Zencoder projects.",
        "Call `host.zencoder_api(method='GET', path='/projects')` to list all accessible projects. \
         Returns a JSON array; extract `id` and `name` from each entry.",
        "/projects", "GET",
        json!([{"name": "method", "param_type": "string", "required": true, "description": "Must be GET"},
               {"name": "path",   "param_type": "string", "required": true, "description": "Must be /projects"}]))
}

fn ts_tasks_list_row(tenant: &str) -> NewPgToolSkill {
    ts_row(tenant, "ts-zencoder-tasks-list",
        "GET /projects/{pid}/tasks[?status=&limit=] — list tasks in a project.",
        "Call `host.zencoder_api(method='GET', path='/projects/{pid}/tasks')`. \
         Optional query params: status (todo|inprogress|inreview|done|cancelled), limit (integer). \
         Append them manually to the path string: /projects/{pid}/tasks?status={s}&limit={n}.",
        "/projects/{{vars.pid}}/tasks", "GET",
        json!([{"name": "method", "param_type": "string", "required": true},
               {"name": "path",   "param_type": "string", "required": true, "description": "Include optional ?status=&limit= query params"}]))
}

fn ts_tasks_get_row(tenant: &str) -> NewPgToolSkill {
    ts_row(tenant, "ts-zencoder-tasks-get",
        "GET /projects/{pid}/tasks/{tid} — fetch one task.",
        "Call `host.zencoder_api(method='GET', path='/projects/{pid}/tasks/{tid}')`. \
         Returns task object with status, description, branch.",
        "/projects/{{vars.pid}}/tasks/{{vars.tid}}", "GET",
        json!([{"name": "method", "param_type": "string", "required": true},
               {"name": "path",   "param_type": "string", "required": true}]))
}

fn ts_tasks_post_row(tenant: &str) -> NewPgToolSkill {
    ts_row(tenant, "ts-zencoder-tasks-post",
        "POST /projects/{pid}/tasks — create a task.",
        "Call `host.zencoder_api(method='POST', path='/projects/{pid}/tasks', body=<json>)`. \
         Body: {title, description?, workflow_id?, start?}. Returns created task object with 'id'.",
        "/projects/{{vars.pid}}/tasks", "POST",
        json!([{"name": "method", "param_type": "string", "required": true},
               {"name": "path",   "param_type": "string", "required": true},
               {"name": "body",   "param_type": "string", "required": true, "description": "JSON: {title, description?, workflow_id?, start?}"}]))
}

fn ts_tasks_patch_row(tenant: &str) -> NewPgToolSkill {
    ts_row(tenant, "ts-zencoder-tasks-patch",
        "PATCH /projects/{pid}/tasks/{tid} — partial update. Body replaces description fully.",
        "Call `host.zencoder_api(method='PATCH', path='/projects/{pid}/tasks/{tid}', body=<json>)`. \
         Body: any subset of {title, description, status}. \
         WARNING: PATCH fully replaces description — read the task first if appending. \
         Status values lowercase: todo|inprogress|inreview|done|cancelled.",
        "/projects/{{vars.pid}}/tasks/{{vars.tid}}", "PATCH",
        json!([{"name": "method", "param_type": "string", "required": true},
               {"name": "path",   "param_type": "string", "required": true},
               {"name": "body",   "param_type": "string", "required": true, "description": "JSON subset of {title, description, status}"}]))
}

fn ts_plan_get_row(tenant: &str) -> NewPgToolSkill {
    ts_row(tenant, "ts-zencoder-plan-get",
        "GET /projects/{pid}/tasks/{tid}/plan — fetch task execution plan.",
        "Call `host.zencoder_api(method='GET', path='/projects/{pid}/tasks/{tid}/plan')`. \
         Returns steps[{name, status}]. Status values: Pending|InProgress|Completed|Skipped. \
         404 = plan not yet created — not an error.",
        "/projects/{{vars.pid}}/tasks/{{vars.tid}}/plan", "GET",
        json!([{"name": "method", "param_type": "string", "required": true},
               {"name": "path",   "param_type": "string", "required": true}]))
}

fn ts_automations_list_row(tenant: &str) -> NewPgToolSkill {
    ts_row(tenant, "ts-zencoder-automations-list",
        "GET /automations[?enabled=true|false] — list automations.",
        "Call `host.zencoder_api(method='GET', path='/automations')`. \
         Optional: append ?enabled=true or ?enabled=false to filter by enabled status.",
        "/automations", "GET",
        json!([{"name": "method", "param_type": "string", "required": true},
               {"name": "path",   "param_type": "string", "required": true}]))
}

fn ts_automations_post_row(tenant: &str) -> NewPgToolSkill {
    ts_row(tenant, "ts-zencoder-automations-post",
        "POST /automations — create a scheduled automation.",
        "Call `host.zencoder_api(method='POST', path='/automations', body=<json>)`. \
         Body: {name, target_project_id?, task_name?, task_description?, \
         schedule_time? (HH:MM 24-hour), schedule_days_of_week? (int[] 0=Sun–6=Sat)}.",
        "/automations", "POST",
        json!([{"name": "method", "param_type": "string", "required": true},
               {"name": "path",   "param_type": "string", "required": true},
               {"name": "body",   "param_type": "string", "required": true,
                "description": "JSON: {name, target_project_id?, task_name?, task_description?, schedule_time?, schedule_days_of_week?}"}]))
}

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

fn leaf_skill(tenant: &str, name: &str, description: &str, body: &str) -> NewPgSkill {
    skill_row(tenant, name, description, body, 1, &["02:orchestrator", "05:validator"])
}

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
        validates_class_code: None,
    }
}

/// Build the `intent_examples` array from `(input, class)` pairs.
fn intents(pairs: &[(&str, u8)]) -> Vec<Value> {
    pairs.iter().map(|(input, cls)| json!({"input": input, "class": cls})).collect()
}

// ---------------------------------------------------------------------------
// PythonCode bodies
// ---------------------------------------------------------------------------

const PC_VALIDATE_UUID: &str = r#"# Pure-logic: validate slot0 as a UUID. Returns {valid, error?}. No host call.
import re as _re
_v = "{{vars.slot0}}"
_ok = bool(_re.match(
    r'^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$', _v, _re.I))
result = {"valid": _ok} if _ok else {"valid": False, "error": f"not a UUID: {_v!r}"}
"#;

const PC_BUILD_TASK_SUMMARY: &str = r#"# Pure-logic: merge task JSON (slot0) and plan JSON (slot1) into a summary dict.
import json as _j
_task = _j.loads("{{vars.slot0}}")
_plan = _j.loads("{{vars.slot1}}")
_d    = _task.get("data", _task)
_pd   = _plan.get("data", _plan)
_steps = _pd.get("steps", [])
_done  = sum(1 for s in _steps if s.get("status") == "Completed")
result = {
    "task_status": _d.get("status", "unknown"),
    "branch":      _d.get("branch"),
    "progress":    f"{_done} of {len(_steps)} steps completed",
    "plan_steps":  [{"name": s.get("name",""), "status": s.get("status","Pending")}
                    for s in _steps],
}
"#;

const PC_LIST_PROJECTS: &str = r#"# Executor: GET /projects.
result = host.zencoder_api(method="GET", path="/projects")
"#;

const PC_LIST_TASKS: &str = r#"# Executor: GET /projects/{pid}/tasks[?status&limit].
# slot0=project_id, slot1=status filter (empty=none), slot2=limit (empty=none)
import re as _re
_pid = "{{vars.slot0}}"
if not _re.match(r'^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$', _pid, _re.I):
    result = {"error": f"invalid project_id: {_pid!r}"}
else:
    _path = f"/projects/{_pid}/tasks"
    _q = []
    _s = "{{vars.slot1}}"
    _l = "{{vars.slot2}}"
    if _s in {"todo","inprogress","inreview","done","cancelled"}:
        _q.append(f"status={_s}")
    if str(_l).isdigit():
        _q.append(f"limit={_l}")
    if _q:
        _path += "?" + "&".join(_q)
    result = host.zencoder_api(method="GET", path=_path)
"#;

const PC_GET_TASK: &str = r#"# Executor: GET /projects/{pid}/tasks/{tid}.
# slot0=project_id, slot1=task_id
import re as _re
_u = _re.compile(r'^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$', _re.I)
_pid, _tid = "{{vars.slot0}}", "{{vars.slot1}}"
if not _u.match(_pid) or not _u.match(_tid):
    result = {"error": "invalid UUID"}
else:
    result = host.zencoder_api(method="GET", path=f"/projects/{_pid}/tasks/{_tid}")
"#;

const PC_GET_PLAN: &str = r#"# Executor: GET /projects/{pid}/tasks/{tid}/plan.
# slot0=project_id, slot1=task_id
import re as _re
_u = _re.compile(r'^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$', _re.I)
_pid, _tid = "{{vars.slot0}}", "{{vars.slot1}}"
if not _u.match(_pid) or not _u.match(_tid):
    result = {"error": "invalid UUID"}
else:
    result = host.zencoder_api(method="GET", path=f"/projects/{_pid}/tasks/{_tid}/plan")
"#;

const PC_CREATE_TASK: &str = r#"# Executor: POST /projects/{pid}/tasks.
# slot0=project_id, slot1=JSON body string
import re as _re
_u = _re.compile(r'^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$', _re.I)
_pid = "{{vars.slot0}}"
if not _u.match(_pid):
    result = {"error": "invalid project_id"}
else:
    result = host.zencoder_api(method="POST", path=f"/projects/{_pid}/tasks", body="{{vars.slot1}}")
"#;

const PC_PATCH_TASK: &str = r#"# Executor: PATCH /projects/{pid}/tasks/{tid}.
# slot0=project_id, slot1=task_id, slot2=JSON body string
import re as _re
_u = _re.compile(r'^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$', _re.I)
_pid, _tid = "{{vars.slot0}}", "{{vars.slot1}}"
if not _u.match(_pid) or not _u.match(_tid):
    result = {"error": "invalid UUID"}
else:
    result = host.zencoder_api(method="PATCH", path=f"/projects/{_pid}/tasks/{_tid}", body="{{vars.slot2}}")
"#;

const PC_LIST_AUTOMATIONS: &str = r#"# Executor: GET /automations[?enabled].
# slot0=enabled filter ("true"|"false"|"" for none)
_f = "{{vars.slot0}}"
_path = "/automations"
if _f in {"true", "false"}:
    _path += f"?enabled={_f}"
result = host.zencoder_api(method="GET", path=_path)
"#;

const PC_CREATE_AUTOMATION: &str = r#"# Executor: POST /automations.
# slot0=JSON body string (LLM-composed, user-confirmed)
import json as _j
_body = "{{vars.slot0}}"
if not _j.loads(_body).get("name","").strip():
    result = {"error": "name must not be empty"}
else:
    result = host.zencoder_api(method="POST", path="/automations", body=_body)
"#;

const PC_POST_AUTH_INSTRUCTIONS: &str = r#"# Posts fixed Zencoder auth setup instructions to chat via host.post_reply.
# No LLM needed — output is deterministic.
result = host.post_reply(answer=(
    "**Zencoder authentication setup**\n\n"
    "1. Run the auth script:\n"
    "   ```\n"
    "   scripts/zencoder-auth.sh\n"
    "   ```\n"
    "   (Windows: `scripts\\zencoder-auth.ps1`)\n\n"
    "2. Copy the JWT from the script output and store it:\n"
    "   ```\n"
    "   brassclaw secret set zencoder_access_token <jwt>\n"
    "   ```\n\n"
    "The token is valid for ~24 hours. Re-run the script when you see a 401 error."
))
"#;

// ---------------------------------------------------------------------------
// Skill bodies
// ---------------------------------------------------------------------------

const SKILL_AUTH_ERROR: &str = r#"What the Zencoder HTTP error codes mean and what to do:
  401 → JWT expired. Tell the user to re-run scripts/zencoder-auth.sh and update
        the secret: brassclaw secret set zencoder_access_token <new_jwt>
  402 → Quota or billing limit reached. Pause all Zencoder calls; wait for the
        user to confirm they have resolved it at auth.zencoder.ai.
  429 → Rate limit hit. Read the Retry-After header value; wait that many seconds; retry once.
  5xx → Transient server error. For GETs: retry up to 3 times with 1/2/4 turn backoff.
        For POST/PATCH: do not retry automatically — report and wait for user.
"#;

const SKILL_RESILIENCE: &str = r#"Three-state model to track across a conversation:
  healthy    — default; last call returned 2xx (or no call yet this session)
  degraded   — last call returned 401/402/429/5xx or a network error
  unavailable — tool returned "not found" / "not registered" (permanent this session)

When degraded or unavailable: skip ALL Zencoder calls for the current turn.
Fall through to native BrassClaw behavior. Tell the user once which fallback ran.
Probe to leave degraded: after 5 turns, or after the user confirms they fixed auth,
or after a different call succeeds. Never probe unavailable.
"#;

const SKILL_LIST_PROJECTS: &str = r#"Use pc-zencoder-list-projects. No parameters required.
Response is an array — extract id and name from each entry. If empty, tell the user
they have no accessible projects and link to auth.zencoder.ai to verify access.
"#;

const SKILL_LIST_TASKS: &str = r#"Use pc-zencoder-list-tasks with project_id (slot0), optional status (slot1),
optional limit (slot2). Status values are lowercase: todo, inprogress, inreview,
done, cancelled. If project_id is unknown, call skill-zencoder-list-projects first.
"#;

const SKILL_GET_TASK: &str = r#"Use pc-zencoder-get-task with project_id (slot0) and task_id (slot1).
Returns current status, description, and branch (if any).
Call this before any patch — PATCH fully replaces description.
"#;

const SKILL_GET_PLAN: &str = r#"Use pc-zencoder-get-plan with project_id (slot0) and task_id (slot1).
Returns steps[]. Step status is PascalCase: Pending|InProgress|Completed|Skipped.
404 from this call is normal for newly created tasks — not an error.
"#;

const SKILL_CREATE_TASK: &str = r#"Use pc-zencoder-create-task with project_id (slot0) and a JSON body (slot1).
Body must include title. For delegation, also include:
  workflow_id: "default-auto-workflow" and start: true.
The response contains the new task's id — retain it in context.
"#;

const SKILL_PATCH_TASK: &str = r#"Use pc-zencoder-patch-task with project_id (slot0), task_id (slot1),
and a JSON body (slot2). Body must include at least one of: title, description, status.
WARNING: PATCH fully replaces description. Always call skill-zencoder-get-task first
to read the current description before appending anything.
"#;

const SKILL_LIST_AUTOMATIONS: &str = r#"Use pc-zencoder-list-automations with optional enabled filter (slot0: "true"|"false"|"").
Returns all global automations. Empty array is valid.
"#;

const SKILL_CREATE_AUTOMATION: &str = r#"Use pc-zencoder-create-automation with a JSON body (slot0).
Required field: name. Optional: target_project_id, task_name, task_description,
task_workflow, schedule_time (HH:MM 24-hour), schedule_days_of_week (int array,
0=Sunday through 6=Saturday). Always confirm the full spec with the user before
dispatching — automation creation cannot be undone easily.
"#;

const SKILL_DOMAIN: &str = r#"Zencoder/Zenflow is a cloud service that runs multi-model AI coding agents
asynchronously. You delegate a coding problem → a remote pipeline executes on a
git branch → you poll for completion. It is a delegated AI worker, not an LLM provider.

WHEN TO USE ZENCODER (routing rules — healthy state only):
  1. task_id present in conversation → check solution status before any local edit
  2. user explicitly delegates coding work → create a task with start:true
  3. user commits + task is inprogress/inreview → block the commit, warn once
  4. plan-mode + task_id in scope → read the Zencoder plan instead of a local doc
  5. code-review + task_id in scope → read task first, then append findings via patch
  6. all other cases → native behavior, no Zencoder call

RESILIENCE: see skill-zencoder-resilience
AUTH ERRORS: see skill-zencoder-auth-error

LEAF SKILLS:
  skill-zencoder-list-projects        find which project to work in
  skill-zencoder-list-tasks           enumerate tasks, filter by status
  skill-zencoder-get-task             read one task before patching or reviewing
  skill-zencoder-get-plan             read the execution plan
  skill-zencoder-create-task          create and optionally start a task
  skill-zencoder-patch-task           update title, description, or status
  skill-zencoder-list-automations     list scheduled automations
  skill-zencoder-create-automation    schedule a recurring task
"#;

// ---------------------------------------------------------------------------
// YAML source strings (WebUI annotation — not parsed by IBS)
// ---------------------------------------------------------------------------

const YAML_LIST_PROJECTS: &str = r#"name: zencoder-list-projects
llm_call_required: false
steps:
  - channel: rust, ts-zencoder-projects-list
  - channel: orchestrator, pc-zencoder-list-projects
"#;

const YAML_LIST_TASKS: &str = r#"name: zencoder-list-tasks
llm_call_required: false
steps:
  - channel: rust, ts-zencoder-tasks-list
  - channel: orchestrator, pc-zencoder-list-tasks
"#;

const YAML_LIST_TASKS_FILTERED: &str = r#"name: zencoder-list-tasks-filtered
llm_call_required: false
variable_patterns: [slot1=status]
steps:
  - channel: rust, ts-zencoder-tasks-list
  - channel: orchestrator, pc-zencoder-list-tasks
"#;

const YAML_GET_TASK: &str = r#"name: zencoder-get-task
llm_call_required: false
variable_patterns: [slot0=project_id, slot1=task_id]
steps:
  - channel: rust, ts-zencoder-tasks-get
  - channel: orchestrator, pc-zencoder-get-task
"#;

const YAML_GET_PLAN: &str = r#"name: zencoder-get-plan
llm_call_required: false
variable_patterns: [slot0=project_id, slot1=task_id]
steps:
  - channel: rust, ts-zencoder-plan-get
  - channel: orchestrator, pc-zencoder-get-plan
"#;

const YAML_CHECK_STATUS: &str = r#"name: zencoder-check-solution-status
llm_call_required: false
variable_patterns: [slot0=project_id, slot1=task_id]
steps:
  - channel: rust, ts-zencoder-tasks-get + ts-zencoder-plan-get
  - channel: orchestrator, pc-zencoder-get-task
  - channel: orchestrator, pc-zencoder-get-plan
  - channel: orchestrator, pc-zencoder-build-task-summary
"#;

const YAML_AUTH_SETUP: &str = r#"name: zencoder-auth-setup
llm_call_required: false
steps:
  - channel: rust, ts-host-post-reply
  - channel: orchestrator, pc-zencoder-post-auth-instructions
"#;

const YAML_SOLVE: &str = r#"name: zencoder-solve-coding-problem
llm_call_required: true
steps:
  - channel: orchestrator (skill context), skill-zencoder + skill-zencoder-create-task + skill-zencoder-list-projects + skill-zencoder-auth-error
  - type: llm — compose task body, confirm project_id, present to user
  - channel: rust, ts-zencoder-tasks-post
  - channel: orchestrator, pc-zencoder-create-task
"#;

const YAML_UPDATE_TASK: &str = r#"name: zencoder-update-task
llm_call_required: true
steps:
  - channel: orchestrator (skill context), skill-zencoder-get-task + skill-zencoder-patch-task + skill-zencoder-auth-error
  - type: llm — read current task, compose PATCH body, confirm with user
  - channel: rust, ts-zencoder-tasks-patch
  - channel: orchestrator, pc-zencoder-patch-task
"#;

const YAML_CREATE_AUTOMATION: &str = r#"name: zencoder-create-automation
llm_call_required: true
steps:
  - channel: orchestrator (skill context), skill-zencoder-create-automation + skill-zencoder-auth-error
  - type: llm — collect details, validate schedule_time, present full spec for confirmation
  - channel: rust, ts-zencoder-automations-post
  - channel: orchestrator, pc-zencoder-create-automation
"#;

// ---------------------------------------------------------------------------
// ExtensionCatalogue overview doc
// ---------------------------------------------------------------------------

const CAT_OVERVIEW: &str = r#"# ext-zencoder — Zencoder AI Coding Agent Extension

Zencoder/Zenflow is a cloud service that runs multi-model AI coding agents asynchronously.
Delegate a coding problem → remote pipeline executes on a git branch → poll for completion.

## Authentication
Run `scripts/zencoder-auth.sh` (Windows: `scripts\zencoder-auth.ps1`), then:
  brassclaw secret set zencoder_access_token <jwt>
Token lifetime: ~24 hours.

## Task groups

### Delegation
- zencoder-solve-coding-problem — delegate a coding problem (Tier 1, LLM composes task)
- zencoder-check-solution-status — check task status + plan progress (Tier 0)

### Task management
- zencoder-list-projects — list accessible projects (Tier 0)
- zencoder-list-tasks — list all tasks in a project (Tier 0)
- zencoder-list-tasks-filtered — list tasks filtered by status (Tier 0)
- zencoder-get-task — read a single task (Tier 0)
- zencoder-update-task — patch title/description/status (Tier 1, LLM composes patch)

### Plans
- zencoder-get-plan — read the execution plan for a task (Tier 0)

### Automations
- zencoder-list-automations — list scheduled automations (Tier 0 — via builtin list pattern)
- zencoder-create-automation — create a scheduled automation (Tier 1, LLM collects details)

### Setup
- zencoder-auth-setup — display authentication instructions (Tier 0)
"#;

// ---------------------------------------------------------------------------
// Skill amendment preamble bodies (Step 6)
// ---------------------------------------------------------------------------

/// Prepended to `skill-coding`.
const AMEND_CODING: &str = r#"## Zencoder Routing (see skill-zencoder for full rules)
If task_id is present in conversation → call zencoder-check-solution-status before
editing locally. If the user explicitly delegates coding work → call
zencoder-solve-coding-problem to create a Zencoder task. Otherwise → native coding
behavior below.
---

"#;

/// Prepended to `skill-commit-workflow`.
const AMEND_COMMIT: &str = r#"## Zencoder Routing (see skill-zencoder for full rules)
If task_id is in context AND the Zencoder task status is inprogress or inreview →
block the commit and warn the user once. Otherwise → native commit behavior below.
---

"#;

/// Prepended to `skill-spawn-coding`.
const AMEND_SPAWN_CODING: &str = r#"## Zencoder Routing (see skill-zencoder for full rules)
Coding delegation (code / file / function / API / test / build / refactor / bug /
PR / branch) → prefer zencoder-solve-coding-problem to delegate to the remote
Zencoder pipeline. Non-coding tasks → native spawn-coding delegation below.
---

"#;
