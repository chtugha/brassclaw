#![cfg(feature = "postgres")]

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use brassclaw_filesystem::PostgresRootFilesystem;
use brassclaw_host_api::VirtualPath;
use brassclaw_product_adapters::{
    AdapterInstallationId, ExternalActorRef, ExternalEventId, ProductAdapterId, ProductInboundAck,
};
use brassclaw_product_workflow::{
    ActionFingerprintKey, IdempotencyDecision, IdempotencyLedger, ProductInboundAction,
    ProductWorkflowError, SourceBindingKey,
};
use brassclaw_product_workflow_storage::RebornPostgresIdempotencyLedger;
use chrono::{Duration, Utc};

fn fingerprint(suffix: &str) -> ActionFingerprintKey {
    fingerprint_for_actor(suffix, "user1")
}

fn fingerprint_for_actor(suffix: &str, actor_id: &str) -> ActionFingerprintKey {
    ActionFingerprintKey::new(
        ProductAdapterId::new("test_adapter").expect("valid adapter"),
        AdapterInstallationId::new("install_alpha").expect("valid installation"),
        ExternalActorRef::new("test", actor_id, Option::<String>::None).expect("valid actor"),
        SourceBindingKey::new("space:0:;conversation:5:conv1;topic:0:;")
            .expect("valid source binding key"),
        ExternalEventId::new(format!("evt:{suffix}")).expect("valid event"),
    )
}

fn custom_root(suffix: &str) -> VirtualPath {
    VirtualPath::new(format!(
        "/engine/product_workflow/idempotency/test_roots/{suffix}"
    ))
    .expect("valid custom ledger root")
}

fn unique_suffix(name: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after unix epoch")
        .as_nanos();
    format!("{name}-{nanos}")
}

async fn assert_settled_action_survives_reopen_and_replays(
    ledger: &dyn IdempotencyLedger,
    reopened: &dyn IdempotencyLedger,
    suffix: &str,
) {
    let received_at = Utc::now();
    let fingerprint = fingerprint(suffix);

    let decision = ledger
        .begin_or_replay(fingerprint.clone(), received_at)
        .await
        .expect("begin");
    let IdempotencyDecision::New(mut action) = decision else {
        panic!("expected new action");
    };
    action.settle(ProductInboundAck::NoOp);
    ledger.settle(action).await.expect("settle");

    let replay = reopened
        .begin_or_replay(fingerprint, received_at + Duration::seconds(1))
        .await
        .expect("replay");

    let IdempotencyDecision::Replay(action) = replay else {
        panic!("expected replay");
    };
    assert_eq!(action.outcome, Some(ProductInboundAck::NoOp));
}

async fn assert_in_flight_action_blocks_until_lease_expires(
    ledger: &dyn IdempotencyLedger,
    suffix: &str,
) {
    let received_at = Utc::now();
    let fingerprint = fingerprint(suffix);

    assert!(matches!(
        ledger
            .begin_or_replay(fingerprint.clone(), received_at)
            .await
            .expect("begin"),
        IdempotencyDecision::New(_)
    ));
    let blocked = ledger
        .begin_or_replay(fingerprint.clone(), received_at + Duration::seconds(5))
        .await
        .expect_err("fresh reservation should block");
    assert!(matches!(blocked, ProductWorkflowError::Transient { .. }));

    let reclaimed = ledger
        .begin_or_replay(fingerprint, received_at + Duration::seconds(11))
        .await
        .expect("expired reservation should be reclaimed");
    assert!(matches!(reclaimed, IdempotencyDecision::New(_)));
}

async fn assert_release_allows_retry_without_waiting_for_lease(
    ledger: &dyn IdempotencyLedger,
    suffix: &str,
) {
    let received_at = Utc::now();
    let fingerprint = fingerprint(suffix);

    let decision = ledger
        .begin_or_replay(fingerprint.clone(), received_at)
        .await
        .expect("begin");
    let IdempotencyDecision::New(action) = decision else {
        panic!("expected new action");
    };
    ledger.release(action).await.expect("release");

    let retry = ledger
        .begin_or_replay(fingerprint, received_at + Duration::seconds(1))
        .await
        .expect("retry after release");
    assert!(matches!(retry, IdempotencyDecision::New(_)));
}

