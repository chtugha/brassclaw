//! PostgreSQL integration tests for [`PgCapabilityLeaseStore`].
//!
//! These tests require a live PostgreSQL server and the `brassclaw_capability_leases`
//! table (V007 migration). Run with:
//!
//! ```bash
//! BRASSCLAW_PG_URL=postgresql://brassclaw@127.0.0.1:5434/brassclaw \
//!   cargo test -p brassclaw_authorization --features integration
//! ```
//!
//! When no database URL is configured, the tests start an isolated embedded PostgreSQL instance.
//! Each test isolates itself by using a fresh UUID as `tenant_id`, so tests can
//! run concurrently against a shared schema without interference.

#![cfg(feature = "integration")]

use std::sync::Arc;

use brassclaw_authorization::{
    CapabilityLease, CapabilityLeaseError, CapabilityLeaseStatus, CapabilityLeaseStore,
    PgCapabilityLeaseStore,
};
use brassclaw_embedded_postgres::{EmbeddedPostgresConfig, ManagedPostgres};
use brassclaw_host_api::{
    AgentId, CapabilityGrant, CapabilityGrantId, CapabilityId, CapabilitySet, CorrelationId,
    EffectKind, ExecutionContext, ExtensionId, GrantConstraints, InvocationFingerprint,
    InvocationId, MountView, NetworkPolicy, Principal, ProjectId, ResourceScope, RuntimeKind,
    TenantId, ThreadId, TrustClass, UserId,
};
use brassclaw_pg::{migrations::run_migrations, pool::build_pool};
use tempfile::TempDir;
use tokio::net::TcpListener;

// ── Test helpers ─────────────────────────────────────────────────────────────

/// Generate a fresh unique tenant ID string for test isolation.
fn fresh_tenant() -> String {
    // CapabilityGrantId::new() generates a UUID-v4 internally; format it as a
    // simple hex string prefixed with "t" so it passes TenantId validation.
    format!("t{}", CapabilityGrantId::new().as_uuid().simple())
}

struct TestDatabase {
    pool: Arc<brassclaw_pg::PgPool>,
    _managed: Option<ManagedPostgres>,
    _temp_dir: Option<TempDir>,
}

async fn test_database() -> TestDatabase {
    let configured_url = std::env::var("BRASSCLAW_PG_URL")
        .or_else(|_| std::env::var("TEST_PG_URL"))
        .ok();
    let (url, managed, temp_dir) = if let Some(url) = configured_url {
        (url, None, None)
    } else {
        let temp_dir = TempDir::new().expect("create embedded PostgreSQL temp directory");
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("reserve embedded PostgreSQL port");
        let port = listener.local_addr().expect("read reserved port").port();
        drop(listener);
        let config = EmbeddedPostgresConfig {
            port,
            data_dir: temp_dir.path().join("data"),
            bin_cache_dir: std::env::temp_dir().join("brassclaw-postgresql-bin-cache"),
            database: "brassclaw".to_string(),
            superuser: "brassclaw".to_string(),
        };
        let managed = ManagedPostgres::start(config)
            .await
            .expect("start embedded PostgreSQL");
        (managed.connection_url(), Some(managed), Some(temp_dir))
    };
    let pool = Arc::new(build_pool(&url).expect("build PostgreSQL test pool"));
    run_migrations(&pool)
        .await
        .expect("apply PostgreSQL test migrations");
    TestDatabase {
        pool,
        _managed: managed,
        _temp_dir: temp_dir,
    }
}

fn make_scope(tenant_id: &str, user_id: &str) -> ResourceScope {
    ResourceScope {
        tenant_id: TenantId::new(tenant_id).unwrap(),
        user_id: UserId::new(user_id).unwrap(),
        agent_id: Some(AgentId::new("test-agent").unwrap()),
        project_id: Some(ProjectId::new("test-project").unwrap()),
        thread_id: Some(ThreadId::new("test-thread").unwrap()),
        invocation_id: InvocationId::new(),
    }
}

