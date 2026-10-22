//! Codex ChatGPT Responses API provider.
//!
//! Implements `LlmProvider` by speaking the OpenAI Responses API protocol
//! (`POST /responses`) used by the ChatGPT backend at
//! `chatgpt.com/backend-api/codex`. This bypasses `rig-core`'s Chat
//! Completions path, which is incompatible with this endpoint.
//!
//! # Warning
//!
//! The ChatGPT backend endpoint (`chatgpt.com/backend-api/codex`) is a
//! **private, undocumented API**. Using subscriber OAuth tokens from a
//! third-party application may violate the token's intended scope or
//! OpenAI's Terms of Service. This feature is provided as-is for
//! convenience and may break without notice.

use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::{Stream, StreamExt};
use reqwest::Client;
use rust_decimal::Decimal;
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};
use std::borrow::Cow;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};

use super::codex_auth;
use crate::error::LlmError;

use super::provider::{
    ChatMessage, CompletionRequest, CompletionResponse, ContentPart, FinishReason, LlmProvider,
    Role, ToolCall, ToolCompletionRequest, ToolCompletionResponse, ToolDefinition,
};

/// Sanitize a tool name to match the Responses API pattern `^[a-zA-Z0-9_-]+$`.
fn sanitize_tool_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn convert_tool_definition(tool: &ToolDefinition) -> Value {
    use crate::tool_schema::{ToolSchemaPolicy, shape_tool_schema};

    let mut description = tool.description.clone();
    let parameters = shape_tool_schema(
        ToolSchemaPolicy::StrictOpenAi,
        &tool.parameters,
        &mut description,
    );

    json!({
        "type": "function",
        "name": sanitize_tool_name(&tool.name),
        "description": description,
        "parameters": parameters,
    })
}

fn build_sanitized_tool_name_map(
    tools: &[ToolDefinition],
) -> Result<std::collections::HashMap<String, String>, LlmError> {
    let mut name_map = std::collections::HashMap::new();
    for tool in tools {
        let sanitized = sanitize_tool_name(&tool.name);
        if let Some(existing) = name_map.insert(sanitized.clone(), tool.name.clone())
            && existing != tool.name
        {
            return Err(LlmError::InvalidResponse {
                provider: "codex_chatgpt".to_string(),
                reason: format!(
                    "tool names `{existing}` and `{}` both map to provider name `{sanitized}`",
                    tool.name
                ),
            });
        }
    }
    Ok(name_map)
}

/// Provider that speaks the Responses API protocol against the ChatGPT backend.
pub(crate) struct CodexChatGptProvider {
    client: Client,
    base_url: String,
    api_key: RwLock<SecretString>,
    /// User-configured model name (or empty/"default" for auto-detect).
    configured_model: String,
    /// Lazily resolved model name (populated on first LLM call).
    resolved_model: tokio::sync::OnceCell<String>,
    /// OAuth refresh token for automatic 401 retry.
    refresh_token: Option<SecretString>,
    /// Path to auth.json for persisting refreshed tokens.
    auth_path: Option<PathBuf>,
    /// Timeout for actual `/responses` requests.
    request_timeout: Duration,
    /// Prevent concurrent 401 handlers from racing the same refresh token.
    refresh_lock: Mutex<()>,
}

impl CodexChatGptProvider {
    #[cfg(test)]
    fn new(base_url: &str, api_key: &str, model: &str) -> Self {
        Self {
            client: crate::registry_http_client("codex_chatgpt", 120).expect("fixture HTTP client"),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: RwLock::new(SecretString::from(api_key.to_string())),
            configured_model: model.to_string(),
            resolved_model: tokio::sync::OnceCell::const_new(),
            refresh_token: None,
            auth_path: None,
            request_timeout: Duration::from_secs(120),
            refresh_lock: Mutex::new(()),
        }
    }

    /// Create a provider with lazy model detection.
    ///
    /// The model is **not** resolved during construction. Instead, it is
    /// resolved on the first LLM call via [`resolve_model`], avoiding the
    /// need for `block_in_place` / `block_on` during provider setup.
    ///
    /// **Model selection priority** (applied at resolution time):
    /// 1. If `configured_model` is non-empty, validate it against the
    ///    `/models` endpoint. If it isn't in the supported list, log a
    ///    warning with available models and fall back to the top model.
    /// 2. If `configured_model` is empty (or a generic placeholder like
    ///    "default"), auto-detect the highest-priority model from the API.
    pub(crate) fn with_lazy_model(
        base_url: &str,
        api_key: SecretString,
        configured_model: &str,
        refresh_token: Option<SecretString>,
        auth_path: Option<PathBuf>,
        request_timeout_secs: u64,
    ) -> Result<Self, LlmError> {
        tracing::debug!(
            "Codex ChatGPT provider uses a private, undocumented API \
             (chatgpt.com/backend-api/codex). This may violate OpenAI's \
             Terms of Service and could break without notice."
        );

        Ok(Self {
            client: crate::registry_http_client("codex_chatgpt", request_timeout_secs)?,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: RwLock::new(api_key),
            configured_model: configured_model.to_string(),
            resolved_model: tokio::sync::OnceCell::const_new(),
            refresh_token,
            auth_path,
            request_timeout: Duration::from_secs(request_timeout_secs),
            refresh_lock: Mutex::new(()),
        })
    }

    /// Preserve a pinned operation's model; legacy calls retain lazy detection.
    async fn selected_request_model(
        &self,
        requested: Option<&str>,
        policy: crate::ProviderDispatchPolicy,
    ) -> Result<String, LlmError> {
        if policy.allows_resubmission() {
            return Ok(self.resolve_model().await.to_owned());
        }
        // A pinned operation must not auto-detect/fall back to a different model.
        crate::provider::normalized_model_override(requested)
            .or_else(|| crate::provider::normalized_model_override(Some(&self.configured_model)))
            .map(str::to_owned)
            .ok_or_else(|| LlmError::ModelNotAvailable {
                provider: "codex_chatgpt".into(),
                model: self.configured_model.clone(),
            })
    }

