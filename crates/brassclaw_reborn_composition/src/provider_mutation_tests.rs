//! Actual provider service, encrypted secrets, PostgreSQL and live reload.
//! The wire transport drops real COMMIT acknowledgements; no result is invented.
use brassclaw_llm::{LlmProvider, SwappableLlmProvider};
use brassclaw_pg::{
    PgPool,
    test_support::{AckBoundary, AckCutRule, AckCutTransport},
};
use brassclaw_product_workflow::{
    LlmConfigService, LlmConfigServiceError, WebUiAuthenticatedCaller,
};
use secrecy::ExposeSecret as _;
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

fn service(
    pool: Arc<PgPool>,
    keys: crate::LlmKeyStore,
    repo: Arc<crate::PgProviderRepo>,
    tenant: &str,
) -> (crate::RebornLlmConfigService, Arc<SwappableLlmProvider>) {
    let primary = Arc::new(SwappableLlmProvider::new(Arc::new(
        crate::runtime::PlaceholderLlmProvider,
    )));
    let sempai = Arc::new(SwappableLlmProvider::new(Arc::new(
        crate::runtime::PlaceholderLlmProvider,
    )));
    let reload = Arc::new(crate::llm_reload::RebornLlmReloadAdapter::new(
        Arc::new(brassclaw_llm::LlmReloadHandle::new(primary, None)),
        Arc::new(brassclaw_llm::SessionManager::new(Default::default())),
        keys.clone(),
        pool.clone(),
        tenant,
    ));
    (
        crate::RebornLlmConfigService::new(keys, pool, repo, tenant)
            .with_reload_trigger(reload)
            .with_sempai_swappable(sempai.clone()),
        sempai,
    )
}
fn caller(tenant: &str) -> WebUiAuthenticatedCaller {
    WebUiAuthenticatedCaller::new(
        brassclaw_host_api::TenantId::new(tenant).unwrap(),
        brassclaw_host_api::UserId::new("mutation-operator").unwrap(),
        None,
        None,
    )
}
fn upsert(id: Uuid, revision: Option<u64>, key: &str) -> Value {
    json!({"mutation_id":id,"id":"receipt-provider","expected_revision":revision,
        "adapter":"open_ai_completions","base_url":"http://127.0.0.1:1/v1",
        "default_model":"receipt-model","api_key":key})
}