fn execution_context(scope: ResourceScope) -> ExecutionContext {
    ExecutionContext {
        invocation_id: scope.invocation_id,
        correlation_id: CorrelationId::new(),
        process_id: None,
        parent_process_id: None,
        tenant_id: scope.tenant_id.clone(),
        user_id: scope.user_id.clone(),
        agent_id: scope.agent_id.clone(),
        project_id: scope.project_id.clone(),
        thread_id: scope.thread_id.clone(),
        extension_id: ExtensionId::new("test-extension").unwrap(),
        runtime: RuntimeKind::FirstParty,
        trust: TrustClass::Sandbox,
        grants: CapabilitySet::default(),
        mounts: MountView::default(),
        resource_scope: scope,
    }
}

fn base_constraints(effects: Vec<EffectKind>, max_invocations: Option<u64>) -> GrantConstraints {
    GrantConstraints {
        allowed_effects: effects,
        mounts: MountView::default(),
        network: NetworkPolicy::default(),
        secrets: Vec::new(),
        resource_ceiling: None,
        expires_at: None,
        max_invocations,
    }
}

fn make_lease(scope: ResourceScope, capability: &str) -> CapabilityLease {
    let cap_id = CapabilityId::new(capability).unwrap();
    let grant = CapabilityGrant {
        id: CapabilityGrantId::new(),
        capability: cap_id,
        grantee: Principal::User(scope.user_id.clone()),
        issued_by: Principal::HostRuntime,
        constraints: base_constraints(vec![EffectKind::DispatchCapability], None),
    };
    CapabilityLease::new(scope, grant)
}

fn make_multi_use_lease(scope: ResourceScope, capability: &str, uses: u64) -> CapabilityLease {
    let cap_id = CapabilityId::new(capability).unwrap();
    let grant = CapabilityGrant {
        id: CapabilityGrantId::new(),
        capability: cap_id,
        grantee: Principal::User(scope.user_id.clone()),
        issued_by: Principal::HostRuntime,
        constraints: base_constraints(vec![EffectKind::DispatchCapability], Some(uses)),
    };
    CapabilityLease::new(scope, grant)
}

fn fingerprint_for(scope: &ResourceScope, capability: &str) -> InvocationFingerprint {
    let cap_id = CapabilityId::new(capability).unwrap();
    InvocationFingerprint::for_dispatch(
        scope,
        &cap_id,
        &brassclaw_host_api::ResourceEstimate::default(),
        &serde_json::json!({}),
    )
    .unwrap()
}

fn make_fingerprinted_lease(
    scope: ResourceScope,
    capability: &str,
    fp: InvocationFingerprint,
) -> CapabilityLease {
    let cap_id = CapabilityId::new(capability).unwrap();
    let grant = CapabilityGrant {
        id: CapabilityGrantId::new(),
        capability: cap_id,
        grantee: Principal::User(scope.user_id.clone()),
        issued_by: Principal::HostRuntime,
        constraints: base_constraints(vec![EffectKind::DispatchCapability], Some(1)),
    };
    let mut lease = CapabilityLease::new(scope, grant);
    lease.invocation_fingerprint = Some(fp);
    lease
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// Two concurrent claims of the same active fingerprinted lease: exactly one
/// succeeds and the other receives `InactiveLease` (the loser sees Claimed).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_claims_exactly_one_succeeds() {
    let database = test_database().await;
    let pool = Arc::clone(&database.pool);
    let tenant = fresh_tenant();
    let scope = make_scope(&tenant, "user-alice");
    let fp = fingerprint_for(&scope, "test.echo");
    let lease = make_fingerprinted_lease(scope.clone(), "test.echo", fp.clone());
    let lease_id = lease.grant.id;

    let store = Arc::new(PgCapabilityLeaseStore::new(Arc::clone(&pool), &tenant));
    store.issue(lease).await.unwrap();

    let s1 = Arc::clone(&store);
    let s2 = Arc::clone(&store);
    let sc1 = scope.clone();
    let sc2 = scope.clone();
    let fp1 = fp.clone();
    let fp2 = fp.clone();

    let t1 = tokio::spawn(async move { s1.claim(&sc1, lease_id, &fp1).await });
    let t2 = tokio::spawn(async move { s2.claim(&sc2, lease_id, &fp2).await });

    let r1 = t1.await.unwrap();
    let r2 = t2.await.unwrap();

    let successes = [&r1, &r2].iter().filter(|r| r.is_ok()).count();
    let failures = [&r1, &r2]
        .iter()
        .filter(|r| matches!(r, Err(CapabilityLeaseError::InactiveLease { .. })))
        .count();

    assert_eq!(
        successes, 1,
        "exactly one claim must succeed; got r1={r1:?} r2={r2:?}"
    );
    assert_eq!(
        failures, 1,
        "exactly one claim must fail as InactiveLease; got r1={r1:?} r2={r2:?}"
    );
}

