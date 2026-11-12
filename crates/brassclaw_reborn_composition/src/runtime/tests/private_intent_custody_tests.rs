// Source-authored native cases. No helper-only success substitutes for the
// original Factory/Driver, actual wire response or exact private settlement.
use brassclaw_product_workflow::MontyVmSettingsStore;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Cut {
    Deadline,
    PolicyEdit,
    Fence,
    ConcurrentStop,
    Abort,
    Panic,
    Shutdown,
    WorkerFailure,
    WorkerAndDrainFailure,
    Acknowledged,
    BeforeStart,
    ClosedBeforeStart,
}

async fn settings_for_intent(
    fixture: &Fixture,
    millis: u64,
) -> brassclaw_resources::PgResourceStoreSettings {
    let store = fixture.runtime.monty_settings_owner.store();
    let original = store.get("default", "default").await.unwrap();
    let mut limits = original.execution_limits;
    limits.resource_store_timeout_millis = millis;
    limits.cancellation_ack_timeout_millis = 1000;
    let changed = store
        .upsert(
            "default",
            "default",
            &serde_json::from_value(json!({
                "expected_revision": original.revision, "execution_limits": limits,
            }))
            .unwrap(),
        )
        .await
        .unwrap();
    let selected = store.resource_store_policy_source().snapshot().unwrap();
    assert_eq!(selected.revision, changed.revision);
    assert_eq!(selected.timeout, Duration::from_millis(millis));
    selected
}

async fn original_never_started(pool: &PgPool, id: Uuid, intent: bool, terminal: bool) {
    let row = pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT a.private_submission_intent_at IS NOT NULL,a.started_at IS NULL,
        (SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1),
        (SELECT count(*) FROM reborn_validator_q1_runs WHERE run_id=$1),
        (SELECT count(*) FROM brassclaw_monty_private_terminals WHERE work_id=$1),
        (SELECT count(*) FROM brassclaw_monty_task_admissions WHERE private_work_id=$1),
        w.private_claim_attempts,w.status
        FROM brassclaw_monty_task_admissions a JOIN brassclaw_turns w ON w.work_id=a.private_work_id
        WHERE a.run_id=$1",
            &[&id],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, bool>(0), intent);
    assert!(row.get::<_, bool>(1));
    assert_eq!((row.get::<_, i64>(2), row.get::<_, i64>(3)), (0, 0));
    assert_eq!(row.get::<_, i64>(4), if terminal { 1 } else { 0 });
    assert_eq!((row.get::<_, i64>(5), row.get::<_, i64>(6)), (1, 1));
    assert_eq!(
        row.get::<_, &str>(7),
        if terminal { "settled" } else { "claimed" }
    );
    if terminal {
        let origin: String = pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT terminal_bytes::jsonb->'completion'->>'completion_origin'
            FROM brassclaw_monty_private_terminals WHERE run_id=$1",
                &[&id],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(origin, "never_submitted");
    }
}

