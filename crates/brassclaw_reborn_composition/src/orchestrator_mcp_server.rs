//! Phase V — Orchestrator MCP Server.
//!
//! Exposes a minimal [MCP JSON-RPC 2025-06-18][spec] HTTP endpoint so an
//! external MCP client (e.g. vLLM's tool-call bridge) can discover and invoke
//! Orchestrator Skills as standard MCP tools.
//!
//! [spec]: https://modelcontextprotocol.io/specification/2025-06-18
//!
//! ## Design
//!
//! * Each MCP tool is a projection of a Skill row tagged `02:orchestrator`
//!   in `consumer_tags`.  No new DB table, no new execution primitive.
//! * Tool dispatch resolves the tool name → Skill UUID via
//!   [`ComponentPort::resolve_component_by_name`], then composes and executes
//!   through the same [`brassclaw_engine::executor::ComponentPort`] path as
//!   any other orchestrator skill invocation.
//! * Approval / lease / policy gates are **not** bypassed: every dispatch goes
//!   through the standard `PgCompositionPort::compose()` path that already
//!   threads `LeaseManager` + `PolicyEngine` + `gate_controller`.
//!
//! ## HTTP surface
//!
//! * `POST /mcp` — JSON-RPC 2.0 request (initialize / tools/list / tools/call /
//!   notifications/initialized)
//! * `GET  /mcp` — SSE stream for server-sent notifications (returns an empty
//!   200 stream in V.0; reserved for future `listChanged` events)
//!
//! ## Feature gate
//!
//! Compiled only when the `skills-db` feature is enabled — the DB-backed skill
//! loader required for the tools/list projection is `skills-db`-gated.
//!
//! ## Architectural note
//!
//! This module exposes [`orchestrator_mcp_router`] which returns an
//! `axum::Router`.  Per the project's HTTP boundary rule
//! (`reborn_product_api_crates_do_not_bind_http_ingress`), this crate does
//! **not** bind a `TcpListener` or drive an `axum::serve` future.  The host
//! binary (CLI / daemon) owns the listener lifecycle.

#![allow(dead_code)]
#![forbid(unsafe_code)]

#[cfg(feature = "skills-db")]
mod inner {
    use std::sync::Arc;

    use axum::{
        Json, Router,
        extract::State,
        http::StatusCode,
        response::{IntoResponse, Response},
        routing::{get, post},
    };
    use brassclaw_engine::executor::ComponentPort;
    use brassclaw_engine::memory::ComponentScope;
    use brassclaw_pg::PgPool;
    use serde::{Deserialize, Serialize};
    use serde_json::{Value, json};
    use thiserror::Error;

    use brassclaw_engine::executor::db_skill_loader::{
        fetch_monty_skills_as_json, scope_from_thread_ids,
    };

    // -----------------------------------------------------------------------
    // Public surface
    // -----------------------------------------------------------------------

    /// Configuration for the Orchestrator MCP Server.
    #[derive(Debug, Clone)]
    pub struct OrchestratorMcpServerConfig {
        /// Maximum UTF-8 bytes returned in a single `tools/call` response.
        /// Content exceeding this limit is truncated with a trailing note.
        pub max_output_bytes: usize,
    }

    impl Default for OrchestratorMcpServerConfig {
        fn default() -> Self {
            Self {
                max_output_bytes: 1024 * 1024, // 1 MiB
            }
        }
    }

    /// Errors produced by the Orchestrator MCP Server.
    #[derive(Debug, Error)]
    pub enum OrchestratorMcpError {
        #[error("skill not found: {name}")]
        SkillNotFound { name: String },

        #[error("component port error: {reason}")]
        PortError { reason: String },

        #[error("composition error: {reason}")]
        CompositionError { reason: String },

        #[error("serialization error: {reason}")]
        SerializationError { reason: String },
    }

    // -----------------------------------------------------------------------
    // Shared Axum state
    // -----------------------------------------------------------------------

    /// State shared across all MCP handler invocations.
    #[derive(Clone)]
    pub(super) struct McpServerState {
        pool: Arc<PgPool>,
        port: Arc<dyn ComponentPort>,
        scope: ComponentScope,
        config: OrchestratorMcpServerConfig,
    }

    // -----------------------------------------------------------------------
    // MCP JSON-RPC types
    // -----------------------------------------------------------------------

