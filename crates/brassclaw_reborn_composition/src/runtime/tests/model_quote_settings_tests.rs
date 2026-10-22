//! Actual global settings publication and PostgreSQL reservation quotes.
//! No model is called and no estimated count is presented as provider usage.
use super::*;
use brassclaw_loop_support::{
    ModelCost, ModelQuoteSettingsSource, StaticModelCostTable, ZeroCostTable,
};
use brassclaw_product_workflow::{MontyMemoryMode, MontyVmSettingsStore};
use brassclaw_turns::run_profile::{LoopModelRequest, ModelWorkRequest};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_model_quote_settings_update_new_holds_and_preserve_existing_quotes() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(MontyMemoryMode::Manual).await;
    let directory = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(RebornRuntimeInput::from_services(
        rig.build_input("quote-owner", directory.path())
            .with_runtime_policy(local_dev_runtime_policy()),
    )))
    .await
    .unwrap();
    let settings = runtime.monty_settings_owner.store();
    let policy = settings.model_quote_policy_source();
    let slot = Arc::new(crate::model_quote_settings::AcknowledgedModelQuoteSettings::new());
    let accountant = crate::budget::build_default_budget_accountant_with_quote_settings(
        runtime.resource_governor.clone(),
        Arc::new(ZeroCostTable),
        runtime.budget_gate_store.clone(),
        Arc::new(brassclaw_resources::NoOpBudgetEventSink),
        &brassclaw_reborn_config::BudgetDefaults::compiled_defaults(),
        Some(slot.clone()),
    )
    .unwrap();
    let first =
        crate::native_admission_fixture::reserve(rig.pool.clone(), "reserve quote before edit")
            .await;
    let request = LoopModelRequest {
        work_id: Some(brassclaw_turns::run_profile::ModelWorkId::new()),
        dispatch_policy: Default::default(),
        messages: vec![],
        surface_version: None,
        model_preference: None,
        capability_view: None,
        resolved_messages: None,
    };
    let work = ModelWorkRequest::for_assistant(&first.context, &request);
    let account =
        brassclaw_resources::ResourceAccount::tenant(first.context.scope.tenant_id.clone());
    runtime
        .resource_governor
        .set_limit(
            account.clone(),
            brassclaw_resources::ResourceLimits::default(),
        )
        .unwrap();
    assert!(
        accountant
            .pre_model_work(&first.context, &work)
            .await
            .is_err(),
        "unbound host policy cannot admit work"
    );
    slot.bind(policy.clone()).unwrap();
    assert!(
        slot.bind(policy.clone()).is_err(),
        "the actual source binds once"
    );
    let before = policy.snapshot().unwrap();
    assert_eq!(before.output_tokens, 8192);
    accountant
        .pre_model_work(&first.context, &work)
        .await
        .unwrap();
    assert_eq!(
        runtime
            .resource_governor
            .account_snapshot(&account)
            .unwrap()
            .unwrap()
            .ledger
            .reserved
            .output_tokens,
        8192
    );
    let initial = settings.get("default", "default").await.unwrap();
    let mut limits = initial.execution_limits;
    limits.model_quote_output_tokens = 37;
    let update = serde_json::from_value(
        serde_json::json!({"expected_revision":initial.revision,"execution_limits":limits}),
    )
    .unwrap();
    let client = rig.pool.get().await.unwrap();
    client.batch_execute("CREATE FUNCTION reject_model_quote_edit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.execution_limits->'model_quote_output_tokens' IS DISTINCT FROM OLD.execution_limits->'model_quote_output_tokens' THEN RAISE EXCEPTION 'deliberate quote settings rejection'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_model_quote_edit BEFORE UPDATE ON reborn_monty_vm_settings FOR EACH ROW EXECUTE FUNCTION reject_model_quote_edit()").await.unwrap();
    assert!(
        settings
            .upsert("default", "default", &update)
            .await
            .is_err()
    );
    assert_eq!(policy.snapshot().unwrap(), before);
    client.batch_execute("DROP TRIGGER reject_model_quote_edit ON reborn_monty_vm_settings; DROP FUNCTION reject_model_quote_edit()").await.unwrap();
    let changed = settings
        .upsert("default", "default", &update)
        .await
        .unwrap();
    assert_eq!(slot.snapshot().unwrap().revision, changed.revision);
    assert_eq!(slot.snapshot().unwrap().output_tokens, 37);
    let mut invalid = changed.execution_limits;
    invalid.model_quote_output_tokens = 0;
    let invalid = serde_json::from_value(serde_json::json!({
        "expected_revision":changed.revision,"execution_limits":invalid
    }))
    .unwrap();
    assert!(matches!(
        settings.upsert("default", "default", &invalid).await,
        Err(brassclaw_product_workflow::MontyVmSettingsError::Invalid(_))
    ));
    assert_eq!(slot.snapshot().unwrap().revision, changed.revision);
    assert_eq!(
        settings
            .runtime_observation(&changed)
            .unwrap()
            .execution_limits
            .unwrap()
            .limits
            .model_quote_output_tokens,
        37
    );
    assert_eq!(
        runtime
            .resource_governor
            .account_snapshot(&account)
            .unwrap()
            .unwrap()
            .ledger
            .reserved
            .output_tokens,
        8192,
        "live edit does not resize an earlier hold"
    );
    let second =
        crate::native_admission_fixture::reserve(rig.pool.clone(), "reserve quote after edit")
            .await;
    let second_work = ModelWorkRequest::for_assistant(&second.context, &request);
    accountant
        .pre_model_work(&second.context, &second_work)
        .await
        .unwrap();
    let nested = accountant.isolated_work_accountant().unwrap();
    let third =
        crate::native_admission_fixture::reserve(rig.pool.clone(), "isolated nested quote").await;
    let third_work = ModelWorkRequest::for_assistant(&third.context, &request);
    nested
        .pre_model_work(&third.context, &third_work)
        .await
        .unwrap();
    assert_eq!(
        runtime
            .resource_governor
            .account_snapshot(&account)
            .unwrap()
            .unwrap()
            .ledger
            .reserved
            .output_tokens,
        8192 + 37 + 37
    );
    // Known row and explicit request estimates take precedence over the
    // operator's unknown-model fallback. These are quotes, not model usage.
    let fourth =
        crate::native_admission_fixture::reserve(rig.pool.clone(), "known quote maximum").await;
    let mut fourth_work = ModelWorkRequest::for_assistant(&fourth.context, &request);
    let known = StaticModelCostTable::new().with_entry(
        fourth_work.model_profile_id.clone(),
        ModelCost {
            input_per_token: rust_decimal::Decimal::ZERO,
            output_per_token: rust_decimal::Decimal::ZERO,
            max_output_tokens: 777,
            cache_write_multiplier_milli: 0,
            cache_read_multiplier_milli: 0,
        },
    );
    let known = brassclaw_loop_support::GovernorBackedAccountant::new(
        runtime.resource_governor.clone(),
        Arc::new(known),
    )
    .with_quote_settings(slot.clone());
    use brassclaw_turns::run_profile::LoopModelBudgetAccountant;
    known
        .pre_model_work(&fourth.context, &fourth_work)
        .await
        .unwrap();
    assert_eq!(
        runtime
            .resource_governor
            .account_snapshot(&account)
            .unwrap()
            .unwrap()
            .ledger
            .reserved
            .output_tokens,
        8192 + 37 + 37 + 777
    );
    known.release_in_flight(&fourth.context, &fourth_work);
    fourth_work.work_id = Some(brassclaw_turns::run_profile::ModelWorkId::new());
    fourth_work.estimated_output_tokens = Some(9);
    known
        .pre_model_work(&fourth.context, &fourth_work)
        .await
        .unwrap();
    assert_eq!(
        runtime
            .resource_governor
            .account_snapshot(&account)
            .unwrap()
            .unwrap()
            .ledger
            .reserved
            .output_tokens,
        8192 + 37 + 37 + 9
    );
    accountant.release_in_flight(&first.context, &work);
    accountant.release_in_flight(&second.context, &second_work);
    nested.release_in_flight(&third.context, &third_work);
    known.release_in_flight(&fourth.context, &fourth_work);
    assert!(
        accountant
            .pre_model_work(&first.context, &work)
            .await
            .is_err(),
        "a closed persistent reservation cannot admit the same logical work again"
    );
    let mut successor = work.clone();
    successor.work_id = Some(brassclaw_turns::run_profile::ModelWorkId::new());
    accountant
        .pre_model_work(&first.context, &successor)
        .await
        .unwrap();
    accountant.release_in_flight(&first.context, &work);
    assert!(
        accountant
            .model_work_dispatching(&first.context, &work)
            .is_err()
    );
    assert!(
        accountant
            .post_model_work(
                &first.context,
                &work,
                brassclaw_turns::run_profile::ModelWorkOutcome::Failure(
                    brassclaw_turns::run_profile::AgentLoopHostErrorKind::Unavailable
                )
            )
            .await
            .is_err()
    );
    let held = runtime
        .resource_governor
        .account_snapshot(&account)
        .unwrap()
        .unwrap()
        .ledger;
    assert_eq!(held.reserved.output_tokens, 37);
    accountant.release_in_flight(&first.context, &successor);
    let ledger = runtime
        .resource_governor
        .account_snapshot(&account)
        .unwrap()
        .unwrap()
        .ledger;
    assert_eq!(ledger.reserved.output_tokens, 0);
    assert_eq!(
        ledger.spent.output_tokens, 0,
        "unsubmitted quotes are not observed spend"
    );
    drop(client);
    drop(settings);
    runtime.shutdown().await.unwrap();
    assert_eq!(slot.snapshot().unwrap().output_tokens, 37);
}
