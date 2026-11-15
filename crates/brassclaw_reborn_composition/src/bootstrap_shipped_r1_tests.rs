use super::*;
use brassclaw_pg::test_support::{AckBoundary, AckCutRule, AckCutTransport};
use secrecy::ExposeSecret;

fn packaged_root() -> ComponentRevisionDraft {
    brassclaw_skills::prefix_generation_components::generation_revision_draft(root_id()).unwrap()
}

#[test]
fn shipped_r1_uses_real_product_root_and_keeps_binding_failures_out_of_python_oracles() {
    use brassclaw_skills::value_contract::{InputContract, ValueContract};
    let root = packaged_root();
    let input =
        InputContract::from_value(&root.document()["input_contract"], REVISION_LIMITS).unwrap();
    for invalid in [
        json!({}),
        json!({"revision":""}),
        json!({"revision":null}),
        json!({"revision":true}),
        json!({"revision":[]}),
        json!({"revision":1}),
    ] {
        assert!(input.bind(&invalid).is_err());
    }
    let specs = specs(root.clone()).unwrap();
    // Draft retention validates the envelope, not every executable contract.
    // Check every actual candidate/instrument schema before native execution;
    // missing explicit checks must fail this short test, not cold startup.
    for draft in specs
        .iter()
        .flat_map(|spec| std::iter::once(&spec.candidate).chain(spec.dependencies.iter()))
    {
        InputContract::from_value(&draft.document()["input_contract"], draft.parsing_limits())
            .unwrap();
        ValueContract::result_from_value(
            &draft.document()["result_contract"],
            draft.parsing_limits(),
        )
        .unwrap();
    }
    assert_eq!(specs.len(), 2);
    assert_eq!(specs[1].candidate.exact_bytes(), root.exact_bytes());
    assert_eq!(specs[0].suites.len(), 4);
    assert_eq!(specs[1].suites.len(), 14);
    for (index, suite) in specs[1].suites.iter().enumerate() {
        let normalized = fixtures::normalize_suite(index, suite).unwrap();
        if suite.python_stop {
            assert_eq!(normalized.steps.len(), 1);
            assert!(
                normalized
                    .expected_python_line
                    .as_deref()
                    .unwrap()
                    .starts_with("ValueError: ")
            );
        } else {
            let producer = format!("{index}:1");
            assert_eq!(normalized.steps[1].constants["step_id"], producer);
            assert_eq!(normalized.steps[1].results["value"], producer);
            assert_eq!(
                specs[1].recipe.document()["input_layouts"][&suite.variant]["steps"]
                    [format!("{index}:2")]["step_id"]["value"],
                producer
            );
        }
    }
    for (index, suite) in specs[0].suites.iter().enumerate().take(2) {
        assert_eq!(
            fixtures::normalize_suite(index, suite).unwrap().steps[0].constants["step_id"],
            "2:7"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_shipped_r1_qualifies_exact_generation_revision_and_reuses_without_execution() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _database = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let home = tempfile::tempdir().unwrap();
    let runtime = Box::pin(crate::shipped_validation::qualify_package(
        crate::build_reborn_runtime(crate::RebornRuntimeInput::from_services(
            rig.build_input("shipped-r1-native", home.path())
                .with_runtime_policy(crate::local_dev_runtime_policy().unwrap()),
        )),
    ))
    .await
    .unwrap();
    let catalogue = &runtime.installed_catalogue;
    let base = SessionInput {
        pool: rig.pool.clone(),
        worker: catalogue.worker_image.clone(),
        root: catalogue.root_definition.clone(),
        executor: catalogue.startup_executor.clone(),
        preparation: catalogue.capture_task_preparation().await.unwrap(),
        storage: catalogue.resource_store_settings.clone(),
        application_checksum: hex::encode(catalogue._image.checksum()),
    };
    let count = || async {
        rig.pool
            .get()
            .await
            .unwrap()
            .query_one("SELECT count(*) FROM brassclaw_monty_task_admissions", &[])
            .await
            .unwrap()
            .get::<_, i64>(0)
    };
    let before = count().await;
    let contexts = qualify_specs(base.clone(), specs(packaged_root()).unwrap())
        .await
        .unwrap();
    assert_eq!(
        count().await,
        before,
        "completed original must not execute the business export again"
    );
    let mut client = rig.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    let instrument = &contexts[0];
    let root = &contexts[1];
    assert_eq!(root.claim.root.uuid, root_id());
    assert_eq!(
        root.profile
            .as_ref()
            .unwrap()
            .components(&root.claim)
            .unwrap()
            .as_slice(),
        std::slice::from_ref(&root.claim.root)
    );
    assert!(
        root.profile.as_ref().unwrap().prerequisites
            == [instrument.verify_final(&tx).await.unwrap()]
    );
    let proof = root.verify_approval(&tx).await.unwrap();
    assert_eq!(proof.id, root.id("approval"));
    let row=tx.query_one("SELECT validation_mode,q2_ref,approval_bytes,(SELECT count(*) FROM reborn_pure_component_approval_closure WHERE approval_id=$1) FROM reborn_pure_component_approvals WHERE approval_id=$1",&[&proof.id]).await.unwrap();
    assert_eq!(row.get::<_, String>(0), "system_seed");
    assert_eq!(row.get::<_, Option<Uuid>>(1), None);
    assert_eq!(row.get::<_, String>(2), root.approval_bytes().unwrap());
    assert_eq!(row.get::<_, i64>(3), 1);
    assert!(
        !tx.query_one(
            "SELECT EXISTS(SELECT 1 FROM reborn_pure_component_approvals WHERE root_id=$1)",
            &[&integer_instrument_id()]
        )
        .await
        .unwrap()
        .get::<_, bool>(0)
    );
    for context in &contexts {
        assert_eq!(context.claim.format, "shipped-pure-r1-execution-context/1");
        assert_eq!(context.claim.scope, "installation-shipped-pure-r1-fixture");
        let qualified = context.qualified(&tx).await.unwrap();
        assert_eq!(qualified.scope, "installation-shipped-pure-r1-only");
        assert_eq!(qualified.completed.len(), context.claim.suites.len());
        assert_eq!(
            context.claim.selection_parser_limits.len(),
            context.claim.suites.len()
        );
    }
    tx.commit().await.unwrap();
    drop(client);
    let changed_root = {
        let original = packaged_root();
        let mut changed: Value = serde_json::from_str(original.exact_bytes()).unwrap();
        changed["document"]["content"] = json!(format!(
            "{}\n# independently changed candidate\n",
            original.document()["content"].as_str().unwrap()
        ));
        ComponentRevisionDraft::from_json(&changed.to_string()).unwrap()
    };
    assert_ne!(changed_root.checksum(), packaged_root().checksum());
    assert!(
        specs(changed_root.clone()).is_err(),
        "an edited installed subject cannot select shipped provenance"
    );
    assert!(rig.pool.get().await.unwrap().query_opt("SELECT approval_id FROM reborn_pure_component_approvals WHERE root_id=$1 AND root_checksum=$2",&[&root_id(),&hex::encode(changed_root.checksum())]).await.unwrap().is_none());
    assert_eq!(count().await, before);
    runtime.shutdown().await.unwrap();
}

fn proxy_input(transport: &AckCutTransport, home: &std::path::Path) -> crate::RebornRuntimeInput {
    crate::RebornRuntimeInput::from_services(
        crate::RebornBuildInput::postgres_with_reborn_home(
            "shipped-r1-ack-native",
            (*transport.pool).clone(),
            brassclaw_secrets::SecretMaterial::from(transport.connection_string().to_owned()),
            home.to_owned(),
        )
        .with_runtime_policy(crate::local_dev_runtime_policy().unwrap()),
    )
}
async fn admission_count(pool: &PgPool) -> i64 {
    pool.get()
        .await
        .unwrap()
        .query_one("SELECT count(*) FROM brassclaw_monty_task_admissions", &[])
        .await
        .unwrap()
        .get(0)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_shipped_r1_completion_and_approval_ack_loss_resolve_originals_without_replay() {
    for stage in ["shipped-pure-r1-completed/1", "pure-component-approval/1"] {
        native_ack_case(stage).await;
    }
}
async fn native_ack_case(stage: &str) {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _database = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let transport = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::after_bind(
            1,
            vec![
                stage.as_bytes().to_vec(),
                root_id().to_string().into_bytes(),
            ],
            vec![],
            AckBoundary::Commit,
        )],
    )
    .await
    .unwrap();
    transport.arm(1).unwrap();
    let home = tempfile::tempdir().unwrap();
    let runtime = Box::pin(crate::shipped_validation::qualify_package(
        crate::build_reborn_runtime(proxy_input(&transport, home.path())),
    ))
    .await
    .unwrap();
    assert_eq!(
        transport.cut_count(),
        1,
        "the selected real COMMIT response must actually be lost"
    );
    let before = admission_count(&transport.pool).await;
    let catalogue = &runtime.installed_catalogue;
    let base = SessionInput {
        pool: transport.pool.clone(),
        worker: catalogue.worker_image.clone(),
        root: catalogue.root_definition.clone(),
        executor: catalogue.startup_executor.clone(),
        preparation: catalogue.capture_task_preparation().await.unwrap(),
        storage: catalogue.resource_store_settings.clone(),
        application_checksum: hex::encode(catalogue._image.checksum()),
    };
    let contexts = qualify_specs(base, specs(packaged_root()).unwrap())
        .await
        .unwrap();
    let mut client = io::acquire(&transport.pool).await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    contexts[1].verify_approval(&tx).await.unwrap();
    tx.commit().await.unwrap();
    client.recycle_after_commit();
    drop(client);
    assert_eq!(admission_count(&transport.pool).await, before);
    assert_eq!(transport.cut_count(), 1);
    runtime.shutdown().await.unwrap();
    transport.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_shipped_r1_unknown_intent_never_executes_or_replays_the_business_root() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _database = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let transport = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::after_bind(
            1,
            vec![
                b"shipped-pure-r1-operation/1".to_vec(),
                root_id().to_string().into_bytes(),
            ],
            vec![],
            AckBoundary::Commit,
        )],
    )
    .await
    .unwrap();
    transport.arm(1).unwrap();
    let home = tempfile::tempdir().unwrap();
    assert!(
        Box::pin(crate::shipped_validation::qualify_package(
            crate::build_reborn_runtime(proxy_input(&transport, home.path()))
        ))
        .await
        .is_err()
    );
    assert_eq!(transport.cut_count(), 1);
    let before = admission_count(&transport.pool).await;
    let originals = || async {
        let row=rig.pool.get().await.unwrap().query_one(
            "SELECT (SELECT count(*) FROM reborn_component_review_evidence WHERE evidence_bytes::jsonb #>> '{report,format}'='shipped-pure-r1-operation/1' AND evidence_bytes::jsonb #>> '{components,0,uuid}'=$1), (SELECT count(*) FROM reborn_component_review_evidence WHERE evidence_bytes::jsonb #>> '{report,format}'='shipped-pure-r1-source/1' AND evidence_bytes::jsonb #>> '{report,root,uuid}'=$1), (SELECT count(*) FROM reborn_pure_component_approvals WHERE root_id=$2)",
            &[&root_id().to_string(),&root_id()],
        ).await.unwrap();
        (
            row.get::<_, i64>(0),
            row.get::<_, i64>(1),
            row.get::<_, i64>(2),
        )
    };
    assert_eq!(
        originals().await,
        (1, 0, 0),
        "intent exists, but no business Source or approval can be fabricated"
    );
    assert!(
        Box::pin(crate::shipped_validation::qualify_package(
            crate::build_reborn_runtime(proxy_input(&transport, home.path()))
        ))
        .await
        .is_err()
    );
    assert_eq!(
        admission_count(&transport.pool).await,
        before,
        "an existing incomplete intent never starts the root fixtures"
    );
    assert_eq!(originals().await, (1, 0, 0));
    assert_eq!(transport.cut_count(), 1);
    transport.shutdown().await.unwrap();
}

