//! Actual event uniqueness, failure atomicity and installed-schema upgrade.
use super::*;
use brassclaw_threads::{ReplayAcceptedInboundMessageRequest, ThreadHistoryRequest};

async fn request(
    threads: &PgSessionThreadService,
    scope: &ThreadScope,
    event: &str,
) -> AcceptInboundMessageRequest {
    let thread = threads
        .ensure_thread(EnsureThreadRequest {
            scope: scope.clone(),
            thread_id: None,
            created_by_actor_id: "inbound-owner".into(),
            title: None,
            metadata_json: None,
        })
        .await
        .unwrap();
    AcceptInboundMessageRequest {
        scope: scope.clone(),
        thread_id: thread.thread_id,
        actor_id: "inbound-owner".into(),
        source_binding_id: Some("inbound ' ü🙂 source".into()),
        reply_target_binding_id: Some("original-reply".into()),
        external_event_id: Some(event.into()),
        content: MessageContent::text("literal input ü🙂\r\n{{vars.data}}"),
    }
}
fn replay(request: &AcceptInboundMessageRequest) -> ReplayAcceptedInboundMessageRequest {
    ReplayAcceptedInboundMessageRequest {
        scope: request.scope.clone(),
        actor_id: request.actor_id.clone(),
        source_binding_id: request.source_binding_id.clone().unwrap(),
        external_event_id: request.external_event_id.clone().unwrap(),
    }
}
async fn thread_bytes(pool: &PgPool, request: &AcceptInboundMessageRequest) -> (i64, Value) {
    let row = pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT version,metadata FROM brassclaw_session_threads WHERE tenant_id=$1 AND id=$2",
            &[
                &request.scope.tenant_id.as_str(),
                &request.thread_id.as_str(),
            ],
        )
        .await
        .unwrap();
    (row.get(0), row.get(1))
}
async fn receipt_count(pool: &PgPool, request: &AcceptInboundMessageRequest) -> i64 {
    pool.get().await.unwrap().query_one(
        "SELECT count(*)::bigint FROM brassclaw_inbound_message_receipts WHERE tenant_id=$1 AND thread_id=$2",
        &[&request.scope.tenant_id.as_str(),&request.thread_id.as_str()],
    ).await.unwrap().get(0)
}

#[tokio::test]
async fn native_inbound_event_is_unique_across_concurrent_threads_and_keeps_exact_input() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _database = rig.lock_db().await;
    inbound_atomic_contract::exercise(
        Arc::new(PgSessionThreadService::new(
            rig.pool.clone(),
            "claimed-child-native",
        )),
        thread_scope(&scope("atomic-ingress")),
    )
    .await;
    let mut system_scope = thread_scope(&scope("system-inbound"));
    system_scope.owner_user_id = None;
    system_scope.project_id =
        Some(brassclaw_host_api::ProjectId::new("nullable-receipt-project").unwrap());
    inbound_atomic_contract::exercise(
        Arc::new(PgSessionThreadService::new(
            rig.pool.clone(),
            "claimed-child-native",
        )),
        system_scope,
    )
    .await;
}

#[tokio::test]
async fn native_inbound_commit_rejection_rolls_back_receipt_message_and_sequence() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _database = rig.lock_db().await;
    let threads = PgSessionThreadService::new(rig.pool.clone(), "claimed-child-native");
    let input = request(
        &threads,
        &thread_scope(&scope("inbound-rollback")),
        "rollback-event",
    )
    .await;
    let before = thread_bytes(&rig.pool, &input).await;
    rig.pool.get().await.unwrap().batch_execute(
        "CREATE FUNCTION reject_inbound_receipt_commit() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN RAISE EXCEPTION 'private deliberate ingress rejection'; END; $$; \
         CREATE CONSTRAINT TRIGGER reject_inbound_receipt_commit AFTER INSERT \
         ON brassclaw_inbound_message_receipts DEFERRABLE INITIALLY DEFERRED \
         FOR EACH ROW EXECUTE FUNCTION reject_inbound_receipt_commit()"
    ).await.unwrap();
    let error = threads
        .accept_inbound_message(input.clone())
        .await
        .unwrap_err();
    assert!(
        matches!(error, brassclaw_threads::SessionThreadError::Backend(ref reason)
        if reason=="thread database error (SQLSTATE P0001)")
    );
    rig.pool
        .get()
        .await
        .unwrap()
        .batch_execute(
            "DROP TRIGGER reject_inbound_receipt_commit ON brassclaw_inbound_message_receipts; \
         DROP FUNCTION reject_inbound_receipt_commit()",
        )
        .await
        .unwrap();
    assert_eq!(thread_bytes(&rig.pool, &input).await, before);
    assert_eq!(receipt_count(&rig.pool, &input).await, 0);
    assert!(
        threads
            .replay_accepted_inbound_message(replay(&input))
            .await
            .unwrap()
            .is_none()
    );
    let admitted = threads.accept_inbound_message(input.clone()).await.unwrap();
    assert!(!admitted.idempotent_replay);
    assert_eq!(admitted.sequence, 1);
    assert_eq!(receipt_count(&rig.pool, &input).await, 1);
}