#[test]
fn provider_request_proof_uses_canonical_json_order() {
    let id = Uuid::new_v4();
    let original = super::ProviderMutation::new(
        id,
        "select",
        serde_json::from_str(r#"{"z":[{"b":2,"a":1}],"a":"same"}"#).unwrap(),
        None,
    )
    .unwrap();
    let replay = super::ProviderMutation::new(
        id,
        "select",
        serde_json::from_str(r#"{"a":"same","z":[{"a":1,"b":2}]}"#).unwrap(),
        None,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_string(&original.request).unwrap(),
        r#"{"a":"same","z":[{"a":1,"b":2}]}"#
    );
    assert_eq!(original.proof.expose_secret(), replay.proof.expose_secret());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_role_edits_fence_concurrent_and_external_writers_without_retargeting() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let pool = rig.pool.clone();
    let tenant = "role-selection-cas";
    let repo = Arc::new(crate::PgProviderRepo::new(pool.as_ref().clone(), tenant));
    repo.initialize().await.unwrap();
    repo.upsert(serde_json::from_value(json!({"id":"role-provider","protocol":"open_ai_completions",
        "default_base_url":"http://127.0.0.1:1/v1","api_key_required":false,
        "model_env":"UNUSED_ROLE_MODEL","default_model":"role-default","description":"Actual role edit fixture"})).unwrap()).await.unwrap();
    let keys = crate::LlmKeyStore::new_postgres(Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            pool.as_ref().clone(),
            "role-selection-authenticated-host-master-fixture".into(),
            tenant,
        )
        .unwrap(),
    ));
    let (a, _) = service(pool.clone(), keys.clone(), repo.clone(), tenant);
    let (b, _) = service(pool.clone(), keys.clone(), repo.clone(), tenant);
    let a = Arc::new(a);
    let b = Arc::new(b);
    let operator = caller(tenant);
    let displayed = a.snapshot(operator.clone()).await.unwrap();
    let revision = displayed
        .providers
        .iter()
        .find(|row| row.id == "role-provider")
        .unwrap()
        .revision;
    let request = |reference: &str, model: &str| {
        json!({"mutation_id":Uuid::new_v4(),
        "provider_id":"role-provider","expected_revision":revision,"model":model,"role":"kohai",
        "expected_selection_ref":reference})
    };
    crate::db_config::save_config_changes(
        &pool,
        tenant,
        &[
            (
                "llm.default.base_url",
                Some("http://127.0.0.1:2/old-target"),
            ),
            (
                "llm.default.api_key_env",
                Some("LEGACY_ROLE_CREDENTIAL_NAME"),
            ),
        ],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let stale = request(&displayed.role_selection_refs["kohai"], "stale-empty-role");
    assert!(matches!(
        a.set_active(
            operator.clone(),
            serde_json::from_value(stale.clone()).unwrap()
        )
        .await,
        Err(LlmConfigServiceError::Conflict { .. })
    ));
    assert!(matches!(
        a.provider_mutation(
            operator.clone(),
            serde_json::from_value(stale["mutation_id"].clone()).unwrap()
        )
        .await,
        Err(LlmConfigServiceError::NotFound)
    ));
    let current = a.snapshot(operator.clone()).await.unwrap();
    let selected = a
        .set_active(
            operator.clone(),
            serde_json::from_value(request(
                &current.role_selection_refs["kohai"],
                "selected-canonical-target",
            ))
            .unwrap(),
        )
        .await
        .unwrap();
    let values = crate::db_config::provider_role_selections(&**pool.get().await.unwrap(), tenant)
        .await
        .unwrap();
    assert!(!values.contains_key("llm.default.base_url"));
    assert!(!values.contains_key("llm.default.api_key_env"));
    assert!(
        matches!(&selected.runtime_publications["kohai"], brassclaw_llm::ProviderPublication::Configured {selection}
        if selection.model == "selected-canonical-target")
    );

    // Retain a real pre-CAS record and selection in the old persisted format.
    // It is an already committed original, not a new request allowed to omit
    // the precondition. No model submission is needed for configuration custody.
    let legacy_id = Uuid::new_v4();
    let legacy_payload = json!({"provider_id":"role-provider","expected_revision":revision,
        "model":"legacy-confirmed-model","role":"kohai"});
    let legacy =
        super::ProviderMutation::new(legacy_id, "select", legacy_payload.clone(), None).unwrap();
    let mut client = pool.get().await.unwrap();
    let tx = client.transaction().await.unwrap();
    legacy.lock(&tx, tenant).await.unwrap();
    repo.lock_active_revision(&tx, "role-provider")
        .await
        .unwrap()
        .unwrap();
    crate::db_config::save_config_changes_in_transaction(
        &tx,
        tenant,
        &[
            ("llm.default.provider_id", Some("role-provider")),
            ("llm.default.model", Some("legacy-confirmed-model")),
        ],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let legacy_receipt = legacy
        .record(
            &tx,
            tenant,
            operator.user_id.as_str(),
            Some(("role-provider", revision)),
            &keys,
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    drop(client);
    let mut legacy_request = legacy_payload;
    legacy_request["mutation_id"] = json!(legacy_id);
    let selected = a
        .set_active(
            operator.clone(),
            serde_json::from_value(legacy_request).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(selected.mutation_receipt.as_ref(), Some(&legacy_receipt));

    // Separate service instances have independent in-process publication gates;
    // only the real shared PostgreSQL precondition can choose one winner.
    let first = request(&selected.role_selection_refs["kohai"], "concurrent-one");
    let second = request(&selected.role_selection_refs["kohai"], "concurrent-two");
    let (one, two) = tokio::join!(
        a.set_active(
            operator.clone(),
            serde_json::from_value(first.clone()).unwrap()
        ),
        b.set_active(
            operator.clone(),
            serde_json::from_value(second.clone()).unwrap()
        ),
    );
    let (winner, result, loser) = match (one, two) {
        (Ok(result), Err(LlmConfigServiceError::Conflict { .. })) => (first, result, second),
        (Err(LlmConfigServiceError::Conflict { .. }), Ok(result)) => (second, result, first),
        _ => panic!("one displayed role reference can publish only one changed selection"),
    };
    assert!(matches!(
        a.provider_mutation(
            operator.clone(),
            serde_json::from_value(loser["mutation_id"].clone()).unwrap()
        )
        .await,
        Err(LlmConfigServiceError::NotFound)
    ));
    let original = result.mutation_receipt.unwrap();
    let pending = request(
        &result.role_selection_refs["kohai"],
        "must-not-overwrite-external-writer",
    );
    let mut external = pool.get().await.unwrap();
    let pid: i32 = external
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .unwrap()
        .get(0);
    let tx = external.transaction().await.unwrap();
    tx.execute("UPDATE brassclaw_config SET value='external-pending-model' WHERE tenant_id=$1 AND key='llm.default.model'", &[&tenant]).await.unwrap();
    let call = {
        let a = a.clone();
        let actor = operator.clone();
        tokio::spawn(async move {
            a.set_active(actor, serde_json::from_value(pending).unwrap())
                .await
        })
    };
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while !pool.get().await.unwrap().query_one(
            "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE query LIKE 'LOCK TABLE brassclaw_config IN SHARE ROW EXCLUSIVE MODE%' AND $1=ANY(pg_blocking_pids(pid)))", &[&pid],
        ).await.unwrap().get::<_, bool>(0) {
            assert!(!call.is_finished(), "role edit reaches the real pending config writer");
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await.expect("actual config writer fences the selection check");
    tx.commit().await.unwrap();
    drop(external);
    assert!(matches!(
        call.await.unwrap(),
        Err(LlmConfigServiceError::Conflict { .. })
    ));
    let replay = a
        .set_active(operator.clone(), serde_json::from_value(winner).unwrap())
        .await
        .unwrap();
    assert_eq!(replay.mutation_receipt, Some(original));
    assert_eq!(
        replay.kohai_active.as_ref().unwrap().model.as_deref(),
        Some("external-pending-model")
    );
    let mut missing = request(&replay.role_selection_refs["kohai"], "missing-precondition");
    missing
        .as_object_mut()
        .unwrap()
        .remove("expected_selection_ref");
    assert!(
        matches!(a.set_active(operator.clone(), serde_json::from_value(missing).unwrap()).await,
        Err(LlmConfigServiceError::InvalidRequest {field:Some(field), ..}) if field == "expected_selection_ref")
    );
    let mut missing = request(
        &replay.role_selection_refs["kohai"],
        "missing-definition-revision",
    );
    missing.as_object_mut().unwrap().remove("expected_revision");
    assert!(
        matches!(a.set_active(operator.clone(), serde_json::from_value(missing).unwrap()).await,
        Err(LlmConfigServiceError::InvalidRequest {field:Some(field), ..}) if field == "expected_revision")
    );
    let mut connection = pool.get().await.unwrap();
    let tx = tokio_postgres::Client::build_transaction(&mut connection)
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    assert!(matches!(
        crate::db_config::save_model_selection_in_transaction(
            &tx,
            tenant,
            "kohai",
            &replay.role_selection_refs["kohai"],
            Some("role-provider"),
            Some("must-not-publish-from-old-snapshot")
        )
        .await,
        Err(crate::db_config::ConfigError::ModelSelectionTransactionIsolation)
    ));
    tx.rollback().await.unwrap();
    let final_view = a.snapshot(operator).await.unwrap();
    assert_eq!(final_view.role_selection_refs, replay.role_selection_refs);
    assert_eq!(
        final_view
            .providers
            .iter()
            .find(|row| row.id == "role-provider")
            .unwrap()
            .revision,
        revision
    );
    let primary = request(
        &final_view.role_selection_refs["kohai"],
        "independent-primary",
    );
    let sempai = json!({"mutation_id":Uuid::new_v4(),"provider_id":"role-provider",
        "expected_revision":revision,"expected_selection_ref":final_view.role_selection_refs["sempai"],
        "model":"independent-sempai","role":"sempai"});
    let (primary, sempai) = tokio::join!(
        a.set_active(caller(tenant), serde_json::from_value(primary).unwrap()),
        b.set_active(caller(tenant), serde_json::from_value(sempai).unwrap()),
    );
    assert!(
        primary.is_ok() && sempai.is_ok(),
        "independent role references must not conflict"
    );
    // Deactivation must enter the same config barrier before any config row
    // locks. Otherwise a different provider's selection can hold the table
    // barrier while waiting on a row held by cleanup, forming a lock cycle.
    let mut other = repo.get("role-provider").await.unwrap().unwrap();
    other.id = "other-role-provider".into();
    repo.upsert(other).await.unwrap();
    let displayed = a.snapshot(caller(tenant)).await.unwrap();
    let change = json!({"mutation_id":Uuid::new_v4(),"provider_id":"other-role-provider",
        "expected_revision":displayed.providers.iter().find(|row| row.id == "other-role-provider").unwrap().revision,
        "expected_selection_ref":displayed.role_selection_refs["kohai"],"model":"other-model","role":"kohai"});
    let mut gate = pool.get().await.unwrap();
    let tx = gate.transaction().await.unwrap();
    tx.execute(
        "UPDATE brassclaw_config SET value=value WHERE tenant_id=$1 AND key='llm.default.model'",
        &[&tenant],
    )
    .await
    .unwrap();
    async fn barriers(pool: &PgPool, count: i64) {
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            while pool.get().await.unwrap().query_one(
                "SELECT count(DISTINCT l.pid) FROM pg_locks l JOIN pg_stat_activity a ON a.pid=l.pid
                 WHERE NOT l.granted AND l.mode='ShareRowExclusiveLock' AND l.relation='brassclaw_config'::regclass
                 AND a.query LIKE 'LOCK TABLE brassclaw_config IN SHARE ROW EXCLUSIVE MODE%'", &[],
            ).await.unwrap().get::<_, i64>(0) != count {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        }).await.expect("both real role writers enter the same config barrier");
    }
    let removal_id = Uuid::new_v4();
    let removal = {
        let a = a.clone();
        tokio::spawn(async move {
            a.delete_provider(caller(tenant), "role-provider".into(), revision, removal_id)
                .await
        })
    };
    barriers(&pool, 1).await;
    let selected = tokio::spawn(async move {
        b.set_active(caller(tenant), serde_json::from_value(change).unwrap())
            .await
    });
    barriers(&pool, 2).await;
    tx.commit().await.unwrap();
    drop(gate);
    assert!(removal.await.unwrap().is_ok());
    assert!(matches!(
        selected.await.unwrap(),
        Err(LlmConfigServiceError::Conflict { .. })
    ));
    let after = a.snapshot(caller(tenant)).await.unwrap();
    assert!(after.kohai_active.is_none());
    assert!(
        after
            .inactive_providers
            .iter()
            .any(|row| row.id == "role-provider")
    );
}

#[tokio::test]
#[ignore = "requires BRASSCLAW_TEST_MODEL_URL and BRASSCLAW_TEST_MODEL_ID for actual operator-selected inference"]
async fn native_role_publication_reaches_actual_primary_and_sempai_model() {
    let endpoint = std::env::var("BRASSCLAW_TEST_MODEL_URL").expect("explicit operator endpoint");
    let model = std::env::var("BRASSCLAW_TEST_MODEL_ID").expect("explicit operator model");
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let pool = rig.pool.clone();
    let tenant = "actual-role-publication";
    let repo = Arc::new(crate::PgProviderRepo::new(pool.as_ref().clone(), tenant));
    repo.initialize().await.unwrap();
    let keys = crate::LlmKeyStore::new_postgres(Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            pool.as_ref().clone(),
            "actual-role-publication-authenticated-master-fixture".into(),
            tenant,
        )
        .unwrap(),
    ));
    let primary = Arc::new(SwappableLlmProvider::new(Arc::new(
        crate::runtime::PlaceholderLlmProvider,
    )));
    let sempai = Arc::new(SwappableLlmProvider::new(Arc::new(
        crate::runtime::PlaceholderLlmProvider,
    )));
    let reload = Arc::new(crate::llm_reload::RebornLlmReloadAdapter::new(
        Arc::new(brassclaw_llm::LlmReloadHandle::new(primary.clone(), None)),
        Arc::new(brassclaw_llm::SessionManager::new(Default::default())),
        keys.clone(),
        pool.clone(),
        tenant,
    ));
    let api = crate::RebornLlmConfigService::new(keys, pool, repo, tenant)
        .with_reload_trigger(reload)
        .with_sempai_swappable(sempai.clone());
    let operator = caller(tenant);
    let initial = api.snapshot(operator.clone()).await.unwrap();
    let mut policy = initial.request_policy_defaults;
    policy.max_retries = 0;
    let mut cache = initial.response_cache_defaults;
    cache.enabled = false;
    let saved = api.upsert_provider(operator.clone(), serde_json::from_value(json!({
        "mutation_id":Uuid::new_v4(),"id":"actual-role-provider","adapter":"open_ai_completions",
        "base_url":endpoint,"default_model":model,"model":model,"set_active":true,
        "expected_selection_ref":initial.role_selection_refs["kohai"],"request_policy":policy,"response_cache":cache,
    })).unwrap()).await.unwrap();
    let revision = saved
        .providers
        .iter()
        .find(|row| row.id == "actual-role-provider")
        .unwrap()
        .revision;
    let retained_primary = primary.pinned_provider().0;
    let selected = api.set_active(operator.clone(), serde_json::from_value(json!({
        "mutation_id":Uuid::new_v4(),"provider_id":"actual-role-provider","expected_revision":revision,
        "expected_selection_ref":saved.role_selection_refs["sempai"],"model":model,"role":"sempai",
    })).unwrap()).await.unwrap();
    assert!(Arc::ptr_eq(&retained_primary, &primary.pinned_provider().0));
    for (role, provider) in [("kohai", primary.clone()), ("sempai", sempai)] {
        assert_eq!(provider.pinned_provider().1, model);
        assert!(
            matches!(&selected.runtime_publications[role], brassclaw_llm::ProviderPublication::Configured {selection}
            if selection.model == model && selection.definition_revision == revision)
        );
        let request =
            brassclaw_llm::CompletionRequest::new(vec![brassclaw_llm::ChatMessage::user(format!(
                "Reply briefly with the word ready. This is the {role} provider publication check."
            ))]);
        let response = provider
            .complete(request)
            .await
            .expect("actual selected inference must succeed");
        assert!(!response.content.trim().is_empty());
        let usage = response.usage.expect("actual operator model reports usage");
        assert!(usage.input_tokens > 0 && usage.output_tokens > 0);
    }
    assert!(matches!(
        api.set_active(
            operator.clone(),
            serde_json::from_value(json!({
                "mutation_id":Uuid::new_v4(),"provider_id":"","role":"kohai",
                "expected_selection_ref":initial.role_selection_refs["kohai"],
            }))
            .unwrap()
        )
        .await,
        Err(LlmConfigServiceError::Conflict { .. })
    ));
    assert!(Arc::ptr_eq(&retained_primary, &primary.pinned_provider().0));
    let current = api.snapshot(operator).await.unwrap();
    assert_eq!(current.role_selection_refs, selected.role_selection_refs);
    assert_eq!(current.runtime_publications, selected.runtime_publications);
}

#[tokio::test]
async fn native_provider_original_mutations_survive_commit_ack_loss_and_later_writers() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let ids = [
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    ];
    let transport = AckCutTransport::start(
        rig.url.expose_secret(),
        ids.iter()
            .enumerate()
            .map(|(index, id)| {
                AckCutRule::after_bind(
                    (index + 1) as u8,
                    vec![
                        id.as_bytes().to_vec(),
                        match index {
                            0 | 1 | 3 => b"upsert".to_vec(),
                            2 => b"deactivate".to_vec(),
                            _ => b"select".to_vec(),
                        },
                    ],
                    vec![],
                    AckBoundary::Commit,
                )
            })
            .collect(),
    )
    .await
    .unwrap();
    let pool = transport.pool.clone();
    let tenant = "provider-original";
    let repo = Arc::new(crate::PgProviderRepo::new(pool.as_ref().clone(), tenant));
    repo.initialize().await.unwrap();
    let keys = crate::LlmKeyStore::new_postgres(Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            pool.as_ref().clone(),
            "original-provider-proof-master-fixture".into(),
            tenant,
        )
        .unwrap(),
    ));
    let (api, sempai) = service(pool.clone(), keys.clone(), repo.clone(), tenant);
    let operator = caller(tenant);
    let original = upsert(ids[0], None, "fixture-key-alpha");
    transport.arm(1).unwrap();
    let saved = api
        .upsert_provider(
            operator.clone(),
            serde_json::from_value(original.clone()).unwrap(),
        )
        .await
        .unwrap();
    let first = saved.mutation_receipt.unwrap();
    assert_eq!(first.mutation_id, ids[0]);
    assert_eq!(first.provider_revision, Some(1));
    assert_eq!(transport.cut_count(), 1);
    let client = pool.get().await.unwrap();
    let before: Vec<(String, String)> = client
        .query(
            "SELECT scope,ciphertext FROM brassclaw_secrets WHERE tenant_id=$1 ORDER BY scope,name",
            &[&tenant],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| (row.get(0), row.get(1)))
        .collect();
    drop(client);
    assert_eq!(
        before.len(),
        2,
        "credential and encrypted original proof only"
    );
    let replay = api
        .upsert_provider(
            operator.clone(),
            serde_json::from_value(original.clone()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replay.mutation_receipt.as_ref(), Some(&first));
    assert_eq!(
        api.provider_mutation(operator.clone(), ids[0])
            .await
            .unwrap(),
        first
    );
    assert_eq!(
        api.reconcile_provider_mutation(operator.clone(), ids[0])
            .await
            .unwrap()
            .mutation_receipt,
        Some(first.clone())
    );
    let client = pool.get().await.unwrap();
    let after: Vec<(String, String)> = client
        .query(
            "SELECT scope,ciphertext FROM brassclaw_secrets WHERE tenant_id=$1 ORDER BY scope,name",
            &[&tenant],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| (row.get(0), row.get(1)))
        .collect();
    assert_eq!(
        before, after,
        "read/retry/reconcile never rewrites credential or proof ciphertext"
    );
    let recorded:Value=client.query_one("SELECT request FROM brassclaw_llm_provider_mutations WHERE tenant_id=$1 AND mutation_id=$2",&[&tenant,&ids[0]]).await.unwrap().get(0);
    assert_eq!(recorded["has_new_key"], true);
    assert!(recorded.get("api_key").is_none());
    assert!(!recorded.to_string().contains("fixture-key-alpha"));
    drop(client);
    let mut changed = original.clone();
    changed["api_key"] = json!("fixture-key-different");
    assert!(matches!(
        api.upsert_provider(operator.clone(), serde_json::from_value(changed).unwrap())
            .await,
        Err(LlmConfigServiceError::Conflict { .. })
    ));
    assert_eq!(
        keys.read("receipt-provider")
            .await
            .unwrap()
            .unwrap()
            .expose_secret(),
        "fixture-key-alpha"
    );

    let wrong_keys = crate::LlmKeyStore::new_postgres(Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            pool.as_ref().clone(),
            "different-proof-master-fixture-value".into(),
            tenant,
        )
        .unwrap(),
    ));
    let (wrong_owner, _) = service(pool.clone(), wrong_keys, repo.clone(), tenant);
    assert!(matches!(
        wrong_owner
            .upsert_provider(
                operator.clone(),
                serde_json::from_value(original.clone()).unwrap()
            )
            .await,
        Err(LlmConfigServiceError::Unavailable)
    ));
    let other_tenant = "another-provider-namespace";
    let other_keys = crate::LlmKeyStore::new_postgres(Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            pool.as_ref().clone(),
            "original-provider-proof-master-fixture".into(),
            other_tenant,
        )
        .unwrap(),
    ));
    let other_repo = Arc::new(crate::PgProviderRepo::new(
        pool.as_ref().clone(),
        other_tenant,
    ));
    let (other_namespace, _) = service(pool.clone(), other_keys, other_repo, other_tenant);
    assert!(matches!(
        other_namespace
            .provider_mutation(caller("another-provider-namespace"), ids[0])
            .await,
        Err(LlmConfigServiceError::NotFound)
    ));

    // Secret-only publications have their own original receipt even when the
    // provider definition and therefore its revision are identical.
    let secret_only = upsert(ids[1], Some(1), "fixture-key-beta");
    transport.arm(2).unwrap();
    let rotated = api
        .upsert_provider(
            operator.clone(),
            serde_json::from_value(secret_only.clone()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rotated.mutation_receipt.unwrap().provider_revision, Some(1));
    assert_eq!(
        keys.read("receipt-provider")
            .await
            .unwrap()
            .unwrap()
            .expose_secret(),
        "fixture-key-beta"
    );
    assert_eq!(
        api.upsert_provider(
            operator.clone(),
            serde_json::from_value(original.clone()).unwrap()
        )
        .await
        .unwrap()
        .mutation_receipt,
        Some(first.clone())
    );
    assert_eq!(
        keys.read("receipt-provider")
            .await
            .unwrap()
            .unwrap()
            .expose_secret(),
        "fixture-key-beta",
        "original readback must never restore its preceding key"
    );

    transport.arm(3).unwrap();
    let removed = api
        .delete_provider(operator.clone(), "receipt-provider".into(), 1, ids[2])
        .await
        .unwrap();
    assert_eq!(
        removed.mutation_receipt.as_ref().unwrap().provider_revision,
        Some(2)
    );
    assert!(
        repo.get_revision("receipt-provider", true)
            .await
            .unwrap()
            .unwrap()
            .inactive
    );
    assert_eq!(
        api.delete_provider(operator.clone(), "receipt-provider".into(), 1, ids[2])
            .await
            .unwrap()
            .mutation_receipt,
        removed.mutation_receipt
    );
    assert!(matches!(
        api.delete_provider(operator.clone(), "receipt-provider".into(), 2, ids[2])
            .await,
        Err(LlmConfigServiceError::Conflict { .. })
    ));

    transport.arm(4).unwrap();
    let readd = upsert(ids[3], Some(2), "fixture-key-gamma");
    let added = api
        .upsert_provider(operator.clone(), serde_json::from_value(readd).unwrap())
        .await
        .unwrap();
    assert_eq!(added.mutation_receipt.unwrap().provider_revision, Some(3));
    assert!(added.sempai_active.is_none());
    transport.arm(5).unwrap();
    let observed = api.snapshot(operator.clone()).await.unwrap();
    let selection = json!({"expected_selection_ref":observed.role_selection_refs["sempai"],"mutation_id":ids[4],"expected_revision":3,"provider_id":"receipt-provider","model":"receipt-sempai","role":"sempai"});
    let selected = api
        .set_active(
            operator.clone(),
            serde_json::from_value(selection.clone()).unwrap(),
        )
        .await
        .unwrap();
    let selected_original = selected.mutation_receipt.unwrap();
    assert_eq!(selected_original.provider_revision, Some(3));
    assert_eq!(
        selected_original.selected_role,
        Some(brassclaw_llm::ProviderRole::Sempai)
    );
    assert_eq!(
        selected_original.role_selections["llm.sempai.model"],
        "receipt-sempai"
    );
    assert_eq!(sempai.model_name(), "receipt-sempai");
    assert_eq!(transport.cut_count(), 5);

    // A real independent writer changes the definition and subsequently clears
    // the role. Original replay/restart must not claim that newer edit as its
    // own, nor re-add the provider or rewrite its original role selection.
    let mut external = repo
        .get_revision("receipt-provider", false)
        .await
        .unwrap()
        .unwrap()
        .definition;
    external.description = "External later definition".into();
    assert_eq!(
        repo.replace_revision(external, Some(3))
            .await
            .unwrap()
            .revision,
        4
    );
    crate::db_config::save_config_changes(
        &pool,
        tenant,
        &[("llm.sempai.provider_id", None), ("llm.sempai.model", None)],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let (restarted, _) = service(pool.clone(), keys.clone(), repo.clone(), tenant);
    let reconciled = restarted
        .reconcile_provider_mutation(operator.clone(), ids[4])
        .await
        .unwrap();
    assert_eq!(reconciled.mutation_receipt, Some(selected_original));
    assert!(reconciled.sempai_active.is_none());
    assert_eq!(
        reconciled
            .providers
            .iter()
            .find(|p| p.id == "receipt-provider")
            .unwrap()
            .revision,
        4
    );
    assert_eq!(
        restarted
            .set_active(operator.clone(), serde_json::from_value(selection).unwrap())
            .await
            .unwrap()
            .sempai_active,
        None
    );
    // Even the original create must not reactivate a later deactivation.
    let final_id = Uuid::new_v4();
    restarted
        .delete_provider(operator.clone(), "receipt-provider".into(), 4, final_id)
        .await
        .unwrap();
    restarted
        .upsert_provider(operator.clone(), serde_json::from_value(original).unwrap())
        .await
        .unwrap();
    assert!(
        repo.get_revision("receipt-provider", true)
            .await
            .unwrap()
            .unwrap()
            .inactive
    );
    assert_eq!(
        repo.get_revision("receipt-provider", true)
            .await
            .unwrap()
            .unwrap()
            .revision,
        5
    );
    transport.shutdown().await.unwrap();
}

#[tokio::test]
async fn native_provider_mutation_concurrency_rollback_and_reload_failure_keep_original_identity() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let pool = rig.pool.clone();
    let tenant = "provider-concurrency";
    let repo = Arc::new(crate::PgProviderRepo::new(pool.as_ref().clone(), tenant));
    repo.initialize().await.unwrap();
    let keys = crate::LlmKeyStore::new_postgres(Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            pool.as_ref().clone(),
            "concurrent-provider-proof-master".into(),
            tenant,
        )
        .unwrap(),
    ));
    let (a, _) = service(pool.clone(), keys.clone(), repo.clone(), tenant);
    let (b, _) = service(pool.clone(), keys.clone(), repo.clone(), tenant);
    let operator = caller(tenant);
    let id = Uuid::new_v4();
    let left = upsert(id, None, "left-fixture-key");
    let right = upsert(id, None, "right-fixture-key");
    let (left_result, right_result) = tokio::join!(
        a.upsert_provider(
            operator.clone(),
            serde_json::from_value(left.clone()).unwrap()
        ),
        b.upsert_provider(
            operator.clone(),
            serde_json::from_value(right.clone()).unwrap()
        )
    );
    let winner = match (&left_result, &right_result) {
        (Ok(_), Err(LlmConfigServiceError::Conflict { .. })) => "left-fixture-key",
        (Err(LlmConfigServiceError::Conflict { .. }), Ok(_)) => "right-fixture-key",
        _ => panic!(
            "one original wins and one conflicts; received left={left_result:?}, right={right_result:?}"
        ),
    };
    assert_eq!(
        keys.read("receipt-provider")
            .await
            .unwrap()
            .unwrap()
            .expose_secret(),
        winner
    );
    let client = pool.get().await.unwrap();
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM brassclaw_llm_provider_mutations WHERE tenant_id=$1",
                &[&tenant]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    // Roll back the actual provider/key/history/proof/receipt transaction.
    client.batch_execute("CREATE FUNCTION reject_provider_receipt() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'deliberate provider receipt rejection'; END $$; CREATE TRIGGER reject_provider_receipt BEFORE INSERT ON brassclaw_llm_provider_mutations FOR EACH ROW EXECUTE FUNCTION reject_provider_receipt()").await.unwrap();
    drop(client);
    let rejected_id = Uuid::new_v4();
    assert!(matches!(
        a.upsert_provider(
            operator.clone(),
            serde_json::from_value(upsert(rejected_id, Some(1), "must-rollback-key")).unwrap()
        )
        .await,
        Err(LlmConfigServiceError::Unavailable)
    ));
    assert!(matches!(
        a.provider_mutation(operator.clone(), rejected_id).await,
        Err(LlmConfigServiceError::NotFound)
    ));
    assert_eq!(
        repo.get_revision("receipt-provider", false)
            .await
            .unwrap()
            .unwrap()
            .revision,
        1
    );
    assert_eq!(
        keys.read("receipt-provider")
            .await
            .unwrap()
            .unwrap()
            .expose_secret(),
        winner
    );
    let client = pool.get().await.unwrap();
    client.batch_execute("DROP TRIGGER reject_provider_receipt ON brassclaw_llm_provider_mutations; DROP FUNCTION reject_provider_receipt()").await.unwrap();
    drop(client);
    // The real reload adapter cannot resolve a foreign invalid role. Publication
    // still has its own original committed receipt, independently readable.
    crate::db_config::save_config_changes(
        &pool,
        tenant,
        &[("llm.default.provider_id", Some("missing-provider"))],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let retry_id = Uuid::new_v4();
    let edit = upsert(retry_id, Some(1), "committed-before-reload-failure");
    assert!(matches!(
        a.upsert_provider(
            operator.clone(),
            serde_json::from_value(edit.clone()).unwrap()
        )
        .await,
        Err(LlmConfigServiceError::Unavailable)
    ));
    let committed = a
        .provider_mutation(operator.clone(), retry_id)
        .await
        .unwrap();
    assert_eq!(committed.provider_revision, Some(1));
    assert_eq!(
        keys.read("receipt-provider")
            .await
            .unwrap()
            .unwrap()
            .expose_secret(),
        "committed-before-reload-failure"
    );
    crate::db_config::save_config_changes(
        &pool,
        tenant,
        &[("llm.default.provider_id", None)],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    assert_eq!(
        a.upsert_provider(operator.clone(), serde_json::from_value(edit).unwrap())
            .await
            .unwrap()
            .mutation_receipt,
        Some(committed.clone())
    );
    let before = keys.read("receipt-provider").await.unwrap().unwrap();
    assert_eq!(
        b.reconcile_provider_mutation(operator.clone(), retry_id)
            .await
            .unwrap()
            .mutation_receipt,
        Some(committed)
    );
    assert_eq!(
        keys.read("receipt-provider")
            .await
            .unwrap()
            .unwrap()
            .expose_secret(),
        before.expose_secret()
    );
    let client = pool.get().await.unwrap();
    assert!(
        client
            .execute(
                "UPDATE brassclaw_llm_provider_mutations SET operation='upsert' WHERE tenant_id=$1",
                &[&tenant]
            )
            .await
            .is_err()
    );
    assert!(
        client
            .execute(
                "DELETE FROM brassclaw_llm_provider_mutations WHERE tenant_id=$1",
                &[&tenant]
            )
            .await
            .is_err()
    );
    assert!(
        client
            .batch_execute("TRUNCATE brassclaw_llm_provider_mutations")
            .await
            .is_err()
    );
}

#[tokio::test]
async fn native_provider_publications_identify_actual_clients_and_serialize_capture() {
    use crate::LlmReloadTrigger as _;
    use brassclaw_llm::{ProviderPublication, SessionManager};
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let pool = rig.pool.clone();
    let tenant = "provider-publication";
    let repo = Arc::new(crate::PgProviderRepo::new(pool.as_ref().clone(), tenant));
    repo.initialize().await.unwrap();
    let keys = crate::LlmKeyStore::new_postgres(Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            pool.as_ref().clone(),
            "provider-publication-authenticated-master-fixture".into(),
            tenant,
        )
        .unwrap(),
    ));
    let primary = Arc::new(SwappableLlmProvider::new(Arc::new(
        crate::runtime::PlaceholderLlmProvider,
    )));
    let sempai = Arc::new(SwappableLlmProvider::new(Arc::new(
        crate::runtime::PlaceholderLlmProvider,
    )));
    let handle = Arc::new(brassclaw_llm::LlmReloadHandle::new(primary.clone(), None));
    let session = Arc::new(SessionManager::new(Default::default()));
    let reload = Arc::new(crate::llm_reload::RebornLlmReloadAdapter::new(
        handle.clone(),
        session.clone(),
        keys.clone(),
        pool.clone(),
        tenant,
    ));
    let api = crate::RebornLlmConfigService::new(keys.clone(), pool.clone(), repo.clone(), tenant)
        .with_reload_trigger(reload.clone())
        .with_sempai_swappable(sempai.clone());
    let operator = caller(tenant);
    let original_id = Uuid::new_v4();
    let mut original = upsert(original_id, None, "publication-key-alpha");
    original["set_active"] = json!(true);
    original["expected_selection_ref"] = json!(
        api.snapshot(operator.clone())
            .await
            .unwrap()
            .role_selection_refs["kohai"]
    );
    let first = api
        .upsert_provider(operator.clone(), serde_json::from_value(original).unwrap())
        .await
        .unwrap();
    let ProviderPublication::Configured {
        selection: first_selection,
    } = &first.runtime_publications["kohai"]
    else {
        panic!("actual configured primary publication required")
    };
    assert_eq!(
        first.desired_publications["kohai"],
        first.runtime_publications["kohai"]
    );
    assert_eq!(
        first.desired_publications["sempai"],
        ProviderPublication::Unconfigured
    );
    assert_eq!(first_selection.definition_revision, 1);
    assert_eq!(first_selection.provider_id, "receipt-provider");
    assert_eq!(first_selection.model, primary.pinned_provider().1);
    let first_client = primary.pinned_provider().0;
    assert_eq!(reload.reload().await.unwrap(), primary.publication());
    assert!(
        Arc::ptr_eq(&first_client, &primary.pinned_provider().0),
        "unchanged reconcile retains cache/circuit/backend"
    );

    // GET remains read-only and available while a key writer owns the row.
    let mut blocked_writer = pool.get().await.unwrap();
    let held = blocked_writer.transaction().await.unwrap();
    assert_eq!(held.query("SELECT scope FROM brassclaw_secrets WHERE tenant_id=$1 AND name='llm_provider_receipt-provider_api_key' FOR UPDATE", &[&tenant]).await.unwrap().len(), 1);
    let read = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        api.snapshot(operator.clone()),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        read.desired_publications["kohai"],
        read.runtime_publications["kohai"]
    );
    held.rollback().await.unwrap();
    drop(blocked_writer);
    // A host key replacement needs no invented provider definition revision.
    keys.put(
        "receipt-provider",
        "publication-key-external-rotation".into(),
    )
    .await
    .unwrap();
    let key_pending = api.snapshot(operator.clone()).await.unwrap();
    assert_ne!(
        key_pending.desired_publications["kohai"],
        key_pending.runtime_publications["kohai"]
    );
    assert_eq!(
        key_pending.runtime_publications["kohai"],
        first.runtime_publications["kohai"]
    );
    reload.reload().await.unwrap();
    let key_applied = api.snapshot(operator.clone()).await.unwrap();
    assert_eq!(
        key_applied.desired_publications["kohai"],
        key_applied.runtime_publications["kohai"]
    );

    let changed_id = Uuid::new_v4();
    let changed = api
        .upsert_provider(
            operator.clone(),
            serde_json::from_value(upsert(changed_id, Some(1), "publication-key-beta")).unwrap(),
        )
        .await
        .unwrap();
    let ProviderPublication::Configured {
        selection: changed_selection,
    } = &changed.runtime_publications["kohai"]
    else {
        panic!("new primary publication required")
    };
    assert_eq!(
        changed_selection.definition_revision, first_selection.definition_revision,
        "key-only publication must not invent a definition revision"
    );
    assert_ne!(
        changed_selection.configuration_ref,
        first_selection.configuration_ref
    );
    assert!(!Arc::ptr_eq(&first_client, &primary.pinned_provider().0));
    let original_receipt = changed.mutation_receipt.clone().unwrap();
    crate::db_config::save_config_changes(
        &pool,
        tenant,
        &[("llm.default.model", Some("external-writer-model"))],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let pending = api.snapshot(operator.clone()).await.unwrap();
    assert_eq!(
        pending.kohai_active.unwrap().model.as_deref(),
        Some("external-writer-model")
    );
    assert_ne!(
        pending.desired_publications["kohai"],
        pending.runtime_publications["kohai"]
    );
    assert_eq!(
        pending.runtime_publications["kohai"], changed.runtime_publications["kohai"],
        "a later desired edit is not an effective acknowledgement"
    );
    let reconciled = api
        .reconcile_provider_mutation(operator.clone(), changed_id)
        .await
        .unwrap();
    assert_eq!(reconciled.mutation_receipt, Some(original_receipt));
    assert_eq!(primary.pinned_provider().1, "external-writer-model");
    assert_eq!(
        reconciled.desired_publications["kohai"],
        reconciled.runtime_publications["kohai"]
    );
    let preserved_primary = primary.pinned_provider().0;
    let preserved_publication = primary.publication();
    let observed = api.snapshot(operator.clone()).await.unwrap();
    let selection = api.set_active(operator.clone(), serde_json::from_value(json!({
        "expected_selection_ref":observed.role_selection_refs["sempai"],
        "expected_revision":observed.providers.iter().find(|row| row.id == "receipt-provider").unwrap().revision,
        "mutation_id":Uuid::new_v4(),"provider_id":"receipt-provider","model":"independent-sempai","role":"sempai"
    })).unwrap()).await.unwrap();
    assert!(Arc::ptr_eq(
        &preserved_primary,
        &primary.pinned_provider().0
    ));
    assert_eq!(
        selection.runtime_publications["kohai"],
        preserved_publication
    );
    let ProviderPublication::Configured {
        selection: sempai_selection,
    } = &selection.runtime_publications["sempai"]
    else {
        panic!("actual Sempai publication required")
    };
    assert_eq!(sempai_selection.model, sempai.pinned_provider().1);
    assert_eq!(sempai_selection.definition_revision, 1);

    // Missing stored credentials fail real factory resolution after capture.
    // Each role records its own exact target without replacing its old client.
    let mut required = repo.get("receipt-provider").await.unwrap().unwrap();
    required.api_key_required = true;
    repo.upsert(required).await.unwrap();
    assert!(keys.delete("receipt-provider").await.unwrap());
    let desired = api.snapshot(operator.clone()).await.unwrap();
    assert!(reload.reload().await.is_err());
    assert!(api.restore_sempai_from_db().await.is_err());
    let failed = api.snapshot(operator.clone()).await.unwrap();
    for role in ["kohai", "sempai"] {
        assert_eq!(
            failed.runtime_publication_failures[role],
            desired.desired_publications[role]
        );
        assert_eq!(
            failed.runtime_publications[role],
            selection.runtime_publications[role]
        );
    }
    assert!(Arc::ptr_eq(
        &preserved_primary,
        &primary.pinned_provider().0
    ));
    crate::db_config::save_config_changes(
        &pool,
        tenant,
        &[("llm.default.model", Some("later-than-failed-attempt"))],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let later = api.snapshot(operator.clone()).await.unwrap();
    assert_ne!(
        later.desired_publications["kohai"], later.runtime_publication_failures["kohai"],
        "a read cannot attribute an older failure to a new desired target"
    );
    assert_eq!(
        later.runtime_publication_failures,
        failed.runtime_publication_failures
    );
    keys.put("receipt-provider", "repaired-publication-credential".into())
        .await
        .unwrap();
    crate::db_config::save_config_changes(
        &pool,
        tenant,
        &[("llm.default.model", Some("external-writer-model"))],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    reload.reload().await.unwrap();
    api.restore_sempai_from_db().await.unwrap();
    let repaired = api.snapshot(operator.clone()).await.unwrap();
    assert!(repaired.runtime_publication_failures.is_empty());
    for role in ["kohai", "sempai"] {
        assert_eq!(
            repaired.runtime_publications[role],
            repaired.desired_publications[role]
        );
    }
    let required_revision = repaired
        .providers
        .iter()
        .find(|row| row.id == "receipt-provider")
        .unwrap()
        .revision;

    // The first actual captured client waits inside the common publication
    // boundary. A second real adapter must capture after it, not build a newer
    // client and then have the first overwrite it on release.
    let captured = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let first = tokio::spawn({
        let (handle, pool, keys, session, captured, release) = (
            handle.clone(),
            pool.clone(),
            keys.clone(),
            session.clone(),
            captured.clone(),
            release.clone(),
        );
        async move {
            handle
                .publish_primary_from(|| async {
                    let selected =
                        crate::llm_catalog::resolve_provider_selection_db(&pool, tenant, &keys)
                            .await
                            .unwrap()
                            .unwrap();
                    let provider = brassclaw_llm::build_static_provider_chain_with_guard(
                        &selected.config,
                        session,
                        selected.guard,
                    )
                    .await?;
                    captured.notify_one();
                    release.notified().await;
                    Ok((
                        provider,
                        ProviderPublication::Configured {
                            selection: selected.revision,
                        },
                    ))
                })
                .await
                .unwrap()
        }
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), captured.notified())
        .await
        .unwrap();
    crate::db_config::save_config_changes(
        &pool,
        tenant,
        &[("llm.default.model", Some("newer-concurrent-model"))],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let mut second = tokio::spawn({
        let reload = reload.clone();
        async move { reload.reload().await.unwrap() }
    });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut second)
            .await
            .is_err()
    );
    release.notify_one();
    let first_ack = first.await.unwrap();
    let second_ack = second.await.unwrap();
    let ProviderPublication::Configured { selection: older } = first_ack else {
        panic!("first exact acknowledgement required")
    };
    assert_eq!(older.model, "external-writer-model");
    let ProviderPublication::Configured { selection: newer } = &second_ack else {
        panic!("second exact acknowledgement required")
    };
    assert_eq!(newer.model, "newer-concurrent-model");
    assert_eq!(primary.publication(), second_ack);
    assert_eq!(primary.pinned_provider().1, "newer-concurrent-model");
    api.delete_provider(
        operator.clone(),
        "receipt-provider".into(),
        required_revision,
        Uuid::new_v4(),
    )
    .await
    .unwrap();
    assert_eq!(primary.publication(), ProviderPublication::Unconfigured);
    assert_eq!(sempai.publication(), ProviderPublication::Unconfigured);
    let disabled = api.snapshot(operator.clone()).await.unwrap();
    assert!(disabled.kohai_active.is_none());
    assert_eq!(
        disabled.desired_publications["kohai"],
        disabled.runtime_publications["kohai"]
    );
    crate::db_config::save_config_changes(
        &pool,
        tenant,
        &[("llm.default.provider_id", Some("external-missing-provider"))],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let invalid = api.snapshot(operator).await.unwrap();
    assert_eq!(
        invalid.kohai_active.unwrap().provider_id,
        "external-missing-provider"
    );
    assert_eq!(
        invalid.desired_publications["kohai"],
        ProviderPublication::Untracked
    );
    assert_eq!(
        invalid.runtime_publications["kohai"],
        ProviderPublication::Unconfigured
    );
    assert!(
        reload.reload().await.is_err(),
        "unknown selection cannot publish an unconfigured/fallback client"
    );
}