#[test]
fn shipped_history_uses_canonical_typed_formatter_and_literal_oracles() {
    use brassclaw_skills::value_contract::{InputContract, ValueContract};
    let id = ShippedFunction::History.root_id();
    let root = brassclaw_skills::global_bootstrap_components::history_formatter_draft(id).unwrap();
    let contract =
        InputContract::from_value(&root.document()["input_contract"], root.parsing_limits())
            .unwrap();
    for invalid in [
        json!({}),
        json!({"user_input":null,"answer":"a","reply_ref":"r"}),
        json!({"user_input":"u","answer":false,"reply_ref":"r"}),
        json!({"user_input":"u","answer":"a","reply_ref":[]}),
    ] {
        assert!(contract.bind(&invalid).is_err());
    }
    let specs = history_specs(root.clone()).unwrap();
    assert_eq!(specs.len(), 2);
    assert_eq!(specs[1].candidate.exact_bytes(), root.exact_bytes());
    assert_eq!(specs[0].suites.len(), 4);
    assert_eq!(specs[1].suites.len(), 4);
    assert_eq!(specs[0].candidate.uuid(), text_instrument_id());
    for spec in &specs {
        for draft in std::iter::once(&spec.candidate).chain(spec.dependencies.iter()) {
            InputContract::from_value(&draft.document()["input_contract"], draft.parsing_limits())
                .unwrap();
            ValueContract::result_from_value(
                &draft.document()["result_contract"],
                draft.parsing_limits(),
            )
            .unwrap();
        }
        for (index, suite) in spec.suites.iter().enumerate() {
            let normalized = fixtures::normalize_suite(index, suite).unwrap();
            if !spec.profile.as_ref().unwrap().approves_function() || suite.python_stop {
                continue;
            }
            assert_eq!(normalized.steps[1].results["value"], format!("{index}:1"));
            assert_eq!(
                normalized.steps[1].constants["step_id"],
                format!("{index}:1")
            );
        }
    }
    let suite = &specs[1].suites[3];
    let input = suite.task_input().unwrap();
    assert_eq!(input["user_input"], "\u{0000}\r\n\\");
    let stored = serde_json::to_value(suite).unwrap();
    // A JSONB string contains JSON source text, not a decoded NUL. The
    // runtime input is decoded only after the immutable fixture is selected.
    assert!(
        !stored["runtime_input"]["json"]
            .as_str()
            .unwrap()
            .contains('\0')
    );
    let restored: Suite = serde_json::from_value(stored).unwrap();
    assert_eq!(restored.task_input().unwrap(), input);
    let layout = &specs[1].recipe.document()["input_layouts"][&suite.variant];
    assert_eq!(layout["task_inputs"], root.document()["input_contract"]);
    assert_eq!(
        layout["steps"]["3:1"]["user_input"],
        json!({"kind":"task_input","reference":"{{vars.user_input}}"})
    );
    assert!(restored.steps[0].constants.as_object().unwrap().is_empty());
    let legacy = serde_json::to_value(&specs[0].suites[0]).unwrap();
    assert!(legacy.get("runtime_input").is_none());
    let mut malformed = restored;
    malformed.runtime_input.as_mut().unwrap().json =
        "{\"user_input\":\"a\",\"user_input\":\"b\"}".into();
    assert!(malformed.task_input().is_err());
    let mut changed: Value = serde_json::from_str(root.exact_bytes()).unwrap();
    changed["document"]["content"] =
        json!("def format_turn_history(inputs):\n    return 'not the packaged formatter'\n");
    assert!(
        history_specs(ComponentRevisionDraft::from_json(&changed.to_string()).unwrap()).is_err()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_shipped_history_qualifies_actual_formatter_and_reuses_originals() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _database = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let home = tempfile::tempdir().unwrap();
    let runtime = Box::pin(crate::shipped_validation::qualify_package(
        crate::build_reborn_runtime(crate::RebornRuntimeInput::from_services(
            rig.build_input("shipped-history-native", home.path())
                .with_runtime_policy(crate::local_dev_runtime_policy().unwrap()),
        )),
    ))
    .await
    .unwrap();
    let catalogue = &runtime.installed_catalogue;
    let id = ShippedFunction::History.root_id();
    let selected = catalogue
        .history
        .program()
        .inputs()
        .instruction()
        .snapshot()
        .revisions()
        .get(&id)
        .unwrap();
    let expected = selected.reference();
    let candidate = selected.draft().clone();
    assert_eq!(
        candidate.exact_bytes(),
        brassclaw_skills::global_bootstrap_components::history_formatter_draft(id)
            .unwrap()
            .exact_bytes()
    );
    let base = SessionInput {
        pool: rig.pool.clone(),
        worker: catalogue.worker_image.clone(),
        root: catalogue.root_definition.clone(),
        executor: catalogue.startup_executor.clone(),
        preparation: catalogue.capture_task_preparation().await.unwrap(),
        storage: catalogue.resource_store_settings.clone(),
        application_checksum: hex::encode(catalogue._image.checksum()),
    };
    let before = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT count(*) FROM brassclaw_monty_task_admissions", &[])
        .await
        .unwrap()
        .get::<_, i64>(0);
    let checked = qualify(base.clone(), &catalogue.history, ShippedFunction::History)
        .await
        .unwrap();
    let contexts = qualify_specs(base, history_specs(candidate).unwrap())
        .await
        .unwrap();
    let after = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT count(*) FROM brassclaw_monty_task_admissions", &[])
        .await
        .unwrap()
        .get::<_, i64>(0);
    assert_eq!(after, before, "verified originals must not execute again");
    let mut client = rig.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    checked.verify(&tx, &catalogue.history).await.unwrap();
    let instrument = &contexts[0];
    let function = &contexts[1];
    assert_eq!(function.claim.root, Reference::from(expected));
    assert!(!instrument.profile.as_ref().unwrap().approves_function());
    assert_eq!(
        function
            .profile
            .as_ref()
            .unwrap()
            .components(&function.claim)
            .unwrap(),
        vec![function.claim.root.clone()]
    );
    assert!(
        function.profile.as_ref().unwrap().prerequisites
            == [instrument.verify_final(&tx).await.unwrap()]
    );
    let approval = function.verify_approval(&tx).await.unwrap();
    let row=tx.query_one("SELECT validation_mode,q2_ref,approval_bytes,(SELECT count(*) FROM reborn_pure_component_approval_closure WHERE approval_id=$1) FROM reborn_pure_component_approvals WHERE approval_id=$1",&[&approval.id]).await.unwrap();
    assert_eq!(row.get::<_, String>(0), "system_seed");
    assert_eq!(row.get::<_, Option<Uuid>>(1), None);
    assert_eq!(row.get::<_, String>(2), function.approval_bytes().unwrap());
    assert_eq!(row.get::<_, i64>(3), 1);
    assert!(
        !tx.query_one(
            "SELECT EXISTS(SELECT 1 FROM reborn_pure_component_approvals WHERE root_id=$1)",
            &[&text_instrument_id()]
        )
        .await
        .unwrap()
        .get::<_, bool>(0)
    );
    tx.commit().await.unwrap();
    drop(client);
    runtime.shutdown().await.unwrap();
}
