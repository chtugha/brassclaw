//! Retained operator inputs -> ordinary durable ingress -> the one global root.
use super::*;
use brassclaw_product_workflow::{
    MeasurePrefixRequest, RetainPrefixDesignProposalRequest, WebUiAuthenticatedCaller,
};
use brassclaw_turns::{TurnRunId, TurnStatus};
use tokio_util::sync::CancellationToken;

pub(super) async fn assert_durable_measurement(
    runtime: &Arc<RebornRuntime>,
    app: &axum::Router,
    generation: &str,
    input: &Value,
    observed: &Value,
) {
    let pool = runtime.installed_catalogue.pool.clone();
    let root = runtime.global_monty_owner.client().root_identity();
    let namespace = runtime.webui_tenant_id();
    let source = crate::prefix_generations::PgPrefixGenerations::new(pool.clone())
        .get("base-prompt", generation)
        .await
        .unwrap();
    let package: Value = serde_json::from_str(&source.generation_bytes).unwrap();
    use base64::{Engine, engine::general_purpose::STANDARD};
    let design: Value = serde_json::from_slice(
        &STANDARD
            .decode(
                package["artifacts"]["profile_design.json"]["data"]
                    .as_str()
                    .unwrap(),
            )
            .unwrap(),
    )
    .unwrap();
    let specification = input["specification"].as_str().unwrap();
    let proposals =
        crate::prefix_model_designs::PgPrefixModelDesigns::new(pool.clone(), namespace.into());
    let proposal = proposals.retain_proposal("base-prompt", RetainPrefixDesignProposalRequest {
        specification: specification.into(),
        proposal_bytes: json!({"format":"brassclaw-prefix-design-proposal/1","profile":"base-prompt","specification":specification,
            "design":{"adapter":"ornith-qwen-system/1","evidence_view":design["evidence_view"],"source_selection":"complete-mandatory","reserve_tokens":8192},
            "rationale":"Offline diagnostic only; no model, design or cache acceptance."}).to_string(),
    }).await.unwrap();
    let client = pool.get().await.unwrap();
    let revision: i64 = client
        .query_one(
            "SELECT revision FROM reborn_prefix_profiles WHERE name='base-prompt'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    let approvals: i64 = client
        .query_one("SELECT count(*) FROM reborn_prefix_design_acceptances", &[])
        .await
        .unwrap()
        .get(0);
    let caller = WebUiAuthenticatedCaller::new(
        runtime.thread_scope.tenant_id.clone(),
        brassclaw_host_api::UserId::new("catalogue-operator").unwrap(),
        Some(runtime.thread_scope.agent_id.clone()),
        runtime.thread_scope.project_id.clone(),
    );
    let request = MeasurePrefixRequest {
        expected_profile_revision: revision as u64,
        generation: generation.into(),
        proposal: proposal.id.clone(),
        artifact_set: input["artifact_set"].as_str().unwrap().into(),
        probes_json: input["probes_json"].as_str().unwrap().into(),
        context_window_tokens: 131072,
        reserved_additional_input_tokens: 2048,
    };
    let store = crate::prefix_job_definitions::Store::new(pool.clone());
    // Invalid captures cannot create either an immutable definition or a run.
    for field in ["generation", "proposal", "artifact_set", "probes"] {
        let mut invalid = request.clone();
        match field {
            "generation" => invalid.generation = "f".repeat(64),
            "proposal" => invalid.proposal = "f".repeat(64),
            "artifact_set" => invalid.artifact_set = "f".repeat(64),
            _ => invalid.probes_json = r#"[{"kind":"text","text":"template bypass"},{"kind":"text","text":"template bypass"}]"#.into(),
        }
        let id = Uuid::new_v4();
        assert!(
            runtime
                .submit_prefix_measurement(id, "base-prompt", invalid, caller.clone())
                .await
                .is_err(),
            "{field}"
        );
        assert!(store.get(id).await.unwrap().is_none());
        assert!(
            client
                .query_opt(
                    "SELECT run_id FROM brassclaw_monty_task_admissions WHERE run_id=$1",
                    &[&id]
                )
                .await
                .unwrap()
                .is_none()
        );
    }
    let id = Uuid::new_v4();
    let path = format!("/api/webchat/v2/prefixes/base-prompt/jobs/{id}");
    assert_eq!(
        post(app, &path, &request, false).await.0,
        axum::http::StatusCode::UNAUTHORIZED
    );
    assert!(store.get(id).await.unwrap().is_none());
    let (status, detail) = post(app, &path, &request, true).await;
    assert_eq!(status, axum::http::StatusCode::OK, "{detail}");
    assert_eq!(detail["job_id"], id.to_string());
    let definition = store.get(id).await.unwrap().unwrap();
    assert_eq!(definition.operation, "measure");
    assert_eq!(definition.mode, "target");
    assert_eq!(definition.request["capture"]["generation"], generation);
    assert_eq!(definition.request["capture"]["proposal"], proposal.id);
    assert!(definition.request["capture"].get("probes_json").is_none());
    assert_eq!(
        runtime
            .wait_for_terminal(
                &definition.scope,
                TurnRunId::from_uuid(id),
                &CancellationToken::new()
            )
            .await
            .unwrap(),
        TurnStatus::Completed
    );
    let result = runtime.inspect_prefix_job("base-prompt", id).await.unwrap();
    assert_eq!(result.answered_tool_calls, 2);
    assert_eq!(result.pending_tool_calls, 0);
    assert_eq!(result.admission_phase.as_deref(), Some("settled"));
    let receipt: Value = serde_json::from_str(result.internal_receipt.as_deref().unwrap()).unwrap();
    assert_eq!(
        receipt["qualification"],
        "calculation-only; not approval or target verification"
    );
    assert_eq!(receipt["model_quality"], "unmeasured");
    assert_eq!(receipt["cache_qualification"], "unverified");
    assert_eq!(
        receipt["common_prefix_tokens"],
        observed["report"]["common_prefix_tokens"]
    );
    assert_eq!(
        receipt["template_sha256"],
        observed["report"]["chat_template_sha256"]
    );
    assert_eq!(receipt["specification"], specification);
    let largest = observed["report"]["probes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["token_count"].as_u64().unwrap())
        .max()
        .unwrap();
    assert_eq!(receipt["required_tokens"], largest + 8192 + 2048);
    assert_eq!(receipt["fits"], largest + 8192 + 2048 <= 131072);
    assert!(
        runtime
            .read_latest_assistant_text(
                &definition.thread_scope,
                &definition.scope.thread_id,
                TurnRunId::from_uuid(id)
            )
            .await
            .unwrap()
            .is_none()
    );
    // Retained references survive selection changes; reconnect must not repeat
    // even one of the two already completed native operations.
    assert_eq!(
        post(app, &path, &request, true).await.0,
        axum::http::StatusCode::OK
    );
    let mut conflict = request;
    conflict.reserved_additional_input_tokens += 1;
    assert_eq!(
        post(app, &path, &conflict, true).await.0,
        axum::http::StatusCode::BAD_REQUEST
    );
    assert_eq!(
        runtime
            .inspect_prefix_job("base-prompt", id)
            .await
            .unwrap()
            .answered_tool_calls,
        2
    );
    let mut corrupt = store.get(id).await.unwrap().unwrap();
    corrupt.input["request_json"] = json!("{}");
    assert!(
        crate::prefix_measurement_capture::verify(pool.clone(), &corrupt)
            .await
            .is_err()
    );
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM reborn_prefix_design_acceptances", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        approvals
    );
    assert_eq!(runtime.global_monty_owner.client().root_identity(), root);
    eprintln!(
        "durable prefix measurement: retained generation/proposal/artifacts, actual global ingress, exact capacity receipt, 2 calls; reconnect did not replay; no design acceptance or activation"
    );
}

async fn post(
    app: &axum::Router,
    path: &str,
    request: &MeasurePrefixRequest,
    authenticated: bool,
) -> (axum::http::StatusCode, Value) {
    use tower::ServiceExt;
    let mut builder = axum::http::Request::builder()
        .method("POST")
        .uri(path)
        .header("Content-Type", "application/json");
    if authenticated {
        builder = builder.header("Authorization", "Bearer catalogue-acceptance");
    }
    let response = app
        .clone()
        .oneshot(
            builder
                .body(axum::body::Body::from(
                    serde_json::to_string(request).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}
