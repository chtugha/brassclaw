//! Contract tests for [`FilesystemTurnStateStore`] against a
//! [`ScopedFilesystem`] over actual local bytes and versioned record owners.
//! Unsupported local transitions must preserve the original. The shape is a
//! single `/turns/state.json` snapshot keyed by the [`MountView`] target.

use std::sync::Arc;

use brassclaw_filesystem::{
    FilesystemError, InMemoryBackend, LocalFilesystem, RootFilesystem, ScopedFilesystem,
};
use brassclaw_host_api::{
    AgentId, HostPath, MountAlias, MountGrant, MountPermissions, MountView, ProjectId, TenantId,
    ThreadId, UserId, VirtualPath,
};
use brassclaw_turns::{
    AcceptedMessageRef, AllowAllTurnAdmissionPolicy, FilesystemTurnStateStore, GetRunStateRequest,
    IdempotencyKey, InMemoryRunProfileResolver, ReplyTargetBindingRef, RunProfileRequest,
    SourceBindingRef, SubmitChildRunRequest, SubmitTurnRequest, SubmitTurnResponse, TurnActor,
    TurnError, TurnLeaseToken, TurnRunId, TurnRunnerId, TurnScope, TurnSpawnTreeStateStore,
    TurnStateStore, TurnStatus,
    runner::{ClaimRunRequest, RecoverExpiredLeasesRequest, TurnRunTransitionPort},
};
use chrono::{TimeZone, Utc};

/// Build a [`LocalFilesystem`] with `/engine` mounted to a tempdir; the
/// `/turns` alias on the outer [`ScopedFilesystem`] resolves under
/// `/engine/...` per the test convention used by the run-state contract.
fn engine_filesystem() -> (Arc<LocalFilesystem>, tempfile::TempDir) {
    let storage = tempfile::tempdir().unwrap();
    let mut fs = LocalFilesystem::new();
    fs.mount_local(
        VirtualPath::new("/engine").unwrap(),
        HostPath::from_path_buf(storage.path().to_path_buf()),
    )
    .unwrap();
    (Arc::new(fs), storage)
}

/// Wrap a [`RootFilesystem`] in a [`ScopedFilesystem`] that exposes the
/// `/turns` mount alias under a tenant/user-scoped subtree of the underlying
/// mount target.
fn scoped_turns_fs_at<F>(backend: Arc<F>, tenant: &str, user: &str) -> Arc<ScopedFilesystem<F>>
where
    F: RootFilesystem,
{
    let tenant_user_prefix = format!("/engine/tenants/{tenant}/users/{user}");
    let mounts = MountView::new(vec![MountGrant::new(
        MountAlias::new("/turns").expect("alias"),
        VirtualPath::new(format!("{tenant_user_prefix}/turns")).expect("target"),
        MountPermissions::read_write_list_delete(),
    )])
    .expect("mount view");
    Arc::new(ScopedFilesystem::with_fixed_view(backend, mounts))
}

fn scoped_turns_fs<F>(backend: Arc<F>) -> Arc<ScopedFilesystem<F>>
where
    F: RootFilesystem,
{
    scoped_turns_fs_at(backend, "test-tenant", "test-user")
}

fn snapshot_virtual_path() -> VirtualPath {
    VirtualPath::new("/engine/tenants/test-tenant/users/test-user/turns/state.json").unwrap()
}

fn turn_scope(thread: &str) -> TurnScope {
    TurnScope::new(
        TenantId::new("tenant1").unwrap(),
        Some(AgentId::new("agent1").unwrap()),
        Some(ProjectId::new("project1").unwrap()),
        ThreadId::new(thread).unwrap(),
    )
}

fn turn_actor() -> TurnActor {
    TurnActor::new(UserId::new("user1").unwrap())
}