#[tokio::test]
async fn native_inbound_lost_commit_ack_retains_one_receipt_and_original_message() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _database = rig.lock_db().await;
    let threads = PgSessionThreadService::new(rig.pool.clone(), "claimed-child-native");
    let input = request(
        &threads,
        &thread_scope(&scope("inbound-ack-loss")),
        "ack-loss-event",
    )
    .await;
    let proxy = CommitAckProxy::start(rig.url.expose_secret()).await;
    let uncertain = PgSessionThreadService::new(proxy.pool.clone(), "claimed-child-native");
    proxy.armed.store(true, Ordering::Release);
    let lost = tokio::time::timeout(
        Duration::from_secs(15),
        uncertain.accept_inbound_message(input.clone()),
    )
    .await
    .unwrap();
    assert!(lost.is_err());
    assert!(proxy.dropped.load(Ordering::Acquire));
    proxy.task.await.unwrap().unwrap();
    let observed = threads
        .replay_accepted_inbound_message(replay(&input))
        .await
        .unwrap()
        .unwrap();
    let before = thread_bytes(&rig.pool, &input).await;
    let exact = threads.accept_inbound_message(input.clone()).await.unwrap();
    assert!(exact.idempotent_replay);
    assert_eq!(exact.message_id, observed.message_id);
    assert_eq!(receipt_count(&rig.pool, &input).await, 1);
    assert_eq!(
        thread_bytes(&rig.pool, &input).await,
        before,
        "exact ACK must not rewrite the transcript"
    );
    let history = threads
        .list_thread_history(ThreadHistoryRequest {
            scope: input.scope,
            thread_id: input.thread_id,
        })
        .await
        .unwrap();
    assert_eq!(history.messages.len(), 1);
    assert_eq!(history.messages[0].message_id, observed.message_id);
    assert_eq!(
        history.messages[0].content.as_deref(),
        Some(input.content.as_text())
    );
}

#[tokio::test]
async fn native_inbound_sequence_exhaustion_preserves_snapshot_and_unclaimed_event() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _database = rig.lock_db().await;
    let threads = PgSessionThreadService::new(rig.pool.clone(), "claimed-child-native");
    let input = request(
        &threads,
        &thread_scope(&scope("inbound-sequence")),
        "sequence-event",
    )
    .await;
    let initial = thread_bytes(&rig.pool, &input).await;
    let client = rig.pool.get().await.unwrap();
    client.execute("UPDATE brassclaw_session_threads SET metadata=jsonb_set(metadata,'{next_sequence}',$3::jsonb) WHERE tenant_id=$1 AND id=$2",
        &[&input.scope.tenant_id.as_str(),&input.thread_id.as_str(),&serde_json::json!(u64::MAX)]).await.unwrap();
    let exhausted = thread_bytes(&rig.pool, &input).await;
    assert!(
        matches!(threads.accept_inbound_message(input.clone()).await,
        Err(brassclaw_threads::SessionThreadError::Backend(reason)) if reason=="thread message sequence exhausted")
    );
    assert_eq!(thread_bytes(&rig.pool, &input).await, exhausted);
    assert_eq!(receipt_count(&rig.pool, &input).await, 0);
    client
        .execute(
            "UPDATE brassclaw_session_threads SET metadata=$3 WHERE tenant_id=$1 AND id=$2",
            &[
                &input.scope.tenant_id.as_str(),
                &input.thread_id.as_str(),
                &initial.1,
            ],
        )
        .await
        .unwrap();
    assert_eq!(
        threads
            .accept_inbound_message(input)
            .await
            .unwrap()
            .sequence,
        1
    );
}

