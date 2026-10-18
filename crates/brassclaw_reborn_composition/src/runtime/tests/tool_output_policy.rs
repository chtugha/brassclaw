//! Actual operator/router/PG/global-Monty acceptance, including post-effect
//! result rejection. No direct handler calls or manufactured component approval.
use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use brassclaw_pg::PgPool;
use brassclaw_product_workflow::instance_tools::{
    InstanceToolSetting, ToolResourceLimits, UpdateInstanceToolSetting,
};
use brassclaw_threads::{MessageKind, ThreadHistoryRequest};
use brassclaw_turns::{TurnRunId, TurnStatus};
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

use crate::runtime::RebornRuntime;

async fn policy(app: &Router, tool: Uuid) -> InstanceToolSetting {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/settings/tool-policies")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    serde_json::from_slice::<Vec<InstanceToolSetting>>(&bytes)
        .unwrap()
        .into_iter()
        .find(|value| value.tool_id == tool)
        .unwrap()
}

async fn records(pool: &PgPool, run: Uuid) -> Value {
    pool.get().await.unwrap().query_one("SELECT COALESCE(jsonb_agg(to_jsonb(i) ORDER BY invocation_id),'[]'::jsonb) FROM brassclaw_monty_tool_invocations i WHERE run_id=$1", &[&run]).await.unwrap().get(0)
}

pub(super) async fn assert_live_output_policy(
    runtime: Arc<RebornRuntime>,
    app: Router,
    pool: Arc<PgPool>,
    tool: Uuid,
) {
    let identity = runtime.global_monty_owner.client().root_identity();
    let mut current = policy(&app, tool).await;
    // Old clients cannot erase a limit by sending only the old toggle shape.
    let invalid=app.clone().oneshot(Request::builder().method("PUT").uri(format!("/api/settings/tool-policies/{tool}"))
        .header("content-type","application/json")
        .body(Body::from(serde_json::json!({"expected_revision":current.revision.to_string(),"enabled":false}).to_string())).unwrap()).await.unwrap();
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(policy(&app, tool).await, current);
    for maximum in [Some(8), Some(8192), None] {
        let request = UpdateInstanceToolSetting {
            expected_revision: current.revision,
            enabled: true,
            limits: ToolResourceLimits {
                max_output_bytes: maximum,
            },
        };
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/api/settings/tool-policies/{tool}"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let updated: InstanceToolSetting = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(updated.revision, current.revision + 1);
        assert_eq!(updated.limits, request.limits);
        assert_eq!(policy(&app, tool).await, updated);
        current = updated;

        let correlation = Uuid::new_v4();
        let conversation = runtime
            .ensure_correlated_conversation(correlation)
            .await
            .unwrap();
        let answer = format!(
            "output limit {}",
            maximum
                .map(|value| value.to_string())
                .unwrap_or_else(|| "cleared".into())
        );
        let command = format!("publish literal reply {answer}");
        let result = runtime
            .send_correlated_user_message(
                &conversation,
                &command,
                correlation,
                tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(result.run_id, TurnRunId::from_uuid(correlation));
        assert_eq!(
            result.status,
            if maximum == Some(8) {
                TurnStatus::Failed
            } else {
                TurnStatus::Completed
            }
        );
        // The advisory estimate is unknown, not fabricated. Actual result
        // bytes fail after the native reply is already durable. Preserve it.
        let history = runtime
            .thread_service
            .list_thread_history(ThreadHistoryRequest {
                scope: runtime.thread_scope.clone(),
                thread_id: conversation.0.clone(),
            })
            .await
            .unwrap();
        assert_eq!(
            history
                .messages
                .iter()
                .filter(|message| message.kind == MessageKind::Assistant
                    && message.content.as_deref() == Some(answer.as_str()))
                .count(),
            1
        );
        let before = records(&pool, correlation).await;
        assert!(!before.as_array().unwrap().is_empty());
        assert_eq!(
            runtime
                .send_correlated_user_message(
                    &conversation,
                    &command,
                    correlation,
                    tokio_util::sync::CancellationToken::new()
                )
                .await
                .unwrap(),
            result
        );
        runtime
            .close_correlated_conversation(&conversation, correlation)
            .await
            .unwrap();
        assert_eq!(
            runtime
                .recover_correlated_user_message(&conversation, correlation)
                .await
                .unwrap(),
            Some(result)
        );
        assert_eq!(
            records(&pool, correlation).await,
            before,
            "result rejection/recovery must not replay a completed reply or history write"
        );
        assert_eq!(
            runtime.global_monty_owner.client().root_identity(),
            identity
        );
    }
    assert_eq!(current.limits, ToolResourceLimits::default());
}
