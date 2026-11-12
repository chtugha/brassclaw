//! Native PG/original/claim tests. All reservations, hosts and native receipts
//! come from the existing owners; the observer only retains their actual handle.
use super::*;
use crate::{
    RebornRuntime,
    global_monty_driver::GlobalTaskPortsFactory,
    global_task_factory::OwnedGlobalTaskFactory,
    prefix_job_definitions::{Definition, Origin, Reservation, Store, Target},
};
use brassclaw_pg::{
    PgPool,
    test_support::{AckBoundary, AckCutRule, AckCutTransport},
};
use brassclaw_turns::{
    PgTurnStateStore, TurnLeaseToken, TurnRunnerId,
    pg_turn_state_store::{PgPrivateClaimParticipant, PgPrivateClaimReservation},
    private_work::{ClaimedPrivateWork, PrivateExecutionContext, PrivateWorkStatus},
    run_profile::MontyTurnDriverPort,
    runner::{ClaimRunRequest, TurnRunTransitionPort},
};
use secrecy::ExposeSecret as _;
use serde_json::json;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

struct Reservations {
    owner: Arc<OwnedGlobalTaskFactory>,
    originals: Mutex<Vec<Arc<dyn PgPrivateClaimReservation>>>,
}
impl PgPrivateClaimParticipant for Reservations {
    fn retain_reservation(
        &self,
        claimed: &ClaimedPrivateWork,
    ) -> Result<Arc<dyn PgPrivateClaimReservation>, brassclaw_turns::TurnError> {
        let original = self.owner.retain_reservation(claimed)?;
        self.originals.lock().unwrap().push(original.clone());
        Ok(original)
    }
}
struct Fixture {
    runtime: Arc<RebornRuntime>,
    pool: Arc<PgPool>,
    store: Arc<PgTurnStateStore>,
    reservations: Arc<Reservations>,
    _home: tempfile::TempDir,
}
impl Fixture {
    async fn start(rig: &crate::runtime::test_pg::PgRig, pool: Arc<PgPool>) -> Self {
        let home = tempfile::tempdir().unwrap();
        let runtime = Arc::new(
            Box::pin(build_reborn_runtime(
                RebornRuntimeInput::from_services(
                    crate::input::RebornBuildInput::postgres_with_reborn_home(
                        "private-admission-owner",
                        pool.as_ref().clone(),
                        rig.url.clone(),
                        home.path().to_path_buf(),
                    )
                    .with_runtime_policy(local_dev_runtime_policy()),
                )
                // The actual Worker remains alive at its existing startup gate; no
                // test readiness flag or substitute claim loop is installed.
                .with_inbound_mcp_required(true)
                .with_runner_settings(crate::runtime_input::TurnRunnerSettings {
                    heartbeat_interval: Duration::from_millis(50),
                    poll_interval: Duration::from_millis(100),
                }),
            ))
            .await
            .unwrap(),
        );
        let store = Arc::new(PgTurnStateStore::new(
            pool.clone(),
            runtime.thread_scope.tenant_id.as_str(),
        ));
        let reservations = Arc::new(Reservations {
            owner: runtime.monty_test_controls.1.clone(),
            originals: Mutex::new(Vec::new()),
        });
        store
            .bind_private_work_participant(reservations.clone())
            .unwrap();
        Self {
            runtime,
            pool,
            store,
            reservations,
            _home: home,
        }
    }
    fn context(&self) -> PrivateExecutionContext {
        PrivateExecutionContext {
            tenant_id: self.runtime.thread_scope.tenant_id.clone(),
            agent_id: Some(self.runtime.thread_scope.agent_id.clone()),
            project_id: self.runtime.thread_scope.project_id.clone(),
            actor: Some(brassclaw_turns::TurnActor::new(
                self.runtime.actor_user_id.clone(),
            )),
        }
    }
    async fn finish(self) {
        shutdown_shared_runtime(self.runtime).await.unwrap();
    }
    async fn original(&self, id: Uuid) -> Definition {
        let catalogue = &self.runtime.installed_catalogue;
        let revision: i64 = self
            .pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT revision FROM reborn_prefix_profiles WHERE name='base-prompt'",
                &[],
            )
            .await
            .unwrap()
            .get(0);
        let target = Target::Profile {
            name: "base-prompt".into(),
            revision: u64::try_from(revision).unwrap(),
        };
        let snapshot = catalogue
            .capture_core_prefix_sources("base-prompt")
            .await
            .unwrap();
        let request =
            json!({"profile":"base-prompt","expected_revision":revision,"mode":"source_only"});
        let input = json!({"profile":"base-prompt","revision":revision.to_string(),"snapshot_bytes":snapshot});
        let preparation = catalogue.capture_task_preparation().await.unwrap();
        Store::new(self.pool.clone())
            .reserve(Reservation {
                id,
                target: &target,
                operation: "generate",
                mode: "source_only",
                request: &request,
                program: catalogue._compiler_program.as_ref().unwrap(),
                input: &input,
                original_input_limits: crate::prefix_job_definitions::captured_input_limits(
                    preparation.policy(),
                )
                .unwrap(),
                origin: &Origin::Private {
                    context: self.context(),
                    preparation: Box::new(preparation),
                },
                created_by: self.runtime.actor_user_id.as_str(),
            })
            .await
            .unwrap()
    }
    async fn definition(&self, id: Uuid) -> Definition {
        Store::new(self.pool.clone())
            .get(id)
            .await
            .unwrap()
            .unwrap()
    }
    async fn ready(&self, id: Uuid) {
        self.runtime
            .installed_catalogue
            .prepare_private_prefix_for_test(self.definition(id).await)
            .await
            .unwrap();
    }
    async fn claim(&self, runner: TurnRunnerId) -> Option<ClaimedPrivateWork> {
        self.store
            .claim_next_private_work(&claim_request(runner), self.reservations.as_ref())
            .await
            .unwrap()
    }
    async fn counts(&self, id: Uuid) -> (String, i64, i64, i64) {
        let row = self.pool.get().await.unwrap().query_one(
            "SELECT status,private_claim_attempts,(SELECT count(*) FROM brassclaw_monty_task_admissions WHERE private_work_id=$1),(SELECT count(*) FROM brassclaw_monty_private_terminals WHERE work_id=$1) FROM brassclaw_turns WHERE work_id=$1", &[&id],
        ).await.unwrap();
        (row.get(0), row.get(1), row.get(2), row.get(3))
    }
    async fn drive(&self, claim: ClaimedPrivateWork) {
        let expected = claim.request().clone();
        let driver = &self.runtime.monty_test_controls.0;
        let waiter = driver
            .private_completion_waiter(claim.attempt().run_id)
            .unwrap();
        let (exit, result) = tokio::time::timeout(Duration::from_secs(60), async {
            tokio::join!(driver.drive_private(Arc::new(claim)), waiter.wait())
        })
        .await
        .expect("actual native task and delivery must finish");
        exit.unwrap();
        let result = result.unwrap();
        let completion = result.internal_completion.unwrap();
        assert_eq!(
            completion.receipt().root,
            self.runtime.global_monty_owner.client().root_identity()
        );
        let mut client = self.pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .unwrap();
        let original = crate::pg_monty_admission::PgMontyAdmission::verify_private_live_completion(
            &tx,
            &completion,
        )
        .await
        .unwrap()
        .unwrap();
        assert!(original.request() == &expected);
        tx.commit().await.unwrap();
    }
}
pub(super) async fn assert_v209_rejects_extra_request_field(pool: &PgPool, id: Uuid) {
    let mut client = pool.get().await.unwrap();
    let original: String = client
        .query_one(
            "SELECT private_request_bytes FROM brassclaw_turns WHERE work_id=$1",
            &[&id],
        )
        .await
        .unwrap()
        .get(0);
    for actor_extra in [false, true] {
        let mut changed: serde_json::Value = serde_json::from_str(&original).unwrap();
        if actor_extra {
            changed["context"]["actor"]
                .as_object_mut()
                .expect("genuine retained actor")
                .insert("unrecognized_actor_field".into(), json!(true));
        } else {
            changed["context"]["unrecognized_context_field"] = json!(true);
        }
        let tx = client.transaction().await.unwrap();
        let error = tx.execute(
            "UPDATE brassclaw_turns SET private_request_bytes=$2,
             private_request_checksum=sha256(convert_to($2,'UTF8')),version=version+1 WHERE work_id=$1",
            &[&id, &changed.to_string()]).await.unwrap_err();
        let error = error
            .as_db_error()
            .expect("actual PostgreSQL guard rejection");
        assert_eq!(error.code().code(), "23514");
        assert_eq!(error.message(), "private request identity is invalid");
        tx.rollback().await.unwrap();
    }
    assert_eq!(
        client
            .query_one(
                "SELECT private_request_bytes FROM brassclaw_turns WHERE work_id=$1",
                &[&id]
            )
            .await
            .unwrap()
            .get::<_, String>(0),
        original
    );
}

