//! Step C.4.5.17 Part 3b — composition-side [`CompositionPort`] impl (the IBS).
//!
//! [`PgCompositionPort`] is the composition-layer impl of the engine-side
//! [`brassclaw_engine::executor::CompositionPort`] trait. It owns a Postgres
//! pool and performs the full composition pipeline for
//! `host.compose_orchestrator(component_id, step_link, user_input)`:
//!
//! 1. SELECT the recipe (class 21) row by `component_id` + scope.
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
//! The cdylib *application* of `rust_directives` (dlopen via
//! `DynamicToolLoader`, which lives in `brassclaw_host_runtime` — downstream of
//! the engine) is a Step C.5/C.6 concern and is deferred: the directives are
//! CARRIED in the returned program (with `artifact_path` left empty for class-0
//! tools, since V071 dropped `cdylib_artifact_path` from `reborn_tools`) so the
//! driver/loader can apply them once that wiring lands.
//!
//! # Feature gate
//!
//! The DB-bound [`PgCompositionPort`] + its [`CompositionPort`] impl require the
//! `skills-db` feature (the engine recipe/component free functions it delegates
//! to are `skills-db`-gated). The pure mapping helpers + their unit tests touch
//! only always-available engine types and compile/run under both configs,
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
use brassclaw_engine::executor::{CompositionPort, CompositionPortError};
#[cfg(feature = "skills-db")]
use brassclaw_engine::memory::composition::compose_program;
#[cfg(feature = "skills-db")]
use brassclaw_engine::memory::instruction_builder::{
    StepDescriptionEntry, build_instruction, capture_variables,
};
#[cfg(feature = "skills-db")]
use brassclaw_engine::memory::retrieval_source::{
    ComponentScope, fetch_components_by_ids, lookup_component_class,
};
#[cfg(feature = "skills-db")]
use brassclaw_engine::memory::{ComposedProgram, ResolvedComponent};
#[cfg(feature = "skills-db")]
use brassclaw_engine::types::recipe::RecipeVariant;
#[cfg(feature = "skills-db")]
use brassclaw_pg::PgPool;

/// Match a variant by `step_link` (§7.3). Returns the first variant whose
/// `step_link` equals the supplied formula, or `None` when no variant matches
/// (caller surfaces [`CompositionPortError::NoVariantMatch`]).
fn match_variant<'a>(variants: &'a [RecipeVariantUngated], step_link: &str) -> Option<&'a RecipeVariantUngated> {
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

/// Postgres-backed [`CompositionPort`] (the IBS). Constructed once at runtime
/// wiring time with the shared `PgPool` and plumbed into the engine
/// `ThreadManager` via `with_composition_port`; until that C.5/C.6 wiring lands
/// the engine passes `None` and `host.compose_orchestrator` degrades gracefully.
#[cfg(feature = "skills-db")]
pub(crate) struct PgCompositionPort {
    pool: Arc<PgPool>,
}

#[cfg(feature = "skills-db")]
impl PgCompositionPort {
    pub(crate) fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    /// The composition pipeline (steps 1-8 above). Takes the pool by reference
    /// so the trait impl can clone the call args into owned data and drive a
    /// `'static` boxed future (the trait's `+ '_` return captures only
    /// `&self`).
    async fn compose_with_pool(
        pool: &PgPool,
        scope: &ComponentScope,
        component_id: Uuid,
        step_link: &str,
        user_input: &str,
    ) -> Result<ComposedProgram, CompositionPortError> {
        // 1. Recipe row — scope filter only (§7.2). JSONB read as text +
        //    serde_json::from_str (engine idiom).
        let client = pool
            .get()
            .await
            .map_err(|e| CompositionPortError::Failure {
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
                   AND user_id    = $3
                   AND agent_id   = $4
                   AND project_id = $5",
                &[
                    &component_id,
                    &scope.tenant_id,
                    &scope.user_id,
                    &scope.agent_id,
                    &scope.project_id,
                ],
            )
            .await
            .map_err(|e| CompositionPortError::Failure {
                reason: e.to_string(),
            })?;
        let Some(row) = row else {
            return Err(CompositionPortError::RecipeNotFound {
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
            serde_json::from_str(&variants_text).unwrap_or_default();
        let Some(matched) = match_variant(&variants, step_link) else {
            return Err(CompositionPortError::NoVariantMatch {
                step_link: step_link.to_string(),
            });
        };
        let variable_patterns = matched.variable_patterns.clone();
        let variant_label = matched.variant_key.clone();
        let _ = (recipe_name, variant_label);

        // 4. StepDescriptions.
        let step_descs: Vec<StepDescriptionEntry> =
            serde_json::from_str(&step_descriptions_text).unwrap_or_default();

        // 5. IBS compile (§0.4, §0.7). A compile failure is a hard composition
        //    error (not the soft-fail the retrieval path takes) — the
        //    orchestrator asked for this exact recipe/variant.
        let instruction = build_instruction(step_link, &step_descs, &variable_patterns, llm_call_required)
            .map_err(|e| CompositionPortError::Failure {
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
                    .map_err(|e| CompositionPortError::Failure {
                        reason: e.to_string(),
                    })?
            {
                pairs.push((*id, class_code));
            }
        }
        let items = fetch_components_by_ids(pool, scope, &pairs)
            .await
            .map_err(|e| CompositionPortError::Failure {
                reason: e.to_string(),
            })?;

        // 8. Resolver map + compose.
        let mut map: HashMap<Uuid, ResolvedComponent> = HashMap::new();
        for item in &items {
            map.insert(item.id, component_item_to_resolved(item));
        }
        let resolver = MapComponentResolver { map: &map };

        Ok(compose_program(&instruction, &resolver, &vars))
    }
}

#[cfg(feature = "skills-db")]
impl CompositionPort for PgCompositionPort {
    fn compose(
        &self,
        scope: &ComponentScope,
        component_id: Uuid,
        step_link: &str,
        user_input: &str,
    ) -> Pin<Box<dyn Future<Output = Result<ComposedProgram, CompositionPortError>> + Send + '_>>
    {
        // Clone the call args into owned data so the boxed future is `'static`
        // (the trait's `+ '_` return captures only `&self`, which a `'static`
        // future satisfies trivially).
        let pool = self.pool.clone();
        let scope = scope.clone();
        let step_link = step_link.to_string();
        let user_input = user_input.to_string();
        Box::pin(async move {
            Self::compose_with_pool(&pool, &scope, component_id, &step_link, &user_input).await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_engine::memory::ComponentItem;
    use uuid::Uuid;

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
        let variants = vec![
            variant(Some("0:1-2"), "ls"),
            variant(Some("1:1"), "pwd"),
        ];
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
            steps: None,
            allowed_tools: None,
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
            steps: None,
            allowed_tools: None,
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
        let map: HashMap<Uuid, ResolvedComponentUngated> = [(id, resolved.clone())].into_iter().collect();
        let resolver = super::MapComponentResolver { map: &map };
        assert_eq!(resolver.resolve(id), Some(resolved));
        assert!(resolver.resolve(Uuid::new_v4()).is_none());
    }
}
