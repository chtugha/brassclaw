use brassclaw_loop_support::HostManagedModelGateway;
use brassclaw_pg::PgPool;
use serde_json::{Value, json};
use std::sync::Arc;

#[cfg(feature = "root-llm-provider")]
pub(super) async fn gateway(pool: &PgPool) -> Arc<dyn HostManagedModelGateway> {
    // Explicit opt-in acceptance against LOCAL_TEST_ENV.md, with real model
    // discovery and the supported PostgreSQL provider catalogue.
    let base = "http://192.168.10.171:8000/v1";
    let models: Value = reqwest::Client::new()
        .get(format!("{base}/models"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let model = models["data"][0]["id"].as_str().unwrap();
    let definition: brassclaw_llm::ProviderDefinition = serde_json::from_value(json!({
        "id":"mcp_native_acceptance", "protocol":"open_ai_completions",
        "default_base_url":base,"model_env":"BRASSCLAW_MCP_ACCEPTANCE_MODEL",
        "default_model":model,"description":"Operator local vLLM acceptance",
        "context_window_tokens":models["data"][0]["max_model_len"]
    }))
    .unwrap();
    let repo = crate::pg_provider_repo::PgProviderRepo::new(pool.clone(), "default");
    repo.upsert(definition).await.unwrap();
    let saved = repo
        .load_all()
        .await
        .unwrap()
        .into_iter()
        .find(|(d, _)| d.id == "mcp_native_acceptance")
        .unwrap()
        .0;
    let config = brassclaw_llm::RegistryProviderConfig::generic(
        saved.protocol,
        saved.id,
        None,
        saved.default_base_url.unwrap(),
        saved.default_model.clone(),
    );
    let provider = brassclaw_llm::create_registry_provider(&config, 120).unwrap();
    let policy = brassclaw_reborn::model_gateway::LlmModelProfilePolicy::new().allow_model_profile(
        brassclaw_turns::run_profile::ModelProfileId::new("interactive_model").unwrap(),
        Some(saved.default_model),
    );
    Arc::new(brassclaw_reborn::model_gateway::LlmProviderModelGateway::new(provider, policy))
}

#[cfg(not(feature = "root-llm-provider"))]
pub(super) async fn gateway(_: &PgPool) -> Arc<dyn HostManagedModelGateway> {
    panic!("live provider acceptance requires root-llm-provider");
}

pub(super) async fn round_trip(runtime: &Arc<crate::runtime::RebornRuntime>, pool: &PgPool) {
    use crate::mcp_server_service::{McpListenerStatus, McpServerState, McpServerStatusResponse};
    struct TestSocket(u16, Arc<std::sync::atomic::AtomicBool>);
    impl McpListenerStatus for TestSocket {
        fn status(&self) -> McpServerStatusResponse {
            let alive = self.1.load(std::sync::atomic::Ordering::SeqCst);
            McpServerStatusResponse {
                state: if alive {
                    McpServerState::Running
                } else {
                    McpServerState::Stopped
                },
                port: alive.then_some(self.0),
                endpoint_url: alive.then(|| format!("http://127.0.0.1:{}/mcp", self.0)),
                error: None,
            }
        }
    }
    let bridge = runtime.mcp_chat_bridge().unwrap();
    let socket = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let port = socket.local_addr().unwrap().port();
    let router = crate::orchestrator_mcp_server::orchestrator_mcp_router_with_chat(
        crate::orchestrator_mcp_server::OrchestratorMcpServerConfig::default(),
        bridge.clone(),
    );
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
    let alive = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let task_alive = alive.clone();
    let listener = tokio::spawn(async move {
        axum::serve(socket, router)
            .with_graceful_shutdown(async {
                let _ = stop_rx.await;
            })
            .await
            .unwrap();
        task_alive.store(false, std::sync::atomic::Ordering::SeqCst);
    });
    runtime
        .attach_mcp_listener_status(Arc::new(TestSocket(port, alive.clone())))
        .unwrap();
    let client = pool.get().await.unwrap();
    client.execute("UPDATE brassclaw_instance_tool_settings SET enabled=true,revision=revision+1 WHERE tool_id IN(SELECT id FROM reborn_tools WHERE capability_id='host.post_reply')", &[]).await.unwrap();
    drop(client);
    let chat = runtime.new_conversation().await.unwrap();
    let reply=runtime.send_user_message(&chat,"Use the publish_literal_reply tool exactly once with command 'publish literal reply provider acceptance Ω'. After receiving its result, answer with that result text. You must call the tool; do not answer from memory.").await.unwrap();
    if !reply.is_successful_final_reply() {
        let client = pool.get().await.unwrap();
        let outcome: Value = client
            .query_one(
                "SELECT outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1",
                &[&reply.run_id.as_uuid()],
            )
            .await
            .unwrap()
            .get(0);
        let exchanges: i64 = client
            .query_one(
                "SELECT count(*) FROM brassclaw_mcp_exchanges WHERE parent_run_id=$1",
                &[&reply.run_id.as_uuid()],
            )
            .await
            .unwrap()
            .get(0);
        let calls: i64 = client
            .query_one(
                "SELECT count(*) FROM brassclaw_mcp_chat_calls WHERE parent_run_id=$1",
                &[&reply.run_id.as_uuid()],
            )
            .await
            .unwrap()
            .get(0);
        panic!("{reply:?}; exchanges={exchanges}; calls={calls}; task outcome={outcome}");
    }
    assert!(
        reply
            .text
            .as_deref()
            .unwrap()
            .contains("provider acceptance Ω")
    );
    let client = pool.get().await.unwrap();
    let exchange=client.query_one("SELECT exchange_id,active,disconnected_at FROM brassclaw_mcp_exchanges WHERE parent_run_id=$1", &[&reply.run_id.as_uuid()]).await.unwrap();
    assert!(!exchange.get::<_, bool>(1));
    assert!(
        exchange
            .get::<_, Option<chrono::DateTime<chrono::Utc>>>(2)
            .is_some()
    );
    let id: uuid::Uuid = exchange.get(0);
    let child = client
        .query_one(
            "SELECT call_id,phase FROM brassclaw_mcp_chat_calls WHERE exchange_id=$1",
            &[&id],
        )
        .await
        .unwrap();
    assert_eq!(child.get::<_, i16>(1), 3);
    let child_id: uuid::Uuid = child.get(0);
    let outcomes=client.query("SELECT outcome FROM brassclaw_monty_task_admissions WHERE run_id IN($1,$2) AND phase='settled'", &[&reply.run_id.as_uuid(),&child_id]).await.unwrap();
    assert_eq!(outcomes.len(), 2);
    let parent: Value = client
        .query_one(
            "SELECT outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1",
            &[&reply.run_id.as_uuid()],
        )
        .await
        .unwrap()
        .get(0);
    let child: Value = client
        .query_one(
            "SELECT outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1",
            &[&child_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(parent["execution"]["intent_outcome"], "no_match");
    assert_eq!(child["execution"]["intent_outcome"], "match");
    assert_eq!(
        parent["execution"]["root"]["vm_id"],
        child["execution"]["root"]["vm_id"]
    );
    drop(client);
    assert!(
        !listener.is_finished(),
        "provider disconnect must leave listener running"
    );
    let response = reqwest::Client::new()
        .get(format!("http://127.0.0.1:{port}/mcp"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    bridge.disconnect_all().await.unwrap();
    stop_tx.send(()).unwrap();
    listener.await.unwrap();
    assert!(!alive.load(std::sync::atomic::Ordering::SeqCst));
}
