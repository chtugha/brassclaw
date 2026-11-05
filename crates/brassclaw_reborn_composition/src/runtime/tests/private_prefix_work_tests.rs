//! Actual source-only compiler through the shared private Work/Worker/global VM.
//! Source output stays quarantined; these cases grant no serving activation.
use super::*;
use brassclaw_product_workflow::WebUiAuthenticatedCaller;
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_source_compiler_uses_shared_worker_without_messages_and_keeps_original() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let directory = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(RebornRuntimeInput::from_services(
        rig.build_input("private-prefix-work-owner", directory.path())
            .with_runtime_policy(local_dev_runtime_policy()),
    )))
    .await
    .unwrap();
    let caller = WebUiAuthenticatedCaller::new(
        runtime.thread_scope.tenant_id.clone(),
        runtime.actor_user_id.clone(),
        Some(runtime.thread_scope.agent_id.clone()),
        runtime.thread_scope.project_id.clone(),
    );
    let revision: i64 = rig
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
    let root = runtime.global_monty_owner.client().root_identity();
    let counts_before: (i64, i64) = {
        let client = rig.pool.get().await.unwrap();
        let row = client.query_one("SELECT (SELECT count(*) FROM brassclaw_session_threads), (SELECT count(*) FROM brassclaw_inbound_message_receipts)", &[]).await.unwrap();
        (row.get(0), row.get(1))
    };
    let rejected = Uuid::new_v4();
    assert!(
        runtime
            .submit_prefix_generation_snapshot(
                rejected,
                "base-prompt",
                revision,
                "{}",
                caller.clone()
            )
            .await
            .is_err()
    );
    assert!(
        !rig.pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM brassclaw_turns WHERE work_id=$1)",
                &[&rejected],
            )
            .await
            .unwrap()
            .get::<_, bool>(0)
    );

    let id = Uuid::new_v4();
    let driver = &runtime.monty_test_controls.0;
    let run = TurnRunId::from_uuid(id);
    let pause = driver.pause_private_publish(run);
    let waiter = driver.private_completion_waiter(run).unwrap();
    assert_eq!(
        runtime
            .submit_current_core_prefix_snapshot(id, "base-prompt", revision, caller.clone())
            .await
            .unwrap(),
        id
    );
    tokio::time::timeout(std::time::Duration::from_secs(60), pause.reached.notified())
        .await
        .unwrap();
    // The actual producer keeps its slot even after every consumer disposes.
    drop(waiter);
    assert!(driver.private_completion_live(run).unwrap());
    let late = driver.private_completion_waiter(run).unwrap();
    assert!(!late.has_result());
    pause.release.notify_one();
    let driven = tokio::time::timeout(std::time::Duration::from_secs(60), late.wait())
        .await
        .unwrap()
        .unwrap();
    let completion = driven
        .internal_completion
        .as_ref()
        .expect("actual native private completion");
    assert_eq!(completion.receipt().root, root);
    {
        let mut client = rig.pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .unwrap();
        let original = crate::pg_monty_admission::PgMontyAdmission::verify_private_live_completion(
            &tx, completion,
        )
        .await
        .unwrap()
        .unwrap();
        assert!(
            matches!(original.request().original(), brassclaw_turns::private_work::PrivateOriginalRef::PrefixSourceOnly { job_id, .. } if *job_id == id)
        );
        tx.commit().await.unwrap();
    }
    let detail = runtime.inspect_prefix_job("base-prompt", id).await.unwrap();
    let wire = serde_json::to_value(&detail).unwrap();
    assert_eq!(wire["format"], "prefix-job-detail/2");
    assert_eq!(wire["work_status"], "settled");
    assert!(wire.get("thread_id").is_none());
    assert!(wire.get("run_status").is_none());
    assert_eq!(detail.admission_phase.as_deref(), Some("settled"));
    assert!(detail.internal_receipt.is_some());
    assert_eq!(detail.pending_tool_calls, 0);
    assert!(detail.answered_tool_calls > 0);
    // Same ID and immutable original are read back without another native run.
    assert_eq!(
        runtime
            .submit_current_core_prefix_snapshot(id, "base-prompt", revision, caller)
            .await
            .unwrap(),
        id
    );
    let client = rig.pool.get().await.unwrap();
    let row = client.query_one("SELECT (SELECT count(*) FROM brassclaw_session_threads), (SELECT count(*) FROM brassclaw_inbound_message_receipts)", &[]).await.unwrap();
    assert_eq!((row.get::<_, i64>(0), row.get::<_, i64>(1)), counts_before);
    let row = client.query_one("SELECT private_claim_attempts,(SELECT count(*) FROM brassclaw_monty_task_admissions WHERE private_work_id=$1),(SELECT count(*) FROM brassclaw_monty_private_terminals WHERE work_id=$1) FROM brassclaw_turns WHERE work_id=$1", &[&id]).await.unwrap();
    assert_eq!(
        (
            row.get::<_, i64>(0),
            row.get::<_, i64>(1),
            row.get::<_, i64>(2)
        ),
        (1, 1, 1)
    );
    assert_eq!(runtime.global_monty_owner.client().root_identity(), root);
}
