//! Exercise complete eligible history through the durable transcript caller.

use std::sync::Arc;

use brassclaw_host_api::{AgentId, TenantId};
use brassclaw_threads::{
    AcceptInboundMessageRequest, EnsureThreadRequest, LoadContextMessagesRequest, MessageContent,
    PgSessionThreadService, RedactMessageRequest, SessionThreadError, SessionThreadService,
    SubmittedUserMessageRequest, ThreadScope,
};

#[tokio::test]
async fn native_submitted_history_is_complete_filtered_and_scope_checked() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let tenant = "history-contract-test";
    let service = PgSessionThreadService::new(Arc::clone(&rig.pool), tenant);
    let scope = ThreadScope {
        tenant_id: TenantId::new(tenant).unwrap(),
        agent_id: AgentId::new("history-agent").unwrap(),
        project_id: None,
        owner_user_id: None,
    };
    let thread = service
        .ensure_thread(EnsureThreadRequest {
            scope: scope.clone(),
            thread_id: None,
            created_by_actor_id: "operator".into(),
            title: None,
            metadata_json: None,
        })
        .await
        .unwrap();
    let mut ids = Vec::new();
    for index in 0..142 {
        let accepted = service
            .accept_inbound_message(AcceptInboundMessageRequest {
                scope: scope.clone(),
                thread_id: thread.thread_id.clone(),
                actor_id: "operator".into(),
                source_binding_id: None,
                reply_target_binding_id: None,
                external_event_id: None,
                content: MessageContent::text(format!("message-{index}")),
            })
            .await
            .unwrap();
        ids.push(accepted.message_id);
    }
    // Index 140 is the exact input; 141 is a newer message and must not leak
    // into its context. The redacted predecessor must not consume the limit.
    service
        .mark_message_submitted(
            &scope,
            &thread.thread_id,
            ids[140],
            "turn-history".into(),
            "run-history".into(),
        )
        .await
        .unwrap();
    service
        .redact_message(RedactMessageRequest {
            scope: scope.clone(),
            thread_id: thread.thread_id.clone(),
            message_id: ids[139],
            redaction_ref: "operator-redaction".into(),
        })
        .await
        .unwrap();
    let request = SubmittedUserMessageRequest {
        scope: scope.clone(),
        thread_id: thread.thread_id.clone(),
        message_id: ids[140],
        turn_id: "turn-history".into(),
        turn_run_id: "run-history".into(),
    };
    let complete = service
        .submitted_turn_input(request.clone(), None)
        .await
        .unwrap();
    assert_eq!(complete.message.content.as_deref(), Some("message-140"));
    assert_eq!(complete.prior_context.messages.len(), 139);
    for (index, message) in complete.prior_context.messages.iter().enumerate() {
        assert_eq!(message.message_id, Some(ids[index]));
        assert_eq!(message.content, format!("message-{index}"));
    }
    let bounded = service
        .submitted_turn_input(request, Some(2))
        .await
        .unwrap();
    assert_eq!(bounded.prior_context.messages.len(), 2);
    assert_eq!(bounded.prior_context.messages[0].message_id, Some(ids[137]));
    assert_eq!(bounded.prior_context.messages[1].message_id, Some(ids[138]));

    let selection = LoadContextMessagesRequest {
        scope: scope.clone(),
        thread_id: thread.thread_id.clone(),
        message_ids: vec![ids[138], ids[0], ids[139]],
    };
    let selected = service
        .load_context_messages(selection.clone())
        .await
        .unwrap();
    assert_eq!(selected.messages.len(), 2);
    assert_eq!(selected.messages[0].message_id, Some(ids[138]));
    assert_eq!(selected.messages[1].message_id, Some(ids[0]));
    let wrong_scope = ThreadScope {
        agent_id: AgentId::new("another-agent").unwrap(),
        ..scope
    };
    assert!(matches!(
        service
            .load_context_messages(LoadContextMessagesRequest {
                scope: wrong_scope,
                ..selection
            })
            .await,
        Err(SessionThreadError::UnknownThread { .. })
    ));
    drop(service);
    drop(rig);
}
