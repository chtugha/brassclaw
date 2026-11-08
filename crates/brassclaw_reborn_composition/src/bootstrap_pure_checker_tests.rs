//! Real PostgreSQL/worker acceptance, to be executed by the supervisor.
use super::*;
use secrecy::ExposeSecret;

async fn read_proof(
    runtime: &crate::RebornRuntime,
    pool: Arc<PgPool>,
) -> CheckedPureCheckerClosure {
    let c = &runtime.installed_catalogue;
    let preparation = c.capture_task_preparation().await.unwrap();
    let structural = crate::bootstrap_validator_approval::qualify(
        pool.clone(),
        c.worker_image.clone(),
        c.root_definition.clone(),
        c.startup_executor.clone(),
        preparation.clone(),
        c.resource_store_settings.clone(),
        hex::encode(c._image.checksum()),
    )
    .await
    .unwrap();
    qualify(
        SessionInput {
            pool,
            worker: c.worker_image.clone(),
            root: c.root_definition.clone(),
            executor: c.startup_executor.clone(),
            preparation,
            storage: c.resource_store_settings.clone(),
            application_checksum: hex::encode(c._image.checksum()),
        },
        &structural,
    )
    .await
    .unwrap()
}
async fn counts(pool: &PgPool) -> (i64, i64, i64, i64) {
    let client = pool.get().await.unwrap();
    let row=client.query_one("SELECT (SELECT count(*) FROM brassclaw_monty_task_admissions),(SELECT count(*) FROM brassclaw_monty_tool_invocations),(SELECT count(*) FROM reborn_component_review_submissions),(SELECT count(*) FROM reborn_component_review_requests)",&[]).await.unwrap();
    (row.get(0), row.get(1), row.get(2), row.get(3))
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_pure_checker_subjects_keep_private_tuple_returns_and_original_typed_fingerprints() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _database = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let home = tempfile::tempdir().unwrap();
    let runtime = Box::pin(crate::build_reborn_runtime(
        crate::RebornRuntimeInput::from_services(
            rig.build_input("pure-checker-native", home.path())
                .with_runtime_policy(crate::local_dev_runtime_policy().unwrap()),
        ),
    ))
    .await
    .unwrap();
    let before = counts(&rig.pool).await;
    let proof = read_proof(&runtime, rig.pool.clone()).await;
    let mut client = rig.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    proof.verify_in_transaction(&tx).await.unwrap();
    assert_eq!(proof.contexts.len(), 12);
    for context in &proof.contexts {
        let profile = context.profile.as_ref().unwrap();
        let is_function = matches!(profile.scope, Scope::Function);
        assert_eq!(
            tx.query_one(
                "SELECT EXISTS(SELECT 1 FROM reborn_pure_component_approvals WHERE approval_id=$1)",
                &[&context.id("approval")]
            )
            .await
            .unwrap()
            .get::<_, bool>(0),
            is_function
        );
        if !is_function {
            assert!(context.approval_bytes().is_err());
        }
        for suite in &context.claim.suites {
            let (_, mut observed) = context
                .read::<Observed>(
                    &tx,
                    context.id(&format!("{}:observed", suite.variant)),
                    "behavior",
                    false,
                )
                .await
                .unwrap()
                .unwrap();
            context.verify_observed(suite, &observed).unwrap();
            observed.steps[0].input_checksum = "00".repeat(32);
            assert!(
                context.verify_observed(suite, &observed).is_err(),
                "foreign input with identical output must fail"
            );
            if !suite.python_stop {
                let (_, mut altered) = context
                    .read::<Observed>(
                        &tx,
                        context.id(&format!("{}:observed", suite.variant)),
                        "behavior",
                        false,
                    )
                    .await
                    .unwrap()
                    .unwrap();
                altered.steps[0].result_checksum = Some("00".repeat(32));
                assert!(
                    context.verify_observed(suite, &altered).is_err(),
                    "receipt cannot replace the accepted native value"
                );
            }
        }
    }
    let reader = &proof.contexts[1];
    let tuple = reader
        .claim
        .suites
        .iter()
        .find(|s| s.variant == "subject_tuple")
        .unwrap();
    let representation = &tuple.expected_receipt.as_ref().unwrap()["return"];
    assert_eq!(representation["native_kind"], "tuple2");
    assert_eq!(representation["first_kind"], "dict");
    assert_eq!(representation["second_kind"], "list");
    assert!(
        proof.contexts[3].claim.suites.len() > 140,
        "complete recursive boundary gallery is mandatory"
    );
    assert_eq!(tx.query_one("SELECT count(*) FROM reborn_component_review_evidence WHERE evidence_bytes::jsonb #>> '{report,format}'='pure-checker-workflow-report/1'",&[]).await.unwrap().get::<_,i64>(0),1);
    tx.commit().await.unwrap();
    drop(client);
    assert_eq!(
        before,
        counts(&rig.pool).await,
        "reading completed qualification cannot create admissions or authored review work"
    );

    let settings = runtime.webui_monty_settings_store();
    let current = settings.get("default", "default").await.unwrap();
    let mut smaller = current.execution_limits;
    let original = u64::try_from(proof.contexts[3].claim_bytes().unwrap().len()).unwrap();
    smaller.max_value_bytes = smaller.max_value_bytes.min(original / 2);
    assert!(smaller.max_value_bytes < original);
    smaller.validate().unwrap();
    settings
        .upsert(
            "default",
            "default",
            &serde_json::from_value(
                json!({"expected_revision":current.revision,"execution_limits":smaller}),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let pool =
        Arc::new(brassclaw_pg::pool::build_pool_with_max_size(rig.url.expose_secret(), 1).unwrap());
    let reread = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        read_proof(&runtime, pool.clone()),
    )
    .await
    .expect("original readback releases pool1 between evidence transactions");
    tokio::time::timeout(
        std::time::Duration::from_secs(60),
        crate::validator_inventory_review::assert_original_pure_workflow_readback(
            &runtime.installed_catalogue,
            pool.clone(),
            &reread,
        ),
    )
    .await
    .expect("whole W original readback uses pool1 without native replay");
    for (old, new) in proof.contexts.iter().zip(&reread.contexts) {
        assert_eq!(old.operation_bytes, new.operation_bytes);
        assert!(old.reservation == new.reservation);
    }
    assert_eq!(
        before,
        counts(&rig.pool).await,
        "smaller acknowledged limits do not change original fixtures or replay checkers"
    );
    pool.close();
    drop(pool);
    drop(reread);
    drop(proof);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn native_pure_fixture_transport_rejects_non_json_returns_before_encoding() {
    use brassclaw_monty_host::{
        process::installed_worker,
        utility::{self, UtilityLimits, UtilityRequest},
    };
    use brassclaw_skills::value_contract::{InputContract, ValueContract};
    let worker = installed_worker().unwrap();
    let limits = UtilityLimits::from_execution(Default::default()).unwrap();
    for source in ["(1, 2)", "b'bytes'", "float('inf')"] {
        let error = utility::execute(
            &worker,
            UtilityRequest::Evaluate {
                source: source.into(),
                inputs: BTreeMap::new(),
                bounds: limits.bounds,
                max_compute_time: std::time::Duration::from_secs(10),
            },
            limits.process,
        )
        .await
        .expect_err("native non-JSON return must fail before an encoder can see it");
        assert_eq!(error.kind, ProcessFailure::Vm(VmFailure::InvalidResult));
        let UtilityRequest::Evaluate {
            source: original, ..
        } = *error.request
        else {
            panic!("original native request required")
        };
        assert_eq!(original, source);
        assert!(error.containment_error.is_none() && error.reap_error.is_none());
    }
    let integer =
        ValueContract::result_from_value(&json!({"type":"integer"}), REVISION_LIMITS).unwrap();
    assert!(integer.validate(&json!(true)).is_err());
    assert!(integer.validate(&json!(1)).is_ok());
    let inputs = InputContract::from_value(
        &json!({"value":{"type":"integer","required":false,"default":1}}),
        REVISION_LIMITS,
    )
    .unwrap();
    assert_eq!(
        inputs.bind(&json!({})).unwrap().get("value"),
        Some(&json!(1))
    );
    assert!(inputs.bind(&json!({"value":null})).is_err());
    assert!(inputs.bind(&json!({"value":true})).is_err());
}