/// Two concurrent consumes of a one-shot lease: exactly one succeeds.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_consumes_one_shot_exactly_one_succeeds() {
    let database = test_database().await;
    let pool = Arc::clone(&database.pool);
    let tenant = fresh_tenant();
    let scope = make_scope(&tenant, "user-bob");
    let lease = make_multi_use_lease(scope.clone(), "test.echo", 1);
    let lease_id = lease.grant.id;

    let store = Arc::new(PgCapabilityLeaseStore::new(Arc::clone(&pool), &tenant));
    store.issue(lease).await.unwrap();

    let s1 = Arc::clone(&store);
    let s2 = Arc::clone(&store);
    let sc1 = scope.clone();
    let sc2 = scope.clone();

    let t1 = tokio::spawn(async move { s1.consume(&sc1, lease_id).await });
    let t2 = tokio::spawn(async move { s2.consume(&sc2, lease_id).await });

    let r1 = t1.await.unwrap();
    let r2 = t2.await.unwrap();

    let successes = [&r1, &r2].iter().filter(|r| r.is_ok()).count();
    assert_eq!(
        successes, 1,
        "exactly one consume must succeed for a one-shot lease; r1={r1:?} r2={r2:?}"
    );
}

/// A two-invocation lease remains Active with one invocation after the first
/// consume and becomes Consumed with zero after the second.
#[tokio::test]
async fn two_invocation_lease_transitions_correctly() {
    let database = test_database().await;
    let pool = Arc::clone(&database.pool);
    let tenant = fresh_tenant();
    let scope = make_scope(&tenant, "user-carol");
    let lease = make_multi_use_lease(scope.clone(), "test.echo", 2);
    let lease_id = lease.grant.id;

    let store = PgCapabilityLeaseStore::new(Arc::clone(&pool), &tenant);
    store.issue(lease).await.unwrap();

    // First consume: should remain Active with 1 remaining.
    let after_first = store.consume(&scope, lease_id).await.unwrap();
    assert_eq!(
        after_first.status,
        CapabilityLeaseStatus::Active,
        "after first consume a 2-invocation lease must still be Active"
    );
    assert_eq!(
        after_first.grant.constraints.max_invocations,
        Some(1),
        "after first consume max_invocations must be 1"
    );

    // Second consume: should become Consumed with 0 remaining.
    let after_second = store.consume(&scope, lease_id).await.unwrap();
    assert_eq!(
        after_second.status,
        CapabilityLeaseStatus::Consumed,
        "after second consume a 2-invocation lease must be Consumed"
    );
    assert_eq!(
        after_second.grant.constraints.max_invocations,
        Some(0),
        "after second consume max_invocations must be 0"
    );
}

/// Consuming a revoked, consumed, or expired lease returns the same typed
/// errors as the in-memory store.
#[tokio::test]
async fn consume_terminal_lease_returns_typed_errors() {
    let database = test_database().await;
    let pool = Arc::clone(&database.pool);
    let tenant = fresh_tenant();
    let scope = make_scope(&tenant, "user-dave");
    let store = PgCapabilityLeaseStore::new(Arc::clone(&pool), &tenant);

    // ── Revoked ──────────────────────────────────────────────────────────────
    let lease_revoked = make_lease(scope.clone(), "test.revoked");
    let id_revoked = lease_revoked.grant.id;
    store.issue(lease_revoked).await.unwrap();
    store.revoke(&scope, id_revoked).await.unwrap();
    let result = store.consume(&scope, id_revoked).await;
    assert!(
        matches!(result, Err(CapabilityLeaseError::InactiveLease { .. })),
        "consuming a revoked lease must return InactiveLease, got {result:?}"
    );

    // ── Consumed (exhausted) ─────────────────────────────────────────────────
    let lease_1shot = make_multi_use_lease(scope.clone(), "test.oneshot", 1);
    let id_1shot = lease_1shot.grant.id;
    store.issue(lease_1shot).await.unwrap();
    store.consume(&scope, id_1shot).await.unwrap();
    let result2 = store.consume(&scope, id_1shot).await;
    assert!(
        matches!(result2, Err(CapabilityLeaseError::ExhaustedLease { .. })),
        "consuming an already-consumed lease must return ExhaustedLease, got {result2:?}"
    );

    // ── Unclaimed fingerprinted lease (not Claimed) ───────────────────────────
    let fp = fingerprint_for(&scope, "test.unclaimed");
    let lease_fp = make_fingerprinted_lease(scope.clone(), "test.unclaimed", fp);
    let id_fp = lease_fp.grant.id;
    store.issue(lease_fp).await.unwrap();
    let result3 = store.consume(&scope, id_fp).await;
    assert!(
        matches!(
            result3,
            Err(CapabilityLeaseError::UnclaimedFingerprintLease { .. })
        ),
        "consuming an unclaimed fingerprinted lease must return UnclaimedFingerprintLease, got {result3:?}"
    );
}