    /// Resolve legacy model selection lazily; `/models` is read at most once.
    async fn resolve_model(&self) -> &str {
        self.resolved_model
            .get_or_init(|| async {
                let api_key = self.api_key.read().await.clone();
                let available = Self::fetch_available_models(&self.client, &self.base_url, &api_key)
                    .await;

                let configured = &self.configured_model;
                if !configured.is_empty() && configured != "default" {
                    // User explicitly configured a model — validate it
                    if available.is_empty() {
                        tracing::debug!(
                            "Could not fetch model list; using configured model '{configured}'"
                        );
                        return configured.clone();
                    }
                    if available.iter().any(|m| m == configured) {
                        tracing::debug!(model = %configured, "Codex ChatGPT: using configured model");
                        return configured.clone();
                    }
                    tracing::debug!(
                        configured = %configured,
                        available = ?available,
                        "Configured model not found in supported list, falling back to top model"
                    );
                    available
                        .into_iter()
                        .next()
                        .unwrap_or_else(|| configured.clone())
                } else {
                    // No user preference — auto-detect
                    if let Some(top) = available.into_iter().next() {
                        tracing::debug!(model = %top, "Codex ChatGPT: auto-detected model");
                        top
                    } else {
                        tracing::debug!(
                            "Could not auto-detect model, using fallback '{configured}'"
                        );
                        configured.clone()
                    }
                }
            })
            .await
    }