async fn intent_exchange(boundary: AckBoundary, cut: Cut) {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let id = Uuid::new_v4();
    let rule = AckCutRule::after_bind(1, vec![id.as_bytes().to_vec()], vec![], boundary).hold();
    let rule = if boundary == AckBoundary::Begin {
        // This caller uses Client::transaction(), not the producer's separate
        // repeatable-read TransactionBuilder START string.
        rule.following_query(b"BEGIN\0".to_vec())
    } else {
        rule
    };
    let wire = AckCutTransport::start(rig.url.expose_secret(), vec![rule])
        .await
        .unwrap();
    let fixture = Fixture::start(&rig, wire.pool.clone()).await;
    assert_hold_server_limits(&rig.pool).await;
    let budget = if matches!(cut, Cut::Deadline | Cut::PolicyEdit) {
        5000
    } else {
        10000
    };
    let selected = settings_for_intent(&fixture, budget).await;
    let settings_before_exchange = fixture
        .runtime
        .monty_settings_owner
        .store()
        .get("default", "default")
        .await
        .unwrap();
    fixture.original(id).await;
    fixture.ready(id).await;
    let originals = prefix_original_bytes(&rig.pool, id).await;
    let claim = Arc::new(fixture.claim(TurnRunnerId::new()).await.unwrap());
    let driver = fixture.runtime.monty_test_controls.0.clone();
    let factory = fixture.reservations.owner.clone();
    let host = factory.private_host(&claim).unwrap();
    let pause = driver.pause_private_before_submit(claim.attempt().run_id);
    let waiter = driver
        .private_completion_waiter(claim.attempt().run_id)
        .unwrap();
    let mut drive = Box::pin(driver.drive_private(claim.clone()));
    tokio::time::timeout(Duration::from_secs(60), async {
        tokio::select! {
            _ = pause.reached.notified() => (),
            _ = &mut drive => panic!("original must reach pre-submit pause"),
        }
    })
    .await
    .unwrap();
    if matches!(cut, Cut::BeforeStart | Cut::ClosedBeforeStart) {
        if cut == Cut::ClosedBeforeStart {
            factory.close_private_intents().await.unwrap();
        } else {
            host.fence_dispatch();
        }
        pause.release.notify_one();
        tokio::time::timeout(Duration::from_secs(30), &mut drive)
            .await
            .unwrap()
            .unwrap();
        let observation = factory
            .test_private_intent_observation(claim.attempt())
            .unwrap();
        // Settlement removes the retained task. Its real terminal additionally
        // proves no intent/start; never infer a physical client from a missing row.
        assert!(observation.is_none());
        original_never_started(&rig.pool, id, false, true).await;
        let result = waiter.wait().await.unwrap();
        assert!(matches!(
            result.exit,
            brassclaw_turns::LoopExit::Cancelled(_)
        ));
        assert!(result.internal_completion.is_none());
        drop(drive);
        fixture.finish().await;
        wire.shutdown().await.unwrap();
        return;
    }
    fixture.pool.resize(1);
    let old = one_connection_identity(&fixture.pool).await;
    let mut ticket = wire.arm_hold(1, old.1).unwrap();
    if boundary == AckBoundary::Begin {
        // Prime only the selector on the actual size-one backend while the
        // original driver is paused before IO; no pause under its row lock.
        let row = fixture
            .pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT work_id FROM brassclaw_turns WHERE work_id=$1",
                &[&id],
            )
            .await
            .unwrap();
        assert_eq!(row.get::<_, Uuid>(0), id);
    }
    pause.release.notify_one();
    let held = tokio::time::timeout(Duration::from_secs(30), async {
        tokio::select! {
            held = ticket.wait_held() => held.unwrap(),
            _ = &mut drive => panic!("original must not complete before selected ACK"),
        }
    })
    .await
    .unwrap();
    assert_eq!(held.backend_pid, old.1);
    assert_eq!(held.boundary, boundary);
    assert_eq!(
        held.following_query_observed,
        boundary == AckBoundary::Begin
    );
    let observed = factory
        .test_private_intent_observation(claim.attempt())
        .unwrap()
        .unwrap();
    assert_eq!(
        observed["last_io_boundary"],
        if boundary == AckBoundary::Begin {
            "begin_started"
        } else {
            "commit_started"
        }
    );
    let started = assert_backend_state(&rig.pool, old.1, boundary == AckBoundary::Begin).await;
    original_never_started(&rig.pool, id, boundary == AckBoundary::Commit, false).await;
    assert_eq!(wire.cut_count(), 0);

    if cut == Cut::PolicyEdit {
        // Permit the existing Settings owner a separate checkout, never enlarge
        // a product pool. The original exchange is already bound to selected.
        fixture.pool.resize(2);
        let changed = settings_for_intent(&fixture, 200).await;
        assert!(changed.revision > selected.revision);
        assert!(
            tokio::time::timeout(Duration::from_millis(400), ticket.wait_closed())
                .await
                .is_err(),
            "the new smaller ACK budget must not replace the original exchange budget"
        );
    }
    match cut {
        Cut::Fence => host.fence_dispatch(),
        Cut::ConcurrentStop => {
            let (a, b) = tokio::time::timeout(Duration::from_secs(3), async {
                tokio::join!(
                    driver.stop_attempt(claim.attempt()),
                    driver.stop_attempt(claim.attempt())
                )
            })
            .await
            .expect("both stops must consume the same original IO completion/proof");
            a.unwrap();
            b.unwrap();
        }
        Cut::Abort | Cut::Panic => {
            factory.interrupt_private_intent_for_test(&host, cut == Cut::Panic)
        }
        Cut::Shutdown | Cut::WorkerFailure | Cut::WorkerAndDrainFailure => {
            let settings = fixture.runtime.monty_settings_owner.store();
            let desired = settings_before_exchange;
            let ownership = fixture.runtime.global_monty_owner.ownership_check();
            if matches!(cut, Cut::WorkerFailure | Cut::WorkerAndDrainFailure) {
                fixture.runtime.worker_handle.abort();
            }
            if cut == Cut::WorkerAndDrainFailure {
                factory.interrupt_private_intent_for_test(&host, false);
            }
            let Fixture { runtime, .. } = fixture;
            let runtime = Arc::try_unwrap(runtime)
                .ok()
                .expect("single runtime lifecycle owner");
            let (result, closed) = tokio::time::timeout(Duration::from_secs(30), async {
                tokio::join!(runtime.shutdown(), async {
                    tokio::time::timeout(Duration::from_secs(3), ticket.wait_closed())
                        .await
                        .expect("active IO must consume the fence before the longer store deadline")
                        .unwrap()
                })
            })
            .await
            .expect("all normal shutdown owners must finish");
            assert_eq!(closed, brassclaw_pg::test_support::AckHoldEnd::PeerClosed);
            if matches!(cut, Cut::WorkerFailure | Cut::WorkerAndDrainFailure) {
                let error = result.unwrap_err().to_string();
                assert!(error.contains("worker_shutdown_cancelled"));
                assert!(!error.contains("private intent test panic"));
                if cut == Cut::WorkerAndDrainFailure {
                    assert!(error.contains("private_intent_shutdown_failed"));
                }
            } else {
                result.unwrap();
            }
            assert!(ownership.is_closed());
            assert!(settings.upsert("default", "default", &serde_json::from_value(json!({
                "expected_revision":desired.revision,"execution_limits":desired.execution_limits,
            })).unwrap()).await.is_err(), "settings owner cleanup must still run");
            assert_backend_ended(&rig.pool, old.1, started).await;
            original_never_started(&rig.pool, id, boundary == AckBoundary::Commit, false).await;
            drop(drive);
            wire.shutdown().await.unwrap();
            return;
        }
        Cut::Acknowledged => {
            ticket.release().await.unwrap();
            factory.before_submit(&host).await.unwrap();
            assert_eq!(
                one_connection_identity(&fixture.pool).await,
                old,
                "an actual acknowledged COMMIT may recycle its original client before service submit"
            );
            host.fence_dispatch();
        }
        _ => (),
    }
    if cut != Cut::Acknowledged {
        // Only this observer is polled here. The original driver/shared future
        // remains parked; neither timeout cleanup nor host-fence cleanup can
        // rely on a stop waiter polling its SQL body.
        let watch = if matches!(cut, Cut::Deadline | Cut::PolicyEdit) {
            15
        } else {
            3
        };
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(watch), ticket.wait_closed())
                .await
                .unwrap()
                .unwrap(),
            brassclaw_pg::test_support::AckHoldEnd::PeerClosed
        );
        assert!(ticket.release().await.is_err());
        assert_backend_ended(&rig.pool, old.1, started).await;
        if cut != Cut::ConcurrentStop {
            let error = factory.before_submit(&host).await.unwrap_err();
            match cut {
                Cut::Deadline | Cut::PolicyEdit => {
                    let text = error.to_string();
                    assert!(text.contains("private_submission_intent"));
                    assert!(text.contains(&format!("\"settings_revision\":{}", selected.revision)));
                    assert!(text.contains(&format!(
                        "\"timeout_millis\":\"{}\"",
                        selected.timeout.as_millis()
                    )));
                }
                Cut::Abort | Cut::Panic => {
                    assert!(
                        matches!(error, brassclaw_turns::run_profile::AgentLoopDriverError::Failed { reason_kind }
                        if reason_kind == if cut == Cut::Abort { "private_submission_intent_join_cancelled" } else { "private_submission_intent_join_panicked" })
                    );
                }
                _ => (),
            }
        }
        fixture.pool.resize(1);
        let new = one_connection_identity(&fixture.pool).await;
        assert_ne!(old.0, new.0);
        assert_ne!(old.1, new.1);
    }
    tokio::time::timeout(Duration::from_secs(30), &mut drive)
        .await
        .unwrap()
        .unwrap();
    let result = waiter.wait().await.unwrap();
    assert!(matches!(
        result.exit,
        brassclaw_turns::LoopExit::Cancelled(_)
    ));
    assert!(result.internal_completion.is_none());
    original_never_started(&rig.pool, id, boundary == AckBoundary::Commit, true).await;
    assert!(prefix_original_bytes(&rig.pool, id).await == originals);
    assert!(fixture.claim(TurnRunnerId::new()).await.is_none());
    assert!(driver.drive_private(claim).await.is_err());
    original_never_started(&rig.pool, id, boundary == AckBoundary::Commit, true).await;
    drop(drive);
    fixture.pool.resize(brassclaw_pg::pool::MAX_POOL_SIZE);
    fixture.finish().await;
    wire.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn held_commit_deadline_progresses_without_submission_shared_poller() {
    intent_exchange(AckBoundary::Commit, Cut::Deadline).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn held_commit_retains_captured_ack_policy_after_live_edit() {
    intent_exchange(AckBoundary::Commit, Cut::PolicyEdit).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn held_begin_and_commit_consume_fence_without_submission_shared_poller() {
    for boundary in [AckBoundary::Begin, AckBoundary::Commit] {
        intent_exchange(boundary, Cut::Fence).await;
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_stops_join_one_original_intent_and_real_service_proof() {
    intent_exchange(AckBoundary::Commit, Cut::ConcurrentStop).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actual_io_abort_and_panic_allow_original_proof_settlement_without_replay() {
    for cut in [Cut::Abort, Cut::Panic] {
        intent_exchange(AckBoundary::Commit, cut).await;
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_drains_held_io_including_after_worker_join_failure() {
    for cut in [
        Cut::Shutdown,
        Cut::WorkerFailure,
        Cut::WorkerAndDrainFailure,
    ] {
        intent_exchange(AckBoundary::Commit, cut).await;
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn acknowledged_intent_recycles_before_service_and_prestart_fence_does_no_io() {
    for cut in [Cut::Acknowledged, Cut::BeforeStart, Cut::ClosedBeforeStart] {
        intent_exchange(AckBoundary::Commit, cut).await;
    }
}

// Pool acquisition and the original claim lock are part of the same budget,
// rather than receiving a fresh deadline at each SQL phase.
async fn intent_contention(hold_pool: bool) {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    // Separate actual runtime pool keeps the independent inspector/blocker usable.
    let pool = Arc::new(
        brassclaw_pg::pool::build_pool_with_max_size(
            rig.url.expose_secret(),
            brassclaw_pg::pool::MAX_POOL_SIZE,
        )
        .unwrap(),
    );
    let fixture = Fixture::start(&rig, pool).await;
    let selected = settings_for_intent(&fixture, 2000).await;
    let id = Uuid::new_v4();
    fixture.original(id).await;
    fixture.ready(id).await;
    let original = prefix_original_bytes(&rig.pool, id).await;
    let claim = Arc::new(fixture.claim(TurnRunnerId::new()).await.unwrap());
    let driver = fixture.runtime.monty_test_controls.0.clone();
    let factory = fixture.reservations.owner.clone();
    let host = factory.private_host(&claim).unwrap();
    let pause = driver.pause_private_before_submit(claim.attempt().run_id);
    let waiter = driver
        .private_completion_waiter(claim.attempt().run_id)
        .unwrap();
    let mut drive = Box::pin(driver.drive_private(claim.clone()));
    tokio::time::timeout(Duration::from_secs(60), async {
        tokio::select! {
            _ = pause.reached.notified() => (),
            _ = &mut drive => panic!("original must reach pre-submit pause"),
        }
    })
    .await
    .unwrap();
    fixture.pool.resize(1);
    let old = one_connection_identity(&fixture.pool).await;
    let pool_guard = if hold_pool {
        Some(fixture.pool.get().await.unwrap())
    } else {
        None
    };
    let mut blocker = rig.pool.get().await.unwrap();
    let blocker_pid: i32 = blocker
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .unwrap()
        .get(0);
    let lock = blocker.transaction().await.unwrap();
    if !hold_pool {
        lock.query_one(
            "SELECT work_id FROM brassclaw_turns WHERE work_id=$1 FOR UPDATE",
            &[&id],
        )
        .await
        .unwrap();
    }
    pause.release.notify_one();
    let blocked = async {
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                let facts = factory.test_private_intent_observation(claim.attempt()).unwrap().unwrap();
                let expected = if hold_pool { "acquire_started" } else { "claim_started" };
                if facts["last_io_boundary"] == expected {
                    if hold_pool { break None; }
                    let rows = rig.pool.get().await.unwrap().query(
                        "SELECT pid,backend_start FROM pg_stat_activity WHERE datname=current_database()
                        AND $1=ANY(pg_blocking_pids(pid))", &[&blocker_pid]
                    ).await.unwrap();
                    if rows.len() == 1 {
                        assert_eq!(rows[0].get::<_, i32>(0), old.1);
                        break Some(rows[0].get::<_, chrono::DateTime<chrono::Utc>>(1));
                    }
                }
                tokio::task::yield_now().await;
            }
        }).await.expect("actual intended pool or original row-lock wait must be reached")
    };
    let started = tokio::select! {
        result = blocked => result,
        _ = &mut drive => panic!("original must not finish before its intended wait"),
    };
    // Park the submission again: only its IO job can consume the store deadline.
    let error = tokio::time::timeout(Duration::from_secs(10), factory.before_submit(&host))
        .await
        .unwrap()
        .unwrap_err();
    let error = error.to_string();
    assert!(error.contains("private_submission_intent"));
    assert!(error.contains(&format!("\"settings_revision\":{}", selected.revision)));
    if let Some(started) = started {
        assert_backend_ended(&rig.pool, old.1, started).await;
        let next = one_connection_identity(&fixture.pool).await;
        assert_ne!(old.0, next.0);
        assert_ne!(old.1, next.1);
    } else {
        // IO never acquired this separately owned client. Do not call its
        // continued existence an IO leak or claim ClientScopeEnded.
        let facts = factory
            .test_private_intent_observation(claim.attempt())
            .unwrap()
            .unwrap();
        assert_eq!(facts["client_scope_ended"], false);
        assert!(pool_guard.is_some());
    }
    original_never_started(&rig.pool, id, false, false).await;
    lock.rollback().await.unwrap();
    drop(blocker);
    drop(pool_guard);
    tokio::time::timeout(Duration::from_secs(30), &mut drive)
        .await
        .unwrap()
        .unwrap();
    let result = waiter.wait().await.unwrap();
    assert!(matches!(
        result.exit,
        brassclaw_turns::LoopExit::Cancelled(_)
    ));
    assert!(result.internal_completion.is_none());
    original_never_started(&rig.pool, id, false, true).await;
    assert!(prefix_original_bytes(&rig.pool, id).await == original);
    assert!(fixture.claim(TurnRunnerId::new()).await.is_none());
    assert!(driver.drive_private(claim).await.is_err());
    drop(drive);
    fixture.pool.resize(brassclaw_pg::pool::MAX_POOL_SIZE);
    fixture.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exhausted_pool_and_original_claim_lock_share_one_captured_deadline() {
    for hold_pool in [true, false] {
        intent_contention(hold_pool).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn close_racing_original_exchange_publication_preserves_proof_and_original() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let fixture = Fixture::start(&rig, rig.pool.clone()).await;
    let id = Uuid::new_v4();
    fixture.original(id).await;
    fixture.ready(id).await;
    let original = prefix_original_bytes(&rig.pool, id).await;
    let claim = Arc::new(fixture.claim(TurnRunnerId::new()).await.unwrap());
    let driver = fixture.runtime.monty_test_controls.0.clone();
    let factory = fixture.reservations.owner.clone();
    let host = factory.private_host(&claim).unwrap();
    let pause = driver.pause_private_before_submit(claim.attempt().run_id);
    let waiter = driver
        .private_completion_waiter(claim.attempt().run_id)
        .unwrap();
    let mut drive = Box::pin(driver.drive_private(claim.clone()));
    tokio::time::timeout(Duration::from_secs(60), async {
        tokio::select! {
            _ = pause.reached.notified() => (),
            _ = &mut drive => panic!("original must reach pre-submit pause"),
        }
    })
    .await
    .unwrap();
    // All three use the same actual original. Close either excludes first
    // publication or fences/joins that one job. The two awaits may not create
    // independent IO even though the outer submission is still parked.
    let (a, b, closed) = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::join!(
            factory.before_submit(&host),
            factory.before_submit(&host),
            factory.close_private_intents()
        )
    })
    .await
    .unwrap();
    assert_eq!(a, b);
    closed.unwrap();
    factory.join_private_intent(&host).await.unwrap();
    let intent: bool = rig.pool.get().await.unwrap().query_one(
        "SELECT private_submission_intent_at IS NOT NULL FROM brassclaw_monty_task_admissions WHERE run_id=$1", &[&id]
    ).await.unwrap().get(0);
    original_never_started(&rig.pool, id, intent, false).await;
    pause.release.notify_one();
    tokio::time::timeout(Duration::from_secs(30), &mut drive)
        .await
        .unwrap()
        .unwrap();
    let result = waiter.wait().await.unwrap();
    assert!(matches!(
        result.exit,
        brassclaw_turns::LoopExit::Cancelled(_)
    ));
    assert!(result.internal_completion.is_none());
    original_never_started(&rig.pool, id, intent, true).await;
    assert!(prefix_original_bytes(&rig.pool, id).await == original);
    assert!(driver.drive_private(claim).await.is_err());
    assert!(fixture.claim(TurnRunnerId::new()).await.is_none());
    drop(drive);
    fixture.finish().await;
}