fn submit_request_for(scope: TurnScope, idempotency_key: &str) -> SubmitTurnRequest {
    SubmitTurnRequest {
        scope,
        actor: turn_actor(),
        accepted_message_ref: AcceptedMessageRef::new(format!("message-{idempotency_key}"))
            .unwrap(),
        source_binding_ref: SourceBindingRef::new("source-web").unwrap(),
        reply_target_binding_ref: ReplyTargetBindingRef::new("reply-web").unwrap(),
        requested_run_profile: Some(RunProfileRequest::new("default").unwrap()),
        idempotency_key: IdempotencyKey::new(idempotency_key).unwrap(),
        received_at: Utc.with_ymd_and_hms(2026, 5, 17, 12, 0, 0).unwrap(),
        requested_run_id: None,
        parent_run_id: None,
        subagent_depth: 0,
        spawn_tree_root_run_id: None,
    }
}

fn accepted_run_id(response: &SubmitTurnResponse) -> TurnRunId {
    let SubmitTurnResponse::Accepted { run_id, .. } = response;
    *run_id
}

#[tokio::test]
async fn filesystem_turn_state_store_does_not_write_unchanged_idle_runner_snapshot() {
    let (backend, _storage_guard) = engine_filesystem();
    let scoped = scoped_turns_fs(Arc::clone(&backend));
    let store = FilesystemTurnStateStore::new(scoped);

    let claimed = store
        .claim_next_run(ClaimRunRequest {
            runner_id: TurnRunnerId::new(),
            lease_token: TurnLeaseToken::new(),
            scope_filter: None,
        })
        .await
        .unwrap();
    assert!(claimed.is_none());

    let recovered = store
        .recover_expired_leases(RecoverExpiredLeasesRequest {
            now: Utc.with_ymd_and_hms(2026, 5, 27, 0, 12, 0).unwrap(),
            scope_filter: None,
        })
        .await
        .unwrap();
    assert!(recovered.recovered.is_empty());

    let err = backend
        .read_file(&snapshot_virtual_path())
        .await
        .unwrap_err();
    assert!(
        matches!(err, FilesystemError::NotFound { .. }),
        "idle no-op runner polling must not create or rewrite the snapshot: {err:?}"
    );
}

fn child_run_request(
    parent_scope: TurnScope,
    parent_run_id: TurnRunId,
    child_scope: TurnScope,
    idempotency_key: &str,
    cap: u32,
) -> SubmitChildRunRequest {
    SubmitChildRunRequest {
        parent_scope,
        parent_run_id,
        child_scope,
        actor: turn_actor(),
        accepted_message_ref: AcceptedMessageRef::new(format!("message-{idempotency_key}"))
            .unwrap(),
        source_binding_ref: SourceBindingRef::new("source-web").unwrap(),
        reply_target_binding_ref: ReplyTargetBindingRef::new("reply-web").unwrap(),
        requested_run_profile: Some(RunProfileRequest::new("default").unwrap()),
        idempotency_key: IdempotencyKey::new(idempotency_key).unwrap(),
        received_at: Utc.with_ymd_and_hms(2026, 5, 17, 12, 0, 0).unwrap(),
        requested_run_id: Some(TurnRunId::new()),
        spawn_tree_descendant_cap: cap,
    }
}

