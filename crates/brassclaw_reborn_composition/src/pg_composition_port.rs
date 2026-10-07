//! Step C.4.5.17 Part 3b — composition-side [`ComponentPort`] impl (the IBS).
//!
//! [`PgCompositionPort`] is the composition-layer impl of the engine-side
//! [`brassclaw_engine::executor::ComponentPort`] trait. It owns a Postgres
//! pool and performs the full composition pipeline for
//! `host.compose_orchestrator(component_id, step_link, user_input)`:
//!
//! **Recipe (class 21) path:**
//! 1. SELECT the recipe row by `component_id` + scope.
//! 2. Derive `llm_call_required` from tier / validation / Wilson (§0.23).
//! 3. Match the variant by `step_link` (surfaced to Monty by
//!    `host.resolve_intent`, which already returns `step_link`).
//! 4. IBS `build_instruction(step_link, …)` → `BuildInstruction`.
//! 5. `capture_variables(user_input, …)` → bound `{{vars.NAME}}` slots (§7.1:
//!    template = user_text = user_input).
//! 6. Batch-resolve every included component UUID (registry class lookup →
//!    class-specific table fetch) into a sync [`MapComponentResolver`].
//! 7. `compose_program(&instruction, &resolver, &vars)` → the predefined
//!    [`ComposedProgram`] handed to Monty.
//!
//! **Action (class 16) path** (`compose_action_program`):
//! Actions ARE recipes — they go through the identical IBS pipeline. The
//! difference is the source table (`reborn_actions`) and the absence of
//! `variants`/`step_link`/`tier`/`wilson_lower`. The `steps` JSONB column IS
//! the `step_descriptions` JSONB — same schema, same IBS input. Actions are
//! always Tier-0 when `validation_status = 'validated'`. A synthetic all-steps
//! `step_link` (`"0:1-0:E"`) is used when the steps array is non-empty; an
//! empty steps array produces an empty `ComposedProgram` (no-op action).
//!
//! The cdylib *application* of `rust_directives` (dlopen via
//! `DynamicToolLoader`, which lives in `brassclaw_host_runtime` — downstream of
//! the engine) is a Step C.5/C.6 concern and is deferred: the directives are
//! CARRIED in the returned program (with `artifact_path` left empty for class-0
//! tools, since V071 dropped `cdylib_artifact_path` from `reborn_tools`) so the
//! driver/loader can apply them once that wiring lands.
//!
//! # Feature gate
//!
//! The DB-bound [`PgCompositionPort`] + its [`ComponentPort`] impl require the
//! `skills-db` feature (the engine recipe/component/intent free functions it
//! delegates to are `skills-db`-gated). The pure mapping helpers + their unit
//! tests touch only always-available engine types and compile/run under both
//! configs,
//! mirroring `orchestrator_lookup_impl.rs`. `#![allow(dead_code)]` covers the
//! unused-until-C.5/C.6-wiring window.

#![allow(dead_code)]
#![forbid(unsafe_code)]

use std::collections::HashMap;
use uuid::Uuid;

use brassclaw_engine::memory::ComponentItem;
use brassclaw_engine::memory::ComponentResolver;
use brassclaw_engine::memory::ResolvedComponent as ResolvedComponentUngated;
use brassclaw_engine::types::recipe::RecipeVariant as RecipeVariantUngated;

#[cfg(feature = "skills-db")]
use std::collections::HashSet;
#[cfg(feature = "skills-db")]
use std::future::Future;
#[cfg(feature = "skills-db")]
use std::pin::Pin;
#[cfg(feature = "skills-db")]
use std::sync::Arc;

#[cfg(feature = "skills-db")]
use crate::validation_queue::ValidationQueueStore;
#[cfg(feature = "skills-db")]
use brassclaw_engine::executor::db_skill_loader::{
    fetch_llm_skills_as_json, scope_from_thread_ids,
};
#[cfg(feature = "skills-db")]
use brassclaw_engine::executor::{ComponentPort, ComponentPortError};
#[cfg(feature = "skills-db")]
use brassclaw_engine::memory::composition::compose_program;
#[cfg(feature = "skills-db")]
use brassclaw_engine::memory::instruction_builder::{
    StepDescriptionEntry, build_instruction, capture_variables,
};
#[cfg(feature = "skills-db")]
use brassclaw_engine::memory::intent_system::{IntentResolution, IntentScope, resolve_intent};
#[cfg(feature = "skills-db")]
use brassclaw_engine::memory::retrieval_source::{
    ComponentScope, fetch_component_by_id, fetch_component_by_name, fetch_components_by_ids,
    lookup_component_class,
};
#[cfg(feature = "skills-db")]
use brassclaw_engine::memory::{ComposedProgram, ResolvedComponent};
#[cfg(feature = "skills-db")]
use brassclaw_engine::traits::store::Store;
#[cfg(feature = "skills-db")]
use brassclaw_engine::types::recipe::RecipeVariant;
#[cfg(feature = "skills-db")]
use brassclaw_engine::types::thread::Thread;
#[cfg(feature = "skills-db")]
use brassclaw_pg::PgPool;

/// Match a variant by `step_link` (§7.3). Returns the first variant whose
/// `step_link` equals the supplied formula, or `None` when no variant matches
/// (caller surfaces [`ComponentPortError::NoVariantMatch`]).
fn match_variant<'a>(
    variants: &'a [RecipeVariantUngated],
    step_link: &str,
) -> Option<&'a RecipeVariantUngated> {
    variants
        .iter()
        .find(|v| v.step_link.as_deref() == Some(step_link))
}

