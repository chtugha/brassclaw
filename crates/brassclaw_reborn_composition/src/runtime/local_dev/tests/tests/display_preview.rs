use std::sync::Arc;

use brassclaw_host_api::{CapabilityDisplayOutputPreview, CapabilityId, InvocationId};
use brassclaw_loop_support::{
    CapabilityResultWrite, LoopCapabilityInputResolver, LoopCapabilityResultWriter,
};
use brassclaw_threads::{
    CapabilityDisplayPreviewEnvelope, EnsureThreadRequest, InMemorySessionThreadService,
    MessageKind, SessionThreadService, ThreadHistoryRequest, ThreadScope,
};

use super::{CapabilityDisplayPreviewStore, LocalDevCapabilityIo, provider_tool_call, run_context};

#[tokio::test]
async fn capability_io_writes_display_preview_to_durable_history() {
    let run_context = run_context("durable-display-preview").await;
    let thread_scope = ThreadScope {
        tenant_id: run_context.scope.tenant_id.clone(),
        agent_id: run_context.scope.agent_id.clone().expect("agent id"),
        project_id: run_context.scope.project_id.clone(),
        owner_user_id: None,
    };
    let thread_service = Arc::new(InMemorySessionThreadService::default());
    thread_service
        .ensure_thread(EnsureThreadRequest {
            scope: thread_scope.clone(),
            thread_id: Some(run_context.thread_id.clone()),
            created_by_actor_id: "actor-a".to_string(),
            title: None,
            metadata_json: None,
        })
        .await
        .expect("thread exists");
    let capability_io = LocalDevCapabilityIo::new_with_durable_previews(
        Arc::new(CapabilityDisplayPreviewStore::default()),
        thread_service.clone(),
        thread_scope.clone(),
    );
    let input_ref = capability_io
        .register_provider_tool_call_input(
            &run_context,
            &provider_tool_call(serde_json::json!({"path": "/workspace/main.rs"})),
        )
        .await
        .expect("input stages");
    let invocation_id = InvocationId::new();
    let capability_id = CapabilityId::new("builtin.write_file").expect("capability id");

    let display_preview = CapabilityDisplayOutputPreview {
        output_summary: Some("Edited 1 file: +1/-1".to_string()),
        output_preview:
            "--- a/workspace/main.rs\n+++ b/workspace/main.rs\n@@ -1,1 +1,1 @@\n-old\n+new\n"
                .to_string(),
        output_kind: "unified_diff".to_string(),
        subtitle: Some("/workspace/main.rs".to_string()),
        truncated: false,
    };

    let result_ref = capability_io
        .write_capability_result(CapabilityResultWrite {
            run_context: &run_context,
            input_ref: &input_ref,
            invocation_id,
            capability_id: &capability_id,
            output: serde_json::json!({"success": true}),
            display_preview: Some(display_preview.clone()),
        })
        .await
        .expect("result stages");

    let retry = capability_io
        .write_capability_result(CapabilityResultWrite {
            run_context: &run_context,
            input_ref: &input_ref,
            invocation_id,
            capability_id: &capability_id,
            output: serde_json::json!({"success":true}),
            display_preview: Some(display_preview.clone()),
        })
        .await
        .expect("same actual producer must reuse the original result/preview");
    assert_eq!(retry, result_ref);
    assert!(
        capability_io
            .write_capability_result(CapabilityResultWrite {
                run_context: &run_context,
                input_ref: &input_ref,
                invocation_id,
                capability_id: &capability_id,
                output: serde_json::json!({"success":false}),
                display_preview: Some(display_preview),
            })
            .await
            .is_err(),
        "different original output must conflict before another preview"
    );
    // Two raw previews can have the same visible truncated prefix. Retain only
    // their digest, but reject a changed hidden tail rather than accepting a
    // different original or copying that raw tail into the projection cache.
    let long_input = capability_io
        .register_provider_tool_call_input(
            &run_context,
            &provider_tool_call(serde_json::json!({"path":"/workspace/long.txt"})),
        )
        .await
        .unwrap();
    let long_invocation = InvocationId::new();
    let raw_marker = "RAW-PREVIEW-TAIL-PRIVATE";
    let long_display = CapabilityDisplayOutputPreview {
        output_summary: None,
        output_preview: format!(
            "{}{raw_marker}",
            "x".repeat(brassclaw_product_adapters::CAPABILITY_DISPLAY_PREVIEW_MAX_BYTES + 1)
        ),
        output_kind: "text".into(),
        subtitle: None,
        truncated: false,
    };
    let long_reference = capability_io
        .write_capability_result(CapabilityResultWrite {
            run_context: &run_context,
            input_ref: &long_input,
            invocation_id: long_invocation,
            capability_id: &capability_id,
            output: serde_json::json!({"success":true}),
            display_preview: Some(long_display.clone()),
        })
        .await
        .unwrap();
    let mut changed_tail = long_display.clone();
    changed_tail.output_preview.push('!');
    assert!(
        capability_io
            .write_capability_result(CapabilityResultWrite {
                run_context: &run_context,
                input_ref: &long_input,
                invocation_id: long_invocation,
                capability_id: &capability_id,
                output: serde_json::json!({"success":true}),
                display_preview: Some(changed_tail),
            })
            .await
            .is_err(),
        "a different hidden raw tail is not the same original"
    );
    assert_eq!(
        capability_io
            .write_capability_result(CapabilityResultWrite {
                run_context: &run_context,
                input_ref: &long_input,
                invocation_id: long_invocation,
                capability_id: &capability_id,
                output: serde_json::json!({"success":true}),
                display_preview: Some(long_display),
            })
            .await
            .unwrap(),
        long_reference
    );

    let history = thread_service
        .list_thread_history(ThreadHistoryRequest {
            scope: thread_scope,
            thread_id: run_context.thread_id.clone(),
        })
        .await
        .expect("history loads");
    assert_eq!(
        history.messages.len(),
        2,
        "each actual original has exactly one preview"
    );
    assert!(history.messages.iter().all(|message| {
        !message
            .content
            .as_deref()
            .is_some_and(|content| content.contains(raw_marker))
    }));
    let preview_message = history
        .messages
        .iter()
        .find(|message| message.kind == MessageKind::CapabilityDisplayPreview)
        .expect("durable preview message");
    let envelope: CapabilityDisplayPreviewEnvelope =
        serde_json::from_str(preview_message.content.as_deref().expect("preview content"))
            .expect("preview envelope parses");

    assert_eq!(envelope.output_kind.as_deref(), Some("unified_diff"));
    assert_eq!(
        envelope.output_summary.as_deref(),
        Some("Edited 1 file: +1/-1")
    );
    assert_eq!(envelope.subtitle.as_deref(), Some("/workspace/main.rs"));
    assert!(
        envelope
            .output_preview
            .as_deref()
            .is_some_and(|preview| preview.contains("+new"))
    );
}

