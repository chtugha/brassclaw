//! Generic webhook ingress for tools.
//!
//! Exposes `/webhook/tools/{tool}` so external webhook providers can POST
//! payloads that are normalized by the target tool into `system_event`s.

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, Method, StatusCode},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::agent::routine_engine::RoutineEngine;
use crate::secrets::SecretsStore;

// ============================================================================
// V1 STUBS - TODO: Remove after V2 migration complete
// ============================================================================

/// Stub for deleted V1 ToolRegistry type
pub struct ToolRegistry {
    // Minimal stub - no fields needed
}

impl ToolRegistry {
    // V1 - async trait not dyn-compatible - commented out
    // /// Stub method to get a tool by name
    // pub fn get(&self, _name: &str) -> Option<Arc<dyn Tool>> {
    //     None
    // }
}

// V1 - async trait not dyn-compatible - commented out entire Tool trait
// pub trait Tool: Send + Sync {
//     fn webhook_capability(&self) -> Option<WebhookCapability> {
//         None
//     }
//
//     // V1 - stub for execute method
//     async fn execute(
//         &self,
//         _params: serde_json::Value,
//         _ctx: &JobContext,
//     ) -> Result<ToolOutput, String> {
//         Err("V1 Tool execute not implemented".to_string())
//     }
// }

/// Stub for deleted V1 ToolOutput
pub struct ToolOutput {
    pub result: serde_json::Value,
}

impl ToolOutput {
    pub fn success(result: serde_json::Value, _duration: std::time::Duration) -> Self {
        Self { result }
    }
}

/// Stub for deleted V1 WebhookCapability
#[derive(Clone, Default)]
pub struct WebhookCapability {
    pub auth_method: String,
    pub hmac_signature_header: Option<String>,
    pub hmac_prefix: Option<String>,
    pub secret_name: Option<String>,
    pub secret_header: Option<String>,
    pub signature_key_secret_name: Option<String>,
    pub hmac_secret_name: Option<String>,
    pub hmac_timestamp_header: Option<String>,
}

/// Stub module for deleted V1 channels::wasm::signature
mod wasm_signature_stubs {
    pub fn verify_discord_signature(
        _key: &str,
        _sig: &str,
        _ts: &str,
        _body: &[u8],
        _now_secs: i64,
    ) -> bool {
        false
    }

    pub fn verify_slack_signature(
        _secret: &str,
        _ts: &str,
        _body: &[u8],
        _sig: &str,
        _now_secs: i64,
    ) -> bool {
        false
    }

    pub fn verify_hmac_sha256_prefixed(
        _secret: &str,
        _body: &[u8],
        _sig: &str,
        _prefix: &str,
    ) -> bool {
        false
    }
}

// ============================================================================
// END V1 STUBS
// ============================================================================

/// Shared routine engine slot, populated by Agent after startup.
pub type RoutineEngineSlot = Arc<tokio::sync::RwLock<Option<Arc<RoutineEngine>>>>;

/// Shared state for the generic tools webhook ingress.
#[derive(Clone)]
pub struct ToolWebhookState {
    pub tools: Arc<ToolRegistry>,
    pub routine_engine: RoutineEngineSlot,
    pub user_id: String,
    pub secrets_store: Option<Arc<dyn SecretsStore + Send + Sync>>,
}

#[derive(Debug, Serialize)]
struct ToolWebhookResponse {
    status: &'static str,
    tool: String,
    emitted_events: usize,
    fired_routines: usize,
}

#[derive(Debug, Deserialize)]
struct ToolWebhookOutput {
    #[serde(default)]
    emit_events: Vec<SystemEventIntent>,
}

#[derive(Debug, Deserialize)]
struct SystemEventIntent {
    source: String,
    event_type: String,
    #[serde(default)]
    payload: serde_json::Value,
}

const MAX_WEBHOOK_BODY_BYTES: usize = 64 * 1024;