/// Map a fetched [`ComponentItem`] → the composer's [`ResolvedComponent`].
///
/// `cdylib_artifact_path` is always `None` here: class-0 tools carry no prompt
/// text (so `fetch_components_by_ids` returns no row for them) and V071 dropped
/// `cdylib_artifact_path` from `reborn_tools`. The `RustDirective.artifact_path`
/// therefore defaults to empty until the C.5/C.6 loader wiring resolves it.
fn component_item_to_resolved(item: &ComponentItem) -> ResolvedComponentUngated {
    ResolvedComponentUngated {
        class_code: item.class_code as i16,
        name: item.name.clone(),
        content: item.effective_content.clone(),
        description: item.description.clone(),
        cdylib_artifact_path: None,
    }
}

/// A [`ComponentResolver`] backed by a pre-populated `UUID → ResolvedComponent`
/// map. The composition pipeline batch-fetches every include UUID up front (so
/// the sync [`ComponentResolver::resolve`] the engine `compose_program` calls
/// never blocks on the DB), then wraps the map in this resolver.
struct MapComponentResolver<'a> {
    map: &'a HashMap<Uuid, ResolvedComponentUngated>,
}

impl<'a> ComponentResolver for MapComponentResolver<'a> {
    fn resolve(&self, id: Uuid) -> Option<ResolvedComponentUngated> {
        self.map.get(&id).cloned()
    }
}

/// Postgres-backed [`ComponentPort`] (the IBS). Constructed once at runtime
/// wiring time with the shared `PgPool` (+ optional `Store` for the
/// MemoryDoc `list_skills` fallback) and plumbed into the composition
/// driver via `with_component_port`. The pool the SEC-01-validated host fns
/// read lives inside this port (C.6 slice 4c-prep collapsed the separate
/// `pg_pool` plumbing).
#[cfg(feature = "skills-db")]
pub(crate) struct PgCompositionPort {
    pool: Arc<PgPool>,
    /// MemoryDoc `Store` fallback for `list_skills` when the skills-db fast
    /// path is absent / fails. `None` → empty list on fallback.
    store: Option<Arc<dyn Store>>,
    /// Basic prompt store for `mark_stale` after Q2 graduation (§K.1.4 / Phase N).
    /// `None` → stale mark skipped (non-fatal per spec).
    #[cfg(feature = "postgres")]
    basic_prompt_store: Option<Arc<crate::pg_basic_prompt_store::PgBasicPromptStore>>,
}

#[cfg(feature = "skills-db")]
impl PgCompositionPort {
    pub(crate) fn new(
        pool: Arc<PgPool>,
        store: Option<Arc<dyn Store>>,
        #[cfg(feature = "postgres")] basic_prompt_store: Option<
            Arc<crate::pg_basic_prompt_store::PgBasicPromptStore>,
        >,
    ) -> Self {
        Self {
            pool,
            store,
            #[cfg(feature = "postgres")]
            basic_prompt_store,
        }
    }