#[tokio::test]
async fn filesystem_claimed_child_release_is_exact_after_reopen_and_preserves_unknown_legacy() {
    let scoped = scoped_turns_fs(Arc::new(InMemoryBackend::new()));
    let store = FilesystemTurnStateStore::new(scoped.clone());
    let resolver = InMemoryRunProfileResolver::default();
    let parent_scope = turn_scope("release-parent");
    let parent = accepted_run_id(
        &store
            .submit_turn(
                submit_request_for(parent_scope.clone(), "release-parent"),
                &AllowAllTurnAdmissionPolicy,
                &resolver,
            )
            .await
            .unwrap(),
    );
    let claim = store
        .claim_next_run(ClaimRunRequest {
            runner_id: TurnRunnerId::new(),
            lease_token: TurnLeaseToken::new(),
            scope_filter: Some(parent_scope.clone()),
        })
        .await
        .unwrap()
        .unwrap();
    let child_scope = turn_scope("release-child");
    let child = accepted_run_id(
        &store
            .submit_claimed_child_turn(
                child_run_request(
                    parent_scope.clone(),
                    parent,
                    child_scope.clone(),
                    "release-child",
                    1,
                ),
                &claim,
                &AllowAllTurnAdmissionPolicy,
                &resolver,
            )
            .await
            .unwrap(),
    );
    assert!(
        store
            .release_claimed_child_reservation(&child_scope, child)
            .await
            .is_err()
    );
    store
        .request_cancel(brassclaw_turns::CancelRunRequest {
            scope: child_scope.clone(),
            actor: turn_actor(),
            run_id: child,
            reason: brassclaw_turns::SanitizedCancelReason::UserRequested,
            idempotency_key: IdempotencyKey::new("release-child-cancel").unwrap(),
        })
        .await
        .unwrap();
    let pending = store.persistence_snapshot().await.unwrap();
    let aggressive = brassclaw_turns::InMemoryTurnStateStore::from_persistence_snapshot(
        pending.clone(),
        brassclaw_turns::InMemoryTurnStateStoreLimits {
            max_terminal_records: 0,
            ..Default::default()
        },
    )
    .unwrap();
    let unrelated_scope = turn_scope("release-unrelated-pruning");
    let unrelated = accepted_run_id(
        &aggressive
            .submit_turn(
                submit_request_for(unrelated_scope.clone(), "release-unrelated-pruning"),
                &AllowAllTurnAdmissionPolicy,
                &resolver,
            )
            .await
            .unwrap(),
    );
    aggressive
        .request_cancel(brassclaw_turns::CancelRunRequest {
            scope: unrelated_scope,
            actor: turn_actor(),
            run_id: unrelated,
            reason: brassclaw_turns::SanitizedCancelReason::UserRequested,
            idempotency_key: IdempotencyKey::new("release-unrelated-pruning-cancel").unwrap(),
        })
        .await
        .unwrap();
    assert!(
        aggressive
            .get_run_record(&child_scope, child)
            .await
            .unwrap()
            .is_some(),
        "terminal cache eviction must preserve an unacknowledged claimed reservation"
    );
    let mut legacy = pending;
    legacy
        .runs
        .iter_mut()
        .find(|run| run.run_id == child)
        .unwrap()
        .claimed_child_reservation_released = None;
    let legacy = brassclaw_turns::InMemoryTurnStateStore::from_persistence_snapshot(
        legacy,
        brassclaw_turns::InMemoryTurnStateStoreLimits::default(),
    )
    .unwrap();
    assert!(
        matches!(
            legacy
                .release_claimed_child_reservation(&child_scope, child)
                .await,
            Err(TurnError::Unavailable { .. })
        ),
        "missing old evidence never implies an exact release"
    );
    assert_eq!(
        legacy.persistence_snapshot().spawn_tree_reservations[0].descendant_count,
        1
    );
    store
        .release_claimed_child_reservation(&child_scope, child)
        .await
        .unwrap();
    let reopened = FilesystemTurnStateStore::new(scoped);
    reopened
        .release_claimed_child_reservation(&child_scope, child)
        .await
        .unwrap();
    let saved = reopened.persistence_snapshot().await.unwrap();
    assert!(saved.spawn_tree_reservations.is_empty());
    assert_eq!(
        saved
            .runs
            .iter()
            .find(|run| run.run_id == child)
            .unwrap()
            .claimed_child_reservation_released,
        Some(true)
    );
}

#[tokio::test]
async fn filesystem_turn_state_store_persists_submit_and_reopens() {
    let (backend, _storage_guard) = engine_filesystem();
    let scoped = scoped_turns_fs(Arc::clone(&backend));
    let store = FilesystemTurnStateStore::new(Arc::clone(&scoped));
    let resolver = InMemoryRunProfileResolver::default();

    let request = submit_request_for(turn_scope("thread-fs-persist"), "idem-fs-persist");
    let response = store
        .submit_turn(request.clone(), &AllowAllTurnAdmissionPolicy, &resolver)
        .await
        .unwrap();
    let run_id = accepted_run_id(&response);

    // Re-construct the store over the same scoped filesystem; the on-disk
    // snapshot must rehydrate the queued run.
    let reopened = FilesystemTurnStateStore::new(scoped);
    let state = reopened
        .get_run_state(GetRunStateRequest {
            scope: request.scope,
            run_id,
        })
        .await
        .unwrap();
    assert_eq!(state.run_id, run_id);
    assert_eq!(state.status, TurnStatus::Queued);
}

