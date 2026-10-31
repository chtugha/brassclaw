use crate::conditional_backend::{ConditionalBackend, scoped};
use brassclaw_filesystem::{CasExpectation, RootFilesystem};
use brassclaw_host_api::{TenantId, UserId};
use brassclaw_outbound::{
    CommunicationPreferenceKey, CommunicationPreferenceRecord, CommunicationPreferenceRepository,
    FilesystemOutboundStateStore,
};
use std::sync::Arc;

pub(crate) async fn verify<F: RootFilesystem + 'static>(inner: Arc<F>) {
    let backend = Arc::new(ConditionalBackend::new(inner));
    let store = FilesystemOutboundStateStore::new(scoped(backend.clone(), "/outbound"));
    let tenant = TenantId::new("conditional-outbound").unwrap();
    let user = UserId::new("conditional-user").unwrap();
    let key = CommunicationPreferenceKey::new(tenant.clone(), user.clone());
    let record = CommunicationPreferenceRecord {
        tenant_id: tenant,
        user_id: user.clone(),
        final_reply_target: None,
        progress_target: None,
        approval_prompt_target: None,
        auth_prompt_target: None,
        default_modality: None,
        updated_at: chrono::Utc::now(),
        updated_by: user,
    };
    backend.support(false, false);
    assert!(
        store
            .put_communication_preference(record.clone())
            .await
            .is_err()
    );
    assert_eq!(
        store
            .load_communication_preference(key.clone())
            .await
            .unwrap(),
        None
    );
    assert!(!backend.calls().is_empty());
    assert!(
        backend
            .calls()
            .iter()
            .all(|cas| matches!(cas, CasExpectation::Absent))
    );

    backend.support(true, false);
    store
        .put_communication_preference(record.clone())
        .await
        .unwrap();
    let mut successor = record.clone();
    successor.updated_by = UserId::new("successor-user").unwrap();
    assert!(
        store
            .put_communication_preference(successor.clone())
            .await
            .is_err()
    );
    assert_eq!(
        store
            .load_communication_preference(key.clone())
            .await
            .unwrap(),
        Some(record)
    );
    assert!(
        backend
            .calls()
            .iter()
            .all(|cas| !matches!(cas, CasExpectation::Any))
    );

    backend.support(true, true);
    store
        .put_communication_preference(successor.clone())
        .await
        .unwrap();
    assert_eq!(
        store.load_communication_preference(key).await.unwrap(),
        Some(successor)
    );
    assert!(
        backend
            .calls()
            .iter()
            .all(|cas| matches!(cas, CasExpectation::Version(_)))
    );
}
