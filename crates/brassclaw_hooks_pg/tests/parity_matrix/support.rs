//! Parity-matrix shared support — Postgres + in-memory legs only.
//! (libSQL leg removed; `brassclaw_hooks_pg` is a Postgres-only crate.)

use std::sync::Arc;
use std::time::Duration;

use brassclaw_hooks::identity::{ExtensionId, HookId, HookLocalId, HookVersion};
use brassclaw_hooks::predicate_state::{
    InMemoryPredicateStateBackend, InvocationKey, PredicateBackendError, PredicateEventId,
    PredicateStateBackend, ValueKey,
};
use brassclaw_host_api::TenantId;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

// ---------------------------------------------------------------------------
// Observation log
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StepOutcome {
    Count(u32),
    Sum(String),
    WindowOverflow,
    OtherError(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Observation {
    pub(crate) label: String,
    pub(crate) outcome: StepOutcome,
    pub(crate) evictions_after: u64,
}

pub(crate) type ObservationLog = Vec<Observation>;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

pub(crate) fn hook_id() -> HookId {
    HookId::derive(
        &ExtensionId::new("ext").expect("ext id"),
        "1.0",
        &HookLocalId::new("h").expect("hook local id"),
        HookVersion::ONE,
    )
}

pub(crate) fn tenant(name: &str) -> TenantId {
    TenantId::new(name).expect("tenant id")
}

pub(crate) fn ev(s: &str) -> PredicateEventId {
    PredicateEventId::new(s).expect("event id")
}

pub(crate) fn base() -> DateTime<Utc> {
    DateTime::from_timestamp(1_700_000_000, 0).expect("fixed timestamp")
}

pub(crate) fn at_secs(secs: i64) -> DateTime<Utc> {
    base() + chrono::Duration::seconds(secs)
}

pub(crate) fn at_millis(ms: i64) -> DateTime<Utc> {
    base() + chrono::Duration::milliseconds(ms)
}

pub(crate) fn inv_key(tenant_name: &str, capability: &str) -> InvocationKey {
    InvocationKey {
        hook_id: hook_id(),
        tenant_id: tenant(tenant_name),
        capability: capability.to_string(),
    }
}

pub(crate) fn val_key(tenant_name: &str, capability: &str, field: &str) -> ValueKey {
    ValueKey {
        hook_id: hook_id(),
        tenant_id: tenant(tenant_name),
        capability: capability.to_string(),
        field: field.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Step drivers
// ---------------------------------------------------------------------------

pub(crate) async fn step_invocation(
    backend: &dyn PredicateStateBackend,
    log: &mut ObservationLog,
    label: &str,
    key: &InvocationKey,
    event_id: &PredicateEventId,
    now: DateTime<Utc>,
    window: Duration,
) {
    let outcome = match backend.record_invocation(key, event_id, now, window).await {
        Ok(c) => StepOutcome::Count(c),
        Err(PredicateBackendError::WindowOverflow { .. }) => StepOutcome::WindowOverflow,
        Err(other) => StepOutcome::OtherError(format!("{other:?}")),
    };
    log.push(Observation {
        label: label.to_string(),
        outcome,
        evictions_after: backend.evictions_observed(),
    });
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn step_value(
    backend: &dyn PredicateStateBackend,
    log: &mut ObservationLog,
    label: &str,
    key: &ValueKey,
    event_id: &PredicateEventId,
    now: DateTime<Utc>,
    value: Decimal,
    window: Duration,
) {
    let outcome = match backend
        .record_value(key, event_id, now, value, window)
        .await
    {
        Ok(s) => StepOutcome::Sum(s.normalize().to_string()),
        Err(PredicateBackendError::WindowOverflow { .. }) => StepOutcome::WindowOverflow,
        Err(other) => StepOutcome::OtherError(format!("{other:?}")),
    };
    log.push(Observation {
        label: label.to_string(),
        outcome,
        evictions_after: backend.evictions_observed(),
    });
}

// ---------------------------------------------------------------------------
// Backend factories
// ---------------------------------------------------------------------------

pub(crate) fn in_memory() -> Arc<dyn PredicateStateBackend> {
    Arc::new(InMemoryPredicateStateBackend::new())
}

/// Build a fresh Postgres backend over a private schema.
///
/// - `Ok(None)` — no DB URL configured; leg is skip-eligible.
/// - `Err(s)` — URL was set but setup failed; always fatal.
pub(crate) async fn postgres_backend() -> Result<Option<Arc<dyn PredicateStateBackend>>, String> {
    use brassclaw_hooks_pg::PostgresPredicateStateBackend;

    let Ok(url) =
        std::env::var("BRASSCLAW_HOOKS_POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL"))
    else {
        return Ok(None);
    };

    let schema = format!(
        "hooks_parity_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );

    {
        let (client, conn) = tokio_postgres::connect(&url, tokio_postgres::NoTls)
            .await
            .map_err(|e| format!("connect: {e}"))?;
        tokio::spawn(conn);
        client
            .batch_execute(&format!("CREATE SCHEMA IF NOT EXISTS {schema}"))
            .await
            .map_err(|e| format!("create schema: {e}"))?;
    }

    let config = url
        .parse::<tokio_postgres::Config>()
        .map_err(|e| format!("parse config: {e}"))?;
    let manager = deadpool_postgres::Manager::new(config, tokio_postgres::NoTls);
    let schema_for_hook = schema.clone();
    let pool = deadpool_postgres::Pool::builder(manager)
        .max_size(8)
        .post_create(deadpool_postgres::Hook::async_fn(move |client, _| {
            let schema = schema_for_hook.clone();
            Box::pin(async move {
                client
                    .batch_execute(&format!("SET search_path TO {schema}"))
                    .await
                    .map_err(|e| deadpool_postgres::HookError::message(e.to_string()))?;
                Ok(())
            })
        }))
        .build()
        .map_err(|e| format!("build pool: {e}"))?;
    let backend = PostgresPredicateStateBackend::new(pool.clone());
    backend
        .run_migrations()
        .await
        .map_err(|e| format!("run migrations: {e}"))?;
    let client = pool.get().await.map_err(|e| format!("pool get: {e}"))?;
    client
        .batch_execute("TRUNCATE TABLE hooks_predicate_invocations, hooks_predicate_values")
        .await
        .map_err(|e| format!("truncate: {e}"))?;
    Ok(Some(Arc::new(backend)))
}

/// When `BRASSCLAW_REQUIRE_POSTGRES=1`, a missing/unreachable Postgres is a
/// HARD failure (CI gate). Local runs without the env var skip cleanly.
pub(crate) fn require_postgres_or_skip(script_name: &str) {
    let required = std::env::var("BRASSCLAW_REQUIRE_POSTGRES")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if required {
        panic!(
            "[{script_name}] BRASSCLAW_REQUIRE_POSTGRES=1 but the Postgres parity leg \
             did not run (BRASSCLAW_HOOKS_POSTGRES_URL / DATABASE_URL unset/unreachable). \
             Refusing to skip-pass under the CI hard-gate."
        );
    }
    eprintln!(
        "[{script_name}] Postgres leg SKIPPED (no reachable DB URL). \
         Set BRASSCLAW_REQUIRE_POSTGRES=1 to make this a hard failure in CI."
    );
}

pub(crate) fn obs_count(label: &str, count: u32, evictions_after: u64) -> Observation {
    Observation {
        label: label.to_string(),
        outcome: StepOutcome::Count(count),
        evictions_after,
    }
}

pub(crate) fn obs_sum(label: &str, sum: &str, evictions_after: u64) -> Observation {
    Observation {
        label: label.to_string(),
        outcome: StepOutcome::Sum(sum.to_string()),
        evictions_after,
    }
}

pub(crate) fn obs_overflow(label: &str, evictions_after: u64) -> Observation {
    Observation {
        label: label.to_string(),
        outcome: StepOutcome::WindowOverflow,
        evictions_after,
    }
}

/// Run `script` against in-memory and Postgres, cross-assert both produce
/// the same log and match the independent hand-computed `expected` oracle.
///
/// Returns the set of leg names that actually ran.
pub(crate) async fn assert_parity<F, Fut>(
    script_name: &str,
    expected: ObservationLog,
    script: F,
) -> Vec<&'static str>
where
    F: Fn(Arc<dyn PredicateStateBackend>) -> Fut,
    Fut: std::future::Future<Output = ObservationLog>,
{
    let mut ran = Vec::new();

    // Reference leg: in-memory (always runs).
    let reference = script(in_memory()).await;
    assert_eq!(
        reference, expected,
        "[{script_name}] in-memory backend diverged from the independent \
         hand-computed oracle"
    );
    ran.push("in-memory");

    // Postgres leg (runs only with a DB URL).
    match postgres_backend().await {
        Ok(Some(pg)) => {
            let pg_log = script(pg).await;
            assert_eq!(
                pg_log, expected,
                "[{script_name}] Postgres diverged from the oracle — \
                 a real cross-backend behavioral bug, do NOT loosen this assertion"
            );
            ran.push("postgres");
        }
        Ok(None) => require_postgres_or_skip(script_name),
        Err(e) => panic!(
            "[{script_name}] Postgres parity leg setup FAILED after the DB URL was \
             found: {e}. A configured-but-unreachable/misconfigured DB must fail \
             loudly, not skip-pass."
        ),
    }

    eprintln!("[{script_name}] parity legs executed: {ran:?}");
    ran
}