#[tokio::test]
async fn filesystem_turn_state_store_hides_records_from_other_tenants_via_mount_view() {
    // Regression for the ScopedFilesystem migration: two stores share one
    // underlying RootFilesystem but each is constructed with a MountView
    // whose `/turns` alias resolves to a different tenant-scoped VirtualPath
    // subtree. Writing the same (thread, idempotency_key) on tenant A's
    // store must NOT make the snapshot visible from tenant B's store. The
    // structural fix routes every op through ScopedFilesystem; two
    // MountViews over the same backend cannot see each other's snapshots.
    let (backend, _storage_guard) = engine_filesystem();
    let scoped_a = scoped_turns_fs_at(Arc::clone(&backend), "tenant-a", "alice");
    let scoped_b = scoped_turns_fs_at(Arc::clone(&backend), "tenant-b", "alice");

    let store_a = FilesystemTurnStateStore::new(Arc::clone(&scoped_a));
    let store_b = FilesystemTurnStateStore::new(Arc::clone(&scoped_b));
    let resolver = InMemoryRunProfileResolver::default();

    let scope_a = TurnScope::new(
        TenantId::new("tenant-a").unwrap(),
        Some(AgentId::new("agent1").unwrap()),
        Some(ProjectId::new("project1").unwrap()),
        ThreadId::new("thread-cross-tenant").unwrap(),
    );
    let scope_b = TurnScope::new(
        TenantId::new("tenant-b").unwrap(),
        Some(AgentId::new("agent1").unwrap()),
        Some(ProjectId::new("project1").unwrap()),
        ThreadId::new("thread-cross-tenant").unwrap(),
    );

    let response_a = store_a
        .submit_turn(
            submit_request_for(scope_a.clone(), "idem-cross-tenant"),
            &AllowAllTurnAdmissionPolicy,
            &resolver,
        )
        .await
        .unwrap();
    let run_id_a = accepted_run_id(&response_a);

    // Tenant A sees its own run.
    let state_a = store_a
        .get_run_state(GetRunStateRequest {
            scope: scope_a.clone(),
            run_id: run_id_a,
        })
        .await
        .unwrap();
    assert_eq!(state_a.run_id, run_id_a);

    // Tenant B does NOT see tenant A's run id, despite the identical
    // (thread, idempotency_key). The mount target prefix in tenant B's
    // ScopedFilesystem resolves to a disjoint VirtualPath, so the snapshot
    // is absent and `get_run_state` reports `ScopeNotFound`.
    let err = store_b
        .get_run_state(GetRunStateRequest {
            scope: scope_b.clone(),
            run_id: run_id_a,
        })
        .await
        .expect_err("tenant B must NOT see tenant A's run (cross-tenant snapshot leak)");
    assert!(matches!(err, brassclaw_turns::TurnError::ScopeNotFound));

    // Tenant B can independently submit with the same idempotency_key and
    // observe its own run id, distinct from tenant A's.
    let response_b = store_b
        .submit_turn(
            submit_request_for(scope_b.clone(), "idem-cross-tenant"),
            &AllowAllTurnAdmissionPolicy,
            &resolver,
        )
        .await
        .unwrap();
    let run_id_b = accepted_run_id(&response_b);
    assert_ne!(
        run_id_a, run_id_b,
        "each tenant snapshot must mint its own run id; collision implies leakage"
    );
}