/// Build routes for tool-driven webhook ingestion.
pub fn routes(state: ToolWebhookState) -> Router {
    Router::new()
        .route("/webhook/tools/{tool}", post(tool_webhook_handler))
        .route(
            "/webhook/tools/{tool}/{*rest}",
            post(tool_webhook_with_rest_handler),
        )
        .route("/webhook/tools/{tool}", get(tool_webhook_health))
        .layer(DefaultBodyLimit::max(MAX_WEBHOOK_BODY_BYTES))
        .with_state(state)
}

// V1 - async trait not dyn-compatible - commented out
async fn tool_webhook_health(
    Path(tool): Path<String>,
    State(_state): State<ToolWebhookState>,
) -> (StatusCode, Json<serde_json::Value>) {
    // let Some(tool_impl) = state.tools.get(&tool) else {
    //     return (
    //         StatusCode::NOT_FOUND,
    //         Json(serde_json::json!({ "error": format!("Tool not found: {tool}") })),
    //     );
    // };
    // if tool_impl.webhook_capability().is_none() {
    //     return (
    //         StatusCode::NOT_FOUND,
    //         Json(serde_json::json!({ "error": format!("Tool does not support webhooks: {tool}") })),
    //     );
    // }
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({ "error": format!("V1 webhook system disabled: {tool}") })),
    )
}

async fn tool_webhook_handler(
    Path(tool): Path<String>,
    State(state): State<ToolWebhookState>,
    method: Method,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    body: axum::body::Bytes,
) -> (StatusCode, Json<serde_json::Value>) {
    tool_webhook_handler_inner(tool, None, state, method, headers, query, body).await
}

async fn tool_webhook_with_rest_handler(
    Path((tool, rest)): Path<(String, String)>,
    State(state): State<ToolWebhookState>,
    method: Method,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    body: axum::body::Bytes,
) -> (StatusCode, Json<serde_json::Value>) {
    tool_webhook_handler_inner(tool, Some(rest), state, method, headers, query, body).await
}

// V1 - async trait not dyn-compatible - commented out
async fn tool_webhook_handler_inner(
    tool: String,
    _rest: Option<String>,
    _state: ToolWebhookState,
    _method: Method,
    _headers: HeaderMap,
    _query: HashMap<String, String>,
    _body: axum::body::Bytes,
) -> (StatusCode, Json<serde_json::Value>) {
    // V1 webhook system disabled
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({ "error": format!("V1 webhook system disabled: {tool}") })),
    )
}

fn header_value<'a>(headers: &'a HeaderMap, key: &str) -> Option<&'a str> {
    // HeaderMap::get() already performs case-insensitive lookup per HTTP spec.
    headers.get(key).and_then(|v| v.to_str().ok())
}

// V1 - async trait not dyn-compatible - commented out
// async fn validate_webhook_auth(
//     tool: &dyn Tool,
//     secrets_store: Option<&(dyn SecretsStore + Send + Sync)>,
//     user_id: &str,
//     headers: &HeaderMap,
//     body: &[u8],
// ) -> Result<(), String> {
//     Err("V1 webhook auth disabled".to_string())
// }

