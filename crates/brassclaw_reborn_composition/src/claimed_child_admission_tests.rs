//! Actual PG turn admission with the original parent claim. This exercises the
//! coordinator, lifecycle wrapper and snapshot aggregate, not R09 activation.
use std::sync::Arc;

use brassclaw_host_api::{AgentId, TenantId, ThreadId, UserId};
use brassclaw_threads::{
    AcceptInboundMessageRequest, EnsureThreadRequest, MessageContent, PgSessionThreadService,
    SessionThreadService, ThreadMessageId, ThreadScope,
};
use brassclaw_turns::{
    AcceptedMessageRef, CancelRunRequest, DefaultTurnCoordinator, DefaultTurnLifecycleEventBus,
    GetRunStateRequest, IdempotencyKey, LifecyclePublishingTurnStateStore, PgTurnStateStore,
    ReplyTargetBindingRef, SanitizedCancelReason, SourceBindingRef, SubmitChildRunRequest,
    SubmitTurnRequest, SubmitTurnResponse, TurnActor, TurnCoordinator, TurnError, TurnLeaseToken,
    TurnRunId, TurnRunnerId, TurnScope, TurnSpawnTreePort, TurnSpawnTreeStateStore, TurnStateStore,
    runner::{ClaimRunRequest, TurnRunTransitionPort},
};

fn scope(thread: &str) -> TurnScope {
    TurnScope::new_with_owner(
        TenantId::new("claimed-child-native").unwrap(),
        Some(AgentId::new("claimed-child-agent").unwrap()),
        None,
        ThreadId::new(thread).unwrap(),
        Some(UserId::new("claimed-child-owner").unwrap()),
    )
}
fn thread_scope(scope: &TurnScope) -> ThreadScope {
    ThreadScope {
        tenant_id: scope.tenant_id.clone(),
        agent_id: scope.agent_id.clone().unwrap(),
        project_id: scope.project_id.clone(),
        owner_user_id: scope.explicit_owner_user_id().cloned(),
    }
}
async fn input(
    threads: &PgSessionThreadService,
    scope: &TurnScope,
) -> (AcceptedMessageRef, ThreadMessageId) {
    threads
        .ensure_thread(EnsureThreadRequest {
            scope: thread_scope(scope),
            thread_id: Some(scope.thread_id.clone()),
            created_by_actor_id: "claimed-child-owner".into(),
            title: None,
            metadata_json: None,
        })
        .await
        .unwrap();
    let message = threads
        .accept_inbound_message(AcceptInboundMessageRequest {
            scope: thread_scope(scope),
            thread_id: scope.thread_id.clone(),
            actor_id: "claimed-child-owner".into(),
            source_binding_id: None,
            reply_target_binding_id: None,
            external_event_id: None,
            content: MessageContent::text("Explicit child-admission input ä🙂\n\n"),
        })
        .await
        .unwrap();
    (
        AcceptedMessageRef::new(format!("msg:{}", message.message_id)).unwrap(),
        message.message_id,
    )
}

