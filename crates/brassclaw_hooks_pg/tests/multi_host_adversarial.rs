//! Cross-backend multi-host adversarial suite — Postgres only.
//!
//! Each "host" is a distinct `PostgresPredicateStateBackend` pointing at the
//! SAME database (distinct `deadpool` pools over one Postgres), simulating
//! distinct processes.
//!
//! # Gating
//!
//! The whole binary is behind `--features integration` so default `cargo test`
//! stays fast. Tests additionally require a reachable server via
//! `BRASSCLAW_HOOKS_POSTGRES_URL` / `DATABASE_URL`; skipped (passing) when
//! the URL is absent.
//!
//! Scenarios:
//! 1. N concurrent writers across 2 hosts — no count desync, exactly-once.
//! 2. Cross-host replay — interleaved id submissions, exactly-once counting.
//! 3. LRU eviction race — concurrent inserts past the per-tenant quota.
//! 4. Per-key cap under attacker flood — fail-closed `WindowOverflow`, bounded.
//!    4b. Cap-boundary race: exactly one of two concurrent writers wins the last slot.
//! 5. Clock-skew — window follows the caller-supplied clock basis.

#![cfg(feature = "integration")]

/// Test Decimal value used in value-deduplication adversarial scenarios.
const TEST_VALUE: i64 = 50;

use std::sync::Arc;
use std::time::Duration;

use brassclaw_hooks::identity::{ExtensionId, HookId, HookLocalId, HookVersion};
use brassclaw_hooks::predicate_state::{
    InvocationKey, MAX_KEYS_PER_TENANT, MAX_SAMPLES_PER_KEY, PredicateBackendError,
    PredicateEventId, PredicateStateBackend, ValueKey,
};
use brassclaw_hooks_pg::PostgresPredicateStateBackend;
use brassclaw_host_api::TenantId;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn hook_id() -> HookId {
    HookId::derive(
        &ExtensionId::new("ext").expect("ext id"),
        "1.0",
        &HookLocalId::new("h").expect("hook local id"),
        HookVersion::ONE,
    )
}

fn tenant(name: &str) -> TenantId {
    TenantId::new(name).expect("tenant id")
}

fn ev(s: &str) -> PredicateEventId {
    PredicateEventId::new(s).expect("event id")
}

fn base() -> DateTime<Utc> {
    DateTime::from_timestamp(1_700_000_000, 0).expect("fixed timestamp")
}

fn at_secs(secs: i64) -> DateTime<Utc> {
    base() + chrono::Duration::seconds(secs)
}

fn at_millis(ms: i64) -> DateTime<Utc> {
    base() + chrono::Duration::milliseconds(ms)
}

fn inv_key(tenant_name: &str, capability: &str) -> InvocationKey {
    InvocationKey {
        hook_id: hook_id(),
        tenant_id: tenant(tenant_name),
        capability: capability.to_string(),
    }
}

