//! Inbound MCP transport boundary, independent of the legacy outbound client.
//!
//! The old raw-Skill projection and compose-and-return-source dispatcher have
//! been removed. MCP may submit only a completed advertised Recipe command
//! through ordinary chat ingress. It must never own a ComponentPort, pool,
//! Monty executor or Rust Tool dispatcher.
//!
//! Discovery can be supplied from the normal-chat catalogue owner. Durable
//! chat correlation/closure and Kohai-owned provider sessions remain unwired;
//! tools/call still reports unavailable after validating a listed command.
//! Instance startup must qualify those adapters before exposing a listener.

#![forbid(unsafe_code)]

/// Shared diagnostic for the HTTP and settings surfaces. Missing infrastructure
/// is not an empty qualified catalogue or a stopped provider connection.
#[cfg(feature = "skills-db")]
pub const MCP_CHAT_UNAVAILABLE: &str = "inbound MCP requires qualified MCP-call Recipe discovery, durable ordinary-chat correlation/closure and Kohai provider-session wiring";

#[cfg(feature = "skills-db")]
mod inner {
    use axum::{
        Json, Router,
        extract::{DefaultBodyLimit, State},
        http::{HeaderMap, StatusCode},
        response::{IntoResponse, Response},
        routing::post,
    };
    use serde::Deserialize;
    use serde_json::{Value, json};
    use std::sync::Arc;

    use super::MCP_CHAT_UNAVAILABLE;
    use crate::mcp_recipe_catalogue::McpRecipeDiscovery;

    const PROTOCOL: &str = "2025-06-18";

    #[derive(Debug, Clone)]
    pub struct OrchestratorMcpServerConfig {
        pub max_request_bytes: usize,
    }

    impl Default for OrchestratorMcpServerConfig {
        fn default() -> Self {
            Self {
                max_request_bytes: 1024 * 1024,
            }
        }
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        jsonrpc: String,
        // Missing id is a notification; explicit null is an invalid request id.
        id: Option<Value>,
        method: String,
        #[serde(default)]
        params: Value,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ToolCall {
        name: String,
        arguments: Command,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Command {
        command: String,
    }

    /// Host-owned ingress supplies authentication before exposing this router.
    /// No SSE notifications are implemented: GET returns 405. Browser Origins
    /// are rejected until an explicit authenticated origin policy is wired.
    pub fn orchestrator_mcp_router(config: OrchestratorMcpServerConfig) -> Router {
        router(config, None)
    }

    /// Only a runtime-owned, qualified discovery facade may supply tools/list.
    /// This does not enable calls or provide provider-session ownership. Host
    /// authentication is still required before mounting either router.
    pub fn orchestrator_mcp_router_with_discovery(
        config: OrchestratorMcpServerConfig,
        discovery: Arc<McpRecipeDiscovery>,
    ) -> Router {
        router(config, Some(discovery))
    }

    fn router(
        config: OrchestratorMcpServerConfig,
        discovery: Option<Arc<McpRecipeDiscovery>>,
    ) -> Router {
        Router::new()
            .route("/mcp", post(handle_post).get(handle_get))
            .layer(DefaultBodyLimit::max(config.max_request_bytes))
            .with_state(discovery)
    }

    async fn handle_get() -> StatusCode {
        StatusCode::METHOD_NOT_ALLOWED
    }

    fn response(id: Value, result: Value) -> Response {
        Json(json!({"jsonrpc": "2.0", "id": id, "result": result})).into_response()
    }

    fn error(id: Value, code: i32, message: &str) -> Response {
        Json(json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}))
            .into_response()
    }