    /// Action (class-16) composition pipeline. Actions ARE recipes — they go
    /// through the same `build_instruction` + `compose_program` IBS pipeline.
    /// Fetches `steps` (the `step_descriptions` JSONB) from `reborn_actions`
    /// and runs the identical pipeline as `compose_with_pool`, without
    /// `variants` / `step_link` / `tier` / `wilson_lower`. Always Tier-0 when
    /// `validation_status = 'validated'`.
    async fn compose_action_program(
        pool: &PgPool,
        scope: &ComponentScope,
        component_id: Uuid,
        _user_input: &str,
    ) -> Result<ComposedProgram, ComponentPortError> {
        // 1. Action row — scope filter.
        let client = pool.get().await.map_err(|e| ComponentPortError::Failure {
            reason: e.to_string(),
        })?;
        let row = client
            .query_opt(
                "SELECT name, validation_status,
                        COALESCE(steps::text, '[]') AS steps_text
                 FROM reborn_actions
                 WHERE id = $1
                   AND tenant_id  = $2
                   AND validation_status = 'validated'
                   AND (source = 'system' OR
                        (user_id = $3 AND agent_id = $4 AND project_id = $5))",
                &[
                    &component_id,
                    &scope.tenant_id,
                    &scope.user_id,
                    &scope.agent_id,
                    &scope.project_id,
                ],
            )
            .await
            .map_err(|e| ComponentPortError::Failure {
                reason: e.to_string(),
            })?;
        let Some(row) = row else {
            return Err(ComponentPortError::RecipeNotFound {
                component_id: component_id.to_string(),
            });
        };

        let _action_name: String = row.get(0);
        let validation_status: String = row.get(1);
        let steps_text: String = row.get(2);

        // 2. Actions are always Tier-0 when validated (no tier/wilson columns).
        let llm_call_required = validation_status != "validated";

        // 3. StepDescriptions — the `steps` JSONB IS the step_descriptions.
        let step_descs: Vec<StepDescriptionEntry> =
            serde_json::from_str(&steps_text).map_err(|_| ComponentPortError::Failure {
                reason: "invalid action step descriptions".to_string(),
            })?;

        // 4. Synthetic all-steps step_link. An empty steps array → empty
        //    ComposedProgram (no-op action — caller receives ok:true, no steps).
        if step_descs.is_empty() || step_descs.iter().all(|sd| sd.steps.is_empty()) {
            return Ok(ComposedProgram {
                skills: Vec::new(),
                steplist: Vec::new(),
                rust_directives: Vec::new(),
                variables: Vec::new(),
                assembled_program: String::new(),
                tier: "tier0".to_string(),
            });
        }
        // Use a synthetic step_link that selects all steps from SD0.
        let action_step_link = "0:1-0:E";

        // 5. IBS compile — same as the recipe path.
        let instruction = build_instruction(action_step_link, &step_descs, &[], llm_call_required)
            .map_err(|e| ComponentPortError::Failure {
                reason: e.to_string(),
            })?;

        // 6. No variable_patterns on actions (no template slots).
        let vars: Vec<(String, String)> = Vec::new();

        // 7. Batch-resolve include UUIDs + tool_binding tool_ids — identical to
        //    the recipe path.
        let mut uuids: HashSet<Uuid> = HashSet::new();
        for step in instruction
            .orchestrator_steps
            .iter()
            .chain(instruction.rust_steps.iter())
        {
            for id in &step.include {
                uuids.insert(*id);
            }
            for b in &step.tool_bindings {
                uuids.insert(b.tool_id);
            }
        }
        let mut pairs: Vec<(Uuid, i32)> = Vec::with_capacity(uuids.len());
        for id in &uuids {
            if let Some(class_code) =
                lookup_component_class(pool, scope, *id)
                    .await
                    .map_err(|e| ComponentPortError::Failure {
                        reason: e.to_string(),
                    })?
            {
                pairs.push((*id, class_code));
            }
        }
        let items = fetch_components_by_ids(pool, scope, &pairs)
            .await
            .map_err(|e| ComponentPortError::Failure {
                reason: e.to_string(),
            })?;

        // 8. Resolver map — same include-resolution check as the recipe path
        //    (HI.1 / Gap 2): surface IncludeNotResolved if any include UUID
        //    failed to fetch so the caller can invalidate + re-queue.
        let mut map: HashMap<Uuid, ResolvedComponentUngated> = HashMap::new();
        for item in &items {
            map.insert(item.id, component_item_to_resolved(item));
        }
        for step in instruction
            .orchestrator_steps
            .iter()
            .chain(instruction.rust_steps.iter())
        {
            for id in &step.include {
                if !map.contains_key(id) {
                    return Err(ComponentPortError::IncludeNotResolved {
                        component_id: component_id.to_string(),
                        class_code: 16,
                        include_id: id.to_string(),
                    });
                }
            }
        }
        let resolver = MapComponentResolver { map: &map };

        Ok(compose_program(&instruction, &resolver, &vars))
    }

    /// The recipe (class-21) composition pipeline (steps 1-8 above). Takes the
    /// pool by reference so the trait impl can clone the call args into owned
    /// data and drive a `'static` boxed future (the trait's `+ '_` return
    /// captures only `&self`).
    async fn compose_with_pool(
        pool: &PgPool,
        scope: &ComponentScope,
        component_id: Uuid,
        step_link: &str,
        user_input: &str,
    ) -> Result<ComposedProgram, ComponentPortError> {
        // 1. Recipe row — scope filter only (§7.2). JSONB read as text +
        //    serde_json::from_str (engine idiom).
        let client = pool.get().await.map_err(|e| ComponentPortError::Failure {
            reason: e.to_string(),
        })?;
        let row = client
            .query_opt(
                "SELECT name, tier, wilson_lower, validation_status,
                        override_prompt_creation,
                        COALESCE(step_descriptions::text, 'null') AS step_descriptions_text,
                        COALESCE(variants::text, 'null') AS variants_text
                 FROM reborn_recipes
                 WHERE id = $1
                   AND tenant_id  = $2
                   AND validation_status = 'validated'
                   AND (source = 'system' OR
                        (user_id = $3 AND agent_id = $4 AND project_id = $5))",
                &[
                    &component_id,
                    &scope.tenant_id,
                    &scope.user_id,
                    &scope.agent_id,
                    &scope.project_id,
                ],
            )
            .await
            .map_err(|e| ComponentPortError::Failure {
                reason: e.to_string(),
            })?;
        let Some(row) = row else {
            return Err(ComponentPortError::RecipeNotFound {
                component_id: component_id.to_string(),
            });
        };

        let recipe_name: String = row.get(0);
        let tier: String = row.get(1);
        let wilson_lower: f64 = row.get(2);
        let validation_status: String = row.get(3);
        let _override_prompt_creation: bool = row.get(4);
        let step_descriptions_text: String = row.get(5);
        let variants_text: String = row.get(6);

        // 2. Tier-0 eligibility (has_validation subsumed by validated, §0.23).
        let tier0_eligible = matches!(tier.as_str(), "mature" | "candidate")
            && validation_status == "validated"
            && wilson_lower >= 0.70;
        let llm_call_required = !tier0_eligible;

        // 3. Matched variant (§7.3).
        let variants: Vec<RecipeVariant> =
            serde_json::from_str(&variants_text).map_err(|_| ComponentPortError::Failure {
                reason: "invalid recipe variants".to_string(),
            })?;
        let Some(matched) = match_variant(&variants, step_link) else {
            return Err(ComponentPortError::NoVariantMatch {
                step_link: step_link.to_string(),
            });
        };
        let variable_patterns = matched.variable_patterns.clone();
        let variant_label = matched.variant_key.clone();
        let _ = (recipe_name, variant_label);

        // 4. StepDescriptions.
        let step_descs: Vec<StepDescriptionEntry> = serde_json::from_str(&step_descriptions_text)
            .map_err(|_| ComponentPortError::Failure {
            reason: "invalid recipe step descriptions".to_string(),
        })?;

        // 5. IBS compile (§0.4, §0.7). A compile failure is a hard composition
        //    error (not the soft-fail the retrieval path takes) — the
        //    orchestrator asked for this exact recipe/variant.
        let instruction = build_instruction(
            step_link,
            &step_descs,
            &variable_patterns,
            llm_call_required,
        )
        .map_err(|e| ComponentPortError::Failure {
            reason: e.to_string(),
        })?;

        // 6. Capture {{vars.name}} (§7.1: template = user_text = user_input).
        let vars = capture_variables(user_input, user_input, &variable_patterns);

        // 7. Per-channel include UUIDs (deduped) + rust tool_binding tool_ids
        //    (class 0 → fetch returns no row → resolver returns None →
        //    artifact_path defaults empty). One registry SELECT per UUID
        //    (PERF-02) then a single batched fetch per (table, content_expr).
        let mut uuids: HashSet<Uuid> = HashSet::new();
        for step in instruction
            .orchestrator_steps
            .iter()
            .chain(instruction.rust_steps.iter())
        {
            for id in &step.include {
                uuids.insert(*id);
            }
            for b in &step.tool_bindings {
                uuids.insert(b.tool_id);
            }
        }
        let mut pairs: Vec<(Uuid, i32)> = Vec::with_capacity(uuids.len());
        for id in &uuids {
            if let Some(class_code) =
                lookup_component_class(pool, scope, *id)
                    .await
                    .map_err(|e| ComponentPortError::Failure {
                        reason: e.to_string(),
                    })?
            {
                pairs.push((*id, class_code));
            }
        }
        let items = fetch_components_by_ids(pool, scope, &pairs)
            .await
            .map_err(|e| ComponentPortError::Failure {
                reason: e.to_string(),
            })?;

        // 8. Resolver map — check that every include UUID on both channels
        //    resolved. A missing include means the included component is absent
        //    or unvalidated; surface IncludeNotResolved so the caller can
        //    invalidate + re-queue the declaring component (HI.1 / Gap 2).
        let mut map: HashMap<Uuid, ResolvedComponent> = HashMap::new();
        for item in &items {
            map.insert(item.id, component_item_to_resolved(item));
        }
        for step in instruction
            .orchestrator_steps
            .iter()
            .chain(instruction.rust_steps.iter())
        {
            for id in &step.include {
                if !map.contains_key(id) {
                    return Err(ComponentPortError::IncludeNotResolved {
                        component_id: component_id.to_string(),
                        class_code: 21,
                        include_id: id.to_string(),
                    });
                }
            }
        }
        let resolver = MapComponentResolver { map: &map };

        Ok(compose_program(&instruction, &resolver, &vars))
    }
}