fn val_key(tenant_name: &str, capability: &str, field: &str) -> ValueKey {
    ValueKey {
        hook_id: hook_id(),
        tenant_id: tenant(tenant_name),
        capability: capability.to_string(),
        field: field.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Postgres cluster
// ---------------------------------------------------------------------------

mod pg_cluster {
    use super::*;
    use deadpool_postgres::Pool;

    static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    const SCHEMA: &str = "hooks_parity_multihost";

    fn db_url() -> Option<String> {
        std::env::var("BRASSCLAW_HOOKS_POSTGRES_URL")
            .or_else(|_| std::env::var("DATABASE_URL"))
            .ok()
    }

    fn build_pool(url: &str) -> Option<Pool> {
        let config = url.parse::<tokio_postgres::Config>().ok()?;
        let manager = deadpool_postgres::Manager::new(config, tokio_postgres::NoTls);
        deadpool_postgres::Pool::builder(manager)
            .max_size(16)
            .post_create(deadpool_postgres::Hook::async_fn(|client, _| {
                Box::pin(async move {
                    client
                        .batch_execute(&format!("SET search_path TO {SCHEMA}"))
                        .await
                        .map_err(|e| deadpool_postgres::HookError::message(e.to_string()))?;
                    Ok(())
                })
            }))
            .build()
            .ok()
    }

    async fn prepare() -> Option<String> {
        let url = db_url()?;
        let (client, conn) = tokio_postgres::connect(&url, tokio_postgres::NoTls)
            .await
            .ok()?;
        tokio::spawn(conn);
        client
            .batch_execute(&format!("CREATE SCHEMA IF NOT EXISTS {SCHEMA}"))
            .await
            .ok()?;
        let pool = build_pool(&url)?;
        let backend = PostgresPredicateStateBackend::new(pool.clone());
        backend.run_migrations().await.ok()?;
        let c = pool.get().await.ok()?;
        c.batch_execute("TRUNCATE TABLE hooks_predicate_invocations, hooks_predicate_values")
            .await
            .ok()?;
        Some(url)
    }

    fn host(url: &str) -> Arc<dyn PredicateStateBackend> {
        Arc::new(PostgresPredicateStateBackend::new(
            build_pool(url).expect("pool"),
        ))
    }

    pub(super) async fn distinct_invocation_scopes(url: &str, tenant: &str) -> usize {
        let pool = build_pool(url).expect("pool");
        let client = pool.get().await.expect("pool get");
        let scope = brassclaw_hooks_pg::test_support::scope_hash_bytes(tenant);
        let row = client
            .query_one(
                "SELECT count(DISTINCT key_hash) FROM hooks_predicate_invocations \
                 WHERE scope_hash = $1",
                &[&&scope[..]],
            )
            .await
            .expect("count distinct key_hash");
        let v: i64 = row.get(0);
        v.max(0) as usize
    }

    fn require_postgres() -> bool {
        std::env::var("BRASSCLAW_REQUIRE_POSTGRES")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    }

    macro_rules! pg_skip_or {
        ($url:ident) => {
            match prepare().await {
                Some(u) => u,
                None => {
                    if require_postgres() {
                        panic!(
                            "BRASSCLAW_REQUIRE_POSTGRES=1 but Postgres multi-host adversarial \
                             could not run (BRASSCLAW_HOOKS_POSTGRES_URL / DATABASE_URL \
                             unset or unreachable). Refusing to skip-pass under the CI hard-gate."
                        );
                    }
                    eprintln!(
                        "skipping postgres multi-host adversarial: \
                         BRASSCLAW_HOOKS_POSTGRES_URL / DATABASE_URL not set"
                    );
                    return;
                }
            }
        };
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[allow(clippy::await_holding_lock)]
    pub(super) async fn postgres_concurrent_writers_no_desync() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let url = pg_skip_or!(url);
        scenario_concurrent_writers_no_desync(host(&url), host(&url), host(&url)).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[allow(clippy::await_holding_lock)]
    pub(super) async fn postgres_cross_host_replay_exactly_once() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let url = pg_skip_or!(url);
        scenario_cross_host_replay_exactly_once(host(&url), host(&url)).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[allow(clippy::await_holding_lock)]
    pub(super) async fn postgres_lru_eviction_race_holds_quota() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let url = pg_skip_or!(url);
        let url_for_count = url.clone();
        scenario_lru_eviction_race_holds_quota(host(&url), move |tenant| {
            let url = url_for_count.clone();
            async move { distinct_invocation_scopes(&url, tenant).await }
        })
        .await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[allow(clippy::await_holding_lock)]
    pub(super) async fn postgres_per_key_cap_fails_closed_under_flood() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let url = pg_skip_or!(url);
        scenario_per_key_cap_fails_closed_under_flood(host(&url), host(&url)).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[allow(clippy::await_holding_lock)]
    pub(super) async fn postgres_cap_boundary_race_admits_exactly_one() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let url = pg_skip_or!(url);
        scenario_cap_boundary_race_admits_exactly_one(host(&url), host(&url), host(&url)).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[allow(clippy::await_holding_lock)]
    pub(super) async fn postgres_clock_skew_follows_caller_clock() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let url = pg_skip_or!(url);
        scenario_clock_skew_follows_caller_clock(host(&url), host(&url)).await;
    }
}

// ---------------------------------------------------------------------------
// Shared scenario implementations
// ---------------------------------------------------------------------------

async fn scenario_concurrent_writers_no_desync(
    host_a: Arc<dyn PredicateStateBackend>,
    host_b: Arc<dyn PredicateStateBackend>,
    observer: Arc<dyn PredicateStateBackend>,
) {
    let key = inv_key("alpha", "cap.concurrent");
    let now = at_secs(0);
    let window = Duration::from_secs(60);
    const N: usize = 48;

    let mut handles = Vec::with_capacity(N);
    for i in 0..N {
        let backend = if i % 2 == 0 {
            Arc::clone(&host_a)
        } else {
            Arc::clone(&host_b)
        };
        let key = key.clone();
        handles.push(tokio::spawn(async move {
            backend
                .record_invocation(&key, &ev(&format!("evt-{i}")), now, window)
                .await
                .expect("record ok")
        }));
    }
    let mut counts: Vec<u32> = Vec::with_capacity(N);
    for h in handles {
        counts.push(h.await.expect("joined"));
    }
    counts.sort_unstable();
    let expected: Vec<u32> = (1..=N as u32).collect();
    assert_eq!(
        counts, expected,
        "the N concurrent distinct-id writers must each return a distinct in-window count"
    );

    let final_count = observer
        .record_invocation(&key, &ev("evt-0"), now, window)
        .await
        .expect("read ok");
    assert_eq!(final_count as usize, N);
}

async fn scenario_cross_host_replay_exactly_once(
    host_a: Arc<dyn PredicateStateBackend>,
    host_b: Arc<dyn PredicateStateBackend>,
) {
    let key = inv_key("alpha", "cap.replay");
    let window = Duration::from_secs(60);

    let c1 = host_a
        .record_invocation(&key, &ev("shared-evt"), at_secs(0), window)
        .await
        .expect("ok");
    assert_eq!(c1, 1);
    let c2 = host_b
        .record_invocation(&key, &ev("shared-evt"), at_secs(1), window)
        .await
        .expect("ok");
    assert_eq!(c2, 1, "cross-host replay must not double-count");
    let c3 = host_b
        .record_invocation(&key, &ev("fresh-evt"), at_secs(2), window)
        .await
        .expect("ok");
    assert_eq!(c3, 2);

    let vkey = val_key("alpha", "cap.spend", "amount");
    let s1 = host_a
        .record_value(
            &vkey,
            &ev("v-shared"),
            at_secs(0),
            Decimal::from(TEST_VALUE),
            window,
        )
        .await
        .expect("ok");
    assert_eq!(s1, Decimal::from(TEST_VALUE));
    let s2 = host_b
        .record_value(
            &vkey,
            &ev("v-shared"),
            at_secs(1),
            Decimal::from(TEST_VALUE),
            window,
        )
        .await
        .expect("ok");
    assert_eq!(
        s2,
        Decimal::from(TEST_VALUE),
        "cross-host value replay must not double-count"
    );
}

async fn scenario_per_key_cap_fails_closed_under_flood(
    host_a: Arc<dyn PredicateStateBackend>,
    host_b: Arc<dyn PredicateStateBackend>,
) {
    let key = inv_key("alpha", "cap.hot");
    let window = Duration::from_secs(3600);

    for i in 0..MAX_SAMPLES_PER_KEY {
        host_a
            .record_invocation(&key, &ev(&format!("e-{i}")), at_millis(i as i64), window)
            .await
            .expect("inserts up to the cap succeed");
    }

    let mut handles = Vec::new();
    for h in [Arc::clone(&host_a), Arc::clone(&host_b)] {
        for j in 0..8 {
            let h = Arc::clone(&h);
            let key = key.clone();
            handles.push(tokio::spawn(async move {
                h.record_invocation(
                    &key,
                    &ev(&format!("flood-{j}-{:p}", Arc::as_ptr(&h))),
                    at_millis(MAX_SAMPLES_PER_KEY as i64 + j),
                    window,
                )
                .await
            }));
        }
    }
    for handle in handles {
        let res = handle.await.expect("joined");
        assert!(
            matches!(res, Err(PredicateBackendError::WindowOverflow { .. })),
            "flood past the per-key cap must fail closed, got {res:?}"
        );
    }

    let replay = host_b
        .record_invocation(
            &key,
            &ev("e-0"),
            at_millis(MAX_SAMPLES_PER_KEY as i64 + 100),
            window,
        )
        .await
        .expect("replay of an in-window id must dedup");
    assert_eq!(replay as usize, MAX_SAMPLES_PER_KEY);
}

async fn scenario_cap_boundary_race_admits_exactly_one(
    host_a: Arc<dyn PredicateStateBackend>,
    host_b: Arc<dyn PredicateStateBackend>,
    observer: Arc<dyn PredicateStateBackend>,
) {
    let key = inv_key("alpha", "cap.boundary-race");
    let window = Duration::from_secs(3600);

    for i in 0..(MAX_SAMPLES_PER_KEY - 1) {
        host_a
            .record_invocation(&key, &ev(&format!("fill-{i}")), at_millis(i as i64), window)
            .await
            .expect("fill below the cap succeeds");
    }

    let now = at_millis(MAX_SAMPLES_PER_KEY as i64);
    let ha = {
        let key = key.clone();
        let host_a = Arc::clone(&host_a);
        tokio::spawn(async move {
            host_a
                .record_invocation(&key, &ev("race-a"), now, window)
                .await
        })
    };
    let hb = {
        let key = key.clone();
        let host_b = Arc::clone(&host_b);
        tokio::spawn(async move {
            host_b
                .record_invocation(&key, &ev("race-b"), now, window)
                .await
        })
    };
    let ra = ha.await.expect("joined a");
    let rb = hb.await.expect("joined b");

    let results = [&ra, &rb];
    let oks: Vec<u32> = results
        .iter()
        .filter_map(|r| r.as_ref().ok().copied())
        .collect();
    let overflows = results
        .iter()
        .filter(|r| matches!(r, Err(PredicateBackendError::WindowOverflow { .. })))
        .count();
    assert_eq!(
        oks.len(),
        1,
        "exactly one writer wins; got ra={ra:?}, rb={rb:?}"
    );
    assert_eq!(overflows, 1, "loser fails closed; got ra={ra:?}, rb={rb:?}");
    assert_eq!(oks[0] as usize, MAX_SAMPLES_PER_KEY);

    let overflow_again = observer
        .record_invocation(
            &key,
            &ev("post-race-fresh"),
            at_millis(MAX_SAMPLES_PER_KEY as i64 + 1),
            window,
        )
        .await;
    assert!(
        matches!(
            overflow_again,
            Err(PredicateBackendError::WindowOverflow { .. })
        ),
        "after boundary race key is at cap; fresh id must fail closed, got {overflow_again:?}"
    );
}

async fn scenario_clock_skew_follows_caller_clock(
    host_a: Arc<dyn PredicateStateBackend>,
    host_b: Arc<dyn PredicateStateBackend>,
) {
    let key = inv_key("alpha", "cap.skew");
    let window = Duration::from_secs(60);

    let c1 = host_a
        .record_invocation(&key, &ev("a-0"), at_secs(0), window)
        .await
        .expect("ok");
    assert_eq!(c1, 1);

    let c2 = host_b
        .record_invocation(&key, &ev("b-skew"), at_secs(10_000), window)
        .await
        .expect("ok");
    assert_eq!(c2, 1, "skewed-ahead host trims the earlier entry");
}

async fn scenario_lru_eviction_race_holds_quota<C, Fut>(
    backend: Arc<dyn PredicateStateBackend>,
    count_scopes: C,
) where
    C: Fn(&'static str) -> Fut,
    Fut: std::future::Future<Output = usize>,
{
    let window = Duration::from_secs(3600);

    let beta = inv_key("beta", "beta.cap");
    backend
        .record_invocation(&beta, &ev("beta-evt"), at_secs(0), window)
        .await
        .expect("ok");

    let flood = MAX_KEYS_PER_TENANT + 16;
    let sem = Arc::new(tokio::sync::Semaphore::new(16));
    let mut handles = Vec::with_capacity(flood);
    for i in 0..flood {
        let backend = Arc::clone(&backend);
        let sem = Arc::clone(&sem);
        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire_owned().await.expect("permit");
            let key = inv_key("alpha", &format!("alpha.cap.{i}"));
            backend
                .record_invocation(
                    &key,
                    &ev(&format!("a-{i}")),
                    at_millis(i as i64 + 1),
                    window,
                )
                .await
                .expect("ok");
        }));
    }
    for h in handles {
        h.await.expect("joined");
    }

    let beta_count = backend
        .record_invocation(&beta, &ev("beta-evt"), at_secs(0), window)
        .await
        .expect("ok");
    assert_eq!(beta_count, 1, "quiet tenant scope must survive the flood");

    let alpha_scopes = count_scopes("alpha").await;
    assert!(
        alpha_scopes <= MAX_KEYS_PER_TENANT,
        "noisy tenant capped; got {alpha_scopes}"
    );
    assert!(
        backend.evictions_observed() >= 1,
        "eviction counter must advance"
    );
}
