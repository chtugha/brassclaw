//! Real connected WebUI facade, PostgreSQL, encrypted credentials and global
//! Recipe execution. No real provider dispatch is needed for technical setup.
use super::*;
use brassclaw_pg::test_support::{AckBoundary, AckCutRule, AckCutTransport};
use brassclaw_product_workflow::{PrefixJobTarget, ProviderMutationReceipt};
use serde_json::{Value, json};
#[path = "provider_bootstrap_limit_tests.rs"]
mod limits;

fn target(receipt: &ProviderMutationReceipt) -> (&str, u64, Uuid) {
    let Some(PrefixJobTarget::ModelSetup {
        specification,
        setup_revision,
    }) = &receipt.bootstrap_target
    else {
        panic!("missing original model setup target");
    };
    (
        specification,
        *setup_revision,
        receipt.bootstrap_job_id.expect("original job link"),
    )
}
async fn complete(runtime: &Arc<RebornRuntime>, receipt: &ProviderMutationReceipt) -> Value {
    let (specification, revision, id) = target(receipt);
    let definition =
        crate::prefix_job_definitions::Store::new(runtime.installed_catalogue.pool.clone())
            .get(id)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(
        runtime
            .wait_for_terminal(
                &definition.scope,
                TurnRunId::from_uuid(id),
                &CancellationToken::new()
            )
            .await
            .unwrap(),
        TurnStatus::Completed
    );
    let status = runtime
        .inspect_prefix_model_setup_job(specification, revision, id)
        .await
        .unwrap();
    assert_eq!(status.answered_tool_calls, 0);
    assert_eq!(status.pending_tool_calls, 0);
    assert_eq!(status.admission_phase.as_deref(), Some("settled"));
    let result = crate::prefix_bootstrap_setup::verify_result(
        &definition.input,
        status.internal_receipt.as_deref().unwrap(),
        definition.original_input_limits,
    )
    .unwrap();
    assert_eq!(result["readiness"]["generate"], "blocked");
    assert_eq!(result["readiness"]["design_acceptance"], "unknown");
    assert!(
        runtime
            .read_latest_assistant_text(
                &definition.thread_scope,
                &definition.scope.thread_id,
                TurnRunId::from_uuid(id)
            )
            .await
            .unwrap()
            .is_none()
    );
    result
}
async fn counts(pool: &brassclaw_pg::PgPool, tenant: &str) -> (i64, i64, i64) {
    let client = pool.get().await.unwrap();
    (client.query_one("SELECT count(*) FROM brassclaw_llm_provider_mutations WHERE tenant_id=$1",&[&tenant]).await.unwrap().get(0),
    client.query_one("SELECT count(*) FROM brassclaw_prefix_job_definitions WHERE scope->>'tenant_id'=$1",&[&tenant]).await.unwrap().get(0),
    client.query_one("SELECT count(*) FROM reborn_prefix_model_specifications WHERE provider_namespace=$1",&[&tenant]).await.unwrap().get(0))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_use_bootstrap_is_atomic_original_model_free_and_preserves_manual_setup() {
    use secrecy::ExposeSecret as _;
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _guard = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    // Give the real fixture an initial physical frame larger than the legacy
    // input domain. Later live tests raise/lower DATA bounds within that already
    // running transport capacity; they do not assume process-frame expansion.
    let settings = crate::pg_monty_vm_settings::PgMontyVmSettingsStore::new(
        rig.pool.clone(),
        "default",
        "default",
    );
    let before =
        brassclaw_product_workflow::MontyVmSettingsStore::get(&settings, "default", "default")
            .await
            .unwrap();
    let mut policy = before.execution_limits;
    policy.max_ipc_frame_bytes =
        u64::try_from(crate::prefix_job_definitions::LEGACY_INPUT_LIMITS.max_bytes)
            .unwrap()
            .checked_mul(2)
            .unwrap();
    policy.validate().unwrap();
    brassclaw_product_workflow::MontyVmSettingsStore::upsert(
        &settings,
        "default",
        "default",
        &serde_json::from_value(
            json!({"expected_revision":before.revision,"execution_limits":policy}),
        )
        .unwrap(),
    )
    .await
    .unwrap();

    let tenant = "native-use-bootstrap";
    let lost = Uuid::new_v4();
    let transport = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::after_bind(
            1,
            vec![lost.as_bytes().to_vec(), b"select".to_vec()],
            vec![],
            AckBoundary::Commit,
        )],
    )
    .await
    .unwrap();
    let pool = transport.pool.clone();
    let repo = crate::PgProviderRepo::new(pool.as_ref().clone(), tenant);
    repo.initialize().await.unwrap();
    repo.upsert(serde_json::from_value(json!({"id":"bootstrap-provider","protocol":"open_ai_completions","default_base_url":"http://127.0.0.1:1/v1","api_key_required":false,"default_model":"bootstrap-default","model_env":"UNUSED_BOOTSTRAP_TEST_MODEL","description":"Technical setup fixture","context_window_tokens":8192})).unwrap()).await.unwrap();
    crate::db_config::save_config_changes(
        &pool,
        tenant,
        &[
            ("llm.default.provider_id", Some("bootstrap-provider")),
            ("llm.default.model", Some("bootstrap-default")),
        ],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let home = tempfile::tempdir().unwrap();
    let input = RebornRuntimeInput::from_services(
        crate::input::RebornBuildInput::postgres_with_reborn_home(
            "bootstrap-operator".into(),
            pool.as_ref().clone(),
            rig.url.clone(),
            home.path().to_path_buf(),
        )
        .with_runtime_policy(local_dev_runtime_policy()),
    )
    .with_identity(RebornRuntimeIdentity {
        tenant_id: tenant.into(),
        agent_id: "bootstrap-agent".into(),
        source_binding_id: "bootstrap-source".into(),
        reply_target_binding_id: "bootstrap-reply".into(),
    })
    .with_poll_settings(PollSettings {
        interval: Duration::from_millis(10),
        max_total: RUNTIME_SEND_TIMEOUT,
    });
    let runtime = Arc::new(Box::pin(build_reborn_runtime(input)).await.unwrap());
    let bundle = crate::build_webui_services(runtime.clone(), None)
        .await
        .unwrap();
    // Keep the real coherent-read guard intact. Installation qualification must
    // work in RR and explicitly reject an RC evidence-reader transaction.
    let mut evidence_client = pool.get().await.unwrap();
    let rc = evidence_client.transaction().await.unwrap();
    assert!(
        runtime
            .installed_catalogue
            .setup_qualification
            .verify_in_transaction(&rc)
            .await
            .is_err()
    );
    rc.rollback().await.unwrap();
    let coherent = evidence_client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    runtime
        .installed_catalogue
        .setup_qualification
        .verify_in_transaction(&coherent)
        .await
        .unwrap();
    coherent.commit().await.unwrap();
    drop(evidence_client);

    let caller = brassclaw_product_workflow::WebUiAuthenticatedCaller::new(
        brassclaw_host_api::TenantId::new(tenant).unwrap(),
        brassclaw_host_api::UserId::new("bootstrap-operator").unwrap(),
        None,
        None,
    );
    let initial = bundle.api.get_llm_config(caller.clone()).await.unwrap();
    let provider = initial
        .providers
        .iter()
        .find(|p| p.id == "bootstrap-provider")
        .unwrap();
    let request = json!({"mutation_id":lost,"provider_id":provider.id,"expected_revision":provider.revision,"expected_selection_ref":initial.role_selection_refs["kohai"],"model":"bootstrap-selected","role":"kohai"});
    // An admission uses a different actual transaction. It must already see
    // the committed original receipt; a premature admission is rejected by PG.
    pool.get().await.unwrap().batch_execute("CREATE FUNCTION require_committed_bootstrap_original() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.scope->>'tenant_id'='native-use-bootstrap' AND NEW.trusted_internal_turn AND NOT EXISTS(SELECT 1 FROM brassclaw_llm_provider_mutations m WHERE m.tenant_id='native-use-bootstrap' AND m.bootstrap_job_id=NEW.run_id) THEN RAISE EXCEPTION 'bootstrap admission preceded original commit'; END IF; RETURN NEW; END $$; CREATE TRIGGER require_committed_bootstrap_original BEFORE INSERT ON brassclaw_monty_task_admissions FOR EACH ROW EXECUTE FUNCTION require_committed_bootstrap_original();").await.unwrap();
    transport.arm(1).unwrap();
    // The existing public owner reconciles its lost COMMIT acknowledgement
    // by reading the original; that read is not a replay of the mutation.
    let recovered = bundle
        .api
        .set_active_llm(
            caller.clone(),
            serde_json::from_value(request.clone()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        transport.cut_count(),
        1,
        "lose real provider commit acknowledgement"
    );
    let original = bundle
        .api
        .provider_mutation(caller.clone(), lost)
        .await
        .unwrap();
    let (specification, revision, job) = target(&original);
    let specification = specification.to_owned();
    assert_eq!(revision, 0);
    assert_eq!(counts(&pool, tenant).await, (1, 1, 1));
    assert_eq!(recovered.mutation_receipt.as_ref(), Some(&original));
    let exact_retry = bundle
        .api
        .set_active_llm(
            caller.clone(),
            serde_json::from_value(request.clone()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(exact_retry.mutation_receipt.as_ref(), Some(&original));
    let via_facade = bundle
        .api
        .get_prefix_model_setup_job(caller.clone(), &specification, revision, &job.to_string())
        .await
        .unwrap();
    assert_eq!(via_facade.job_id, job.to_string());
    assert!(
        bundle
            .api
            .get_prefix_model_setup_job(caller.clone(), &specification, 1, &job.to_string())
            .await
            .is_err()
    );
    let output = complete(&runtime, &original).await;
    assert_eq!(output["target"]["model"], "bootstrap-selected");
    assert_eq!(
        output["configuration"]["json"]["configured_context_window"],
        Value::Null,
        "a different model does not inherit the configured default-model context"
    );
    assert_eq!(counts(&pool, tenant).await, (1, 1, 1));
    let manual=bundle.api.update_prefix_model_setup_draft(caller.clone(),&specification,serde_json::from_value(json!({"operation_id":Uuid::new_v4(),"expected_revision":0,"edits":[{"field":"max_prefix_tokens","value":0},{"field":"max_prompt_tokens","value":null}]})).unwrap()).await.unwrap();
    assert_eq!(manual.revision, 1);
    let mut next = request.clone();
    next["mutation_id"] = json!(Uuid::new_v4());
    next["expected_selection_ref"] = json!(recovered.role_selection_refs["kohai"]);
    let selected = bundle
        .api
        .set_active_llm(caller.clone(), serde_json::from_value(next).unwrap())
        .await
        .unwrap();
    let second = selected.mutation_receipt.as_ref().unwrap();
    assert_eq!(target(second).1, 1);
    assert_ne!(target(second).2, job);
    let output = complete(&runtime, second).await;
    assert_eq!(
        output["setup"]["fields"]["max_prefix_tokens"],
        json!({"value":0,"origin":"manual"})
    );
    assert_eq!(
        output["setup"]["fields"]["max_prompt_tokens"],
        json!({"value":null,"origin":"manual"})
    );
    assert_eq!(
        bundle
            .api
            .provider_mutation(caller.clone(), lost)
            .await
            .unwrap(),
        original
    );
    let retried = bundle
        .api
        .set_active_llm(caller.clone(), serde_json::from_value(request).unwrap())
        .await
        .unwrap();
    assert_eq!(retried.mutation_receipt.as_ref(), Some(&original));
    assert_eq!(target(retried.mutation_receipt.as_ref().unwrap()).1, 0);
    // A real Upsert.set_active without request.model must retain the model that
    // the actual draft SDK configuration resolved, not an absent/default echo.
    let before = repo
        .get_revision("bootstrap-provider", false)
        .await
        .unwrap()
        .unwrap();
    let saved=bundle.api.upsert_llm_provider(caller.clone(),serde_json::from_value(json!({"mutation_id":Uuid::new_v4(),"id":"bootstrap-provider","expected_revision":before.revision,"adapter":"open_ai_completions","name":"Prepared active update","default_model":"prepared-upsert-default","set_active":true,"expected_selection_ref":retried.role_selection_refs["kohai"],"api_key":"bootstrap-supplied-credential"})).unwrap()).await.unwrap();
    let output = complete(&runtime, saved.mutation_receipt.as_ref().unwrap()).await;
    assert_eq!(output["target"]["model"], "prepared-upsert-default");
    assert_eq!(
        saved.kohai_active.as_ref().unwrap().model.as_deref(),
        Some("prepared-upsert-default")
    );
    assert!(!output.to_string().contains("bootstrap-supplied-credential"));
    let before_count = counts(&pool, tenant).await;
    let before = repo
        .get_revision("bootstrap-provider", false)
        .await
        .unwrap()
        .unwrap();
    let observer = pool.get().await.unwrap();
    observer.batch_execute("CREATE FUNCTION reject_use_bootstrap_metadata() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.operation='bootstrap_setup' AND NEW.scope->>'tenant_id'='native-use-bootstrap' THEN RAISE EXCEPTION 'deliberate bootstrap reservation rejection'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_use_bootstrap_metadata BEFORE INSERT ON brassclaw_prefix_job_definitions FOR EACH ROW EXECUTE FUNCTION reject_use_bootstrap_metadata();").await.unwrap();
    let rollback = json!({"mutation_id":Uuid::new_v4(),"id":"bootstrap-provider","expected_revision":before.revision,"adapter":"open_ai_completions","name":"Must roll back","default_model":"must-not-publish","set_active":true,"expected_selection_ref":saved.role_selection_refs["kohai"],"api_key":"must-rollback-bootstrap-key"});
    assert!(
        bundle
            .api
            .upsert_llm_provider(caller.clone(), serde_json::from_value(rollback).unwrap())
            .await
            .is_err()
    );
    observer.batch_execute("DROP TRIGGER reject_use_bootstrap_metadata ON brassclaw_prefix_job_definitions; DROP FUNCTION reject_use_bootstrap_metadata();").await.unwrap();
    assert_eq!(counts(&pool, tenant).await, before_count);
    assert_eq!(
        repo.get_revision("bootstrap-provider", false)
            .await
            .unwrap()
            .unwrap()
            .revision,
        before.revision
    );
    assert_eq!(
        runtime
            .services()
            .llm_key_store()
            .read("bootstrap-provider")
            .await
            .unwrap()
            .unwrap()
            .expose_secret(),
        "bootstrap-supplied-credential"
    );
    let unchanged = bundle.api.get_llm_config(caller.clone()).await.unwrap();
    assert_eq!(
        unchanged.role_selection_refs["kohai"],
        saved.role_selection_refs["kohai"]
    );
    // Fail after reservation as well: receipt failure must remove the job and
    // newly captured specification with the provider/credential writes.
    observer.batch_execute("CREATE FUNCTION reject_use_bootstrap_receipt() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.tenant_id='native-use-bootstrap' THEN RAISE EXCEPTION 'deliberate linked receipt rejection'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_use_bootstrap_receipt BEFORE INSERT ON brassclaw_llm_provider_mutations FOR EACH ROW EXECUTE FUNCTION reject_use_bootstrap_receipt();").await.unwrap();
    assert!(bundle.api.upsert_llm_provider(caller.clone(),serde_json::from_value(json!({"mutation_id":Uuid::new_v4(),"id":"bootstrap-provider","expected_revision":before.revision,"adapter":"open_ai_completions","default_model":"also-must-not-publish","set_active":true,"expected_selection_ref":saved.role_selection_refs["kohai"],"api_key":"also-must-rollback"})).unwrap()).await.is_err());
    observer.batch_execute("DROP TRIGGER reject_use_bootstrap_receipt ON brassclaw_llm_provider_mutations; DROP FUNCTION reject_use_bootstrap_receipt();").await.unwrap();
    assert_eq!(counts(&pool, tenant).await, before_count);
    assert_eq!(
        repo.get_revision("bootstrap-provider", false)
            .await
            .unwrap()
            .unwrap()
            .revision,
        before.revision
    );
    drop(observer);
    // Existing encrypted credential owner changes while the real Use commit is
    // waiting at the provider row: the retained preflight must fail closed.
    let mut blocker = pool.get().await.unwrap();
    let tx = blocker.transaction().await.unwrap();
    let pid: i32 = tx
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .unwrap()
        .get(0);
    tx.query_one("SELECT id FROM brassclaw_llm_providers WHERE tenant_id=$1 AND id='bootstrap-provider' FOR UPDATE",&[&tenant]).await.unwrap();
    let api = bundle.api.clone();
    let operator = caller.clone();
    let token = saved.role_selection_refs["kohai"].clone();
    let expected = before.revision;
    let pending = tokio::spawn(async move {
        api.set_active_llm(operator,serde_json::from_value(json!({"mutation_id":Uuid::new_v4(),"provider_id":"bootstrap-provider","expected_revision":expected,"expected_selection_ref":token,"model":"must-not-use-rotated-key","role":"kohai"})).unwrap()).await
    });
    tokio::time::timeout(Duration::from_secs(10),async{
        loop{if pool.get().await.unwrap().query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE query LIKE '%FROM brassclaw_llm_providers WHERE tenant_id=$1 AND id=$2%' AND $1=ANY(pg_blocking_pids(pid)))",&[&pid]).await.unwrap().get::<_,bool>(0){break;}assert!(!pending.is_finished());tokio::time::sleep(Duration::from_millis(10)).await;}
    }).await.expect("real Use writer waits after SDK preparation");
    runtime
        .services()
        .llm_key_store()
        .put("bootstrap-provider", "rotated-during-preparation".into())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    drop(blocker);
    assert!(pending.await.unwrap().is_err());
    assert_eq!(counts(&pool, tenant).await, before_count);
    assert_eq!(
        bundle
            .api
            .provider_mutation(caller.clone(), lost)
            .await
            .unwrap(),
        original,
        "original retry survives later model/auth edits"
    );
    pool.get().await.unwrap().batch_execute("DROP TRIGGER require_committed_bootstrap_original ON brassclaw_monty_task_admissions; DROP FUNCTION require_committed_bootstrap_original();").await.unwrap();
    limits::assert_original_input_limits(&runtime, bundle.api.as_ref(), &caller).await;
    drop(bundle);
    shutdown_shared_runtime(runtime).await.unwrap();
}