async fn assert_duplicate_reservation_contention_serializes(
    first: &dyn IdempotencyLedger,
    second: &dyn IdempotencyLedger,
    suffix: &str,
) {
    let received_at = Utc::now();
    let fingerprint = fingerprint(suffix);

    let (left, right) = tokio::join!(
        first.begin_or_replay(fingerprint.clone(), received_at),
        second.begin_or_replay(fingerprint, received_at),
    );
    let results = [left, right];
    let new_count = results
        .iter()
        .filter(|result| matches!(result, Ok(IdempotencyDecision::New(_))))
        .count();
    let blocked_count = results
        .iter()
        .filter(|result| matches!(result, Err(ProductWorkflowError::Transient { .. })))
        .count();

    assert_eq!(new_count, 1);
    assert_eq!(blocked_count, 1);
}

async fn assert_superseded_reservation_cannot_settle(ledger: &dyn IdempotencyLedger, suffix: &str) {
    let received_at = Utc::now();
    let fingerprint = fingerprint(suffix);

    let IdempotencyDecision::New(mut stale_action) = ledger
        .begin_or_replay(fingerprint.clone(), received_at)
        .await
        .expect("begin")
    else {
        panic!("expected new action");
    };

    let IdempotencyDecision::New(mut replacement) = ledger
        .begin_or_replay(fingerprint, received_at + Duration::seconds(11))
        .await
        .expect("expired reservation should be reclaimed")
    else {
        panic!("expected reclaimed action");
    };

    stale_action.settle(ProductInboundAck::NoOp);
    let stale_error = ledger
        .settle(stale_action)
        .await
        .expect_err("superseded action must not settle");
    assert!(matches!(
        stale_error,
        ProductWorkflowError::Transient { .. }
    ));

    replacement.settle(ProductInboundAck::NoOp);
    ledger
        .settle(replacement)
        .await
        .expect("replacement settle");
}

async fn assert_settle_missing_reservation_returns_transient(
    ledger: &dyn IdempotencyLedger,
    suffix: &str,
) {
    let received_at = Utc::now();
    let mut action = ProductInboundAction::begin(fingerprint(suffix), received_at);
    action.settle(ProductInboundAck::NoOp);

    let error = ledger
        .settle(action)
        .await
        .expect_err("missing reservation must not settle");
    assert!(matches!(error, ProductWorkflowError::Transient { .. }));
}

async fn assert_custom_root_isolated_from_default_root(
    custom: &dyn IdempotencyLedger,
    default: &dyn IdempotencyLedger,
    suffix: &str,
) {
    let received_at = Utc::now();
    let fingerprint = fingerprint(suffix);
    let IdempotencyDecision::New(mut action) = custom
        .begin_or_replay(fingerprint.clone(), received_at)
        .await
        .expect("begin in custom root")
    else {
        panic!("expected new custom-root action");
    };
    action.settle(ProductInboundAck::NoOp);
    custom.settle(action).await.expect("settle custom root");

    let default_decision = default
        .begin_or_replay(fingerprint, received_at + Duration::seconds(1))
        .await
        .expect("begin in default root");
    assert!(matches!(default_decision, IdempotencyDecision::New(_)));
}

async fn assert_actor_identity_is_part_of_fingerprint_path(
    ledger: &dyn IdempotencyLedger,
    suffix: &str,
) {
    let received_at = Utc::now();
    let first_actor = fingerprint_for_actor(suffix, "user1");
    let second_actor = fingerprint_for_actor(suffix, "user2");

    assert!(matches!(
        ledger
            .begin_or_replay(first_actor, received_at)
            .await
            .expect("begin first actor"),
        IdempotencyDecision::New(_)
    ));
    assert!(matches!(
        ledger
            .begin_or_replay(second_actor, received_at)
            .await
            .expect("begin second actor"),
        IdempotencyDecision::New(_)
    ));
}

#[tokio::test]
async fn postgres_settled_action_survives_reopen_and_replays_when_configured() {
    let Some(filesystem) = postgres_filesystem().await else {
        return;
    };
    let ledger = RebornPostgresIdempotencyLedger::new(Arc::clone(&filesystem));
    let reopened = RebornPostgresIdempotencyLedger::new(filesystem);

    assert_settled_action_survives_reopen_and_replays(
        &ledger,
        &reopened,
        &unique_suffix("postgres-settled-replay"),
    )
    .await;
}