    /// Query `/models?client_version=0.111.0` and return the list of available
    /// model slugs, ordered by priority (highest first).
    async fn fetch_available_models(
        client: &Client,
        base_url: &str,
        api_key: &SecretString,
    ) -> Vec<String> {
        let url = format!("{base_url}/models?client_version=0.111.0");
        let resp = match client
            .get(&url)
            .bearer_auth(api_key.expose_secret())
            .timeout(Duration::from_secs(10))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::debug!("Failed to fetch Codex models: {e}");
                return Vec::new();
            }
        };
        if !resp.status().is_success() {
            tracing::debug!(status = %resp.status(), "Failed to fetch Codex models");
            return Vec::new();
        }
        let body: Value = match resp.json().await {
            Ok(v) => v,
            Err(_) => return Vec::new(),
        };
        // The response has { "models": [ { "slug": "...", ... }, ... ] }
        body.get("models")
            .and_then(|m| m.as_array())
            .map(|models| {
                models
                    .iter()
                    .filter_map(|m| {
                        m.get("slug")
                            .and_then(|s| s.as_str())
                            .map(|s| s.to_string())
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Convert BrassClaw messages to Responses API request JSON.
    fn build_request_body(
        &self,
        model: &str,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        tool_choice: Option<&str>,
    ) -> Value {
        // Extract system instructions
        let instructions: String = messages
            .iter()
            .filter(|m| m.role == Role::System)
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");

        // Convert non-system messages to Responses API input items
        let input: Vec<Value> = messages
            .iter()
            .filter(|m| m.role != Role::System)
            .flat_map(Self::message_to_input_items)
            .collect();

        // Convert tool definitions. Responses API function names allow only
        // `[a-zA-Z0-9_-]`, while host capability ids can contain dots
        // (`builtin.echo`, MCP ids, etc.). Keep provider-facing names sanitized
        // and map them back to original names after the response is parsed.
        let api_tools: Vec<Value> = tools.iter().map(convert_tool_definition).collect();

        let mut body = json!({
            "model": model,
            "instructions": instructions,
            "input": input,
            "stream": true,
            "store": false,
        });

        // Only add `reasoning` for models that support it;
        // the Responses API hard-rejects it on non-reasoning models.
        if crate::reasoning_models::supports_openai_reasoning(model) {
            body["reasoning"] = crate::responses_reasoning::summary_request();
        }

        if !api_tools.is_empty() {
            body["tools"] = json!(api_tools);
            body["tool_choice"] = json!(tool_choice.unwrap_or("auto"));
        }

        body
    }

    /// Convert a single ChatMessage to one or more Responses API input items.
    fn message_to_input_items(msg: &ChatMessage) -> Vec<Value> {
        let mut items = Vec::new();

        match msg.role {
            Role::User => {
                // Build content array: if content_parts is populated, use it
                // to include multimodal content (images). Otherwise fall back
                // to the plain text content field.
                let content = if !msg.content_parts.is_empty() {
                    msg.content_parts
                        .iter()
                        .map(|part| match part {
                            ContentPart::Text { text } => json!({
                                "type": "input_text",
                                "text": text,
                            }),
                            ContentPart::ImageUrl { image_url } => json!({
                                "type": "input_image",
                                "image_url": image_url.url,
                            }),
                        })
                        .collect::<Vec<_>>()
                } else {
                    vec![json!({
                        "type": "input_text",
                        "text": msg.content,
                    })]
                };

                items.push(json!({
                    "type": "message",
                    "role": "user",
                    "content": content,
                }));
            }
            Role::Assistant => {
                // If the assistant message has tool calls, emit function_call items
                if let Some(ref tool_calls) = msg.tool_calls {
                    // Emit the assistant text as a message if non-empty
                    if !msg.content.is_empty() {
                        items.push(json!({
                            "type": "message",
                            "role": "assistant",
                            "content": [{
                                "type": "output_text",
                                "text": msg.content,
                            }],
                        }));
                    }
                    for tc in tool_calls {
                        let args = if tc.arguments.is_string() {
                            tc.arguments.as_str().unwrap_or("{}").to_string()
                        } else {
                            serde_json::to_string(&tc.arguments).unwrap_or_default()
                        };
                        items.push(json!({
                            "type": "function_call",
                            "name": sanitize_tool_name(&tc.name),
                            "arguments": args,
                            "call_id": tc.id,
                        }));
                    }
                } else {
                    items.push(json!({
                        "type": "message",
                        "role": "assistant",
                        "content": [{
                            "type": "output_text",
                            "text": msg.content,
                        }],
                    }));
                }
            }
            Role::Tool => {
                items.push(json!({
                    "type": "function_call_output",
                    "call_id": msg.tool_call_id.as_deref().unwrap_or(""),
                    "output": msg.content,
                }));
            }
            Role::System => {
                // System messages are handled via `instructions` field
            }
        }

        items
    }

    /// Send a request and parse the SSE response.
    ///
    /// On HTTP 401, if a refresh token is available, attempts to refresh
    /// the access token and retry the request once.
    async fn send_request(
        &self,
        body: Value,
        dispatch_policy: crate::ProviderDispatchPolicy,
    ) -> Result<ResponsesResult, LlmError> {
        let url = format!("{}/responses", self.base_url);

        tracing::debug!(
            url = %url,
            model = %body.get("model").and_then(|m| m.as_str()).unwrap_or("?"),
            "Codex ChatGPT: sending request"
        );

        let api_key = self.api_key.read().await.clone();
        let resp =
            Self::send_http_request(&self.client, &url, &api_key, &body, self.request_timeout)
                .await?;

        let status = resp.status();
        if status.as_u16() == 401 && dispatch_policy.allows_resubmission() {
            // Attempt token refresh if we have a refresh token
            if let Some(ref rt) = self.refresh_token {
                let _refresh_guard = self.refresh_lock.lock().await;
                let current_token = self.api_key.read().await.clone();

                if current_token.expose_secret() != api_key.expose_secret() {
                    tracing::debug!(
                        "Received 401, but another request already refreshed the token"
                    );
                    let retry_resp = Self::send_http_request(
                        &self.client,
                        &url,
                        &current_token,
                        &body,
                        self.request_timeout,
                    )
                    .await?;
                    let retry_status = retry_resp.status();
                    if !retry_status.is_success() {
                        return Err(crate::response_body::rejection(
                            retry_resp,
                            None,
                            "codex_chatgpt",
                        )
                        .await);
                    }
                    return Self::parse_sse_response_stream(retry_resp, self.request_timeout).await;
                }

                tracing::debug!("Received 401, attempting token refresh");
                if let Some(new_token) =
                    codex_auth::refresh_access_token(&self.client, rt, self.auth_path.as_deref())
                        .await
                {
                    // Update stored api_key
                    *self.api_key.write().await = new_token.clone();
                    tracing::debug!("Token refreshed, retrying request");

                    // Retry the request with the new token
                    let retry_resp = Self::send_http_request(
                        &self.client,
                        &url,
                        &new_token,
                        &body,
                        self.request_timeout,
                    )
                    .await?;

                    let retry_status = retry_resp.status();
                    if !retry_status.is_success() {
                        return Err(crate::response_body::rejection(
                            retry_resp,
                            None,
                            "codex_chatgpt",
                        )
                        .await);
                    }

                    return Self::parse_sse_response_stream(retry_resp, self.request_timeout).await;
                } else {
                    tracing::debug!(
                        "Token refresh failed. Please re-authenticate with: codex --login"
                    );
                }
            }

            // No refresh token or refresh failed — return the 401 error
            // Drain the response body to release the connection
            let _ = resp.text().await;
            return Err(LlmError::AuthFailed {
                provider: "codex_chatgpt".to_string(),
            });
        }

        if !status.is_success() {
            return Err(crate::response_body::rejection(resp, None, "codex_chatgpt").await);
        }

        Self::parse_sse_response_stream(resp, self.request_timeout).await
    }

    /// Low-level HTTP POST to the /responses endpoint.
    async fn send_http_request(
        client: &Client,
        url: &str,
        api_key: &SecretString,
        body: &Value,
        timeout: Duration,
    ) -> Result<reqwest::Response, LlmError> {
        client
            .post(url)
            .bearer_auth(api_key.expose_secret())
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .json(body)
            .timeout(timeout)
            .send()
            .await
            .map_err(|e| LlmError::RequestFailed {
                provider: "codex_chatgpt".to_string(),
                reason: format!("HTTP request failed: {}", e.without_url()),
            })
    }

    async fn parse_sse_response_stream(
        resp: reqwest::Response,
        idle_timeout: Duration,
    ) -> Result<ResponsesResult, LlmError> {
        let stream = resp
            .bytes_stream()
            .map(|chunk| chunk.map_err(|e| e.to_string()));
        Self::parse_sse_stream(stream, idle_timeout).await
    }

    async fn parse_sse_stream<S>(
        stream: S,
        idle_timeout: Duration,
    ) -> Result<ResponsesResult, LlmError>
    where
        S: Stream<Item = Result<bytes::Bytes, String>> + Unpin,
    {
        let mut result = ResponsesResult::default();
        let mut stream = stream.eventsource();

        loop {
            match tokio::time::timeout(idle_timeout, stream.next()).await {
                Ok(Some(Ok(event))) => {
                    let data = event.data.trim();
                    if data.is_empty() {
                        continue;
                    }
                    if data == "[DONE]" {
                        return Ok(result);
                    }

                    let parsed: Value = match serde_json::from_str(data) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    let event_type = Self::resolve_sse_event_type(event.event.as_str(), &parsed);
                    if Self::handle_sse_event(&mut result, event_type.as_ref(), &parsed) {
                        return Ok(result);
                    }
                }
                Ok(Some(Err(e))) => {
                    return Err(LlmError::RequestFailed {
                        provider: "codex_chatgpt".to_string(),
                        reason: format!("Failed to read SSE stream: {e}"),
                    });
                }
                Ok(None) => return Ok(result),
                Err(_) => {
                    return Err(LlmError::RequestFailed {
                        provider: "codex_chatgpt".to_string(),
                        reason: format!(
                            "Timed out waiting for SSE event after {}s",
                            idle_timeout.as_secs()
                        ),
                    });
                }
            }
        }
    }

    /// Parse SSE events from the response text.
    #[cfg(test)]
    fn parse_sse_response(sse_text: &str) -> Result<ResponsesResult, LlmError> {
        let mut result = ResponsesResult::default();
        let mut current_event_type = String::new();

        for line in sse_text.lines() {
            if let Some(event) = line.strip_prefix("event: ") {
                current_event_type = event.trim().to_string();
                continue;
            }

            if let Some(data) = line.strip_prefix("data: ") {
                let data = data.trim();
                if data.is_empty() {
                    continue;
                }
                if data == "[DONE]" {
                    return Ok(result);
                }

                let parsed: Value = match serde_json::from_str(data) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let event_type = Self::resolve_sse_event_type(current_event_type.as_str(), &parsed);
                if Self::handle_sse_event(&mut result, event_type.as_ref(), &parsed) {
                    return Ok(result);
                }
            }
        }

        Ok(result)
    }

    fn resolve_sse_event_type<'a>(event_type: &'a str, parsed: &'a Value) -> Cow<'a, str> {
        if !event_type.is_empty() && event_type != "message" {
            return Cow::Borrowed(event_type);
        }
        parsed
            .get("type")
            .and_then(|value| value.as_str())
            .map(Cow::Borrowed)
            .unwrap_or_else(|| Cow::Borrowed(event_type))
    }

    fn handle_sse_event(result: &mut ResponsesResult, event_type: &str, parsed: &Value) -> bool {
        match event_type {
            "response.output_text.delta" => {
                if let Some(delta) = parsed.get("delta").and_then(|d| d.as_str()) {
                    result.text.push_str(delta);
                }
            }
            "response.output_text.done" => {
                if result.text.is_empty()
                    && let Some(text) = parsed.get("text").and_then(|d| d.as_str())
                {
                    result.text.push_str(text);
                }
            }
            event_type
                if crate::responses_reasoning::apply_summary_event(
                    &mut result.reasoning,
                    event_type,
                    parsed,
                ) => {}
            "response.output_item.added" => {
                // Capture function call metadata when the item is first added.
                // The item has: id (item_id), call_id, name, type.
                let item = parsed.get("item").unwrap_or(parsed);
                if item.get("type").and_then(|t| t.as_str()) == Some("function_call") {
                    let item_id = item
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let call_id = item
                        .get("call_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let name = item
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    result
                        .pending_tool_calls
                        .entry(item_id)
                        .or_insert_with(|| PendingToolCall {
                            call_id,
                            name,
                            arguments: String::new(),
                        });
                }
            }
            "response.function_call_arguments.delta" => {
                // Delta events use `item_id` (not `call_id`)
                if let Some(item_id) = parsed.get("item_id").and_then(|v| v.as_str())
                    && let Some(entry) = result.pending_tool_calls.get_mut(item_id)
                    && let Some(delta) = parsed.get("delta").and_then(|d| d.as_str())
                {
                    entry.arguments.push_str(delta);
                }
            }
            "response.function_call_arguments.done" => {
                if let Some(item_id) = parsed.get("item_id").and_then(|v| v.as_str())
                    && let Some(entry) = result.pending_tool_calls.get_mut(item_id)
                    && let Some(arguments) = parsed.get("arguments").and_then(|d| d.as_str())
                {
                    entry.arguments = arguments.to_string();
                }
            }
            "response.output_item.done" => {
                if let Some(item) = parsed.get("item").or_else(|| parsed.get("output")) {
                    Self::merge_completed_output_item(result, item, result.text.is_empty());
                }
            }
            "response.completed" | "response.incomplete" | "response.failed" => {
                if let Some(response) = parsed.get("response")
                    && let Some(usage) = response.get("usage")
                {
                    result.usage = crate::TokenUsage::from_reported_counts(
                        usage.get("input_tokens").and_then(|v| v.as_u64()),
                        usage.get("output_tokens").and_then(|v| v.as_u64()),
                        0,
                        0,
                    );
                }
                if let Some(response) = parsed.get("response").filter(|r| r.is_object()) {
                    result.status =
                        crate::provider::responses_terminal_status(event_type, response)
                            .map(str::to_owned);
                    result.incomplete_reason = response
                        .get("incomplete_details")
                        .and_then(|v| v.get("reason"))
                        .and_then(|v| v.as_str())
                        .map(str::to_owned);
                    Self::merge_completed_response_output(result, response);
                }
                tracing::debug!(
                    content_bytes = result.text.len(),
                    tool_call_count = result.pending_tool_calls.len(),
                    input_tokens = ?result.usage.map(|usage| usage.input_tokens),
                    output_tokens = ?result.usage.map(|usage| usage.output_tokens),
                    "Codex ChatGPT: parsed completed response"
                );
                return true;
            }
            _ => {}
        }

        false
    }

    fn merge_completed_response_output(result: &mut ResponsesResult, response: &Value) {
        if result.text.is_empty()
            && let Some(output_text) = response.get("output_text").and_then(|value| value.as_str())
        {
            result.text.push_str(output_text);
        }

        let allow_text_fallback = result.text.is_empty();
        if let Some(output) = response.get("output").and_then(|value| value.as_array()) {
            for item in output {
                Self::merge_completed_output_item(result, item, allow_text_fallback);
            }
        }
    }

    fn merge_completed_output_item(
        result: &mut ResponsesResult,
        item: &Value,
        allow_text_fallback: bool,
    ) {
        match item.get("type").and_then(|value| value.as_str()) {
            Some("message") if allow_text_fallback => {
                Self::append_output_message_text(&mut result.text, item);
            }
            Some("function_call") => {
                let item_id = item
                    .get("id")
                    .and_then(|value| value.as_str())
                    .or_else(|| item.get("call_id").and_then(|value| value.as_str()))
                    .unwrap_or("")
                    .to_string();
                if item_id.is_empty() {
                    return;
                }
                let call_id = item
                    .get("call_id")
                    .and_then(|value| value.as_str())
                    .unwrap_or(&item_id)
                    .to_string();
                let name = item
                    .get("name")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string();
                let arguments = item
                    .get("arguments")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string();
                result
                    .pending_tool_calls
                    .entry(item_id)
                    .and_modify(|existing| {
                        if !call_id.is_empty() {
                            existing.call_id = call_id.clone();
                        }
                        if !name.is_empty() {
                            existing.name = name.clone();
                        }
                        if !arguments.is_empty() {
                            existing.arguments = arguments.clone();
                        }
                    })
                    .or_insert_with(|| PendingToolCall {
                        call_id,
                        name,
                        arguments,
                    });
            }
            _ => {}
        }
    }

    fn append_output_message_text(output: &mut String, item: &Value) {
        let Some(content) = item.get("content").and_then(|value| value.as_array()) else {
            return;
        };
        for part in content {
            if part.get("type").and_then(|value| value.as_str()) == Some("output_text")
                && let Some(text) = part.get("text").and_then(|value| value.as_str())
            {
                output.push_str(text);
            }
        }
    }
}

#[derive(Debug, Default)]
struct ResponsesResult {
    text: String,
    reasoning: String,
    /// Keyed by item_id (the SSE item identifier, e.g. "fc_...").
    pending_tool_calls: std::collections::HashMap<String, PendingToolCall>,
    usage: Option<crate::TokenUsage>,
    status: Option<String>,
    incomplete_reason: Option<String>,
}

#[derive(Debug)]
struct PendingToolCall {
    /// The call_id from the API (e.g. "call_..."), used to match results.
    call_id: String,
    name: String,
    arguments: String,
}

#[async_trait]
impl LlmProvider for CodexChatGptProvider {
    fn model_name(&self) -> std::borrow::Cow<'_, str> {
        // Return resolved model if available, otherwise the configured name.
        std::borrow::Cow::Borrowed(
            self.resolved_model
                .get()
                .map(|s| s.as_str())
                .unwrap_or(&self.configured_model),
        )
    }

    fn cost_per_token(&self) -> (Decimal, Decimal) {
        // ChatGPT backend doesn't expose per-token pricing
        (Decimal::ZERO, Decimal::ZERO)
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        let model = self
            .selected_request_model(request.model.as_deref(), request.dispatch_policy)
            .await?;
        let mut messages = request.messages;
        crate::provider::sanitize_tool_messages(&mut messages);
        let body = self.build_request_body(&model, &messages, &[], None);
        let result = self.send_request(body, request.dispatch_policy).await?;

        Ok(CompletionResponse {
            content: result.text,
            usage: result.usage,

            finish_reason: FinishReason::from_responses_status(
                result.status.as_deref(),
                result.incomplete_reason.as_deref(),
                !result.pending_tool_calls.is_empty(),
            ),
            reasoning: crate::responses_reasoning::finish_summary(result.reasoning),
        })
    }

    async fn complete_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse, LlmError> {
        let mut messages = request.messages;
        crate::provider::sanitize_tool_messages(&mut messages);
        let name_map = build_sanitized_tool_name_map(&request.tools)?;
        let model = self
            .selected_request_model(request.model.as_deref(), request.dispatch_policy)
            .await?;
        let body = self.build_request_body(
            &model,
            &messages,
            &request.tools,
            request.tool_choice.as_deref(),
        );
        let result = self.send_request(body, request.dispatch_policy).await?;

        let mut finish_reason = FinishReason::from_responses_status(
            result.status.as_deref(),
            result.incomplete_reason.as_deref(),
            !result.pending_tool_calls.is_empty(),
        );
        let mut valid_arguments = true;
        let tool_calls: Vec<ToolCall> = result
            .pending_tool_calls
            .into_values()
            .map(|tc| {
                let returned_name = tc.name;
                let name = name_map
                    .get(&returned_name)
                    .cloned()
                    .unwrap_or(returned_name);
                let (args, valid) = crate::provider::decode_provider_arguments(&tc.arguments);
                valid_arguments &= valid;
                ToolCall {
                    id: tc.call_id,
                    name,
                    arguments: args,
                    reasoning: None,
                    signature: None,
                }
            })
            .collect();
        if !valid_arguments {
            finish_reason = FinishReason::Unknown;
        }

        Ok(ToolCompletionResponse {
            content: if result.text.is_empty() {
                None
            } else {
                Some(result.text)
            },
            tool_calls,
            usage: result.usage,

            finish_reason,

            reasoning: crate::responses_reasoning::finish_summary(result.reasoning),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use futures::stream;

    #[tokio::test]
    async fn single_submission_uses_the_selected_codex_model_and_private_http_rejections() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let observations = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let recorded = observations.clone();
        let (stop, mut stopped) = tokio::sync::watch::channel(false);
        let server = tokio::spawn(async move {
            loop {
                let (mut socket, _) = tokio::select! {
                    _ = stopped.changed() => break,
                    connection = listener.accept() => connection.unwrap(),
                };
                let mut bytes = Vec::new();
                let mut buffer = [0; 4096];
                loop {
                    let count = socket.read(&mut buffer).await.unwrap();
                    assert_ne!(count, 0);
                    bytes.extend_from_slice(&buffer[..count]);
                    if let Some(end) = bytes.windows(4).position(|data| data == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]);
                        let length = headers
                            .lines()
                            .find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                name.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        if bytes.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                recorded.lock().await.push(bytes);
                let body = "private-codex-rejection sk-private-body";
                socket.write_all(format!("HTTP/1.1 401 Unauthorized\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
            }
        });
        let provider = CodexChatGptProvider::with_lazy_model(
            &endpoint,
            SecretString::from("local-fixture-key"),
            "configured-other-model",
            None,
            None,
            5,
        )
        .unwrap();
        let mut request =
            CompletionRequest::new(vec![ChatMessage::user("hello")]).with_model("selected-model");
        request.dispatch_policy = crate::ProviderDispatchPolicy::SingleSubmission;
        let error = provider.complete(request).await.unwrap_err();
        assert!(matches!(error, LlmError::AuthFailed { .. }));
        assert!(!format!("{error} {error:?}").contains("private-codex"));
        let requests = observations.lock().await;
        assert_eq!(
            requests.len(),
            1,
            "no model discovery or fallback submission"
        );
        assert!(requests[0].starts_with(b"POST /responses "));
        let end = requests[0]
            .windows(4)
            .position(|data| data == b"\r\n\r\n")
            .unwrap();
        let body: Value = serde_json::from_slice(&requests[0][end + 4..]).unwrap();
        assert_eq!(body["model"], "selected-model");
        assert!(body.get("dispatch_policy").is_none());
        drop(requests);
        stop.send(true).unwrap();
        server.await.unwrap();
    }

    #[test]
    fn test_message_conversion_user() {
        let items = CodexChatGptProvider::message_to_input_items(&ChatMessage::user("hello"));
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["type"], "message");
        assert_eq!(items[0]["role"], "user");
        assert_eq!(items[0]["content"][0]["type"], "input_text");
        assert_eq!(items[0]["content"][0]["text"], "hello");
    }

    #[test]
    fn test_message_conversion_user_with_image() {
        use super::super::provider::ImageUrl;
        let parts = vec![
            ContentPart::Text {
                text: "What's in this image?".to_string(),
            },
            ContentPart::ImageUrl {
                image_url: ImageUrl {
                    url: "data:image/png;base64,iVBOR...".to_string(),
                    detail: None,
                },
            },
        ];
        let msg = ChatMessage::user_with_parts("", parts);
        let items = CodexChatGptProvider::message_to_input_items(&msg);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["type"], "message");
        assert_eq!(items[0]["role"], "user");
        let content = items[0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
        assert_eq!(content[0]["type"], "input_text");
        assert_eq!(content[0]["text"], "What's in this image?");
        assert_eq!(content[1]["type"], "input_image");
        assert_eq!(content[1]["image_url"], "data:image/png;base64,iVBOR...");
    }
    #[test]
    fn test_message_conversion_assistant() {
        let items = CodexChatGptProvider::message_to_input_items(&ChatMessage::assistant("hi"));
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["type"], "message");
        assert_eq!(items[0]["role"], "assistant");
        assert_eq!(items[0]["content"][0]["type"], "output_text");
    }

    #[test]
    fn test_message_conversion_tool_result() {
        let msg = ChatMessage::tool_result("call_1", "search", "result text");
        let items = CodexChatGptProvider::message_to_input_items(&msg);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["type"], "function_call_output");
        assert_eq!(items[0]["call_id"], "call_1");
        assert_eq!(items[0]["output"], "result text");
    }

    #[test]
    fn test_message_conversion_assistant_with_tool_calls() {
        let tc = ToolCall {
            id: "call_1".to_string(),
            name: "search".to_string(),
            arguments: json!({"query": "rust"}),
            reasoning: None,
            signature: None,
        };
        let msg = ChatMessage::assistant_with_tool_calls(Some("thinking...".into()), vec![tc]);
        let items = CodexChatGptProvider::message_to_input_items(&msg);
        // Should produce: 1 text message + 1 function_call
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["type"], "message");
        assert_eq!(items[1]["type"], "function_call");
        assert_eq!(items[1]["name"], "search");
        assert_eq!(items[1]["call_id"], "call_1");
    }

    #[test]
    fn test_message_conversion_sanitizes_tool_call_name() {
        let tc = ToolCall {
            id: "call_1".to_string(),
            name: "builtin.echo".to_string(),
            arguments: json!({"input": "hello"}),
            reasoning: None,
            signature: None,
        };
        let msg = ChatMessage::assistant_with_tool_calls(None, vec![tc]);
        let items = CodexChatGptProvider::message_to_input_items(&msg);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["type"], "function_call");
        assert_eq!(items[0]["name"], "builtin_echo");
    }

    #[test]
    fn test_build_request_extracts_system_as_instructions() {
        let provider = CodexChatGptProvider::new("https://example.com", "key", "gpt-4o");
        let messages = vec![
            ChatMessage::system("You are helpful."),
            ChatMessage::user("hello"),
        ];
        let body = provider.build_request_body("gpt-4o", &messages, &[], None);
        assert_eq!(body["instructions"], "You are helpful.");
        // input should only contain the user message, not the system message
        assert_eq!(body["input"].as_array().unwrap().len(), 1);
        // store must be false for ChatGPT backend
        assert_eq!(body["store"], false);
    }

    #[test]
    fn test_build_request_sanitizes_tool_definition_names() {
        let provider = CodexChatGptProvider::new("https://example.com", "key", "gpt-4o");
        let messages = vec![ChatMessage::user("hello")];
        let tools = vec![ToolDefinition {
            name: "builtin.echo".to_string(),
            description: "Echo input".to_string(),
            parameters: json!({"type": "object"}),
        }];
        let body = provider.build_request_body("gpt-4o", &messages, &tools, None);
        assert_eq!(body["tools"][0]["name"], "builtin_echo");
    }

    #[test]
    fn test_build_request_flattens_ref_tool_schema() {
        let provider = CodexChatGptProvider::new("https://example.com", "key", "gpt-4o");
        let messages = vec![ChatMessage::user("hello")];
        let tools = vec![ToolDefinition {
            name: "builtin__apply_patch".to_string(),
            description: "Apply a patch".to_string(),
            parameters: json!({"$ref": "schemas/builtin/apply-patch.input.v1.json"}),
        }];
        let body = provider.build_request_body("gpt-4o", &messages, &tools, None);
        assert_eq!(body["tools"][0]["parameters"]["type"], "object");
        assert!(body["tools"][0]["parameters"].get("properties").is_some());
    }

    #[test]
    fn test_parse_sse_text_response() {
        let sse = r#"event: response.output_text.delta
data: {"delta":"Hello"}

event: response.output_text.delta
data: {"delta":" world!"}

event: response.completed
data: {"response":{"usage":{"input_tokens":10,"output_tokens":5}}}

"#;
        let result = CodexChatGptProvider::parse_sse_response(sse).unwrap();
        assert_eq!(result.text, "Hello world!");
        assert_eq!(result.usage.unwrap().input_tokens, 10);
        assert_eq!(result.usage.unwrap().output_tokens, 5);
        assert!(result.pending_tool_calls.is_empty());
    }

    #[test]
    fn test_parse_sse_data_only_type_field_text_response() {
        let sse = r#"data: {"type":"response.output_text.delta","delta":"Hello"}

data: {"type":"response.output_text.delta","delta":" world!"}

data: {"type":"response.completed","response":{"usage":{"input_tokens":10,"output_tokens":5}}}

"#;
        let result = CodexChatGptProvider::parse_sse_response(sse).unwrap();
        assert_eq!(result.text, "Hello world!");
        assert_eq!(result.usage.unwrap().input_tokens, 10);
        assert_eq!(result.usage.unwrap().output_tokens, 5);
        assert!(result.pending_tool_calls.is_empty());
    }

    #[test]
    fn test_parse_sse_output_text_done_without_delta() {
        let sse = r#"data: {"type":"response.output_text.done","text":"Hello from done."}

data: {"type":"response.completed","response":{"usage":{"input_tokens":10,"output_tokens":5}}}

"#;
        let result = CodexChatGptProvider::parse_sse_response(sse).unwrap();
        assert_eq!(result.text, "Hello from done.");
        assert_eq!(result.usage.unwrap().input_tokens, 10);
        assert_eq!(result.usage.unwrap().output_tokens, 5);
        assert!(result.pending_tool_calls.is_empty());
    }

    #[test]
    fn sse_terminal_status_is_required_and_incompletion_does_not_promote_tool_proposals() {
        let initial = "data: {\"type\":\"response.output_text.delta\",\"delta\":\"partial\"}\n\ndata: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"function_call\",\"id\":\"item1\",\"call_id\":\"call1\",\"name\":\"search\"}}\n\n";
        for (terminal, expected) in [
            ("data: [DONE]\n\n", FinishReason::Unknown),
            (
                "data: {\"type\":\"response.incomplete\",\"response\":{\"status\":\"incomplete\",\"incomplete_details\":{\"reason\":\"max_output_tokens\"},\"usage\":{\"input_tokens\":8,\"output_tokens\":1}}}\n\n",
                FinishReason::Length,
            ),
            (
                "data: {\"type\":\"response.incomplete\",\"response\":{\"status\":\"incomplete\",\"incomplete_details\":{\"reason\":\"content_filter\"}}}\n\n",
                FinishReason::ContentFilter,
            ),
            (
                "data: {\"type\":\"response.failed\",\"response\":{\"status\":\"failed\"}}\n\n",
                FinishReason::Unknown,
            ),
        ] {
            let result =
                CodexChatGptProvider::parse_sse_response(&format!("{initial}{terminal}")).unwrap();
            assert_eq!(result.text, "partial");
            assert_eq!(result.pending_tool_calls.len(), 1);
            assert_eq!(
                FinishReason::from_responses_status(
                    result.status.as_deref(),
                    result.incomplete_reason.as_deref(),
                    true
                ),
                expected
            );
            if expected == FinishReason::Length {
                assert_eq!(result.usage.unwrap().output_tokens, 1);
            }
        }
    }

    #[test]
    fn test_parse_sse_completed_response_output_text_without_deltas() {
        let sse = r#"data: {"type":"response.completed","response":{"output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Hello from final output."}]}],"usage":{"input_tokens":10,"output_tokens":5}}}

"#;
        let result = CodexChatGptProvider::parse_sse_response(sse).unwrap();
        assert_eq!(result.text, "Hello from final output.");
        assert_eq!(result.usage.unwrap().input_tokens, 10);
        assert_eq!(result.usage.unwrap().output_tokens, 5);
        assert!(result.pending_tool_calls.is_empty());
    }

    #[test]
    fn test_parse_sse_completed_response_prefers_output_text_over_message_fallback() {
        let sse = r#"data: {"type":"response.completed","response":{"output_text":"Hello from output_text.","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Duplicate fallback text."}]}],"usage":{"input_tokens":10,"output_tokens":5}}}

"#;
        let result = CodexChatGptProvider::parse_sse_response(sse).unwrap();
        assert_eq!(result.text, "Hello from output_text.");
        assert_eq!(result.usage.unwrap().input_tokens, 10);
        assert_eq!(result.usage.unwrap().output_tokens, 5);
        assert!(result.pending_tool_calls.is_empty());
    }

    #[test]
    fn test_parse_sse_reasoning_summary_response() {
        let sse = r#"event: response.reasoning_summary_text.delta
data: {"delta":"Thinking Steps\n"}

event: response.reasoning_summary_text.delta
data: {"delta":"[] Inspect context."}

event: response.output_text.delta
data: {"delta":"Done."}

event: response.completed
data: {"response":{"usage":{"input_tokens":10,"output_tokens":5}}}

"#;
        let result = CodexChatGptProvider::parse_sse_response(sse).unwrap();
        assert_eq!(result.text, "Done.");
        assert_eq!(
            crate::responses_reasoning::finish_summary(result.reasoning).as_deref(),
            Some("Thinking Steps\n[] Inspect context.")
        );
    }

    #[test]
    fn test_parse_sse_tool_call() {
        // Real API format: output_item.added has item.id (item_id) + item.call_id,
        // delta events use item_id (not call_id)
        let sse = r#"event: response.output_item.added
data: {"item":{"id":"fc_1","type":"function_call","call_id":"call_1","name":"search"}}

event: response.function_call_arguments.delta
data: {"item_id":"fc_1","delta":"{\"query\":"}

event: response.function_call_arguments.delta
data: {"item_id":"fc_1","delta":"\"rust\"}"}

event: response.completed
data: {"response":{"usage":{"input_tokens":20,"output_tokens":15}}}

"#;
        let result = CodexChatGptProvider::parse_sse_response(sse).unwrap();
        assert!(result.text.is_empty());
        assert_eq!(result.pending_tool_calls.len(), 1);
        let tc = result.pending_tool_calls.get("fc_1").unwrap();
        assert_eq!(tc.call_id, "call_1");
        assert_eq!(tc.name, "search");
        assert_eq!(tc.arguments, "{\"query\":\"rust\"}");
    }

    #[test]
    fn test_parse_sse_tool_call_done_events() {
        let sse = r#"event: response.output_item.added
data: {"item":{"id":"fc_1","type":"function_call","call_id":"call_1","name":"search"}}

event: response.function_call_arguments.done
data: {"item_id":"fc_1","arguments":"{\"query\":\"rust\"}"}

event: response.output_item.done
data: {"item":{"id":"fc_1","type":"function_call","call_id":"call_1","name":"search"}}

event: response.completed
data: {"response":{"usage":{"input_tokens":20,"output_tokens":15}}}

"#;
        let result = CodexChatGptProvider::parse_sse_response(sse).unwrap();
        assert!(result.text.is_empty());
        assert_eq!(result.pending_tool_calls.len(), 1);
        let tc = result.pending_tool_calls.get("fc_1").unwrap();
        assert_eq!(tc.call_id, "call_1");
        assert_eq!(tc.name, "search");
        assert_eq!(tc.arguments, "{\"query\":\"rust\"}");
    }

    #[test]
    fn test_parse_sse_completed_response_output_tool_call_without_deltas() {
        let sse = r#"data: {"type":"response.completed","response":{"output":[{"type":"function_call","id":"fc_1","call_id":"call_1","name":"search","arguments":"{\"query\":\"rust\"}"}],"usage":{"input_tokens":20,"output_tokens":15}}}

"#;
        let result = CodexChatGptProvider::parse_sse_response(sse).unwrap();
        assert!(result.text.is_empty());
        assert_eq!(result.pending_tool_calls.len(), 1);
        let tc = result.pending_tool_calls.get("fc_1").unwrap();
        assert_eq!(tc.call_id, "call_1");
        assert_eq!(tc.name, "search");
        assert_eq!(tc.arguments, "{\"query\":\"rust\"}");
    }

    #[tokio::test]
    async fn test_parse_sse_stream_response() {
        let stream = stream::iter(vec![
            Ok(Bytes::from_static(
                b"event: response.output_text.delta\ndata: {\"delta\":\"Hello\"}\n\n",
            )),
            Ok(Bytes::from_static(
                b"event: response.output_text.delta\ndata: {\"delta\":\" world\"}\n\n",
            )),
            Ok(Bytes::from_static(
                b"event: response.completed\ndata: {\"response\":{\"usage\":{\"input_tokens\":3,\"output_tokens\":2}}}\n\n",
            )),
        ]);

        let result = CodexChatGptProvider::parse_sse_stream(stream, Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(result.text, "Hello world");
        assert_eq!(result.usage.unwrap().input_tokens, 3);
        assert_eq!(result.usage.unwrap().output_tokens, 2);
    }

    #[tokio::test]
    async fn test_parse_sse_stream_data_only_type_field_response() {
        let stream = stream::iter(vec![
            Ok(Bytes::from_static(
                b"data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hello\"}\n\n",
            )),
            Ok(Bytes::from_static(
                b"data: {\"type\":\"response.output_text.delta\",\"delta\":\" world\"}\n\n",
            )),
            Ok(Bytes::from_static(
                b"data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":3,\"output_tokens\":2}}}\n\n",
            )),
        ]);

        let result = CodexChatGptProvider::parse_sse_stream(stream, Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(result.text, "Hello world");
        assert_eq!(result.usage.unwrap().input_tokens, 3);
        assert_eq!(result.usage.unwrap().output_tokens, 2);
    }

    #[tokio::test]
    async fn test_parse_sse_done_marker_stops_parsing() {
        let sse = r#"data: {"type":"response.output_text.delta","delta":"hello"}

data: [DONE]

data: {"type":"response.output_text.delta","delta":" ignored"}

"#;
        let result = CodexChatGptProvider::parse_sse_response(sse).unwrap();
        assert_eq!(result.text, "hello");

        let stream = stream::iter(vec![
            Ok(Bytes::from_static(
                b"data: {\"type\":\"response.output_text.delta\",\"delta\":\"hello\"}\n\n",
            )),
            Ok(Bytes::from_static(b"data: [DONE]\n\n")),
            Ok(Bytes::from_static(
                b"data: {\"type\":\"response.output_text.delta\",\"delta\":\" ignored\"}\n\n",
            )),
        ]);
        let result = CodexChatGptProvider::parse_sse_stream(stream, Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(result.text, "hello");
    }

    #[tokio::test]
    async fn complete_with_tools_remaps_sanitized_response_tool_names() {
        let base_url = responses_api_test_server::spawn(
            r#"event: response.output_item.added
data: {"item":{"id":"fc_1","type":"function_call","call_id":"call_1","name":"builtin_echo"}}

event: response.function_call_arguments.delta
data: {"item_id":"fc_1","delta":"{\"message\":\"hello\"}"}

event: response.completed
data: {"response":{"usage":{"input_tokens":3,"output_tokens":2}}}

"#,
        )
        .await;
        let provider = CodexChatGptProvider::new(&base_url, "test-key", "gpt-4o");
        let request = ToolCompletionRequest::new(
            vec![ChatMessage::user("use echo")],
            vec![ToolDefinition {
                name: "builtin.echo".to_string(),
                description: "Echo input".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "message": {"type": "string"}
                    }
                }),
            }],
        );

        let response = provider
            .complete_with_tools(request)
            .await
            .expect("tool completion succeeds");

        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name, "builtin.echo");
        assert_eq!(
            response.tool_calls[0].arguments,
            json!({"message": "hello"})
        );
    }

    #[tokio::test]
    async fn complete_with_tools_accepts_data_only_text_response() {
        let base_url = responses_api_test_server::spawn(
            r#"data: {"type":"response.output_text.delta","delta":"Hello from data-only SSE."}

data: {"type":"response.completed","response":{"usage":{"input_tokens":3,"output_tokens":2}}}

"#,
        )
        .await;
        let provider = CodexChatGptProvider::new(&base_url, "test-key", "gpt-4o");
        let request = ToolCompletionRequest::new(
            vec![ChatMessage::user("hello")],
            vec![ToolDefinition {
                name: "builtin.echo".to_string(),
                description: "Echo input".to_string(),
                parameters: json!({"type": "object"}),
            }],
        );

        let response = provider
            .complete_with_tools(request)
            .await
            .expect("tool-capable completion succeeds");

        assert_eq!(
            response.content.as_deref(),
            Some("Hello from data-only SSE.")
        );
        assert!(response.tool_calls.is_empty());
        assert_eq!(response.finish_reason, FinishReason::Stop);
    }

    #[tokio::test]
    async fn complete_with_tools_preserves_empty_and_null_argument_values_and_rejects_malformed_arguments()
     {
        for arguments in [
            r#"{"empty":"","nothing":null,"nested":{"literal":""}}"#,
            "{incomplete",
        ] {
            let response = serde_json::json!({"type":"response.completed","response":{"status":"completed","output":[{"type":"function_call","id":"item1","call_id":"call1","name":"builtin_echo","arguments":arguments}],"usage":{"input_tokens":5,"output_tokens":2}}});
            let sse = format!("data: {response}\n\n");
            let base_url = responses_api_test_server::spawn(sse).await;
            let provider = CodexChatGptProvider::new(&base_url, "test-key", "gpt-4o");
            let result = provider
                .complete_with_tools(ToolCompletionRequest::new(
                    vec![ChatMessage::user("use literal arguments")],
                    vec![ToolDefinition {
                        name: "builtin.echo".into(),
                        description: "Echo data".into(),
                        parameters: json!({"type":"object"}),
                    }],
                ))
                .await
                .unwrap();
            assert_eq!(result.tool_calls.len(), 1);
            assert_eq!(result.tool_calls[0].name, "builtin.echo");
            if let Ok(expected) = serde_json::from_str::<Value>(arguments) {
                assert_eq!(result.finish_reason, FinishReason::ToolUse);
                assert_eq!(result.tool_calls[0].arguments, expected);
            } else {
                assert_eq!(result.finish_reason, FinishReason::Unknown);
                assert_eq!(
                    result.tool_calls[0].arguments,
                    Value::String(arguments.into())
                );
            }
        }
    }

    #[tokio::test]
    async fn complete_with_tools_rejects_colliding_sanitized_tool_names_before_request() {
        let provider = CodexChatGptProvider::new("http://127.0.0.1:9", "test-key", "gpt-4o");
        let request = ToolCompletionRequest::new(
            vec![ChatMessage::user("use a tool")],
            vec![
                ToolDefinition {
                    name: "foo.bar".to_string(),
                    description: "First tool".to_string(),
                    parameters: json!({"type": "object"}),
                },
                ToolDefinition {
                    name: "foo_bar".to_string(),
                    description: "Second tool".to_string(),
                    parameters: json!({"type": "object"}),
                },
            ],
        );

        let error = provider
            .complete_with_tools(request)
            .await
            .expect_err("colliding provider tool names must fail closed");

        assert!(
            error
                .to_string()
                .contains("both map to provider name `foo_bar`"),
            "unexpected error: {error}"
        );
    }

    mod responses_api_test_server {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        /// Tiny in-process Responses API fixture for caller-level provider tests.
        /// The test must exercise model resolution plus `/responses`, so a real
        /// loopback HTTP boundary is intentional here.
        pub(super) async fn spawn(sse_body: impl Into<String>) -> String {
            let sse_body = sse_body.into();
            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind test server");
            let addr = listener.local_addr().expect("local addr");
            tokio::spawn(async move {
                for _ in 0..2 {
                    let (mut socket, _) = listener.accept().await.expect("accept request");
                    // 4096-byte buffer for reading the raw HTTP/1.1 request
                    // from the test socket — sufficient for all fixture requests.
                    const REQUEST_READ_BUFFER_BYTES: usize = 4096;
                    let mut request = [0u8; REQUEST_READ_BUFFER_BYTES];
                    let bytes_read = socket.read(&mut request).await.expect("read request");
                    let request = String::from_utf8_lossy(&request[..bytes_read]);
                    if request.starts_with("GET /models") {
                        write_response(
                            &mut socket,
                            "application/json",
                            r#"{"models":[{"slug":"gpt-4o"}]}"#,
                        )
                        .await;
                    } else if request.starts_with("POST /responses") {
                        write_response(&mut socket, "text/event-stream", &sse_body).await;
                    } else {
                        write_response(&mut socket, "text/plain", "not found").await;
                    }
                }
            });
            format!("http://{addr}")
        }

        async fn write_response(
            socket: &mut tokio::net::TcpStream,
            content_type: &str,
            body: &str,
        ) {
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("write response");
        }
    }
}