async fn restore_pre_receipt_schema(pool: &PgPool) {
    pool.get()
        .await
        .unwrap()
        .batch_execute(
            "DROP INDEX brassclaw_thread_inbound_receipts_idx; \
         DROP TABLE brassclaw_inbound_message_receipts; \
         DELETE FROM refinery_schema_history WHERE version=176",
        )
        .await
        .unwrap();
}
#[tokio::test]
async fn native_inbound_upgrade_keeps_existing_receipt_and_rejects_corrupt_legacy_graph() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _database = rig.lock_db().await;
    let threads = PgSessionThreadService::new(rig.pool.clone(), "claimed-child-native");
    let input = request(
        &threads,
        &thread_scope(&scope("inbound-upgrade")),
        "upgrade-event",
    )
    .await;
    let accepted = threads.accept_inbound_message(input.clone()).await.unwrap();
    let original = thread_bytes(&rig.pool, &input).await;
    let mut system_scope = input.scope.clone();
    system_scope.owner_user_id = None;
    system_scope.project_id =
        Some(brassclaw_host_api::ProjectId::new("legacy-system-project").unwrap());
    let system_input = request(&threads, &system_scope, "same-upgrade-event").await;
    let system_accepted = threads
        .accept_inbound_message(system_input.clone())
        .await
        .unwrap();
    let system_original = thread_bytes(&rig.pool, &system_input).await;
    restore_pre_receipt_schema(&rig.pool).await;
    let client = rig.pool.get().await.unwrap();
    client.execute("UPDATE brassclaw_session_threads SET metadata=jsonb_set(metadata,'{inbound_idempotency,0,thread_id}',to_jsonb('wrong-legacy-thread'::text)) WHERE tenant_id=$1 AND id=$2",
        &[&input.scope.tenant_id.as_str(),&input.thread_id.as_str()]).await.unwrap();
    let corrupt = thread_bytes(&rig.pool, &input).await;
    assert!(
        brassclaw_pg::migrations::run_migrations(&rig.pool)
            .await
            .is_err()
    );
    assert_eq!(thread_bytes(&rig.pool, &input).await, corrupt);
    assert!(
        client
            .query_one(
                "SELECT to_regclass('brassclaw_inbound_message_receipts') IS NULL",
                &[]
            )
            .await
            .unwrap()
            .get::<_, bool>(0)
    );
    assert_eq!(
        client
            .query_one(
                "SELECT count(*)::bigint FROM refinery_schema_history WHERE version=176",
                &[]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    client
        .execute(
            "UPDATE brassclaw_session_threads SET metadata=$3 WHERE tenant_id=$1 AND id=$2",
            &[
                &input.scope.tenant_id.as_str(),
                &input.thread_id.as_str(),
                &original.1,
            ],
        )
        .await
        .unwrap();
    brassclaw_pg::migrations::run_migrations(&rig.pool)
        .await
        .unwrap();
    brassclaw_pg::migrations::run_migrations(&rig.pool)
        .await
        .unwrap();
    assert_eq!(thread_bytes(&rig.pool, &input).await, original);
    assert_eq!(receipt_count(&rig.pool, &input).await, 1);
    let replayed = threads.accept_inbound_message(input.clone()).await.unwrap();
    assert!(replayed.idempotent_replay);
    assert_eq!(replayed.message_id, accepted.message_id);
    assert_eq!(thread_bytes(&rig.pool, &input).await, original);
    assert_eq!(
        thread_bytes(&rig.pool, &system_input).await,
        system_original
    );
    assert_eq!(receipt_count(&rig.pool, &system_input).await, 1);
    let system_replay = threads.accept_inbound_message(system_input).await.unwrap();
    assert!(system_replay.idempotent_replay);
    assert_eq!(system_replay.message_id, system_accepted.message_id);
}

#[tokio::test]
async fn native_inbound_upgrade_rejects_ambiguous_legacy_event_without_a_winner() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _database = rig.lock_db().await;
    let threads = PgSessionThreadService::new(rig.pool.clone(), "claimed-child-native");
    let input = request(
        &threads,
        &thread_scope(&scope("ambiguous-upgrade")),
        "ambiguous-event",
    )
    .await;
    let original = threads.accept_inbound_message(input.clone()).await.unwrap();
    let before = thread_bytes(&rig.pool, &input).await;
    let client = rig.pool.get().await.unwrap();
    let shadow = super::super::message_submission_tests::duplicate_legacy_receipt(
        &client,
        &input.scope,
        &input.thread_id,
        &before.1,
    )
    .await;
    restore_pre_receipt_schema(&rig.pool).await;
    assert!(
        brassclaw_pg::migrations::run_migrations(&rig.pool)
            .await
            .is_err()
    );
    assert_eq!(thread_bytes(&rig.pool, &input).await, before);
    assert!(
        client
            .query_one(
                "SELECT to_regclass('brassclaw_inbound_message_receipts') IS NULL",
                &[]
            )
            .await
            .unwrap()
            .get::<_, bool>(0)
    );
    assert_eq!(client.query_one("SELECT count(*)::bigint FROM brassclaw_session_threads WHERE tenant_id=$1 AND id=ANY($2)",
        &[&input.scope.tenant_id.as_str(),&vec![input.thread_id.to_string(),shadow.clone()]]).await.unwrap().get::<_,i64>(0),2);
    client
        .execute(
            "DELETE FROM brassclaw_session_threads WHERE tenant_id=$1 AND id=$2",
            &[&input.scope.tenant_id.as_str(), &shadow],
        )
        .await
        .unwrap();
    brassclaw_pg::migrations::run_migrations(&rig.pool)
        .await
        .unwrap();
    assert_eq!(
        threads
            .accept_inbound_message(input)
            .await
            .unwrap()
            .message_id,
        original.message_id
    );
}