#[tokio::test]
async fn filesystem_turn_state_store_persists_lineage_and_tree_reservations() {
    let backend = Arc::new(InMemoryBackend::new());
    let scoped = scoped_turns_fs(Arc::clone(&backend));
    let store = FilesystemTurnStateStore::new(Arc::clone(&scoped));
    let resolver = InMemoryRunProfileResolver::default();

    let parent_scope = turn_scope("thread-fs-parent");
    let parent = accepted_run_id(
        &store
            .submit_turn(
                submit_request_for(parent_scope.clone(), "idem-fs-parent"),
                &AllowAllTurnAdmissionPolicy,
                &resolver,
            )
            .await
            .unwrap(),
    );

    let child_scope = turn_scope("thread-fs-child");
    let child_run_id = accepted_run_id(
        &store
            .submit_child_turn(
                child_run_request(
                    parent_scope.clone(),
                    parent,
                    child_scope.clone(),
                    "idem-fs-child",
                    3,
                ),
                &AllowAllTurnAdmissionPolicy,
                &resolver,
            )
            .await
            .unwrap(),
    );

    let child_b_scope = turn_scope("thread-fs-child-b");
    let reservation = store
        .reserve_tree_descendants(&child_scope, parent, 1, 3)
        .await
        .unwrap();
    assert_eq!(reservation.descendant_count, 2);
    assert!(matches!(
        store
            .reserve_tree_descendants(&child_b_scope, parent, 2, 3)
            .await,
        Err(TurnError::CapacityExceeded { .. })
    ));
    store
        .release_tree_descendants(&child_b_scope, parent, 1)
        .await
        .unwrap();

    let reopened = FilesystemTurnStateStore::new(scoped);
    let children = reopened.children_of(&parent_scope, parent).await.unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].run_id, child_run_id);
    assert_eq!(children[0].parent_run_id, Some(parent));
    assert_eq!(
        reopened
            .get_run_record(&child_scope, child_run_id)
            .await
            .unwrap()
            .unwrap()
            .spawn_tree_root_run_id,
        Some(parent)
    );
    assert_eq!(
        reopened
            .reserve_tree_descendants(&child_b_scope, parent, 1, 3)
            .await
            .unwrap()
            .descendant_count,
        2
    );
}

#[tokio::test]
async fn filesystem_spawn_tree_reads_are_scope_checked() {
    let backend = Arc::new(InMemoryBackend::new());
    let scoped = scoped_turns_fs(Arc::clone(&backend));
    let store = FilesystemTurnStateStore::new(Arc::clone(&scoped));
    let resolver = InMemoryRunProfileResolver::default();

    let parent_scope = turn_scope("thread-fs-scope-parent");
    let parent = accepted_run_id(
        &store
            .submit_turn(
                submit_request_for(parent_scope.clone(), "idem-fs-scope-parent"),
                &AllowAllTurnAdmissionPolicy,
                &resolver,
            )
            .await
            .unwrap(),
    );
    let child_scope = turn_scope("thread-fs-scope-child");
    let child = accepted_run_id(
        &store
            .submit_child_turn(
                child_run_request(
                    parent_scope.clone(),
                    parent,
                    child_scope.clone(),
                    "idem-fs-scope-child",
                    4,
                ),
                &AllowAllTurnAdmissionPolicy,
                &resolver,
            )
            .await
            .unwrap(),
    );

    let reopened = FilesystemTurnStateStore::new(scoped);
    assert_eq!(
        reopened
            .children_of(&parent_scope, parent)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        reopened
            .children_of(&child_scope, parent)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        reopened
            .children_of(&parent_scope, TurnRunId::new())
            .await
            .unwrap()
            .is_empty()
    );

    let foreign_scope = TurnScope::new(
        TenantId::new("foreign-tenant").unwrap(),
        Some(AgentId::new("agent1").unwrap()),
        Some(ProjectId::new("project1").unwrap()),
        ThreadId::new("thread-fs-scope-parent").unwrap(),
    );
    assert!(
        reopened
            .children_of(&foreign_scope, parent)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        reopened
            .get_run_record(&foreign_scope, parent)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        reopened
            .get_run_record(&parent_scope, child)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        reopened
            .get_run_record(&child_scope, child)
            .await
            .unwrap()
            .unwrap()
            .run_id,
        child
    );
}

