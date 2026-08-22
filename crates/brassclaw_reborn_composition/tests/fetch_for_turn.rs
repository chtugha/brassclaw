//!
//! Integration tests for `PostgresSource::fetch_for_turn` against a real
//! Postgres-16 schema (V000–V061, so `reborn_components` + its triggers, the
//! `reborn_intent_inputs.step_link` column from V054, and the
//! `reborn_recipes.step_descriptions`/`variants` JSONB columns from V050 all
//! exist). The `engine` crate has no testcontainers dev-dep, so these DB-backed
//! tests live in the composition `tests/` tier (established Phase B/C/D/E.1
//! pattern) and mirror `components_registry.rs` / `intent_step_link`.
//!
//! These cover the Phase E.6 DB-dependent half of the plan's test list (the
//! pure-mechanism half — `knowledge: both` UUID-in-both-channels and the
//! `{{vars.dir}}` capture→substitute chain — lives as true unit tests in
//! `brassclaw_engine::memory::instruction_builder::tests`). Each test starts an
//! isolated Postgres-16 testcontainer, runs the full migration set, seeds
//! components + an intent input, calls `fetch_for_turn`, and asserts the
//! `FetchForTurnResult` variant + channel split + routing signals. Returns
//! early (pass) when docker/testcontainers is unavailable. Gated to `skills-db`
//! because `PostgresSource`/`fetch_for_turn` are skills-db-only.
//!
//! Plan E.6 test list → test fn map:
//! - #1 SplitResult channel split by class     → `split_result_channels_split_by_class`
//! - #3 ActionShortCircuit                      → `action_match_returns_action_short_circuit`
//! - #4 step_link: None → Components unchanged  → `recipe_match_step_link_none_returns_components`
//! - #5b {{vars.dir}} no-op at Phase E          → `substitution_noop_at_phase_e_preserves_placeholder`
//! - #6 routing.wilson_lower populated          → `routing_wilson_lower_populated_from_recipe_row`
//! - #7 registry lookup resolves seeded UUIDs   → `registry_lookup_resolves_seeded_step_include_uuids`
//! - Integration#1 full intent match → split    → `full_intent_match_correct_channel_split_by_class_code`

#![cfg(feature = "skills-db")]

use std::collections::HashSet;
use std::sync::Arc;

use brassclaw_engine::memory::{
    ComponentScope, FetchForTurnResult, PostgresSource, RetrievalSource,
};
use tokio_postgres::types::ToSql;
use uuid::Uuid;

const STEP_LINK: &str = "0:0-0:E";
const SENDER: &str = "02:orchestrator";
const TOKEN_BUDGET: usize = 8000;

struct PgRig {
    // Held for the test's lifetime so the container stays up.
    _container: testcontainers_modules::testcontainers::ContainerAsync<
        testcontainers_modules::postgres::Postgres,
    >,
    pool: deadpool_postgres::Pool,
}

/// Start an isolated Postgres-16 testcontainer, build a pool, and run every
/// migration (V000–V061). Returns `None` (skip) when docker is unavailable.
async fn pg_rig_or_skip() -> Option<PgRig> {
    use testcontainers_modules::testcontainers::{ImageExt, runners::AsyncRunner};

    let image = testcontainers_modules::postgres::Postgres::default()
        .with_db_name("brassclaw_test")
        .with_user("postgres")
        .with_password("postgres")
        .with_tag("16-alpine");
    let container = match image.start().await {
        Ok(c) => c,
        Err(error) => {
            eprintln!("skipping fetch_for_turn tests: docker/testcontainers unavailable ({error})");
            return None;
        }
    };
    let host = match container.get_host().await {
        Ok(h) => h,
        Err(error) => {
            eprintln!("skipping fetch_for_turn tests: no host ({error})");
            return None;
        }
    };
    let port = match container.get_host_port_ipv4(5432).await {
        Ok(p) => p,
        Err(error) => {
            eprintln!("skipping fetch_for_turn tests: no port ({error})");
            return None;
        }
    };
    let url = format!("postgres://postgres:postgres@{host}:{port}/brassclaw_test");
    let cfg: tokio_postgres::Config = url.parse().expect("testcontainer url parses");
    let manager = deadpool_postgres::Manager::new(cfg, tokio_postgres::NoTls);
    let pool = deadpool_postgres::Pool::builder(manager)
        .max_size(4)
        .build()
        .expect("Postgres pool must build");
    brassclaw_pg::migrations::run_migrations(&pool)
        .await
        .expect("migrations must apply");
    Some(PgRig {
        _container: container,
        pool,
    })
}

