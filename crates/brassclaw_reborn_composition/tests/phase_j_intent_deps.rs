//! Phase J integration tests — J.1 skill intent seeding + J.3 dependency traversal.
//!
//! Tests are gated to `skills-db` and skip when docker is unavailable.
//!
//! ## J.1 tests
//!
//! - `skill_intent_examples_seed_and_resolve` — seed a skill's intent_examples
//!   into `reborn_intent_inputs` (simulating the J.1 wiring path) and verify
//!   that `resolve_intent` returns a `Match` for each seeded example.
//!
//! ## J.3 tests
//!
//! - `resolve_deps_all_traversal` — `dependency_registry` with `[all]` returns
//!   the full transitive closure.
//! - `resolve_deps_selective_indices` — selective traversal returns only the
//!   requested indices, skipping others.
//! - `resolve_deps_deduplication` — UUID already in `visited` is skipped.
//! - `resolve_deps_cycle_guard` — A → B → A cycle terminates without recursion.
//! - `resolve_deps_channel_routing` — class-13 ToolSkill → rust_items; others
//!   → orchestrator_items.

#![cfg(feature = "skills-db")]

use std::sync::Arc;

use brassclaw_engine::memory::instruction_builder::{
    DependencyExpr, DependencyNode, DependencySubExpr,
};
use brassclaw_engine::memory::intent_system::{
    InputClass, IntentResolution, IntentScope, IntentSource, resolve_intent, seed_intent_input,
};
use brassclaw_engine::memory::retrieval_source::{ComponentScope, resolve_dependencies};
use uuid::Uuid;

// ─── testcontainer rig ───────────────────────────────────────────────────────

struct PgRig {
    _container: testcontainers_modules::testcontainers::ContainerAsync<
        testcontainers_modules::postgres::Postgres,
    >,
    pool: deadpool_postgres::Pool,
}

async fn pg_rig_or_skip() -> Option<PgRig> {
    use testcontainers_modules::testcontainers::{ImageExt, runners::AsyncRunner};

    let image = testcontainers_modules::postgres::Postgres::default()
        .with_db_name("brassclaw_test")
        .with_user("postgres")
        .with_password("postgres")
        .with_tag("16-alpine");
    let container = match image.start().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("skipping phase_j tests: docker unavailable ({e})");
            return None;
        }
    };
    let host = container.get_host().await.ok()?;
    let port = container.get_host_port_ipv4(5432).await.ok()?;
    let url = format!("postgres://postgres:postgres@{host}:{port}/brassclaw_test");
    let cfg: tokio_postgres::Config = url.parse().expect("parse url");
    let mgr = deadpool_postgres::Manager::new(cfg, tokio_postgres::NoTls);
    let pool = deadpool_postgres::Pool::builder(mgr)
        .max_size(4)
        .build()
        .expect("pool build");
    brassclaw_pg::migrations::run_migrations(&pool)
        .await
        .expect("migrations");
    Some(PgRig { _container: container, pool })
}

// ─── helpers ─────────────────────────────────────────────────────────────────

fn unique_scope() -> IntentScope {
    IntentScope {
        tenant_id: format!("t-{}", Uuid::new_v4()),
        user_id: "u".into(),
        agent_id: "a".into(),
        project_id: "p".into(),
    }
}

fn component_scope_from(s: &IntentScope) -> ComponentScope {
    ComponentScope {
        tenant_id: s.tenant_id.clone(),
        user_id: s.user_id.clone(),
        agent_id: s.agent_id.clone(),
        project_id: s.project_id.clone(),
    }
}

/// Insert a minimal validated `reborn_specs` row (class 12).
/// Returns the new component's UUID.
async fn insert_validated_spec(
    pool: &deadpool_postgres::Pool,
    scope: &IntentScope,
    name: &str,
    dep_registry_json: Option<&str>,
) -> Uuid {
    let id = Uuid::new_v4();
    let client = pool.get().await.expect("pool client");
    let dep_json = dep_registry_json.unwrap_or("[]");
    client
        .execute(
            "INSERT INTO reborn_specs
                (id, tenant_id, user_id, agent_id, project_id,
                 name, description, content,
                 class_code, source, validation_status, dependency_registry)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12::jsonb)",
            &[
                &id,
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &name,
                &"spec description",
                &"spec content",
                &(12i32),
                &"system",
                &"validated",
                &dep_json,
            ],
        )
        .await
        .expect("insert spec");
    // Register in reborn_components (V061 trigger should fire on INSERT; this
    // is a belt-and-suspenders guard for the test isolation case).
    client
        .execute(
            "INSERT INTO reborn_components (id, tenant_id, user_id, agent_id, project_id, class_code)
             VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT (id) DO NOTHING",
            &[
                &id,
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &(12i32),
            ],
        )
        .await
        .expect("registry row");
    id
}

