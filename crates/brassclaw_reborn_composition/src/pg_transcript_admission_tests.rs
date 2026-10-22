//! Actual transaction rollback and lost-COMMIT transport over canonical stores.
use super::*;
use brassclaw_threads::{RedactMessageRequest, SubmittedUserMessageRequest};

fn atomic_store(pool: Arc<PgPool>) -> PgTurnStateStore {
    PgTurnStateStore::new(pool.clone(), "claimed-child-native").with_admission_commit_participant(
        Arc::new(crate::pg_transcript_admission::PgTranscriptAdmission::new(
            pool,
            "claimed-child-native",
        )),
    )
}

async fn persisted_turn(pool: &PgPool, scope: &TurnScope) -> (i64, Value) {
    let row = pool.get().await.unwrap().query_one(
        "SELECT version,payload FROM brassclaw_turns WHERE tenant_id=$1 AND turn_id=$2 AND status='snapshot'",
        &[&scope.tenant_id.as_str(), &scope.thread_id.as_str()],
    ).await.unwrap();
    (row.get(0), row.get(1))
}

async fn persisted_thread(pool: &PgPool, scope: &TurnScope) -> (i64, Value) {
    let row = pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT version,metadata FROM brassclaw_session_threads WHERE tenant_id=$1 AND id=$2",
            &[&scope.tenant_id.as_str(), &scope.thread_id.as_str()],
        )
        .await
        .unwrap();
    (row.get(0), row.get(1))
}

fn child_request(
    parent: &brassclaw_turns::runner::ClaimedTurnRun,
    scope: TurnScope,
    actor: TurnActor,
    accepted: AcceptedMessageRef,
) -> SubmitChildRunRequest {
    SubmitChildRunRequest {
        parent_scope: parent.state.scope.clone(),
        parent_run_id: parent.state.run_id,
        child_scope: scope,
        actor,
        accepted_message_ref: accepted,
        source_binding_ref: SourceBindingRef::new("atomic-child-source").unwrap(),
        reply_target_binding_ref: ReplyTargetBindingRef::new("atomic-child-reply").unwrap(),
        requested_run_profile: None,
        idempotency_key: IdempotencyKey::new(uuid::Uuid::new_v4().to_string()).unwrap(),
        received_at: chrono::Utc::now(),
        requested_run_id: Some(TurnRunId::new()),
        spawn_tree_descendant_cap: 1,
    }
}