    async fn handle_post(
        State(discovery): State<Option<Arc<McpRecipeDiscovery>>>,
        headers: HeaderMap,
        Json(raw): Json<Value>,
    ) -> Response {
        if headers.contains_key("origin") {
            return StatusCode::FORBIDDEN.into_response();
        }
        if let Some(version) = headers.get("mcp-protocol-version")
            && version != PROTOCOL
        {
            return StatusCode::BAD_REQUEST.into_response();
        }
        let id = raw.get("id").cloned().unwrap_or(Value::Null);
        let Ok(req) = serde_json::from_value::<Request>(raw.clone()) else {
            return error(id, -32600, "invalid JSON-RPC request");
        };
        if req.jsonrpc != "2.0" {
            return error(id, -32600, "expected JSON-RPC 2.0");
        }
        if !raw
            .as_object()
            .is_some_and(|object| object.contains_key("id"))
        {
            // A notification must never dispatch a Tool or create a chat.
            return if req.method == "notifications/initialized" {
                StatusCode::ACCEPTED.into_response()
            } else {
                StatusCode::BAD_REQUEST.into_response()
            };
        }
        if !matches!(req.id, Some(Value::String(_)) | Some(Value::Number(_))) {
            return error(id, -32600, "request id must be a string or number");
        }
        match req.method.as_str() {
            "initialize" => {
                let Some(version) = req.params.get("protocolVersion").and_then(Value::as_str)
                else {
                    return error(id, -32602, "missing protocolVersion");
                };
                if version != PROTOCOL {
                    return error(id, -32602, "unsupported protocolVersion");
                }
                response(
                    id,
                    json!({
                        "protocolVersion": PROTOCOL,
                        "capabilities": {"tools": {"listChanged": false}},
                        "serverInfo": {"name": "brassclaw-orchestrator", "version": env!("CARGO_PKG_VERSION")}
                    }),
                )
            }
            "tools/list" => {
                // The bounded catalogue is returned as one atomic generation;
                // cursors are unsupported until a pinned pagination owner exists.
                if !req.params.is_null() && !req.params.as_object().is_some_and(|p| p.is_empty()) {
                    return error(id, -32602, "tools/list accepts no cursor or filters");
                }
                match discovery.as_ref().and_then(|owner| owner.snapshot().ok()) {
                    Some(snapshot) => response(id, snapshot.tools_list()),
                    None => error(id, -32603, MCP_CHAT_UNAVAILABLE),
                }
            }
            "tools/call" => {
                let Ok(call) = serde_json::from_value::<ToolCall>(req.params) else {
                    return error(
                        id,
                        -32602,
                        "expected name and arguments containing only command",
                    );
                };
                if call.name.trim().is_empty() || call.arguments.command.trim().is_empty() {
                    return error(id, -32602, "name and completed command must be nonempty");
                }
                if let Some(owner) = discovery {
                    let Ok(advertised) = owner.snapshot() else {
                        return error(id, -32603, MCP_CHAT_UNAVAILABLE);
                    };
                    if owner
                        .validate_advertised_command(
                            &advertised,
                            &call.name,
                            &call.arguments.command,
                        )
                        .is_err()
                    {
                        return error(id, -32602, "unknown tool or invalid completed command");
                    }
                }
                // No raw-Skill lookup, string flattening, composition or source
                // result. No call is admitted until its advertised contract can
                // be verified and submitted through the ordinary chat owner.
                error(id, -32603, MCP_CHAT_UNAVAILABLE)
            }
            _ => error(id, -32601, "method not found"),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use axum::{
            body::{Body, to_bytes},
            http::Request as HttpRequest,
        };
        use tower::ServiceExt;

        #[tokio::test]
        async fn qualified_mcp_list_distinguishes_empty_catalogue_from_missing_owner() {
            let discovery = Arc::new(McpRecipeDiscovery::new());
            discovery
                .publish_installed(None, uuid::Uuid::from_u128(17), &[])
                .unwrap();
            let router = orchestrator_mcp_router_with_discovery(
                OrchestratorMcpServerConfig::default(),
                discovery,
            );
            for (method, params, expected) in [
                ("tools/list", json!({}), json!({"tools":[]})),
                ("tools/list", json!({"cursor":"stale"}), Value::Null),
                (
                    "tools/call",
                    json!({"name":"raw_skill","arguments":{"command":"raw_skill x"}}),
                    Value::Null,
                ),
            ] {
                let response = router.clone().oneshot(HttpRequest::post("/mcp")
                    .header("content-type","application/json")
                    .body(Body::from(json!({"jsonrpc":"2.0","id":"qualified-list","method":method,"params":params}).to_string())).unwrap()).await.unwrap();
                let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
                let body: Value = serde_json::from_slice(&bytes).unwrap();
                assert_eq!(body["id"], "qualified-list");
                if expected.is_null() {
                    assert_eq!(body["error"]["code"], -32602);
                } else {
                    assert_eq!(body["result"], expected);
                }
            }
        }

        async fn request(body: Value) -> (StatusCode, Value) {
            let response = orchestrator_mcp_router(OrchestratorMcpServerConfig::default())
                .oneshot(
                    HttpRequest::post("/mcp")
                        .header("content-type", "application/json")
                        .body(Body::from(body.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
            let status = response.status();
            let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
            (
                status,
                if bytes.is_empty() {
                    Value::Null
                } else {
                    serde_json::from_slice(&bytes).unwrap()
                },
            )
        }

        #[tokio::test]
        async fn inbound_mcp_preserves_list_request_id_and_reports_missing_chat_support() {
            let (_, result) =
                request(json!({"jsonrpc":"2.0", "id":"list-17", "method":"tools/list"})).await;
            assert_eq!(result["id"], "list-17");
            assert_eq!(result["error"]["code"], -32603);
            assert!(result.get("result").is_none());
        }

        #[tokio::test]
        async fn inbound_mcp_never_returns_source_or_admits_unlisted_commands() {
            let (_, result) = request(json!({"jsonrpc":"2.0", "id":3, "method":"tools/call",
                "params":{"name":"read_file", "arguments":{"command":"read file /tmp/a"}}}))
            .await;
            assert_eq!(result["id"], 3);
            assert_eq!(result["error"]["code"], -32603);
            assert!(result.get("result").is_none());
            for args in [
                json!({"code":"host.shell('x')"}),
                json!({"slot0":"x"}),
                json!({"command":"read file x", "python":"print(1)"}),
            ] {
                let (_, result) = request(json!({"jsonrpc":"2.0", "id":4, "method":"tools/call",
                    "params":{"name":"read_file", "arguments":args}}))
                .await;
                assert_eq!(result["error"]["code"], -32602);
            }
        }

        #[tokio::test]
        async fn inbound_mcp_notifications_have_no_json_rpc_reply_or_dispatch() {
            let (status, body) =
                request(json!({"jsonrpc":"2.0", "method":"notifications/initialized"})).await;
            assert_eq!(status, StatusCode::ACCEPTED);
            assert_eq!(body, Value::Null);
            let (status, _) = request(json!({"jsonrpc":"2.0", "method":"tools/call",
                "params":{"name":"x","arguments":{"command":"x"}}}))
            .await;
            assert_eq!(status, StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn inbound_mcp_rejects_browser_origins_and_does_not_fake_sse() {
            let router = orchestrator_mcp_router(OrchestratorMcpServerConfig::default());
            let response = router
                .clone()
                .oneshot(HttpRequest::get("/mcp").body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
            let response = router
                .oneshot(
                    HttpRequest::post("/mcp")
                        .header("content-type", "application/json")
                        .header("origin", "https://attacker.example")
                        .body(Body::from(
                            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
        }
    }
}

#[cfg(feature = "skills-db")]
pub use inner::{
    OrchestratorMcpServerConfig, orchestrator_mcp_router, orchestrator_mcp_router_with_discovery,
};