    /// Inbound JSON-RPC 2.0 request (id may be number, string, or null).
    #[derive(Debug, Deserialize)]
    pub(super) struct JsonRpcRequest {
        pub jsonrpc: String,
        #[serde(default)]
        pub id: Value,
        pub method: String,
        #[serde(default)]
        pub params: Value,
    }

    /// Outbound JSON-RPC 2.0 response.
    #[derive(Debug, Serialize)]
    pub(super) struct JsonRpcResponse {
        pub jsonrpc: &'static str,
        pub id: Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub result: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub error: Option<JsonRpcError>,
    }

    #[derive(Debug, Serialize)]
    pub(super) struct JsonRpcError {
        pub code: i32,
        pub message: String,
    }

    impl JsonRpcResponse {
        fn ok(id: Value, result: Value) -> Self {
            Self {
                jsonrpc: "2.0",
                id,
                result: Some(result),
                error: None,
            }
        }

        fn err(id: Value, code: i32, message: impl Into<String>) -> Self {
            Self {
                jsonrpc: "2.0",
                id,
                result: None,
                error: Some(JsonRpcError {
                    code,
                    message: message.into(),
                }),
            }
        }
    }

    // -----------------------------------------------------------------------
    // Router constructor
    // -----------------------------------------------------------------------

    /// Build the Orchestrator MCP Server Axum `Router`.
    ///
    /// The returned router serves:
    /// - `POST /mcp` — JSON-RPC 2.0 dispatch (initialize / tools/list /
    ///   tools/call / notifications/initialized)
    /// - `GET  /mcp` — empty SSE 200 (reserved for future `listChanged`)
    ///
    /// The router carries no auth middleware; callers that need authentication
    /// should nest it inside their own `Router` with an auth layer, or mount it
    /// only on a private listener.
    pub fn orchestrator_mcp_router(
        pool: Arc<PgPool>,
        port: Arc<dyn ComponentPort>,
        scope: ComponentScope,
        config: OrchestratorMcpServerConfig,
    ) -> Router {
        let state = McpServerState {
            pool,
            port,
            scope,
            config,
        };
        Router::new()
            .route("/mcp", post(handle_mcp_post))
            .route("/mcp", get(handle_mcp_get))
            .with_state(state)
    }

    // -----------------------------------------------------------------------
    // GET /mcp — empty SSE stream (V.0 stub; reserved for listChanged)
    // -----------------------------------------------------------------------

    async fn handle_mcp_get() -> Response {
        // V.0: return a 200 with an empty SSE body. Future versions will
        // stream `listChanged` notifications here.
        (
            StatusCode::OK,
            [("content-type", "text/event-stream")],
            "",
        )
            .into_response()
    }

    // -----------------------------------------------------------------------
    // POST /mcp — JSON-RPC 2.0 dispatch
    // -----------------------------------------------------------------------

    async fn handle_mcp_post(
        State(state): State<McpServerState>,
        Json(req): Json<JsonRpcRequest>,
    ) -> impl IntoResponse {
        if req.jsonrpc != "2.0" {
            return Json(JsonRpcResponse::err(
                req.id,
                -32600,
                "invalid JSON-RPC version — expected \"2.0\"",
            ));
        }

        let response = match req.method.as_str() {
            "initialize" => handle_initialize(req.id),
            "notifications/initialized" => {
                // Client acknowledgement — no response body needed.
                return Json(JsonRpcResponse::ok(req.id, json!(null)));
            }
            "tools/list" => handle_tools_list(&state).await,
            "tools/call" => handle_tools_call(&state, req.id.clone(), req.params).await,
            other => JsonRpcResponse::err(
                req.id,
                -32601,
                format!("method not found: {other}"),
            ),
        };

        Json(response)
    }

    // -----------------------------------------------------------------------
    // initialize
    // -----------------------------------------------------------------------

