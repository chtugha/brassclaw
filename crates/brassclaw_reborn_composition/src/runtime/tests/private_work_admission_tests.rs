//! Native PG/original/claim tests. All reservations, hosts and native receipts
//! come from the existing owners; the observer only retains their actual handle.
use super::*;
use crate::{
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
use std::sync::{Arc, Mutex};
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
            .prepare_private_prefix(self.definition(id).await)
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
        catalogue.prepare_private_prefix(a),
        catalogue.prepare_private_prefix(b)
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
    assert_eq!(fixture.reservations.originals.lock().unwrap().len(), 1);
    fixture.drive(claim).await;
    assert_eq!(fixture.counts(id).await, ("settled".into(), 1, 1, 1));

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
            .prepare_private_prefix(fixture.definition(collision).await)
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
                    b"private-work-definition/1".to_vec(),
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
            .prepare_private_prefix(fixture.definition(id).await)
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
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let fixture = Fixture::start(&rig, rig.pool.clone()).await;
    let driver = fixture.runtime.monty_test_controls.0.clone();
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
    catalogue.prepare_private_prefix(winner).await.unwrap();
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
        .prepare_private_prefix(fixture.definition(id).await)
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
    fixture.original(missed).await;
    let waiter = fixture
        .runtime
        .monty_test_controls
        .0
        .private_completion_waiter(TurnRunId::from_uuid(missed))
        .unwrap();
    // This original producer has no wake binding and calls no runtime wake.
    // The existing Worker's configured 100 ms fallback is the only claim trigger.
    assert!(rebuilt.private_work_wake.get().is_none());
    rebuilt
        .prepare_private_prefix(fixture.definition(missed).await)
        .await
        .unwrap();
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
