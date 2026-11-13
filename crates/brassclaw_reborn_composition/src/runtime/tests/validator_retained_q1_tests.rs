//! Integration test: end-to-end `run_q1_validation_retained` pass (PKG-C Step 6).
//!
//! Verifies the complete retained Q1 path:
//! 1. Structural validator Recipe drafts are seeded.
//! 2. A class-22 component is submitted to review via `PgReviewSubmissionStore`.
//! 3. `retain_validator_selection` seals the selection (done inside `run_q1_validation_retained`).
//! 4. `ValidationQueueStore::submit` enqueues the component.
//! 5. `run_q1_validation_retained` is called with the bridge wired to the live test driver.
//! 6. `Q1Outcome::StructuralObserved` is returned.
//! 7. DB evidence rows in `reborn_component_review_evidence` and `reborn_validator_q1_runs`
//!    confirm `verify_receipt` ran and persisted real Q1 evidence.
//!
//! This test exercises the full live path:
//! `run_q1_validation_retained` → `ValidatorTaskDriver::submit` →
//! original private Work/Ready → existing TurnRunnerWorker → GlobalMontyDriver →
//! private capture using A03's original validator input → ReviewAdmissionSource →
//! verify_receipt → persist_validator_q1_evidence → native live settlement.
use super::*;
use crate::{
    installed_monty_catalogue::ValidatorServiceBridge,
    pg_python_code_store::{NewPgPythonCode, PgPythonCodeStore},
    q1_orchestrator::{Q1Outcome, run_q1_validation_retained},
    validation_queue::ValidationQueueStore,
    validator_task_driver::ValidatorTaskDriver,
};
use brassclaw_engine::memory::retrieval_source::ComponentScope;
use brassclaw_skills::{
    component_revision::ComponentRevisionDraft, review_submission_store::ReviewSubmissionDraft,
};
use sha2::Digest;
use uuid::Uuid;

fn q1_retained_scope() -> ComponentScope {
    ComponentScope {
        tenant_id: "q1-retained-t".into(),
        user_id: "q1-retained-u".into(),
        agent_id: "q1-retained-a".into(),
        project_id: "q1-retained-p".into(),
    }
}

