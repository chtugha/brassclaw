use super::*;
use brassclaw_host_api::{AgentId, TenantId};

#[tokio::test]
async fn exhausted_memory_sequence_leaves_input_and_event_unclaimed() {
    let service = InMemorySessionThreadService::default();
    let scope = ThreadScope {
        tenant_id: TenantId::new("sequence-owner").unwrap(),
        agent_id: AgentId::new("sequence-agent").unwrap(),
        project_id: None,
        owner_user_id: None,
    };
    let thread = service
        .ensure_thread(EnsureThreadRequest {
            scope: scope.clone(),
            thread_id: None,
            created_by_actor_id: "owner".into(),
            title: None,
            metadata_json: None,
        })
        .await
        .unwrap();
    service
        .state
        .lock()
        .await
        .threads
        .get_mut(&thread.thread_id)
        .unwrap()
        .next_sequence = u64::MAX;
    let request = AcceptInboundMessageRequest {
        scope: scope.clone(),
        thread_id: thread.thread_id.clone(),
        actor_id: "owner".into(),
        source_binding_id: Some("source".into()),
        external_event_id: Some("event".into()),
        reply_target_binding_id: None,
        content: MessageContent::text("preserve input"),
    };
    assert!(
        matches!(service.accept_inbound_message(request.clone()).await,
        Err(SessionThreadError::Backend(reason)) if reason=="thread message sequence exhausted")
    );
    let state = service.state.lock().await;
    assert!(state.inbound_idempotency.is_empty());
    assert_eq!(state.threads[&thread.thread_id].next_sequence, u64::MAX);
    assert!(state.threads[&thread.thread_id].messages.is_empty());
    drop(state);
    service
        .state
        .lock()
        .await
        .threads
        .get_mut(&thread.thread_id)
        .unwrap()
        .next_sequence = 1;
    assert_eq!(
        service
            .accept_inbound_message(request)
            .await
            .unwrap()
            .sequence,
        1
    );
}
