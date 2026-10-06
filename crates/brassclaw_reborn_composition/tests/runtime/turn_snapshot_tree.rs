use super::common;

use std::sync::Arc;

use brassclaw_host_api::{TenantId, ThreadId, UserId};
use brassclaw_turns::{
    AcceptedMessageRef, AllowAllTurnAdmissionPolicy, GetRunStateRequest, IdempotencyKey,
    InMemoryRunProfileResolver, PgTurnStateStore, ReplyTargetBindingRef, SourceBindingRef,
    SubmitChildRunRequest, SubmitTurnRequest, SubmitTurnResponse, TurnActor, TurnRunId, TurnScope,
    TurnSpawnTreeStateStore, TurnStateStore,
};

fn scope(thread: &str) -> TurnScope {
    TurnScope::new(
        TenantId::new("tree-test").unwrap(),
        None,
        None,
        ThreadId::new(thread).unwrap(),
    )
}

async fn root(store: &PgTurnStateStore, scope: TurnScope) -> TurnRunId {
    let SubmitTurnResponse::Accepted { run_id, .. } = store
        .submit_turn(
            SubmitTurnRequest {
                scope,
                actor: TurnActor::new(UserId::new("owner").unwrap()),
                accepted_message_ref: AcceptedMessageRef::new("msg:fixture").unwrap(),
                source_binding_ref: SourceBindingRef::new("source:fixture").unwrap(),
                reply_target_binding_ref: ReplyTargetBindingRef::new("reply:fixture").unwrap(),
                requested_run_profile: None,
                idempotency_key: IdempotencyKey::new("root").unwrap(),
                received_at: chrono::Utc::now(),
                requested_run_id: None,
                parent_run_id: None,
                subagent_depth: 0,
                spawn_tree_root_run_id: None,
            },
            &AllowAllTurnAdmissionPolicy,
            &InMemoryRunProfileResolver::default(),
        )
        .await
        .unwrap();
    run_id
}

fn child(parent: TurnRunId, thread: &str) -> SubmitChildRunRequest {
    SubmitChildRunRequest {
        parent_scope: scope("parent"),
        parent_run_id: parent,
        child_scope: scope(thread),
        actor: TurnActor::new(UserId::new("owner").unwrap()),
        accepted_message_ref: AcceptedMessageRef::new("msg:child-fixture").unwrap(),
        source_binding_ref: SourceBindingRef::new("source:fixture").unwrap(),
        reply_target_binding_ref: ReplyTargetBindingRef::new("reply:fixture").unwrap(),
        requested_run_profile: None,
        idempotency_key: IdempotencyKey::new(thread).unwrap(),
        received_at: chrono::Utc::now(),
        requested_run_id: None,
        spawn_tree_descendant_cap: 1,
    }
}

#[tokio::test]
async fn cross_thread_children_share_atomic_capacity_and_keep_scoped_reads() {
    let rig = common::pg_rig().await;
    let store = PgTurnStateStore::new(Arc::clone(&rig.pool), "tree-test");
    let parent = root(&store, scope("parent")).await;
    let policy = AllowAllTurnAdmissionPolicy;
    let resolver = InMemoryRunProfileResolver::default();
    let (a, b) = tokio::join!(
        store.submit_child_turn(child(parent, "child-a"), &policy, &resolver),
        store.submit_child_turn(child(parent, "child-b"), &policy, &resolver),
    );
    assert_ne!(
        a.is_ok(),
        b.is_ok(),
        "exactly one child may consume the remaining capacity"
    );
    let children = store.children_of(&scope("parent"), parent).await.unwrap();
    assert_eq!(children.len(), 1);
    let child = &children[0];
    assert_eq!(
        store
            .get_run_state(GetRunStateRequest {
                scope: child.scope.clone(),
                run_id: child.run_id,
            })
            .await
            .unwrap()
            .run_id,
        child.run_id
    );
    let projection = store.interaction_snapshot(&child.scope).await.unwrap();
    assert_eq!(projection.runs.len(), 1);
    assert_eq!(projection.runs[0].run_id, child.run_id);
    assert!(
        store
            .get_run_state(GetRunStateRequest {
                scope: child.scope.clone(),
                run_id: parent,
            })
            .await
            .is_err()
    );
    let reopened = PgTurnStateStore::new(Arc::clone(&rig.pool), "tree-test");
    assert_eq!(
        reopened
            .children_of(&scope("parent"), parent)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn occupied_child_snapshot_rejects_link_without_mutating_either_tree() {
    let rig = common::pg_rig().await;
    let store = PgTurnStateStore::new(Arc::clone(&rig.pool), "tree-test");
    let parent = root(&store, scope("parent")).await;
    let independent = root(&store, scope("occupied-child")).await;
    assert!(
        store
            .submit_child_turn(
                child(parent, "occupied-child"),
                &AllowAllTurnAdmissionPolicy,
                &InMemoryRunProfileResolver::default()
            )
            .await
            .is_err()
    );
    assert!(
        store
            .children_of(&scope("parent"), parent)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        store
            .get_run_state(GetRunStateRequest {
                scope: scope("occupied-child"),
                run_id: independent,
            })
            .await
            .unwrap()
            .run_id,
        independent
    );
    let free = store
        .submit_child_turn(
            child(parent, "free-child"),
            &AllowAllTurnAdmissionPolicy,
            &InMemoryRunProfileResolver::default(),
        )
        .await;
    assert!(
        free.is_ok(),
        "rolled-back linkage must not consume descendant capacity: {free:?}"
    );
}

#[tokio::test]
async fn corrupted_run_snapshot_is_reported_instead_of_omitted() {
    let rig = common::pg_rig().await;
    let store = PgTurnStateStore::new(Arc::clone(&rig.pool), "tree-test");
    root(&store, scope("parent")).await;
    let client = rig.pool.get().await.unwrap();
    client.execute(
        "UPDATE brassclaw_turns SET payload = jsonb_set(payload, '{runs}', '[{}]'::jsonb) WHERE tenant_id = 'tree-test'",
        &[],
    ).await.unwrap();
    assert!(store.all_active_runs_snapshot().await.is_err());
    assert!(store.interaction_snapshot(&scope("parent")).await.is_err());
}
