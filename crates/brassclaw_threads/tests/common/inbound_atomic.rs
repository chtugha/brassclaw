use brassclaw_threads::{
    AcceptInboundMessageRequest, EnsureThreadRequest, MessageContent, RedactMessageRequest,
    SessionThreadError, SessionThreadService, ThreadHistoryRequest, ThreadScope,
};
use std::sync::Arc;

/// Concurrent real calls against one supported backend. Both threads use the
/// same canonical event key; one message wins, with no overwrite or new input.
pub(super) async fn exercise(service: Arc<dyn SessionThreadService>, scope: ThreadScope) {
    let mut requests = Vec::new();
    for _ in 0..2 {
        let thread = service
            .ensure_thread(EnsureThreadRequest {
                scope: scope.clone(),
                thread_id: None,
                created_by_actor_id: "event-owner".into(),
                title: None,
                metadata_json: None,
            })
            .await
            .unwrap();
        requests.push(AcceptInboundMessageRequest {
            scope: scope.clone(),
            thread_id: thread.thread_id,
            actor_id: "event-owner".into(),
            source_binding_id: Some("atomic ' ü🙂 source".into()),
            reply_target_binding_id: Some("original-reply".into()),
            external_event_id: Some("one-event".into()),
            content: MessageContent::text(
                "exact original ü🙂\r\n```python\nraise ValueError('data')\n```\n",
            ),
        });
    }
    let results = futures::future::join_all(
        requests
            .iter()
            .cloned()
            .map(|request| service.accept_inbound_message(request)),
    )
    .await;
    let mut winner = None;
    for (request, result) in requests.iter().zip(results) {
        match result {
            Ok(accepted) => {
                assert!(
                    winner.is_none(),
                    "one event cannot create messages in two threads"
                );
                assert!(!accepted.idempotent_replay);
                winner = Some((request.clone(), accepted));
            }
            Err(SessionThreadError::IdempotentReplayThreadMismatch { .. }) => {}
            other => panic!("unexpected actual admission result: {other:?}"),
        }
    }
    let (request, accepted) = winner.unwrap();
    let repeated = service
        .accept_inbound_message(request.clone())
        .await
        .unwrap();
    assert!(repeated.idempotent_replay);
    assert_eq!(repeated.message_id, accepted.message_id);
    for field in ["actor", "content", "reply"] {
        let mut changed = request.clone();
        match field {
            "actor" => changed.actor_id = "different-event-owner".into(),
            "content" => changed.content = MessageContent::text("changed content must fail"),
            "reply" => changed.reply_target_binding_id = Some("changed-reply".into()),
            _ => unreachable!(),
        }
        assert!(service.accept_inbound_message(changed).await.is_err());
    }
    let mut count = 0;
    for original in &requests {
        let history = service
            .list_thread_history(ThreadHistoryRequest {
                scope: scope.clone(),
                thread_id: original.thread_id.clone(),
            })
            .await
            .unwrap();
        count += history.messages.len();
        if original.thread_id == accepted.thread_id {
            assert_eq!(history.messages.len(), 1);
            assert_eq!(history.messages[0].message_id, accepted.message_id);
            assert_eq!(
                history.messages[0].content.as_deref(),
                Some(request.content.as_text())
            );
        }
    }
    assert_eq!(count, 1);
    service
        .redact_message(RedactMessageRequest {
            scope: scope.clone(),
            thread_id: accepted.thread_id.clone(),
            message_id: accepted.message_id,
            redaction_ref: "redacted-event".into(),
        })
        .await
        .unwrap();
    assert!(
        service
            .accept_inbound_message(request.clone())
            .await
            .is_err()
    );
    service
        .delete_thread(&scope, &accepted.thread_id)
        .await
        .unwrap();
    let mut after_delete = requests
        .into_iter()
        .find(|candidate| candidate.thread_id != accepted.thread_id)
        .unwrap();
    after_delete.content =
        MessageContent::text("new event after explicitly deleting original thread");
    let replacement = service.accept_inbound_message(after_delete).await.unwrap();
    assert!(!replacement.idempotent_replay);
    assert_ne!(replacement.message_id, accepted.message_id);
}
