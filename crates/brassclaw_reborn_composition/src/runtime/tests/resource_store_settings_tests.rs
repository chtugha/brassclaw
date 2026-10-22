//! Real acknowledged settings, actual runtime governor/gates and delayed PostgreSQL.
use super::*;
use brassclaw_product_workflow::{MontyMemoryMode, MontyVmSettingsStore};
use brassclaw_resources::ResourceGovernorStore;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_resource_store_deadline_is_live_captured_and_survives_owner_shutdown() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(MontyMemoryMode::Manual).await;
    let directory = tempfile::tempdir().unwrap();
    let runtime = Arc::new(
        Box::pin(build_reborn_runtime(RebornRuntimeInput::from_services(
            rig.build_input("resource-deadline-owner", directory.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )))
        .await
        .unwrap(),
    );
    let settings = runtime.monty_settings_owner.store();
    let policy = settings.resource_store_policy_source();
    let initial = settings.get("default", "default").await.unwrap();
    assert_eq!(policy.snapshot().unwrap().revision, initial.revision);
    assert_eq!(
        policy.snapshot().unwrap().timeout,
        Duration::from_millis(2000)
    );
    let mut invalid = initial.execution_limits;
    invalid.resource_store_timeout_millis = 0;
    let invalid = serde_json::from_value(
        serde_json::json!({"expected_revision":initial.revision,"execution_limits":invalid}),
    )
    .unwrap();
    assert!(matches!(
        settings.upsert("default", "default", &invalid).await,
        Err(brassclaw_product_workflow::MontyVmSettingsError::Invalid(_))
    ));
    assert_eq!(policy.snapshot().unwrap().revision, initial.revision);
    let identity = runtime.global_monty_owner.client().root_identity();
    let client = rig.pool.get().await.unwrap();
    client.batch_execute("CREATE FUNCTION reject_resource_deadline() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.execution_limits->'resource_store_timeout_millis' IS DISTINCT FROM OLD.execution_limits->'resource_store_timeout_millis' THEN RAISE EXCEPTION 'deliberate settings rejection'; END IF; RETURN NEW; END $$;
        CREATE TRIGGER reject_resource_deadline BEFORE UPDATE ON reborn_monty_vm_settings FOR EACH ROW EXECUTE FUNCTION reject_resource_deadline()").await.unwrap();
    let mut limits = initial.execution_limits;
    limits.resource_store_timeout_millis = 50;
    let rejected = serde_json::from_value(
        serde_json::json!({"expected_revision":initial.revision,"execution_limits":limits}),
    )
    .unwrap();
    assert!(
        settings
            .upsert("default", "default", &rejected)
            .await
            .is_err()
    );
    assert_eq!(policy.snapshot().unwrap().revision, initial.revision);
    assert_eq!(
        settings.get("default", "default").await.unwrap().revision,
        initial.revision
    );
    client.batch_execute("DROP TRIGGER reject_resource_deadline ON reborn_monty_vm_settings; DROP FUNCTION reject_resource_deadline();
        CREATE FUNCTION delay_resource_write() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_sleep(0.3); RETURN NEW; END $$;
        CREATE TRIGGER delay_resource_write BEFORE INSERT OR UPDATE ON brassclaw_resource_accounts FOR EACH ROW EXECUTE FUNCTION delay_resource_write()").await.unwrap();
    // The read selects the old policy. Publish the smaller acknowledged policy
    // inside the actual store mutation, before its real delayed CAS write.
    let store =
        brassclaw_resources::PgResourceGovernorStore::new(rig.pool.clone(), "deadline-capture")
            .with_settings_source(policy.clone());
    let edit_settings = settings.clone();
    store
        .update(move |_| {
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    edit_settings
                        .upsert("default", "default", &rejected)
                        .await
                        .unwrap();
                })
            });
            Ok(())
        })
        .unwrap();
    let changed = settings.get("default", "default").await.unwrap();
    assert_eq!(policy.snapshot().unwrap().revision, changed.revision);
    assert_eq!(
        policy.snapshot().unwrap().timeout,
        Duration::from_millis(50)
    );
    let observed = settings.runtime_observation(&changed).unwrap();
    assert_eq!(
        observed
            .execution_limits
            .unwrap()
            .limits
            .resource_store_timeout_millis,
        50
    );
    // This is the actual governor selected by the runtime, not the standalone
    // store above. A delayed mutation now fails at the smaller live deadline.
    let account = brassclaw_resources::ResourceAccount::user(
        runtime.thread_scope.tenant_id.clone(),
        runtime.actor_user_id.clone(),
    );
    let before_write = client.query_opt("SELECT version FROM brassclaw_resource_accounts WHERE tenant_id=$1 AND period_key='__governor__'", &[&runtime.webui_tenant_id()]).await.unwrap().map_or(0, |row| row.get::<_, i64>(0));
    let result = runtime
        .resource_governor
        .set_limit(account, brassclaw_resources::ResourceLimits::default());
    let Err(brassclaw_resources::ResourceError::Storage { reason }) = result else {
        panic!("actual runtime governor must use live deadline")
    };
    assert!(reason.contains("timed out"));
    // Readback waits for the real server operation. A client timeout did not
    // cancel/replay it or prove there was no durable write.
    client.batch_execute("DROP TRIGGER delay_resource_write ON brassclaw_resource_accounts; DROP FUNCTION delay_resource_write()").await.unwrap();
    let settled = client.query_one("SELECT version FROM brassclaw_resource_accounts WHERE tenant_id=$1 AND period_key='__governor__'", &[&runtime.webui_tenant_id()]).await.unwrap();
    assert_eq!(
        settled.get::<_, i64>(0),
        before_write + 1,
        "the timed-out mutation actually committed exactly once"
    );
    // Gate reads use the same actual source. Hold a genuine database lock.
    client
        .batch_execute("BEGIN; LOCK TABLE brassclaw_budget_gates IN ACCESS EXCLUSIVE MODE")
        .await
        .unwrap();
    let scope = brassclaw_host_api::ResourceScope {
        tenant_id: runtime.thread_scope.tenant_id.clone(),
        user_id: runtime.actor_user_id.clone(),
        agent_id: None,
        project_id: None,
        thread_id: None,
        invocation_id: brassclaw_host_api::InvocationId::new(),
    };
    let gate_result = runtime
        .budget_gate_store
        .get(&scope, brassclaw_resources::BudgetGateId::new());
    client.batch_execute("COMMIT").await.unwrap();
    let Err(brassclaw_resources::BudgetGateError::Storage { reason }) = gate_result else {
        panic!("actual gate store must use live deadline")
    };
    assert!(reason.contains("timed out"));
    assert_eq!(
        runtime.global_monty_owner.client().root_identity(),
        identity
    );
    let last = policy.snapshot().unwrap();
    drop(client);
    drop(settings);
    Arc::try_unwrap(runtime)
        .ok()
        .expect("single runtime owner")
        .shutdown()
        .await
        .unwrap();
    assert_eq!(
        policy.snapshot().unwrap(),
        last,
        "settlement retains last acknowledged policy without retaining the VM"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_resource_status_avoids_writes_but_commits_real_period_rollover() {
    use brassclaw_host_api::{ResourceEstimate, ResourceUsage};
    use brassclaw_resources::ResourceGovernor;
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let tenant = brassclaw_host_api::TenantId::new("read-only-ledger").unwrap();
    let governor = brassclaw_resources::PersistentResourceGovernor::new(
        brassclaw_resources::PgResourceGovernorStore::new(rig.pool.clone(), tenant.as_str()),
    );
    let account = brassclaw_resources::ResourceAccount::tenant(tenant.clone());
    governor
        .set_limit(
            account.clone(),
            brassclaw_resources::ResourceLimits {
                period: brassclaw_resources::BudgetPeriod::Rolling24h,
                ..Default::default()
            },
        )
        .unwrap();
    let scope = brassclaw_host_api::ResourceScope {
        tenant_id: tenant.clone(),
        user_id: brassclaw_host_api::UserId::new("status-owner").unwrap(),
        agent_id: None,
        project_id: None,
        thread_id: None,
        invocation_id: brassclaw_host_api::InvocationId::new(),
    };
    let output = b"observed resource bytes";
    let reservation = governor
        .reserve(
            scope.clone(),
            ResourceEstimate {
                output_bytes: Some(output.len() as u64),
                ..Default::default()
            },
        )
        .unwrap();
    let client = rig.pool.get().await.unwrap();
    let version = client
        .query_one(
            "SELECT version FROM brassclaw_resource_accounts WHERE tenant_id=$1",
            &[&tenant.as_str()],
        )
        .await
        .unwrap()
        .get::<_, i64>(0);
    client.batch_execute("CREATE FUNCTION reject_status_write() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'deliberate ledger write rejection'; END $$; CREATE TRIGGER reject_status_write BEFORE UPDATE ON brassclaw_resource_accounts FOR EACH ROW EXECUTE FUNCTION reject_status_write()").await.unwrap();
    let snapshot = governor.account_snapshot(&account).unwrap().unwrap();
    assert_eq!(snapshot.ledger.reserved.output_bytes, output.len() as u64);
    assert!(
        governor.release(reservation.id).is_err(),
        "actual mutations still fail closed while writes are rejected"
    );
    assert_eq!(
        client
            .query_one(
                "SELECT version FROM brassclaw_resource_accounts WHERE tenant_id=$1",
                &[&tenant.as_str()]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        version,
        "status did not write or increment the CAS version"
    );
    client
        .batch_execute("DROP TRIGGER reject_status_write ON brassclaw_resource_accounts")
        .await
        .unwrap();
    governor
        .reconcile(
            reservation.id,
            ResourceUsage {
                output_bytes: output.len() as u64,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        governor
            .account_snapshot(&account)
            .unwrap()
            .unwrap()
            .ledger
            .spent
            .output_bytes,
        output.len() as u64
    );
    let mut pending_scope = scope;
    pending_scope.invocation_id = brassclaw_host_api::InvocationId::new();
    let pending = governor
        .reserve(
            pending_scope,
            ResourceEstimate {
                output_bytes: Some(7),
                ..Default::default()
            },
        )
        .unwrap();
    let row = client
        .query_one(
            "SELECT payload,version FROM brassclaw_resource_accounts WHERE tenant_id=$1",
            &[&tenant.as_str()],
        )
        .await
        .unwrap();
    let mut payload: serde_json::Value = row.get(0);
    let anchors = payload["state"]["period_anchors"].as_array_mut().unwrap();
    assert_eq!(anchors.len(), 1, "one actual configured tenant period");
    anchors[0][1] = serde_json::to_value(chrono::Utc::now() - chrono::Duration::days(2)).unwrap();
    // This changes real persisted input, not the store or clock implementation.
    client.execute("UPDATE brassclaw_resource_accounts SET payload=$2,version=version+1 WHERE tenant_id=$1", &[&tenant.as_str(), &payload]).await.unwrap();
    let before_rollover = row.get::<_, i64>(1) + 1;
    client.batch_execute("CREATE TRIGGER reject_status_write BEFORE UPDATE ON brassclaw_resource_accounts FOR EACH ROW EXECUTE FUNCTION reject_status_write()").await.unwrap();
    assert!(
        governor.account_snapshot(&account).is_err(),
        "a real rollover may not be reported committed after a rejected write"
    );
    let unchanged = client
        .query_one(
            "SELECT payload,version FROM brassclaw_resource_accounts WHERE tenant_id=$1",
            &[&tenant.as_str()],
        )
        .await
        .unwrap();
    assert_eq!(unchanged.get::<_, serde_json::Value>(0), payload);
    assert_eq!(unchanged.get::<_, i64>(1), before_rollover);
    client.batch_execute("DROP TRIGGER reject_status_write ON brassclaw_resource_accounts; DROP FUNCTION reject_status_write()").await.unwrap();
    let rolled = governor.account_snapshot(&account).unwrap().unwrap();
    assert_eq!(rolled.ledger.spent.output_bytes, 0);
    assert_eq!(
        rolled.ledger.reserved.output_bytes, 7,
        "period rollover retains unresolved holds"
    );
    assert_eq!(
        client
            .query_one(
                "SELECT version FROM brassclaw_resource_accounts WHERE tenant_id=$1",
                &[&tenant.as_str()]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        before_rollover + 1
    );
    governor.account_snapshot(&account).unwrap();
    assert_eq!(
        client
            .query_one(
                "SELECT version FROM brassclaw_resource_accounts WHERE tenant_id=$1",
                &[&tenant.as_str()]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        before_rollover + 1,
        "the next unchanged read does not write"
    );
    governor.release(pending.id).unwrap();
    let empty = brassclaw_resources::PersistentResourceGovernor::new(
        brassclaw_resources::PgResourceGovernorStore::new(rig.pool.clone(), "empty-status-ledger"),
    );
    assert!(empty.account_snapshot(&account).unwrap().is_none());
    assert!(!client.query_one("SELECT EXISTS(SELECT 1 FROM brassclaw_resource_accounts WHERE tenant_id='empty-status-ledger')", &[]).await.unwrap().get::<_, bool>(0), "an empty status read must not create a ledger");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_resource_cap_edits_preserve_windows_and_advance_expired_ledgers() {
    use brassclaw_host_api::{ResourceEstimate, ResourceUsage};
    use brassclaw_resources::{BudgetPeriod, ResourceGovernor, ResourceLimits};
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let tenant = brassclaw_host_api::TenantId::new("cap-edit-ledger").unwrap();
    let governor = brassclaw_resources::PersistentResourceGovernor::new(
        brassclaw_resources::PgResourceGovernorStore::new(rig.pool.clone(), tenant.as_str()),
    );
    let account = brassclaw_resources::ResourceAccount::tenant(tenant.clone());
    let limits = |bytes| ResourceLimits {
        max_output_bytes: Some(bytes),
        period: BudgetPeriod::Rolling24h,
        ..Default::default()
    };
    governor.set_limit(account.clone(), limits(100)).unwrap();
    let mut scope = brassclaw_host_api::ResourceScope {
        tenant_id: tenant.clone(),
        user_id: brassclaw_host_api::UserId::new("cap-owner").unwrap(),
        agent_id: None,
        project_id: None,
        thread_id: None,
        invocation_id: brassclaw_host_api::InvocationId::new(),
    };
    let bytes = b"cap edit observation".len() as u64;
    let first = governor
        .reserve(
            scope.clone(),
            ResourceEstimate {
                output_bytes: Some(bytes),
                ..Default::default()
            },
        )
        .unwrap();
    governor
        .reconcile(
            first.id,
            ResourceUsage {
                output_bytes: bytes,
                ..Default::default()
            },
        )
        .unwrap();
    scope.invocation_id = brassclaw_host_api::InvocationId::new();
    let pending = governor
        .reserve(
            scope.clone(),
            ResourceEstimate {
                output_bytes: Some(7),
                ..Default::default()
            },
        )
        .unwrap();
    let client = rig.pool.get().await.unwrap();
    let before: serde_json::Value = client
        .query_one(
            "SELECT payload FROM brassclaw_resource_accounts WHERE tenant_id=$1",
            &[&tenant.as_str()],
        )
        .await
        .unwrap()
        .get(0);
    let anchor = before["state"]["period_anchors"].clone();
    assert_eq!(anchor.as_array().unwrap().len(), 1);
    governor.set_limit(account.clone(), limits(200)).unwrap();
    let after: serde_json::Value = client
        .query_one(
            "SELECT payload FROM brassclaw_resource_accounts WHERE tenant_id=$1",
            &[&tenant.as_str()],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        after["state"]["period_anchors"], anchor,
        "a cap-only edit must not restart a running budget window"
    );
    let active = governor.account_snapshot(&account).unwrap().unwrap();
    assert_eq!(active.limits.unwrap().max_output_bytes, Some(200));
    assert_eq!(active.ledger.spent.output_bytes, bytes);
    assert_eq!(active.ledger.reserved.output_bytes, 7);
    let mut expired = after;
    expired["state"]["period_anchors"][0][1] =
        serde_json::to_value(chrono::Utc::now() - chrono::Duration::days(2)).unwrap();
    client.execute("UPDATE brassclaw_resource_accounts SET payload=$2,version=version+1 WHERE tenant_id=$1", &[&tenant.as_str(), &expired]).await.unwrap();
    governor.set_limit(account.clone(), limits(300)).unwrap();
    let advanced = governor.account_snapshot(&account).unwrap().unwrap();
    assert_eq!(advanced.limits.unwrap().max_output_bytes, Some(300));
    assert_eq!(
        advanced.ledger.spent.output_bytes, 0,
        "cap editing must advance expired usage instead of extending it"
    );
    assert_eq!(advanced.ledger.reserved.output_bytes, 7);
    governor
        .reconcile(
            pending.id,
            ResourceUsage {
                output_bytes: 7,
                ..Default::default()
            },
        )
        .unwrap();
    scope.invocation_id = brassclaw_host_api::InvocationId::new();
    let last = governor
        .reserve(
            scope,
            ResourceEstimate {
                output_bytes: Some(3),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        governor
            .account_snapshot(&account)
            .unwrap()
            .unwrap()
            .ledger
            .spent
            .output_bytes,
        7
    );
    let calendar = BudgetPeriod::Calendar {
        tz: "UTC".parse().unwrap(),
        unit: brassclaw_resources::PeriodUnit::Day,
    };
    governor
        .set_limit(
            account.clone(),
            ResourceLimits {
                max_output_bytes: Some(400),
                period: calendar.clone(),
                ..Default::default()
            },
        )
        .unwrap();
    let changed = governor.account_snapshot(&account).unwrap().unwrap();
    assert_eq!(changed.limits.unwrap().period, calendar);
    assert_eq!(
        changed.ledger.spent.output_bytes, 0,
        "an explicit period change retains its reset semantics"
    );
    assert_eq!(changed.ledger.reserved.output_bytes, 3);
    governor.release(last.id).unwrap();
    let retained: serde_json::Value = client
        .query_one(
            "SELECT payload FROM brassclaw_resource_accounts WHERE tenant_id=$1",
            &[&tenant.as_str()],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        retained["state"]["reservations"].as_array().unwrap().len(),
        3,
        "period/cap changes retain original reservation receipts"
    );
}

mod aggregate_representation {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../brassclaw_resources/tests/support/aggregate_representation_contract.rs"
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_resource_aggregate_representation_is_atomic_and_retained() {
    use brassclaw_resources::{
        LiveMontyTaskSettings, MontyTaskLimits, MontyTaskSettingsRevision,
        PersistentResourceGovernor, PgResourceGovernorStore, ResourceGovernor,
    };
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let governor = PersistentResourceGovernor::new(PgResourceGovernorStore::new(
        rig.pool.clone(),
        "native-representation-ledger",
    ));
    governor
        .bind_task_budget_settings(
            LiveMontyTaskSettings::new(MontyTaskSettingsRevision {
                revision: 1,
                limits: MontyTaskLimits {
                    max_compute_time: Duration::from_secs(600),
                    token_budgets_enabled: false,
                },
            })
            .unwrap(),
        )
        .unwrap();
    aggregate_representation::aggregate_representation_contract(&governor);
}
