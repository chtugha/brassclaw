use crate::conditional_backend::{ConditionalBackend, scoped};
use brassclaw_filesystem::{CasExpectation, RootFilesystem};
use brassclaw_host_api::{AgentId, TenantId, ThreadId, UserId};
use brassclaw_turns::{
    AcceptedMessageRef, AllowAllTurnAdmissionPolicy, FilesystemTurnStateStore, IdempotencyKey,
    InMemoryRunProfileResolver, ReplyTargetBindingRef, SourceBindingRef, SubmitTurnRequest,
    TurnActor, TurnLeaseToken, TurnRunnerId, TurnScope, TurnStateStore,
    runner::{ClaimRunRequest, TurnRunTransitionPort},
};
use std::sync::Arc;

pub(crate) async fn verify<F: RootFilesystem + 'static>(inner: Arc<F>) {
    let backend = Arc::new(ConditionalBackend::new(inner));
    let store = FilesystemTurnStateStore::new(scoped(backend.clone(), "/turns"));
    let scope = TurnScope::new(
        TenantId::new("conditional-turn").unwrap(),
        Some(AgentId::new("conditional-agent").unwrap()),
        None,
        ThreadId::new("conditional-thread").unwrap(),
    );
    let request = SubmitTurnRequest {
        scope: scope.clone(),
        actor: TurnActor::new(UserId::new("conditional-user").unwrap()),
        accepted_message_ref: AcceptedMessageRef::new("conditional-message").unwrap(),
        source_binding_ref: SourceBindingRef::new("conditional-source").unwrap(),
        reply_target_binding_ref: ReplyTargetBindingRef::new("conditional-reply").unwrap(),
        requested_run_profile: None,
        idempotency_key: IdempotencyKey::new("conditional-submit").unwrap(),
        received_at: chrono::Utc::now(),
        requested_run_id: None,
        parent_run_id: None,
        subagent_depth: 0,
        spawn_tree_root_run_id: None,
    };
    let resolver = InMemoryRunProfileResolver::default();
    let empty = store.persistence_snapshot().await.unwrap();
    backend.support(false, false);
    assert!(
        store
            .submit_turn(request.clone(), &AllowAllTurnAdmissionPolicy, &resolver)
            .await
            .is_err()
    );
    assert_eq!(store.persistence_snapshot().await.unwrap(), empty);
    assert_eq!(backend.calls(), vec![CasExpectation::Absent]);

    backend.support(true, false);
    store
        .submit_turn(request, &AllowAllTurnAdmissionPolicy, &resolver)
        .await
        .unwrap();
    let before = store.persistence_snapshot().await.unwrap();
    let claim = ClaimRunRequest {
        runner_id: TurnRunnerId::new(),
        lease_token: TurnLeaseToken::new(),
        scope_filter: Some(scope),
    };
    assert!(store.claim_next_run(claim.clone()).await.is_err());
    assert_eq!(store.persistence_snapshot().await.unwrap(), before);
    assert!(
        backend
            .calls()
            .iter()
            .all(|cas| !matches!(cas, CasExpectation::Any))
    );
    backend.support(true, true);
    assert!(store.claim_next_run(claim).await.unwrap().is_some());
    assert!(
        backend
            .calls()
            .iter()
            .all(|cas| matches!(cas, CasExpectation::Version(_)))
    );
}
