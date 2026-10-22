//! Real PostgreSQL reservation accounting, including a committed release whose
//! actual COMMIT acknowledgement is cut by a local byte transport.
use super::*;
use brassclaw_pg::PgPool;
use brassclaw_pg::test_support::{AckCutRule, AckCutTransport};
use secrecy::ExposeSecret;
use serde_json::Value;
use std::time::Duration;

#[path = "pg_inbound_atomic_tests.rs"]
mod atomic_inbound;

#[cfg(feature = "skills-db")]
#[path = "pg_transcript_admission_tests.rs"]
mod atomic_transcript;

async fn commit_ack_proxy(connection: &str) -> AckCutTransport {
    AckCutTransport::start(connection, vec![AckCutRule::next_commit(1)])
        .await
        .unwrap()
}

async fn reservation_count(pool: &PgPool, scope: &TurnScope, root: TurnRunId) -> u64 {
    let snapshot:Value = pool.get().await.unwrap().query_one(
        "SELECT payload FROM brassclaw_turns WHERE tenant_id=$1 AND status='snapshot' AND turn_id=$2",
        &[&scope.tenant_id.as_str(),&scope.thread_id.as_str()],
    ).await.unwrap().get(0);
    let snapshot: brassclaw_turns::TurnPersistenceSnapshot =
        serde_json::from_value(snapshot).unwrap();
    snapshot
        .spawn_tree_reservations
        .iter()
        .find(|reservation| reservation.root_run_id == root)
        .map(|reservation| reservation.descendant_count)
        .unwrap_or(0)
}