/// A scope with a different user cannot claim, consume, revoke, or read the
/// lease. Wrong-scope lookups must behave as unknown.
#[tokio::test]
async fn cross_user_cannot_access_lease() {
    let database = test_database().await;
    let pool = Arc::clone(&database.pool);
    let tenant = fresh_tenant();
    let owner_scope = make_scope(&tenant, "user-owner");
    let attacker_scope = make_scope(&tenant, "user-attacker");
    let fp = fingerprint_for(&owner_scope, "test.echo");
    let lease = make_fingerprinted_lease(owner_scope.clone(), "test.echo", fp.clone());
    let lease_id = lease.grant.id;

    let store = PgCapabilityLeaseStore::new(Arc::clone(&pool), &tenant);
    store.issue(lease).await.unwrap();

    // get: wrong user must see None
    let got = store.get(&attacker_scope, lease_id).await;
    assert!(
        got.is_none(),
        "cross-user get must return None, got {got:?}"
    );

    // claim: wrong user must get UnknownLease
    let result = store.claim(&attacker_scope, lease_id, &fp).await;
    assert!(
        matches!(result, Err(CapabilityLeaseError::UnknownLease { .. })),
        "cross-user claim must return UnknownLease, got {result:?}"
    );

    // consume: wrong user must get UnknownLease
    let result = store.consume(&attacker_scope, lease_id).await;
    assert!(
        matches!(result, Err(CapabilityLeaseError::UnknownLease { .. })),
        "cross-user consume must return UnknownLease, got {result:?}"
    );

    // revoke: wrong user must get UnknownLease
    let result = store.revoke(&attacker_scope, lease_id).await;
    assert!(
        matches!(result, Err(CapabilityLeaseError::UnknownLease { .. })),
        "cross-user revoke must return UnknownLease, got {result:?}"
    );
}

/// Duplicate `issue` does not report success (returns a Persistence error).
#[tokio::test]
async fn duplicate_issue_returns_error() {
    let database = test_database().await;
    let pool = Arc::clone(&database.pool);
    let tenant = fresh_tenant();
    let scope = make_scope(&tenant, "user-erin");
    let lease = make_lease(scope.clone(), "test.echo");

    let store = PgCapabilityLeaseStore::new(Arc::clone(&pool), &tenant);
    store.issue(lease.clone()).await.unwrap();

    // Issue again with the exact same lease (same id).
    let result = store.issue(lease).await;
    assert!(
        matches!(result, Err(CapabilityLeaseError::Persistence { .. })),
        "duplicate issue must return a Persistence error, got {result:?}"
    );
}

