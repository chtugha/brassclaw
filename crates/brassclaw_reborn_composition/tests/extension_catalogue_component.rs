//! Phase C integration tests for the class-23 (ExtensionCatalogue) retrieval
//! arms in `brassclaw_engine::memory::retrieval_source`:
//! - `PostgresSource::fetch_for_consumer` UNION ALL arm (class 23) — the
//!   `02:orchestrator` consumer-tag delivery path. `overview_doc` is projected
//!   as `effective_content` via `COALESCE(NULLIF(prior_knowledge_content,''),
//!   overview_doc)`.
//! - `fetch_component_by_id` UUID-lookup arm (class 23) — the direct UUID path.
//!
//! Each test starts an isolated Postgres-16 testcontainer, runs the full
//! migration set (V000–V053, so `reborn_extension_catalogues` exists), and
//! returns early (pass) when docker/testcontainers is unavailable. Gated to
//! `skills-db` because `PostgresSource` / `fetch_component_by_id` are
//! skills-db-only.

#![cfg(feature = "skills-db")]

use std::sync::Arc;

use brassclaw_engine::memory::retrieval_source::{
    ComponentScope, PostgresSource, RetrievalSource, fetch_component_by_id,
};

struct PgRig {
    // Held for the test's lifetime so the container stays up.
    _container: testcontainers_modules::testcontainers::ContainerAsync<
        testcontainers_modules::postgres::Postgres,
    >,
    pool: deadpool_postgres::Pool,
}

/// Start an isolated Postgres-16 testcontainer, build a pool, and run every
/// migration (V000–V053). Returns `None` (skip) when docker is unavailable.
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
            eprintln!(
                "skipping extension_catalogue_component tests: docker/testcontainers unavailable ({error})"
            );
            return None;
        }
    };
    let host = match container.get_host().await {
        Ok(h) => h,
        Err(error) => {
            eprintln!("skipping extension_catalogue_component tests: no host ({error})");
            return None;
        }
    };
    let port = match container.get_host_port_ipv4(5432).await {
        Ok(p) => p,
        Err(error) => {
            eprintln!("skipping extension_catalogue_component tests: no port ({error})");
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

fn test_scope() -> ComponentScope {
    ComponentScope {
        tenant_id: "t".into(),
        user_id: "u".into(),
        agent_id: "a".into(),
        project_id: "p".into(),
    }
}

/// Insert a validated `reborn_extension_catalogues` row that is deliverable to
/// `02:orchestrator` — `validation_status = 'validated'` and NO `05:validator`
/// tag, so the SEC-01 delivery gate (`'05:validator' != ALL(consumer_tags)`)
/// passes. UUID-derived name keeps parallel runs off the `UNIQUE(scope, name)`
/// constraint. Carries one task group + one child component id (the catalogue
/// shape, §0.2). Returns the assigned UUID.
async fn insert_validated_extension_catalogue(
    pool: &deadpool_postgres::Pool,
    scope: &ComponentScope,
) -> uuid::Uuid {
    let name = format!("cat-{}", uuid::Uuid::new_v4());
    let desc = "Catalogue covering local file management".to_string();
    let overview =
        "This catalogue covers local file management. Its Recipes handle these task groups..."
            .to_string();
    let task_groups = serde_json::json!([
        {
            "group_name": "file-management",
            "summary": "Local file management recipes",
            "recipe_ids": ["recipe-read-file", "recipe-write-file"]
        }
    ]);
    let child_ids = vec![uuid::Uuid::new_v4()];
    let tags = vec!["02:orchestrator".to_string()];
    let client = pool.get().await.expect("pool client");
    let row = client
        .query_one(
            "INSERT INTO reborn_extension_catalogues
                 (tenant_id, user_id, agent_id, project_id,
                  name, description, version, overview_doc,
                  task_groups, child_component_ids, consumer_tags, validation_status)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, 'validated')
             RETURNING id",
            &[
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &name,
                &desc,
                &"1.0",
                &overview,
                &task_groups,
                &child_ids,
                &tags,
            ],
        )
        .await
        .expect("insert validated extension_catalogue");
    row.get(0)
}

#[tokio::test]
async fn extension_catalogue_retrieved_via_fetch_for_consumer() {
    let Some(rig) = pg_rig_or_skip().await else {
        return;
    };
    let scope = test_scope();
    let id = insert_validated_extension_catalogue(&rig.pool, &scope).await;

    // The UNION ALL fallback projects every validated component table — the
    // class-23 arm must surface this row for the `02:orchestrator` consumer,
    // with `overview_doc` as `effective_content`.
    let source = PostgresSource::new(Arc::new(rig.pool.clone()));
    let items = source
        .fetch_for_consumer(&scope, "", 10_000, "02:orchestrator")
        .await
        .expect("fetch_for_consumer");

    let cat = items
        .iter()
        .find(|i| i.id == id)
        .expect("extension_catalogue row retrieved via the UNION ALL arm");
    assert_eq!(cat.class_code, 23);
    assert!(cat.name.starts_with("cat-"));
    assert!(
        cat.effective_content
            .contains("This catalogue covers local file management")
    );
}

#[tokio::test]
async fn extension_catalogue_retrieved_via_fetch_component_by_id() {
    let Some(rig) = pg_rig_or_skip().await else {
        return;
    };
    let scope = test_scope();
    let id = insert_validated_extension_catalogue(&rig.pool, &scope).await;

    // The direct UUID-lookup path must map class 23 →
    // `reborn_extension_catalogues` and return the row through the SEC-01 gate.
    let items = fetch_component_by_id(&rig.pool, &scope, id, 23)
        .await
        .expect("fetch_component_by_id");
    assert_eq!(items.len(), 1, "exactly one row for the UUID + class 23");
    let cat = &items[0];
    assert_eq!(cat.id, id);
    assert_eq!(cat.class_code, 23);
    assert!(cat.name.starts_with("cat-"));
    assert!(
        cat.effective_content
            .contains("This catalogue covers local file management")
    );
}