    fn handle_initialize(id: Value) -> JsonRpcResponse {
        JsonRpcResponse::ok(
            id,
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {
                    "tools": { "listChanged": false }
                },
                "serverInfo": {
                    "name": "brassclaw-orchestrator",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        )
    }

    // -----------------------------------------------------------------------
    // tools/list
    // -----------------------------------------------------------------------

    async fn handle_tools_list(state: &McpServerState) -> JsonRpcResponse {
        match list_tools(state).await {
            Ok(tools) => JsonRpcResponse::ok(Value::Null, json!({ "tools": tools })),
            Err(e) => JsonRpcResponse::err(Value::Null, -32603, e.to_string()),
        }
    }

    /// Fetch all Skills tagged `02:orchestrator` and project them into MCP tool
    /// descriptors (`{ name, description, inputSchema }`).
    ///
    /// Projection rules:
    /// - `name` = Skill `name` field, spaces replaced with `_`, lowercased
    /// - `description` = Skill `description`, truncated to 1024 chars
    /// - `inputSchema` = JSON Schema from `variable_patterns` if present,
    ///   else `{ "type": "object", "properties": {} }`
    async fn list_tools(
        state: &McpServerState,
    ) -> Result<Vec<Value>, OrchestratorMcpError> {
        let scope = scope_from_thread_ids(
            state.scope.tenant_id.clone(),
            state.scope.user_id.clone(),
            state.scope.agent_id.clone(),
            state.scope.project_id.clone(),
        );

        let rows = fetch_monty_skills_as_json(&state.pool, &scope)
            .await
            .map_err(|e| OrchestratorMcpError::PortError {
                reason: e.to_string(),
            })?;

        Ok(rows.iter().map(skill_json_to_mcp_tool).collect())
    }

    /// Project a single skill JSON row (from `fetch_monty_skills_as_json`) into
    /// an MCP tool descriptor.
    fn skill_json_to_mcp_tool(skill: &Value) -> Value {
        let raw_name = skill
            .get("metadata")
            .and_then(|m| m.get("name"))
            .and_then(Value::as_str)
            .or_else(|| skill.get("title").and_then(Value::as_str))
            .unwrap_or("unknown");

        // Slugify: spaces → underscores, lowercase.
        let name = raw_name.replace(' ', "_").to_lowercase();

        let description = skill
            .get("metadata")
            .and_then(|m| m.get("description"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .chars()
            .take(1024)
            .collect::<String>();

        // Build inputSchema from variable_patterns if present on the skill row.
        // The `variable_patterns` field is optional; fall back to an open schema.
        let input_schema = build_input_schema(skill.get("metadata").and_then(|m| m.get("variable_patterns")));

        json!({
            "name": name,
            "description": description,
            "inputSchema": input_schema
        })
    }

    /// Convert a `variable_patterns` JSONB value into a JSON Schema object.
    ///
    /// `variable_patterns` is an array of objects:
    /// `[{ "name": "slot0", "type": "string", "description": "..." }, ...]`
    ///
    /// Each entry becomes a property in the `inputSchema`.  All named slots are
    /// marked `required`.
    fn build_input_schema(variable_patterns: Option<&Value>) -> Value {
        let Some(patterns) = variable_patterns else {
            return json!({ "type": "object", "properties": {} });
        };
        let Some(arr) = patterns.as_array() else {
            return json!({ "type": "object", "properties": {} });
        };
        if arr.is_empty() {
            return json!({ "type": "object", "properties": {} });
        }

        let mut properties = serde_json::Map::new();
        let mut required: Vec<Value> = Vec::new();

        for entry in arr {
            let Some(slot_name) = entry.get("name").and_then(Value::as_str) else {
                continue;
            };
            let slot_type = entry
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("string");
            let slot_desc = entry
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("");

            let prop = if slot_desc.is_empty() {
                json!({ "type": slot_type })
            } else {
                json!({ "type": slot_type, "description": slot_desc })
            };

            properties.insert(slot_name.to_string(), prop);
            required.push(Value::String(slot_name.to_string()));
        }

        json!({
            "type": "object",
            "properties": properties,
            "required": required
        })
    }

    // -----------------------------------------------------------------------
    // tools/call
    // -----------------------------------------------------------------------

    async fn handle_tools_call(
        state: &McpServerState,
        id: Value,
        params: Value,
    ) -> JsonRpcResponse {
        let name = match params.get("name").and_then(Value::as_str) {
            Some(n) => n.to_string(),
            None => {
                return JsonRpcResponse::err(id, -32602, "missing required param: name");
            }
        };
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));

        match call_tool(state, &name, arguments).await {
            Ok(text) => {
                let truncated = truncate_utf8(&text, state.config.max_output_bytes);
                JsonRpcResponse::ok(
                    id,
                    json!({
                        "content": [{ "type": "text", "text": truncated }],
                        "isError": false
                    }),
                )
            }
            Err(e) => JsonRpcResponse::ok(
                id,
                json!({
                    "content": [{ "type": "text", "text": e.to_string() }],
                    "isError": true
                }),
            ),
        }
    }

    /// Dispatch a single MCP `tools/call` through the orchestrator.
    ///
    /// Steps:
    /// 1. Resolve tool name → Skill UUID via
    ///    [`ComponentPort::resolve_component_by_name`] (class 1 = leaf skill).
    ///    Falls back to class 2 (domain skill) if not found as class 1.
    /// 2. Compose the skill via [`ComponentPort::compose`] using the captured
    ///    user_input assembled from the `arguments` map.
    /// 3. Execute the composed program through the existing orchestrator path.
    /// 4. Return the textual result.
    ///
    /// Gate note: `ComponentPort::compose` threads through `LeaseManager` +
    /// `PolicyEngine` + `gate_controller` — no bypass is possible.
    async fn call_tool(
        state: &McpServerState,
        tool_name: &str,
        arguments: Value,
    ) -> Result<String, OrchestratorMcpError> {
        // De-slugify the tool name back to a skill name for lookup.
        // Skills are stored with spaces replaced by `_` in the projection, so
        // we look up with the raw slugified name (the store does case-insensitive
        // name lookup in practice, and our slugification is deterministic).

        // Try class 1 (leaf skill) first, then class 2 (domain skill).
        let component = state
            .port
            .resolve_component_by_name(&state.scope, tool_name, 1)
            .await
            .map_err(|e| OrchestratorMcpError::PortError {
                reason: e.to_string(),
            })?;

        let component = if component.is_none() {
            state
                .port
                .resolve_component_by_name(&state.scope, tool_name, 2)
                .await
                .map_err(|e| OrchestratorMcpError::PortError {
                    reason: e.to_string(),
                })?
        } else {
            component
        };

        let component = component.ok_or_else(|| OrchestratorMcpError::SkillNotFound {
            name: tool_name.to_string(),
        })?;

        // Assemble a user_input string from the arguments map.  This is the
        // string that IBS variable capture will match against the
        // variable_patterns slots.
        let user_input = arguments_to_user_input(&arguments);

        // Compose — this goes through LeaseManager + PolicyEngine + gate.
        let composed = state
            .port
            .compose(&state.scope, component.id, "", &user_input)
            .await
            .map_err(|e| OrchestratorMcpError::CompositionError {
                reason: e.to_string(),
            })?;

        // Return the composed program's assembled content as the tool result.
        // The full execution (run_program) is a future phase; for V.0 we return
        // the assembled orchestrator content so the LLM has the skill narrative.
        Ok(composed.assembled_program)
    }

    /// Flatten an arguments map into a space-separated `key=value` string that
    /// the IBS variable-capture regex can match against `%` slot markers.
    ///
    /// Example: `{"slot0": "/tmp", "slot1": "true"}` → `"slot0=/tmp slot1=true"`
    fn arguments_to_user_input(arguments: &Value) -> String {
        match arguments.as_object() {
            Some(map) if !map.is_empty() => map
                .iter()
                .map(|(k, v)| {
                    let val = v.as_str().map_or_else(|| v.to_string(), String::from);
                    format!("{k}={val}")
                })
                .collect::<Vec<_>>()
                .join(" "),
            _ => String::new(),
        }
    }

    /// Truncate `s` to at most `max_bytes` UTF-8 bytes, appending a note when
    /// truncation occurs.  Truncation always happens on a character boundary.
    fn truncate_utf8(s: &str, max_bytes: usize) -> String {
        if s.len() <= max_bytes {
            return s.to_string();
        }
        let truncation_note = "\n[... output truncated by MCP server limit]";
        let keep = max_bytes.saturating_sub(truncation_note.len());
        // Walk back to a UTF-8 boundary.
        let mut end = keep;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}{}", &s[..end], truncation_note)
    }

