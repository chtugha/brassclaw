//! Real PostgreSQL acceptance for the resource-store caller and upgrade.

use std::sync::Arc;

use brassclaw_host_api::{ResourceScope, TenantId, UserId};
use brassclaw_resources::{
    BudgetApprovalGate, BudgetGateError, BudgetGateId, BudgetGateOutcome, BudgetGateStatus,
    BudgetGateStore, ResourceAccount, ResourceApprovalNeeded, ResourceDimension, ResourceLimits,
    ResourceValue, pg_store::PgBudgetGateStore,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

const EXACT_AMOUNT: Decimal = dec!(0.1234567890123456789012345678);
const UPGRADE: &str =
    include_str!("../../brassclaw_pg/migrations/V110__exact_budget_gate_amounts.sql");

fn gate(ordinal: u128) -> BudgetApprovalGate {
    let expires_at = DateTime::from_timestamp(1_800_000_000, 123_456_789).unwrap();
    BudgetApprovalGate {
        id: BudgetGateId::from_uuid(uuid::Uuid::from_u128(ordinal)),
        needed: ResourceApprovalNeeded {
            account: ResourceAccount::tenant(TenantId::new("gate-integrity").unwrap()),
            dimension: ResourceDimension::Usd,
            limit: ResourceValue::Decimal(dec!(1)),
            current_usage: ResourceValue::Decimal(dec!(0)),
            active_reserved: ResourceValue::Decimal(dec!(0)),
            requested: ResourceValue::Decimal(EXACT_AMOUNT),
            utilization: 0.123,
            period_end: None,
        },
        opened_at: expires_at - chrono::Duration::hours(24),
        expires_at,
        status: BudgetGateStatus::Pending,
    }
}

fn cancel() -> BudgetGateOutcome {
    BudgetGateOutcome::Cancel {
        by: UserId::new("operator").unwrap(),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_budget_gate_upgrade_restores_only_proven_rounding() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let mut client = rig.pool.get().await.unwrap();
    client
        .batch_execute(
            "ALTER TABLE brassclaw_budget_gates ALTER COLUMN requested_amount TYPE NUMERIC(18,6)",
        )
        .await
        .unwrap();
    let original = gate(1);
    let payload = serde_json::to_value(&original).unwrap();
    client.execute(
        "INSERT INTO brassclaw_budget_gates (id, tenant_id, gate_kind, requested_amount, payload, expires_at) \
         VALUES ($1, 'gate-integrity', 'usd', $2::text::numeric, $3, $4)",
        &[&original.id.to_string(), &EXACT_AMOUNT.to_string(), &payload, &original.expires_at],
    ).await.unwrap();
    let rounded: String = client
        .query_one(
            "SELECT requested_amount::text FROM brassclaw_budget_gates",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(rounded, "0.123457");
    let mut unchanged = gate(2);
    unchanged.needed.requested = ResourceValue::Decimal(dec!(9));
    let unchanged_payload = serde_json::to_value(&unchanged).unwrap();
    let untouched_at: DateTime<Utc> = client.query_one(
        "INSERT INTO brassclaw_budget_gates (id, tenant_id, gate_kind, requested_amount, payload, expires_at) \
         VALUES ($1, 'gate-integrity', 'usd', 9, $2, $3) RETURNING updated_at",
        &[&unchanged.id.to_string(), &unchanged_payload, &unchanged.expires_at],
    ).await.unwrap().get(0);
    let transaction = client.transaction().await.unwrap();
    transaction.batch_execute(UPGRADE).await.unwrap();
    transaction.commit().await.unwrap();
    let store = PgBudgetGateStore::new(Arc::clone(&rig.pool), "gate-integrity");
    assert_eq!(
        store.get(&ResourceScope::system(), original.id).unwrap(),
        Some(original.clone())
    );
    let still_untouched: DateTime<Utc> = client
        .query_one(
            "SELECT updated_at FROM brassclaw_budget_gates WHERE id=$1",
            &[&unchanged.id.to_string()],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(still_untouched, untouched_at);
    client
        .execute(
            "DELETE FROM brassclaw_budget_gates WHERE id=$1",
            &[&unchanged.id.to_string()],
        )
        .await
        .unwrap();

    // The migration must not reinterpret malformed declarations or unrelated
    // amount corruption as the known V019 rounding defect.
    client
        .batch_execute(
            "ALTER TABLE brassclaw_budget_gates ALTER COLUMN requested_amount TYPE NUMERIC(18,6)",
        )
        .await
        .unwrap();
    for mutation in [
        "UPDATE brassclaw_budget_gates SET payload = payload #- '{needed,requested,kind}'",
        "UPDATE brassclaw_budget_gates SET payload = jsonb_set(payload, '{needed,requested,value}', '\"bad\"')",
        "UPDATE brassclaw_budget_gates SET requested_amount = 1",
    ] {
        client
            .execute(
                "UPDATE brassclaw_budget_gates SET payload=$1, requested_amount=0.123457",
                &[&payload],
            )
            .await
            .unwrap();
        client.batch_execute(mutation).await.unwrap();
        let transaction = client.transaction().await.unwrap();
        assert!(transaction.batch_execute(UPGRADE).await.is_err());
        transaction.rollback().await.unwrap();
        let kind: String = client
            .query_one(
                "SELECT format_type(atttypid, atttypmod) FROM pg_attribute \
             WHERE attrelid='brassclaw_budget_gates'::regclass AND attname='requested_amount'",
                &[],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(kind, "numeric(18,6)");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_budget_gate_store_rejects_inconsistent_envelopes_without_mutation() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let store = PgBudgetGateStore::new(Arc::clone(&rig.pool), "gate-integrity");
    let scope = ResourceScope::system();
    let original = gate(1);
    store.open(&scope, original.clone()).unwrap();
    store.open(&scope, original.clone()).unwrap();
    assert_eq!(
        store.get(&scope, original.id).unwrap(),
        Some(original.clone())
    );

    let mut integer = gate(2);
    integer.needed.dimension = ResourceDimension::InputTokens;
    integer.needed.requested = ResourceValue::Integer(u64::MAX);
    store.open(&scope, integer.clone()).unwrap();
    assert_eq!(store.get(&scope, integer.id).unwrap(), Some(integer));
    let mut before_epoch = gate(4);
    before_epoch.expires_at = DateTime::from_timestamp(946_684_799, 123_456_789).unwrap();
    before_epoch.opened_at = before_epoch.expires_at - chrono::Duration::hours(24);
    store.open(&scope, before_epoch.clone()).unwrap();
    assert_eq!(
        store.get(&scope, before_epoch.id).unwrap(),
        Some(before_epoch)
    );
    let client = rig.pool.get().await.unwrap();
    let exact: String = client
        .query_one(
            "SELECT requested_amount::text FROM brassclaw_budget_gates WHERE id=$1",
            &[&original.id.to_string()],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(Decimal::from_str_exact(&exact).unwrap(), EXACT_AMOUNT);

    let original_payload = serde_json::to_value(&original).unwrap();
    for mutation in [
        "UPDATE brassclaw_budget_gates SET requested_amount=1 WHERE id=$1",
        "UPDATE brassclaw_budget_gates SET status='approved' WHERE id=$1",
        "UPDATE brassclaw_budget_gates SET gate_kind='input_tokens' WHERE id=$1",
        "UPDATE brassclaw_budget_gates SET expires_at=expires_at+interval '1 second' WHERE id=$1",
        "UPDATE brassclaw_budget_gates SET expires_at=NULL WHERE id=$1",
        "UPDATE brassclaw_budget_gates SET expires_at='infinity' WHERE id=$1",
        "UPDATE brassclaw_budget_gates SET payload=jsonb_set(payload, '{id}', '\"00000000-0000-0000-0000-000000000003\"') WHERE id=$1",
    ] {
        client
            .execute(
                "UPDATE brassclaw_budget_gates SET status='pending', gate_kind='usd', \
             expires_at=$2, requested_amount=$3::text::numeric, payload=$4 WHERE id=$1",
                &[
                    &original.id.to_string(),
                    &original.expires_at,
                    &EXACT_AMOUNT.to_string(),
                    &original_payload,
                ],
            )
            .await
            .unwrap();
        client
            .execute(mutation, &[&original.id.to_string()])
            .await
            .unwrap();
        let before = client
            .query_one(
                "SELECT to_jsonb(g) FROM brassclaw_budget_gates g WHERE id=$1",
                &[&original.id.to_string()],
            )
            .await
            .unwrap()
            .get::<_, serde_json::Value>(0);
        assert!(matches!(
            store.get(&scope, original.id),
            Err(BudgetGateError::Storage { .. })
        ));
        assert!(matches!(
            store.open(&scope, original.clone()),
            Err(BudgetGateError::Storage { .. })
        ));
        assert!(matches!(
            store.resolve(&scope, original.id, cancel(), Utc::now()),
            Err(BudgetGateError::Storage { .. })
        ));
        // A terminal indexed status is outside these two pending selectors.
        if !mutation.contains("status='approved'") {
            assert!(matches!(
                store.list_pending(&scope),
                Err(BudgetGateError::Storage { .. })
            ));
            // NULL/infinite deadlines are outside the SQL expiry selector.
            if !mutation.contains("expires_at=NULL") && !mutation.contains("'infinity'") {
                assert!(matches!(
                    store.expire_pending_older_than(
                        &scope,
                        original.expires_at + chrono::Duration::days(1)
                    ),
                    Err(BudgetGateError::Storage { .. })
                ));
            }
        }
        let after = client
            .query_one(
                "SELECT to_jsonb(g) FROM brassclaw_budget_gates g WHERE id=$1",
                &[&original.id.to_string()],
            )
            .await
            .unwrap()
            .get::<_, serde_json::Value>(0);
        assert_eq!(before, after);
    }
    let mut terminal = gate(3);
    terminal.status = BudgetGateStatus::Expired { at: Utc::now() };
    assert!(matches!(
        store.open(&scope, terminal.clone()),
        Err(BudgetGateError::Storage { .. })
    ));
    assert!(store.get(&scope, terminal.id).unwrap().is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_budget_gate_expiry_and_resolution_keep_one_durable_transition() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let store = Arc::new(PgBudgetGateStore::new(
        Arc::clone(&rig.pool),
        "gate-integrity",
    ));
    let scope = ResourceScope::system();
    let first = gate(1);
    let second = gate(2);
    store.open(&scope, first.clone()).unwrap();
    store.open(&scope, second.clone()).unwrap();
    // Both SQL timestamps fall in the cutoff's microsecond, but their full
    // deadlines are later. This is ordinary precision, not corruption.
    assert!(
        store
            .expire_pending_older_than(&scope, first.expires_at - chrono::Duration::nanoseconds(1))
            .unwrap()
            .is_empty()
    );
    let client = rig.pool.get().await.unwrap();
    client
        .execute(
            "UPDATE brassclaw_budget_gates SET requested_amount=1 WHERE id=$1",
            &[&second.id.to_string()],
        )
        .await
        .unwrap();
    assert!(matches!(
        store.expire_pending_older_than(&scope, first.expires_at),
        Err(BudgetGateError::Storage { .. })
    ));
    assert_eq!(
        store.get(&scope, first.id).unwrap(),
        Some(first.clone()),
        "the earlier row update must roll back too"
    );
    client
        .execute(
            "UPDATE brassclaw_budget_gates SET requested_amount=$2::text::numeric WHERE id=$1",
            &[&second.id.to_string(), &EXACT_AMOUNT.to_string()],
        )
        .await
        .unwrap();

    let barrier = Arc::new(tokio::sync::Barrier::new(2));
    let id = first.id;
    let mut contenders = Vec::new();
    for outcome in [
        cancel(),
        BudgetGateOutcome::Approve {
            increased_limit: ResourceLimits::default(),
            by: UserId::new("operator").unwrap(),
        },
    ] {
        let store = Arc::clone(&store);
        let barrier = Arc::clone(&barrier);
        contenders.push(tokio::spawn(async move {
            barrier.wait().await;
            store.resolve(&ResourceScope::system(), id, outcome, Utc::now())
        }));
    }
    let mut winner = None;
    let mut losses = 0;
    for contender in contenders {
        match contender.await.unwrap() {
            Ok(resolved) => {
                assert!(winner.replace(resolved).is_none());
            }
            Err(BudgetGateError::AlreadyResolved { id }) => {
                assert_eq!(id, first.id);
                losses += 1;
            }
            other => panic!("unexpected competing resolution: {other:?}"),
        }
    }
    assert_eq!(losses, 1);
    assert_eq!(store.get(&scope, first.id).unwrap(), winner);
    let expired = store
        .expire_pending_older_than(&scope, second.expires_at)
        .unwrap();
    assert_eq!(expired.len(), 1);
    assert_eq!(expired[0].id, second.id);
    assert_eq!(
        store.get(&scope, second.id).unwrap(),
        Some(expired[0].clone())
    );
}
