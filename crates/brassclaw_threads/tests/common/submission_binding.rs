use brassclaw_host_api::{CapabilityId, ThreadId};
use brassclaw_threads::{
    AcceptInboundMessageRequest, AppendAssistantDraftRequest, AppendToolResultReferenceRequest,
    EnsureThreadRequest, MessageContent, MessageKind, MessageStatus,
    ProviderToolCallReferenceEnvelope, RedactMessageRequest, SessionThreadService,
    SubmittedUserMessageRequest, ThreadHistoryRequest, ThreadMessageId, ThreadScope,
    ToolResultSafeSummary,
};

/// The same public contract over actual supported backends. No coordinator
/// lifecycle or successful external effect is supplied by this fixture.
pub(super) async fn exercise(
    service: &dyn SessionThreadService,
    scope: ThreadScope,
) -> (ThreadId, ThreadMessageId) {
    let thread = service
        .ensure_thread(EnsureThreadRequest {
            scope: scope.clone(),
            thread_id: None,
            created_by_actor_id: "submission-contract".into(),
            title: None,
            metadata_json: None,
        })
        .await
        .unwrap();
    let accepted = service
        .accept_inbound_message(AcceptInboundMessageRequest {
            scope: scope.clone(),
            thread_id: thread.thread_id.clone(),
            actor_id: "submission-contract".into(),
            source_binding_id: None,
            reply_target_binding_id: None,
            external_event_id: None,
            content: MessageContent::text(
                "exact ü🙂 ' {{vars.data}}\r\n```python\nraise ValueError('data')\n```\n",
            ),
        })
        .await
        .unwrap();
    service
        .mark_message_deferred_busy(&scope, &thread.thread_id, accepted.message_id)
        .await
        .unwrap();
    let first = service
        .mark_message_submitted(
            &scope,
            &thread.thread_id,
            accepted.message_id,
            "original-turn".into(),
            "original-run".into(),
        )
        .await
        .unwrap();
    let repeated = service
        .mark_message_submitted(
            &scope,
            &thread.thread_id,
            accepted.message_id,
            "original-turn".into(),
            "original-run".into(),
        )
        .await
        .unwrap();
    assert_eq!(first, repeated);
    for (turn, run) in [
        ("another-turn", "original-run"),
        ("original-turn", "another-run"),
    ] {
        assert!(
            service
                .mark_message_submitted(
                    &scope,
                    &thread.thread_id,
                    accepted.message_id,
                    turn.into(),
                    run.into()
                )
                .await
                .is_err()
        );
    }
    assert!(
        service
            .mark_message_deferred_busy(&scope, &thread.thread_id, accepted.message_id)
            .await
            .is_err()
    );
    assert_eq!(
        service
            .submitted_user_message(SubmittedUserMessageRequest {
                scope: scope.clone(),
                thread_id: thread.thread_id.clone(),
                message_id: accepted.message_id,
                turn_id: "original-turn".into(),
                turn_run_id: "original-run".into(),
            })
            .await
            .unwrap(),
        first
    );
    let assistant = service
        .append_assistant_draft(AppendAssistantDraftRequest {
            scope: scope.clone(),
            thread_id: thread.thread_id.clone(),
            turn_run_id: "assistant-run".into(),
            content: MessageContent::text("must remain an assistant draft"),
        })
        .await
        .unwrap();
    assert!(
        service
            .mark_message_submitted(
                &scope,
                &thread.thread_id,
                assistant.message_id,
                "another-turn".into(),
                "another-run".into()
            )
            .await
            .is_err()
    );
    assert!(
        service
            .mark_message_deferred_busy(&scope, &thread.thread_id, assistant.message_id)
            .await
            .is_err()
    );
    service
        .redact_message(RedactMessageRequest {
            scope: scope.clone(),
            thread_id: thread.thread_id.clone(),
            message_id: accepted.message_id,
            redaction_ref: "operator-redaction".into(),
        })
        .await
        .unwrap();
    assert!(
        service
            .mark_message_submitted(
                &scope,
                &thread.thread_id,
                accepted.message_id,
                "original-turn".into(),
                "original-run".into()
            )
            .await
            .is_err()
    );
    let fresh = service
        .accept_inbound_message(AcceptInboundMessageRequest {
            scope: scope.clone(),
            thread_id: thread.thread_id.clone(),
            actor_id: "submission-contract".into(),
            source_binding_id: None,
            reply_target_binding_id: None,
            external_event_id: None,
            content: MessageContent::text("still accepted"),
        })
        .await
        .unwrap();
    for (turn, run) in [("", "run"), ("turn", "")] {
        assert!(
            service
                .mark_message_submitted(
                    &scope,
                    &thread.thread_id,
                    fresh.message_id,
                    turn.into(),
                    run.into()
                )
                .await
                .is_err()
        );
    }
    let tool = service
        .append_tool_result_reference(AppendToolResultReferenceRequest {
            scope: scope.clone(),
            thread_id: thread.thread_id.clone(),
            turn_run_id: "redacted-tool-run".into(),
            result_ref: "result:redacted-tool".into(),
            safe_summary: ToolResultSafeSummary::new("visible summary").unwrap(),
            provider_call: Some(ProviderToolCallReferenceEnvelope {
                provider_id: "provider".into(),
                provider_model_id: "model".into(),
                provider_turn_id: "provider-turn".into(),
                provider_call_id: "provider-call".into(),
                provider_tool_name: "demo__echo".into(),
                capability_id: CapabilityId::new("demo.echo").unwrap(),
                arguments: serde_json::json!({"message":"private argument"}),
                response_reasoning: Some("private response reasoning".into()),
                reasoning: Some("private call reasoning".into()),
                signature: Some("private signature".into()),
            }),
            model_observation: Some(serde_json::json!({"text":"private observation"})),
        })
        .await
        .unwrap();
    assert!(tool.content.is_some() && tool.tool_result_provider_call.is_some());
    let redacted_tool = service
        .redact_message(RedactMessageRequest {
            scope: scope.clone(),
            thread_id: thread.thread_id.clone(),
            message_id: tool.message_id,
            redaction_ref: "tool-redaction".into(),
        })
        .await
        .unwrap();
    assert_eq!(redacted_tool.status, MessageStatus::Redacted);
    assert!(redacted_tool.content.is_none());
    assert!(redacted_tool.tool_result_provider_call.is_none());
    let history = service
        .list_thread_history(ThreadHistoryRequest {
            scope,
            thread_id: thread.thread_id.clone(),
        })
        .await
        .unwrap();
    let preserved = history
        .messages
        .iter()
        .find(|message| message.message_id == assistant.message_id)
        .unwrap();
    assert_eq!(preserved.kind, MessageKind::Assistant);
    assert_eq!(preserved.status, MessageStatus::Draft);
    assert_eq!(preserved.content, assistant.content);
    let redacted = history
        .messages
        .iter()
        .find(|message| message.message_id == accepted.message_id)
        .unwrap();
    assert_eq!(redacted.status, MessageStatus::Redacted);
    assert!(redacted.content.is_none());
    let stored_tool = history
        .messages
        .iter()
        .find(|message| message.message_id == tool.message_id)
        .unwrap();
    assert_eq!(stored_tool, &redacted_tool);
    let unbound = history
        .messages
        .iter()
        .find(|message| message.message_id == fresh.message_id)
        .unwrap();
    assert_eq!(unbound.status, MessageStatus::Accepted);
    assert!(unbound.turn_id.is_none() && unbound.turn_run_id.is_none());
    (thread.thread_id, fresh.message_id)
}