async fn claimed_parent(
    store: &PgTurnStateStore,
    coordinator: &dyn TurnCoordinator,
    threads: &PgSessionThreadService,
    parent_scope: &TurnScope,
    actor: &TurnActor,
) -> brassclaw_turns::runner::ClaimedTurnRun {
    let (accepted, message) = input(threads, parent_scope).await;
    let SubmitTurnResponse::Accepted {
        turn_id,
        run_id: root,
        ..
    } = coordinator
        .submit_turn(SubmitTurnRequest {
            scope: parent_scope.clone(),
            actor: actor.clone(),
            accepted_message_ref: accepted,
            source_binding_ref: SourceBindingRef::new("reservation-source").unwrap(),
            reply_target_binding_ref: ReplyTargetBindingRef::new("reservation-reply").unwrap(),
            requested_run_profile: None,
            idempotency_key: IdempotencyKey::new("reservation-parent").unwrap(),
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
            &thread_scope(parent_scope),
            &parent_scope.thread_id,
            message,
            turn_id.to_string(),
            root.to_string(),
        )
        .await
        .unwrap();
    store
        .claim_next_run(ClaimRunRequest {
            runner_id: TurnRunnerId::new(),
            lease_token: TurnLeaseToken::new(),
            scope_filter: Some(parent_scope.clone()),
        })
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn native_claimed_child_reservation_survives_lost_commit_ack_without_releasing_sibling() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _database = rig.lock_db().await;
    let store = Arc::new(PgTurnStateStore::new(
        rig.pool.clone(),
        "claimed-child-native",
    ));
    let coordinator = Arc::new(DefaultTurnCoordinator::new(store.clone()));
    let threads = PgSessionThreadService::new(rig.pool.clone(), "claimed-child-native");
    let parent_scope = scope("reservation-parent");
    let actor = TurnActor::new(UserId::new("claimed-child-owner").unwrap());
    let claim = claimed_parent(
        store.as_ref(),
        coordinator.as_ref(),
        &threads,
        &parent_scope,
        &actor,
    )
    .await;
    let root = claim.state.run_id;
    let mut children = Vec::new();
    for name in ["reservation-first", "reservation-sibling"] {
        let child_scope = scope(name);
        let (accepted, message) = input(&threads, &child_scope).await;
        let SubmitTurnResponse::Accepted {
            turn_id, run_id, ..
        } = coordinator
            .submit_claimed_child_run(
                SubmitChildRunRequest {
                    parent_scope: parent_scope.clone(),
                    parent_run_id: root,
                    child_scope: child_scope.clone(),
                    actor: actor.clone(),
                    accepted_message_ref: accepted,
                    source_binding_ref: SourceBindingRef::new(name).unwrap(),
                    reply_target_binding_ref: ReplyTargetBindingRef::new(name).unwrap(),
                    requested_run_profile: None,
                    idempotency_key: IdempotencyKey::new(name).unwrap(),
                    received_at: chrono::Utc::now(),
                    requested_run_id: Some(TurnRunId::new()),
                    spawn_tree_descendant_cap: 2,
                },
                &claim,
            )
            .await
            .unwrap();
        threads
            .mark_message_submitted(
                &thread_scope(&child_scope),
                &child_scope.thread_id,
                message,
                turn_id.to_string(),
                run_id.to_string(),
            )
            .await
            .unwrap();
        children.push((child_scope, run_id));
    }
    let (first_scope, first) = &children[0];
    let (sibling_scope, sibling) = &children[1];
    assert_eq!(reservation_count(&rig.pool, &parent_scope, root).await, 2);
    assert!(
        store
            .release_claimed_child_reservation(sibling_scope, *sibling)
            .await
            .is_err(),
        "a running or queued child cannot release its reservation"
    );
    coordinator
        .cancel_tree(CancelRunRequest {
            scope: first_scope.clone(),
            actor: actor.clone(),
            run_id: *first,
            reason: SanitizedCancelReason::UserRequested,
            idempotency_key: IdempotencyKey::new("reservation-first-cancel").unwrap(),
        })
        .await
        .unwrap();
    let proxy = commit_ack_proxy(rig.url.expose_secret()).await;
    let uncertain = PgTurnStateStore::new(proxy.pool.clone(), "claimed-child-native");
    proxy.arm(1).unwrap();
    let lost = tokio::time::timeout(
        Duration::from_secs(15),
        uncertain.release_claimed_child_reservation(first_scope, *first),
    )
    .await
    .unwrap();
    assert!(
        lost.is_err(),
        "the actual committed write must lose its acknowledgement"
    );
    assert_eq!(
        proxy.cut_count(),
        1,
        "a real COMMIT response must have been cut"
    );
    proxy.shutdown().await.unwrap();
    assert_eq!(reservation_count(&rig.pool, &parent_scope, root).await, 1);
    let reopened = PgTurnStateStore::new(rig.pool.clone(), "claimed-child-native");
    assert_eq!(
        reopened
            .get_run_record(first_scope, *first)
            .await
            .unwrap()
            .unwrap()
            .claimed_child_reservation_released,
        Some(true)
    );
    reopened
        .release_claimed_child_reservation(first_scope, *first)
        .await
        .unwrap();
    assert_eq!(
        reservation_count(&rig.pool, &parent_scope, root).await,
        1,
        "repeating the exact release cannot consume the sibling's slot"
    );
    assert!(
        reopened
            .release_tree_descendants(&parent_scope, root, 1)
            .await
            .is_err(),
        "an unkeyed legacy release cannot consume a claimed child's slot"
    );
    assert_eq!(reservation_count(&rig.pool, &parent_scope, root).await, 1);
    let mut foreign = first_scope.clone();
    foreign.thread_id = ThreadId::new("wrong-reservation-thread").unwrap();
    assert!(
        reopened
            .release_claimed_child_reservation(&foreign, *first)
            .await
            .is_err()
    );
    coordinator
        .cancel_tree(CancelRunRequest {
            scope: sibling_scope.clone(),
            actor,
            run_id: *sibling,
            reason: SanitizedCancelReason::UserRequested,
            idempotency_key: IdempotencyKey::new("reservation-sibling-cancel").unwrap(),
        })
        .await
        .unwrap();
    reopened
        .release_claimed_child_reservation(sibling_scope, *sibling)
        .await
        .unwrap();
    reopened
        .release_claimed_child_reservation(sibling_scope, *sibling)
        .await
        .unwrap();
    assert_eq!(reservation_count(&rig.pool, &parent_scope, root).await, 0);
}

#[tokio::test]
async fn native_lost_child_admission_ack_preserves_identity_and_cancels_below_failed_parent() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _database = rig.lock_db().await;
    let store = Arc::new(PgTurnStateStore::new(
        rig.pool.clone(),
        "claimed-child-native",
    ));
    let coordinator = DefaultTurnCoordinator::new(store.clone());
    let threads = PgSessionThreadService::new(rig.pool.clone(), "claimed-child-native");
    let parent_scope = scope("unknown-admission-parent");
    let actor = TurnActor::new(UserId::new("claimed-child-owner").unwrap());
    let claim = claimed_parent(
        store.as_ref(),
        &coordinator,
        &threads,
        &parent_scope,
        &actor,
    )
    .await;
    let root = claim.state.run_id;
    let child_scope = scope("unknown-admission-child");
    let (accepted, _) = input(&threads, &child_scope).await;
    let child = TurnRunId::new();
    let request = SubmitChildRunRequest {
        parent_scope: parent_scope.clone(),
        parent_run_id: root,
        child_scope: child_scope.clone(),
        actor: actor.clone(),
        accepted_message_ref: accepted.clone(),
        source_binding_ref: SourceBindingRef::new("unknown-admission-source").unwrap(),
        reply_target_binding_ref: ReplyTargetBindingRef::new("unknown-admission-reply").unwrap(),
        requested_run_profile: None,
        idempotency_key: IdempotencyKey::new("unknown-admission-child").unwrap(),
        received_at: chrono::Utc::now(),
        requested_run_id: Some(child),
        spawn_tree_descendant_cap: 2,
    };
    let proxy = commit_ack_proxy(rig.url.expose_secret()).await;
    let uncertain = DefaultTurnCoordinator::new(Arc::new(PgTurnStateStore::new(
        proxy.pool.clone(),
        "claimed-child-native",
    )));
    proxy.arm(1).unwrap();
    let lost = tokio::time::timeout(
        Duration::from_secs(15),
        uncertain.submit_claimed_child_run(request.clone(), &claim),
    )
    .await
    .unwrap();
    assert!(lost.is_err());
    assert_eq!(
        proxy.cut_count(),
        1,
        "the server's real child-admission COMMIT acknowledgement must be cut"
    );
    proxy.shutdown().await.unwrap();
    let actual = store
        .get_run_record(&child_scope, child)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(actual.parent_run_id, Some(root));
    assert_eq!(actual.spawn_tree_root_run_id, Some(root));
    assert_eq!(actual.accepted_message_ref, accepted);
    assert_eq!(actual.status, brassclaw_turns::TurnStatus::Queued);
    assert_eq!(actual.claim_count, 0);
    assert_eq!(actual.claimed_child_reservation_released, Some(false));
    assert_eq!(reservation_count(&rig.pool, &parent_scope, root).await, 1);

    // Admission may exist even though its caller received an error. Failure of
    // that parent fences a retry of the original claim, including an otherwise
    // identical idempotency key. It never mints a replacement child identity.
    store
        .fail_run(brassclaw_turns::runner::FailRunRequest {
            run_id: root,
            runner_id: claim.runner_id,
            lease_token: claim.lease_token,
            failure: brassclaw_turns::SanitizedFailure::new("child_admission_acknowledgement_lost")
                .unwrap(),
        })
        .await
        .unwrap();
    assert!(
        coordinator
            .submit_claimed_child_run(request, &claim)
            .await
            .is_err()
    );
    assert_eq!(
        store.children_of(&parent_scope, root).await.unwrap().len(),
        1
    );
    let cancel = CancelRunRequest {
        scope: parent_scope.clone(),
        actor: actor.clone(),
        run_id: root,
        reason: SanitizedCancelReason::UserRequested,
        idempotency_key: IdempotencyKey::new("unknown-admission-tree-cancel").unwrap(),
    };
    let mut foreign = cancel.clone();
    foreign.actor = TurnActor::new(UserId::new("another-parent-owner").unwrap());
    assert!(matches!(
        coordinator.cancel_tree(foreign).await,
        Err(TurnError::Unauthorized)
    ));
    assert_eq!(
        store
            .get_run_record(&child_scope, child)
            .await
            .unwrap()
            .unwrap()
            .status,
        brassclaw_turns::TurnStatus::Queued
    );
    let results = coordinator.cancel_tree(cancel.clone()).await.unwrap();
    assert_eq!(results.len(), 2);
    let parent = results
        .iter()
        .find(|(_, response)| response.run_id == root)
        .unwrap();
    assert!(parent.1.already_terminal);
    assert_eq!(parent.1.status, brassclaw_turns::TurnStatus::Failed);
    assert_eq!(
        store
            .get_run_record(&child_scope, child)
            .await
            .unwrap()
            .unwrap()
            .status,
        brassclaw_turns::TurnStatus::Cancelled
    );
    store
        .release_claimed_child_reservation(&child_scope, child)
        .await
        .unwrap();
    coordinator.cancel_tree(cancel).await.unwrap();
    store
        .release_claimed_child_reservation(&child_scope, child)
        .await
        .unwrap();
    assert_eq!(reservation_count(&rig.pool, &parent_scope, root).await, 0);
    let final_child = store
        .get_run_record(&child_scope, child)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(final_child.claim_count, 0);
    assert_eq!(final_child.claimed_child_reservation_released, Some(true));
    assert_eq!(
        store.children_of(&parent_scope, root).await.unwrap().len(),
        1
    );
    let parent = coordinator
        .get_run_state(GetRunStateRequest {
            scope: parent_scope,
            run_id: root,
        })
        .await
        .unwrap();
    assert_eq!(parent.status, brassclaw_turns::TurnStatus::Failed);
    assert_eq!(
        parent.failure.unwrap().category(),
        "child_admission_acknowledgement_lost"
    );
}
