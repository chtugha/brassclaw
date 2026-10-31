use crate::conditional_backend::{ConditionalBackend, scoped};
use brassclaw_filesystem::{CasExpectation, RootFilesystem};
use brassclaw_host_api::{AgentId, TenantId, ThreadId, UserId};
use brassclaw_threads::{
    AppendAssistantDraftRequest, EnsureThreadRequest, FilesystemSessionThreadService,
    MessageContent, SessionThreadService, ThreadHistoryRequest, ThreadScope,
    UpdateAssistantDraftRequest,
};
use std::sync::Arc;

pub(crate) async fn verify<F: RootFilesystem + 'static>(inner: Arc<F>) {
    let backend = Arc::new(ConditionalBackend::new(inner));
    let store = FilesystemSessionThreadService::new(scoped(backend.clone(), "/threads"));
    let scope = ThreadScope {
        tenant_id: TenantId::new("conditional-thread").unwrap(),
        agent_id: AgentId::new("conditional-agent").unwrap(),
        project_id: None,
        owner_user_id: Some(UserId::new("conditional-user").unwrap()),
    };
    let thread_id = ThreadId::new("conditional-thread").unwrap();
    let request = EnsureThreadRequest {
        scope: scope.clone(),
        thread_id: Some(thread_id.clone()),
        created_by_actor_id: "conditional-user".into(),
        title: None,
        metadata_json: None,
    };
    backend.support(false, false);
    assert!(store.ensure_thread(request.clone()).await.is_err());
    assert_eq!(backend.calls(), vec![CasExpectation::Absent]);

    backend.support(true, true);
    store.ensure_thread(request).await.unwrap();
    let message = store
        .append_assistant_draft(AppendAssistantDraftRequest {
            scope: scope.clone(),
            thread_id: thread_id.clone(),
            turn_run_id: "conditional-run".into(),
            content: MessageContent::text("original"),
        })
        .await
        .unwrap();
    let history = ThreadHistoryRequest {
        scope: scope.clone(),
        thread_id: thread_id.clone(),
    };
    let before = store.list_thread_history(history.clone()).await.unwrap();
    let update = UpdateAssistantDraftRequest {
        scope,
        thread_id,
        message_id: message.message_id,
        content: MessageContent::text("successor"),
    };
    backend.support(true, false);
    assert!(store.update_assistant_draft(update.clone()).await.is_err());
    assert_eq!(
        store.list_thread_history(history.clone()).await.unwrap(),
        before
    );
    assert!(!backend.calls().is_empty());
    assert!(
        backend
            .calls()
            .iter()
            .all(|cas| matches!(cas, CasExpectation::Version(_)))
    );
    backend.support(true, true);
    store.update_assistant_draft(update).await.unwrap();
    assert_ne!(store.list_thread_history(history).await.unwrap(), before);
}
