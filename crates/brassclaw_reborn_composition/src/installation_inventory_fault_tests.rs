//! Task-scoped hooks around the real installation consumer. No replacement
//! Tool/port result, audit deletion, alternate admission ID or schema is used.
use super::*;
use brassclaw_pg::test_support::{AckBoundary, AckCutRule, AckCutTransport};
use brassclaw_skills::component_revision::{ComponentRevisionDraft, RetainedComponentSnapshot};
use std::sync::Mutex as StdMutex;

tokio::task_local! { static PLAN: Arc<Plan>; }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fault {
    IntentAck,
    AdmissionReserveAck,
    AdmissionBindingAck,
    NativeAck,
    CompletionAck,
    IntentTimeout,
    LateCancel,
    CancelAck,
    CloseReject,
    CloseAck,
    TurnCloseAck,
    PreterminalAndCloseReject,
    Negative,
}
struct Plan {
    fault: Fault,
    profile: store::InstallationProfile,
    transport: Arc<AckCutTransport>,
    context: StdMutex<Option<Arc<Context>>>,
    reserved_run: OnceLock<Uuid>,
    original_run: OnceLock<Uuid>,
    cancel_run: OnceLock<Uuid>,
    baseline: tokio::sync::OnceCell<Audit>,
    after: tokio::sync::OnceCell<Audit>,
    consumer_status: OnceLock<&'static str>,
}
fn plan() -> Option<Arc<Plan>> {
    PLAN.try_with(Arc::clone).ok()
}