#[tokio::test]
async fn filesystem_claimed_child_admission_keeps_original_parent_fence_after_reopen() {
    let scoped = scoped_turns_fs(Arc::new(InMemoryBackend::new()));
    let store = FilesystemTurnStateStore::new(scoped.clone());
    let resolver = InMemoryRunProfileResolver::default();
    let parent_scope = turn_scope("claimed-fs-parent");
    let parent = accepted_run_id(
        &store
            .submit_turn(
                submit_request_for(parent_scope.clone(), "claimed-fs-parent-submit"),
                &AllowAllTurnAdmissionPolicy,
                &resolver,
            )
            .await
            .unwrap(),
    );
    let claimed = store
        .claim_next_run(ClaimRunRequest {
            runner_id: TurnRunnerId::new(),
            lease_token: TurnLeaseToken::new(),
            scope_filter: Some(parent_scope.clone()),
        })
        .await
        .unwrap()
        .unwrap();
    let request = child_run_request(
        parent_scope.clone(),
        parent,
        turn_scope("claimed-fs-child"),
        "claimed-fs-child-submit",
        3,
    );
    let before = store.persistence_snapshot().await.unwrap();
    let mut foreign_actor = request.clone();
    foreign_actor.actor = TurnActor::new(UserId::new("foreign-claimed-child-actor").unwrap());
    assert!(matches!(
        store
            .submit_claimed_child_turn(
                foreign_actor.clone(),
                &claimed,
                &AllowAllTurnAdmissionPolicy,
                &resolver,
            )
            .await,
        Err(TurnError::Conflict { .. })
    ));
    assert_eq!(store.persistence_snapshot().await.unwrap(), before);
    let accepted = store
        .submit_claimed_child_turn(
            request.clone(),
            &claimed,
            &AllowAllTurnAdmissionPolicy,
            &resolver,
        )
        .await
        .unwrap();
    let reopened = FilesystemTurnStateStore::new(scoped);
    assert!(matches!(
        reopened
            .submit_claimed_child_turn(
                foreign_actor,
                &claimed,
                &AllowAllTurnAdmissionPolicy,
                &resolver,
            )
            .await,
        Err(TurnError::Conflict { .. })
    ));
    let mut stale = claimed.clone();
    stale.lease_token = TurnLeaseToken::new();
    assert!(matches!(
        reopened
            .submit_claimed_child_turn(
                request.clone(),
                &stale,
                &AllowAllTurnAdmissionPolicy,
                &resolver
            )
            .await,
        Err(TurnError::LeaseMismatch)
    ));
    assert_eq!(
        reopened
            .submit_claimed_child_turn(request, &claimed, &AllowAllTurnAdmissionPolicy, &resolver)
            .await
            .unwrap(),
        accepted
    );
    assert_eq!(
        reopened
            .children_of(&parent_scope, parent)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn filesystem_claimed_child_admission_rejects_byte_only_cas_without_a_write() {
    // Build a real claimed snapshot on a versioned owner, then reopen those
    // exact retained bytes on LocalFilesystem. Neither ordinary nor claimed
    // transitions may replace that snapshot without supported versioned CAS.
    let (backend, _storage_guard) = engine_filesystem();
    let prepared = FilesystemTurnStateStore::new(scoped_turns_fs(Arc::new(InMemoryBackend::new())));
    let store = &prepared;
    let resolver = InMemoryRunProfileResolver::default();
    let parent_scope = turn_scope("claimed-byte-only-parent");
    let parent = accepted_run_id(
        &store
            .submit_turn(
                submit_request_for(parent_scope.clone(), "claimed-byte-only-parent-submit"),
                &AllowAllTurnAdmissionPolicy,
                &resolver,
            )
            .await
            .unwrap(),
    );
    let claimed = store
        .claim_next_run(ClaimRunRequest {
            runner_id: TurnRunnerId::new(),
            lease_token: TurnLeaseToken::new(),
            scope_filter: Some(parent_scope.clone()),
        })
        .await
        .unwrap()
        .unwrap();
    let before = store.persistence_snapshot().await.unwrap();
    backend
        .put(
            &snapshot_virtual_path(),
            brassclaw_filesystem::Entry::bytes(serde_json::to_vec_pretty(&before).unwrap()),
            brassclaw_filesystem::CasExpectation::Absent,
        )
        .await
        .unwrap();
    let store = FilesystemTurnStateStore::new(scoped_turns_fs(backend));
    assert!(matches!(
        store
            .submit_claimed_child_turn(
                child_run_request(
                    parent_scope,
                    parent,
                    turn_scope("claimed-byte-only-child"),
                    "claimed-byte-only-child-submit",
                    3,
                ),
                &claimed,
                &AllowAllTurnAdmissionPolicy,
                &resolver,
            )
            .await,
        Err(TurnError::Unavailable { .. })
    ));
    assert_eq!(store.persistence_snapshot().await.unwrap(), before);
}

#[tokio::test]
async fn local_byte_only_ordinary_claim_preserves_queued_original() {
    let (backend, _storage_guard) = engine_filesystem();
    let store = FilesystemTurnStateStore::new(scoped_turns_fs(backend));
    store
        .submit_turn(
            submit_request_for(turn_scope("local-original"), "local-original"),
            &AllowAllTurnAdmissionPolicy,
            &InMemoryRunProfileResolver::default(),
        )
        .await
        .unwrap();
    let before = store.persistence_snapshot().await.unwrap();
    assert!(matches!(
        store
            .claim_next_run(ClaimRunRequest {
                runner_id: TurnRunnerId::new(),
                lease_token: TurnLeaseToken::new(),
                scope_filter: None
            })
            .await,
        Err(TurnError::Unavailable { .. })
    ));
    assert_eq!(store.persistence_snapshot().await.unwrap(), before);
}

#[tokio::test]
async fn filesystem_native_auth_wait_reopens_and_resumes_original_claim_without_worker_queue() {
    use brassclaw_host_api::{AuthDispatchBoundary, InvocationId};
    use brassclaw_turns::{
        ResumeTurnPrecondition, ResumeTurnRequest,
        runner::{HeartbeatRequest, native_auth::BlockNativeAuthRequest},
    };

    // Use the existing versioned record owner. Byte-only LocalFilesystem
    // deliberately rejects record replacement rather than faking atomic CAS.
    let scoped = scoped_turns_fs(Arc::new(InMemoryBackend::new()));
    let store = FilesystemTurnStateStore::new(scoped.clone());
    let scope = turn_scope("native-auth-filesystem");
    store
        .submit_turn(
            submit_request_for(scope.clone(), "native-auth-fs-submit"),
            &AllowAllTurnAdmissionPolicy,
            &InMemoryRunProfileResolver::default(),
        )
        .await
        .unwrap();
    let claim = store
        .claim_next_run(ClaimRunRequest {
            runner_id: TurnRunnerId::new(),
            lease_token: TurnLeaseToken::new(),
            scope_filter: Some(scope.clone()),
        })
        .await
        .unwrap()
        .unwrap();
    let wait = BlockNativeAuthRequest {
        scope: scope.clone(),
        run_id: claim.state.run_id,
        runner_id: claim.runner_id,
        lease_token: claim.lease_token,
        gate_ref: brassclaw_turns::GateRef::new(format!(
            "gate:auth:native:{}",
            InvocationId::new()
        ))
        .unwrap(),
        boundary: AuthDispatchBoundary::BeforeDispatch,
        credential_requirements: Vec::new(),
    };
    let blocked = store.block_native_auth(wait.clone()).await.unwrap();
    let original = store.persistence_snapshot().await.unwrap();
    assert_eq!(original.runs[0].claim_count, 1);
    assert_eq!(original.runs[0].runner_id, Some(claim.runner_id));
    assert!(original.checkpoints.is_empty());
    drop(store);

    let reopened = FilesystemTurnStateStore::new(scoped.clone());
    let cursor = reopened
        .heartbeat(HeartbeatRequest {
            run_id: wait.run_id,
            runner_id: wait.runner_id,
            lease_token: wait.lease_token,
        })
        .await
        .unwrap();
    assert_eq!(cursor, blocked.event_cursor);
    assert_eq!(
        reopened.persistence_snapshot().await.unwrap().events,
        original.events
    );
    let input = ResumeTurnRequest {
        scope: scope.clone(),
        actor: turn_actor(),
        run_id: wait.run_id,
        gate_resolution_ref: blocked.gate_ref.unwrap(),
        source_binding_ref: blocked.source_binding_ref,
        reply_target_binding_ref: blocked.reply_target_binding_ref,
        idempotency_key: IdempotencyKey::new("native-auth-fs-resume").unwrap(),
        precondition: ResumeTurnPrecondition::BlockedAuthGate,
    };
    let resumed = reopened.resume_turn(input.clone()).await.unwrap();
    assert_eq!(resumed.status, TurnStatus::Running);
    drop(reopened);

    let resolved = FilesystemTurnStateStore::new(scoped);
    assert_eq!(resolved.resume_turn(input.clone()).await.unwrap(), resumed);
    let snapshot = resolved.persistence_snapshot().await.unwrap();
    assert_eq!(snapshot.runs[0].runner_id, Some(claim.runner_id));
    assert_eq!(snapshot.runs[0].lease_token, Some(claim.lease_token));
    assert_eq!(snapshot.runs[0].claim_count, 1);
    assert!(snapshot.checkpoints.is_empty());
    assert!(
        !snapshot.runs[0]
            .native_auth_wait
            .as_ref()
            .unwrap()
            .is_waiting()
    );
    let mut conflict = input;
    conflict.reply_target_binding_ref = ReplyTargetBindingRef::new("another-reply").unwrap();
    assert!(matches!(
        resolved.resume_turn(conflict).await,
        Err(TurnError::Conflict { .. })
    ));
    assert!(
        resolved
            .claim_next_run(ClaimRunRequest {
                runner_id: TurnRunnerId::new(),
                lease_token: TurnLeaseToken::new(),
                scope_filter: Some(scope),
            })
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(resolved.persistence_snapshot().await.unwrap(), snapshot);
}

#[tokio::test]
async fn filesystem_native_auth_wait_on_byte_only_owner_preserves_original_custody() {
    use brassclaw_host_api::{AuthDispatchBoundary, InvocationId};
    use brassclaw_turns::{
        ResumeTurnPrecondition, ResumeTurnRequest,
        runner::{HeartbeatRequest, native_auth::BlockNativeAuthRequest},
    };
    let prepared = FilesystemTurnStateStore::new(scoped_turns_fs(Arc::new(InMemoryBackend::new())));
    let scope = turn_scope("native-auth-byte-only");
    prepared
        .submit_turn(
            submit_request_for(scope.clone(), "native-auth-byte-submit"),
            &AllowAllTurnAdmissionPolicy,
            &InMemoryRunProfileResolver::default(),
        )
        .await
        .unwrap();
    let claim = prepared
        .claim_next_run(ClaimRunRequest {
            runner_id: TurnRunnerId::new(),
            lease_token: TurnLeaseToken::new(),
            scope_filter: Some(scope.clone()),
        })
        .await
        .unwrap()
        .unwrap();
    let wait = BlockNativeAuthRequest {
        scope: scope.clone(),
        run_id: claim.state.run_id,
        runner_id: claim.runner_id,
        lease_token: claim.lease_token,
        gate_ref: brassclaw_turns::GateRef::new(format!(
            "gate:auth:native:{}",
            InvocationId::new()
        ))
        .unwrap(),
        boundary: AuthDispatchBoundary::BeforeDispatch,
        credential_requirements: Vec::new(),
    };
    let blocked = prepared.block_native_auth(wait).await.unwrap();
    let original = prepared.persistence_snapshot().await.unwrap();
    let (backend, _storage_guard) = engine_filesystem();
    backend
        .put(
            &snapshot_virtual_path(),
            brassclaw_filesystem::Entry::bytes(serde_json::to_vec(&original).unwrap()),
            brassclaw_filesystem::CasExpectation::Absent,
        )
        .await
        .unwrap();
    let readonly = FilesystemTurnStateStore::new(scoped_turns_fs(backend));
    assert!(matches!(
        readonly
            .heartbeat(HeartbeatRequest {
                run_id: claim.state.run_id,
                runner_id: claim.runner_id,
                lease_token: claim.lease_token,
            })
            .await,
        Err(TurnError::Unavailable { .. })
    ));
    assert_eq!(readonly.persistence_snapshot().await.unwrap(), original);
    assert!(matches!(
        readonly
            .resume_turn(ResumeTurnRequest {
                scope,
                actor: turn_actor(),
                run_id: claim.state.run_id,
                gate_resolution_ref: blocked.gate_ref.unwrap(),
                source_binding_ref: blocked.source_binding_ref,
                reply_target_binding_ref: blocked.reply_target_binding_ref,
                idempotency_key: IdempotencyKey::new("native-auth-byte-resume").unwrap(),
                precondition: ResumeTurnPrecondition::BlockedAuthGate,
            })
            .await,
        Err(TurnError::Unavailable { .. })
    ));
    assert_eq!(readonly.persistence_snapshot().await.unwrap(), original);
}
