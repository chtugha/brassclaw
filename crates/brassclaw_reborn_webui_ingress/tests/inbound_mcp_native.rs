#![cfg(feature = "skills-db")]
use brassclaw_reborn_composition::{
    RebornBuildInput, RebornRuntimeInput, build_reborn_runtime, local_dev_runtime_policy,
};
use brassclaw_reborn_webui_ingress::mcp_listener_spawner::InboundMcpListener;
use serde_json::{Value, json};
use std::sync::Arc;
#[path = "../../brassclaw_reborn_composition/tests/common/native_pg.rs"]
mod native_pg;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn instance_listener_keeps_serving_across_provider_disconnect() {
    let pg = native_pg::NativePostgres::start().await;
    let client = pg.pool.get().await.unwrap();
    client.execute("UPDATE reborn_monty_vm_settings SET memory_policy=jsonb_set(memory_policy,'{mode}','\"manual\"'),max_memory_bytes=536870912,revision=revision+1 WHERE tenant_id='default' AND agent_id='default'", &[]).await.unwrap();
    drop(client);
    let home = tempfile::tempdir().unwrap();
    let runtime = Arc::new(
        build_reborn_runtime(
            RebornRuntimeInput::from_services(
                RebornBuildInput::postgres_with_reborn_home(
                    "mcp-host-acceptance",
                    (*pg.pool).clone(),
                    pg.url.clone(),
                    home.path().to_owned(),
                )
                .with_runtime_policy(local_dev_runtime_policy().unwrap()),
            )
            .with_inbound_mcp_required(true),
        )
        .await
        .unwrap(),
    );
    let bridge = runtime.mcp_chat_bridge().unwrap();
    let listener = InboundMcpListener::start(0, bridge.clone()).await.unwrap();
    let status = listener.status();
    runtime.attach_mcp_listener_status(status.clone()).unwrap();
    let chat = runtime.new_conversation().await.unwrap();
    let parent = runtime
        .send_user_message(&chat, "publish literal reply Ready.")
        .await
        .unwrap();
    assert!(parent.is_successful_final_reply());
    for literal in [
        "quoted ' Unicode ü and {{not_source}}",
        "first line\nsecond line with \\ and %",
    ] {
        let chat = runtime.new_conversation().await.unwrap();
        assert!(
            runtime
                .send_user_message(&chat, &format!("publish literal reply {literal}"))
                .await
                .unwrap()
                .is_successful_final_reply()
        );
    }
    let client = pg.pool.get().await.unwrap();
    client.execute("UPDATE brassclaw_instance_tool_settings SET enabled=false,revision=revision+1 WHERE tool_id IN(SELECT id FROM reborn_tools WHERE capability_id='host.post_reply')", &[]).await.unwrap();
    drop(client);
    let chat = runtime.new_conversation().await.unwrap();
    assert!(
        !runtime
            .send_user_message(
                &chat,
                "publish literal reply policy-blocked qualification case"
            )
            .await
            .unwrap()
            .is_successful_final_reply()
    );
    assert_eq!(
        runtime
            .mcp_recipe_discovery()
            .snapshot()
            .unwrap()
            .tools_list()["tools"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let client = pg.pool.get().await.unwrap();
    client.execute("UPDATE brassclaw_instance_tool_settings SET enabled=true,revision=revision+1 WHERE tool_id IN(SELECT id FROM reborn_tools WHERE capability_id='host.post_reply')", &[]).await.unwrap();
    drop(client);
    let exchange = bridge.connect(parent.run_id.as_uuid()).await.unwrap();
    let endpoint = status.status().endpoint_url.unwrap();
    let http = reqwest::Client::new();
    let init=http.post(&endpoint).bearer_auth(exchange.bearer_token()).header("accept","application/json, text/event-stream")
        .json(&json!({"jsonrpc":"2.0","id":"init","method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"host-acceptance","version":"1"}}})).send().await.unwrap();
    assert!(init.status().is_success());
    for body in [
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":"list","method":"tools/list"}),
        json!({"jsonrpc":"2.0","id":"call","method":"tools/call","params":{"name":"publish_literal_reply","arguments":{"command":"publish literal reply real host listener"}}}),
    ] {
        let response = http
            .post(&endpoint)
            .bearer_auth(exchange.bearer_token())
            .header("accept", "application/json, text/event-stream")
            .header("mcp-protocol-version", "2025-06-18")
            .header("mcp-session-id", exchange.id().to_string())
            .json(&body)
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());
        if body["id"] == "call" {
            let reply: Value = response.json().await.unwrap();
            assert_eq!(reply["result"]["content"][0]["text"], "real host listener");
        }
    }
    exchange.disconnect().await.unwrap();
    assert!(status.status().endpoint_url.is_some());
    assert_eq!(
        http.get(&endpoint)
            .bearer_auth(exchange.bearer_token())
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    drop(exchange);
    drop(bridge);
    listener.shutdown().await.unwrap();
    assert!(status.status().endpoint_url.is_none());
    Arc::try_unwrap(runtime)
        .ok()
        .expect("host releases runtime")
        .shutdown()
        .await
        .unwrap();
}