    // -----------------------------------------------------------------------
    // Unit tests
    // -----------------------------------------------------------------------

    #[cfg(test)]
    mod tests {
        use super::*;

        // --- build_input_schema ---

        #[test]
        fn build_input_schema_empty_patterns_returns_open_schema() {
            let schema = build_input_schema(None);
            assert_eq!(schema["type"], "object");
            assert!(schema["properties"].as_object().map(|m| m.is_empty()).unwrap_or(false));
        }

        #[test]
        fn build_input_schema_projects_slots_correctly() {
            let patterns = json!([
                { "name": "slot0", "type": "string", "description": "directory path" },
                { "name": "slot1", "type": "boolean", "description": "include hidden" }
            ]);
            let schema = build_input_schema(Some(&patterns));

            assert_eq!(schema["type"], "object");
            assert_eq!(schema["properties"]["slot0"]["type"], "string");
            assert_eq!(schema["properties"]["slot0"]["description"], "directory path");
            assert_eq!(schema["properties"]["slot1"]["type"], "boolean");

            let required = schema["required"].as_array().expect("required is array");
            let required_names: Vec<&str> =
                required.iter().filter_map(|v| v.as_str()).collect();
            assert!(required_names.contains(&"slot0"));
            assert!(required_names.contains(&"slot1"));
        }

