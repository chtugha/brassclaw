//! Real runtime/PG/HTTP acceptance, invoked by the held-parent integration test.
use super::{McpProviderExchange, RebornRuntime};
use axum::body::{Body, Bytes};
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

async fn set_limits(runtime: &RebornRuntime, request: u32, response: u32) {
    let store = runtime.webui_monty_settings_store();
    let current = store.get("default", "default").await.unwrap();
    let mut limits = current.execution_limits;
    limits.max_mcp_request_bytes = request;
    limits.max_mcp_response_bytes = response;
    let desired = store
        .upsert(
            "default",
            "default",
            &serde_json::from_value(json!({
                "expected_revision":current.revision,"execution_limits":limits,
            }))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        runtime.acknowledged_monty_execution_limits().await.unwrap(),
        limits
    );
    let observed = store
        .runtime_observation(&desired)
        .unwrap()
        .execution_limits
        .unwrap();
    assert_eq!(observed.effective_revision, desired.revision);
    assert_eq!(observed.limits, limits);
}

pub(crate) async fn assert_native_transport_limits(
    runtime: &Arc<RebornRuntime>,
    exchange: &Arc<McpProviderExchange>,
    endpoint: &str,
) {
    use crate::mcp_provider_gateway::native_post_for_transport_test;
    use crate::orchestrator_mcp_server::{
        OrchestratorMcpServerConfig, orchestrator_mcp_router_with_chat,
    };
    let original = runtime.acknowledged_monty_execution_limits().await.unwrap();
    let http = reqwest::Client::builder().no_proxy().build().unwrap();
    let listing = json!({"jsonrpc":"2.0","id":"large-request","method":"tools/list"});
    let mut padded = listing.to_string();
    padded.push_str(&" ".repeat(1024 * 1024));
    let post = |body: String, token: &str| {
        http.post(endpoint)
            .bearer_auth(token)
            .header("mcp-session-id", exchange.id().to_string())
            .header("mcp-protocol-version", "2025-06-18")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .body(body)
    };
    assert_eq!(
        post(padded.clone(), exchange.bearer_token())
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    // Authentication precedes JSON decoding and large body buffering.
    assert_eq!(
        post("malformed JSON".into(), "invalid")
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let larger = 3 * 1024 * 1024;
    set_limits(runtime, larger, larger).await;
    let result: Value = post(padded.clone(), exchange.bearer_token())
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(result["result"], exchange.tools_list());

    // Observe the real body extractor's first poll, after it captured the
    // acknowledged larger limit. A reduction must affect only subsequent reads.
    let router = orchestrator_mcp_router_with_chat(
        OrchestratorMcpServerConfig::default(),
        runtime.mcp_chat_bridge().unwrap(),
    );
    let (entered, reading) = tokio::sync::oneshot::channel();
    let (release, released) = tokio::sync::oneshot::channel();
    let stream = futures::stream::once(async move {
        entered.send(()).unwrap();
        released.await.unwrap();
        Ok::<_, std::io::Error>(Bytes::from(padded))
    });
    let request = Request::post("/mcp")
        .header(
            "authorization",
            format!("Bearer {}", exchange.bearer_token()),
        )
        .header("mcp-session-id", exchange.id().to_string())
        .header("mcp-protocol-version", "2025-06-18")
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .body(Body::from_stream(stream))
        .unwrap();
    let reading_request = tokio::spawn(router.oneshot(request));
    tokio::time::timeout(Duration::from_secs(5), reading)
        .await
        .unwrap()
        .unwrap();
    set_limits(runtime, 1, larger).await;
    release.send(()).unwrap();
    let accepted = reading_request.await.unwrap().unwrap();
    assert_eq!(accepted.status(), StatusCode::OK);
    assert_eq!(
        post(listing.to_string(), exchange.bearer_token())
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    set_limits(runtime, larger, larger).await;

    let text = "x".repeat(1024 * 1024 + 32);
    let packet = json!({"jsonrpc":"2.0","id":"complete-large-answer","method":"tools/call",
        "params":{"name":"publish_literal_reply","arguments":{"command":format!("publish literal reply {text}")}}});
    let complete = match native_post_for_transport_test(exchange.clone(), endpoint, packet).await {
        Ok(response) => response,
        Err(error) => {
            let pool = runtime.mcp_chat_bridge().unwrap().pool.clone();
            let client = pool.get().await.unwrap();
            let rows = client.query("SELECT c.phase,a.phase,a.outcome->>'status',a.outcome->>'reason_kind' FROM brassclaw_mcp_chat_calls c LEFT JOIN brassclaw_monty_task_admissions a ON a.run_id=c.call_id WHERE c.exchange_id=$1 AND c.request_id=$2", &[&exchange.id(),&json!("complete-large-answer")]).await.unwrap();
            let state: Vec<_> = rows
                .iter()
                .map(|row| {
                    (
                        row.get::<_, i16>(0),
                        row.get::<_, Option<String>>(1),
                        row.get::<_, Option<String>>(2),
                        row.get::<_, Option<String>>(3),
                    )
                })
                .collect();
            panic!("complete large answer failed: {error:?}; original durable state: {state:?}");
        }
    };
    assert_eq!(complete["isError"], false);
    assert_eq!(complete["content"][0]["text"].as_str(), Some(text.as_str()));

    // An incompressible ID crosses both the former 256-byte guard and the
    // database B-tree entry capacity. Preserve its exact JSON value on the wire;
    // only its canonical digest belongs in the durable uniqueness index.
    let long_id = json!(format!(
        "{}ä\"\\",
        (0..400)
            .map(|_| uuid::Uuid::new_v4().to_string())
            .collect::<String>()
    ));
    let bridge = runtime.mcp_chat_bridge().unwrap();
    // Reconstruct the predecessor index over actual completed correlation rows,
    // then apply the shipped upgrade inside one transaction. No identity,
    // phase, receipt or message reference may change during backfill.
    let mut migration_client = bridge.pool.get().await.unwrap();
    let tx = migration_client.transaction().await.unwrap();
    let before: Value = tx.query_one("SELECT jsonb_agg(to_jsonb(c)-'request_id_digest' ORDER BY call_id) FROM brassclaw_mcp_chat_calls c", &[])
        .await.unwrap().get(0);
    assert!(!before.as_array().unwrap().is_empty());
    tx.batch_execute(
        "ALTER TABLE brassclaw_mcp_chat_calls DROP COLUMN request_id_digest;
        DROP FUNCTION brassclaw_mcp_request_id_digest(jsonb);
        ALTER TABLE brassclaw_mcp_chat_calls ADD UNIQUE(parent_run_id,owner_scope,request_id)",
    )
    .await
    .unwrap();
    tx.batch_execute(include_str!(
        "../../brassclaw_pg/migrations/V164__mcp_request_identity_digest.sql"
    ))
    .await
    .unwrap();
    let after: Value = tx.query_one("SELECT jsonb_agg(to_jsonb(c)-'request_id_digest' ORDER BY call_id) FROM brassclaw_mcp_chat_calls c", &[])
        .await.unwrap().get(0);
    assert_eq!(after, before);
    tx.commit().await.unwrap();
    drop(migration_client);
    let client = bridge.pool.get().await.unwrap();
    for (initial_id, duplicate_id) in [
        (long_id.clone(), long_id.clone()),
        (json!(1), json!(1.0)),
        (json!(-0.0), json!(0)),
    ] {
        let packet = |id: Value| {
            json!({"jsonrpc":"2.0","id":id,"method":"tools/call",
            "params":{"name":"publish_literal_reply","arguments":{"command":"publish literal reply exact request identity"}}})
        };
        let answer: Value = post(
            packet(initial_id.clone()).to_string(),
            exchange.bearer_token(),
        )
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
        assert_eq!(answer["id"], initial_id);
        assert_eq!(
            answer["result"]["content"][0]["text"],
            "exact request identity"
        );
        let row = client.query_one("SELECT c.call_id,c.request_id,(SELECT count(*) FROM brassclaw_monty_tool_invocations i WHERE i.run_id=c.call_id) FROM brassclaw_mcp_chat_calls c WHERE c.parent_run_id=$1 AND c.owner_scope=$2 AND c.request_id_digest=brassclaw_mcp_request_id_digest($3) AND c.request_id=$3",
            &[&exchange.exchange.parent_run_id,&bridge.owner_scope,&initial_id]).await.unwrap();
        let call_id: uuid::Uuid = row.get(0);
        assert_eq!(row.get::<_, Value>(1), initial_id);
        let effects: i64 = row.get(2);
        assert!(effects > 0);
        let repeated: Value = post(
            packet(duplicate_id.clone()).to_string(),
            exchange.bearer_token(),
        )
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
        assert_eq!(repeated["id"], duplicate_id);
        assert_eq!(repeated["result"], answer["result"]);
        let after = client.query_one("SELECT call_id,(SELECT count(*) FROM brassclaw_monty_tool_invocations i WHERE i.run_id=c.call_id) FROM brassclaw_mcp_chat_calls c WHERE c.parent_run_id=$1 AND c.owner_scope=$2 AND c.request_id_digest=brassclaw_mcp_request_id_digest($3) AND c.request_id=$3",
            &[&exchange.exchange.parent_run_id,&bridge.owner_scope,&duplicate_id]).await.unwrap();
        assert_eq!(after.get::<_, uuid::Uuid>(0), call_id);
        assert_eq!(after.get::<_, i64>(1), effects);
        let mut conflict = packet(duplicate_id);
        conflict["params"]["arguments"]["command"] = json!("publish literal reply changed request");
        let rejected: Value = post(conflict.to_string(), exchange.bearer_token())
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        assert!(rejected.get("error").is_some());
    }
    drop(client);

    // A response ceiling can fail after the effect completed. Restoring the
    // limit reads the original cached receipt; it must never replay that effect.
    let packet = json!({"jsonrpc":"2.0","id":"bounded-response","method":"tools/call",
        "params":{"name":"publish_literal_reply","arguments":{"command":"publish literal reply response survived"}}});
    set_limits(runtime, larger, 8).await;
    assert!(
        native_post_for_transport_test(exchange.clone(), endpoint, packet.clone())
            .await
            .is_err()
    );
    let pool = runtime.mcp_chat_bridge().unwrap().pool.clone();
    let client = pool.get().await.unwrap();
    let row = client.query_one("SELECT c.call_id,c.phase,(SELECT count(*) FROM brassclaw_monty_tool_invocations i WHERE i.run_id=c.call_id) FROM brassclaw_mcp_chat_calls c WHERE c.exchange_id=$1 AND c.request_id=$2", &[&exchange.id(),&json!("bounded-response")]).await.unwrap();
    let original_run: uuid::Uuid = row.get(0);
    assert_eq!(row.get::<_, i16>(1), 3);
    let original_invocations: i64 = row.get(2);
    assert!(original_invocations > 0);
    set_limits(runtime, larger, larger).await;
    let complete = native_post_for_transport_test(exchange.clone(), endpoint, packet)
        .await
        .unwrap();
    assert_eq!(complete["content"][0]["text"], "response survived");
    let after = client.query_one("SELECT c.call_id,(SELECT count(*) FROM brassclaw_monty_tool_invocations i WHERE i.run_id=c.call_id) FROM brassclaw_mcp_chat_calls c WHERE c.exchange_id=$1 AND c.request_id=$2", &[&exchange.id(),&json!("bounded-response")]).await.unwrap();
    assert_eq!(after.get::<_, uuid::Uuid>(0), original_run);
    assert_eq!(after.get::<_, i64>(1), original_invocations);
    set_limits(
        runtime,
        original.max_mcp_request_bytes,
        original.max_mcp_response_bytes,
    )
    .await;
}