/// Insert a minimal validated `reborn_tool_skills` row (class 13 — ToolSkill).
/// ToolSkill deps route to the rust channel in `resolve_dependencies`.
async fn insert_validated_toolskill(
    pool: &deadpool_postgres::Pool,
    scope: &IntentScope,
    name: &str,
    dep_registry_json: Option<&str>,
) -> Uuid {
    let id = Uuid::new_v4();
    let client = pool.get().await.expect("pool client");
    let dep_json = dep_registry_json.unwrap_or("[]");
    client
        .execute(
            "INSERT INTO reborn_tool_skills
                (id, tenant_id, user_id, agent_id, project_id,
                 name, description, content,
                 class_code, source, validation_status, dependency_registry)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12::jsonb)",
            &[
                &id,
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &name,
                &"toolskill description",
                &"toolskill content",
                &(13i32),
                &"system",
                &"validated",
                &dep_json,
            ],
        )
        .await
        .expect("insert toolskill");
    // Belt-and-suspenders: V061 trigger fires on INSERT but guard for test isolation.
    client
        .execute(
            "INSERT INTO reborn_components (id, tenant_id, user_id, agent_id, project_id, class_code)
             VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT (id) DO NOTHING",
            &[
                &id,
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &(13i32),
            ],
        )
        .await
        .expect("registry row");
    id
}

// ─── J.1 tests ───────────────────────────────────────────────────────────────

/// J.1: Seeding a skill's `intent_examples` into `reborn_intent_inputs`
/// (simulating the composition-layer J.1 wiring) and resolving via
/// `resolve_intent` returns a `Match` for each example.
///
/// This test verifies the **wiring contract**: given that a skill row exists
/// with `validation_status = 'validated'`, calling `seed_intent_input` for
/// each `{input, class}` entry in `intent_examples` (which is what the J.1
/// hook does) makes the examples resolvable via `resolve_intent`.
#[tokio::test]
async fn skill_intent_examples_seed_and_resolve() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let pool = Arc::new(rig.pool);
    let scope = unique_scope();

    // Create a real skill component id to associate the intents with.
    let skill_id = Uuid::new_v4();
    let client = pool.get().await.expect("pool client");
    client
        .execute(
            "INSERT INTO reborn_skills
                (id, tenant_id, user_id, agent_id, project_id,
                 name, description, body, compatibility,
                 class_code, consumer_tags, intent_examples,
                 source, validation_status)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,'[]'::jsonb,$12,$13)",
            &[
                &skill_id,
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &format!("skill-{}", Uuid::new_v4().simple()),
                &"Use this skill for J.1 integration test",
                &"Skill body.",
                &"brassclaw-class:llm",
                &(3i16),
                &vec!["02:orchestrator", "03:llm"],
                &"system",
                &"validated",
            ],
        )
        .await
        .expect("insert skill row");

    // Two intent examples: one word-class, one sentence-class.
    let word_query = format!("jitest-{}", Uuid::new_v4().simple());
    let sentence_query = format!("list all open items for test {}", Uuid::new_v4());

    // J.1 wiring: seed each example into reborn_intent_inputs.
    seed_intent_input(
        &pool,
        &scope,
        &word_query,
        InputClass::Word,
        skill_id,
        3, // skill class_code = 3 (Llm)
        IntentSource::Seeded,
        None,
    )
    .await
    .expect("seed word example");

    seed_intent_input(
        &pool,
        &scope,
        &sentence_query,
        InputClass::Sentence,
        skill_id,
        3,
        IntentSource::Seeded,
        None,
    )
    .await
    .expect("seed sentence example");

    // Now resolve_intent should find the skill for each query.
    let word_result = resolve_intent(&pool, &scope, &word_query)
        .await
        .expect("resolve word");
    match word_result {
        IntentResolution::Match { component_id, .. } => {
            assert_eq!(component_id, skill_id, "word query must resolve to skill_id");
        }
        other => panic!("expected Match for word query, got {other:?}"),
    }

    let sentence_result = resolve_intent(&pool, &scope, &sentence_query)
        .await
        .expect("resolve sentence");
    match sentence_result {
        IntentResolution::Match { component_id, .. } => {
            assert_eq!(
                component_id, skill_id,
                "sentence query must resolve to skill_id"
            );
        }
        other => panic!("expected Match for sentence query, got {other:?}"),
    }
}

// ─── J.3 tests ───────────────────────────────────────────────────────────────