#[tokio::test]
async fn native_claimed_child_admission_retains_lineage_and_rejects_stale_parent() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let store = Arc::new(PgTurnStateStore::new(
        Arc::clone(&rig.pool),
        "claimed-child-native",
    ));
    let publishing = Arc::new(LifecyclePublishingTurnStateStore::new(
        store.clone(),
        Arc::new(DefaultTurnLifecycleEventBus::new()),
    ));
    let coordinator = Arc::new(DefaultTurnCoordinator::new(publishing));
    let threads = PgSessionThreadService::new(Arc::clone(&rig.pool), "claimed-child-native");
    let parent_scope = scope("opaque-parent-thread");
    let (accepted, parent_message) = input(&threads, &parent_scope).await;
    let actor = TurnActor::new(UserId::new("claimed-child-owner").unwrap());
    let SubmitTurnResponse::Accepted {
        turn_id,
        run_id: parent_run,
        ..
    } = coordinator
        .submit_turn(SubmitTurnRequest {
            scope: parent_scope.clone(),
            actor: actor.clone(),
            accepted_message_ref: accepted,
            source_binding_ref: SourceBindingRef::new("source:claimed-child").unwrap(),
            reply_target_binding_ref: ReplyTargetBindingRef::new("reply:claimed-child").unwrap(),
            requested_run_profile: None,
            idempotency_key: IdempotencyKey::new("claimed-parent-submit").unwrap(),
            received_at: chrono::Utc::now(),
            requested_run_id: None,
            parent_run_id: None,
            subagent_depth: 0,
            spawn_tree_root_run_id: None,
        })
        .await
        .unwrap();
    threads
        .mark_message_submitted(
            &thread_scope(&parent_scope),
            &parent_scope.thread_id,
            parent_message,
            turn_id.to_string(),
            parent_run.to_string(),
        )
        .await
        .unwrap();
    let claimed = store
        .claim_next_run(ClaimRunRequest {
            runner_id: TurnRunnerId::new(),
            lease_token: TurnLeaseToken::new(),
            scope_filter: Some(parent_scope.clone()),
        })
        .await
        .unwrap()
        .unwrap();
    let child_scope = scope("opaque-child-thread");
    let (accepted, child_message) = input(&threads, &child_scope).await;
    let child_run = TurnRunId::new();
    let request = SubmitChildRunRequest {
        parent_scope: parent_scope.clone(),
        parent_run_id: parent_run,
        child_scope: child_scope.clone(),
        actor: actor.clone(),
        accepted_message_ref: accepted,
        source_binding_ref: SourceBindingRef::new("source:claimed-child").unwrap(),
        reply_target_binding_ref: ReplyTargetBindingRef::new("reply:claimed-child").unwrap(),
        requested_run_profile: None,
        idempotency_key: IdempotencyKey::new("claimed-child-submit").unwrap(),
        received_at: chrono::Utc::now(),
        requested_run_id: Some(child_run),
        spawn_tree_descendant_cap: 3,
    };
    let mut stale = claimed.clone();
    stale.lease_token = TurnLeaseToken::new();
    assert!(matches!(
        coordinator
            .submit_claimed_child_run(request.clone(), &stale)
            .await,
        Err(TurnError::LeaseMismatch)
    ));
    assert!(
        store
            .children_of(&parent_scope, parent_run)
            .await
            .unwrap()
            .is_empty()
    );
    let mut foreign_actor = request.clone();
    foreign_actor.actor = TurnActor::new(UserId::new("foreign-claimed-child-actor").unwrap());
    assert!(matches!(
        coordinator
            .submit_claimed_child_run(foreign_actor.clone(), &claimed)
            .await,
        Err(TurnError::Conflict { .. })
    ));
    assert!(
        store
            .children_of(&parent_scope, parent_run)
            .await
            .unwrap()
            .is_empty()
    );
    let original = coordinator
        .submit_claimed_child_run(request.clone(), &claimed)
        .await
        .unwrap();
    let SubmitTurnResponse::Accepted {
        turn_id, run_id, ..
    } = original.clone();
    assert_eq!(run_id, child_run);
    threads
        .mark_message_submitted(
            &thread_scope(&child_scope),
            &child_scope.thread_id,
            child_message,
            turn_id.to_string(),
            child_run.to_string(),
        )
        .await
        .unwrap();
    let reopened = Arc::new(PgTurnStateStore::new(
        Arc::clone(&rig.pool),
        "claimed-child-native",
    ));
    let replay = DefaultTurnCoordinator::new(reopened.clone());
    assert!(matches!(
        replay
            .submit_claimed_child_run(foreign_actor, &claimed)
            .await,
        Err(TurnError::Conflict { .. })
    ));
    assert_eq!(
        replay
            .submit_claimed_child_run(request.clone(), &claimed)
            .await
            .unwrap(),
        original
    );
    assert!(matches!(
        replay
            .submit_claimed_child_run(request.clone(), &stale)
            .await,
        Err(TurnError::LeaseMismatch)
    ));
    let record = reopened
        .get_run_record(&child_scope, child_run)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.parent_run_id, Some(parent_run));
    assert_eq!(record.spawn_tree_root_run_id, Some(parent_run));
    assert_eq!(record.subagent_depth, 1);
    coordinator
        .cancel_tree(CancelRunRequest {
            scope: parent_scope.clone(),
            actor,
            run_id: parent_run,
            reason: SanitizedCancelReason::UserRequested,
            idempotency_key: IdempotencyKey::new("claimed-parent-cancel").unwrap(),
        })
        .await
        .unwrap();
    assert!(
        replay
            .submit_claimed_child_run(request, &claimed)
            .await
            .is_err()
    );
    assert_eq!(
        reopened
            .children_of(&parent_scope, parent_run)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        reopened
            .get_run_state(GetRunStateRequest {
                scope: child_scope,
                run_id: child_run
            })
            .await
            .unwrap()
            .run_id,
        child_run
    );
}
