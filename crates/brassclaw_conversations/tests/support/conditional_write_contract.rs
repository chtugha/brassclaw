use crate::conditional_backend::{ConditionalBackend, scoped};
use brassclaw_conversations::{
    AdapterInstallationId, AdapterKind, ExternalActorRef, RebornFilesystemConversationServices,
};
use brassclaw_filesystem::{CasExpectation, RootFilesystem};
use brassclaw_host_api::{TenantId, UserId};
use std::sync::Arc;

pub(crate) async fn verify<F: RootFilesystem + 'static>(inner: Arc<F>) {
    let backend = Arc::new(ConditionalBackend::new(inner));
    let filesystem = scoped(backend.clone(), "/conversations");
    let tenant = TenantId::new("conditional-conversation").unwrap();
    let adapter = AdapterKind::new("telegram").unwrap();
    let installation = AdapterInstallationId::new("conditional").unwrap();
    let actor = ExternalActorRef::new("user", "original").unwrap();
    let user = UserId::new("original-user").unwrap();
    backend.support(false, false);
    let store = RebornFilesystemConversationServices::new(filesystem.clone())
        .await
        .unwrap();
    assert!(
        store
            .pair_external_actor(
                tenant.clone(),
                adapter.clone(),
                installation.clone(),
                actor.clone(),
                user.clone()
            )
            .await
            .is_err()
    );
    assert!(!backend.calls().is_empty());
    assert!(
        backend
            .calls()
            .iter()
            .all(|cas| matches!(cas, CasExpectation::Absent))
    );

    backend.support(true, false);
    let store = RebornFilesystemConversationServices::new(filesystem.clone())
        .await
        .unwrap();
    store
        .pair_external_actor(
            tenant.clone(),
            adapter.clone(),
            installation.clone(),
            actor.clone(),
            user.clone(),
        )
        .await
        .unwrap();
    let path = brassclaw_host_api::VirtualPath::new(
        "/engine/conditional-owner-contract/conversations/state.json",
    )
    .unwrap();
    let before = backend.inner.get(&path).await.unwrap().unwrap();
    assert!(
        store
            .pair_external_actor(
                tenant.clone(),
                adapter.clone(),
                installation.clone(),
                ExternalActorRef::new("user", "next").unwrap(),
                user.clone()
            )
            .await
            .is_err()
    );
    assert_eq!(backend.inner.get(&path).await.unwrap().unwrap(), before);
    assert!(
        backend
            .calls()
            .iter()
            .all(|cas| !matches!(cas, CasExpectation::Any))
    );

    backend.support(true, true);
    let reopened = RebornFilesystemConversationServices::new(filesystem)
        .await
        .unwrap();
    reopened
        .pair_external_actor(
            tenant,
            adapter,
            installation,
            ExternalActorRef::new("user", "next").unwrap(),
            user,
        )
        .await
        .unwrap();
    assert_ne!(backend.inner.get(&path).await.unwrap().unwrap(), before);
    assert!(
        backend
            .calls()
            .iter()
            .all(|cas| matches!(cas, CasExpectation::Version(_)))
    );
}