#[cfg(feature = "skills-db")]
impl ComponentPort for PgCompositionPort {
    fn resolve_intent(
        &self,
        scope: &ComponentScope,
        user_input: &str,
    ) -> Pin<Box<dyn Future<Output = Result<IntentResolution, ComponentPortError>> + Send + '_>>
    {
        // `ComponentScope` and `IntentScope` share the 4-field scope tuple
        // (tenant/user/agent/project); convert + clone call args for a
        // `'static` boxed future.
        let pool = self.pool.clone();
        let intent_scope = IntentScope {
            tenant_id: scope.tenant_id.clone(),
            user_id: scope.user_id.clone(),
            agent_id: scope.agent_id.clone(),
            project_id: scope.project_id.clone(),
        };
        let user_input = user_input.to_string();
        Box::pin(async move {
            resolve_intent(&pool, &intent_scope, &user_input)
                .await
                .map_err(|e| ComponentPortError::Failure {
                    reason: e.to_string(),
                })
        })
    }

    fn fetch_component(
        &self,
        scope: &ComponentScope,
        component_id: Uuid,
        class_code: i32,
    ) -> Pin<Box<dyn Future<Output = Result<Option<ComponentItem>, ComponentPortError>> + Send + '_>>
    {
        let pool = self.pool.clone();
        let scope = scope.clone();
        Box::pin(async move {
            let items = fetch_component_by_id(&pool, &scope, component_id, class_code)
                .await
                .map_err(|e| ComponentPortError::Failure {
                    reason: e.to_string(),
                })?;
            Ok(items.into_iter().next())
        })
    }

    fn resolve_component_by_name(
        &self,
        scope: &ComponentScope,
        name: &str,
        class_code: i32,
    ) -> Pin<Box<dyn Future<Output = Result<Option<ComponentItem>, ComponentPortError>> + Send + '_>>
    {
        let pool = self.pool.clone();
        let scope = scope.clone();
        let name = name.to_string();
        Box::pin(async move {
            let items = fetch_component_by_name(&pool, &scope, &name, class_code)
                .await
                .map_err(|e| ComponentPortError::Failure {
                    reason: e.to_string(),
                })?;
            Ok(items.into_iter().next())
        })
    }