// V1 - deleted: Tests reference V1 types that no longer exist
/*
#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;
    use axum::body::Body;
    use tower::ServiceExt;

    use crate::context::JobContext;
    use crate::secrets::{CreateSecretParams, InMemorySecretsStore, SecretsCrypto};
    // V1 - deleted: use crate::tools::{Tool, ToolError, ToolOutput, ToolRegistry};

    use super::*;

    struct TestWebhookTool;
    struct ProtectedWebhookTool;
    struct HmacWebhookTool;
    /// Tool that declares webhook_capability() but with no auth mechanism configured.
    struct MisconfiguredWebhookTool;

    #[async_trait]
    impl Tool for TestWebhookTool {
        fn name(&self) -> &str {
            "test_webhook"
        }

        fn description(&self) -> &str {
            "test"
        }

        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({"type":"object"})
        }

        async fn execute(
            &self,
            _params: serde_json::Value,
            _ctx: &JobContext,
        ) -> Result<ToolOutput, ToolError> {
            Ok(ToolOutput::success(
                serde_json::json!({"emit_events":[]}),
                Duration::from_millis(1),
            ))
        }
    }

    #[async_trait]
    impl Tool for ProtectedWebhookTool {
        fn name(&self) -> &str {
            "protected_webhook"
        }

        fn description(&self) -> &str {
            "protected test"
        }

        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({"type":"object"})
        }

        async fn execute(
            &self,
            _params: serde_json::Value,
            _ctx: &JobContext,
        ) -> Result<ToolOutput, ToolError> {
            Ok(ToolOutput::success(
                serde_json::json!({"emit_events":[]}),
                Duration::from_millis(1),
            ))
        }

        fn webhook_capability(&self) -> Option<crate::wasm_runtime::WebhookCapability> {
            Some(crate::wasm_runtime::WebhookCapability {
                secret_name: Some("test_webhook_secret".to_string()),
                secret_header: Some("x-webhook-secret".to_string()),
                ..Default::default()
            })
        }
    }

    #[async_trait]
    impl Tool for HmacWebhookTool {
        fn name(&self) -> &str {
            "hmac_webhook"
        }

        fn description(&self) -> &str {
            "hmac test"
        }

        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({"type":"object"})
        }

        async fn execute(
            &self,
            _params: serde_json::Value,
            _ctx: &JobContext,
        ) -> Result<ToolOutput, ToolError> {
            Ok(ToolOutput::success(
                serde_json::json!({"emit_events":[]}),
                Duration::from_millis(1),
            ))
        }

        fn webhook_capability(&self) -> Option<crate::wasm_runtime::WebhookCapability> {
            Some(crate::wasm_runtime::WebhookCapability {
                hmac_secret_name: Some("hmac_secret".to_string()),
                hmac_signature_header: Some("x-hub-signature-256".to_string()),
                hmac_prefix: Some("sha256=".to_string()),
                ..Default::default()
            })
        }
    }

    #[async_trait]
    impl Tool for MisconfiguredWebhookTool {
        fn name(&self) -> &str {
            "misconfigured_webhook"
        }

        fn description(&self) -> &str {
            "misconfigured test"
        }

        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({"type":"object"})
        }

        async fn execute(
            &self,
            _params: serde_json::Value,
            _ctx: &JobContext,
        ) -> Result<ToolOutput, ToolError> {
            Ok(ToolOutput::success(
                serde_json::json!({"emit_events":[]}),
                Duration::from_millis(1),
            ))
        }

        fn webhook_capability(&self) -> Option<crate::wasm_runtime::WebhookCapability> {
            Some(crate::wasm_runtime::WebhookCapability::default())
        }
    }

    #[tokio::test]
    async fn returns_not_found_for_unknown_tool() {
        let tools = Arc::new(ToolRegistry::new());
        let app = routes(ToolWebhookState {
            tools,
            routine_engine: Arc::new(tokio::sync::RwLock::new(None)),
            user_id: "test".to_string(),
            secrets_store: None,
        });

        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/webhook/tools/missing")
            .body(Body::from("{}"))
            .expect("request");
        let resp = ServiceExt::<axum::http::Request<Body>>::oneshot(app, req)
            .await
            .expect("response");
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn rejects_tool_without_webhook_capability() {
        let tools = Arc::new(ToolRegistry::new());
        tools.register(Arc::new(TestWebhookTool)).await;
        let app = routes(ToolWebhookState {
            tools,
            routine_engine: Arc::new(tokio::sync::RwLock::new(None)),
            user_id: "test".to_string(),
            secrets_store: None,
        });

        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/webhook/tools/test_webhook")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"ok":true}"#))
            .expect("request");
        let resp = ServiceExt::<axum::http::Request<Body>>::oneshot(app, req)
            .await
            .expect("response");
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn rejects_when_required_secret_missing() {
        let tools = Arc::new(ToolRegistry::new());
        tools.register(Arc::new(ProtectedWebhookTool)).await;

        let secrets = Arc::new(InMemorySecretsStore::new(Arc::new(
            SecretsCrypto::new(secrecy::SecretString::from(
                "test-key-at-least-32-chars-long!!".to_string(),
            ))
            .expect("crypto"),
        )));
        secrets
            .create(
                "test",
                CreateSecretParams::new("test_webhook_secret", "s3cret"),
            )
            .await
            .expect("secret create");

        let app = routes(ToolWebhookState {
            tools,
            routine_engine: Arc::new(tokio::sync::RwLock::new(None)),
            user_id: "test".to_string(),
            secrets_store: Some(secrets),
        });

        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/webhook/tools/protected_webhook")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"ok":true}"#))
            .expect("request");
        let resp = ServiceExt::<axum::http::Request<Body>>::oneshot(app, req)
            .await
            .expect("response");
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn accepts_with_valid_hmac_signature() {
        use hmac::Mac;

        let tools = Arc::new(ToolRegistry::new());
        tools.register(Arc::new(HmacWebhookTool)).await;

        let secrets = Arc::new(InMemorySecretsStore::new(Arc::new(
            SecretsCrypto::new(secrecy::SecretString::from(
                "test-key-at-least-32-chars-long!!".to_string(),
            ))
            .expect("crypto"),
        )));
        secrets
            .create(
                "test",
                CreateSecretParams::new("hmac_secret", "github-secret"),
            )
            .await
            .expect("secret create");

        let app = routes(ToolWebhookState {
            tools,
            routine_engine: Arc::new(tokio::sync::RwLock::new(None)),
            user_id: "test".to_string(),
            secrets_store: Some(secrets),
        });

        let payload = br#"{"action":"opened"}"#;
        let mut mac =
            hmac::Hmac::<sha2::Sha256>::new_from_slice(b"github-secret").expect("hmac key");
        mac.update(payload);
        let sig = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));

        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/webhook/tools/hmac_webhook")
            .header("content-type", "application/json")
            .header("x-hub-signature-256", sig)
            .body(Body::from(payload.to_vec()))
            .expect("request");
        let resp = ServiceExt::<axum::http::Request<Body>>::oneshot(app, req)
            .await
            .expect("response");
        assert_eq!(resp.status(), StatusCode::ACCEPTED);
    }

    #[tokio::test]
    async fn rejects_empty_webhook_capability_as_misconfigured() {
        let tools = Arc::new(ToolRegistry::new());
        tools.register(Arc::new(MisconfiguredWebhookTool)).await;

        let secrets = Arc::new(InMemorySecretsStore::new(Arc::new(
            SecretsCrypto::new(secrecy::SecretString::from(
                "test-key-at-least-32-chars-long!!".to_string(),
            ))
            .expect("crypto"),
        )));

        let app = routes(ToolWebhookState {
            tools,
            routine_engine: Arc::new(tokio::sync::RwLock::new(None)),
            user_id: "test".to_string(),
            secrets_store: Some(secrets),
        });

        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/webhook/tools/misconfigured_webhook")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"ok":true}"#))
            .expect("request");
        let resp = ServiceExt::<axum::http::Request<Body>>::oneshot(app, req)
            .await
            .expect("response");
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn health_check_returns_ok_for_webhook_capable_tool() {
        let tools = Arc::new(ToolRegistry::new());
        tools.register(Arc::new(ProtectedWebhookTool)).await;
        let app = routes(ToolWebhookState {
            tools,
            routine_engine: Arc::new(tokio::sync::RwLock::new(None)),
            user_id: "test".to_string(),
            secrets_store: None,
        });

        let req = axum::http::Request::builder()
            .method("GET")
            .uri("/webhook/tools/protected_webhook")
            .body(Body::empty())
            .expect("request");
        let resp = ServiceExt::<axum::http::Request<Body>>::oneshot(app, req)
            .await
            .expect("response");
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn health_check_returns_not_found_for_non_webhook_tool() {
        let tools = Arc::new(ToolRegistry::new());
        tools.register(Arc::new(TestWebhookTool)).await;
        let app = routes(ToolWebhookState {
            tools,
            routine_engine: Arc::new(tokio::sync::RwLock::new(None)),
            user_id: "test".to_string(),
            secrets_store: None,
        });

        let req = axum::http::Request::builder()
            .method("GET")
            .uri("/webhook/tools/test_webhook")
            .body(Body::empty())
            .expect("request");
        let resp = ServiceExt::<axum::http::Request<Body>>::oneshot(app, req)
            .await
            .expect("response");
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
*/