/// A fresh, isolated scope per test (unique tenant) so parallel tests never
/// collide on any component / intent row.
fn unique_scope() -> ComponentScope {
    ComponentScope {
        tenant_id: format!("t-{}", Uuid::new_v4()),
        user_id: "u".to_string(),
        agent_id: "a".to_string(),
        project_id: "p".to_string(),
    }
}

/// A slug-style name satisfying the `^[a-z0-9]([a-z0-9-]*[a-z0-9])?$` (1-64)
/// CHECK on `reborn_skills`/`reborn_actions`/`reborn_tool_skills`/
/// `reborn_recipes`. UUID-derived so parallel runs stay off the
/// `UNIQUE(scope, name)` constraint.
fn unique_name(prefix: &str) -> String {
    format!("{}{}", prefix, &Uuid::new_v4().simple().to_string()[..12])
}

/// Build a `PostgresSource` over the rig's pool (cheap pool clone → Arc).
fn source(rig: &PgRig) -> PostgresSource {
    PostgresSource::new(Arc::new(rig.pool.clone()))
}

// ---------------------------------------------------------------------------
// JSON helpers — build `step_descriptions` / `variants` as serde_json::Value
// then stringify, so the DB round-trip exercises the SAME deserialization path
// production uses (the IBS reads the stored JSONB via serde_json::from_str).
// UUIDs are emitted as strings (uuid's serde reads human-readable strings).
// ---------------------------------------------------------------------------

/// One `StepEntry` with `type: component` and the given `include` UUIDs.
fn step(stepnumber: u32, knowledge: &str, include: &[Uuid]) -> serde_json::Value {
    let include_str: Vec<String> = include.iter().map(|u| u.to_string()).collect();
    serde_json::json!({
        "stepnumber": stepnumber,
        "knowledge": knowledge,
        "goal": "g",
        "content": "c",
        "type": "component",
        "include": include_str,
        "tool_bindings": [],
    })
}

/// A single `StepDescriptionEntry` (desc_idx 0) wrapping `steps`, stringified.
fn step_descs_text(steps: Vec<serde_json::Value>) -> String {
    let sds = serde_json::json!([{
        "desc_idx": 0,
        "label": "sd0",
        "yaml_source": "steps: []",
        "steps": steps,
    }]);
    serde_json::to_string(&sds).expect("step_descriptions serializes")
}

/// A single `RecipeVariant` matching `STEP_LINK`, stringified.
fn variants_text(variable_patterns: Vec<serde_json::Value>) -> String {
    let v = serde_json::json!([{
        "variant_key": "v",
        "step_link": STEP_LINK,
        "intent_examples": [],
        "variable_patterns": variable_patterns,
    }]);
    serde_json::to_string(&v).expect("variants serializes")
}

// ---------------------------------------------------------------------------
// Seed helpers — supply only NOT-NULL-no-default columns; every other column
// has a default. Sub-components are seeded `validation_status='validated'` so
// the SEC-01 gate in `fetch_components_by_ids` returns them. The V061
// `maintain_components_registry` AFTER-INSERT trigger auto-populates
// `reborn_components` on every insert, so `lookup_component_class` resolves the
// seeded UUIDs without any manual registry row.
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
async fn insert_recipe(
    pool: &deadpool_postgres::Pool,
    scope: &ComponentScope,
    id: Uuid,
    step_descs: Option<String>,
    variants: Option<String>,
    tier: &str,
    wilson_lower: f64,
    validation_status: &str,
) {
    let name = unique_name("recipe");
    let client = pool.get().await.expect("pool client");
    client
        .execute(
            "INSERT INTO reborn_recipes
                 (id, tenant_id, user_id, agent_id, project_id, name, description,
                  step_descriptions, variants, tier, wilson_lower, validation_status)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8::jsonb,$9::jsonb,$10,$11,$12)",
            &[
                &id as &(dyn ToSql + Sync),
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &name,
                &"test recipe description",
                &step_descs,
                &variants,
                &tier,
                &wilson_lower,
                &validation_status,
            ],
        )
        .await
        .expect("insert reborn_recipes");
}