/// J.3: `dependency_registry` with `[all]` returns the full transitive closure.
/// Chain: root → A (all) → B (no sub-deps).
#[tokio::test]
async fn resolve_deps_all_traversal() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let pool = Arc::new(rig.pool);
    let scope = unique_scope();
    let comp_scope = component_scope_from(&scope);

    // B: leaf, no deps.
    let b_id = insert_validated_spec(
        &pool,
        &scope,
        &format!("b-{}", Uuid::new_v4().simple()),
        Some("[]"),
    )
    .await;
    // A: depends on B at index 0 (All — but B has no deps so depth stops).
    let a_dep_reg = serde_json::json!([
        {"idx": 0, "component_id": b_id, "class_code": 12, "label": "b"}
    ]);
    let a_id = insert_validated_spec(
        &pool,
        &scope,
        &format!("a-{}", Uuid::new_v4().simple()),
        Some(&a_dep_reg.to_string()),
    )
    .await;
    // root: depends on A at index 0 with `All` sub-expression.
    let root_dep_reg = serde_json::json!([
        {"idx": 0, "component_id": a_id, "class_code": 12, "label": "a"}
    ]);
    let root_id = insert_validated_spec(
        &pool,
        &scope,
        &format!("root-{}", Uuid::new_v4().simple()),
        Some(&root_dep_reg.to_string()),
    )
    .await;

    // Expr: index 0 with sub=All → fetches A + recurses into A's deps (B).
    let expr: DependencyExpr = vec![DependencyNode {
        idx: 0,
        sub: Some(DependencySubExpr::All),
    }];
    let mut visited = std::collections::HashSet::new();
    let (orch, rust) = resolve_dependencies(&pool, &comp_scope, root_id, &expr, &mut visited)
        .await
        .expect("resolve_dependencies");

    let ids: Vec<Uuid> = orch.iter().map(|i| i.id).collect();
    assert!(ids.contains(&a_id), "A should be in orchestrator_items");
    assert!(
        ids.contains(&b_id),
        "B should be in orchestrator_items (transitive via All)"
    );
    assert!(rust.is_empty(), "no ToolSkill deps → rust_items empty");
}

/// J.3: Selective traversal with indices 2 and 6 fetches only those components,
/// skipping index 4.
#[tokio::test]
async fn resolve_deps_selective_indices() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let pool = Arc::new(rig.pool);
    let scope = unique_scope();
    let comp_scope = component_scope_from(&scope);

    let c2 = insert_validated_spec(&pool, &scope, &format!("c2-{}", Uuid::new_v4().simple()), None).await;
    let c4 = insert_validated_spec(&pool, &scope, &format!("c4-{}", Uuid::new_v4().simple()), None).await;
    let c6 = insert_validated_spec(&pool, &scope, &format!("c6-{}", Uuid::new_v4().simple()), None).await;

    let root_dep_reg = serde_json::json!([
        {"idx": 2, "component_id": c2, "class_code": 12, "label": "c2"},
        {"idx": 4, "component_id": c4, "class_code": 12, "label": "c4"},
        {"idx": 6, "component_id": c6, "class_code": 12, "label": "c6"}
    ]);
    let root_id = insert_validated_spec(
        &pool,
        &scope,
        &format!("root-{}", Uuid::new_v4().simple()),
        Some(&root_dep_reg.to_string()),
    )
    .await;

    // Select only indices 2 and 6 (no sub — leaf only).
    let expr: DependencyExpr = vec![
        DependencyNode { idx: 2, sub: None },
        DependencyNode { idx: 6, sub: None },
    ];
    let mut visited = std::collections::HashSet::new();
    let (orch, _rust) = resolve_dependencies(&pool, &comp_scope, root_id, &expr, &mut visited)
        .await
        .expect("resolve_dependencies");

    let ids: Vec<Uuid> = orch.iter().map(|i| i.id).collect();
    assert!(ids.contains(&c2), "c2 (index 2) must be fetched");
    assert!(ids.contains(&c6), "c6 (index 6) must be fetched");
    assert!(!ids.contains(&c4), "c4 (index 4) must NOT be fetched");
}

/// J.3: A UUID already in the `visited` set is not re-fetched.
#[tokio::test]
async fn resolve_deps_deduplication() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let pool = Arc::new(rig.pool);
    let scope = unique_scope();
    let comp_scope = component_scope_from(&scope);

    let dep_id =
        insert_validated_spec(&pool, &scope, &format!("dep-{}", Uuid::new_v4().simple()), None)
            .await;
    let root_dep_reg = serde_json::json!([
        {"idx": 0, "component_id": dep_id, "class_code": 12, "label": "dep"}
    ]);
    let root_id = insert_validated_spec(
        &pool,
        &scope,
        &format!("root-{}", Uuid::new_v4().simple()),
        Some(&root_dep_reg.to_string()),
    )
    .await;

    let expr: DependencyExpr = vec![DependencyNode { idx: 0, sub: None }];
    // Pre-populate visited with dep_id so it is "already seen".
    let mut visited = std::collections::HashSet::new();
    visited.insert(dep_id);

    let (orch, _rust) = resolve_dependencies(&pool, &comp_scope, root_id, &expr, &mut visited)
        .await
        .expect("resolve_dependencies");

    assert!(
        orch.is_empty(),
        "dep already in visited must be skipped; got {:?}",
        orch.iter().map(|i| i.id).collect::<Vec<_>>()
    );
}