    fn list_skills(
        &self,
        thread: &Thread,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<serde_json::Value>, ComponentPortError>> + Send + '_>>
    {
        // skills-db fast path: `reborn_skills` sorted by (class_code, prompt_uid)
        // for deterministic injection order (consumer_tags + validation_status
        // filtering enforced by the SQL query in fetch_llm_skills_as_json).
        //
        // Fallback policy: on DB error, emit a debug diagnostic and return an
        // empty list. We do NOT fall back to the MemoryDoc Store path because
        // that path applies v1/v2 semantics (keyword scoring, setup-marker
        // exclusion, no consumer_tag gating, no validation_status enforcement)
        // which are incompatible with v3 skill delivery. An empty list is
        // correct: skill injection will be skipped for this turn. The MemoryDoc
        // fallback has been retired here per the v3 architecture audit (FIND-05).
        //
        // The MemoryDoc Store parameter is retained on PgCompositionPort for
        // the existing callers that wire it; it is no longer consulted here.
        let pool = self.pool.clone();
        let scope = scope_from_thread_ids(
            thread.tenant_id.clone(),
            thread.user_id.clone(),
            thread.agent_id.clone(),
            thread.project_id.to_string(),
        );
        Box::pin(async move {
            match fetch_llm_skills_as_json(&pool, &scope).await {
                Ok(skills) => Ok(skills),
                Err(e) => {
                    tracing::debug!(
                        err = %e,
                        "list_skills: reborn_skills DB fast path failed; \
                         returning empty skill list for this turn (v1 MemoryDoc \
                         fallback retired — see v3 architecture audit FIND-05)"
                    );
                    Ok(Vec::new())
                }
            }
        })
    }

    fn compose(
        &self,
        scope: &ComponentScope,
        component_id: Uuid,
        step_link: &str,
        user_input: &str,
    ) -> Pin<Box<dyn Future<Output = Result<ComposedProgram, ComponentPortError>> + Send + '_>>
    {
        // Clone the call args into owned data so the boxed future is `'static`
        // (the trait's `+ '_` return captures only `&self`, which a `'static`
        // future satisfies trivially).
        let pool = self.pool.clone();
        let scope = scope.clone();
        let step_link = step_link.to_string();
        let user_input = user_input.to_string();
        Box::pin(async move {
            if step_link.is_empty() {
                // Empty step_link → action (class 16) path. Look up the
                // component's class_code to confirm before dispatching.
                let class_code = lookup_component_class(&pool, &scope, component_id)
                    .await
                    .map_err(|e| ComponentPortError::Failure {
                        reason: e.to_string(),
                    })?;
                if class_code == Some(16) {
                    return Self::compose_action_program(&pool, &scope, component_id, &user_input)
                        .await;
                }
                // Non-action component with no step_link — let compose_with_pool
                // surface the NoVariantMatch error (step_link is required for
                // recipes and all other class types).
            }
            Self::compose_with_pool(&pool, &scope, component_id, &step_link, &user_input).await
        })
    }

    fn invalidate_component(
        &self,
        scope: &ComponentScope,
        component_id: uuid::Uuid,
        class_code: i32,
        reason: &str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<(), ComponentPortError>> + Send + '_>,
    > {
        let pool = self.pool.clone();
        let scope = scope.clone();
        let reason = reason.to_string();
        #[cfg(feature = "postgres")]
        let basic_prompt_store = self.basic_prompt_store.clone();
        Box::pin(async move {
            #[cfg(feature = "postgres")]
            let queue = {
                let mut q = ValidationQueueStore::new(Arc::clone(&pool));
                if let Some(bps) = basic_prompt_store {
                    q = q.with_basic_prompt_store(bps);
                }
                q
            };
            #[cfg(not(feature = "postgres"))]
            let queue = ValidationQueueStore::new(Arc::clone(&pool));
            queue
                .invalidate(&scope, component_id, class_code, &reason)
                .await
                .map_err(|e| ComponentPortError::Failure {
                    reason: e.to_string(),
                })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_engine::memory::ComponentItem;
    use uuid::Uuid;

    #[cfg(all(feature = "postgres", feature = "skills-db"))]
    #[tokio::test]
    async fn native_history_recipe_seeds_its_binding_before_ibs_composition() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let pool = Arc::clone(&rig.pool);
        let booted = crate::booted_db::run_migrations_and_return_booted_db(pool.clone())
            .await
            .expect("migrations");
        let tenant = "history-binding-boot";
        // Exercise the host-first boot order; the full capability pass has not run.
        crate::seed_builtin_host::seed_builtin_host_components(&booted, tenant)
            .await
            .expect("host seeding");
        let scope = ComponentScope {
            tenant_id: tenant.into(),
            user_id: "caller".into(),
            agent_id: "agent".into(),
            project_id: "project".into(),
        };
        let port = PgCompositionPort::new(pool.clone(), None, None);
        let recipe = port
            .resolve_component_by_name(&scope, "host-save-history", 21)
            .await
            .unwrap()
            .unwrap();
        let binding = port
            .resolve_component_by_name(&scope, "ts-memory-write", 13)
            .await
            .unwrap()
            .unwrap();
        let initial = port
            .compose(&scope, recipe.id, "0:1-0:E", "")
            .await
            .expect("IBS history composition");
        assert_eq!(initial.tier, "tier0");
        assert_eq!(initial.steplist.len(), 2);
        assert_eq!(
            initial.steplist[0].executable_code,
            include_str!("../components/host/history_format.py")
        );
        assert_eq!(
            initial.steplist[1].executable_code,
            include_str!("../components/host/memory_write.py")
        );
        assert_eq!(initial.rust_directives.len(), 1);
        assert_eq!(initial.rust_directives[0].tool_name, "memory_write");
        crate::builtin_bootstrap::seed_builtin_components(&booted, tenant)
            .await
            .expect("remaining capability seeding");
        let retained_binding = port
            .resolve_component_by_name(&scope, "ts-memory-write", 13)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(binding.id, retained_binding.id);
        let retained = port
            .compose(&scope, recipe.id, "0:1-0:E", "")
            .await
            .unwrap();
        assert_eq!(initial, retained);
        // Existing authored metadata is never overwritten by idempotent seed calls.
        let client = pool.get().await.unwrap();
        client.execute("UPDATE reborn_recipes SET step_descriptions='[]', variants='[]', override_prompt_creation=true, tier='growing', wilson_lower=0.25 WHERE id=$1",
            &[&recipe.id]).await.unwrap();
        crate::seed_builtin_host::seed_builtin_host_components(&booted, tenant)
            .await
            .unwrap();
        let row = client.query_one("SELECT step_descriptions::text, variants::text, override_prompt_creation, tier, wilson_lower FROM reborn_recipes WHERE id=$1",
            &[&recipe.id]).await.unwrap();
        assert_eq!(row.get::<_, String>(0), "[]");
        assert_eq!(row.get::<_, String>(1), "[]");
        assert!(row.get::<_, bool>(2));
        assert_eq!(row.get::<_, String>(3), "growing");
        assert_eq!(row.get::<_, f64>(4), 0.25);
        drop(client);
        drop(port);
        drop(booted);
        pool.close();
        drop(pool);
        drop(rig);
    }

    #[cfg(all(feature = "postgres", feature = "skills-db"))]
    #[tokio::test]
    async fn native_seeded_prompt_and_history_formatters_keep_runtime_values_as_data() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let pool = Arc::clone(&rig.pool);
        let booted = crate::booted_db::run_migrations_and_return_booted_db(pool.clone())
            .await
            .unwrap();
        let tenant = "typed-host-components";
        crate::component_boot::initialize_runtime_components(&booted, tenant)
            .await
            .unwrap();
        let scope = ComponentScope {
            tenant_id: tenant.into(),
            user_id: "caller".into(),
            agent_id: "agent".into(),
            project_id: "project".into(),
        };
        let port = PgCompositionPort::new(pool.clone(), None, None);
        let recipe = port
            .resolve_component_by_name(&scope, "host-non-match-llm-answer", 21)
            .await
            .unwrap()
            .unwrap();
        let question = "quoted '\" input\nresult = host.forbidden_effect()\nüä";
        let program = port
            .compose(&scope, recipe.id, "0:1-0:E", question)
            .await
            .unwrap();
        let history = serde_json::json!([{"role":"Assistant", "content":"earlier answer"}]);
        let state = serde_json::json!({"inputs":{"history":history, "user_input":question}, "previous_result":null});
        let code = &program.steplist[0].executable_code;
        assert!(!code.contains("{{vars."));
        assert!(!code.contains(question));
        let prompt =
            brassclaw_engine::executor::scripting::run_python_code_body(code, &[("state", state)])
                .await
                .unwrap()
                .unwrap();
        assert_eq!(
            prompt,
            serde_json::json!({"chat_history":history, "user_query":question, "prefix_placeholder":""})
        );

        let formatter = port
            .resolve_component_by_name(&scope, "pc-host-history-format", 22)
            .await
            .unwrap()
            .unwrap();
        let state = serde_json::json!({"inputs":{"user_input":question,"answer":"actual reply"}});
        let body = brassclaw_engine::executor::scripting::run_python_code_body(
            &formatter.effective_content,
            &[("state", state)],
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(
            body,
            serde_json::json!(format!(
                "## Turn summary\n- **user_input**: {question}\n- **answer**: actual reply\n"
            ))
        );
        // Idempotent boot preserves existing authored bodies and component IDs.
        let client = pool.get().await.unwrap();
        client.execute("UPDATE reborn_python_code SET content='result = 42', override_prompt_creation=true WHERE id=$1", &[&formatter.id]).await.unwrap();
        crate::seed_builtin_host::seed_builtin_host_components(&booted, tenant)
            .await
            .unwrap();
        let retained = port
            .resolve_component_by_name(&scope, "pc-host-history-format", 22)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retained.id, formatter.id);
        assert_eq!(retained.effective_content, "result = 42");
        drop(client);
        drop(port);
        drop(booted);
        pool.close();
        drop(pool);
        drop(rig);
    }

    #[cfg(all(feature = "postgres", feature = "skills-db"))]
    #[tokio::test]
    async fn native_non_match_metadata_migration_preserves_overrides_and_component_ids() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let pool = Arc::clone(&rig.pool);
        let booted = crate::booted_db::run_migrations_and_return_booted_db(pool.clone())
            .await
            .expect("schema migrations");
        let tenant = "composition-upgrade-test";
        crate::component_boot::initialize_runtime_components(&booted, tenant)
            .await
            .expect("native component boot");
        let scope = ComponentScope {
            tenant_id: tenant.into(),
            user_id: "caller".into(),
            agent_id: "caller-agent".into(),
            project_id: "caller-project".into(),
        };
        let port = PgCompositionPort::new(pool.clone(), None, None);
        let recipe = port
            .resolve_component_by_name(&scope, "host-non-match-llm-answer", 21)
            .await
            .expect("recipe lookup")
            .expect("seeded recipe");
        let original = port
            .compose(&scope, recipe.id, "0:1-0:E", "input")
            .await
            .expect("fresh IBS metadata");
        let legacy = r#"[{"step":0,"action":"assemble_prompt","desc":"Assemble prompt"},
                         {"step":1,"action":"kohai_complete","desc":"Call Kohai"}]"#;
        let migration =
            include_str!("../../brassclaw_pg/migrations/V093__non_match_recipe_ibs_metadata.sql");
        let client = pool.get().await.expect("database connection");
        client
            .execute(
                "UPDATE reborn_recipes SET variants=NULL, step_descriptions=$2::text::jsonb,
             override_prompt_creation=true WHERE id=$1",
                &[&recipe.id, &legacy],
            )
            .await
            .expect("legacy override fixture");
        client
            .batch_execute(migration)
            .await
            .expect("migration preserves override");
        let preserved: Option<String> = client
            .query_one(
                "SELECT variants::text FROM reborn_recipes WHERE id=$1",
                &[&recipe.id],
            )
            .await
            .expect("read override")
            .get(0);
        assert!(preserved.is_none());
        client
            .execute(
                "UPDATE reborn_recipes SET override_prompt_creation=false WHERE id=$1",
                &[&recipe.id],
            )
            .await
            .expect("known unmodified legacy metadata");
        client
            .batch_execute(migration)
            .await
            .expect("upgrade legacy metadata");
        let upgraded = port
            .compose(&scope, recipe.id, "0:1-0:E", "input")
            .await
            .expect("upgraded Recipe compiles through IBS");
        assert_eq!(upgraded, original);
        let upgraded_at: String = client
            .query_one(
                "SELECT updated_at::text FROM reborn_recipes WHERE id=$1",
                &[&recipe.id],
            )
            .await
            .expect("upgrade timestamp")
            .get(0);
        client
            .batch_execute(migration)
            .await
            .expect("idempotent migration");
        let unchanged_at: String = client
            .query_one(
                "SELECT updated_at::text FROM reborn_recipes WHERE id=$1",
                &[&recipe.id],
            )
            .await
            .expect("unchanged timestamp")
            .get(0);
        assert_eq!(unchanged_at, upgraded_at);
        drop(client);
        drop(port);
        drop(booted);
        pool.close();
        drop(pool);
        drop(rig);
    }

    #[cfg(all(feature = "postgres", feature = "skills-db"))]
    #[tokio::test]
    async fn native_composition_enforces_catalog_visibility_and_resolves_rust_includes() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let pool = Arc::clone(&rig.pool);
        let booted = crate::booted_db::run_migrations_and_return_booted_db(pool.clone())
            .await
            .expect("schema migrations");
        let tenant = "composition-visibility-test";
        crate::component_boot::initialize_runtime_components(&booted, tenant)
            .await
            .expect("native component boot");
        let caller = ComponentScope {
            tenant_id: tenant.into(),
            user_id: "caller".into(),
            agent_id: "caller-agent".into(),
            project_id: "caller-project".into(),
        };
        let port = PgCompositionPort::new(pool.clone(), None, None);
        let recipe = port
            .resolve_component_by_name(&caller, "host-assemble-prefix-bundle", 21)
            .await
            .expect("catalog lookup")
            .expect("system recipe visible to caller");
        let program = port
            .compose(&caller, recipe.id, "0:1-0:E", "accepted input")
            .await
            .expect("IBS compiles a visible system recipe");
        assert_eq!(program.tier, "tier0");
        assert_eq!(program.steplist.len(), 2);
        assert!(
            program
                .steplist
                .iter()
                .all(|step| !step.executable_code.is_empty())
        );

        let other_tenant = ComponentScope {
            tenant_id: "other-tenant".into(),
            ..caller.clone()
        };
        assert!(matches!(
            port.compose(&other_tenant, recipe.id, "0:1-0:E", "input")
                .await,
            Err(ComponentPortError::RecipeNotFound { .. })
        ));

        let client = pool.get().await.expect("database connection");
        client
            .execute(
                "UPDATE reborn_recipes SET source = 'authored' WHERE id = $1",
                &[&recipe.id],
            )
            .await
            .expect("make recipe private to its stored scope");
        assert!(matches!(
            port.compose(&caller, recipe.id, "0:1-0:E", "input").await,
            Err(ComponentPortError::RecipeNotFound { .. })
        ));
        let owner = ComponentScope {
            tenant_id: tenant.into(),
            user_id: brassclaw_host_api::SYSTEM_RESERVED_ID.into(),
            agent_id: "default".into(),
            project_id: "system".into(),
        };
        port.compose(&owner, recipe.id, "0:1-0:E", "input")
            .await
            .expect("validated private recipe remains visible to owner");
        client
            .execute(
                "UPDATE reborn_recipes SET validation_status = 'pending' WHERE id = $1",
                &[&recipe.id],
            )
            .await
            .expect("withdraw validation");
        assert!(matches!(
            port.compose(&owner, recipe.id, "0:1-0:E", "input").await,
            Err(ComponentPortError::RecipeNotFound { .. })
        ));
        client.execute(
            "UPDATE reborn_recipes SET source = 'system', validation_status = 'validated' WHERE id = $1",
            &[&recipe.id],
        ).await.expect("restore system recipe");

        let binding = port
            .resolve_component_by_name(&caller, "ts-host-sweep-validated-components", 13)
            .await
            .expect("binding lookup")
            .expect("seeded binding");
        client
            .execute(
                "UPDATE reborn_tool_skills SET validation_status = 'pending' WHERE id = $1",
                &[&binding.id],
            )
            .await
            .expect("withdraw binding validation");
        assert!(matches!(
            port.compose(&caller, recipe.id, "0:1-0:E", "input").await,
            Err(ComponentPortError::IncludeNotResolved { include_id, .. })
                if include_id == binding.id.to_string()
        ));
        drop(client);
        drop(port);
        drop(booted);
        pool.close();
        drop(pool);
        drop(rig);
    }