/// SQL status column and serialized JSON status remain synchronized after every
/// transition: the round-tripped lease loaded from the DB must have the expected
/// Rust enum variant, and the raw JSONB column must contain the serde variant name.
#[tokio::test]
async fn status_column_and_jsonb_are_synchronized() {
    let database = test_database().await;
    let pool = Arc::clone(&database.pool);
    let tenant = fresh_tenant();
    let scope = make_scope(&tenant, "user-frank");
    let store = PgCapabilityLeaseStore::new(Arc::clone(&pool), &tenant);
    let lease = make_lease(scope.clone(), "test.echo");
    let lease_id = lease.grant.id;

    // Issue — Active
    store.issue(lease).await.unwrap();
    assert_status_columns(&pool, &tenant, &lease_id, "active", "Active").await;

    // Revoke — Revoked
    store.revoke(&scope, lease_id).await.unwrap();
    assert_status_columns(&pool, &tenant, &lease_id, "revoked", "Revoked").await;

    // New lease for consume test
    let lease2 = make_multi_use_lease(scope.clone(), "test.echo2", 1);
    let lease_id2 = lease2.grant.id;
    store.issue(lease2).await.unwrap();
    assert_status_columns(&pool, &tenant, &lease_id2, "active", "Active").await;
    store.consume(&scope, lease_id2).await.unwrap();
    assert_status_columns(&pool, &tenant, &lease_id2, "consumed", "Consumed").await;

    // New fingerprinted lease for claim test
    let fp = fingerprint_for(&scope, "test.echo3");
    let lease3 = make_fingerprinted_lease(scope.clone(), "test.echo3", fp.clone());
    let lease_id3 = lease3.grant.id;
    store.issue(lease3).await.unwrap();
    assert_status_columns(&pool, &tenant, &lease_id3, "active", "Active").await;
    store.claim(&scope, lease_id3, &fp).await.unwrap();
    assert_status_columns(&pool, &tenant, &lease_id3, "claimed", "Claimed").await;
}

#[tokio::test]
async fn store_rejects_mismatched_tenant_and_filters_exact_scope() {
    let database = test_database().await;
    let pool = Arc::clone(&database.pool);
    let tenant = fresh_tenant();
    let other_tenant = fresh_tenant();
    let store = PgCapabilityLeaseStore::new(Arc::clone(&pool), &tenant);
    let wrong_tenant_scope = make_scope(&other_tenant, "user-scope");
    let wrong_tenant_lease = make_lease(wrong_tenant_scope.clone(), "test.wrong-tenant");

    assert!(matches!(
        store.issue(wrong_tenant_lease).await,
        Err(CapabilityLeaseError::Persistence { .. })
    ));
    assert!(store.leases_for_scope(&wrong_tenant_scope).await.is_empty());

    let first_scope = make_scope(&tenant, "user-scope");
    let mut second_scope = first_scope.clone();
    second_scope.invocation_id = InvocationId::new();
    let first_lease = make_lease(first_scope.clone(), "test.first");
    let second_lease = make_lease(second_scope.clone(), "test.second");
    let first_id = first_lease.grant.id;
    let second_id = second_lease.grant.id;
    store.issue(first_lease).await.unwrap();
    store.issue(second_lease).await.unwrap();

    assert!(store.get(&second_scope, first_id).await.is_none());
    assert!(matches!(
        store.consume(&second_scope, first_id).await,
        Err(CapabilityLeaseError::UnknownLease { .. })
    ));

    let visible = store.leases_for_scope(&first_scope).await;
    assert_eq!(visible.len(), 2);
    assert!(visible.iter().any(|lease| lease.grant.id == first_id));
    assert!(visible.iter().any(|lease| lease.grant.id == second_id));

    let active = store
        .active_leases_for_context(&execution_context(first_scope))
        .await;
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].grant.id, first_id);
    assert_ne!(active[0].grant.id, second_id);
}

/// Helper: assert both the SQL `status` column (lowercase) and the JSONB
/// `grant→status` field (serde variant name, e.g. "Active") hold the expected
/// values.
async fn assert_status_columns(
    pool: &brassclaw_pg::PgPool,
    tenant: &str,
    lease_id: &CapabilityGrantId,
    expected_sql: &str,
    expected_json: &str,
) {
    let client = pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT status, \"grant\"->>'status' AS json_status \
             FROM brassclaw_capability_leases \
             WHERE id = $1 AND tenant_id = $2",
            &[&lease_id.as_uuid().to_string(), &tenant],
        )
        .await
        .unwrap();
    let sql_status: String = row.get(0);
    let json_status: String = row.get(1);
    assert_eq!(
        sql_status, expected_sql,
        "SQL status column must be '{expected_sql}' for lease {lease_id}"
    );
    assert_eq!(
        json_status, expected_json,
        "JSONB grant.status must be '{expected_json}' for lease {lease_id}"
    );
}