/// Two real PG owners must converge on the same immutable preview without
/// rewriting its snapshot on retry, including after another owner is recreated.
#[cfg(all(feature = "postgres", feature = "skills-db"))]
#[tokio::test]
async fn native_pg_preview_retry_preserves_original_and_rejects_conflicting_returns() {
    use brassclaw_threads::{
        AppendCapabilityDisplayPreviewRequest, CapabilityDisplayPreviewEnvelopeInput,
        CapabilityDisplayPreviewStatus, PgSessionThreadService, SessionThreadError,
    };
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let context = run_context("native-preview-original").await;
    let scope = ThreadScope {
        tenant_id: context.scope.tenant_id.clone(),
        agent_id: context.scope.agent_id.clone().unwrap(),
        project_id: context.scope.project_id.clone(),
        owner_user_id: None,
    };
    let owner = PgSessionThreadService::new(rig.pool.clone(), scope.tenant_id.as_str());
    let other = PgSessionThreadService::new(rig.pool.clone(), scope.tenant_id.as_str());
    owner
        .ensure_thread(EnsureThreadRequest {
            scope: scope.clone(),
            thread_id: Some(context.thread_id.clone()),
            created_by_actor_id: "native-preview-owner".into(),
            title: None,
            metadata_json: None,
        })
        .await
        .unwrap();
    let before: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT version FROM brassclaw_session_threads WHERE id=$1 AND tenant_id=$2",
            &[&context.thread_id.as_str(), &scope.tenant_id.as_str()],
        )
        .await
        .unwrap()
        .get(0);
    let original = AppendCapabilityDisplayPreviewRequest {
        scope: scope.clone(),
        thread_id: context.thread_id.clone(),
        turn_run_id: context.run_id.to_string(),
        preview: CapabilityDisplayPreviewEnvelope::new(CapabilityDisplayPreviewEnvelopeInput {
            invocation_id: InvocationId::new(),
            capability_id: CapabilityId::new("builtin.write_file").unwrap(),
            status: CapabilityDisplayPreviewStatus::Completed,
            title: "original preview".into(),
            subtitle: None,
            input_summary: None,
            output_summary: None,
            output_preview: Some("original visible result".into()),
            output_kind: Some("text".into()),
            output_bytes: None,
            result_ref: Some("result:preview-original".into()),
            truncated: false,
            updated_at: chrono::Utc::now(),
        })
        .unwrap(),
    };
    let (left, right) = tokio::join!(
        owner.append_capability_display_preview(original.clone()),
        other.append_capability_display_preview(original.clone()),
    );
    let first = left.unwrap();
    assert_eq!(right.unwrap().message_id, first.message_id);
    assert_eq!(first.tool_result_ref, original.preview.result_ref);
    let read_snapshot = || async {
        let row = rig.pool.get().await.unwrap().query_one(
            "SELECT metadata,version FROM brassclaw_session_threads WHERE id=$1 AND tenant_id=$2",
            &[&context.thread_id.as_str(), &scope.tenant_id.as_str()],
        ).await.unwrap();
        (row.get::<_, serde_json::Value>(0), row.get::<_, i64>(1))
    };
    let snapshot = read_snapshot().await;
    assert_eq!(
        snapshot.1,
        before + 1,
        "only the original append advances the snapshot"
    );
    let restarted = PgSessionThreadService::new(rig.pool.clone(), scope.tenant_id.as_str());
    let mut changed_time = original.clone();
    changed_time.preview.updated_at += chrono::Duration::seconds(1);
    let mut changed_payload = original.clone();
    changed_payload.preview.output_preview = Some("foreign result".into());
    let mut changed_run = original.clone();
    changed_run.turn_run_id = "foreign-run".into();
    for changed in [changed_time, changed_payload, changed_run] {
        assert!(matches!(
            restarted.append_capability_display_preview(changed).await,
            Err(SessionThreadError::Backend(reason)) if reason == "capability preview original conflicts"
        ));
    }
    let retry = restarted
        .append_capability_display_preview(original)
        .await
        .unwrap();
    assert_eq!(retry.message_id, first.message_id);
    assert_eq!(
        read_snapshot().await,
        snapshot,
        "retries and rejected substitutions never rewrite the original"
    );
    let history = restarted
        .list_thread_history(ThreadHistoryRequest {
            scope,
            thread_id: context.thread_id,
        })
        .await
        .unwrap();
    assert_eq!(history.messages.len(), 1);
}