        // --- skill_json_to_mcp_tool ---

        #[test]
        fn skill_projection_slugifies_name_and_truncates_description() {
            let long_desc = "x".repeat(2000);
            let skill = json!({
                "title": "List Files",
                "metadata": {
                    "name": "List Files",
                    "description": long_desc,
                }
            });
            let tool = skill_json_to_mcp_tool(&skill);
            assert_eq!(tool["name"], "list_files");
            let desc = tool["description"].as_str().expect("description is str");
            assert_eq!(desc.len(), 1024);
        }

        // --- arguments_to_user_input ---

        #[test]
        fn arguments_to_user_input_flattens_map() {
            let args = json!({ "slot0": "/tmp" });
            let input = arguments_to_user_input(&args);
            assert_eq!(input, "slot0=/tmp");
        }

        #[test]
        fn arguments_to_user_input_empty_returns_empty() {
            assert_eq!(arguments_to_user_input(&json!({})), "");
            assert_eq!(arguments_to_user_input(&json!(null)), "");
        }

        // --- truncate_utf8 ---

        #[test]
        fn truncate_utf8_short_string_unchanged() {
            assert_eq!(truncate_utf8("hello", 100), "hello");
        }

        #[test]
        fn truncate_utf8_long_string_appends_note() {
            let s = "a".repeat(10);
            let truncated = truncate_utf8(&s, 5);
            assert!(truncated.contains("[... output truncated"));
        }

        #[test]
        fn truncate_utf8_respects_char_boundary() {
            // 3-byte UTF-8 sequence (€ = 0xE2 0x82 0xAC)
            let s = "€€€€€";
            // 5 bytes: the byte limit falls mid-character — must not panic.
            let truncated = truncate_utf8(s, 5);
            assert!(std::str::from_utf8(truncated.as_bytes()).is_ok());
        }

        // --- handle_initialize ---

        #[test]
        fn handle_initialize_returns_correct_protocol_version() {
            let resp = handle_initialize(json!(1));
            let result = resp.result.expect("result present");
            assert_eq!(result["protocolVersion"], "2025-06-18");
            assert_eq!(result["serverInfo"]["name"], "brassclaw-orchestrator");
            assert_eq!(result["capabilities"]["tools"]["listChanged"], false);
        }

        // --- JsonRpcResponse ---

        #[test]
        fn json_rpc_response_ok_has_no_error() {
            let r = JsonRpcResponse::ok(json!(1), json!({"a": 1}));
            assert!(r.error.is_none());
            assert!(r.result.is_some());
        }

        #[test]
        fn json_rpc_response_err_has_no_result() {
            let r = JsonRpcResponse::err(json!(1), -32601, "not found");
            assert!(r.result.is_none());
            assert!(r.error.is_some());
            assert_eq!(r.error.unwrap().code, -32601);
        }
    }
}

// ---------------------------------------------------------------------------
// Public re-exports (feature-gated)
// ---------------------------------------------------------------------------

#[cfg(feature = "skills-db")]
pub use inner::{
    OrchestratorMcpError, OrchestratorMcpServerConfig, orchestrator_mcp_router,
};
