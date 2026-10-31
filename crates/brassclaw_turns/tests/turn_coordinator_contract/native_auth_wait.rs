//! Actual neutral-store custody tests, not global Monty/Auth/component activation.
use super::*;
use brassclaw_host_api::{AuthDispatchBoundary, InvocationId};
use brassclaw_turns::{
    ResumeTurnPrecondition,
    runner::native_auth::{BlockNativeAuthRequest, native_auth_gate},
};

async fn setup(
    thread: &str,
) -> (
    DefaultTurnCoordinator<InMemoryTurnStateStore>,
    Arc<InMemoryTurnStateStore>,
    ClaimedTurnRun,
    BlockNativeAuthRequest,
) {
    let (coordinator, store) = coordinator();
    coordinator
        .submit_turn(submit_request(thread, "native-wait-submit"))
        .await
        .unwrap();
    let claimed = store
        .claim_next_run(ClaimRunRequest {
            runner_id: TurnRunnerId::new(),
            lease_token: TurnLeaseToken::new(),
            scope_filter: Some(scope(thread)),
        })
        .await
        .unwrap()
        .unwrap();
    let request = BlockNativeAuthRequest {
        scope: claimed.state.scope.clone(),
        run_id: claimed.state.run_id,
        runner_id: claimed.runner_id,
        lease_token: claimed.lease_token,
        invocation_id: InvocationId::new(),
        boundary: AuthDispatchBoundary::BeforeDispatch,
        credential_requirements: Vec::new(),
    };
    (coordinator, store, claimed, request)
}
fn resume(request: &BlockNativeAuthRequest, state: &TurnRunState) -> ResumeTurnRequest {
    ResumeTurnRequest {
        scope: state.scope.clone(),
        actor: actor(),
        run_id: request.run_id,
        gate_resolution_ref: native_auth_gate(request.invocation_id).unwrap(),
        source_binding_ref: state.source_binding_ref.clone(),
        reply_target_binding_ref: state.reply_target_binding_ref.clone(),
        idempotency_key: IdempotencyKey::new("native-wait-resume").unwrap(),
        precondition: ResumeTurnPrecondition::BlockedAuthGate,
    }
}
fn next_claim(request: &BlockNativeAuthRequest) -> ClaimRunRequest {
    ClaimRunRequest {
        runner_id: TurnRunnerId::new(),
        lease_token: TurnLeaseToken::new(),
        scope_filter: Some(request.scope.clone()),
    }
}

#[tokio::test]
async fn native_auth_wait_retains_claim_across_snapshot_and_resumes_without_queue_or_checkpoint() {
    let (_, store, claimed, request) = setup("native-wait-custody").await;
    let before = store.persistence_snapshot();
    let state = store.block_native_auth(request.clone()).await.unwrap();
    assert_eq!(state.status, TurnStatus::BlockedAuth);
    assert!(state.checkpoint_id.is_none());
    assert_eq!(
        store.block_native_auth(request.clone()).await.unwrap(),
        state
    );
    let blocked = store.persistence_snapshot();
    assert_eq!(blocked.runs[0].runner_id, Some(claimed.runner_id));
    assert_eq!(blocked.runs[0].lease_token, Some(claimed.lease_token));
    assert_eq!(blocked.runs[0].claim_count, before.runs[0].claim_count);
    assert!(blocked.checkpoints.is_empty());
    let bytes = serde_json::to_vec(&blocked).unwrap();
    let reopened = InMemoryTurnStateStore::from_persistence_snapshot(
        serde_json::from_slice(&bytes).unwrap(),
        Default::default(),
    )
    .unwrap();
    let heartbeat_cursor = reopened
        .heartbeat(HeartbeatRequest {
            run_id: request.run_id,
            runner_id: claimed.runner_id,
            lease_token: claimed.lease_token,
        })
        .await
        .unwrap();
    assert_eq!(heartbeat_cursor, state.event_cursor);
    let renewed = reopened.persistence_snapshot();
    assert_eq!(renewed.events, blocked.events);
    assert_eq!(renewed.runs[0].event_cursor, state.event_cursor);
    assert!(renewed.runs[0].lease_expires_at >= blocked.runs[0].lease_expires_at);
    let input = resume(&request, &state);
    let response = reopened.resume_turn(input.clone()).await.unwrap();
    assert_eq!(response.status, TurnStatus::Running);
    assert_eq!(reopened.resume_turn(input).await.unwrap(), response);
    assert!(
        reopened
            .claim_next_run(next_claim(&request))
            .await
            .unwrap()
            .is_none()
    );
    let resolved = reopened.persistence_snapshot();
    assert_eq!(resolved.runs[0].claim_count, 1);
    assert_eq!(resolved.runs[0].runner_id, Some(claimed.runner_id));
    assert_eq!(resolved.runs[0].lease_token, Some(claimed.lease_token));
    assert!(
        !resolved.runs[0]
            .native_auth_wait
            .as_ref()
            .unwrap()
            .is_waiting()
    );
    assert!(
        reopened.block_native_auth(request).await.is_err(),
        "one resolved occurrence cannot create another auth wait"
    );
}