async fn insert_tool_skill(
    pool: &deadpool_postgres::Pool,
    scope: &ComponentScope,
    id: Uuid,
    content: &str,
) {
    let name = unique_name("ts");
    let client = pool.get().await.expect("pool client");
    client
        .execute(
            "INSERT INTO reborn_tool_skills
                 (id, tenant_id, user_id, agent_id, project_id, name, description,
                  content, validation_status)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'validated')",
            &[
                &id as &(dyn ToSql + Sync),
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &name,
                &"tool-skill description",
                &content,
            ],
        )
        .await
        .expect("insert reborn_tool_skills");
}

async fn insert_skill(
    pool: &deadpool_postgres::Pool,
    scope: &ComponentScope,
    id: Uuid,
    body: &str,
) {
    let name = unique_name("skill");
    let class_code: i16 = 3;
    let client = pool.get().await.expect("pool client");
    client
        .execute(
            "INSERT INTO reborn_skills
                 (id, tenant_id, user_id, agent_id, project_id, name, description,
                  body, class_code, validation_status)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
            &[
                &id as &(dyn ToSql + Sync),
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &name,
                &"skill description",
                &body,
                &class_code,
                &"validated",
            ],
        )
        .await
        .expect("insert reborn_skills");
}

async fn insert_python_code(
    pool: &deadpool_postgres::Pool,
    scope: &ComponentScope,
    id: Uuid,
    content: &str,
) {
    let name = unique_name("py");
    let client = pool.get().await.expect("pool client");
    client
        .execute(
            "INSERT INTO reborn_python_code
                 (id, tenant_id, user_id, agent_id, project_id, name, description,
                  content, validation_status)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'validated')",
            &[
                &id as &(dyn ToSql + Sync),
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &name,
                &"python-code description",
                &content,
            ],
        )
        .await
        .expect("insert reborn_python_code");
}

async fn insert_action(
    pool: &deadpool_postgres::Pool,
    scope: &ComponentScope,
    id: Uuid,
    name: &str,
) {
    let client = pool.get().await.expect("pool client");
    client
        .execute(
            "INSERT INTO reborn_actions
                 (id, tenant_id, user_id, agent_id, project_id, name, description,
                  validation_status)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
            &[
                &id as &(dyn ToSql + Sync),
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &name,
                &"action description",
                &"validated",
            ],
        )
        .await
        .expect("insert reborn_actions");
}

#[allow(clippy::too_many_arguments)]
async fn insert_intent_input(
    pool: &deadpool_postgres::Pool,
    scope: &ComponentScope,
    input_text: &str,
    input_class: i16,
    component_id: Uuid,
    component_class_code: i32,
    score: i32,
    step_link: Option<String>,
) {
    let client = pool.get().await.expect("pool client");
    client
        .execute(
            "INSERT INTO reborn_intent_inputs
                 (tenant_id, user_id, agent_id, project_id, input_text, input_class,
                  component_id, component_class_code, score, step_link)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
            &[
                &scope.tenant_id as &(dyn ToSql + Sync),
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &input_text,
                &input_class,
                &component_id,
                &component_class_code,
                &score,
                &step_link,
            ],
        )
        .await
        .expect("insert reborn_intent_inputs");
}

