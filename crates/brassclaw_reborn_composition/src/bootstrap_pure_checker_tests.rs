//! Real PostgreSQL/worker acceptance, to be executed by the supervisor.
use super::*;
use secrecy::ExposeSecret;

#[test]
fn pure_fixture_receipt_key_matches_variant_producer_and_manifest() {
    use super::super::fixtures::{FixtureStep, Suite, checker_suites, normalize_suite, recipe_at};
    use crate::pg_review_admission_source::{ValidatorReceiptProfile, recorded_receipt_results};
    let root = brassclaw_skills::validator_components::structural_validator_pc_id(22);
    // Reuse a real shipped positive producer fixture; only its transport is the
    // subject of this test. No fabricated native observation or approval exists.
    let producer = checker_suites(22, root)[0].steps[0].clone();
    let suite = |name: &str| Suite {
        variant: name.into(),
        steps: vec![
            producer.clone(),
            FixtureStep {
                id: "0:2".into(),
                component: encoder_id(),
                export: "encode_pure_fixture_result".into(),
                constants: json!({"kind":"rule","step_id":"0:1"}),
                results: BTreeMap::from([("rule".into(), "0:1".into())]),
            },
        ],
        expected_receipt: None,
        python_stop: false,
        expected_python_line: None,
    };
    let suites = [suite("first"), suite("second")];
    let recipe = recipe_at(
        packaged_component_id("validator:pure-qualification:test-step-identity"),
        root,
        &suites,
    )
    .unwrap();
    let document = recipe.document();
    assert_eq!(document["variants"][1]["step_link"], "1:1-1:E");
    let bindings = &document["input_layouts"]["second"]["steps"]["1:2"];
    let producer_id = bindings["rule"]["step_id"].as_str().unwrap();
    let receipt_key = bindings["step_id"]["value"].as_str().unwrap();
    assert_eq!(producer_id, "1:1");
    assert_eq!(
        receipt_key, producer_id,
        "actual encoder input must name its actual producer"
    );
    let manifest = normalize_suite(1, &suites[1]).unwrap();
    assert_eq!(manifest.steps[0].id, producer_id);
    assert_eq!(manifest.steps[1].results["rule"], producer_id);
    assert_eq!(manifest.steps[1].constants["step_id"], receipt_key);
    let rule = json!({"rule_id":"structural_presence","ok":true,"errors":[]});
    let receipt =
        json!({"format":"validator-step-results/1","results":{receipt_key:rule}}).to_string();
    let actual = recorded_receipt_results(
        ValidatorReceiptProfile::Inventory,
        &receipt,
        &[producer_id],
        &manifest.steps[1].id,
        REVISION_LIMITS,
    )
    .unwrap();
    assert_eq!(actual[producer_id], rule);
    let foreign = json!({"format":"validator-step-results/1","results":{"0:1":rule}}).to_string();
    assert_eq!(
        recorded_receipt_results(
            ValidatorReceiptProfile::Inventory,
            &foreign,
            &[producer_id],
            &manifest.steps[1].id,
            REVISION_LIMITS
        )
        .unwrap_err(),
        "validator_receipt_steps_invalid"
    );

    // A direct encoder test supplies a literal key rather than a result edge.
    // Namespace conversion must leave that actual typed input untouched.
    let direct = |name: &str| Suite {
        variant: name.into(),
        steps: vec![FixtureStep {
            id: "0:1".into(),
            component: encoder_id(),
            export: "encode_pure_fixture_result".into(),
            constants: json!({"kind":"text","step_id":"fixture:1","text":"data"}),
            results: BTreeMap::new(),
        }],
        expected_receipt: Some(
            json!({"format":"validator-step-results/1","results":{"fixture:1":"data"}}),
        ),
        python_stop: false,
        expected_python_line: None,
    };
    let direct = [direct("first"), direct("second")];
    let recipe = recipe_at(
        packaged_component_id("validator:pure-qualification:test-literal-key"),
        encoder_id(),
        &direct,
    )
    .unwrap();
    assert_eq!(
        recipe.document()["input_layouts"]["second"]["steps"]["1:1"]["step_id"]["value"],
        "fixture:1"
    );
    assert_eq!(
        normalize_suite(1, &direct[1]).unwrap().steps[0].constants["step_id"],
        "fixture:1"
    );
}

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

// The historical codec is deliberately fixed here independently of the scoped
// builder. Struct order preserves its original outer bytes, not just JSON meaning.
#[derive(Serialize)]
struct OriginalStructuralSubject {
    format: &'static str,
    subject_id: Uuid,
    package_selector: &'static str,
    candidate: Value,
    dependencies: Value,
    base_comparison: Value,
    package_context: Value,
    q1_qualification: bool,
    semantic_approval: bool,
    catalogue_activation: bool,
}
#[derive(Serialize)]
struct OriginalStructuralEnvelope {
    format: &'static str,
    evidence_id: Uuid,
    kind: &'static str,
    validation_mode: &'static str,
    association_checksum: String,
    components: Vec<OriginalStructuralReference>,
    succeeded: bool,
    reviewed_evidence: Vec<Value>,
    report: Value,
}
#[derive(Serialize, Deserialize)]
struct OriginalStructuralReference {
    uuid: Uuid,
    class_code: i32,
    version: u64,
    checksum: String,
}

