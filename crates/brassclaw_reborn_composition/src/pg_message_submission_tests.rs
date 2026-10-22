//! Real PostgreSQL transcript submission and snapshot identity contracts.
use super::*;

#[tokio::test]
async fn native_ensure_thread_preserves_zero_version_tenant_and_raced_scope_identity() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _database = rig.lock_db().await;
    let original_scope = thread_scope(&scope("ensure-identity"));
    let service = PgSessionThreadService::new(rig.pool.clone(), "claimed-child-native");
    let mut mismatched_scope = original_scope.clone();
    mismatched_scope.tenant_id = TenantId::new("foreign-service-scope").unwrap();
    let rejected_id = ThreadId::new(format!("rejected-{}", uuid::Uuid::new_v4())).unwrap();
    assert!(matches!(
        service.ensure_thread(EnsureThreadRequest {
            scope: mismatched_scope,
            thread_id: Some(rejected_id.clone()),
            created_by_actor_id: "claimed-child-owner".into(),
            title: None,
            metadata_json: None,
        }).await,
        Err(brassclaw_threads::SessionThreadError::ThreadScopeMismatch { thread_id }) if thread_id == rejected_id
    ));
    let rejected_rows: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT count(*)::bigint FROM brassclaw_session_threads WHERE id=$1",
            &[&rejected_id.as_str()],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(rejected_rows, 0, "a foreign scope must never be persisted");
    let original = service
        .ensure_thread(EnsureThreadRequest {
            scope: original_scope.clone(),
            thread_id: None,
            created_by_actor_id: "claimed-child-owner".into(),
            title: Some("original empty thread".into()),
            metadata_json: None,
        })
        .await
        .unwrap();
    let client = rig.pool.get().await.unwrap();
    // V049 explicitly permits the bootstrap/legacy zero generation. It is not
    // permission for another tenant or raced scope to overwrite the identity.
    client
        .execute(
            "UPDATE brassclaw_session_threads SET version=0 WHERE tenant_id=$1 AND id=$2",
            &[
                &original_scope.tenant_id.as_str(),
                &original.thread_id.as_str(),
            ],
        )
        .await
        .unwrap();
    let before = client
        .query_one(
            "SELECT version,metadata FROM brassclaw_session_threads WHERE tenant_id=$1 AND id=$2",
            &[
                &original_scope.tenant_id.as_str(),
                &original.thread_id.as_str(),
            ],
        )
        .await
        .unwrap();
    let mut foreign_scope = original_scope.clone();
    foreign_scope.tenant_id = TenantId::new("foreign-submission-tenant").unwrap();
    let foreign = PgSessionThreadService::new(rig.pool.clone(), foreign_scope.tenant_id.as_str());
    assert!(
        foreign
            .ensure_thread(EnsureThreadRequest {
                scope: foreign_scope,
                thread_id: Some(original.thread_id.clone()),
                created_by_actor_id: "foreign-submission-owner".into(),
                title: None,
                metadata_json: None,
            })
            .await
            .is_err()
    );
    let after = client
        .query_one(
            "SELECT version,metadata FROM brassclaw_session_threads WHERE tenant_id=$1 AND id=$2",
            &[
                &original_scope.tenant_id.as_str(),
                &original.thread_id.as_str(),
            ],
        )
        .await
        .unwrap();
    assert_eq!(after.get::<_, i64>(0), 0);
    assert_eq!(
        before.get::<_, serde_json::Value>(1),
        after.get::<_, serde_json::Value>(1)
    );
    client.execute(
        "UPDATE brassclaw_session_threads SET metadata=jsonb_set(metadata,'{record,thread_id}',to_jsonb('unrelated-thread'::text)) \
         WHERE tenant_id=$1 AND id=$2",
        &[&original_scope.tenant_id.as_str(), &original.thread_id.as_str()],
    ).await.unwrap();
    assert!(matches!(service.ensure_thread(EnsureThreadRequest {
        scope:original_scope.clone(), thread_id:Some(original.thread_id.clone()),
        created_by_actor_id:"claimed-child-owner".into(), title:None, metadata_json:None,
    }).await, Err(brassclaw_threads::SessionThreadError::Backend(reason)) if reason=="thread snapshot identity mismatch"));
    client
        .execute(
            "UPDATE brassclaw_session_threads SET metadata=$3 WHERE tenant_id=$1 AND id=$2",
            &[
                &original_scope.tenant_id.as_str(),
                &original.thread_id.as_str(),
                &before.get::<_, serde_json::Value>(1),
            ],
        )
        .await
        .unwrap();
    let target = ThreadId::new(uuid::Uuid::new_v4().to_string()).unwrap();
    let mut writer = rig.pool.get().await.unwrap();
    let transaction = writer.transaction().await.unwrap();
    transaction
        .execute(
            "INSERT INTO brassclaw_session_threads \
         (id,tenant_id,user_id,agent_id,project_id,created_by_actor_id,title,metadata,version) \
         SELECT $2,tenant_id,user_id,agent_id,project_id,created_by_actor_id,title,\
         jsonb_set(metadata,'{record,thread_id}',to_jsonb($2::text)),version \
         FROM brassclaw_session_threads WHERE tenant_id=$1 AND id=$3",
            &[
                &original_scope.tenant_id.as_str(),
                &target.as_str(),
                &original.thread_id.as_str(),
            ],
        )
        .await
        .unwrap();
    let mut requested_scope = original_scope.clone();
    requested_scope.agent_id = AgentId::new("raced-submission-agent").unwrap();
    let requester = PgSessionThreadService::new(rig.pool.clone(), "claimed-child-native");
    let requested_target = target.clone();
    let waiting = tokio::spawn(async move {
        requester
            .ensure_thread(EnsureThreadRequest {
                scope: requested_scope,
                thread_id: Some(requested_target),
                created_by_actor_id: "raced-owner".into(),
                title: Some("must not overwrite the winner".into()),
                metadata_json: None,
            })
            .await
    });
    // Observe the actual blocked INSERT after its MVCC read saw no committed
    // row. No fake service result or scheduler sleep chooses the winner.
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let blocked: bool = client
                .query_one(
                    "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() \
                 AND pid<>pg_backend_pid() AND wait_event_type='Lock' \
                 AND query LIKE 'INSERT INTO brassclaw_session_threads %')",
                    &[],
                )
                .await
                .unwrap()
                .get(0);
            if blocked {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    transaction.commit().await.unwrap();
    let error = waiting.await.unwrap().unwrap_err();
    assert!(
        matches!(error, brassclaw_threads::SessionThreadError::ThreadScopeMismatch { ref thread_id } if *thread_id==target)
    );
    let winner = service
        .ensure_thread(EnsureThreadRequest {
            scope: original_scope.clone(),
            thread_id: Some(target.clone()),
            created_by_actor_id: "claimed-child-owner".into(),
            title: None,
            metadata_json: None,
        })
        .await
        .unwrap();
    assert_eq!(winner.scope, original_scope);
    assert_eq!(winner.title.as_deref(), Some("original empty thread"));
    let generation: i64 = client
        .query_one(
            "SELECT version FROM brassclaw_session_threads WHERE tenant_id=$1 AND id=$2",
            &[&winner.scope.tenant_id.as_str(), &target.as_str()],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(generation, 0);
}

#[tokio::test]
async fn native_message_submission_keeps_exact_binding_and_snapshot_generation() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _database = rig.lock_db().await;
    let service = PgSessionThreadService::new(rig.pool.clone(), "claimed-child-native");
    let scope = thread_scope(&scope("native-submission"));
    let (thread, message) = submission_binding_contract::exercise(&service, scope.clone()).await;
    let first = service
        .mark_message_submitted(
            &scope,
            &thread,
            message,
            "exact-turn".into(),
            "exact-run".into(),
        )
        .await
        .unwrap();
    let client = rig.pool.get().await.unwrap();
    let before = client
        .query_one(
            "SELECT version,metadata FROM brassclaw_session_threads WHERE tenant_id=$1 AND id=$2",
            &[&scope.tenant_id.as_str(), &thread.as_str()],
        )
        .await
        .unwrap();
    assert_eq!(
        service
            .mark_message_submitted(
                &scope,
                &thread,
                message,
                "exact-turn".into(),
                "exact-run".into()
            )
            .await
            .unwrap(),
        first
    );
    assert!(
        service
            .mark_message_submitted(
                &scope,
                &thread,
                message,
                "wrong-turn".into(),
                "exact-run".into()
            )
            .await
            .is_err()
    );
    let after = client
        .query_one(
            "SELECT version,metadata FROM brassclaw_session_threads WHERE tenant_id=$1 AND id=$2",
            &[&scope.tenant_id.as_str(), &thread.as_str()],
        )
        .await
        .unwrap();
    assert_eq!(
        before.get::<_, i64>(0),
        after.get::<_, i64>(0),
        "an exact metadata acknowledgement is read-only"
    );
    assert_eq!(
        before.get::<_, serde_json::Value>(1),
        after.get::<_, serde_json::Value>(1)
    );
    client
        .execute(
            "UPDATE brassclaw_session_threads SET version=$3 WHERE tenant_id=$1 AND id=$2",
            &[&scope.tenant_id.as_str(), &thread.as_str(), &i64::MAX],
        )
        .await
        .unwrap();
    // Repeating the existing binding still needs no generation increment.
    assert_eq!(
        service
            .mark_message_submitted(
                &scope,
                &thread,
                message,
                "exact-turn".into(),
                "exact-run".into()
            )
            .await
            .unwrap(),
        first
    );
    let exhausted = service
        .append_assistant_draft(brassclaw_threads::AppendAssistantDraftRequest {
            scope: scope.clone(),
            thread_id: thread.clone(),
            turn_run_id: "another-run".into(),
            content: MessageContent::text("must not be written at exhausted generation"),
        })
        .await;
    assert!(
        matches!(exhausted, Err(brassclaw_threads::SessionThreadError::Backend(reason))
        if reason == "thread snapshot version exhausted")
    );
    let unchanged = client
        .query_one(
            "SELECT version,metadata FROM brassclaw_session_threads WHERE tenant_id=$1 AND id=$2",
            &[&scope.tenant_id.as_str(), &thread.as_str()],
        )
        .await
        .unwrap();
    assert_eq!(unchanged.get::<_, i64>(0), i64::MAX);
    assert_eq!(
        unchanged.get::<_, serde_json::Value>(1),
        before.get::<_, serde_json::Value>(1)
    );
    client
        .execute(
            "UPDATE brassclaw_session_threads SET version=-1 WHERE tenant_id=$1 AND id=$2",
            &[&scope.tenant_id.as_str(), &thread.as_str()],
        )
        .await
        .unwrap();
    assert!(
        matches!(service.list_thread_history(brassclaw_threads::ThreadHistoryRequest {
        scope:scope.clone(), thread_id:thread.clone(),
    }).await, Err(brassclaw_threads::SessionThreadError::Backend(reason)) if reason == "invalid thread snapshot version")
    );
    client
        .execute(
            "UPDATE brassclaw_session_threads SET version=$3 WHERE tenant_id=$1 AND id=$2",
            &[
                &scope.tenant_id.as_str(),
                &thread.as_str(),
                &before.get::<_, i64>(0),
            ],
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn native_turn_snapshot_generation_exhaustion_preserves_original_state() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _database = rig.lock_db().await;
    let store = Arc::new(PgTurnStateStore::new(
        rig.pool.clone(),
        "claimed-child-native",
    ));
    let coordinator = DefaultTurnCoordinator::new(store.clone());
    let threads = PgSessionThreadService::new(rig.pool.clone(), "claimed-child-native");
    let scope = scope("snapshot-generation");
    let (accepted, _) = input(&threads, &scope).await;
    let actor = TurnActor::new(UserId::new("claimed-child-owner").unwrap());
    let SubmitTurnResponse::Accepted { run_id, .. } = coordinator
        .submit_turn(SubmitTurnRequest {
            scope: scope.clone(),
            actor: actor.clone(),
            accepted_message_ref: accepted,
            source_binding_ref: SourceBindingRef::new("snapshot-generation-source").unwrap(),
            reply_target_binding_ref: ReplyTargetBindingRef::new("snapshot-generation-reply")
                .unwrap(),
            requested_run_profile: None,
            idempotency_key: IdempotencyKey::new("snapshot-generation-submit").unwrap(),
            received_at: chrono::Utc::now(),
            requested_run_id: None,
            parent_run_id: None,
            subagent_depth: 0,
            spawn_tree_root_run_id: None,
        })
        .await
        .unwrap();
    let client = rig.pool.get().await.unwrap();
    let before = client.query_one("SELECT version,payload FROM brassclaw_turns WHERE tenant_id=$1 AND turn_id=$2 AND status='snapshot'",
        &[&scope.tenant_id.as_str(), &scope.thread_id.as_str()]).await.unwrap();
    client.execute("UPDATE brassclaw_turns SET version=$3 WHERE tenant_id=$1 AND turn_id=$2 AND status='snapshot'",
        &[&scope.tenant_id.as_str(), &scope.thread_id.as_str(), &i64::MAX]).await.unwrap();
    let cancel = CancelRunRequest {
        scope: scope.clone(),
        actor,
        run_id,
        reason: SanitizedCancelReason::UserRequested,
        idempotency_key: IdempotencyKey::new("snapshot-generation-cancel").unwrap(),
    };
    assert!(matches!(coordinator.cancel_tree(cancel.clone()).await,
        Err(TurnError::Unavailable { reason }) if reason == "turn-state snapshot version exhausted"));
    let unchanged = client.query_one("SELECT version,payload FROM brassclaw_turns WHERE tenant_id=$1 AND turn_id=$2 AND status='snapshot'",
        &[&scope.tenant_id.as_str(), &scope.thread_id.as_str()]).await.unwrap();
    assert_eq!(unchanged.get::<_, i64>(0), i64::MAX);
    assert_eq!(
        unchanged.get::<_, serde_json::Value>(1),
        before.get::<_, serde_json::Value>(1)
    );
    client.execute("UPDATE brassclaw_turns SET version=-1 WHERE tenant_id=$1 AND turn_id=$2 AND status='snapshot'",
        &[&scope.tenant_id.as_str(), &scope.thread_id.as_str()]).await.unwrap();
    assert!(
        matches!(coordinator.get_run_state(GetRunStateRequest { scope:scope.clone(),run_id }).await,
        Err(TurnError::Unavailable { reason }) if reason == "invalid turn-state snapshot version")
    );
    client.execute("UPDATE brassclaw_turns SET version=$3 WHERE tenant_id=$1 AND turn_id=$2 AND status='snapshot'",
        &[&scope.tenant_id.as_str(), &scope.thread_id.as_str(), &before.get::<_,i64>(0)]).await.unwrap();
    coordinator.cancel_tree(cancel).await.unwrap();
}

#[tokio::test]
async fn native_replay_lookup_preserves_exact_scope_actor_and_receipt_identity() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _database = rig.lock_db().await;
    let service = PgSessionThreadService::new(rig.pool.clone(), "claimed-child-native");
    let scope = thread_scope(&scope("replay-scope"));
    let (thread, _) = inbound_replay_contract::exercise(&service, scope.clone()).await;
    let request = brassclaw_threads::ReplayAcceptedInboundMessageRequest {
        scope: scope.clone(),
        actor_id: "replay-owner".into(),
        source_binding_id: "shared ' ü binding".into(),
        external_event_id: "shared-event".into(),
    };
    let client = rig.pool.get().await.unwrap();
    let before = client
        .query_one(
            "SELECT metadata FROM brassclaw_session_threads WHERE tenant_id=$1 AND id=$2",
            &[&scope.tenant_id.as_str(), &thread.as_str()],
        )
        .await
        .unwrap()
        .get::<_, serde_json::Value>(0);
    client.execute("UPDATE brassclaw_session_threads SET metadata=jsonb_set(metadata,'{inbound_idempotency,0,thread_id}',to_jsonb('wrong-receipt-thread'::text)) WHERE tenant_id=$1 AND id=$2", &[&scope.tenant_id.as_str(), &thread.as_str()]).await.unwrap();
    assert!(
        matches!(service.replay_accepted_inbound_message(request.clone()).await,
        Err(brassclaw_threads::SessionThreadError::Backend(reason)) if reason=="inbound receipt identity mismatch")
    );
    client
        .execute(
            "UPDATE brassclaw_session_threads SET metadata=$3 WHERE tenant_id=$1 AND id=$2",
            &[&scope.tenant_id.as_str(), &thread.as_str(), &before],
        )
        .await
        .unwrap();
    assert!(
        service
            .replay_accepted_inbound_message(request.clone())
            .await
            .unwrap()
            .is_some()
    );
    // Legacy conflicting receipts must be surfaced, never selected by row
    // order. Insert an actual second coherent snapshot in this private DB.
    let shadow_id = duplicate_legacy_receipt(&client, &scope, &thread, &before).await;
    assert!(
        matches!(service.replay_accepted_inbound_message(request.clone()).await,
        Err(brassclaw_threads::SessionThreadError::Backend(reason)) if reason=="inbound receipt is ambiguous")
    );
    client
        .execute(
            "DELETE FROM brassclaw_session_threads WHERE tenant_id=$1 AND id=$2",
            &[&scope.tenant_id.as_str(), &shadow_id],
        )
        .await
        .unwrap();
    assert!(
        service
            .replay_accepted_inbound_message(request)
            .await
            .unwrap()
            .is_some()
    );
}