/// Full end-to-end Q1 retained pass: seeds drafts, creates a class-22 component,
/// submits to review, submits to queue, calls `run_q1_validation_retained` and
/// asserts `Q1Outcome::StructuralObserved` plus real evidence rows in the DB.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn run_q1_validation_retained_passes_and_persists_evidence() {
    retained_validator_pass(DeliveryCase::Ordinary).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_validator_delivers_after_factory_release_to_original_and_late_waiters() {
    retained_validator_pass(DeliveryCase::DelayedPublish).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_validator_worker_shutdown_delivers_unresolved_without_replay() {
    retained_validator_pass(DeliveryCase::WorkerShutdown).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_validator_heartbeat_loss_delivers_unresolved_without_replay() {
    retained_validator_pass(DeliveryCase::HeartbeatLoss).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_validator_prepare_race_keeps_first_raw_original_across_ack_changes() {
    retained_validator_pass(DeliveryCase::PrepareRace).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_validator_v207_to_v209_upgrade_preserves_original_and_native_q1() {
    retained_validator_pass(DeliveryCase::Upgrade209).await;
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DeliveryCase {
    Ordinary,
    Upgrade209,
    PrepareRace,
    DelayedPublish,
    WorkerShutdown,
    HeartbeatLoss,
}

/// Existing actual authoring fixture shared with private custody acceptance.
pub(super) async fn queued_structural_original(
    rig: &crate::runtime::test_pg::PgRig,
) -> (ComponentScope, ValidationQueueStore, Uuid, Uuid) {
    let scope = q1_retained_scope();
    let store = ValidationQueueStore::new(rig.pool.clone());

    // Insert the component row into reborn_python_code first — capture_q1_candidate
    // requires the row to exist in the component table (component_bytes query).
    let component_name = format!("test-component-{}", Uuid::new_v4().as_simple());
    let pc_store = PgPythonCodeStore::new(rig.pool.clone());
    let component_id = pc_store
        .insert(NewPgPythonCode {
            tenant_id: scope.tenant_id.clone(),
            user_id: scope.user_id.clone(),
            agent_id: scope.agent_id.clone(),
            project_id: scope.project_id.clone(),
            name: component_name.clone(),
            description: "Minimal test PythonCode component for Q1 retained integration test."
                .into(),
            content: "def retain_value(inputs):\n    return inputs['value']\n".into(),
            prior_knowledge_content: None,
            override_prompt_creation: false,
            consumer_tags: vec!["02:orchestrator".into(), "05:validator".into()],
            intent_examples: None,
            source: "authored".into(),
            dependency_registry: None,
            includes: vec![],
            content_checksum: None,
        })
        .await
        .expect("PgPythonCodeStore::insert must succeed");

    // Build the ComponentRevisionDraft referencing the assigned component_id.
    let doc = serde_json::json!({
        "format": "component-revision/1",
        "uuid": component_id,
        "class_code": 22,
        "document": {
            "consumer_tags":["02:orchestrator","05:validator"],"prior_knowledge_content":null,
            "override_prompt_creation":false,"intent_examples":null,"dependency_registry":null,"includes":[],
            "similarity_parent_id":null,"replaces_id":null,"parent_version":null,
            "name": component_name,
            "description": "Minimal test PythonCode component for Q1 retained integration test.",
            "content": "def retain_value(inputs):\n    return inputs['value']\n",
            "input_contract": {
                "value": {"type": "string", "required": true, "checks": []}
            },
            "result_contract": {"type": "string"},
            "preload": {
                "format": "python-preload/2",
                "exports": {
                    "retain_value": {
                        "symbol": "retain_value",
                        "parameters": ["inputs"],
                        "mapping": true
                    }
                },
                "private_functions": {},
                "constants": [],
                "imports": [],
                "dependencies": [],
                "default_export": "retain_value"
            }
        },
        "dependencies": [],
        "association": null
    })
    .to_string();

    let submission_id = Uuid::new_v4();
    let original_draft = ReviewSubmissionDraft::new(
        submission_id,
        "validator-structural-review",
        ComponentRevisionDraft::from_json(&doc).expect("minimal class-22 doc must parse"),
        None,
        vec![],
    )
    .expect("ReviewSubmissionDraft::new must succeed");

    // Submit to the validation queue (required for capture_q1_candidate).
    // Discovery notification is a no-op here (bridge not wired on the store).
    store
        .submit(&scope, component_id, 22, None)
        .await
        .expect("ValidationQueueStore::submit must succeed");
    let mut client = rig.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let queue_id: Uuid = tx
        .query_one(
            "SELECT id FROM reborn_validation_queue WHERE component_id=$1",
            &[&component_id],
        )
        .await
        .unwrap()
        .get(0);
    ValidationQueueStore::submit_original_in_transaction(
        &tx,
        &original_draft,
        &[(scope.clone(), component_id, queue_id)],
        &[queue_id],
        "existing_queue",
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    drop(client);

    (scope, store, component_id, submission_id)
}

async fn retained_validator_pass(case: DeliveryCase) {
    let rig = if case == DeliveryCase::Upgrade209 {
        crate::runtime::test_pg::pg_rig_at_version(207).await
    } else {
        crate::runtime::test_pg::pg_rig().await
    };
    if case == DeliveryCase::Upgrade209 {
        assert_eq!(
            rig.pool
                .get()
                .await
                .unwrap()
                .query_one("SELECT max(version) FROM refinery_schema_history", &[])
                .await
                .unwrap()
                .get::<_, i32>(0),
            207
        );
    }
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let directory = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(
        RebornRuntimeInput::from_services(
            rig.build_input("q1-retained-pass-owner", directory.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )
        .with_runner_settings(crate::runtime_input::TurnRunnerSettings {
            heartbeat_interval: std::time::Duration::from_millis(50),
            ..Default::default()
        }),
    ))
    .await
    .unwrap();

    // Build bridge from the real test-path driver.
    let bridge = ValidatorServiceBridge {
        driver: std::sync::Arc::new(ValidatorTaskDriver::new(
            runtime.monty_test_controls.0.clone(),
        )),
        pool: rig.pool.clone(),
    };

    // Seed validator drafts for all classes including class 22.
    crate::builtin_bootstrap::retain_validator_check_drafts(&rig.pool)
        .await
        .expect("retain_validator_check_drafts must succeed");

    let (scope, store, component_id, submission_id) = queued_structural_original(&rig).await;

    // An unrelated validator result for the same submission must not decide
    // this execution. Deliberately future-date this negative historical fixture
    // so a latest-row query would select it after the real run finishes. It is
    // not a trusted positive observation or an approval fixture.
    let decoy = Uuid::new_v4();
    let decoy_bytes = serde_json::json!({
        "format":"component-review-evidence/1", "evidence_id":decoy,
        "kind":"q1", "succeeded":false,
        "report":{"fixture":"unrelated negative validator result"}
    })
    .to_string();
    let decoy_checksum = hex::encode(sha2::Sha256::digest(decoy_bytes.as_bytes()));
    let decoy_selection = hex::encode(sha2::Sha256::digest(
        b"unrelated validator selection fixture",
    ));
    {
        let mut client = rig.pool.get().await.unwrap();
        let tx = client.transaction().await.unwrap();
        tx.execute(
            "INSERT INTO reborn_component_review_evidence(evidence_id,evidence_bytes,checksum)
             VALUES($1,$2,$3)",
            &[&decoy, &decoy_bytes, &decoy_checksum],
        )
        .await
        .unwrap();
        tx.execute(
            "INSERT INTO reborn_validator_q1_runs
             (run_id,submission_id,validator_selection_checksum,candidate_class_code,
              task_handle,evidence_id,succeeded,recorded_at)
             VALUES($1,$2,$3,22,$4,$1,false,clock_timestamp()+interval '1 day')",
            &[&decoy, &submission_id, &decoy_selection, &Uuid::new_v4()],
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }

    let counts_before: (i64, i64) = {
        let client = rig.pool.get().await.unwrap();
        let row = client.query_one("SELECT (SELECT count(*) FROM brassclaw_session_threads), (SELECT count(*) FROM brassclaw_inbound_message_receipts)", &[]).await.unwrap();
        (row.get(0), row.get(1))
    };
    let run = brassclaw_turns::TurnRunId::from_uuid(submission_id);
    let driver = runtime.monty_test_controls.0.clone();
    let pause = match case {
        DeliveryCase::Ordinary | DeliveryCase::PrepareRace | DeliveryCase::Upgrade209 => None,
        DeliveryCase::HeartbeatLoss => Some(driver.pause_private_start(run)),
        _ => Some(driver.pause_private_publish(run)),
    };
    let catalogue = runtime.installed_catalogue.clone();
    let first_entered = std::sync::Arc::new(tokio::sync::Notify::new());
    let first_release = std::sync::Arc::new(tokio::sync::Notify::new());
    let first_capture = catalogue.capture_task_preparation().await.unwrap();
    if case == DeliveryCase::PrepareRace {
        *catalogue.capture_gate.lock().await = Some((first_entered.clone(), first_release.clone()));
    }
    // The existing real Worker claims Ready Work. The hook only delays delivery
    // after drive_host has completed its actual native/SQL settlement.
    let validation = run_q1_validation_retained(
        &rig.pool,
        &scope,
        component_id,
        22,
        &store,
        brassclaw_monty_host::utility::UtilityLimits::from_execution(Default::default()).unwrap(),
        Some(&bridge),
    );
    let observe = async {
        if case == DeliveryCase::PrepareRace {
            tokio::time::timeout(std::time::Duration::from_secs(60), first_entered.notified())
                .await
                .unwrap();
            let settings = runtime.webui_monty_settings_store();
            let before = settings.get("default", "default").await.unwrap();
            let mut smaller = before.execution_limits;
            smaller.max_value_bytes = 256;
            smaller.validate().unwrap();
            let changed = settings.upsert("default", "default", &serde_json::from_value(
                serde_json::json!({"expected_revision":before.revision,"execution_limits":smaller})
            ).unwrap()).await.unwrap();
            let second_capture = catalogue.capture_task_preparation().await.unwrap();
            assert_eq!(second_capture.revision(), changed.revision);
            assert_ne!(first_capture.bytes(), second_capture.bytes());
            let second_entered = std::sync::Arc::new(tokio::sync::Notify::new());
            let second_release = std::sync::Arc::new(tokio::sync::Notify::new());
            *catalogue.capture_gate.lock().await =
                Some((second_entered.clone(), second_release.clone()));
            let second = catalogue.prepare_private_validator(submission_id);
            let release = async {
                tokio::time::timeout(
                    std::time::Duration::from_secs(60),
                    second_entered.notified(),
                )
                .await
                .unwrap();
                // Both real producers saw absent Work. Restore current dispatch
                // limits, while the second preparation retains its smaller ACK.
                settings.upsert("default", "default", &serde_json::from_value(
                    serde_json::json!({"expected_revision":changed.revision,"execution_limits":before.execution_limits})
                ).unwrap()).await.unwrap();
                first_release.notify_one();
                let original = tokio::time::timeout(std::time::Duration::from_secs(60), async {
                    loop {
                        let row = rig.pool.get().await.unwrap().query_opt(
                            "SELECT definition_bytes,checksum FROM brassclaw_monty_private_preparations WHERE work_id=$1",
                            &[&submission_id]).await.unwrap();
                        if let Some(row) = row { break (row.get::<_,String>(0),row.get::<_,Vec<u8>>(1)); }
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    }
                }).await.unwrap();
                second_release.notify_one();
                original
            };
            let (second, original) =
                tokio::time::timeout(std::time::Duration::from_secs(90), async {
                    tokio::join!(second, release)
                })
                .await
                .unwrap();
            let second =
                second.expect("smaller fresh preparation must return authenticated original");
            let definition = {
                let mut client = rig.pool.get().await.unwrap();
                let tx = client
                    .build_transaction()
                    .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
                    .read_only(true)
                    .start()
                    .await
                    .unwrap();
                let restored = crate::private_work_definition::PrivateWorkDefinition::restore(
                    &tx,
                    &second.request,
                    second.request_checksum,
                    &original.0,
                    original.1.clone().try_into().unwrap(),
                )
                .await
                .unwrap();
                tx.commit().await.unwrap();
                restored
            };
            let crate::private_work_definition::DomainCapture::ValidatorStructural {
                capture_bytes,
                ..
            } = definition.domain()
            else {
                panic!("validator capture")
            };
            let capture: serde_json::Value = serde_json::from_str(capture_bytes).unwrap();
            assert!(
                capture["input_bytes"].as_str().unwrap().len() > 256,
                "the offered ACK must actually reject fresh original input"
            );
            assert_eq!(definition.preparation().bytes(), first_capture.bytes());
            assert_ne!(definition.preparation().bytes(), second_capture.bytes());
            let state_store = brassclaw_turns::PgTurnStateStore::new(
                rig.pool.clone(),
                second.request.context().tenant_id.as_str(),
            );
            // Same UUID never authorizes a changed raw subject, original or actor.
            for mutation in 0..3 {
                let mut changed_request = second.request.clone();
                let brassclaw_turns::private_work::PrivateWorkRequest::V1 {
                    original, context, ..
                } = &mut changed_request;
                match mutation {
                    0 => {
                        let brassclaw_turns::private_work::PrivateOriginalRef::ValidatorStructural { subject_checksum, .. } = original else { panic!("validator original") };
                        subject_checksum[0] ^= 1;
                    }
                    1 => {
                        let brassclaw_turns::private_work::PrivateOriginalRef::ValidatorStructural { submission_id, .. } = original else { panic!("validator original") };
                        *submission_id = Uuid::new_v4();
                    }
                    _ => context.actor = None,
                }
                assert!(state_store.submit_private_work(&changed_request,
                    brassclaw_turns::private_work::PrivatePreparationReason::PreparationPending, catalogue.as_ref()).await.is_err());
            }
            let row = rig.pool.get().await.unwrap().query_one(
                "SELECT definition_bytes,checksum FROM brassclaw_monty_private_preparations WHERE work_id=$1", &[&submission_id]).await.unwrap();
            assert_eq!(row.get::<_, String>(0), original.0);
            assert_eq!(row.get::<_, Vec<u8>>(1), original.1);
        }
        if let Some(pause) = pause {
            pause.reached.notified().await;
            if case == DeliveryCase::HeartbeatLoss {
                // Fail the actual renewal of still-open Work at PostgreSQL.
                // The real Worker receives that error, drops its driver future,
                // stops/fences the exact claim and cannot submit a native task.
                let client = rig.pool.get().await.unwrap();
                client.batch_execute(&format!("CREATE FUNCTION b02_reject_private_heartbeat() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF OLD.work_id='{}'::uuid AND OLD.status='claimed' AND NEW.status='claimed' THEN RAISE EXCEPTION 'native heartbeat fault injection'; END IF; RETURN NEW; END $$; CREATE TRIGGER b02_reject_private_heartbeat BEFORE UPDATE ON brassclaw_turns FOR EACH ROW EXECUTE FUNCTION b02_reject_private_heartbeat();", submission_id)).await.unwrap();
                drop(client);
                let late = driver.private_completion_waiter(run).unwrap();
                let result = tokio::time::timeout(std::time::Duration::from_secs(60), late.wait())
                    .await
                    .unwrap();
                assert!(
                    matches!(result, Err(brassclaw_turns::run_profile::AgentLoopDriverError::Failed { reason_kind })
                    if reason_kind == "private_completion_producer_interrupted")
                );
                tokio::time::timeout(std::time::Duration::from_secs(60), async {
                    loop {
                        let client = rig.pool.get().await.unwrap();
                        let status: String = client
                            .query_one(
                                "SELECT status FROM brassclaw_turns WHERE work_id=$1",
                                &[&submission_id],
                            )
                            .await
                            .unwrap()
                            .get(0);
                        if status == "reconciliation_required" {
                            break;
                        }
                        drop(client);
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    }
                })
                .await
                .unwrap();
                rig.pool.get().await.unwrap().batch_execute("DROP TRIGGER b02_reject_private_heartbeat ON brassclaw_turns; DROP FUNCTION b02_reject_private_heartbeat();").await.unwrap();
                return;
            }
            let row = rig.pool.get().await.unwrap().query_one(
                "SELECT w.status,a.phase FROM brassclaw_turns w JOIN brassclaw_monty_task_admissions a ON a.private_work_id=w.work_id WHERE w.work_id=$1",
                &[&submission_id],
            ).await.unwrap();
            assert_eq!(row.get::<_, &str>(0), "settled");
            assert_eq!(row.get::<_, &str>(1), "settled");
            assert!(driver.private_completion_live(run).unwrap());
            let departed = driver.private_completion_waiter(run).unwrap();
            assert!(!departed.has_result());
            drop(departed);
            let late = driver.private_completion_waiter(run).unwrap();
            assert!(!late.has_result());
            assert!(driver.private_completion_live(run).unwrap());
            if case == DeliveryCase::WorkerShutdown {
                runtime.worker_cancel.cancel();
                let result = tokio::time::timeout(std::time::Duration::from_secs(60), late.wait())
                    .await
                    .expect("actual Worker interruption must resolve its waiter");
                assert!(
                    matches!(result, Err(brassclaw_turns::run_profile::AgentLoopDriverError::Failed { reason_kind })
                    if reason_kind == "private_completion_producer_interrupted")
                );
                return;
            }
            // Keep a healthy settled original pending across several actual
            // Worker heartbeat periods. It must not lose native delivery.
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            assert!(driver.private_completion_live(run).unwrap());
            // Consume the first caller's signal. The second actual submit must
            // pass its Settled-state/live-producer check before publication.
            pause.consumer_waiting.notified().await;
            let second_driver = ValidatorTaskDriver::new(driver.clone());
            let second = second_driver.submit(rig.pool.clone(), submission_id);
            let release = async {
                pause.consumer_waiting.notified().await;
                pause.release.notify_one();
            };
            let (second, ()) = tokio::join!(second, release);
            assert_eq!(
                second
                    .unwrap()
                    .internal_completion
                    .unwrap()
                    .attempt()
                    .run_id,
                run
            );
            let original = late.wait().await.unwrap();
            assert_eq!(original.internal_completion.unwrap().attempt().run_id, run);
        }
    };
    let (outcome, ()) = tokio::join!(validation, observe);
    {
        let client = rig.pool.get().await.unwrap();
        let row = client.query_one("SELECT (SELECT count(*) FROM brassclaw_session_threads), (SELECT count(*) FROM brassclaw_inbound_message_receipts)", &[]).await.unwrap();
        assert_eq!(
            (row.get::<_, i64>(0), row.get::<_, i64>(1)),
            counts_before,
            "private validator must not fabricate threads or inbound messages"
        );
        let row = client.query_one("SELECT w.record_kind,w.turn_id,a.origin_kind,a.scope,(SELECT count(*) FROM brassclaw_monty_private_terminals t WHERE t.work_id=w.work_id) FROM brassclaw_turns w JOIN brassclaw_monty_task_admissions a ON a.private_work_id=w.work_id WHERE w.work_id=$1", &[&submission_id]).await.unwrap();
        if case == DeliveryCase::PrepareRace {
            let counts = client.query_one("SELECT private_claim_generation,private_claim_attempts,
                (SELECT count(*) FROM brassclaw_monty_task_admissions a WHERE a.private_work_id=w.work_id),
                (SELECT count(*) FROM brassclaw_monty_private_preparations p WHERE p.work_id=w.work_id)
                FROM brassclaw_turns w WHERE work_id=$1", &[&submission_id]).await.unwrap();
            assert_eq!(
                (
                    counts.get::<_, i64>(0),
                    counts.get::<_, i64>(1),
                    counts.get::<_, i64>(2),
                    counts.get::<_, i64>(3)
                ),
                (1, 1, 1, 1)
            );
        }
        assert_eq!(row.get::<_, &str>(0), "private_work");
        assert_eq!(row.get::<_, Option<String>>(1), None);
        assert_eq!(row.get::<_, &str>(2), "private_work");
        assert_eq!(row.get::<_, Option<serde_json::Value>>(3), None);
        assert_eq!(
            row.get::<_, i64>(4),
            if case == DeliveryCase::HeartbeatLoss {
                0
            } else {
                1
            }
        );
    }

    if matches!(
        case,
        DeliveryCase::WorkerShutdown | DeliveryCase::HeartbeatLoss
    ) {
        assert!(
            outcome.is_err(),
            "an interrupted native delivery cannot promote Q1"
        );
        assert!(
            store
                .list(&scope, Some(2))
                .await
                .unwrap()
                .iter()
                .all(|row| row.component_id != component_id)
        );
        if case == DeliveryCase::HeartbeatLoss {
            assert!(!rig.pool.get().await.unwrap().query_one(
                "SELECT EXISTS(SELECT 1 FROM brassclaw_monty_private_deliveries WHERE work_id=$1)",
                &[&submission_id]).await.unwrap().get::<_,bool>(0),
                "pre-submission interruption cannot create post-Factory delivery");
        }
        return;
    }
    let outcome = outcome.expect("run_q1_validation_retained must not return Err");
    assert!(
        matches!(outcome, Q1Outcome::StructuralObserved),
        "expected genuine structural observations for a valid class-22 component; got {outcome:?}"
    );

    if case == DeliveryCase::Upgrade209 {
        assert_eq!(
            rig.pool
                .get()
                .await
                .unwrap()
                .query_one(
                    "SELECT count(*) FROM refinery_schema_history WHERE version IN (208,209)",
                    &[]
                )
                .await
                .unwrap()
                .get::<_, i64>(0),
            2
        );
        super::private_work_admission_tests::assert_v209_rejects_extra_request_field(
            &rig.pool,
            submission_id,
        )
        .await;
        let row = rig.pool.get().await.unwrap().query_one(
            "SELECT terminal_bytes::jsonb->'completion'->'outcome'->>'status' FROM brassclaw_monty_private_terminals WHERE work_id=$1",
            &[&submission_id]).await.unwrap();
        assert_eq!(row.get::<_, &str>(0), "internal_completed");
    }

    // Assert Q1 evidence rows were written by verify_receipt.
    let client = rig.pool.get().await.unwrap();
    let ev_count: i64 = client
        .query_one(
            "SELECT count(*) FROM reborn_component_review_evidence e
             JOIN reborn_validator_q1_runs r ON r.evidence_id = e.evidence_id
             WHERE r.submission_id = $1 AND r.succeeded = true",
            &[&submission_id],
        )
        .await
        .expect("evidence query must succeed")
        .get(0);
    assert!(
        ev_count >= 1,
        "expected at least one Q1 evidence+run row for submission {submission_id} with succeeded=true; got {ev_count}"
    );

    // Structural success has no full class-Q1 qualification and cannot graduate.
    let rows = store
        .list(&scope, Some(1))
        .await
        .expect("list must succeed");
    let row = rows
        .iter()
        .find(|r| r.component_id == component_id)
        .expect("queue row remains state 1 after structural observations");
    assert_eq!(
        row.state, 1,
        "structural-only observations cannot produce state 2"
    );
}

/// T5: end-to-end fail — component with empty `content` produces `Q1Outcome::Failed`.
///
/// Seeds validator drafts, creates a class-22 component with an empty `content`
/// field (which the structural validator rejects), submits it for review and to the
/// queue, calls `run_q1_validation_retained`, and asserts `Q1Outcome::Failed` with
/// a non-empty error list. The queue row must stay at state 1 (Q1 failed records
/// the failure but does not promote to state 2).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn run_q1_validation_retained_fails_for_empty_content() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let directory = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(RebornRuntimeInput::from_services(
        rig.build_input("q1-retained-fail-owner", directory.path())
            .with_runtime_policy(local_dev_runtime_policy()),
    )))
    .await
    .unwrap();

    let bridge = ValidatorServiceBridge {
        driver: std::sync::Arc::new(ValidatorTaskDriver::new(
            runtime.monty_test_controls.0.clone(),
        )),
        pool: rig.pool.clone(),
    };

    crate::builtin_bootstrap::retain_validator_check_drafts(&rig.pool)
        .await
        .expect("retain_validator_check_drafts must succeed");

    let scope = ComponentScope {
        tenant_id: "q1-retained-fail-t".into(),
        user_id: "q1-retained-fail-u".into(),
        agent_id: "q1-retained-fail-a".into(),
        project_id: "q1-retained-fail-p".into(),
    };
    let store = ValidationQueueStore::new(rig.pool.clone());

    // Insert the component row into reborn_python_code first — capture_q1_candidate
    // requires the row to exist in the component table (component_bytes query).
    // The content is deliberately empty so the structural validator rejects it.
    let fail_name = format!("fail-component-{}", Uuid::new_v4().as_simple());
    let pc_store = PgPythonCodeStore::new(rig.pool.clone());
    let component_id = pc_store
        .insert(NewPgPythonCode {
            tenant_id: scope.tenant_id.clone(),
            user_id: scope.user_id.clone(),
            agent_id: scope.agent_id.clone(),
            project_id: scope.project_id.clone(),
            name: fail_name.clone(),
            description: "A component whose content is deliberately empty.".into(),
            content: "".into(),
            prior_knowledge_content: None,
            override_prompt_creation: false,
            consumer_tags: vec!["02:orchestrator".into(), "05:validator".into()],
            intent_examples: None,
            source: "authored".into(),
            dependency_registry: None,
            includes: vec![],
            content_checksum: None,
        })
        .await
        .expect("PgPythonCodeStore::insert must succeed");

    let doc = serde_json::json!({
        "format": "component-revision/1",
        "uuid": component_id,
        "class_code": 22,
        "document": {
            "consumer_tags":["02:orchestrator","05:validator"],"prior_knowledge_content":null,
            "override_prompt_creation":false,"intent_examples":null,"dependency_registry":null,"includes":[],
            "similarity_parent_id":null,"replaces_id":null,"parent_version":null,
            "name": fail_name,
            "description": "A component whose content is deliberately empty.",
            "content": "",
            "input_contract": {},
            "result_contract": {"type": "null"},
            "preload": {
                "format": "python-preload/2",
                "exports": {},
                "private_functions": {},
                "constants": [],
                "imports": [],
                "dependencies": [],
                "default_export": null
            }
        },
        "dependencies": [],
        "association": null
    })
    .to_string();

    let submission_id = Uuid::new_v4();
    let original_draft = brassclaw_skills::review_submission_store::ReviewSubmissionDraft::new(
        submission_id,
        "validator-structural-review",
        brassclaw_skills::component_revision::ComponentRevisionDraft::from_json(&doc)
            .expect("malformed doc must parse at the JSON/schema level"),
        None,
        vec![],
    )
    .expect("ReviewSubmissionDraft::new must succeed");

    store
        .submit(&scope, component_id, 22, None)
        .await
        .expect("ValidationQueueStore::submit must succeed");
    let mut client = rig.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let queue_id: Uuid = tx
        .query_one(
            "SELECT id FROM reborn_validation_queue WHERE component_id=$1",
            &[&component_id],
        )
        .await
        .unwrap()
        .get(0);
    ValidationQueueStore::submit_original_in_transaction(
        &tx,
        &original_draft,
        &[(scope.clone(), component_id, queue_id)],
        &[queue_id],
        "existing_queue",
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    drop(client);

    let outcome = run_q1_validation_retained(
        &rig.pool,
        &scope,
        component_id,
        22,
        &store,
        brassclaw_monty_host::utility::UtilityLimits::from_execution(Default::default()).unwrap(),
        Some(&bridge),
    )
    .await
    .expect("run_q1_validation_retained must not return Err for a structural failure");

    match &outcome {
        Q1Outcome::Failed { errors } => {
            assert!(
                !errors.is_empty(),
                "expected non-empty error list for empty-content component; got none"
            );
            assert!(
                errors.iter().any(|e| e.ends_with(": content is empty")),
                "expected the actual rule diagnostic, not a generic failure; got: {errors:?}"
            );
        }
        other => panic!("expected Q1Outcome::Failed for empty-content component; got {other:?}"),
    }

    // Queue row must remain at state 1 (Q1_pending/failed) — NOT promoted to 2.
    let rows = store
        .list(&scope, Some(1))
        .await
        .expect("list must succeed");
    assert!(
        rows.iter().any(|r| r.component_id == component_id),
        "queue row must stay at state 1 after Q1Outcome::Failed"
    );
}

/// Test helper: creates a queued submission with its qualified validator
/// selection already retained, without completing full Q1 delivery. Used by
/// B3-06 reconciler tests that need `selection_preparation_present == true`
/// without running the full native settlement pipeline.
///
/// Returns `(submission_id, component_id)`.
pub(super) async fn queued_structural_original_with_selection(
    rig: &crate::runtime::test_pg::PgRig,
    runtime: &super::RebornRuntime,
) -> (Uuid, Uuid) {
    use brassclaw_engine::{
        executor::{retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram},
        memory::{
            retained_inputs::prepare_retained_unbound_program,
            retained_instruction::{WorkflowClass, compile_retained_recipe},
        },
    };
    use brassclaw_skills::{
        component_revision::{REVISION_LIMITS, RetainedComponentSnapshot},
        review_submission_store::PgReviewSubmissionStore,
        revision_store::PgComponentRevisionStore,
        validator_components::{
            structural_validator_aggregate_id, structural_validator_collect_id,
            structural_validator_pc_id, structural_validator_receipt_id,
            structural_validator_recipe_id,
        },
    };

    // Seed the validator check drafts for all classes.
    crate::builtin_bootstrap::retain_validator_check_drafts(&rig.pool)
        .await
        .expect("retain_validator_check_drafts must succeed");

    let (_scope, _store, component_id, submission_id) = queued_structural_original(rig).await;

    // Build the inspected programme from validator revision drafts (test-only
    // draft-head path; production selection creation belongs to B1-07/B2-04).
    let class_i16: i16 = 22;
    let revision_store = PgComponentRevisionStore::new(rig.pool.clone());
    let recipe_id = structural_validator_recipe_id(class_i16);
    let pc_id = structural_validator_pc_id(class_i16);
    let collect_id = structural_validator_collect_id(class_i16);
    let aggregate_id = structural_validator_aggregate_id(class_i16);
    let receipt_id = structural_validator_receipt_id(class_i16);

    let (_, recipe_version) = revision_store
        .read_allocation_head(recipe_id)
        .await
        .unwrap()
        .expect("recipe draft must be seeded");
    let (_, pc_version) = revision_store
        .read_allocation_head(pc_id)
        .await
        .unwrap()
        .expect("pc draft must be seeded");
    let (_, collect_version) = revision_store
        .read_allocation_head(collect_id)
        .await
        .unwrap()
        .expect("collect draft must be seeded");
    let (_, aggregate_version) = revision_store
        .read_allocation_head(aggregate_id)
        .await
        .unwrap()
        .expect("aggregate draft must be seeded");
    let (_, receipt_version) = revision_store
        .read_allocation_head(receipt_id)
        .await
        .unwrap()
        .expect("receipt draft must be seeded");

    let recipe_rev = revision_store
        .read_revision(recipe_id, recipe_version)
        .await
        .unwrap();
    let pc_rev = revision_store
        .read_revision(pc_id, pc_version)
        .await
        .unwrap();
    let collect_rev = revision_store
        .read_revision(collect_id, collect_version)
        .await
        .unwrap();
    let aggregate_rev = revision_store
        .read_revision(aggregate_id, aggregate_version)
        .await
        .unwrap();
    let receipt_rev = revision_store
        .read_revision(receipt_id, receipt_version)
        .await
        .unwrap();

    let snapshot = std::sync::Arc::new(
        RetainedComponentSnapshot::new(
            &[recipe_id],
            vec![recipe_rev, pc_rev, collect_rev, aggregate_rev, receipt_rev],
        )
        .unwrap(),
    );

    let preparation = runtime
        .installed_catalogue
        .capture_task_preparation()
        .await
        .unwrap();

    let instruction = compile_retained_recipe(
        snapshot,
        recipe_id,
        &format!("validator-class-{class_i16}"),
        WorkflowClass::Deterministic,
    )
    .unwrap();
    let program = prepare_retained_unbound_program(instruction).unwrap();

    let inspected = std::sync::Arc::new(
        InspectedRetainedProgram::inspect_with_limits(
            RetainedProgram::Unbound(std::sync::Arc::new(program)),
            runtime.installed_catalogue.worker.as_path(),
            preparation.limits(),
        )
        .await
        .unwrap(),
    );

    let subject = {
        let mut client = rig.pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .unwrap();
        let s = PgReviewSubmissionStore::read_validator_subject_in_transaction(
            &tx,
            submission_id,
            REVISION_LIMITS,
        )
        .await
        .unwrap()
        .expect("subject must exist");
        tx.commit().await.unwrap();
        s
    };

    crate::pg_validator_q1_admission::retain_validator_selection(&rig.pool, &subject, &inspected)
        .await
        .expect("retain_validator_selection must succeed");

    (submission_id, component_id)
}