async fn assert_installation_subject_store_boundary(pool: &PgPool) {
    use brassclaw_skills::review_submission_store::installation as store;
    let mut client = pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let mut originals = Vec::new();
    let mut negative_ids = Vec::new();
    for (format, selector) in [
        (
            "installation-validator-subject-record/1",
            "validator:structural-presence",
        ),
        (
            "installation-pure-fixture-subject/1",
            "validator:pure-qualification:workflow",
        ),
    ] {
        let row = tx
            .query_one(
                "SELECT evidence_id,evidence_bytes,checksum FROM reborn_component_review_evidence
             WHERE evidence_bytes::jsonb #>> '{report,format}'=$1
             AND evidence_bytes::jsonb #>> '{report,package_selector}'=$2
             ORDER BY evidence_id LIMIT 1",
                &[&format, &selector],
            )
            .await
            .unwrap();
        let id: Uuid = row.get(0);
        let bytes: String = row.get(1);
        let checksum: String = row.get(2);
        let original = strict_json(&bytes, REVISION_LIMITS).unwrap();
        let read = store::read_subject_in_transaction(&tx, id, REVISION_LIMITS)
            .await
            .unwrap()
            .expect("each actual allowed original pair must read back");
        assert_eq!(read.reference().id, id);
        assert_eq!(read.reference().checksum, checksum);
        assert_eq!(read.value()["subject_bytes"], bytes);
        assert_eq!(evidence::checksum(&bytes), checksum);
        if format == "installation-validator-subject-record/1" {
            let subject = &original["report"];
            let candidate_id =
                Uuid::parse_str(subject["candidate"]["reference"]["uuid"].as_str().unwrap())
                    .unwrap();
            let identity = json!({"format":"installation-validator-subject-identity/1",
                "candidate":subject["candidate"]["reference"],"components":original["components"],
                "package_context":subject["package_context"]})
            .to_string();
            let historical_id = Uuid::new_v5(&candidate_id, identity.as_bytes());
            assert_eq!(
                id, historical_id,
                "the old identity contains no fixture scope"
            );
            let report = OriginalStructuralSubject {
                format: "installation-validator-subject-record/1",
                subject_id: historical_id,
                package_selector: "validator:structural-presence",
                candidate: subject["candidate"].clone(),
                dependencies: subject["dependencies"].clone(),
                base_comparison: Value::Null,
                package_context: subject["package_context"].clone(),
                q1_qualification: false,
                semantic_approval: false,
                catalogue_activation: false,
            };
            let historical = OriginalStructuralEnvelope {
                format: "component-review-evidence/1",
                evidence_id: historical_id,
                kind: "behavior",
                validation_mode: "system_seed",
                association_checksum: subject["package_context"]["association_checksum"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
                components: serde_json::from_value(original["components"].clone()).unwrap(),
                succeeded: false,
                reviewed_evidence: vec![],
                report: serde_json::to_value(report).unwrap(),
            };
            assert_eq!(
                serde_json::to_string(&historical).unwrap(),
                bytes,
                "the existing structural codec's full bytes remain unchanged"
            );
        }
        let other_format = if format == "installation-validator-subject-record/1" {
            "installation-pure-fixture-subject/1"
        } else {
            "installation-validator-subject-record/1"
        };
        let other_selector = if selector == "validator:structural-presence" {
            "validator:pure-qualification:workflow"
        } else {
            "validator:structural-presence"
        };
        for (case, field, value) in [
            ("crossed_format", "format", json!(other_format)),
            (
                "crossed_selector",
                "package_selector",
                json!(other_selector),
            ),
            ("positive_q1", "q1_qualification", json!(true)),
            ("positive_semantics", "semantic_approval", json!(true)),
            ("positive_activation", "catalogue_activation", json!(true)),
            ("positive_envelope", "succeeded", json!(true)),
        ] {
            let negative_id = Uuid::new_v5(&id, format!("f01-store-boundary:{case}").as_bytes());
            let mut negative = original.clone();
            negative["evidence_id"] = json!(negative_id);
            negative["report"]["subject_id"] = json!(negative_id);
            if field == "succeeded" {
                negative[field] = value;
            } else {
                negative["report"][field] = value;
            }
            let negative_bytes = negative.to_string();
            assert_eq!(tx.execute(
                "INSERT INTO reborn_component_review_evidence(evidence_id,evidence_bytes,checksum) VALUES($1,$2,$3)",
                &[&negative_id, &negative_bytes, &evidence::checksum(&negative_bytes)],
            ).await.unwrap(), 1);
            assert!(
                matches!(
                    store::read_subject_in_transaction(&tx, negative_id, REVISION_LIMITS).await,
                    Err(brassclaw_skills::review_submission_store::SubmissionStoreError::Invalid)
                ),
                "actual Store reader must reject {format}/{case} as invalid data"
            );
            negative_ids.push(negative_id);
        }
        originals.push((id, bytes, checksum));
    }
    assert_ne!(originals[0].0, originals[1].0);
    // Negative fixtures have new IDs and exist only inside this test transaction.
    // Never update, delete or commit a change to an immutable original.
    tx.rollback().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    for (id, bytes, checksum) in originals {
        let row = tx.query_one("SELECT evidence_bytes,checksum FROM reborn_component_review_evidence WHERE evidence_id=$1", &[&id]).await.unwrap();
        assert_eq!(row.get::<_, String>(0), bytes);
        assert_eq!(row.get::<_, String>(1), checksum);
        assert!(
            store::read_subject_in_transaction(&tx, id, REVISION_LIMITS)
                .await
                .unwrap()
                .is_some()
        );
    }
    assert_eq!(tx.query_one("SELECT count(*) FROM reborn_component_review_evidence WHERE evidence_id=ANY($1::uuid[])", &[&negative_ids]).await.unwrap().get::<_,i64>(0),0);
    tx.commit().await.unwrap();
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
    assert_installation_subject_store_boundary(&rig.pool).await;
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