pub(super) async fn duplicate_legacy_receipt(
    client: &tokio_postgres::Client,
    scope: &brassclaw_threads::ThreadScope,
    thread: &ThreadId,
    before: &serde_json::Value,
) -> String {
    let shadow_id = uuid::Uuid::new_v4().to_string();
    let mut shadow = before.clone();
    shadow["record"]["thread_id"] = serde_json::json!(shadow_id);
    let mut ids = std::collections::HashMap::new();
    for message in shadow["messages"].as_array_mut().unwrap() {
        let old = message["message_id"].as_str().unwrap().to_owned();
        let new = uuid::Uuid::new_v4().to_string();
        message["message_id"] = serde_json::json!(new);
        message["thread_id"] = serde_json::json!(shadow_id);
        ids.insert(old, new);
    }
    for receipt in shadow["inbound_idempotency"].as_array_mut().unwrap() {
        let message = receipt["message_id"].as_str().unwrap();
        receipt["message_id"] = serde_json::json!(ids[message]);
        receipt["thread_id"] = serde_json::json!(shadow_id);
    }
    client.execute(
        "INSERT INTO brassclaw_session_threads (id,tenant_id,user_id,agent_id,project_id,created_by_actor_id,title,metadata,version) \
         SELECT $2,tenant_id,user_id,agent_id,project_id,created_by_actor_id,title,$4,version \
         FROM brassclaw_session_threads WHERE tenant_id=$1 AND id=$3",
        &[&scope.tenant_id.as_str(), &shadow_id, &thread.as_str(), &shadow],
    ).await.unwrap();
    shadow_id
}