fn claim_request(runner_id: TurnRunnerId) -> ClaimRunRequest {
    ClaimRunRequest {
        runner_id,
        lease_token: TurnLeaseToken::new(),
        scope_filter: None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_prepare_claim_cas_and_chat_execution_collision_preserve_one_original() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let fixture = Fixture::start(&rig, rig.pool.clone()).await;
    let id = Uuid::new_v4();
    fixture.original(id).await;
    let a = fixture.definition(id).await;
    let b = fixture.definition(id).await;
    let catalogue = &fixture.runtime.installed_catalogue;
    let (a, b) = tokio::join!(
        catalogue.prepare_private_prefix_for_test(a),
        catalogue.prepare_private_prefix_for_test(b)
    );
    a.unwrap();
    b.unwrap();
    // A repeated producer reads its same committed original, never a new seal.
    fixture.ready(id).await;
    let state = fixture
        .store
        .get_private_work_state(id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.status, PrivateWorkStatus::Ready);
    assert_eq!(state.version, 2);
    let mut conflict = state.request.clone();
    let brassclaw_turns::private_work::PrivateWorkRequest::V1 {
        original_workflow_checksum,
        ..
    } = &mut conflict;
    *original_workflow_checksum = Some([19; 32]);
    assert!(
        fixture
            .store
            .submit_private_work(
                &conflict,
                brassclaw_turns::private_work::PrivatePreparationReason::PreparationPending,
                catalogue.as_ref()
            )
            .await
            .is_err()
    );
    let mut client = fixture.pool.get().await.unwrap();
    let tx = client.transaction().await.unwrap();
    let seal: Vec<u8> = tx
        .query_one(
            "SELECT checksum FROM brassclaw_monty_private_preparations WHERE work_id=$1",
            &[&id],
        )
        .await
        .unwrap()
        .get(0);
    assert!(
        fixture
            .store
            .seal_private_work_in_transaction(
                &tx,
                id,
                1,
                state.request_checksum,
                seal.try_into().unwrap()
            )
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
    drop(client);
    let (a, b) = tokio::join!(
        fixture.claim(TurnRunnerId::new()),
        fixture.claim(TurnRunnerId::new())
    );
    let claim = match (a, b) {
        (Some(a), None) | (None, Some(a)) => a,
        _ => panic!("exactly one real claim"),
    };
    assert_eq!(fixture.counts(id).await, ("claimed".into(), 1, 1, 0));
    catalogue
        .read_private_prefix_original(&fixture.definition(id).await)
        .await
        .unwrap();
    assert_eq!(fixture.reservations.originals.lock().unwrap().len(), 1);
    fixture.drive(claim).await;
    assert_eq!(fixture.counts(id).await, ("settled".into(), 1, 1, 1));
    catalogue
        .read_private_prefix_original(&fixture.definition(id).await)
        .await
        .unwrap();

    let cancelled = Uuid::new_v4();
    fixture.original(cancelled).await;
    fixture.ready(cancelled).await;
    fixture.store.cancel_private_work(cancelled).await.unwrap();
    assert!(fixture.claim(TurnRunnerId::new()).await.is_none());
    assert_eq!(
        fixture.counts(cancelled).await,
        ("cancelled".into(), 0, 0, 0)
    );

    // This is a genuine ordinary inbound message/queued chat, used solely to
    // test the global execution-ID namespace before either origin has effects.
    let chat = fixture.runtime.new_conversation().await.unwrap();
    let send_lock = fixture.runtime.send_lock_for(&chat).await;
    let send_guard = send_lock.lock().await;
    let admitted = fixture
        .runtime
        .admit_user_message_internal(
            &chat,
            "real collision fixture",
            &CancellationToken::new(),
            None,
            None,
        )
        .await
        .unwrap();
    drop(send_guard);
    let collision = admitted.run_id.as_uuid();
    fixture.original(collision).await;
    assert!(
        catalogue
            .prepare_private_prefix_for_test(fixture.definition(collision).await)
            .await
            .is_err()
    );
    let row = fixture.pool.get().await.unwrap().query_one("SELECT (SELECT count(*) FROM brassclaw_turns WHERE record_kind='chat_run_address' AND execution_run_id=$1),(SELECT count(*) FROM brassclaw_turns WHERE record_kind='private_work' AND execution_run_id=$1),(SELECT count(*) FROM brassclaw_monty_task_admissions WHERE run_id=$1)", &[&collision]).await.unwrap();
    assert_eq!(
        (
            row.get::<_, i64>(0),
            row.get::<_, i64>(1),
            row.get::<_, i64>(2)
        ),
        (1, 0, 0)
    );
    fixture.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_ready_and_claim_lost_commit_ack_keep_original_native_reservation() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let id = Uuid::new_v4();
    let intent_lost = Uuid::new_v4();
    let runner = TurnRunnerId::new();
    let wire = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![
            AckCutRule::after_bind(
                1,
                vec![
                    id.as_bytes().to_vec(),
                    b"private-work-definition/2".to_vec(),
                ],
                vec![],
                AckBoundary::Commit,
            ),
            AckCutRule::after_bind(
                2,
                vec![id.as_bytes().to_vec(), runner.as_uuid().as_bytes().to_vec()],
                vec![],
                AckBoundary::Commit,
            ),
            AckCutRule::after_bind(
                3,
                vec![intent_lost.as_bytes().to_vec()],
                vec![],
                AckBoundary::Commit,
            ),
        ],
    )
    .await
    .unwrap();
    let fixture = Fixture::start(&rig, wire.pool.clone()).await;
    fixture.original(id).await;
    wire.arm(1).unwrap();
    assert!(
        fixture
            .runtime
            .installed_catalogue
            .prepare_private_prefix_for_test(fixture.definition(id).await)
            .await
            .is_err()
    );
    assert_eq!(wire.cut_count(), 1);
    let before: String = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT definition_bytes FROM brassclaw_monty_private_preparations WHERE work_id=$1",
            &[&id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(fixture.counts(id).await, ("ready".into(), 0, 0, 0));
    fixture.ready(id).await;
    let after: String = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT definition_bytes FROM brassclaw_monty_private_preparations WHERE work_id=$1",
            &[&id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(before, after);
    wire.arm(2).unwrap();
    assert!(
        fixture
            .store
            .claim_next_private_work(&claim_request(runner), fixture.reservations.as_ref())
            .await
            .is_err()
    );
    assert_eq!(wire.cut_count(), 2);
    let reservation = fixture.reservations.originals.lock().unwrap()[0].clone();
    let original = reservation.original_claim().unwrap().clone();
    let host = fixture.reservations.owner.private_host(&original).unwrap();
    let readback = rig.pool.get().await.unwrap().query_one("SELECT w.status,w.private_runner_id,w.private_claim_checksum,a.private_submission_intent_at IS NULL FROM brassclaw_turns w JOIN brassclaw_monty_task_admissions a ON a.private_work_id=w.work_id WHERE w.work_id=$1", &[&id]).await.unwrap();
    assert_eq!(readback.get::<_, &str>(0), "claimed");
    assert_eq!(readback.get::<_, Uuid>(1), runner.as_uuid());
    use sha2::Digest as _;
    assert_eq!(
        readback.get::<_, Vec<u8>>(2),
        sha2::Sha256::digest(serde_json::to_vec(&original.attempt().lease_token).unwrap()).to_vec()
    );
    assert!(readback.get::<_, bool>(3));
    assert!(fixture.claim(runner).await.is_none());
    assert!(fixture.claim(TurnRunnerId::new()).await.is_none());
    assert!(Arc::ptr_eq(
        &host,
        &fixture.reservations.owner.private_host(&original).unwrap()
    ));
    assert_eq!(fixture.reservations.originals.lock().unwrap().len(), 1);
    // Explicit test-supervisor continuation only after authentic original
    // readback: same in-memory handle, original claim and first native dispatch.
    fixture.drive(original).await;
    assert_eq!(fixture.counts(id).await, ("settled".into(), 1, 1, 1));
    assert!(fixture.claim(runner).await.is_none());
    fixture.original(intent_lost).await;
    fixture.ready(intent_lost).await;
    let claim = Arc::new(fixture.claim(runner).await.unwrap());
    let driver = &fixture.runtime.monty_test_controls.0;
    let pause = driver.pause_private_before_submit(claim.attempt().run_id);
    let waiter = driver
        .private_completion_waiter(claim.attempt().run_id)
        .unwrap();
    let interrupt = async {
        pause.reached.notified().await;
        wire.arm(3).unwrap();
        pause.release.notify_one();
    };
    let (drive, ()) = tokio::time::timeout(Duration::from_secs(60), async {
        tokio::join!(driver.drive_private(claim.clone()), interrupt)
    })
    .await
    .expect("actual submission-intent ACK loss must finish without replay");
    drive.unwrap();
    assert_eq!(wire.cut_count(), 3);
    let result = waiter.wait().await.unwrap();
    assert!(matches!(
        result.exit,
        brassclaw_turns::LoopExit::Cancelled(_)
    ));
    assert!(result.internal_completion.is_none());
    let row = rig.pool.get().await.unwrap().query_one("SELECT private_submission_intent_at IS NOT NULL,started_at IS NULL FROM brassclaw_monty_task_admissions WHERE run_id=$1", &[&intent_lost]).await.unwrap();
    assert!(row.get::<_, bool>(0));
    assert!(row.get::<_, bool>(1));
    assert_eq!(
        fixture.counts(intent_lost).await,
        ("settled".into(), 1, 1, 1)
    );
    assert!(fixture.claim(runner).await.is_none());
    assert!(driver.drive_private(claim).await.is_err());
    assert_eq!(
        fixture.counts(intent_lost).await,
        ("settled".into(), 1, 1, 1)
    );
    fixture.finish().await;
    wire.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_cancel_before_submit_and_running_native_task_keep_exact_terminals() {
    private_native_terminals(false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_prefix_v207_to_v209_upgrade_checks_real_originals_and_both_native_terminals() {
    private_native_terminals(true).await;
}

async fn private_native_terminals(upgrade: bool) {
    let rig = if upgrade {
        crate::runtime::test_pg::pg_rig_at_version(207).await
    } else {
        crate::runtime::test_pg::pg_rig().await
    };
    if upgrade {
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
    let fixture = Fixture::start(&rig, rig.pool.clone()).await;
    let driver = fixture.runtime.monty_test_controls.0.clone();
    if upgrade {
        // Runtime startup applies the real compiled successor via run_migrations.
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
        let completed = Uuid::new_v4();
        fixture.original(completed).await;
        fixture.ready(completed).await;
        let claim = fixture.claim(TurnRunnerId::new()).await.unwrap();
        fixture.drive(claim).await;
        assert_eq!(fixture.counts(completed).await, ("settled".into(), 1, 1, 1));
        let row = rig.pool.get().await.unwrap().query_one(
            "SELECT terminal_bytes::jsonb->'completion'->>'completion_origin',terminal_bytes::jsonb->'completion'->'outcome'->>'status'
             FROM brassclaw_monty_private_terminals WHERE work_id=$1", &[&completed]).await.unwrap();
        assert_eq!(
            (row.get::<_, &str>(0), row.get::<_, &str>(1)),
            ("native_receipt", "internal_completed")
        );
        assert_v209_rejects_extra_request_field(&rig.pool, completed).await;
    }

    let id = Uuid::new_v4();
    fixture.original(id).await;
    fixture.ready(id).await;
    let claim = Arc::new(fixture.claim(TurnRunnerId::new()).await.unwrap());
    let pause = driver.pause_private_before_submit(claim.attempt().run_id);
    let waiter = driver
        .private_completion_waiter(claim.attempt().run_id)
        .unwrap();
    let cancel = async {
        pause.reached.notified().await;
        fixture.store.cancel_private_work(id).await.unwrap();
        pause.release.notify_one();
    };
    let (drive, ()) = tokio::time::timeout(Duration::from_secs(60), async {
        tokio::join!(driver.drive_private(claim.clone()), cancel)
    })
    .await
    .expect("actual pre-submit cancellation must acknowledge");
    drive.unwrap();
    let result = waiter.wait().await.unwrap();
    assert!(matches!(
        result.exit,
        brassclaw_turns::LoopExit::Cancelled(_)
    ));
    assert!(result.internal_completion.is_none());
    assert_eq!(fixture.counts(id).await, ("settled".into(), 1, 1, 1));
    let row = fixture.pool.get().await.unwrap().query_one("SELECT a.started_at IS NULL,a.private_submission_intent_at IS NULL,t.terminal_bytes::jsonb->'completion'->>'completion_origin',(SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1),(SELECT count(*) FROM reborn_validator_q1_runs WHERE run_id=$1) FROM brassclaw_monty_task_admissions a JOIN brassclaw_monty_private_terminals t USING(run_id) WHERE a.run_id=$1", &[&id]).await.unwrap();
    assert!(row.get::<_, bool>(0));
    assert!(row.get::<_, bool>(1));
    assert_eq!(row.get::<_, &str>(2), "never_submitted");
    assert_eq!((row.get::<_, i64>(3), row.get::<_, i64>(4)), (0, 0));
    assert!(fixture.claim(TurnRunnerId::new()).await.is_none());

    let running = Uuid::new_v4();
    fixture.original(running).await;
    fixture.ready(running).await;
    let claim = Arc::new(fixture.claim(TurnRunnerId::new()).await.unwrap());
    let host = fixture.reservations.owner.private_host(&claim).unwrap();
    let mut blocker = rig.pool.get().await.unwrap();
    let pid: i32 = blocker
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .unwrap()
        .get(0);
    let tx = blocker.transaction().await.unwrap();
    tx.batch_execute("LOCK TABLE brassclaw_monty_recipe_selections IN SHARE MODE")
        .await
        .unwrap();
    let waiter = driver
        .private_completion_waiter(claim.attempt().run_id)
        .unwrap();
    let observer = async {
        tokio::time::timeout(Duration::from_secs(30),async {
            loop {
                let blocked: bool = rig.pool.get().await.unwrap().query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND $1=ANY(pg_blocking_pids(pid)) AND query LIKE '%INSERT INTO brassclaw_monty_recipe_selections%')", &[&pid]).await.unwrap().get(0);
                if blocked { break; } tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.unwrap();
        let started: bool = rig.pool.get().await.unwrap().query_one("SELECT phase='started' AND private_submission_intent_at IS NOT NULL FROM brassclaw_monty_task_admissions WHERE run_id=$1", &[&running]).await.unwrap().get(0);
        assert!(started, "cancel a genuinely admitted running native task");
        assert!(fixture.claim(TurnRunnerId::new()).await.is_none());
        let state = fixture
            .store
            .get_private_work_state(running)
            .await
            .unwrap()
            .unwrap();
        let mut client = fixture.pool.get().await.unwrap();
        let no_replay = client.transaction().await.unwrap();
        assert!(
            fixture
                .store
                .seal_private_work_in_transaction(
                    &no_replay,
                    running,
                    i64::try_from(state.version).unwrap(),
                    claim.request_checksum(),
                    claim.definition_checksum()
                )
                .await
                .is_err(),
            "even this original's current metadata cannot return started Work to Ready"
        );
        no_replay.rollback().await.unwrap();
        drop(client);
        fixture.store.cancel_private_work(running).await.unwrap();
        let stop = driver.stop_attempt(claim.attempt());
        tokio::pin!(stop);
        // Poll the actual addressed cancellation first: dispatch is fenced
        // synchronously before releasing the blocked genuine host transaction.
        let early = futures::poll!(stop.as_mut());
        assert!(host.check_dispatch_open().is_err());
        tx.rollback().await.unwrap();
        match early {
            std::task::Poll::Ready(result) => result.unwrap(),
            std::task::Poll::Pending => stop.await.unwrap(),
        }
    };
    let (drive, ()) = tokio::time::timeout(Duration::from_secs(60), async {
        tokio::join!(driver.drive_private(claim.clone()), observer)
    })
    .await
    .expect("actual native running cancellation must acknowledge");
    drive.unwrap();
    let result = waiter.wait().await.unwrap();
    assert!(matches!(
        result.exit,
        brassclaw_turns::LoopExit::Cancelled(_)
    ));
    assert!(result.internal_completion.is_none());
    assert_eq!(fixture.counts(running).await, ("settled".into(), 1, 1, 1));
    let row = fixture.pool.get().await.unwrap().query_one("SELECT t.terminal_bytes::jsonb->'completion'->>'completion_origin',a.outcome->>'status',a.outcome->>'reason_kind',(SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1),(SELECT count(*) FROM reborn_validator_q1_runs WHERE run_id=$1) FROM brassclaw_monty_task_admissions a JOIN brassclaw_monty_private_terminals t USING(run_id) WHERE a.run_id=$1", &[&running]).await.unwrap();
    assert_eq!(
        (
            row.get::<_, &str>(0),
            row.get::<_, &str>(1),
            row.get::<_, &str>(2)
        ),
        ("native_receipt", "failed", "task_cancelled")
    );
    assert_eq!((row.get::<_, i64>(3), row.get::<_, i64>(4)), (0, 0));
    assert!(fixture.claim(TurnRunnerId::new()).await.is_none());
    assert!(
        driver.drive_private(claim).await.is_err(),
        "never replay a terminal original"
    );
    fixture.finish().await;
}

async fn blocked_original_writers(pool: &PgPool, blocker: i32, count: i64) {
    tokio::time::timeout(Duration::from_secs(30),async {
        loop {
            let actual: i64 = pool.get().await.unwrap().query_one("SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND $1=ANY(pg_blocking_pids(pid)) AND query LIKE '%pg_advisory_xact_lock(hashtextextended%'",&[&blocker]).await.unwrap().get(0);
            if actual == count { break; } tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_same_id_capture_race_ready_reconstruction_and_worker_fallback_keep_original_bytes()
{
    use crate::installed_monty_catalogue::{InstalledCatalogueStartup, InstalledMontyCatalogue};
    use brassclaw_product_workflow::WebUiAuthenticatedCaller;
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let fixture = Fixture::start(&rig, rig.pool.clone()).await;
    let catalogue = &fixture.runtime.installed_catalogue;
    let id = Uuid::new_v4();
    let revision: i64 = fixture
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT revision FROM reborn_prefix_profiles WHERE name='base-prompt'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    let revision = u64::try_from(revision).unwrap();
    let source = catalogue
        .capture_core_prefix_sources("base-prompt")
        .await
        .unwrap();
    let caller = WebUiAuthenticatedCaller::new(
        fixture.context().tenant_id,
        fixture.runtime.actor_user_id.clone(),
        fixture.context().agent_id,
        fixture.context().project_id,
    );
    let settings = fixture.runtime.webui_monty_settings_store();
    let before = settings.get("default", "default").await.unwrap();
    let preparation_a = catalogue.capture_task_preparation().await.unwrap();
    let mut blocker = rig.pool.get().await.unwrap();
    let pid: i32 = blocker
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .unwrap()
        .get(0);
    let tx = blocker.transaction().await.unwrap();
    tx.query_one(
        "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
        &[&format!("brassclaw-prefix-original:{id}")],
    )
    .await
    .unwrap();
    let first = {
        let runtime = fixture.runtime.clone();
        let source = source.clone();
        let caller = caller.clone();
        tokio::spawn(async move {
            runtime
                .submit_prefix_generation_snapshot(id, "base-prompt", revision, &source, caller)
                .await
        })
    };
    blocked_original_writers(&rig.pool, pid, 1).await;
    // Both requests originally saw absent metadata. Only internal ACK settings
    // change; this second offered input does not fit the smaller fresh limits.
    let mut smaller = before.execution_limits;
    smaller.max_value_bytes = u64::try_from(source.len() / 2).unwrap();
    smaller.validate().unwrap();
    let changed = settings
        .upsert(
            "default",
            "default",
            &serde_json::from_value(
                json!({"expected_revision":before.revision,"execution_limits":smaller}),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let preparation_b = catalogue.capture_task_preparation().await.unwrap();
    assert_eq!(preparation_b.revision(), changed.revision);
    assert_ne!(preparation_a.bytes(), preparation_b.bytes());
    let second = {
        let runtime = fixture.runtime.clone();
        let source = source.clone();
        let caller = caller.clone();
        tokio::spawn(async move {
            runtime
                .submit_prefix_generation_snapshot(id, "base-prompt", revision, &source, caller)
                .await
        })
    };
    blocked_original_writers(&rig.pool, pid, 2).await;
    tx.rollback().await.unwrap();
    drop(blocker);
    assert_eq!(first.await.unwrap().unwrap(), id);
    assert_eq!(second.await.unwrap().unwrap(), id);
    let original = fixture.definition(id).await;
    let Origin::Private { preparation, .. } = &original.origin else {
        panic!("actual private original")
    };
    assert_eq!(preparation.bytes(), preparation_a.bytes());
    let program_bytes = original.program_bytes.clone();
    let input_bytes = original.input_bytes.clone();
    assert_eq!(original.input["snapshot_bytes"], source);
    let ready_bytes: String = fixture
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT definition_bytes FROM brassclaw_monty_private_preparations WHERE work_id=$1",
            &[&id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(fixture.counts(id).await, ("ready".into(), 0, 0, 0));
    // L-01: the detached /2 includes the actual legacy root policy, distinct
    // from this Work's acknowledged preparation. Authentic checksums alone do
    // not allow a self-declared policy to relabel the retained root.
    {
        use crate::private_work_definition::{PrivateWorkDefinition, checksum};
        let state = fixture
            .store
            .get_private_work_state(id)
            .await
            .unwrap()
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&ready_bytes).unwrap();
        assert_eq!(value["format"], "private-work-definition/2");
        assert!(value["root_preparation_bytes"].is_null());
        assert!(value["root_preparation_checksum"].is_null());
        let mut client = fixture.pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .unwrap();
        let detached = PrivateWorkDefinition::restore(
            &tx,
            &state.request,
            state.request_checksum,
            &ready_bytes,
            checksum(&ready_bytes),
        )
        .await
        .unwrap();
        assert_eq!(detached.bytes(), ready_bytes);
        let legacy = ready_bytes
            .replace("private-work-definition/2", "private-work-definition/1")
            .replace(
                ",\"root_preparation_bytes\":null,\"root_preparation_checksum\":null",
                "",
            );
        let old = PrivateWorkDefinition::restore(
            &tx,
            &state.request,
            state.request_checksum,
            &legacy,
            checksum(&legacy),
        )
        .await
        .unwrap();
        assert_eq!(old.root().reference(), detached.root().reference());
        let relabelled = ready_bytes
            .replace(
                "\"root_preparation_bytes\":null",
                &format!(
                    "\"root_preparation_bytes\":{}",
                    serde_json::to_string(preparation_a.bytes()).unwrap()
                ),
            )
            .replace(
                "\"root_preparation_checksum\":null",
                &format!(
                    "\"root_preparation_checksum\":{}",
                    serde_json::to_string(preparation_a.checksum()).unwrap()
                ),
            );
        let forged = PrivateWorkDefinition::restore(
            &tx,
            &state.request,
            state.request_checksum,
            &relabelled,
            checksum(&relabelled),
        )
        .await;
        assert!(forged.is_err());
        tx.commit().await.unwrap();
    }
    // A changed explicit payload/owner and a Chat origin must still conflict.
    let target = &original.target;
    let input = json!({"profile":"base-prompt","revision":revision.to_string(),"snapshot_bytes":"changed explicit source"});
    assert!(
        Store::new(fixture.pool.clone())
            .reserve(Reservation {
                id,
                target,
                operation: "generate",
                mode: "source_only",
                request: &original.request,
                program: catalogue._compiler_program.as_ref().unwrap(),
                input: &input,
                original_input_limits: original.original_input_limits,
                origin: &original.origin,
                created_by: &original.created_by
            })
            .await
            .is_err()
    );
    let offered_b = Origin::Private {
        context: fixture.context(),
        preparation: Box::new(preparation_b.clone()),
    };
    assert!(
        original.origin != offered_b,
        "full capture equality remains strict"
    );
    let reservation = Reservation {
        id,
        target: &original.target,
        operation: "generate",
        mode: "source_only",
        request: &original.request,
        program: catalogue._compiler_program.as_ref().unwrap(),
        input: &original.input,
        original_input_limits: original.original_input_limits,
        origin: &original.origin,
        created_by: &original.created_by,
    };
    let mut client = fixture.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let existing = Store::reserve_private_in_transaction(
        &tx,
        Reservation {
            origin: &offered_b,
            ..reservation
        },
    )
    .await
    .unwrap();
    assert!(!existing.newly_retained());
    assert!(
        catalogue
            .prepare_private_prefix_in_transaction(&tx, &existing)
            .await
            .is_err()
    );
    catalogue
        .read_private_prefix_original_in_transaction(&tx, existing.definition())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    drop(client);
    let mut foreign_context = fixture.context();
    foreign_context.actor = Some(brassclaw_turns::TurnActor::new(
        brassclaw_host_api::UserId::new("other-original-owner").unwrap(),
    ));
    let foreign = Origin::Private {
        context: foreign_context,
        preparation: preparation.clone(),
    };
    assert!(matches!(
        Store::new(fixture.pool.clone())
            .reserve(Reservation {
                origin: &foreign,
                created_by: "other-original-owner",
                ..reservation
            })
            .await,
        Err(crate::prefix_job_definitions::Error::Conflict)
    ));
    let chat = fixture.runtime.new_conversation().await.unwrap();
    let chat_origin = Origin::Chat {
        scope: fixture.runtime.turn_scope_for(&chat.0),
        thread_scope: fixture.runtime.thread_scope.clone(),
    };
    assert!(matches!(
        Store::new(fixture.pool.clone())
            .reserve(Reservation {
                origin: &chat_origin,
                ..reservation
            })
            .await,
        Err(crate::prefix_job_definitions::Error::Conflict)
    ));
    let other_target = Target::Profile {
        name: "base-prompt".into(),
        revision: revision + 1,
    };
    let other_request =
        json!({"profile":"base-prompt","expected_revision":revision+1,"mode":"source_only"});
    assert!(matches!(
        Store::new(fixture.pool.clone())
            .reserve(Reservation {
                target: &other_target,
                request: &other_request,
                ..reservation
            })
            .await,
        Err(crate::prefix_job_definitions::Error::Conflict)
    ));
    // The derived-source API also re-reads a concurrent original if its
    // collector reaches a newer smaller limit after the first absence read.
    let derived = Uuid::new_v4();
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    *catalogue.capture_gate.lock().await = Some((entered.clone(), release.clone()));
    let collected = {
        let runtime = fixture.runtime.clone();
        let caller = caller.clone();
        tokio::spawn(async move {
            runtime
                .submit_current_core_prefix_snapshot(derived, "base-prompt", revision, caller)
                .await
        })
    };
    entered.notified().await;
    let winner = Store::new(fixture.pool.clone())
        .reserve(Reservation {
            id: derived,
            target: &original.target,
            operation: "generate",
            mode: "source_only",
            request: &original.request,
            program: catalogue._compiler_program.as_ref().unwrap(),
            input: &original.input,
            original_input_limits: original.original_input_limits,
            origin: &original.origin,
            created_by: &original.created_by,
        })
        .await
        .unwrap();
    let winner_input = winner.input_bytes.clone();
    catalogue
        .prepare_private_prefix_for_test(winner)
        .await
        .unwrap();
    release.notify_one();
    assert_eq!(collected.await.unwrap().unwrap(), derived);
    let derived_original = fixture.definition(derived).await;
    assert_eq!(derived_original.input_bytes, winner_input);
    let Origin::Private {
        preparation: derived_preparation,
        ..
    } = derived_original.origin
    else {
        panic!("private derived original")
    };
    assert_eq!(derived_preparation.bytes(), preparation_a.bytes());
    // Restore live technical capacity before executing, with a later ACK revision.
    let restored = settings.upsert("default","default",&serde_json::from_value(json!({"expected_revision":changed.revision,"execution_limits":before.execution_limits})).unwrap()).await.unwrap();
    assert!(restored.revision > preparation_a.revision());
    let current = settings.get("default", "default").await.unwrap();
    let rebuilt = InstalledMontyCatalogue::boot(
        fixture.pool.clone(),
        catalogue.kernel.clone(),
        catalogue.worker_image.clone(),
        &catalogue.qualification_scope,
        catalogue.memory_mounts.clone(),
        InstalledCatalogueStartup {
            root: catalogue.root_definition.clone(),
            settings: &current,
            executor: catalogue.startup_executor.clone(),
            resource_store: catalogue.resource_store_settings.clone(),
        },
    )
    .await
    .unwrap();
    rebuilt
        .bind_private_work_context(fixture.context())
        .unwrap();
    rebuilt.bind_preparation_settings(settings.clone()).unwrap();
    assert!(!Arc::ptr_eq(&rebuilt, catalogue));
    assert!(Arc::ptr_eq(&rebuilt.worker_image, &catalogue.worker_image));
    assert_ne!(
        rebuilt.capture_task_preparation().await.unwrap().bytes(),
        preparation_a.bytes()
    );
    rebuilt
        .prepare_private_prefix_for_test(fixture.definition(id).await)
        .await
        .unwrap();
    let same_bytes: String = fixture
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT definition_bytes FROM brassclaw_monty_private_preparations WHERE work_id=$1",
            &[&id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(ready_bytes, same_bytes);
    let service = fixture.runtime.global_monty_owner.client();
    let root = service.root_identity();
    let ports = Arc::new(
        OwnedGlobalTaskFactory::new(
            fixture.pool.clone(),
            fixture.runtime.global_monty_owner.ownership_check(),
            Arc::new(rebuilt.clone()),
            service.clone(),
            fixture
                .runtime
                .monty_settings_owner
                .store()
                .recipe_capacity_source(),
            fixture
                .runtime
                .monty_settings_owner
                .store()
                .cancellation_policy_source(),
            None,
        )
        .unwrap(),
    );
    let driver = crate::global_monty_driver::GlobalMontyDriver::new(
        service.clone(),
        fixture.runtime.thread_service.clone(),
        ports.clone(),
    )
    .unwrap();
    let store = PgTurnStateStore::new(fixture.pool.clone(), fixture.context().tenant_id.as_str());
    let claim = Arc::new(
        store
            .claim_next_private_work(&claim_request(TurnRunnerId::new()), ports.as_ref())
            .await
            .unwrap()
            .unwrap(),
    );
    let waiter = driver
        .private_completion_waiter(claim.attempt().run_id)
        .unwrap();
    let (drive, result) = tokio::time::timeout(Duration::from_secs(60), async {
        tokio::join!(driver.drive_private(claim), waiter.wait())
    })
    .await
    .expect("restored original must execute on its retained native worker");
    drive.unwrap();
    assert_eq!(
        result.unwrap().internal_completion.unwrap().receipt().root,
        root
    );
    let original = fixture.definition(id).await;
    assert_eq!(original.program_bytes, program_bytes);
    assert_eq!(original.input_bytes, input_bytes);
    assert_eq!(fixture.counts(id).await, ("settled".into(), 1, 1, 1));
    drop(driver);
    drop(ports);
    let derived_claim = fixture.claim(TurnRunnerId::new()).await.unwrap();
    assert_eq!(derived_claim.request().work_id(), derived);
    fixture.drive(derived_claim).await;
    assert_eq!(fixture.counts(derived).await, ("settled".into(), 1, 1, 1));

    // Release the actual startup gate using the existing real loopback listener.
    // Let the initial queued wake be consumed while there is no Ready work.
    let listener = mcp_provider_acceptance::start_listener(&fixture.runtime).await;
    tokio::time::timeout(Duration::from_secs(30),async {
        loop {
            let scanned: bool = rig.pool.get().await.unwrap().query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND state='idle' AND query LIKE '%SELECT kind,snapshot_thread,scope FROM (%')",&[]).await.unwrap().get(0);
            if scanned { break; } tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.unwrap();
    tokio::time::sleep(Duration::from_millis(250)).await;
    let missed = Uuid::new_v4();
    let waiter = fixture
        .runtime
        .monty_test_controls
        .0
        .private_completion_waiter(TurnRunId::from_uuid(missed))
        .unwrap();
    // Commit the actual production participants without its subsequent wake:
    // the producer may disappear after COMMIT before returning to the caller.
    // The existing Worker's configured 100 ms fallback is the only claim trigger.
    assert!(rebuilt.private_work_wake.get().is_none());
    let preparation = rebuilt.capture_task_preparation().await.unwrap();
    let target = Target::Profile {
        name: "base-prompt".into(),
        revision,
    };
    let request =
        json!({"profile":"base-prompt","expected_revision":revision,"mode":"source_only"});
    let input =
        json!({"profile":"base-prompt","revision":revision.to_string(),"snapshot_bytes":source});
    let origin = Origin::Private {
        context: fixture.context(),
        preparation: Box::new(preparation),
    };
    let mut client = fixture.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let reserved = Store::reserve_private_in_transaction(
        &tx,
        Reservation {
            id: missed,
            target: &target,
            operation: "generate",
            mode: "source_only",
            request: &request,
            program: rebuilt._compiler_program.as_ref().unwrap(),
            input: &input,
            original_input_limits: original.original_input_limits,
            origin: &origin,
            created_by: fixture.runtime.actor_user_id.as_str(),
        },
    )
    .await
    .unwrap();
    assert!(reserved.newly_retained());
    rebuilt
        .prepare_private_prefix_in_transaction(&tx, &reserved)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    drop(client);
    let result = tokio::time::timeout(Duration::from_secs(30), waiter.wait())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.internal_completion.unwrap().receipt().root, root);
    assert_eq!(fixture.counts(missed).await, ("settled".into(), 1, 1, 1));
    listener.shutdown().await;
    drop(rebuilt);
    drop(settings);
    fixture.finish().await;
}

// B03a exercises the real producer transaction. The earlier fixtures above
// deliberately retain historical split originals to test native protocol cuts.
async fn atomic_prefix_request(
    fixture: &Fixture,
) -> (
    u64,
    String,
    brassclaw_product_workflow::WebUiAuthenticatedCaller,
) {
    let revision: i64 = fixture
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT revision FROM reborn_prefix_profiles WHERE name='base-prompt'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    let source = fixture
        .runtime
        .installed_catalogue
        .capture_core_prefix_sources("base-prompt")
        .await
        .unwrap();
    let caller = brassclaw_product_workflow::WebUiAuthenticatedCaller::new(
        fixture.context().tenant_id,
        fixture.runtime.actor_user_id.clone(),
        fixture.context().agent_id,
        fixture.context().project_id,
    );
    (u64::try_from(revision).unwrap(), source, caller)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_prefix_producer_commit_ack_recovers_complete_original_with_one_connection() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let id = Uuid::new_v4();
    let wire = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::after_bind(
            1,
            vec![
                id.as_bytes().to_vec(),
                b"private-work-definition/2".to_vec(),
            ],
            vec![],
            AckBoundary::Commit,
        )],
    )
    .await
    .unwrap();
    let fixture = Fixture::start(&rig, wire.pool.clone()).await;
    let (revision, source, caller) = atomic_prefix_request(&fixture).await;
    fixture.pool.resize(1);
    wire.arm(1).unwrap();
    assert_eq!(
        tokio::time::timeout(
            Duration::from_secs(30),
            fixture.runtime.submit_prefix_generation_snapshot(
                id,
                "base-prompt",
                revision,
                &source,
                caller.clone()
            )
        )
        .await
        .unwrap()
        .unwrap(),
        id
    );
    assert_eq!(wire.cut_count(), 1);
    assert_eq!(fixture.counts(id).await, ("ready".into(), 0, 0, 0));
    let original: String = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT definition_bytes FROM brassclaw_monty_private_preparations WHERE work_id=$1",
            &[&id],
        )
        .await
        .unwrap()
        .get(0);
    fixture.pool.resize(brassclaw_pg::pool::MAX_POOL_SIZE);
    let settings = fixture.runtime.webui_monty_settings_store();
    let before = settings.get("default", "default").await.unwrap();
    let mut smaller = before.execution_limits;
    smaller.max_value_bytes = u64::try_from(source.len() / 2).unwrap();
    settings
        .upsert(
            "default",
            "default",
            &serde_json::from_value(json!({
                "expected_revision":before.revision,"execution_limits":smaller,
            }))
            .unwrap(),
        )
        .await
        .unwrap();
    // A historical read never applies current input limits or profile revision.
    rig.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE reborn_prefix_profiles SET revision=revision+1 WHERE name='base-prompt'",
            &[],
        )
        .await
        .unwrap();
    fixture.pool.resize(1);
    assert_eq!(
        tokio::time::timeout(
            Duration::from_secs(30),
            fixture.runtime.submit_prefix_generation_snapshot(
                id,
                "base-prompt",
                revision,
                &source,
                caller.clone()
            )
        )
        .await
        .unwrap()
        .unwrap(),
        id
    );
    fixture.store.cancel_private_work(id).await.unwrap();
    assert_eq!(fixture.runtime.submit_prefix_generation_snapshot(
        id, "base-prompt", revision, &source, caller.clone(),
    ).await.unwrap(), id);
    assert!(
        fixture
            .runtime
            .submit_prefix_generation_snapshot(
                id,
                "base-prompt",
                revision,
                "changed explicit input",
                caller,
            )
            .await
            .is_err()
    );
    assert_eq!(fixture.counts(id).await, ("cancelled".into(), 0, 0, 0));
    let preserved: String = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT definition_bytes FROM brassclaw_monty_private_preparations WHERE work_id=$1",
            &[&id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(original, preserved);
    assert_eq!(wire.cut_count(), 1);
    fixture.pool.resize(brassclaw_pg::pool::MAX_POOL_SIZE);
    drop(settings);
    fixture.finish().await;
    wire.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_prefix_producer_rolls_back_original_work_definition_and_ready_together() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let fixture = Fixture::start(&rig, rig.pool.clone()).await;
    let (revision, source, caller) = atomic_prefix_request(&fixture).await;
    rig.pool
        .get()
        .await
        .unwrap()
        .batch_execute(
            "CREATE SEQUENCE b03a_reject_reached;
         CREATE FUNCTION b03a_reject_prefix_write() RETURNS trigger LANGUAGE plpgsql AS $$
         BEGIN PERFORM nextval('b03a_reject_reached');
         RAISE EXCEPTION 'actual prefix transaction rejection' USING ERRCODE='23514'; END $$;",
        )
        .await
        .unwrap();
    for (index, (table, trigger)) in [
        (
            "brassclaw_turns",
            "CREATE TRIGGER b03a_reject BEFORE INSERT ON brassclaw_turns FOR EACH ROW WHEN (NEW.record_kind='private_work') EXECUTE FUNCTION b03a_reject_prefix_write()",
        ),
        (
            "brassclaw_monty_private_preparations",
            "CREATE TRIGGER b03a_reject BEFORE INSERT ON brassclaw_monty_private_preparations FOR EACH ROW EXECUTE FUNCTION b03a_reject_prefix_write()",
        ),
        (
            "brassclaw_turns",
            "CREATE TRIGGER b03a_reject BEFORE UPDATE ON brassclaw_turns FOR EACH ROW WHEN (NEW.record_kind='private_work' AND NEW.status='ready') EXECUTE FUNCTION b03a_reject_prefix_write()",
        ),
        (
            "brassclaw_monty_private_preparations",
            "CREATE CONSTRAINT TRIGGER b03a_reject AFTER INSERT ON brassclaw_monty_private_preparations DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION b03a_reject_prefix_write()",
        ),
    ].into_iter().enumerate() {
        let id = Uuid::new_v4();
        rig.pool
            .get()
            .await
            .unwrap()
            .batch_execute(trigger)
            .await
            .unwrap();
        assert!(
            fixture
                .runtime
                .submit_prefix_generation_snapshot(
                    id,
                    "base-prompt",
                    revision,
                    &source,
                    caller.clone(),
                )
                .await
                .is_err()
        );
        let reached = rig.pool.get().await.unwrap().query_one(
            "SELECT last_value,is_called FROM b03a_reject_reached", &[]).await.unwrap();
        assert!(reached.get::<_,bool>(1), "selected PostgreSQL fault boundary was not reached");
        assert_eq!(reached.get::<_,i64>(0), i64::try_from(index+1).unwrap(),
            "exactly one real reject-trigger invocation is required at each selected boundary");
        let row = rig
            .pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT (SELECT count(*) FROM brassclaw_prefix_job_definitions WHERE job_id=$1),
             (SELECT count(*) FROM brassclaw_turns WHERE work_id=$1),
             (SELECT count(*) FROM brassclaw_monty_private_preparations WHERE work_id=$1),
             (SELECT count(*) FROM brassclaw_monty_task_admissions WHERE run_id=$1)",
                &[&id],
            )
            .await
            .unwrap();
        assert_eq!(
            (
                row.get::<_, i64>(0),
                row.get::<_, i64>(1),
                row.get::<_, i64>(2),
                row.get::<_, i64>(3)
            ),
            (0, 0, 0, 0)
        );
        rig.pool
            .get()
            .await
            .unwrap()
            .batch_execute(&format!("DROP TRIGGER b03a_reject ON {table}"))
            .await
            .unwrap();
    }
    rig.pool
        .get()
        .await
        .unwrap()
        .batch_execute(
            "DROP FUNCTION b03a_reject_prefix_write(); DROP SEQUENCE b03a_reject_reached",
        )
        .await
        .unwrap();
    fixture.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_prefix_producer_does_not_complete_historical_absent_or_unsealed_work() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let fixture = Fixture::start(&rig, rig.pool.clone()).await;
    for unsealed in [false, true] {
        let id = Uuid::new_v4();
        let definition = fixture.original(id).await;
        if unsealed {
            use sha2::Digest as _;
            let Origin::Private {
                context,
                preparation,
            } = &definition.origin
            else {
                panic!("private original")
            };
            let request = brassclaw_turns::private_work::PrivateWorkRequest::V1 {
                work_id: id,
                run_id: TurnRunId::from_uuid(id),
                original: brassclaw_turns::private_work::PrivateOriginalRef::PrefixSourceOnly {
                    job_id: id,
                    definition_checksum: definition.original_checksum(),
                },
                context: context.clone(),
                original_preparation_checksum: Some(
                    hex::decode(preparation.checksum())
                        .unwrap()
                        .try_into()
                        .unwrap(),
                ),
                original_workflow_checksum: Some(
                    sha2::Sha256::digest(definition.program_bytes.as_bytes()).into(),
                ),
            };
            fixture
                .store
                .submit_private_work(
                    &request,
                    brassclaw_turns::private_work::PrivatePreparationReason::PreparationPending,
                    fixture.runtime.installed_catalogue.as_ref(),
                )
                .await
                .unwrap();
        }
        let caller = brassclaw_product_workflow::WebUiAuthenticatedCaller::new(
            fixture.context().tenant_id,
            fixture.runtime.actor_user_id.clone(),
            fixture.context().agent_id,
            fixture.context().project_id,
        );
        let Target::Profile { revision, .. } = definition.target else {
            panic!("profile original")
        };
        assert!(
            fixture
                .runtime
                .submit_prefix_generation_snapshot(
                    id,
                    "base-prompt",
                    revision,
                    definition.input["snapshot_bytes"].as_str().unwrap(),
                    caller
                )
                .await
                .is_err()
        );
        assert_eq!(
            fixture.definition(id).await.input_bytes,
            definition.input_bytes
        );
        let row = rig
            .pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT (SELECT count(*) FROM brassclaw_turns WHERE work_id=$1),
             (SELECT count(*) FROM brassclaw_monty_private_preparations WHERE work_id=$1),
             (SELECT count(*) FROM brassclaw_monty_task_admissions WHERE run_id=$1)",
                &[&id],
            )
            .await
            .unwrap();
        assert_eq!(
            (
                row.get::<_, i64>(0),
                row.get::<_, i64>(1),
                row.get::<_, i64>(2)
            ),
            (i64::from(unsealed), 0, 0)
        );
    }
    fixture.finish().await;
}

// G-01 uses the B03a producer, not the historical split-original fixture above.
// The blocked turn Worker cannot claim it. EffectiveSettings::get reads the
// acknowledged watch view (no SQL); the background settings source uses SELECT,
// and post-turn delivery has neither a configured model nor an event here.
// Ownership roundtrips use a permanently detached session. The rule additionally
// requires the original Bind, this backend PID and this exact frontend START;
// an unrelated/default/read-only transaction cannot trigger the BEGIN hold.
fn producer_hold_rule(id: Uuid, boundary: AckBoundary) -> AckCutRule {
    let required = if boundary == AckBoundary::Begin {
        vec![id.as_bytes().to_vec()]
    } else {
        vec![
            id.as_bytes().to_vec(),
            b"private-work-definition/2".to_vec(),
        ]
    };
    let rule = AckCutRule::after_bind(1, required, vec![], boundary).hold();
    if boundary == AckBoundary::Begin {
        rule.following_query(b"START TRANSACTION ISOLATION LEVEL REPEATABLE READ\0".to_vec())
    } else {
        rule
    }
}

async fn one_connection_identity(pool: &PgPool) -> (String, i32) {
    tokio::time::timeout(Duration::from_secs(30), async {
        assert_eq!(pool.status().max_size, 1);
        let client = pool.get().await.unwrap();
        let status = pool.status();
        assert_eq!((status.max_size, status.size, status.available), (1, 1, 0));
        let pid = client
            .query_one("SELECT pg_backend_pid()", &[])
            .await
            .unwrap()
            .get(0);
        (brassclaw_pg::PgClient::id(&client).to_string(), pid)
    })
    .await
    .expect("size-one pool must provide an actual usable connection")
}

async fn prefix_atomic_counts(pool: &PgPool, id: Uuid) -> [i64; 6] {
    let row = pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT (SELECT count(*) FROM brassclaw_prefix_job_definitions WHERE job_id=$1),
         (SELECT count(*) FROM brassclaw_turns WHERE work_id=$1),
         (SELECT count(*) FROM brassclaw_monty_private_preparations WHERE work_id=$1),
         (SELECT count(*) FROM brassclaw_monty_task_admissions WHERE run_id=$1),
         (SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1),
         (SELECT count(*) FROM brassclaw_monty_private_terminals WHERE work_id=$1)",
            &[&id],
        )
        .await
        .unwrap();
    std::array::from_fn(|index| row.get(index))
}

async fn prefix_original_bytes(pool: &PgPool, id: Uuid) -> serde_json::Value {
    // Keep complete originals privately in memory; assert only boolean equality
    // so a failed assertion cannot dump source/input/answer bytes into a receipt.
    pool.get()
        .await
        .unwrap()
        .query_one(
            "SELECT jsonb_build_object('job',to_jsonb(d),'preparation',to_jsonb(p),
          'request',w.private_request_bytes,
          'request_checksum',encode(w.private_request_checksum,'hex'),
          'definition_checksum',encode(w.private_definition_checksum,'hex'))
         FROM brassclaw_prefix_job_definitions d
         JOIN brassclaw_turns w ON w.work_id=d.job_id
         JOIN brassclaw_monty_private_preparations p ON p.work_id=d.job_id
         WHERE d.job_id=$1",
            &[&id],
        )
        .await
        .unwrap()
        .get(0)
}

async fn assert_backend_state(
    pool: &PgPool,
    pid: i32,
    in_transaction: bool,
) -> chrono::DateTime<chrono::Utc> {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let row = pool
                .get()
                .await
                .unwrap()
                .query_one(
                    "SELECT state,xact_start IS NOT NULL,backend_start
                 FROM pg_stat_activity WHERE pid=$1",
                    &[&pid],
                )
                .await
                .unwrap();
            let expected = if in_transaction {
                "idle in transaction"
            } else {
                "idle"
            };
            if row.get::<_, &str>(0) == expected && row.get::<_, bool>(1) == in_transaction {
                return row.get(2);
            }
        }
    })
    .await
    .expect("actual held backend must remain connected in its real transaction state")
}

async fn assert_backend_ended(pool: &PgPool, pid: i32, started: chrono::DateTime<chrono::Utc>) {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let exists: bool = pool.get().await.unwrap().query_one(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid=$1 AND backend_start=$2)",
                &[&pid, &started],
            ).await.unwrap().get(0);
            if !exists {
                break;
            }
        }
    })
    .await
    .expect("the original backend must actually terminate, not become an orphaned transaction");
}

async fn assert_hold_server_limits(pool: &PgPool) {
    // The isolated native fixture must not race the test's explicit cancellation
    // with an issuer/database timer. These are observations, not config writes.
    let row = pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT current_setting('statement_timeout'),
         current_setting('idle_in_transaction_session_timeout'),
         current_setting('transaction_timeout',true),version()",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, &str>(0), "0");
    assert_eq!(row.get::<_, &str>(1), "0");
    assert!(
        row.get::<_, Option<&str>>(2)
            .is_none_or(|value| value == "0")
    );
    eprintln!(
        "held ACK native backend: {}; statement/idle-transaction/transaction deadlines disabled",
        row.get::<_, &str>(3)
    );
}

type PrefixProducer = tokio_util::task::AbortOnDropHandle<Result<Uuid, crate::RebornRuntimeError>>;

struct HeldProducer {
    ticket: brassclaw_pg::test_support::AckHoldTicket,
    producer: PrefixProducer,
    identity: (String, i32),
    observation: brassclaw_pg::test_support::AckHoldObservation,
}

async fn finish_prefix_producer(
    mut producer: PrefixProducer,
) -> Result<Uuid, crate::RebornRuntimeError> {
    match tokio::time::timeout(Duration::from_secs(30), &mut producer).await {
        Ok(result) => result.unwrap(),
        Err(_) => {
            producer.abort();
            assert!(producer.await.unwrap_err().is_cancelled());
            panic!("producer deadline elapsed; its task was aborted and joined");
        }
    }
}

async fn start_held_producer(
    fixture: &Fixture,
    wire: &AckCutTransport,
    id: Uuid,
    revision: u64,
    source: String,
    caller: brassclaw_product_workflow::WebUiAuthenticatedCaller,
) -> HeldProducer {
    fixture.pool.resize(1);
    let identity = one_connection_identity(&fixture.pool).await;
    assert!(wire.arm(1).is_err(), "a hold must never use the cut API");
    let mut ticket = wire.arm_hold(1, identity.1).unwrap();
    assert!(
        wire.arm_hold(1, identity.1).is_err(),
        "an active generation cannot be replaced"
    );
    let runtime = fixture.runtime.clone();
    let task = tokio_util::task::AbortOnDropHandle::new(tokio::spawn(async move {
        runtime
            .submit_prefix_generation_snapshot(id, "base-prompt", revision, &source, caller)
            .await
    }));
    let held = tokio::time::timeout(Duration::from_secs(30), ticket.wait_held())
        .await
        .expect("the selected actual producer ACK must be reached")
        .unwrap();
    assert_eq!(held.backend_pid, identity.1);
    assert!(held.session > 0 && held.generation > 0);
    assert_eq!(
        held.following_query_observed,
        held.boundary == AckBoundary::Begin
    );
    assert!(
        !task.is_finished(),
        "no producer completion before its real ACK"
    );
    assert_eq!(
        wire.cut_count(),
        0,
        "holding must not first cut the connection"
    );
    eprintln!(
        "held ACK generation={} session={} backend_pid={} boundary={:?}",
        held.generation, held.session, held.backend_pid, held.boundary
    );
    HeldProducer {
        ticket,
        producer: task,
        identity,
        observation: held,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_prefix_producer_held_begin_cancel_discards_unconfirmed_client() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let id = Uuid::new_v4();
    let wire = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![producer_hold_rule(id, AckBoundary::Begin)],
    )
    .await
    .unwrap();
    let fixture = Fixture::start(&rig, wire.pool.clone()).await;
    assert_hold_server_limits(&rig.pool).await;
    let (revision, source, caller) = atomic_prefix_request(&fixture).await;
    let HeldProducer {
        mut ticket,
        producer,
        identity: old,
        observation: held,
    } = start_held_producer(&fixture, &wire, id, revision, source, caller).await;
    let started = assert_backend_state(
        &rig.pool,
        held.backend_pid,
        held.boundary == AckBoundary::Begin,
    )
    .await;
    assert_eq!(prefix_atomic_counts(&rig.pool, id).await, [0; 6]);
    producer.abort();
    assert!(
        producer.await.unwrap_err().is_cancelled(),
        "actual producer future must be dropped"
    );
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(30), ticket.wait_closed())
            .await
            .unwrap()
            .unwrap(),
        brassclaw_pg::test_support::AckHoldEnd::PeerClosed
    );
    assert!(
        ticket.release().await.is_err(),
        "disconnect is not a released ACK"
    );
    // tokio-postgres 0.7.18 queues rollback on BEGIN cancellation. EvidenceClient
    // additionally detaches the unconfirmed client; the rollback need not reach
    // the wire before ClientWrapper aborts its connection task.
    let new = tokio::time::timeout(
        Duration::from_secs(30),
        one_connection_identity(&fixture.pool),
    )
    .await
    .unwrap();
    assert_ne!(old.0, new.0);
    assert_ne!(old.1, new.1);
    assert!(
        Store::new(fixture.pool.clone())
            .get(id)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(prefix_atomic_counts(&fixture.pool, id).await, [0; 6]);
    assert_backend_ended(&rig.pool, old.1, started).await;
    fixture.pool.resize(brassclaw_pg::pool::MAX_POOL_SIZE);
    fixture.finish().await;
    wire.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_prefix_producer_held_commit_cancel_reads_exact_original_without_replay() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let id = Uuid::new_v4();
    let wire = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![producer_hold_rule(id, AckBoundary::Commit)],
    )
    .await
    .unwrap();
    let fixture = Fixture::start(&rig, wire.pool.clone()).await;
    assert_hold_server_limits(&rig.pool).await;
    let (revision, source, caller) = atomic_prefix_request(&fixture).await;
    let HeldProducer {
        mut ticket,
        producer,
        identity: old,
        observation: held,
    } = start_held_producer(
        &fixture,
        &wire,
        id,
        revision,
        source.clone(),
        caller.clone(),
    )
    .await;
    let started = assert_backend_state(
        &rig.pool,
        held.backend_pid,
        held.boundary == AckBoundary::Begin,
    )
    .await;
    assert_eq!(
        prefix_atomic_counts(&rig.pool, id).await,
        [1, 1, 1, 0, 0, 0]
    );
    let original = prefix_original_bytes(&rig.pool, id).await;
    producer.abort();
    assert!(producer.await.unwrap_err().is_cancelled());
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(30), ticket.wait_closed())
            .await
            .unwrap()
            .unwrap(),
        brassclaw_pg::test_support::AckHoldEnd::PeerClosed
    );
    assert!(ticket.release().await.is_err());
    let new = tokio::time::timeout(
        Duration::from_secs(30),
        one_connection_identity(&fixture.pool),
    )
    .await
    .unwrap();
    assert_ne!(old.0, new.0);
    assert_ne!(old.1, new.1);
    assert_backend_ended(&rig.pool, old.1, started).await;
    assert_eq!(
        tokio::time::timeout(
            Duration::from_secs(30),
            fixture.runtime.submit_prefix_generation_snapshot(
                id,
                "base-prompt",
                revision,
                &source,
                caller.clone()
            )
        )
        .await
        .unwrap()
        .unwrap(),
        id
    );
    fixture
        .runtime
        .installed_catalogue
        .read_private_prefix_original(&fixture.definition(id).await)
        .await
        .unwrap();
    assert_eq!(fixture.counts(id).await, ("ready".into(), 0, 0, 0));
    assert_eq!(
        prefix_atomic_counts(&fixture.pool, id).await,
        [1, 1, 1, 0, 0, 0]
    );
    assert!(
        original == prefix_original_bytes(&fixture.pool, id).await,
        "readback must retain all original bytes"
    );
    for (profile, offered) in [
        ("base-prompt", "changed source"),
        ("prefix-maker", source.as_str()),
    ] {
        assert!(matches!(
            fixture
                .runtime
                .submit_prefix_generation_snapshot(id, profile, revision, offered, caller.clone())
                .await,
            Err(crate::RebornRuntimeError::PrefixJob(
                brassclaw_product_workflow::InterceptorConfigServiceError::Conflict
            ))
        ));
    }
    let mut other = caller.clone();
    other.user_id = brassclaw_host_api::UserId::new("held-commit-other-owner").unwrap();
    assert!(matches!(
        fixture
            .runtime
            .submit_prefix_generation_snapshot(id, "base-prompt", revision, &source, other)
            .await,
        Err(crate::RebornRuntimeError::PrefixJob(
            brassclaw_product_workflow::InterceptorConfigServiceError::Conflict
        ))
    ));

    // End the max-one readback phase before exercising the real native owner.
    // This is the first execution of proven-unbegun Ready work, never a replay.
    fixture.pool.resize(brassclaw_pg::pool::MAX_POOL_SIZE);
    let root = fixture.runtime.global_monty_owner.client().root_identity();
    fixture
        .drive(fixture.claim(TurnRunnerId::new()).await.unwrap())
        .await;
    assert_eq!(fixture.counts(id).await, ("settled".into(), 1, 1, 1));
    let observed = prefix_atomic_counts(&fixture.pool, id).await;
    assert_eq!(
        [
            observed[0],
            observed[1],
            observed[2],
            observed[3],
            observed[5]
        ],
        [1; 5]
    );
    assert!(
        observed[4] > 0,
        "native completion must include actual retained invocations"
    );
    let answered: i64 = fixture
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT count(*) FROM brassclaw_monty_tool_invocations
         WHERE run_id=$1 AND phase='answered' AND attempt_count=1",
            &[&id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        answered, observed[4],
        "each original native invocation must have its actual single-attempt answer"
    );
    let invocations = fixture.pool.get().await.unwrap().query_one(
        "SELECT jsonb_agg(to_jsonb(i) ORDER BY invocation_id) FROM brassclaw_monty_tool_invocations i WHERE run_id=$1", &[&id],
    ).await.unwrap().get::<_, serde_json::Value>(0);
    assert_eq!(
        fixture
            .runtime
            .submit_prefix_generation_snapshot(id, "base-prompt", revision, &source, caller)
            .await
            .unwrap(),
        id
    );
    assert!(fixture.claim(TurnRunnerId::new()).await.is_none());
    assert_eq!(prefix_atomic_counts(&fixture.pool, id).await, observed);
    let after = fixture.pool.get().await.unwrap().query_one(
        "SELECT jsonb_agg(to_jsonb(i) ORDER BY invocation_id) FROM brassclaw_monty_tool_invocations i WHERE run_id=$1", &[&id],
    ).await.unwrap().get::<_, serde_json::Value>(0);
    assert!(
        invocations == after,
        "all native invocation IDs/arguments/answers must remain original"
    );
    assert!(original == prefix_original_bytes(&fixture.pool, id).await);
    assert_eq!(
        fixture.runtime.global_monty_owner.client().root_identity(),
        root
    );
    fixture.finish().await;
    wire.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_prefix_producer_held_ack_release_reuses_confirmed_connection() {
    for boundary in [AckBoundary::Begin, AckBoundary::Commit] {
        let rig = crate::runtime::test_pg::pg_rig().await;
        let _db = rig.lock_db().await;
        rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
            .await;
        let id = Uuid::new_v4();
        let wire = AckCutTransport::start(
            rig.url.expose_secret(),
            vec![producer_hold_rule(id, boundary)],
        )
        .await
        .unwrap();
        let fixture = Fixture::start(&rig, wire.pool.clone()).await;
        assert_hold_server_limits(&rig.pool).await;
        let (revision, source, caller) = atomic_prefix_request(&fixture).await;
        let HeldProducer {
            mut ticket,
            producer,
            identity: old,
            observation: held,
        } = start_held_producer(
            &fixture,
            &wire,
            id,
            revision,
            source.clone(),
            caller.clone(),
        )
        .await;
        assert_backend_state(
            &rig.pool,
            held.backend_pid,
            held.boundary == AckBoundary::Begin,
        )
        .await;
        assert_eq!(
            prefix_atomic_counts(&rig.pool, id).await,
            if boundary == AckBoundary::Begin {
                [0; 6]
            } else {
                [1, 1, 1, 0, 0, 0]
            }
        );
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(30), ticket.release())
                .await
                .unwrap()
                .unwrap(),
            held
        );
        assert!(
            ticket.release().await.is_err(),
            "release capability is single use"
        );
        assert_eq!(finish_prefix_producer(producer).await.unwrap(), id);
        assert_eq!(
            one_connection_identity(&fixture.pool).await,
            old,
            "confirmed Commit must recycle its healthy client"
        );
        assert_backend_state(&rig.pool, old.1, false).await;
        let original = prefix_original_bytes(&fixture.pool, id).await;
        let second = Uuid::new_v4();
        assert_eq!(
            tokio::time::timeout(
                Duration::from_secs(30),
                fixture.runtime.submit_prefix_generation_snapshot(
                    second,
                    "base-prompt",
                    revision,
                    &source,
                    caller
                )
            )
            .await
            .unwrap()
            .unwrap(),
            second
        );
        assert_eq!(one_connection_identity(&fixture.pool).await, old);
        assert_eq!(
            prefix_atomic_counts(&fixture.pool, id).await,
            [1, 1, 1, 0, 0, 0]
        );
        assert_eq!(
            prefix_atomic_counts(&fixture.pool, second).await,
            [1, 1, 1, 0, 0, 0]
        );
        assert!(original == prefix_original_bytes(&fixture.pool, id).await);
        // A new arm may reuse a rule ID; an old completed ticket cannot clear it.
        let mut next = wire.arm_hold(1, old.1).unwrap();
        assert!(ticket.release().await.is_err());
        drop(ticket);
        assert_eq!(
            next.status(),
            brassclaw_pg::test_support::AckHoldStatus::Armed
        );
        assert!(wire.arm_hold(1, old.1).is_err());
        fixture.pool.resize(brassclaw_pg::pool::MAX_POOL_SIZE);
        fixture.finish().await;
        wire.shutdown().await.unwrap();
        assert_eq!(
            next.wait_closed().await.unwrap(),
            brassclaw_pg::test_support::AckHoldEnd::Shutdown
        );
        assert!(next.release().await.is_err());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_prefix_producer_held_ack_shutdown_fails_without_fabricated_success() {
    for boundary in [AckBoundary::Begin, AckBoundary::Commit] {
        let rig = crate::runtime::test_pg::pg_rig().await;
        let _db = rig.lock_db().await;
        rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
            .await;
        let id = Uuid::new_v4();
        let wire = AckCutTransport::start(
            rig.url.expose_secret(),
            vec![producer_hold_rule(id, boundary)],
        )
        .await
        .unwrap();
        let fixture = Fixture::start(&rig, wire.pool.clone()).await;
        assert_hold_server_limits(&rig.pool).await;
        let (revision, source, caller) = atomic_prefix_request(&fixture).await;
        let HeldProducer {
            mut ticket,
            producer,
            observation: held,
            ..
        } = start_held_producer(&fixture, &wire, id, revision, source, caller).await;
        let started = assert_backend_state(
            &rig.pool,
            held.backend_pid,
            held.boundary == AckBoundary::Begin,
        )
        .await;
        let expected = if boundary == AckBoundary::Begin {
            [0; 6]
        } else {
            [1, 1, 1, 0, 0, 0]
        };
        assert_eq!(prefix_atomic_counts(&rig.pool, id).await, expected);
        let original = if boundary == AckBoundary::Commit {
            Some(prefix_original_bytes(&rig.pool, id).await)
        } else {
            None
        };
        // Actual full transport shutdown joins every relay, including this hold.
        // It deliberately also loses the runtime's detached ownership session.
        tokio::time::timeout(Duration::from_secs(30), wire.shutdown())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            ticket.wait_closed().await.unwrap(),
            brassclaw_pg::test_support::AckHoldEnd::Shutdown
        );
        assert!(ticket.release().await.is_err());
        assert!(finish_prefix_producer(producer).await.is_err());
        assert_backend_ended(&rig.pool, held.backend_pid, started).await;
        assert_eq!(prefix_atomic_counts(&rig.pool, id).await, expected);
        if let Some(original) = original {
            assert!(
                original == prefix_original_bytes(&rig.pool, id).await,
                "unacknowledged committed original must remain intact without replay"
            );
        }
        // Loss of the instance owner is reported, not laundered into healthy
        // runtime shutdown. This still invokes/join-cleans its real owners.
        let result = tokio::time::timeout(
            Duration::from_secs(60),
            shutdown_shared_runtime(fixture.runtime),
        )
        .await
        .unwrap();
        assert!(
            matches!(result, Err(crate::RebornRuntimeError::InvalidArgument { reason })
            if reason == "global Monty shutdown requires reconciliation")
        );
    }
}

// Child module reuses the real private admission fixture and wire observers.
mod intent_custody {
    use super::*;
    include!("private_intent_custody_tests.rs");
}

mod delivery_snapshot {
    use super::*;
    include!("private_delivery_snapshot_tests.rs");
}
