// Use the existing actual Source-review/parser Workflow for a nonempty original
// invocation set. Prefix-only work cannot prove an invocation row lock.
use crate::pg_monty_admission::{PgMontyAdmission, PrivateOriginalTerminalStatus};

#[derive(Clone, Copy)]
enum Writer {
    Terminal,
    Delivery,
}

async fn source_writer_retains_invocation_lock(writer: Writer) {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let home = tempfile::tempdir().unwrap();
    let (runtime, frame) = start(&rig, home.path()).await;
    let (id, component) = (Uuid::new_v4(), Uuid::new_v4());
    let run = brassclaw_turns::TurnRunId::from_uuid(id);
    let mut review = OriginalReview::new(&runtime, &frame, id);
    let waiter = runtime
        .monty_test_controls
        .0
        .private_completion_waiter(run)
        .unwrap();
    let pause = match writer {
        Writer::Delivery => Some(runtime.monty_test_controls.1.pause_private_settlement(run)),
        Writer::Terminal => None,
    };
    let mut guard = rig.pool.get().await.unwrap();
    let blocker_pid: i32 = guard
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .unwrap()
        .get(0);
    let guard = guard.transaction().await.unwrap();
    // SHARE holds the actual writer's INSERT while permitting original reads
    // and the ROW SHARE lock needed by the author's foreign-key checks.
    let (lock_sql, insert_prefix) = match writer {
        Writer::Terminal => (
            "LOCK TABLE brassclaw_monty_private_terminals IN SHARE MODE",
            "INSERT INTO brassclaw_monty_private_terminals%",
        ),
        Writer::Delivery => (
            "LOCK TABLE brassclaw_monty_private_deliveries IN SHARE MODE",
            "INSERT INTO brassclaw_monty_private_deliveries%",
        ),
    };
    if matches!(writer, Writer::Terminal) {
        guard.batch_execute(lock_sql).await.unwrap();
    }
    let mut queued = request(id, component, false);
    queued.create_queue_project = Some("snapshot-original-lock".into());
    let receipt = submit(&runtime, &frame, queued).await.unwrap();
    assert_eq!(receipt.queue_ids.len(), 1);
    assert!(!receipt.catalogue_activated);
    if let Some(pause) = pause {
        tokio::time::timeout(frame.duration(), pause.reached.notified())
            .await
            .unwrap();
        // Acquire the delivery-table hold only after real terminal COMMIT. An
        // earlier hold could block the author's original submit/readback itself.
        guard.batch_execute(lock_sql).await.unwrap();
        pause.release.notify_one();
    }
    let writer_pid = tokio::time::timeout(frame.duration(), async {
        loop {
            let rows = rig
                .pool
                .get()
                .await
                .unwrap()
                .query(
                    "SELECT pid FROM pg_stat_activity WHERE datname=current_database()
                AND $1=ANY(pg_blocking_pids(pid)) AND query LIKE $2",
                    &[&blocker_pid, &insert_prefix],
                )
                .await
                .unwrap();
            if !rows.is_empty() {
                assert_eq!(rows.len(), 1);
                break rows[0].get::<_, i32>(0);
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("actual original writer must reach its own held INSERT");
    assert_ne!(writer_pid, blocker_pid);
    let invocations = rig.pool.get().await.unwrap().query(
        "SELECT phase,attempt_count,selection_bytes FROM brassclaw_monty_tool_invocations WHERE run_id=$1",
        &[&id],
    ).await.unwrap();
    assert_eq!(
        invocations.len(),
        1,
        "one actual original parser Tool invocation"
    );
    assert_eq!(invocations[0].get::<_, &str>(0), "answered");
    assert_eq!(invocations[0].get::<_, i16>(1), 1);
    let selected: Value = serde_json::from_str(invocations[0].get(2)).unwrap();
    assert_eq!(
        selected["capability_id"],
        brassclaw_host_runtime::SOURCE_INSPECTION_CAPABILITY_ID
    );
    let mut probe = rig.pool.get().await.unwrap();
    let probe = probe.transaction().await.unwrap();
    let error = probe.query(
        "SELECT invocation_id FROM brassclaw_monty_tool_invocations WHERE run_id=$1 FOR UPDATE NOWAIT",
        &[&id],
    ).await.expect_err("the actual terminal/delivery writer must retain its invocation SHARE lock");
    assert_eq!(error.code().unwrap().code(), "55P03");
    probe.rollback().await.unwrap();
    guard.rollback().await.unwrap();
    // Keep all existing positive-source, structural-only, no-approval and exact
    // parser assertions; metadata locks cannot replace the real native result.
    assert_scope(&runtime, &frame, &mut review, component, true).await;
    assert!(read_positive(&runtime, &frame, id).await.is_some());
    let driven = tokio::time::timeout(frame.duration(), waiter.wait())
        .await
        .unwrap()
        .unwrap();
    let completion = driven
        .internal_completion
        .as_ref()
        .expect("real delivered native result");
    let context = brassclaw_turns::private_work::PrivateExecutionContext {
        tenant_id: runtime.thread_scope.tenant_id.clone(),
        agent_id: Some(runtime.thread_scope.agent_id.clone()),
        project_id: runtime.thread_scope.project_id.clone(),
        actor: Some(brassclaw_turns::TurnActor::new(
            runtime.actor_user_id.clone(),
        )),
    };
    // The old writer released its lock. Hold the actual row independently now;
    // every historical path must read the original under READ ONLY without a
    // new row lock, while retaining all original/checksum/proof comparisons.
    let mut blocker = rig.pool.get().await.unwrap();
    let locked = blocker.transaction().await.unwrap();
    let rows = locked
        .query(
            "SELECT invocation_id FROM brassclaw_monty_tool_invocations WHERE run_id=$1 FOR UPDATE",
            &[&id],
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    let mut client = rig.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    tokio::time::timeout(frame.duration(), async {
        let delivery = PgMontyAdmission::read_private_delivery(&tx, id, &context)
            .await
            .unwrap()
            .unwrap();
        let live = PgMontyAdmission::verify_private_live_completion(&tx, completion)
            .await
            .unwrap()
            .unwrap();
        assert!(delivery.original().request() == live.request());
        assert!(matches!(
            PgMontyAdmission::read_private_terminal_status(&tx, id, &context)
                .await
                .unwrap(),
            Some(PrivateOriginalTerminalStatus::Successful)
        ));
        // This is the actual original UI projection, including H01 readback.
        let view = crate::pg_review_submission::get_with_catalogue(
            &runtime.installed_catalogue.pool,
            &id.to_string(),
            &runtime.installed_catalogue,
        )
        .await
        .unwrap();
        let work = view.work.unwrap();
        assert!(matches!(
            work.status,
            ReviewWorkStatus::StructuralObservationsConfirmed
        ));
        assert_eq!(
            work.delivery_checksum.as_deref(),
            Some(hex::encode(delivery.checksum()).as_str())
        );
        assert!(!work.q1_qualified);
    })
    .await
    .expect("historical readers must not acquire the writer's invocation row lock");
    tx.commit().await.unwrap();
    drop(client);
    locked.rollback().await.unwrap();
    drop(blocker);
    let attempts: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT private_claim_attempts FROM brassclaw_turns WHERE work_id=$1",
            &[&id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(attempts, 1);
    runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn original_parser_rows_keep_native_writer_locks_and_allow_read_only_delivery_projection() {
    for writer in [Writer::Terminal, Writer::Delivery] {
        source_writer_retains_invocation_lock(writer).await;
    }
}
