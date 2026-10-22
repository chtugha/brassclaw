use brassclaw_host_api::{AgentId, ProjectId, ThreadId, UserId};
use brassclaw_threads::{
    AcceptInboundMessageRequest, EnsureThreadRequest, MessageContent,
    ReplayAcceptedInboundMessageRequest, SessionThreadService, ThreadMessageId, ThreadScope,
};

/// Same external event in another scope is another receipt. A lookup must
/// never disclose or select the other scope's/actor's message metadata.
pub(super) async fn exercise(
    service: &dyn SessionThreadService,
    scope: ThreadScope,
) -> (ThreadId, ThreadMessageId) {
    let thread = service
        .ensure_thread(EnsureThreadRequest {
            scope: scope.clone(),
            thread_id: None,
            created_by_actor_id: "replay-owner".into(),
            title: None,
            metadata_json: None,
        })
        .await
        .unwrap();
    let message = service
        .accept_inbound_message(AcceptInboundMessageRequest {
            scope: scope.clone(),
            thread_id: thread.thread_id.clone(),
            actor_id: "replay-owner".into(),
            source_binding_id: Some("shared ' ü binding".into()),
            reply_target_binding_id: Some("original-reply".into()),
            external_event_id: Some("shared-event".into()),
            content: MessageContent::text("original private input"),
        })
        .await
        .unwrap();
    let request = ReplayAcceptedInboundMessageRequest {
        scope: scope.clone(),
        actor_id: "replay-owner".into(),
        source_binding_id: "shared ' ü binding".into(),
        external_event_id: "shared-event".into(),
    };
    let original = service
        .replay_accepted_inbound_message(request.clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(original.thread_id, thread.thread_id);
    assert_eq!(original.message_id, message.message_id);
    let mut foreign = request.clone();
    foreign.actor_id = "another-actor".into();
    assert!(
        service
            .replay_accepted_inbound_message(foreign)
            .await
            .unwrap()
            .is_none()
    );
    let mut other_scopes = Vec::new();
    let mut agent = scope.clone();
    agent.agent_id = AgentId::new("other-replay-agent").unwrap();
    other_scopes.push(agent);
    let mut project = scope.clone();
    project.project_id = Some(ProjectId::new("other-replay-project").unwrap());
    other_scopes.push(project);
    let mut owner = scope.clone();
    owner.owner_user_id = Some(UserId::new("other-replay-owner").unwrap());
    other_scopes.push(owner);
    for other in &other_scopes {
        assert!(
            service
                .replay_accepted_inbound_message(ReplayAcceptedInboundMessageRequest {
                    scope: other.clone(),
                    ..request.clone()
                })
                .await
                .unwrap()
                .is_none()
        );
    }
    let other_scope = other_scopes.remove(0);
    let other_thread = service
        .ensure_thread(EnsureThreadRequest {
            scope: other_scope.clone(),
            thread_id: None,
            created_by_actor_id: "other-replay-actor".into(),
            title: None,
            metadata_json: None,
        })
        .await
        .unwrap();
    let other_message = service
        .accept_inbound_message(AcceptInboundMessageRequest {
            scope: other_scope.clone(),
            thread_id: other_thread.thread_id.clone(),
            actor_id: "other-replay-actor".into(),
            source_binding_id: Some(request.source_binding_id.clone()),
            reply_target_binding_id: None,
            external_event_id: Some(request.external_event_id.clone()),
            content: MessageContent::text("other private input"),
        })
        .await
        .unwrap();
    let other = service
        .replay_accepted_inbound_message(ReplayAcceptedInboundMessageRequest {
            scope: other_scope.clone(),
            actor_id: "other-replay-actor".into(),
            ..request.clone()
        })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(other.thread_id, other_thread.thread_id);
    assert_eq!(other.message_id, other_message.message_id);
    assert_eq!(other.scope, other_scope);
    assert_eq!(
        service
            .replay_accepted_inbound_message(request)
            .await
            .unwrap()
            .unwrap(),
        original
    );
    (thread.thread_id, message.message_id)
}