#[tokio::test]
async fn native_auth_wait_rejects_dispatch_unknown_stale_scope_and_changed_binding() {
    let (_, store, _, request) = setup("native-wait-reject").await;
    let before = store.persistence_snapshot();
    for boundary in [
        AuthDispatchBoundary::DispatchAttempted,
        AuthDispatchBoundary::Unproven,
    ] {
        let mut rejected = request.clone();
        rejected.boundary = boundary;
        assert!(store.block_native_auth(rejected).await.is_err());
    }
    let mut rejected = request.clone();
    rejected.lease_token = TurnLeaseToken::new();
    assert_eq!(
        store.block_native_auth(rejected).await.unwrap_err(),
        TurnError::LeaseMismatch
    );
    let mut rejected = request.clone();
    rejected.scope = scope("another-native-thread");
    assert_eq!(
        store.block_native_auth(rejected).await.unwrap_err(),
        TurnError::ScopeNotFound
    );
    assert_eq!(store.persistence_snapshot(), before);
    let state = store.block_native_auth(request.clone()).await.unwrap();
    let blocked = store.persistence_snapshot();
    let mut input = resume(&request, &state);
    input.source_binding_ref = SourceBindingRef::new("different-source").unwrap();
    assert!(store.resume_turn(input).await.is_err());
    assert_eq!(store.persistence_snapshot().runs, blocked.runs);
    assert!(
        store
            .claim_next_run(next_claim(&request))
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn native_auth_wait_cancellation_requires_original_runner_acknowledgement() {
    let (coordinator, store, claimed, request) = setup("native-wait-cancel").await;
    let state = store.block_native_auth(request.clone()).await.unwrap();
    let cancelled = coordinator
        .cancel_run(cancel_request(
            "native-wait-cancel",
            request.run_id,
            "cancel-native",
        ))
        .await
        .unwrap();
    assert_eq!(cancelled.status, TurnStatus::CancelRequested);
    assert!(store.resume_turn(resume(&request, &state)).await.is_err());
    assert!(
        store
            .heartbeat(HeartbeatRequest {
                run_id: request.run_id,
                runner_id: claimed.runner_id,
                lease_token: claimed.lease_token,
            })
            .await
            .is_err(),
        "cancellation must not renew its acknowledgement deadline"
    );
    let terminal = store
        .cancel_run(CancelRunCompletionRequest {
            run_id: request.run_id,
            runner_id: claimed.runner_id,
            lease_token: claimed.lease_token,
        })
        .await
        .unwrap();
    assert_eq!(terminal.status, TurnStatus::Cancelled);
    assert!(
        store
            .claim_next_run(next_claim(&request))
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn native_auth_wait_expiry_and_relinquish_never_queue_a_replacement_attempt() {
    let (_, store, claimed, request) = setup("native-wait-expiry").await;
    store.block_native_auth(request.clone()).await.unwrap();
    assert!(
        store
            .relinquish_run(brassclaw_turns::runner::RelinquishRunRequest {
                run_id: request.run_id,
                runner_id: claimed.runner_id,
                lease_token: claimed.lease_token
            })
            .await
            .is_err()
    );
    let recovered = store
        .recover_expired_leases(RecoverExpiredLeasesRequest {
            now: Utc::now() + ChronoDuration::days(1),
            scope_filter: Some(request.scope.clone()),
        })
        .await
        .unwrap();
    assert_eq!(recovered.recovered.len(), 1);
    assert_eq!(recovered.recovered[0].status, TurnStatus::Failed);
    assert!(
        store
            .claim_next_run(next_claim(&request))
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(store.persistence_snapshot().runs[0].claim_count, 1);
}

#[tokio::test]
async fn native_auth_wait_snapshot_rejects_erased_claim_or_queued_custody() {
    let (_, store, _, request) = setup("native-wait-snapshot").await;
    store.block_native_auth(request).await.unwrap();
    let original = store.persistence_snapshot();
    let mut missing = original.clone();
    missing.runs[0].runner_id = None;
    assert!(
        InMemoryTurnStateStore::from_persistence_snapshot(missing, Default::default()).is_err()
    );
    let mut queued = original;
    queued.runs[0].status = TurnStatus::Queued;
    assert!(InMemoryTurnStateStore::from_persistence_snapshot(queued, Default::default()).is_err());
}

#[tokio::test]
async fn native_auth_resume_original_survives_restore_and_conflicting_retry_is_rejected() {
    let (_, store, _, request) = setup("native-wait-resume-original").await;
    let state = store.block_native_auth(request.clone()).await.unwrap();
    let input = resume(&request, &state);
    let response = store.resume_turn(input.clone()).await.unwrap();
    let snapshot = store.persistence_snapshot();
    let reopened = InMemoryTurnStateStore::from_persistence_snapshot(
        serde_json::from_slice(&serde_json::to_vec(&snapshot).unwrap()).unwrap(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(reopened.resume_turn(input.clone()).await.unwrap(), response);
    let mut changed = input;
    changed.reply_target_binding_ref = ReplyTargetBindingRef::new("different-reply").unwrap();
    assert!(matches!(
        reopened.resume_turn(changed).await,
        Err(TurnError::Conflict { .. })
    ));
    assert_eq!(reopened.persistence_snapshot().runs, snapshot.runs);
    assert!(
        reopened
            .claim_next_run(next_claim(&request))
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn native_auth_resume_requires_auth_precondition_and_preserves_the_original_wait() {
    let (_, store, _, request) = setup("native-wait-precondition").await;
    let state = store.block_native_auth(request.clone()).await.unwrap();
    let before = store.persistence_snapshot();
    let mut input = resume(&request, &state);
    input.precondition = ResumeTurnPrecondition::AnyBlockedGate;
    assert!(store.resume_turn(input).await.is_err());
    assert_eq!(store.persistence_snapshot().runs, before.runs);
    assert!(
        store
            .claim_next_run(next_claim(&request))
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn native_auth_resolved_custody_cannot_become_a_legacy_checkpoint() {
    let (_, store, claimed, request) = setup("native-wait-no-checkpoint").await;
    let state = store.block_native_auth(request.clone()).await.unwrap();
    store.resume_turn(resume(&request, &state)).await.unwrap();
    let before = store.persistence_snapshot();
    assert!(
        store
            .block_run(BlockRunRequest {
                run_id: request.run_id,
                runner_id: claimed.runner_id,
                lease_token: claimed.lease_token,
                checkpoint_id: TurnCheckpointId::new(),
                state_ref: block_state_ref(),
                reason: brassclaw_turns::BlockedReason::Auth {
                    gate_ref: GateRef::new("gate:auth:legacy").unwrap(),
                    credential_requirements: Vec::new(),
                },
            })
            .await
            .is_err()
    );
    assert_eq!(store.persistence_snapshot(), before);
}

#[tokio::test]
async fn native_auth_resolved_cancellation_preserves_the_bounded_ack_deadline() {
    let (coordinator, store, claimed, request) = setup("native-wait-cancel-resolved").await;
    let state = store.block_native_auth(request.clone()).await.unwrap();
    store.resume_turn(resume(&request, &state)).await.unwrap();
    let cancelled = coordinator
        .cancel_run(cancel_request(
            "native-wait-cancel-resolved",
            request.run_id,
            "native-resolved-cancel",
        ))
        .await
        .unwrap();
    assert_eq!(cancelled.status, TurnStatus::CancelRequested);
    let before = store.persistence_snapshot();
    assert!(
        store
            .heartbeat(HeartbeatRequest {
                run_id: request.run_id,
                runner_id: claimed.runner_id,
                lease_token: claimed.lease_token,
            })
            .await
            .is_err()
    );
    assert_eq!(store.persistence_snapshot(), before);
    let terminal = store
        .cancel_run(CancelRunCompletionRequest {
            run_id: request.run_id,
            runner_id: claimed.runner_id,
            lease_token: claimed.lease_token,
        })
        .await
        .unwrap();
    assert_eq!(terminal.status, TurnStatus::Cancelled);
}

#[tokio::test]
async fn native_auth_resume_publishes_lifecycle_without_a_worker_wake() {
    let (_, store, claimed, request) = setup("native-wait-lifecycle").await;
    let sink = Arc::new(InMemoryTurnEventSink::default());
    let port = lifecycle_publishing_store(store.clone(), None, Some(sink.clone()));
    let wake = Arc::new(RecordingWakeNotifier::default());
    let coordinator = DefaultTurnCoordinator::new(port.clone()).with_wake_notifier(wake.clone());
    let state = port.block_native_auth(request.clone()).await.unwrap();
    assert_eq!(
        port.block_native_auth(request.clone()).await.unwrap(),
        state
    );
    let blocked_events = sink.events();
    assert_eq!(blocked_events.len(), 1);
    assert_eq!(blocked_events[0].kind, TurnEventKind::Blocked);
    assert_eq!(blocked_events[0].cursor, state.event_cursor);
    assert_eq!(
        port.heartbeat(HeartbeatRequest {
            run_id: request.run_id,
            runner_id: claimed.runner_id,
            lease_token: claimed.lease_token,
        })
        .await
        .unwrap(),
        state.event_cursor
    );
    assert_eq!(sink.events(), blocked_events);
    let input = resume(&request, &state);
    let resolved = coordinator.resume_turn(input.clone()).await.unwrap();
    assert_eq!(resolved.status, TurnStatus::Running);
    assert_eq!(coordinator.resume_turn(input).await.unwrap(), resolved);
    assert!(wake.wakes().is_empty());
    assert_eq!(
        sink.events()
            .iter()
            .filter(|event| event.kind == TurnEventKind::Resumed)
            .count(),
        1
    );
}

#[tokio::test]
async fn native_auth_resume_rejects_corrupt_persisted_originals() {
    let (_, store, _, request) = setup("native-wait-original-validation").await;
    let state = store.block_native_auth(request.clone()).await.unwrap();
    store.resume_turn(resume(&request, &state)).await.unwrap();
    let original = store.persistence_snapshot();
    for mutation in 0..3 {
        let mut snapshot = original.clone();
        let record = snapshot
            .idempotency_records
            .iter_mut()
            .find(|record| {
                matches!(
                    record.replay,
                    TurnIdempotencyReplay::NativeAuthResumeSucceeded { .. }
                )
            })
            .unwrap();
        match mutation {
            0 => record.key = IdempotencyKey::new("another-original-key").unwrap(),
            1 => record.run_id = Some(TurnRunId::new()),
            _ => {
                let TurnIdempotencyReplay::NativeAuthResumeSucceeded { request, .. } =
                    &mut record.replay
                else {
                    unreachable!()
                };
                request.precondition = ResumeTurnPrecondition::AnyBlockedGate;
            }
        }
        assert!(
            InMemoryTurnStateStore::from_persistence_snapshot(snapshot, Default::default())
                .is_err()
        );
    }
}

#[tokio::test]
async fn native_auth_cancel_ack_expiry_fails_and_fences_without_fabricating_ack() {
    for resolved in [false, true] {
        let thread = if resolved {
            "native-ack-expired-resolved"
        } else {
            "native-ack-expired-waiting"
        };
        let (coordinator, store, claimed, request) = setup(thread).await;
        let state = store.block_native_auth(request.clone()).await.unwrap();
        if resolved {
            store.resume_turn(resume(&request, &state)).await.unwrap();
        }
        coordinator
            .cancel_run(cancel_request(
                thread,
                request.run_id,
                "native-ack-expired-cancel",
            ))
            .await
            .unwrap();
        let recovered = store
            .recover_expired_leases(RecoverExpiredLeasesRequest {
                now: Utc::now() + ChronoDuration::days(1),
                scope_filter: Some(request.scope.clone()),
            })
            .await
            .unwrap();
        assert_eq!(recovered.recovered.len(), 1);
        let outcome = &recovered.recovered[0];
        assert_eq!(outcome.status, TurnStatus::Failed);
        assert_eq!(
            outcome.failure.as_ref().unwrap().category(),
            "native_cancel_ack_expired"
        );
        assert!(
            store
                .cancel_run(CancelRunCompletionRequest {
                    run_id: request.run_id,
                    runner_id: claimed.runner_id,
                    lease_token: claimed.lease_token,
                })
                .await
                .is_err()
        );
        assert!(
            store
                .claim_next_run(next_claim(&request))
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(store.persistence_snapshot().runs[0].claim_count, 1);
    }
}

#[tokio::test]
async fn native_auth_runner_failure_is_not_vm_cancellation_acknowledgement() {
    let (coordinator, store, claimed, request) = setup("native-ack-runner-failure").await;
    store.block_native_auth(request.clone()).await.unwrap();
    coordinator
        .cancel_run(cancel_request(
            "native-ack-runner-failure",
            request.run_id,
            "native-ack-runner-cancel",
        ))
        .await
        .unwrap();
    let failed = store
        .record_runner_failure(RecordRunnerFailureRequest {
            run_id: request.run_id,
            runner_id: claimed.runner_id,
            lease_token: claimed.lease_token,
            failure: SanitizedFailure::new("native_supervisor_failed").unwrap(),
        })
        .await
        .unwrap();
    assert_eq!(failed.status, TurnStatus::Failed);
    assert_eq!(
        failed.failure.as_ref().unwrap().category(),
        "native_supervisor_failed"
    );
    assert!(
        store
            .claim_next_run(next_claim(&request))
            .await
            .unwrap()
            .is_none()
    );
}