    #[cfg(all(feature = "postgres", feature = "skills-db"))]
    #[tokio::test]
    async fn native_composition_reports_malformed_recipe_data_as_a_contract_failure() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let pool = Arc::clone(&rig.pool);
        let booted = crate::booted_db::run_migrations_and_return_booted_db(pool.clone())
            .await
            .expect("schema migrations");
        let tenant = "composition-contract-test";
        crate::component_boot::initialize_runtime_components(&booted, tenant)
            .await
            .expect("native component boot");
        let scope = ComponentScope {
            tenant_id: tenant.to_string(),
            user_id: brassclaw_host_api::SYSTEM_RESERVED_ID.to_string(),
            agent_id: "default".to_string(),
            project_id: "system".to_string(),
        };
        let port = PgCompositionPort::new(pool.clone(), None, None);
        let component = port
            .resolve_component_by_name(&scope, "host-non-match-llm-answer", 21)
            .await
            .expect("native component lookup")
            .expect("seeded no-match recipe");
        let program = port
            .compose(&scope, component.id, "0:1-0:E", "accepted input")
            .await
            .expect("seeded no-match Recipe compiles through IBS");
        assert_eq!(program.tier, "tier1");
        assert_eq!(program.steplist.len(), 2);
        assert_eq!(program.rust_directives.len(), 1);
        assert_eq!(program.rust_directives[0].tool_name, "host.kohai_complete");
        let client = pool.get().await.expect("database connection");
        let valid_variants = r#"[{"variant_key":"default","step_link":"0:1-0:E"}]"#;
        for (variants, descriptions, expected) in [
            ("{}", "[]", "invalid recipe variants"),
            (valid_variants, "{}", "invalid recipe step descriptions"),
        ] {
            let updated = client
                .execute(
                    "UPDATE reborn_recipes SET variants = $2::text::jsonb, \
                     step_descriptions = $3::text::jsonb WHERE id = $1",
                    &[&component.id, &variants, &descriptions],
                )
                .await
                .expect("persist malformed component data");
            assert_eq!(updated, 1);
            let error = port
                .compose(&scope, component.id, "0:1-0:E", "accepted input")
                .await
                .expect_err("malformed data must not become an empty program or missing match");
            assert!(matches!(
                error,
                ComponentPortError::Failure { reason } if reason == expected
            ));
        }
        drop(client);
        drop(port);
        drop(booted);
        pool.close();
        drop(pool);
        drop(rig);
    }

    fn variant(step_link: Option<&str>, key: &str) -> RecipeVariantUngated {
        RecipeVariantUngated {
            variant_key: key.to_string(),
            description: None,
            step_link: step_link.map(str::to_string),
            intent_examples: Vec::new(),
            variable_patterns: Vec::new(),
        }
    }

    #[test]
    fn match_variant_returns_the_variant_with_matching_step_link() {
        let variants = vec![variant(Some("0:1-2"), "ls"), variant(Some("1:1"), "pwd")];
        let matched = super::match_variant(&variants, "1:1");
        assert_eq!(matched.map(|v| v.variant_key.as_str()), Some("pwd"));
    }

    #[test]
    fn match_variant_returns_none_when_no_step_link_matches() {
        let variants = vec![variant(Some("0:1-2"), "ls"), variant(None, "legacy")];
        assert!(super::match_variant(&variants, "9:9").is_none());
    }

    #[test]
    fn component_item_to_resolved_passes_placeholders_through_unbound() {
        // The resolver does NOT bind {{vars.NAME}} — that is compose_program's
        // job. Mapping must pass the content through verbatim.
        let item = ComponentItem {
            id: Uuid::nil(),
            class_code: 22,
            prompt_uid: 0,
            name: "pc-greet".into(),
            description: "greet".into(),
            effective_content: "print('hi {{vars.name}}')".into(),
            override_prompt_creation: false,
        };
        let resolved = super::component_item_to_resolved(&item);
        assert_eq!(resolved.class_code, 22);
        assert_eq!(resolved.name, "pc-greet");
        assert_eq!(resolved.content, "print('hi {{vars.name}}')");
    }

    #[test]
    fn component_item_to_resolved_maps_fields_and_defaults_no_artifact_path_clean() {
        let item = ComponentItem {
            id: Uuid::nil(),
            class_code: 22,
            prompt_uid: 0,
            name: "pc-greet".into(),
            description: "greet".into(),
            effective_content: "print('hi')".into(),
            override_prompt_creation: false,
        };
        let resolved = super::component_item_to_resolved(&item);
        assert_eq!(resolved.class_code, 22);
        assert_eq!(resolved.name, "pc-greet");
        assert_eq!(resolved.description, "greet");
        assert_eq!(resolved.content, "print('hi')");
        assert!(resolved.cdylib_artifact_path.is_none());
    }

    #[test]
    fn map_component_resolver_resolves_known_and_skips_missing() {
        let id = Uuid::new_v4();
        let resolved = ResolvedComponentUngated {
            class_code: 1,
            name: "skill-x".into(),
            content: "body".into(),
            description: String::new(),
            cdylib_artifact_path: None,
        };
        let map: HashMap<Uuid, ResolvedComponentUngated> =
            [(id, resolved.clone())].into_iter().collect();
        let resolver = super::MapComponentResolver { map: &map };
        assert_eq!(resolver.resolve(id), Some(resolved));
        assert!(resolver.resolve(Uuid::new_v4()).is_none());
    }
}
