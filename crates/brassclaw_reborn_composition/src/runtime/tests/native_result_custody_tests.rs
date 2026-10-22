//! Ordinary approved Match with an actual failed native-answer COMMIT. The
//! original reply must survive until evidence acknowledgement without replay.
use super::*;
use brassclaw_turns::run_profile::MontyTurnDriverPort;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_reply_answer_survives_failed_commit_and_reconciles_without_replay() {
    let rig = super::super::test_pg::pg_rig().await;
    let _db_guard = rig.lock_db().await;
    let root = tempfile::tempdir().unwrap();
    let runtime = Arc::new(
        Box::pin(build_reborn_runtime(
            RebornRuntimeInput::from_services(
                rig.build_input("native-answer-custody-owner", root.path())
                    .with_runtime_policy(local_dev_runtime_policy()),
            )
            .with_poll_settings(PollSettings {
                interval: Duration::from_millis(10),
                max_total: RUNTIME_SEND_TIMEOUT,
            }),
        ))
        .await
        .unwrap(),
    );
    let client = rig.pool.get().await.unwrap();
    let conversation = runtime.new_conversation().await.unwrap();
    let correlation = uuid::Uuid::new_v4();
    let run = TurnRunId::from_uuid(correlation);
    let literal = "Native reply ü🙂 ' {{vars.data}}\n```python\nraise RuntimeError('data')\n```";
    client
        .batch_execute(&format!(
            r#"
        CREATE FUNCTION reject_native_reply_answer() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN
            IF NEW.run_id='{correlation}'::uuid AND NEW.phase='answered' THEN
                RAISE EXCEPTION 'deliberate native answer COMMIT rejection';
            END IF;
            RETURN NEW;
        END $$;
        CREATE CONSTRAINT TRIGGER reject_native_reply_answer
            AFTER UPDATE ON brassclaw_monty_tool_invocations
            DEFERRABLE INITIALLY DEFERRED FOR EACH ROW
            EXECUTE FUNCTION reject_native_reply_answer();
    "#
        ))
        .await
        .unwrap();
    let reply = runtime
        .send_correlated_user_message(
            &conversation,
            &format!("publish literal reply {literal}"),
            correlation,
            tokio_util::sync::CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(matches!(
        reply.status,
        TurnStatus::Failed | TurnStatus::RecoveryRequired
    ));
    let driver = &runtime.monty_test_controls.0;
    let host = driver
        .retained_host_for_run(run)
        .unwrap()
        .expect("unacknowledged real native answer must retain its original task owner");
    let attempt = host.attempt();
    let reference = host
        .finalized_reply_ref()
        .expect("the native reply effect really occurred");
    let (observed_ref, observed_content) = host.finalized_reply_evidence().unwrap();
    assert_eq!(observed_ref, reference.as_str());
    assert_eq!(observed_content, literal);
    assert_eq!(host.review_dispatch_counts().0, 0);
    let before = client.query_one("SELECT invocation_id,phase,answer_bytes FROM brassclaw_monty_tool_invocations WHERE run_id=$1", &[&correlation]).await.unwrap();
    let invocation: uuid::Uuid = before.get(0);
    assert!(
        crate::installed_monty_catalogue::InstalledMontyCatalogue::native_answer_retained_for_test(
            invocation
        )
        .unwrap(),
        "an unacknowledged actual native answer must remain in host custody"
    );
    assert_eq!(before.get::<_, &str>(1), "dispatch_intent");
    assert!(before.get::<_, Option<String>>(2).is_none());
    assert_eq!(
        client
            .query_one(
                "SELECT phase FROM brassclaw_monty_task_admissions WHERE run_id=$1",
                &[&correlation]
            )
            .await
            .unwrap()
            .get::<_, &str>(0),
        "started"
    );
    assert!(
        runtime
            .global_monty_owner
            .client()
            .retained_attempts()
            .retained
            > 0
    );
    client.batch_execute("DROP TRIGGER reject_native_reply_answer ON brassclaw_monty_tool_invocations; DROP FUNCTION reject_native_reply_answer();").await.unwrap();
    // The original claim is now failed/fenced. Evidence acknowledgement is
    // still allowed using the same private journal/receipt; execution is not.
    driver.stop_attempt(attempt).await.unwrap();
    let after = client.query_one("SELECT invocation_id,phase,answer_bytes,attempt_count::bigint FROM brassclaw_monty_tool_invocations WHERE run_id=$1", &[&correlation]).await.unwrap();
    assert_eq!(after.get::<_, uuid::Uuid>(0), invocation);
    assert_eq!(after.get::<_, &str>(1), "answered");
    let answer: serde_json::Value = serde_json::from_str(after.get(2)).unwrap();
    assert_eq!(
        answer,
        serde_json::json!({"kind":"return","value":reference.as_str()})
    );
    assert_eq!(after.get::<_, i64>(3), 1);
    assert!(!crate::installed_monty_catalogue::InstalledMontyCatalogue::native_answer_retained_for_test(invocation).unwrap(),
        "confirmed journal acknowledgement must free the redundant host payload even while the failed task stays retained");
    let outcome: serde_json::Value = client
        .query_one(
            "SELECT outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1",
            &[&correlation],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        outcome["status"], "failed",
        "acknowledgement never converts the failed Recipe to success"
    );
    assert_eq!(outcome["execution"]["all_selected_recipes_complete"], false);
    assert_eq!(
        outcome["execution"]["reply"]["reference"],
        reference.as_str()
    );
    assert_eq!(host.review_dispatch_counts().0, 0);
    let history = runtime
        .thread_service
        .list_thread_history(brassclaw_threads::ThreadHistoryRequest {
            scope: runtime.thread_scope.clone(),
            thread_id: conversation.0.clone(),
        })
        .await
        .unwrap();
    let actual = history
        .messages
        .iter()
        .filter(|message| {
            message.kind == brassclaw_threads::MessageKind::Assistant
                && message.status == brassclaw_threads::MessageStatus::Finalized
        })
        .collect::<Vec<_>>();
    assert_eq!(actual.len(), 1);
    assert_eq!(actual[0].content.as_deref(), Some(literal));
    assert_eq!(format!("msg:{}", actual[0].message_id), reference.as_str());
    drop(client);
    shutdown_shared_runtime(runtime).await.unwrap();
}