async fn submitted(
    threads: &PgSessionThreadService,
    scope: &TurnScope,
    message: ThreadMessageId,
    turn: brassclaw_turns::TurnId,
    run: TurnRunId,
) -> brassclaw_threads::ThreadMessageRecord {
    threads
        .submitted_user_message(SubmittedUserMessageRequest {
            scope: thread_scope(scope),
            thread_id: scope.thread_id.clone(),
            message_id: message,
            turn_id: turn.to_string(),
            turn_run_id: run.to_string(),
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn native_atomic_admission_binds_input_and_rolls_back_invalid_or_rejected_commit() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _database = rig.lock_db().await;
    let store = Arc::new(atomic_store(rig.pool.clone()));
    let coordinator = DefaultTurnCoordinator::new(store.clone());
    let threads = PgSessionThreadService::new(rig.pool.clone(), "claimed-child-native");
    let parent_scope = scope("atomic-parent");
    let actor = TurnActor::new(UserId::new("claimed-child-owner").unwrap());
    let (accepted, parent_message) = input(&threads, &parent_scope).await;
    let request = SubmitTurnRequest {
        scope: parent_scope.clone(),
        actor: actor.clone(),
        accepted_message_ref: accepted.clone(),
        source_binding_ref: SourceBindingRef::new("atomic-source").unwrap(),
        reply_target_binding_ref: ReplyTargetBindingRef::new("atomic-reply").unwrap(),
        requested_run_profile: None,
        idempotency_key: IdempotencyKey::new("atomic-root").unwrap(),
        received_at: chrono::Utc::now(),
        requested_run_id: None,
        parent_run_id: None,
        subagent_depth: 0,
        spawn_tree_root_run_id: None,
    };
    let response = coordinator.submit_turn(request.clone()).await.unwrap();
    let SubmitTurnResponse::Accepted {
        turn_id, run_id, ..
    } = response.clone();
    // No producer-side mark_message_submitted has occurred.
    let bound = submitted(&threads, &parent_scope, parent_message, turn_id, run_id).await;
    assert_eq!(bound.status, brassclaw_threads::MessageStatus::Submitted);
    let root_before = persisted_turn(&rig.pool, &parent_scope).await;
    let message_before = persisted_thread(&rig.pool, &parent_scope).await;
    assert_eq!(coordinator.submit_turn(request).await.unwrap(), response);
    assert_eq!(persisted_turn(&rig.pool, &parent_scope).await, root_before);
    assert_eq!(
        persisted_thread(&rig.pool, &parent_scope).await,
        message_before
    );
    let parent = store
        .claim_next_run(ClaimRunRequest {
            runner_id: TurnRunnerId::new(),
            lease_token: TurnLeaseToken::new(),
            scope_filter: Some(parent_scope.clone()),
        })
        .await
        .unwrap()
        .unwrap();
    for kind in ["redacted", "foreign-message", "commit-rejected"] {
        let child_scope = scope(&format!("atomic-{kind}"));
        let (mut child_accepted, message) = input(&threads, &child_scope).await;
        if kind == "redacted" {
            threads
                .redact_message(RedactMessageRequest {
                    scope: thread_scope(&child_scope),
                    thread_id: child_scope.thread_id.clone(),
                    message_id: message,
                    redaction_ref: "redacted-before-admission".into(),
                })
                .await
                .unwrap();
        } else if kind == "foreign-message" {
            child_accepted = accepted.clone();
        } else {
            // Real deferred constraint failure after both snapshot and message
            // UPDATEs have executed. No fake service or success is returned.
            rig.pool.get().await.unwrap().batch_execute(
                "CREATE FUNCTION reject_atomic_transcript_commit() RETURNS trigger LANGUAGE plpgsql AS $$ \
                 BEGIN IF NEW.id='atomic-commit-rejected' THEN \
                 RAISE EXCEPTION 'deliberate deferred admission rejection'; END IF; RETURN NEW; END; $$; \
                 CREATE CONSTRAINT TRIGGER reject_atomic_transcript_commit \
                 AFTER UPDATE ON brassclaw_session_threads DEFERRABLE INITIALLY DEFERRED \
                 FOR EACH ROW EXECUTE FUNCTION reject_atomic_transcript_commit()"
            ).await.unwrap();
        }
        let turn_before = persisted_turn(&rig.pool, &parent_scope).await;
        let input_before = persisted_thread(&rig.pool, &child_scope).await;
        let request = child_request(&parent, child_scope.clone(), actor.clone(), child_accepted);
        let child = request.requested_run_id.unwrap();
        assert!(
            coordinator
                .submit_claimed_child_run(request, &parent)
                .await
                .is_err()
        );
        if kind == "commit-rejected" {
            rig.pool
                .get()
                .await
                .unwrap()
                .batch_execute(
                    "DROP TRIGGER reject_atomic_transcript_commit ON brassclaw_session_threads; \
                 DROP FUNCTION reject_atomic_transcript_commit()",
                )
                .await
                .unwrap();
        }
        assert_eq!(persisted_turn(&rig.pool, &parent_scope).await, turn_before);
        assert_eq!(
            persisted_thread(&rig.pool, &child_scope).await,
            input_before
        );
        assert!(
            store
                .get_run_record(&child_scope, child)
                .await
                .unwrap()
                .is_none()
        );
        let alias: bool = rig.pool.get().await.unwrap().query_one(
            "SELECT EXISTS(SELECT 1 FROM brassclaw_turn_snapshot_threads WHERE tenant_id=$1 AND thread_id=$2)",
            &[&child_scope.tenant_id.as_str(), &child_scope.thread_id.as_str()],
        ).await.unwrap().get(0);
        assert!(!alias);
        assert_eq!(reservation_count(&rig.pool, &parent_scope, run_id).await, 0);
    }
    // Rejected commits have consumed no child reservation or parent claim.
    let child_scope = scope("atomic-after-rejections");
    let (accepted, message) = input(&threads, &child_scope).await;
    let SubmitTurnResponse::Accepted {
        turn_id,
        run_id: child,
        ..
    } = coordinator
        .submit_claimed_child_run(
            child_request(&parent, child_scope.clone(), actor.clone(), accepted),
            &parent,
        )
        .await
        .unwrap();
    submitted(&threads, &child_scope, message, turn_id, child).await;
    assert_eq!(reservation_count(&rig.pool, &parent_scope, run_id).await, 1);
    coordinator
        .cancel_tree(CancelRunRequest {
            scope: child_scope.clone(),
            actor,
            run_id: child,
            reason: SanitizedCancelReason::UserRequested,
            idempotency_key: IdempotencyKey::new("atomic-success-cancel").unwrap(),
        })
        .await
        .unwrap();
    store
        .release_claimed_child_reservation(&child_scope, child)
        .await
        .unwrap();
    assert_eq!(reservation_count(&rig.pool, &parent_scope, run_id).await, 0);
}

#[tokio::test]
async fn native_atomic_child_admission_keeps_input_link_when_commit_ack_is_lost() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _database = rig.lock_db().await;
    let store = Arc::new(atomic_store(rig.pool.clone()));
    let coordinator = DefaultTurnCoordinator::new(store.clone());
    let threads = PgSessionThreadService::new(rig.pool.clone(), "claimed-child-native");
    let parent_scope = scope("atomic-lost-parent");
    let actor = TurnActor::new(UserId::new("claimed-child-owner").unwrap());
    let parent = claimed_parent(
        store.as_ref(),
        &coordinator,
        &threads,
        &parent_scope,
        &actor,
    )
    .await;
    let child_scope = scope("atomic-lost-child");
    let (accepted, message) = input(&threads, &child_scope).await;
    let request = child_request(&parent, child_scope.clone(), actor.clone(), accepted);
    let child = request.requested_run_id.unwrap();
    let proxy = commit_ack_proxy(rig.url.expose_secret()).await;
    let uncertain = DefaultTurnCoordinator::new(Arc::new(atomic_store(proxy.pool.clone())));
    proxy.arm(1).unwrap();
    let result = tokio::time::timeout(
        Duration::from_secs(15),
        uncertain.submit_claimed_child_run(request, &parent),
    )
    .await
    .unwrap();
    assert!(result.is_err());
    assert_eq!(proxy.cut_count(), 1);
    proxy.shutdown().await.unwrap();
    // Reconcile by reading the original IDs; do not submit or execute again.
    let record = store
        .get_run_record(&child_scope, child)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.parent_run_id, Some(parent.state.run_id));
    assert_eq!(record.status, brassclaw_turns::TurnStatus::Queued);
    assert_eq!(record.claim_count, 0);
    let bound = submitted(&threads, &child_scope, message, record.turn_id, child).await;
    let before = persisted_thread(&rig.pool, &child_scope).await;
    assert_eq!(
        threads
            .mark_message_submitted(
                &thread_scope(&child_scope),
                &child_scope.thread_id,
                message,
                record.turn_id.to_string(),
                child.to_string(),
            )
            .await
            .unwrap(),
        bound
    );
    assert_eq!(persisted_thread(&rig.pool, &child_scope).await, before);
    assert_eq!(
        reservation_count(&rig.pool, &parent_scope, parent.state.run_id).await,
        1
    );
    coordinator
        .cancel_tree(CancelRunRequest {
            scope: child_scope.clone(),
            actor,
            run_id: child,
            reason: SanitizedCancelReason::UserRequested,
            idempotency_key: IdempotencyKey::new("atomic-lost-cancel").unwrap(),
        })
        .await
        .unwrap();
    store
        .release_claimed_child_reservation(&child_scope, child)
        .await
        .unwrap();
    assert_eq!(
        reservation_count(&rig.pool, &parent_scope, parent.state.run_id).await,
        0
    );
}