// ---------------------------------------------------------------------------
// #1 — SplitResult channels split by class (rust=tool_skill 13, orch=skill 3 + python_code 22)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn split_result_channels_split_by_class() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let scope = unique_scope();

    let tool_skill_id = Uuid::new_v4();
    let skill_id = Uuid::new_v4();
    let py_id = Uuid::new_v4();
    insert_tool_skill(&rig.pool, &scope, tool_skill_id, "ts body").await;
    insert_skill(&rig.pool, &scope, skill_id, "skill body").await;
    insert_python_code(&rig.pool, &scope, py_id, "py body").await;

    let recipe_id = Uuid::new_v4();
    let step_descs = step_descs_text(vec![
        step(1, "rust", &[tool_skill_id]),
        step(2, "orchestrator", &[skill_id]),
        step(3, "orchestrator", &[py_id]),
    ]);
    let variants = variants_text(vec![]);
    insert_recipe(
        &rig.pool,
        &scope,
        recipe_id,
        Some(step_descs),
        Some(variants),
        "seedling",
        0.0,
        "pending",
    )
    .await;

    // 2-word query → Partial (input_class 2); intent input_class=2 is preferred.
    insert_intent_input(
        &rig.pool,
        &scope,
        "list files",
        2,
        recipe_id,
        21,
        10,
        Some(STEP_LINK.into()),
    )
    .await;

    let result = source(&rig)
        .fetch_for_turn(&scope, "list files", TOKEN_BUDGET, SENDER)
        .await
        .expect("fetch_for_turn succeeds");

    match result {
        FetchForTurnResult::SplitResult {
            rust_items,
            orchestrator_items,
            instruction,
            ..
        } => {
            assert_eq!(rust_items.len(), 1, "rust channel has the tool_skill only");
            assert_eq!(rust_items[0].class_code, 13);

            let orch_classes: HashSet<i32> =
                orchestrator_items.iter().map(|i| i.class_code).collect();
            assert_eq!(
                orch_classes,
                [3, 22].iter().copied().collect(),
                "orchestrator channel has skill + python_code"
            );

            let instruction = instruction.expect("instruction compiled");
            assert_eq!(instruction.rust_steps.len(), 1);
            assert_eq!(instruction.orchestrator_steps.len(), 2);
        }
        other => panic!("expected SplitResult, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// #3 — Action (class 16) intent match → ActionShortCircuit (name from the
// resolve_intent LEFT JOIN, no second fetch)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn action_match_returns_action_short_circuit() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let scope = unique_scope();

    let action_id = Uuid::new_v4();
    let action_name = unique_name("act");
    insert_action(&rig.pool, &scope, action_id, &action_name).await;

    insert_intent_input(&rig.pool, &scope, "run job", 2, action_id, 16, 10, None).await;

    let result = source(&rig)
        .fetch_for_turn(&scope, "run job", TOKEN_BUDGET, SENDER)
        .await
        .expect("fetch_for_turn succeeds");

    match result {
        FetchForTurnResult::ActionShortCircuit { component_id, name } => {
            assert_eq!(component_id, action_id);
            assert_eq!(name, action_name);
        }
        other => panic!("expected ActionShortCircuit, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// #4 — class-21 match with step_link: None → legacy fetch_component_by_id →
// Components (unchanged path). The recipe must be validated for the SEC-01 gate.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn recipe_match_step_link_none_returns_components() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let scope = unique_scope();

    let recipe_id = Uuid::new_v4();
    // No step_descriptions / variants — the legacy path does not read them.
    insert_recipe(
        &rig.pool,
        &scope,
        recipe_id,
        None,
        None,
        "seedling",
        0.0,
        "validated",
    )
    .await;

    insert_intent_input(&rig.pool, &scope, "show help", 2, recipe_id, 21, 10, None).await;

    let result = source(&rig)
        .fetch_for_turn(&scope, "show help", TOKEN_BUDGET, SENDER)
        .await
        .expect("fetch_for_turn succeeds");

    match result {
        FetchForTurnResult::Components(items) => {
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].class_code, 21);
        }
        other => panic!("expected Components, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// #5b — {{vars.dir}} substitution is a wired NO-OP at Phase E (exact-match
// intent ⇒ no `%` template ⇒ vars=[] ⇒ placeholder preserved). Phase M activates
// real substitution via the same wiring.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn substitution_noop_at_phase_e_preserves_placeholder() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let scope = unique_scope();

    let skill_id = Uuid::new_v4();
    // The skill body carries an unsubstituted placeholder.
    insert_skill(
        &rig.pool,
        &scope,
        skill_id,
        "Run ls inside {{vars.dir}} now",
    )
    .await;

    let recipe_id = Uuid::new_v4();
    let step_descs = step_descs_text(vec![step(1, "orchestrator", &[skill_id])]);
    // The matched variant declares a `dir` variable pattern, but Phase E's
    // exact-match intent yields no `%`-template slots, so capture_variables
    // returns [] and substitution is inert.
    let variants = variants_text(vec![serde_json::json!({ "name": "dir" })]);
    insert_recipe(
        &rig.pool,
        &scope,
        recipe_id,
        Some(step_descs),
        Some(variants),
        "seedling",
        0.0,
        "pending",
    )
    .await;

    insert_intent_input(
        &rig.pool,
        &scope,
        "list files",
        2,
        recipe_id,
        21,
        10,
        Some(STEP_LINK.into()),
    )
    .await;

    let result = source(&rig)
        .fetch_for_turn(&scope, "list files", TOKEN_BUDGET, SENDER)
        .await
        .expect("fetch_for_turn succeeds");

    match result {
        FetchForTurnResult::SplitResult {
            orchestrator_items, ..
        } => {
            let skill = orchestrator_items
                .iter()
                .find(|i| i.id == skill_id)
                .expect("skill fetched into orchestrator channel");
            assert!(
                skill.effective_content.contains("{{vars.dir}}"),
                "Phase E no-op: placeholder preserved unchanged, got {}",
                skill.effective_content
            );
        }
        other => panic!("expected SplitResult, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// #6 — routing.wilson_lower populated from the recipe row; tier0_eligible when
// tier ∈ {mature, candidate} + validated + wilson_lower ≥ 0.70.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn routing_wilson_lower_populated_from_recipe_row() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let scope = unique_scope();

    let skill_id = Uuid::new_v4();
    insert_skill(&rig.pool, &scope, skill_id, "skill body").await;

    let recipe_id = Uuid::new_v4();
    let step_descs = step_descs_text(vec![step(1, "orchestrator", &[skill_id])]);
    let variants = variants_text(vec![]);
    insert_recipe(
        &rig.pool,
        &scope,
        recipe_id,
        Some(step_descs),
        Some(variants),
        "mature",
        0.82,
        "validated",
    )
    .await;

    insert_intent_input(
        &rig.pool,
        &scope,
        "run job",
        2,
        recipe_id,
        21,
        10,
        Some(STEP_LINK.into()),
    )
    .await;

    let result = source(&rig)
        .fetch_for_turn(&scope, "run job", TOKEN_BUDGET, SENDER)
        .await
        .expect("fetch_for_turn succeeds");

    match result {
        FetchForTurnResult::SplitResult { routing, .. } => {
            assert!(
                (routing.wilson_lower - 0.82).abs() < 1e-9,
                "wilson_lower carried from the recipe row: {}",
                routing.wilson_lower
            );
            assert!(routing.tier0_eligible, "mature + validated + 0.82 ≥ 0.70");
            assert!(!routing.llm_call_required, "Tier-0 ⇒ no LLM call");
        }
        other => panic!("expected SplitResult, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// #7 — registry lookup resolves seeded step-include UUIDs end-to-end in the
// SplitResult flow (the V061 trigger-maintained `reborn_components` registry
// is exercised in context by `fetch_recipe_split_result`).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn registry_lookup_resolves_seeded_step_include_uuids() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let scope = unique_scope();

    let tool_skill_id = Uuid::new_v4();
    let skill_id = Uuid::new_v4();
    insert_tool_skill(&rig.pool, &scope, tool_skill_id, "ts body").await;
    insert_skill(&rig.pool, &scope, skill_id, "skill body").await;

    let recipe_id = Uuid::new_v4();
    let step_descs = step_descs_text(vec![
        step(1, "rust", &[tool_skill_id]),
        step(2, "orchestrator", &[skill_id]),
    ]);
    let variants = variants_text(vec![]);
    insert_recipe(
        &rig.pool,
        &scope,
        recipe_id,
        Some(step_descs),
        Some(variants),
        "seedling",
        0.0,
        "pending",
    )
    .await;

    insert_intent_input(
        &rig.pool,
        &scope,
        "do thing",
        2,
        recipe_id,
        21,
        10,
        Some(STEP_LINK.into()),
    )
    .await;

    let result = source(&rig)
        .fetch_for_turn(&scope, "do thing", TOKEN_BUDGET, SENDER)
        .await
        .expect("fetch_for_turn succeeds");

    match result {
        FetchForTurnResult::SplitResult {
            rust_items,
            orchestrator_items,
            ..
        } => {
            // The registry resolved each step UUID → its class, and
            // fetch_components_by_ids returned the seeded rows.
            assert!(
                rust_items
                    .iter()
                    .any(|i| i.id == tool_skill_id && i.class_code == 13),
                "tool_skill resolved into the rust channel"
            );
            assert!(
                orchestrator_items
                    .iter()
                    .any(|i| i.id == skill_id && i.class_code == 3),
                "skill resolved into the orchestrator channel"
            );
        }
        other => panic!("expected SplitResult, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Integration#1 — full intent match → correct channel split by class_code.
// A Rust step + a Both step + an Orchestrator step:
//   rust channel     = {rust step include A, both step include B} → 2 × class 13
//   orch channel     = {both step include B, orch step include C} → B (13) + C (3)
//   the both-step UUID B appears in BOTH channels; wilson_lower=0.75 (Tier-0).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn full_intent_match_correct_channel_split_by_class_code() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let scope = unique_scope();

    let tool_skill_a = Uuid::new_v4();
    let tool_skill_b = Uuid::new_v4();
    let skill_c = Uuid::new_v4();
    insert_tool_skill(&rig.pool, &scope, tool_skill_a, "ts a body").await;
    insert_tool_skill(&rig.pool, &scope, tool_skill_b, "ts b body").await;
    insert_skill(&rig.pool, &scope, skill_c, "skill c body").await;

    let recipe_id = Uuid::new_v4();
    let step_descs = step_descs_text(vec![
        step(1, "rust", &[tool_skill_a]),
        step(2, "both", &[tool_skill_b]),
        step(3, "orchestrator", &[skill_c]),
    ]);
    let variants = variants_text(vec![]);
    insert_recipe(
        &rig.pool,
        &scope,
        recipe_id,
        Some(step_descs),
        Some(variants),
        "mature",
        0.75,
        "validated",
    )
    .await;

    insert_intent_input(
        &rig.pool,
        &scope,
        "full match",
        2,
        recipe_id,
        21,
        10,
        Some(STEP_LINK.into()),
    )
    .await;

    let result = source(&rig)
        .fetch_for_turn(&scope, "full match", TOKEN_BUDGET, SENDER)
        .await
        .expect("fetch_for_turn succeeds");

    match result {
        FetchForTurnResult::SplitResult {
            rust_items,
            orchestrator_items,
            routing,
            instruction,
        } => {
            // Rust channel: A + B, both tool_skills (class 13).
            assert_eq!(rust_items.len(), 2, "rust channel has A + B");
            assert!(
                rust_items.iter().all(|i| i.class_code == 13),
                "rust channel is all tool_skills"
            );

            // Orchestrator channel: B (class 13) + C (class 3).
            let orch_classes: HashSet<i32> =
                orchestrator_items.iter().map(|i| i.class_code).collect();
            assert_eq!(
                orch_classes,
                [13, 3].iter().copied().collect(),
                "orchestrator channel is B + C"
            );

            // The both-step UUID B appears in BOTH channels (§0.8 per-channel fetch).
            assert!(
                rust_items.iter().any(|i| i.id == tool_skill_b),
                "both-step UUID B in rust channel"
            );
            assert!(
                orchestrator_items.iter().any(|i| i.id == tool_skill_b),
                "both-step UUID B in orchestrator channel"
            );

            assert!(
                (routing.wilson_lower - 0.75).abs() < 1e-9,
                "wilson_lower carried: {}",
                routing.wilson_lower
            );
            assert!(routing.tier0_eligible);

            let instruction = instruction.expect("instruction compiled");
            assert_eq!(
                instruction.rust_steps.len(),
                2,
                "rust_steps = rust step + both step"
            );
            assert_eq!(
                instruction.orchestrator_steps.len(),
                2,
                "orchestrator_steps = both step + orch step"
            );
        }
        other => panic!("expected SplitResult, got {other:?}"),
    }
}