/// J.3: Cycle A → B → A terminates without infinite recursion.
#[tokio::test]
async fn resolve_deps_cycle_guard() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let pool = Arc::new(rig.pool);
    let scope = unique_scope();
    let comp_scope = component_scope_from(&scope);

    let a_id = Uuid::new_v4();
    let b_id = Uuid::new_v4();

    // A→B and B→A (cycle).
    let a_dep = serde_json::json!([{"idx": 0, "component_id": b_id, "class_code": 12, "label": "b"}]);
    let b_dep = serde_json::json!([{"idx": 0, "component_id": a_id, "class_code": 12, "label": "a"}]);

    let client = pool.get().await.expect("pool client");
    for (id, name_suffix, dep) in
        [(a_id, "a", &a_dep), (b_id, "b", &b_dep)]
    {
        let name = format!("{name_suffix}-{}", Uuid::new_v4().simple());
        client
            .execute(
                "INSERT INTO reborn_specs
                    (id, tenant_id, user_id, agent_id, project_id,
                     name, description, content, class_code,
                     source, validation_status, dependency_registry)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12::jsonb)",
                &[
                    &id,
                    &scope.tenant_id,
                    &scope.user_id,
                    &scope.agent_id,
                    &scope.project_id,
                    &name,
                    &"desc",
                    &"content",
                    &(12i32),
                    &"system",
                    &"validated",
                    &dep.to_string(),
                ],
            )
            .await
            .expect("insert spec");
        client
            .execute(
                "INSERT INTO reborn_components (id, tenant_id, user_id, agent_id, project_id, class_code)
                 VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT (id) DO NOTHING",
                &[
                    &id,
                    &scope.tenant_id,
                    &scope.user_id,
                    &scope.agent_id,
                    &scope.project_id,
                    &(12i32),
                ],
            )
            .await
            .expect("registry");
    }

    let expr: DependencyExpr = vec![DependencyNode {
        idx: 0,
        sub: Some(DependencySubExpr::All),
    }];
    let mut visited = std::collections::HashSet::new();
    // Must complete (not stack-overflow) and return B without re-visiting A.
    let result = resolve_dependencies(&pool, &comp_scope, a_id, &expr, &mut visited).await;
    assert!(result.is_ok(), "cycle must not error: {:?}", result.err());
    let (orch, _) = result.unwrap();
    let ids: Vec<Uuid> = orch.iter().map(|i| i.id).collect();
    assert!(ids.contains(&b_id), "B must appear in results");
    assert!(!ids.contains(&a_id), "A must not re-appear (cycle guard via visited)");
}

/// J.3: class-13 ToolSkill dependencies route to `rust_items`;
/// class-12 Spec dependencies route to `orchestrator_items`.
#[tokio::test]
async fn resolve_deps_channel_routing() {
    let rig = match pg_rig_or_skip().await {
        Some(r) => r,
        None => return,
    };
    let pool = Arc::new(rig.pool);
    let scope = unique_scope();
    let comp_scope = component_scope_from(&scope);

    let ts_id = insert_validated_toolskill(
        &pool,
        &scope,
        &format!("ts-{}", Uuid::new_v4().simple()),
        None,
    )
    .await;
    let sp_id = insert_validated_spec(
        &pool,
        &scope,
        &format!("sp-{}", Uuid::new_v4().simple()),
        None,
    )
    .await;

    let root_dep_reg = serde_json::json!([
        {"idx": 0, "component_id": ts_id, "class_code": 13, "label": "ts"},
        {"idx": 1, "component_id": sp_id, "class_code": 12, "label": "sp"}
    ]);
    let root_id = insert_validated_spec(
        &pool,
        &scope,
        &format!("root-{}", Uuid::new_v4().simple()),
        Some(&root_dep_reg.to_string()),
    )
    .await;

    let expr: DependencyExpr = vec![
        DependencyNode { idx: 0, sub: None },
        DependencyNode { idx: 1, sub: None },
    ];
    let mut visited = std::collections::HashSet::new();
    let (orch, rust) = resolve_dependencies(&pool, &comp_scope, root_id, &expr, &mut visited)
        .await
        .expect("resolve_dependencies");

    assert!(
        rust.iter().any(|i| i.id == ts_id),
        "ToolSkill (class 13) must be in rust_items; got {:?}",
        rust.iter().map(|i| i.id).collect::<Vec<_>>()
    );
    assert!(
        orch.iter().any(|i| i.id == sp_id),
        "Spec (class 12) must be in orchestrator_items; got {:?}",
        orch.iter().map(|i| i.id).collect::<Vec<_>>()
    );
}