#[tokio::test]
async fn postgres_in_flight_action_blocks_until_lease_expires_when_configured() {
    let Some(filesystem) = postgres_filesystem().await else {
        return;
    };
    let ledger =
        RebornPostgresIdempotencyLedger::with_in_flight_lease(filesystem, Duration::seconds(10));

    assert_in_flight_action_blocks_until_lease_expires(&ledger, &unique_suffix("postgres-lease"))
        .await;
}

#[tokio::test]
async fn postgres_release_allows_retry_without_waiting_for_lease_when_configured() {
    let Some(filesystem) = postgres_filesystem().await else {
        return;
    };
    let ledger =
        RebornPostgresIdempotencyLedger::with_in_flight_lease(filesystem, Duration::seconds(60));

    assert_release_allows_retry_without_waiting_for_lease(
        &ledger,
        &unique_suffix("postgres-release"),
    )
    .await;
}

#[tokio::test]
async fn postgres_duplicate_reservation_contention_serializes_when_configured() {
    let Some(filesystem) = postgres_filesystem().await else {
        return;
    };
    let first = RebornPostgresIdempotencyLedger::with_in_flight_lease(
        Arc::clone(&filesystem),
        Duration::seconds(10),
    );
    let second =
        RebornPostgresIdempotencyLedger::with_in_flight_lease(filesystem, Duration::seconds(10));

    assert_duplicate_reservation_contention_serializes(
        &first,
        &second,
        &unique_suffix("postgres-contention"),
    )
    .await;
}

#[tokio::test]
async fn postgres_superseded_reservation_cannot_settle_when_configured() {
    let Some(filesystem) = postgres_filesystem().await else {
        return;
    };
    let ledger =
        RebornPostgresIdempotencyLedger::with_in_flight_lease(filesystem, Duration::seconds(10));

    assert_superseded_reservation_cannot_settle(&ledger, &unique_suffix("postgres-superseded"))
        .await;
}

#[tokio::test]
async fn postgres_settle_missing_reservation_returns_transient_when_configured() {
    let Some(filesystem) = postgres_filesystem().await else {
        return;
    };
    let ledger = RebornPostgresIdempotencyLedger::new(filesystem);

    assert_settle_missing_reservation_returns_transient(
        &ledger,
        &unique_suffix("postgres-missing-settle"),
    )
    .await;
}

#[tokio::test]
async fn postgres_custom_root_isolated_from_default_root_when_configured() {
    let Some(filesystem) = postgres_filesystem().await else {
        return;
    };
    let custom = RebornPostgresIdempotencyLedger::with_root(
        Arc::clone(&filesystem),
        custom_root("postgres"),
        Duration::seconds(60),
    );
    let default = RebornPostgresIdempotencyLedger::new(filesystem);

    assert_custom_root_isolated_from_default_root(
        &custom,
        &default,
        &unique_suffix("postgres-custom-root"),
    )
    .await;
}

#[tokio::test]
async fn postgres_actor_identity_is_part_of_fingerprint_path_when_configured() {
    let Some(filesystem) = postgres_filesystem().await else {
        return;
    };
    let ledger = RebornPostgresIdempotencyLedger::new(filesystem);

    assert_actor_identity_is_part_of_fingerprint_path(
        &ledger,
        &unique_suffix("postgres-actor-isolation"),
    )
    .await;
}

async fn postgres_filesystem() -> Option<Arc<PostgresRootFilesystem>> {
    let url = match std::env::var("BRASSCLAW_PRODUCT_WORKFLOW_POSTGRES_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!(
                "skipping postgres product workflow ledger contract: BRASSCLAW_PRODUCT_WORKFLOW_POSTGRES_URL not set"
            );
            return None;
        }
    };
    let config = match url.parse::<tokio_postgres::Config>() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("skipping postgres product workflow ledger contract: invalid url ({error})");
            return None;
        }
    };
    let manager = deadpool_postgres::Manager::new(config, tokio_postgres::NoTls);
    let pool = deadpool_postgres::Pool::builder(manager)
        .max_size(4)
        .build()
        .expect("postgres pool builds");
    if let Err(error) = pool.get().await {
        eprintln!(
            "skipping postgres product workflow ledger contract: database unavailable ({error})"
        );
        return None;
    }
    let filesystem = Arc::new(PostgresRootFilesystem::new(pool));
    if let Err(error) = filesystem.run_migrations().await {
        eprintln!(
            "skipping postgres product workflow ledger contract: filesystem migrations failed ({error})"
        );
        return None;
    }
    Some(filesystem)
}