/// Exact rows/statuses from the actual review, approval, queue and catalogue
/// owners. False behavior carriers are allowed; Q1/Q2 and any success are not.
#[derive(Debug, PartialEq)]
pub(super) struct Audit(BTreeMap<String, Value>);
impl Audit {
    pub(super) async fn read(pool: &PgPool) -> Result<Self, RebornBuildError> {
        let mut client = pool.get().await.map_err(invalid)?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(invalid)?;
        let mut rows = BTreeMap::new();
        // These are existing tables from V102/104/149/200/201/202/206,
        // V051's legacy review owner, and V108/136's actual catalogue owners.
        for table in [
            "reborn_component_review_submissions",
            "reborn_component_review_requests",
            "reborn_validation_queue",
            "reborn_component_review_queue_bindings",
            "reborn_component_review_queue_requests",
            "reborn_skill_association_approvals",
            "reborn_pure_component_approvals",
            "reborn_pure_component_approval_closure",
            "reborn_pure_evidence_submission_links",
            "reborn_validator_q1_runs",
            "brassclaw_monty_active_catalogue",
            "brassclaw_monty_catalogue_activations",
            "brassclaw_monty_boot_catalogues",
            "reborn_python_code",
            "reborn_skills",
        ] {
            // Static test-owned identifiers, never runtime input.
            let sql = format!(
                "SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text),'[]'::jsonb) FROM {table} t"
            );
            rows.insert(
                table.to_owned(),
                tx.query_one(&sql, &[]).await.map_err(invalid)?.get(0),
            );
        }
        rows.insert("V102_grants_or_Q1_Q2".into(),tx.query_one("SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY evidence_id),'[]'::jsonb) FROM reborn_component_review_evidence t WHERE evidence_bytes::jsonb->>'kind' IN ('q1','q2') OR (evidence_bytes::jsonb->'succeeded') IS DISTINCT FROM 'false'::jsonb",&[]).await.map_err(invalid)?.get(0));
        tx.commit().await.map_err(invalid)?;
        Ok(Self(rows))
    }
}
pub(super) async fn before_subject(pool: &PgPool) -> Result<(), RebornBuildError> {
    if let Some(plan) = plan() {
        // Parser/native/ToolSkill prerequisite qualification is complete here.
        plan.baseline
            .set(Audit::read(pool).await?)
            .map_err(|_| invalid("test consumer baseline already captured"))?;
    }
    Ok(())
}
pub(super) async fn after_consumer(
    pool: &PgPool,
    result: &Result<InstallationSourceObservations, RebornBuildError>,
) -> Result<(), RebornBuildError> {
    if let Some(plan) = plan() {
        if let Ok(observations) = result {
            plan.consumer_status
                .set(observations.status())
                .map_err(|_| invalid("test consumer status already captured"))?;
        }
        if let Some(before) = plan.baseline.get() {
            let after = Audit::read(pool).await?;
            assert_eq!(
                before, &after,
                "installation consumer must not write Q1/Q2, approvals, queue/status or catalogue activation"
            );
            plan.after
                .set(after)
                .map_err(|_| invalid("test consumer final audit already captured"))?;
        }
    }
    Ok(())
}
fn negative_draft(
    original: ComponentRevisionDraft,
) -> Result<ComponentRevisionDraft, RebornBuildError> {
    let mut value: Value = serde_json::from_str(original.exact_bytes()).map_err(invalid)?;
    // Real immutable class-22 input with an invalid declared function
    // signature. Its original source is never executed as candidate code.
    value["document"]["preload"]["exports"]["check_structural_presence"]["parameters"] =
        json!(["unexpected"]);
    ComponentRevisionDraft::from_json(&value.to_string()).map_err(invalid)
}
pub(super) fn expected_subject(
    original: ComponentRevisionDraft,
) -> Result<ComponentRevisionDraft, RebornBuildError> {
    if plan().is_some_and(|p| p.fault == Fault::Negative) {
        negative_draft(original)
    } else {
        Ok(original)
    }
}
pub(super) async fn negative_subject(
    pool: &Arc<PgPool>,
    original: Arc<RetainedComponentSnapshot>,
) -> Result<Arc<RetainedComponentSnapshot>, RebornBuildError> {
    if !plan().is_some_and(|p| p.fault == Fault::Negative) {
        return Ok(original);
    }
    let id = packaged_component_id("validator:structural-presence");
    let revision = &original.revisions()[&id];
    assert_eq!(revision.reference().version, 1);
    let draft = negative_draft(
        brassclaw_skills::validator_components::structural_presence_draft(id).map_err(invalid)?,
    )?;
    let store = brassclaw_skills::revision_store::PgComponentRevisionStore::new(pool.clone());
    let reference = store
        .stage(&draft, revision.reference().version)
        .await
        .map_err(invalid)?;
    Ok(Arc::new(
        store
            .read_exact(&[id], &[reference])
            .await
            .map_err(invalid)?,
    ))
}
pub(super) async fn before_intent(context: &Arc<Context>) -> Result<(), RebornBuildError> {
    let Some(plan) = plan().filter(|p| p.profile == context.selection.profile()) else {
        return Ok(());
    };
    {
        let mut captured = plan.context.lock().unwrap();
        assert!(
            captured.is_none(),
            "fresh rig selects one original operation, never a replacement"
        );
        *captured = Some(context.clone());
    }
    match plan.fault {
        Fault::IntentAck => plan.transport.arm(1).map_err(invalid)?,
        Fault::AdmissionBindingAck => plan.transport.arm(3).map_err(invalid)?,
        Fault::NativeAck => plan.transport.arm(4).map_err(invalid)?,
        Fault::CompletionAck => plan.transport.arm(5).map_err(invalid)?,
        Fault::CloseReject | Fault::PreterminalAndCloseReject | Fault::IntentTimeout => {
            let client = context.pool.get().await.map_err(invalid)?;
            if matches!(
                plan.fault,
                Fault::CloseReject | Fault::PreterminalAndCloseReject
            ) {
                let admission = context.id("original-admission");
                client.batch_execute(&format!(r#"CREATE FUNCTION installation_close_fault() RETURNS trigger LANGUAGE plpgsql AS $$
                    BEGIN IF NEW.phase='settled' AND EXISTS(SELECT 1 FROM reborn_component_review_evidence WHERE evidence_id='{admission}'::uuid AND evidence_bytes::jsonb #>> '{{report,run_id}}'=NEW.run_id::text) THEN RAISE EXCEPTION 'original installation close rejected'; END IF; RETURN NEW; END $$;
                    CREATE TRIGGER installation_close_fault BEFORE UPDATE ON brassclaw_monty_task_admissions FOR EACH ROW EXECUTE FUNCTION installation_close_fault();"#)).await.map_err(invalid)?;
                if plan.fault == Fault::PreterminalAndCloseReject {
                    let preterminal = context.id("preterminal");
                    client.batch_execute(&format!(r#"CREATE FUNCTION installation_preterminal_fault() RETURNS trigger LANGUAGE plpgsql AS $$
                    BEGIN IF NEW.evidence_id='{preterminal}'::uuid THEN RAISE EXCEPTION 'original installation preterminal write rejected'; END IF; RETURN NEW; END $$;
                    CREATE TRIGGER installation_preterminal_fault BEFORE INSERT ON reborn_component_review_evidence FOR EACH ROW EXECUTE FUNCTION installation_preterminal_fault();"#)).await.map_err(invalid)?;
                }
            } else {
                let delay = context
                    .storage
                    .snapshot()
                    .map_err(invalid)?
                    .validate()
                    .map_err(invalid)?
                    .timeout
                    .checked_mul(3)
                    .ok_or_else(|| invalid("test deadline overflow"))?;
                let operation = context.operation;
                client.batch_execute(&format!(r#"CREATE FUNCTION installation_intent_delay() RETURNS trigger LANGUAGE plpgsql AS $$
                    BEGIN IF NEW.evidence_id='{operation}'::uuid THEN PERFORM pg_advisory_xact_lock(547122); PERFORM pg_sleep({}); END IF; RETURN NEW; END $$;
                    CREATE TRIGGER installation_intent_delay BEFORE INSERT ON reborn_component_review_evidence FOR EACH ROW EXECUTE FUNCTION installation_intent_delay();"#,delay.as_secs_f64())).await.map_err(invalid)?;
            }
        }
        _ => {}
    }
    Ok(())
}
/// Called immediately before the existing actual reservation transaction, only
/// for the contained installation-source-observer identity in cfg(test).
pub(crate) fn before_reservation(run: Uuid, reference: &str) -> Result<(), RebornBuildError> {
    if let Some(plan) = plan() {
        let selected = plan.context.lock().unwrap().as_ref().is_some_and(|c| {
            c.selection.profile() == plan.profile
                && reference == format!("installation-source-observation:{}", c.operation)
        });
        if selected {
            plan.original_run
                .set(run)
                .map_err(|_| invalid("test original run already captured"))?;
        }
        if selected && plan.fault == Fault::AdmissionReserveAck {
            plan.reserved_run
                .set(run)
                .map_err(|_| invalid("test original reservation already captured"))?;
            plan.transport.arm(2).map_err(invalid)?;
        }
    }
    Ok(())
}
pub(super) async fn after_quiescence(
    context: &Context,
    admitted: &crate::bootstrap_validation::Admitted,
) -> Result<(), RebornBuildError> {
    if plan().is_some_and(|p| {
        p.profile == context.selection.profile()
            && matches!(p.fault, Fault::LateCancel | Fault::CancelAck)
    }) {
        if let Some(plan) = plan() {
            plan.cancel_run
                .set(admitted.attempt.run_id.as_uuid())
                .map_err(|_| invalid("test original cancel run already captured"))?;
        }
        brassclaw_turns::TurnStateStore::request_cancel(
            admitted.state.as_ref(),
            brassclaw_turns::CancelRunRequest {
                scope: admitted.chat_context()?.scope.clone(),
                actor: admitted
                    .chat_context()?
                    .actor
                    .clone()
                    .ok_or_else(|| invalid("test actual actor missing"))?,
                run_id: admitted.attempt.run_id,
                reason: brassclaw_turns::SanitizedCancelReason::UserRequested,
                idempotency_key: brassclaw_turns::IdempotencyKey::new(format!(
                    "installation-late-cancel:{}",
                    context.operation
                ))
                .map_err(invalid)?,
            },
        )
        .await
        .map_err(invalid)?;
    }
    Ok(())
}

pub(crate) fn before_cancel(run: Uuid) -> Result<(), RebornBuildError> {
    if let Some(plan) =
        plan().filter(|p| p.fault == Fault::CancelAck && p.cancel_run.get() == Some(&run))
    {
        plan.transport.arm(6).map_err(invalid)?;
    }
    Ok(())
}

pub(crate) fn before_close(run: Uuid) -> Result<(), RebornBuildError> {
    if let Some(plan) =
        plan().filter(|p| p.fault == Fault::CloseAck && p.original_run.get() == Some(&run))
    {
        plan.transport.arm(7).map_err(invalid)?;
    }
    Ok(())
}
pub(crate) fn before_turn_close(run: Uuid) -> Result<(), RebornBuildError> {
    if let Some(plan) =
        plan().filter(|p| p.fault == Fault::TurnCloseAck && p.original_run.get() == Some(&run))
    {
        plan.transport.arm(8).map_err(invalid)?;
    }
    Ok(())
}

async fn run_case(fault: Fault, profile: store::InstallationProfile) {
    use secrecy::ExposeSecret;
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let transport = Arc::new(
        AckCutTransport::start(
            rig.url.expose_secret(),
            vec![
                AckCutRule::after_bind(
                    1,
                    vec![b"installation-validator-operation/1".to_vec()],
                    vec![],
                    AckBoundary::Commit,
                ),
                AckCutRule::next_commit(2),
                AckCutRule::next_commit(6),
                AckCutRule::next_commit(7),
                AckCutRule::next_commit(8),
                AckCutRule::after_bind(
                    3,
                    vec![b"installation-validator-admission/1".to_vec()],
                    vec![],
                    AckBoundary::Commit,
                ),
                AckCutRule::after_bind(
                    4,
                    vec![
                        b"{\"kind\":\"return\",\"value\":\"".to_vec(),
                        b"python-source-inventory/1".to_vec(),
                    ],
                    vec![],
                    AckBoundary::Commit,
                ),
                AckCutRule::after_bind(
                    5,
                    vec![b"installation-validator-completed/1".to_vec()],
                    vec![],
                    AckBoundary::Commit,
                ),
            ],
        )
        .await
        .unwrap(),
    );
    let plan = Arc::new(Plan {
        fault,
        profile,
        transport: transport.clone(),
        context: StdMutex::new(None),
        reserved_run: OnceLock::new(),
        original_run: OnceLock::new(),
        cancel_run: OnceLock::new(),
        baseline: tokio::sync::OnceCell::new(),
        after: tokio::sync::OnceCell::new(),
        consumer_status: OnceLock::new(),
    });
    let home = tempfile::tempdir().unwrap();
    let input = crate::RebornRuntimeInput::from_services(
        crate::RebornBuildInput::postgres_with_reborn_home(
            "installation-source-fault-owner",
            (*transport.pool).clone(),
            brassclaw_secrets::SecretMaterial::from(transport.connection_string().to_owned()),
            home.path().to_owned(),
        )
        .with_runtime_policy(crate::local_dev_runtime_policy().unwrap()),
    );
    let result = PLAN
        .scope(plan.clone(), Box::pin(crate::build_reborn_runtime(input)))
        .await;
    let context = plan
        .context
        .lock()
        .unwrap()
        .clone()
        .expect("actual selected installation profile reached its original intent");
    assert!(
        plan.baseline.get().is_some(),
        "real S prerequisites precede consumer baseline"
    );
    assert_eq!(
        plan.baseline.get(),
        plan.after.get(),
        "complete consumer owner/status comparison must actually run"
    );
    let failure = result.as_ref().err().map(ToString::to_string);
    let succeeds = matches!(
        fault,
        Fault::AdmissionBindingAck
            | Fault::CompletionAck
            | Fault::CloseAck
            | Fault::TurnCloseAck
            | Fault::Negative
    );
    assert_eq!(
        result.is_ok(),
        succeeds,
        "actual boot result for {fault:?}/{profile:?}"
    );
    if let Ok(runtime) = result {
        runtime.shutdown().await.unwrap();
    }
    let expected_cuts = usize::from(matches!(
        fault,
        Fault::IntentAck
            | Fault::AdmissionReserveAck
            | Fault::AdmissionBindingAck
            | Fault::NativeAck
            | Fault::CompletionAck
            | Fault::CancelAck
            | Fault::CloseAck
            | Fault::TurnCloseAck
    ));
    assert_eq!(
        transport.cut_count(),
        expected_cuts,
        "the actual targeted wire acknowledgement must be cut"
    );

    if fault == Fault::IntentTimeout {
        let client = rig.pool.get().await.unwrap();
        // Observe server-side transaction completion. Client timeout alone is
        // not proof of rollback or absence, and never permits dispatch here.
        client
            .query_one("SELECT pg_advisory_lock(547122)", &[])
            .await
            .unwrap();
        client
            .query_one("SELECT pg_advisory_unlock(547122)", &[])
            .await
            .unwrap();
        client.batch_execute("DROP TRIGGER installation_intent_delay ON reborn_component_review_evidence; DROP FUNCTION installation_intent_delay()").await.unwrap();
    }
    if matches!(fault, Fault::CloseReject | Fault::PreterminalAndCloseReject) {
        rig.pool.get().await.unwrap().batch_execute("DROP TRIGGER installation_close_fault ON brassclaw_monty_task_admissions; DROP FUNCTION installation_close_fault()").await.unwrap();
    }
    if fault == Fault::PreterminalAndCloseReject {
        rig.pool.get().await.unwrap().batch_execute("DROP TRIGGER installation_preterminal_fault ON reborn_component_review_evidence; DROP FUNCTION installation_preterminal_fault()").await.unwrap();
    }
    if matches!(fault, Fault::LateCancel | Fault::CancelAck) {
        let message = failure.as_ref().unwrap();
        assert!(
            message.contains("cleanup confirmed Cancelled"),
            "confirmed cancellation must be reported as terminal cleanup: {message}"
        );
        assert!(!message.contains("unacknowledged") && !message.contains("unresolved"));
    }
    if matches!(fault, Fault::CloseReject | Fault::PreterminalAndCloseReject) {
        let message = failure.as_ref().unwrap();
        assert!(
            message.contains("original cleanup failed")
                && message.contains("original close readback failed"),
            "both actual close failure and failed original readback must survive: {message}"
        );
        if fault == Fault::PreterminalAndCloseReject {
            let operation = message.split("original cleanup failed").next().unwrap();
            assert!(
                operation.contains("model-free contained receipt or quiescence incomplete")
                    || operation.contains("bootstrap Recipe"),
                "actual worker/receipt observation failure must precede its additional cleanup failure: {message}"
            );
        }
    }
    let mut client = rig.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    let operation = evidence::read_evidence(&tx, context.operation)
        .await
        .unwrap();
    let binding = context
        .read::<AdmissionBinding>(&tx, context.id("original-admission"))
        .await
        .unwrap();
    let pre = context
        .read::<Preterminal>(&tx, context.id("preterminal"))
        .await
        .unwrap();
    let quiescence = context
        .read::<QuiescenceRecord>(&tx, context.id("quiescence"))
        .await
        .unwrap();
    let completion = evidence::read_evidence(&tx, context.id("completed"))
        .await
        .unwrap();
    // Keep exact originals, including failed ones. No audit/intent row is ever
    // removed and no substitute operation ID is selected for another attempt.
    if fault != Fault::IntentTimeout {
        assert!(operation.is_some());
    }
    if matches!(
        fault,
        Fault::IntentAck | Fault::IntentTimeout | Fault::AdmissionReserveAck
    ) {
        assert!(binding.is_none());
        assert!(pre.is_none());
        assert!(quiescence.is_none());
        assert!(completion.is_none());
    }
    if let Some(run) = plan.reserved_run.get() {
        let row = tx
            .query_one(
                "SELECT phase FROM brassclaw_monty_task_admissions WHERE run_id=$1",
                &[run],
            )
            .await
            .unwrap();
        assert_eq!(
            row.get::<_, String>(0),
            "reserved",
            "lost real admission ACK stays original and unexecuted"
        );
        assert_eq!(
            tx.query_one(
                "SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1",
                &[run]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
            0
        );
    }
    if let Some((_, binding)) = &binding {
        let rows=tx.query("SELECT phase,attempt_count,answer_bytes FROM brassclaw_monty_tool_invocations WHERE run_id=$1",&[&binding.run_id]).await.unwrap();
        assert_eq!(
            rows.len(),
            1,
            "selected profile dispatched its actual native primitive once"
        );
        assert_eq!(rows[0].get::<_, String>(0), "answered");
        assert_eq!(rows[0].get::<_, i16>(1), 1);
        assert!(
            rows[0]
                .get::<_, Option<String>>(2)
                .unwrap()
                .contains("python-source-inventory/1")
        );
    }
    if matches!(
        fault,
        Fault::LateCancel | Fault::CancelAck | Fault::CloseReject
    ) {
        assert!(pre.is_some());
        assert!(quiescence.is_some());
        assert!(completion.is_none());
    }
    if matches!(fault, Fault::NativeAck | Fault::PreterminalAndCloseReject) {
        assert!(pre.is_none());
        assert!(completion.is_none());
    }
    if fault == Fault::PreterminalAndCloseReject {
        assert!(
            quiescence.is_none(),
            "failed original receipt has no model-free quiescence proof"
        );
    }

    if succeeds {
        assert!(pre.is_some());
        assert!(quiescence.is_some());
        assert!(completion.is_some());
        let checks = &pre.as_ref().unwrap().1.checks;
        assert_eq!(
            checks["aggregation"]["pass"],
            json!(fault != Fault::Negative)
        );
        assert_eq!(checks["q1_qualification"], false);
        assert_eq!(checks["semantic_approval"], false);
        assert_eq!(checks["catalogue_activation"], false);
        if fault == Fault::Negative {
            assert_eq!(
                plan.consumer_status.get(),
                Some(&"source_completed_negative_not_ready")
            );
            let rules = checks["rules"].as_array().unwrap();
            assert!(
                rules.iter().any(|r| r["rule_id"] == "python_definitions"
                    && r["ok"] == false
                    && !r["errors"].as_array().unwrap().is_empty()),
                "real native definition mismatch produces a completed negative rule, never a fabricated receipt"
            );
        } else {
            assert_eq!(
                plan.consumer_status.get(),
                Some(&"source_observed_positive_not_qualified")
            );
        }
    }
    let counts:Value=tx.query_one("SELECT jsonb_build_array((SELECT count(*) FROM brassclaw_monty_task_admissions),(SELECT count(*) FROM brassclaw_monty_tool_invocations))",&[]).await.unwrap().get(0);
    tx.commit().await.unwrap();
    drop(client);
    if matches!(fault, Fault::LateCancel | Fault::CancelAck) {
        let binding = &binding.as_ref().unwrap().1;
        let actual = brassclaw_turns::PgTurnStateStore::new(
            rig.pool.clone(),
            crate::bootstrap_validation::NAMESPACE,
        )
        .get_run_record(&binding.scope, TurnRunId::from_uuid(binding.run_id))
        .await
        .unwrap()
        .unwrap();
        assert_eq!(actual.status, TurnStatus::Cancelled);
        assert!(
            actual.runner_id.is_none()
                && actual.lease_token.is_none()
                && actual.lease_expires_at.is_none(),
            "actual cancel ACK clears the original claim"
        );
    }
    let original = context.read_original().await;
    if succeeds {
        assert_eq!(original.unwrap().unwrap().1, fault != Fault::Negative);
    } else if operation.is_some() {
        assert!(
            original.is_err(),
            "incomplete original blocks another admission"
        );
    } else {
        assert!(
            original.unwrap().is_none(),
            "definitive absent first intent has no execution proof"
        );
    }
    let mut client = rig.pool.get().await.unwrap();
    let tx = client.transaction().await.unwrap();
    assert_eq!(
        evidence::read_evidence(&tx, context.operation)
            .await
            .unwrap(),
        operation,
        "original intent bytes remain unchanged"
    );
    let after:Value=tx.query_one("SELECT jsonb_build_array((SELECT count(*) FROM brassclaw_monty_task_admissions),(SELECT count(*) FROM brassclaw_monty_tool_invocations))",&[]).await.unwrap().get(0);
    assert_eq!(
        counts, after,
        "readback never reserves or replays an effect"
    );
    tx.commit().await.unwrap();
    drop(client);
    drop(context);
    drop(plan);
    match Arc::try_unwrap(transport) {
        Ok(transport) => transport.shutdown().await.unwrap(),
        Err(_) => panic!("test transport owner leaked"),
    };
}

macro_rules! cases {
    ($($name:ident:$fault:ident,$profile:ident;)*)=>{$(
        #[tokio::test(flavor="multi_thread",worker_threads=2)]
        async fn $name(){run_case(Fault::$fault,store::InstallationProfile::$profile).await;}
    )*};
}
cases! {
    installation_structure_intent_ack:IntentAck,Structure;
    installation_pure_intent_ack:IntentAck,Pure;
    installation_structure_admission_reserve_ack:AdmissionReserveAck,Structure;
    installation_pure_admission_reserve_ack:AdmissionReserveAck,Pure;
    installation_structure_admission_binding_ack:AdmissionBindingAck,Structure;
    installation_pure_admission_binding_ack:AdmissionBindingAck,Pure;
    installation_structure_native_ack:NativeAck,Structure;
    installation_pure_native_ack:NativeAck,Pure;
    installation_structure_completion_ack:CompletionAck,Structure;
    installation_pure_completion_ack:CompletionAck,Pure;
    installation_structure_intent_io_timeout:IntentTimeout,Structure;
    installation_pure_intent_io_timeout:IntentTimeout,Pure;
    installation_structure_late_cancel:LateCancel,Structure;
    installation_pure_late_cancel:LateCancel,Pure;
    installation_structure_close_error:CloseReject,Structure;
    installation_pure_close_error:CloseReject,Pure;
    installation_structure_cancel_ack:CancelAck,Structure;
    installation_pure_cancel_ack:CancelAck,Pure;
    installation_structure_close_ack:CloseAck,Structure;
    installation_pure_close_ack:CloseAck,Pure;
    installation_structure_turn_close_ack:TurnCloseAck,Structure;
    installation_pure_turn_close_ack:TurnCloseAck,Pure;
    installation_structure_operation_and_cleanup_fail:PreterminalAndCloseReject,Structure;
    installation_pure_operation_and_cleanup_fail:PreterminalAndCloseReject,Pure;
    installation_actual_negative_rules:Negative,Pure;
}
