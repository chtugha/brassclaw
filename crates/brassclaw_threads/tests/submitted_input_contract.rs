use brassclaw_host_api::{AgentId, TenantId};
use brassclaw_threads::{
    AcceptInboundMessageRequest, EnsureThreadRequest, InMemorySessionThreadService, MessageContent,
    SessionThreadError, SessionThreadService, SubmittedUserMessageRequest, ThreadMessageId,
    ThreadScope,
};

async fn fixture() -> (InMemorySessionThreadService, SubmittedUserMessageRequest) {
    let service = InMemorySessionThreadService::default();
    let scope = ThreadScope {
        tenant_id: TenantId::new("instance").unwrap(),
        agent_id: AgentId::new("agent").unwrap(),
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
    let mut first = None;
    for (text, turn, run) in [
        ("first input", "turn-a", "run-a"),
        ("later input", "turn-b", "run-b"),
    ] {
        let accepted = service
            .accept_inbound_message(AcceptInboundMessageRequest {
                scope: scope.clone(),
                thread_id: thread.thread_id.clone(),
                actor_id: "owner".into(),
                source_binding_id: None,
                reply_target_binding_id: None,
                external_event_id: None,
                content: MessageContent::text(text),
            })
            .await
            .unwrap();
        service
            .mark_message_submitted(
                &scope,
                &thread.thread_id,
                accepted.message_id,
                turn.into(),
                run.into(),
            )
            .await
            .unwrap();
        first.get_or_insert(accepted.message_id);
    }
    (
        service,
        SubmittedUserMessageRequest {
            scope,
            thread_id: thread.thread_id,
            message_id: first.unwrap(),
            turn_id: "turn-a".into(),
            turn_run_id: "run-a".into(),
        },
    )
}

#[tokio::test]
async fn later_submission_does_not_replace_claimed_input() {
    let (service, request) = fixture().await;
    let message = service
        .submitted_user_message(request.clone())
        .await
        .unwrap();
    assert_eq!(message.message_id, request.message_id);
    assert_eq!(message.content.as_deref(), Some("first input"));
    // A retry resolves the same durable input, without consuming it.
    assert_eq!(
        service.submitted_user_message(request).await.unwrap(),
        message
    );
}

#[tokio::test]
async fn another_runs_input_is_rejected() {
    let (service, mut request) = fixture().await;
    request.turn_run_id = "run-b".into();
    assert!(matches!(
        service.submitted_user_message(request).await,
        Err(SessionThreadError::SubmittedInputMismatch { .. })
    ));
}

#[tokio::test]
async fn another_turns_input_is_rejected() {
    let (service, mut request) = fixture().await;
    request.turn_id = "turn-b".into();
    assert!(matches!(
        service.submitted_user_message(request).await,
        Err(SessionThreadError::SubmittedInputMismatch { .. })
    ));
}

#[tokio::test]
async fn missing_message_is_an_error() {
    let (service, mut request) = fixture().await;
    request.message_id = ThreadMessageId::new();
    assert!(matches!(
        service.submitted_user_message(request).await,
        Err(SessionThreadError::UnknownMessage { .. })
    ));
}

#[tokio::test]
async fn missing_thread_propagates_storage_failure() {
    let (service, mut request) = fixture().await;
    request.thread_id = brassclaw_host_api::ThreadId::new("missing").unwrap();
    assert!(matches!(
        service.submitted_user_message(request).await,
        Err(SessionThreadError::UnknownThread { .. })
    ));
}

#[tokio::test]
async fn redacted_input_cannot_be_executed() {
    let (service, request) = fixture().await;
    service
        .redact_message(brassclaw_threads::RedactMessageRequest {
            scope: request.scope.clone(),
            thread_id: request.thread_id.clone(),
            message_id: request.message_id,
            redaction_ref: "redaction:test".into(),
        })
        .await
        .unwrap();
    assert!(matches!(
        service.submitted_user_message(request).await,
        Err(SessionThreadError::SubmittedInputMismatch { .. })
    ));
}

#[tokio::test]
async fn wrong_scope_cannot_read_admitted_input() {
    let (service, mut request) = fixture().await;
    request.scope.tenant_id = TenantId::new("other-instance").unwrap();
    assert!(matches!(
        service.submitted_user_message(request).await,
        Err(SessionThreadError::UnknownThread { .. })
    ));
}

#[tokio::test]
async fn accepted_but_unsubmitted_message_is_rejected() {
    let (service, mut request) = fixture().await;
    let accepted = service
        .accept_inbound_message(AcceptInboundMessageRequest {
            scope: request.scope.clone(),
            thread_id: request.thread_id.clone(),
            actor_id: "owner".into(),
            source_binding_id: None,
            reply_target_binding_id: None,
            external_event_id: None,
            content: MessageContent::text("not yet submitted"),
        })
        .await
        .unwrap();
    request.message_id = accepted.message_id;
    assert!(matches!(
        service.submitted_user_message(request).await,
        Err(SessionThreadError::SubmittedInputMismatch { .. })
    ));
}
