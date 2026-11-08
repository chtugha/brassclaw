//! Native acceptance source. These cases must run in Root's coherent batch;
//! no source-only result is a behavioral qualification receipt.
use super::*;
use brassclaw_pg::test_support::{AckBoundary, AckCutRule, AckCutTransport};
use brassclaw_skills::component_revision::ComponentRevisionDraft;
use secrecy::ExposeSecret;

tokio::task_local! { static FAULT: Fault; }
tokio::task_local! { static RACE: Arc<tokio::sync::Barrier>; }
pub(super) async fn before_new_intent(context: &Context) {
    if context.claim.root.uuid == fixtures::receipt_id() {
        if let Ok(barrier) = RACE.try_with(Arc::clone) {
            barrier.wait().await;
        }
    }
}
#[derive(Clone, Copy)]
enum Fault {
    #[cfg(unix)]
    WorkerExit,
    LateCancel,
}
#[cfg(unix)]
pub(super) fn force_exit(context: &Context, suite: &Suite) -> bool {
    context.claim.root.uuid == brassclaw_skills::validator_components::structural_validator_pc_id(0)
        && suite.variant == "stop_json"
        && FAULT
            .try_with(|f| matches!(f, Fault::WorkerExit))
            .unwrap_or(false)
}
pub(super) async fn before_close(
    context: &Context,
    suite: &Suite,
    admitted: &crate::bootstrap_validation::Admitted,
) -> Result<(), RebornBuildError> {
    if context.claim.root.uuid
        == brassclaw_skills::validator_components::structural_validator_pc_id(0)
        && suite.variant == "selected"
        && FAULT
            .try_with(|f| matches!(f, Fault::LateCancel))
            .unwrap_or(false)
    {
        brassclaw_turns::TurnStateStore::request_cancel(
            admitted.state.as_ref(),
            brassclaw_turns::CancelRunRequest {
                scope: admitted.chat_context()?.scope.clone(),
                actor: admitted
                    .chat_context()?
                    .actor
                    .clone()
                    .ok_or_else(|| invalid("actual checker actor missing"))?,
                run_id: admitted.attempt.run_id,
                reason: brassclaw_turns::SanitizedCancelReason::UserRequested,
                idempotency_key: brassclaw_turns::IdempotencyKey::new(format!(
                    "checker-native-cancel:{}",
                    context.operation_id
                ))
                .map_err(invalid)?,
            },
        )
        .await
        .map_err(invalid)?;
    }
    Ok(())
}
async fn read_proof(runtime: &crate::RebornRuntime, pool: Arc<PgPool>) -> CheckedStructuralHelpers {
    let catalogue = &runtime.installed_catalogue;
    qualify(
        pool,
        catalogue.worker_image.clone(),
        catalogue.root_definition.clone(),
        catalogue.startup_executor.clone(),
        catalogue.capture_task_preparation().await.unwrap(),
        catalogue.resource_store_settings.clone(),
        hex::encode(catalogue._image.checksum()),
    )
    .await
    .unwrap()
}
async fn assert_first_claim_race(runtime: &crate::RebornRuntime, base: &CheckedStructuralHelpers) {
    // A distinct migrated fixture database has no prior claims. Reuse the
    // genuine paired installation executor/preparation and retained worker/
    // application artifacts, not fake native results or a second product VM.
    let fixture = crate::runtime::test_pg::pg_rig().await;
    let root = Arc::new(
        crate::global_root_seed::retain_packaged_global_root(
            fixture.pool.clone(),
            crate::builtin_bootstrap::GLOBAL_ORCHESTRATOR_SEED,
        )
        .await
        .unwrap(),
    );
    let preparation = base.contexts[0].preparation().unwrap();
    let catalogue = &runtime.installed_catalogue;
    let call = || {
        qualify(
            fixture.pool.clone(),
            catalogue.worker_image.clone(),
            root.clone(),
            catalogue.startup_executor.clone(),
            preparation.clone(),
            catalogue.resource_store_settings.clone(),
            hex::encode(catalogue._image.checksum()),
        )
    };
    let barrier = Arc::new(tokio::sync::Barrier::new(2));
    let (left, right) = tokio::join!(
        RACE.scope(barrier.clone(), call()),
        RACE.scope(barrier, call())
    );
    assert_ne!(
        left.is_ok(),
        right.is_ok(),
        "only the first committed original may execute; its loser is unresolved without replay"
    );
    let proof = left
        .ok()
        .or_else(|| right.ok())
        .expect("one genuine first-claim producer completes");
    verify(&fixture.pool, &proof).await;
    let expected = i64::try_from(
        proof
            .contexts
            .iter()
            .map(|c| c.claim.suites.len())
            .sum::<usize>(),
    )
    .unwrap();
    assert_eq!(
        counters(&fixture.pool).await,
        json!([expected, expected, expected]),
        "same-ID race must not duplicate even an unexecuted native admission"
    );
}
async fn counters(pool: &PgPool) -> Value {
    let client = pool.get().await.unwrap();
    let row=client.query_one("SELECT
        (SELECT count(*) FROM reborn_component_review_evidence WHERE evidence_bytes::jsonb #>> '{report,format}'='structural-checker-admission/1'),
        (SELECT count(*) FROM brassclaw_monty_task_admissions),
        (SELECT count(*) FROM reborn_component_review_evidence WHERE evidence_bytes::jsonb #>> '{report,format}'='structural-checker-observed/1')",&[]).await.unwrap();
    json!([
        row.get::<_, i64>(0),
        row.get::<_, i64>(1),
        row.get::<_, i64>(2)
    ])
}
async fn verify(pool: &PgPool, proof: &CheckedStructuralHelpers) {
    let mut client = pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    proof.verify_in_transaction(&tx).await.unwrap();
    tx.commit().await.unwrap();
}
async fn retain_and_reject_fabricated_legacy(context: &Context) {
    // Deliberate historical audit fixture, not a behavioral oracle. It claims
    // the exact current root yet contains only the old unperformed check list.
    let identity = Uuid::new_v5(
        &Uuid::from_u128(0xb8a7_3c5f_1d92_4e6a_9f30_2c84_e7d1_5b02),
        format!("validator:class-0:{}", context.claim.root.uuid).as_bytes(),
    );
    let q1 = Uuid::new_v5(&identity, b"q1");
    let behavior = Uuid::new_v5(&identity, b"behavior");
    let approval = Uuid::new_v5(&identity, b"approval");
    let component = serde_json::to_value(&context.claim.root).unwrap();
    for (id, kind) in [(q1, "q1"), (behavior, "behavior")] {
        let bytes = evidence::system_record(
            id,
            kind,
            true,
            &evidence::checksum(&context.claim.root.uuid.to_string()),
            &[component.clone()],
            json!({"checks":["no-host-calls","effect-free-load"]}),
        );
        context.pool.get().await.unwrap().execute("INSERT INTO reborn_component_review_evidence(evidence_id,evidence_bytes,checksum) VALUES($1,$2,$3)",&[&id,&bytes,&evidence::checksum(&bytes)]).await.unwrap();
    }
    let bytes=json!({"format":"pure-component-approval/1","approval_id":approval,"root":component,"components":[component],"validation_mode":"system_seed","q1_ref":q1,"q2_ref":null,"behavioral_refs":[behavior]}).to_string();
    let declaration = PureComponentApprovalDeclaration::from_json(&bytes, REVISION_LIMITS).unwrap();
    crate::pg_pure_component_approval::record_pure_component_approval(&context.pool, &declaration)
        .await
        .unwrap();
    let mut client = context.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    assert!(
        context
            .read::<Qualified>(&tx, q1, "q1", true)
            .await
            .is_err(),
        "matching root/presence/check names cannot mint native qualification"
    );
    let native = context.verify_approval(&tx).await.unwrap();
    assert_ne!(native.id, approval);
    assert_eq!(
        tx.query_one(
            "SELECT approval_bytes FROM reborn_pure_component_approvals WHERE approval_id=$1",
            &[&approval]
        )
        .await
        .unwrap()
        .get::<_, String>(0),
        bytes,
        "invalid historical rows remain immutable audit data"
    );
    tx.commit().await.unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_structural_checker_exports_keep_originals_after_pruning_smaller_limits_and_authored_head()
 {
    use brassclaw_turns::{TurnSpawnTreeStateStore, TurnStateStore};
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _database = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let home = tempfile::tempdir().unwrap();
    let runtime = Box::pin(crate::build_reborn_runtime(
        crate::RebornRuntimeInput::from_services(
            rig.build_input("structural-checker-native", home.path())
                .with_runtime_policy(crate::local_dev_runtime_policy().unwrap()),
        ),
    ))
    .await
    .unwrap();
    let proof = read_proof(&runtime, rig.pool.clone()).await;
    verify(&rig.pool, &proof).await;
    assert_first_claim_race(&runtime, &proof).await;
    retain_and_reject_fabricated_legacy(&proof.contexts[1]).await;
    let settings = runtime.webui_monty_settings_store();
    let current = settings.get("default", "default").await.unwrap();
    let mut larger_depth = current.execution_limits;
    larger_depth.max_value_depth =
        u32::try_from(REVISION_LIMITS.max_depth.checked_add(1).unwrap()).unwrap();
    larger_depth.validate().unwrap();
    settings
        .upsert(
            "default",
            "default",
            &serde_json::from_value(
                json!({"expected_revision":current.revision,"execution_limits":larger_depth}),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let acknowledged = runtime
        .installed_catalogue
        .capture_task_preparation()
        .await
        .unwrap();
    let program = proof.contexts[1].program("selected").unwrap();
    let actual = InspectedRetainedProgram::inspect_with_limits(
        RetainedProgram::Tools(program.clone()),
        &runtime.installed_catalogue.worker_image.path(),
        acknowledged.limits(),
    )
    .await
    .unwrap();
    assert_eq!(
        actual
            .pure_component(proof.contexts[1].claim.root.uuid)
            .unwrap()
            .components()
            .len(),
        1,
        "immutable schema metadata keeps its own profile when actual task depth is larger"
    );
    assert_eq!(
        proof.contexts.len(),
        brassclaw_skills::validator_components::VALIDATOR_CLASS_CODES.len() + 1
    );
    let before = counters(&rig.pool).await;
    for context in &proof.contexts {
        for suite in &context.claim.suites {
            let mut client = rig.pool.get().await.unwrap();
            let tx = client
                .build_transaction()
                .isolation_level(IsolationLevel::RepeatableRead)
                .read_only(true)
                .start()
                .await
                .unwrap();
            let (_, admission) = context
                .read::<Admission>(
                    &tx,
                    context.id(&format!("{}:admission", suite.variant)),
                    "behavior",
                    false,
                )
                .await
                .unwrap()
                .unwrap();
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
                "same-output foreign input must not qualify"
            );
            tx.commit().await.unwrap();
            drop(client);
            let state = brassclaw_turns::PgTurnStateStore::new(
                rig.pool.clone(),
                crate::bootstrap_validation::NAMESPACE,
            )
            .with_limits(brassclaw_turns::InMemoryTurnStateStoreLimits {
                max_terminal_records: 0,
                ..Default::default()
            });
            let run = brassclaw_turns::TurnRunId::from_uuid(admission.run_id);
            state
                .request_cancel_tree(brassclaw_turns::CancelRunRequest {
                    scope: admission.scope.clone(),
                    actor: brassclaw_turns::TurnActor::new(
                        brassclaw_host_api::UserId::new("installation-owner").unwrap(),
                    ),
                    run_id: run,
                    reason: brassclaw_turns::SanitizedCancelReason::OperatorRequested,
                    idempotency_key: brassclaw_turns::IdempotencyKey::new(format!(
                        "prune-checker-{run}"
                    ))
                    .unwrap(),
                })
                .await
                .unwrap();
            assert!(
                state
                    .get_run_record(&admission.scope, run)
                    .await
                    .unwrap()
                    .is_none()
            );
        }
    }
    let candidate = &proof.contexts[1];
    for bad_input in [
        json!({}),
        json!({"subject_json":null}),
        json!({"subject_json":1}),
    ] {
        assert!(
            candidate
                .program("selected")
                .unwrap()
                .inputs()
                .bind_step_inputs("0:1", &bad_input)
                .is_err(),
            "invalid typed invocation inputs reject without counting as export behavior"
        );
    }
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let original = candidate
        .program(&candidate.claim.suites[0].variant)
        .unwrap()
        .inputs()
        .instruction()
        .snapshot()
        .revisions()[&candidate.claim.root.uuid]
        .draft();
    let mut authored: Value = serde_json::from_str(original.exact_bytes()).unwrap();
    authored["document"]["content"] = json!(format!(
        "{}\n# deliberate authored draft override\n",
        original.document()["content"].as_str().unwrap()
    ));
    let authored = ComponentRevisionDraft::from_json(&authored.to_string()).unwrap();
    let head = store
        .read_allocation_head(authored.uuid())
        .await
        .unwrap()
        .unwrap()
        .1;
    let authored_ref = store.stage(&authored, head).await.unwrap();
    let settings = runtime.webui_monty_settings_store();
    let current = settings.get("default", "default").await.unwrap();
    let mut smaller = current.execution_limits;
    // Derive the reduction from the actual retained claim. It remains immutable
    // metadata even when it is larger than today's executing-task byte budget.
    let retained_claim_bytes = serde_json::to_vec(&candidate.claim).unwrap().len();
    smaller.max_value_bytes = smaller.max_value_bytes.min(retained_claim_bytes / 2);
    assert!(smaller.max_value_bytes < retained_claim_bytes);
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
    let reread = read_proof(&runtime, pool.clone()).await;
    verify(&pool, &reread).await;
    assert_eq!(
        before,
        counters(&rig.pool).await,
        "completed original cannot create another admission/execution"
    );
    for (a, b) in proof.contexts.iter().zip(&reread.contexts) {
        assert_eq!(a.operation_bytes, b.operation_bytes);
        assert_eq!(a.approval_bytes().unwrap(), b.approval_bytes().unwrap());
    }
    assert_eq!(
        store
            .read_allocation_head(authored.uuid())
            .await
            .unwrap()
            .unwrap()
            .1,
        authored_ref.version
    );
    assert_eq!(
        reread.contexts[1].claim.root.version, candidate.claim.root.version,
        "closed package selection preserves old canonical bytes without choosing an authored head"
    );
    pool.close();
    drop(pool);
    drop(reread);
    drop(proof);
    runtime.shutdown().await.unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_structural_checker_completion_ack_loss_is_original_and_nonreplaying() {
    ack_case(false).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_structural_checker_approval_ack_loss_is_original_and_nonreplaying() {
    ack_case(true).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_structural_checker_intent_ack_loss_stays_unexecuted_and_unresolved() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _database = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let transport = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::after_bind(
            1,
            vec![b"structural-checker-operation/1".to_vec()],
            vec![],
            AckBoundary::Commit,
        )],
    )
    .await
    .unwrap();
    transport.arm(1).unwrap();
    let home = tempfile::tempdir().unwrap();
    let input = || {
        crate::RebornRuntimeInput::from_services(
            crate::RebornBuildInput::postgres_with_reborn_home(
                "checker-intent-ack-native",
                (*transport.pool).clone(),
                brassclaw_secrets::SecretMaterial::from(transport.connection_string().to_owned()),
                home.path().to_owned(),
            )
            .with_runtime_policy(crate::local_dev_runtime_policy().unwrap()),
        )
    };
    assert!(
        Box::pin(crate::build_reborn_runtime(input()))
            .await
            .is_err()
    );
    assert_eq!(
        transport.cut_count(),
        1,
        "the actual retained intent commit acknowledgement was lost"
    );
    let before = counters(&transport.pool).await;
    assert_eq!(before[0], 0);
    assert_eq!(
        before[2], 0,
        "an unacknowledged new intent cannot execute an export"
    );
    assert!(
        Box::pin(crate::build_reborn_runtime(input()))
            .await
            .is_err()
    );
    assert_eq!(
        before,
        counters(&transport.pool).await,
        "existing incomplete intent cannot be replayed even after a readable COMMIT"
    );
    transport.shutdown().await.unwrap();
}
async fn ack_case(approval: bool) {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _database = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let required = if approval {
        vec![
            b"pure-component-approval/1".to_vec(),
            fixtures::receipt_id().to_string().into_bytes(),
        ]
    } else {
        vec![b"structural-checker-completed/1".to_vec()]
    };
    let transport = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::after_bind(
            1,
            required,
            vec![],
            AckBoundary::Commit,
        )],
    )
    .await
    .unwrap();
    transport.arm(1).unwrap();
    let home = tempfile::tempdir().unwrap();
    let input = crate::RebornRuntimeInput::from_services(
        crate::RebornBuildInput::postgres_with_reborn_home(
            "checker-ack-native",
            (*transport.pool).clone(),
            brassclaw_secrets::SecretMaterial::from(transport.connection_string().to_owned()),
            home.path().to_owned(),
        )
        .with_runtime_policy(crate::local_dev_runtime_policy().unwrap()),
    );
    let runtime = Box::pin(crate::build_reborn_runtime(input)).await.unwrap();
    assert_eq!(
        transport.cut_count(),
        1,
        "targeted real COMMIT acknowledgement must be lost"
    );
    let before = counters(&transport.pool).await;
    let proof = read_proof(&runtime, transport.pool.clone()).await;
    verify(&transport.pool, &proof).await;
    assert_eq!(before, counters(&transport.pool).await);
    drop(proof);
    runtime.shutdown().await.unwrap();
    transport.shutdown().await.unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg(unix)]
async fn native_structural_checker_worker_death_cannot_satisfy_expected_python_stop() {
    fault_case(Fault::WorkerExit).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_structural_checker_late_cancellation_never_approves_or_replays() {
    fault_case(Fault::LateCancel).await;
}
async fn fault_case(fault: Fault) {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _database = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let home = tempfile::tempdir().unwrap();
    let input = || {
        crate::RebornRuntimeInput::from_services(
            rig.build_input("checker-fault-native", home.path())
                .with_runtime_policy(crate::local_dev_runtime_policy().unwrap()),
        )
    };
    let first = FAULT
        .scope(fault, Box::pin(crate::build_reborn_runtime(input())))
        .await;
    assert!(
        first.is_err(),
        "actual worker/cancellation fault must prevent readiness"
    );
    let before = counters(&rig.pool).await;
    let second = Box::pin(crate::build_reborn_runtime(input())).await;
    assert!(
        second.is_err(),
        "incomplete original is unresolved, never rerun"
    );
    assert_eq!(
        before,
        counters(&rig.pool).await,
        "second boot cannot create replacement checker executions"
    );
    let roots = brassclaw_skills::validator_components::VALIDATOR_CLASS_CODES
        .iter()
        .map(|c| brassclaw_skills::validator_components::structural_validator_pc_id(*c))
        .collect::<Vec<_>>();
    let count: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT count(*) FROM reborn_pure_component_approvals WHERE root_id=ANY($1::uuid[])",
            &[&roots],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 0);
}
