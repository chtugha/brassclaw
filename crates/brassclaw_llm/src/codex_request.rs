//! The native Codex Responses request codec and closed transport contract.
//! No sessions, credentials, network sends, model measurements or approvals.
//! API wire observations are distinct from effective server encoding/output.
use crate::LlmError;
use crate::final_request_guard::FinalRequestError;
use crate::provider::{ChatMessage, ContentPart, Role, ToolDefinition};
use reqwest::header::{
    ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, USER_AGENT,
};
use std::collections::{BTreeMap, BTreeSet};

pub const TRANSPORT: &str = "brassclaw-native-codex-responses/1";

/// Resolve exactly the original dedicated Codex base, without Chat /v1 rewriting.
/// No credentials, query or fragment can hide inside an accepted endpoint.
pub fn endpoint(base: &str) -> Result<reqwest::Url, FinalRequestError> {
    if base.is_empty() || base.trim() != base || base.chars().any(char::is_control) {
        return Err(FinalRequestError::Target);
    }
    let parsed = reqwest::Url::parse(base).map_err(|_| FinalRequestError::Target)?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err(FinalRequestError::Target);
    }
    // Parse before concatenation: a trailing slash must never erase a query.
    reqwest::Url::parse(&format!("{}/responses", base.trim_end_matches('/')))
        .map_err(|_| FinalRequestError::Target)
}

/// Actual nonsecret headers used by production and no-send observations.
/// Registry HTTP construction supplies no custom default-header configuration;
/// explicit Accept overrides reqwest's implicit */*. Host/length are HTTP framing.
pub(crate) fn technical_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("openai-beta"),
        HeaderValue::from_static("responses=experimental"),
    );
    headers.insert(
        HeaderName::from_static("originator"),
        HeaderValue::from_static("brassclaw"),
    );
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(concat!("brassclaw/", env!("CARGO_PKG_VERSION"))),
    );
    headers.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers
}

fn verify_fixed_headers(headers: &HeaderMap) -> Result<(), FinalRequestError> {
    if technical_headers().iter().any(|(name, value)| {
        headers.get_all(name).iter().count() != 1 || headers.get(name) != Some(value)
    }) {
        return Err(FinalRequestError::Binding);
    }
    Ok(())
}

#[cfg(feature = "registry-provider-factory")]
pub(crate) fn verify_technical_headers(headers: &HeaderMap) -> Result<(), FinalRequestError> {
    verify_fixed_headers(headers)?;
    if headers.len() != technical_headers().len() {
        return Err(FinalRequestError::Binding);
    }
    Ok(())
}

/// Verify the closed production header profile. Credential ownership/freshness
/// is independently rechecked by the existing native host-session observer.
/// Sensitive values are inspected in-place and never returned or logged.
pub fn verify_runtime_headers(headers: &HeaderMap) -> Result<(), FinalRequestError> {
    verify_fixed_headers(headers)?;
    let account = HeaderName::from_static("chatgpt-account-id");
    if headers.len() != technical_headers().len() + 2
        || [&AUTHORIZATION, &account].iter().any(|name| {
            let mut values = headers.get_all(*name).iter();
            !matches!((values.next(), values.next()), (Some(value), None)
                if value.is_sensitive() && !value.as_bytes().is_empty())
        })
        || !headers.get(AUTHORIZATION).is_some_and(|value| {
            value.as_bytes().starts_with(b"Bearer ") && value.as_bytes().len() > 7
        })
    {
        return Err(FinalRequestError::Binding);
    }
    Ok(())
}

/// Same bounded, no-redirect/retry client contract as production. This identity
/// covers the real codec, provider hook/defaults, tool/reasoning shaping and SDK.
/// It is evidence identity only, never a Prefix acceptance or count.
pub fn contract_checksum() -> String {
    use sha2::Digest;
    let mut hash = sha2::Sha256::new();
    for source in [
        include_str!("codex_request.rs"),
        include_str!("openai_codex_provider.rs"),
        include_str!("final_request_transport.rs"),
        include_str!("request_qualification.rs"),
        include_str!("lib.rs"),
        include_str!("final_request_guard.rs"),
        include_str!("provider.rs"),
        include_str!("provider_observation.rs"),
        include_str!("config.rs"),
        include_str!("resolution.rs"),
        include_str!("token_refreshing.rs"),
        include_str!("codex_host_session.rs"),
        include_str!("tool_schema.rs"),
        include_str!("reasoning_models.rs"),
        include_str!("responses_reasoning.rs"),
        include_str!("../Cargo.toml"),
        include_str!("../../../Cargo.lock"),
    ] {
        hash.update((source.len() as u64).to_be_bytes());
        hash.update(source.as_bytes());
    }
    format!("{:x}", hash.finalize())
}

fn unsupported() -> LlmError {
    LlmError::FinalRequestRejected {
        reason: FinalRequestError::Body,
    }
}

/// max_tokens remains an original caller setting, NOT an effective wire output
/// bound: this native protocol currently serializes no output limit. Qualification
/// of actual output/encoding semantics remains mandatory in the Prefix owner.
pub(crate) fn validate_options(
    model: Option<&str>,
    selected: &str,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    stop: Option<&[String]>,
    tool_choice: Option<&str>,
) -> Result<(), LlmError> {
    if selected.is_empty()
        || selected.trim() != selected
        || selected.chars().any(char::is_control)
        || model.is_some_and(|model| model != selected)
        || max_tokens == Some(0)
        || temperature.is_some()
        || stop.is_some()
        || tool_choice.is_some_and(|choice| choice != "auto")
    {
        return Err(unsupported());
    }
    Ok(())
}

// Reject profiles this codec cannot represent rather than silently dropping
// images, foreign reasoning/signatures, Tool IDs or colliding sanitized names.
fn validate_messages(
    messages: &[ChatMessage],
    tools: Option<&[ToolDefinition]>,
) -> Result<(), LlmError> {
    let mut names = BTreeMap::new();
    let mut calls = BTreeMap::new();
    let mut results = BTreeSet::new();
    if let Some(tools) = tools {
        for tool in tools {
            let name = sanitize_tool_name(&tool.name);
            if name.is_empty() || names.insert(name, tool.name.as_str()).is_some() {
                return Err(unsupported());
            }
        }
    }
    for message in messages {
        if message.reasoning.is_some()
            || (message.role != Role::Tool && message.name.is_some())
            || message
                .content_parts
                .iter()
                .any(|part| matches!(part, ContentPart::ImageUrl { .. }))
            || (!message.content_parts.is_empty()
                && !matches!(message.role, Role::User | Role::Assistant))
            || (message.role != Role::Assistant && message.tool_calls.is_some())
            || (message.role != Role::Tool && message.tool_call_id.is_some())
        {
            return Err(unsupported());
        }
        if let Some(tool_calls) = &message.tool_calls {
            if tool_calls.is_empty() {
                return Err(unsupported());
            }
            for call in tool_calls {
                let name = sanitize_tool_name(&call.name);
                if call.id.trim().is_empty()
                    || name.is_empty()
                    || call.reasoning.is_some()
                    || call.signature.is_some()
                    || calls.insert(call.id.as_str(), call.name.as_str()).is_some()
                    || names
                        .get(&name)
                        .is_some_and(|original| *original != call.name.as_str())
                {
                    return Err(unsupported());
                }
                if let Some(original) = names.insert(name, call.name.as_str())
                    && original != call.name.as_str()
                {
                    return Err(unsupported());
                }
            }
        }
        if message.role == Role::Tool {
            let id = message.tool_call_id.as_deref().ok_or_else(unsupported)?;
            let original = calls.get(id).ok_or_else(unsupported)?;
            if id.trim().is_empty()
                || !results.insert(id)
                || message
                    .name
                    .as_deref()
                    .is_some_and(|name| name != *original)
            {
                return Err(unsupported());
            }
        }
    }
    Ok(())
}

fn text_parts(message: &ChatMessage, kind: &str) -> Vec<serde_json::Value> {
    let part = |text: &str| {
        let mut value = serde_json::json!({"type":kind,"text":text});
        if kind == "output_text" {
            value["annotations"] = serde_json::json!([]);
        }
        value
    };
    let mut parts = vec![part(&message.content)];
    parts.extend(
        message
            .content_parts
            .iter()
            .filter_map(|content| match content {
                ContentPart::Text { text } => Some(part(text)),
                // Rejected before conversion by validate_messages.
                ContentPart::ImageUrl { .. } => None,
            }),
    );
    parts
}

/// Build the actual request once. Only production passes authenticated headers;
/// the no-send observer consumes and discards its unauthenticated Request.
pub(crate) fn build_request(
    client: &reqwest::Client,
    endpoint: &reqwest::Url,
    body: &serde_json::Value,
    headers: HeaderMap,
) -> Result<reqwest::Request, LlmError> {
    client
        .post(endpoint.clone())
        .headers(headers)
        .json(body)
        .build()
        .map_err(|error| LlmError::Http(error.without_url()))
}

pub(crate) fn body(
    model: &str,
    messages: &[ChatMessage],
    tools: Option<&[ToolDefinition]>,
) -> Result<serde_json::Value, LlmError> {
    validate_messages(messages, tools)?;
    // Separate system messages into `instructions`
    let instructions: String = messages
        .iter()
        .filter(|m| m.role == Role::System)
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");

    // Convert non-system messages to Responses API format
    let input: Vec<serde_json::Value> = messages
        .iter()
        .filter(|m| m.role != Role::System)
        .enumerate()
        .flat_map(|(i, m)| convert_message(m, i))
        .collect();

    let mut body = serde_json::json!({
        "model": model,
        "store": false,
        "stream": true,
        "input": input,
        "text": { "verbosity": "medium" },
    });

    if crate::reasoning_models::supports_openai_reasoning(model) {
        body["reasoning"] = crate::responses_reasoning::summary_request();
        body["include"] = serde_json::json!(["reasoning.encrypted_content"]);
    }

    if messages.iter().any(|m| m.role == Role::System) {
        body["instructions"] = serde_json::Value::String(instructions);
    }

    if let Some(tools) = tools
        && !tools.is_empty()
    {
        let tools_json: Vec<serde_json::Value> =
            tools.iter().map(convert_tool_definition).collect();
        body["tools"] = serde_json::Value::Array(tools_json);
        body["tool_choice"] = serde_json::Value::String("auto".to_string());
        body["parallel_tool_calls"] = serde_json::Value::Bool(true);
    }

    Ok(body)
}

/// Convert a single `ChatMessage` to Responses API `input` items.
///
/// Returns a Vec because assistant messages with tool_calls produce
/// one `function_call` item per tool call.
pub(crate) fn convert_message(msg: &ChatMessage, index: usize) -> Vec<serde_json::Value> {
    match msg.role {
        Role::System => {
            // System messages are handled separately as `instructions`
            vec![]
        }
        Role::User => {
            vec![serde_json::json!({
                "role": "user",
                "content": text_parts(msg, "input_text"),
            })]
        }
        Role::Assistant => {
            let mut items = Vec::new();
            if !msg.content.is_empty() || !msg.content_parts.is_empty() || msg.tool_calls.is_none()
            {
                items.push(serde_json::json!({
                    "type": "message", "role": "assistant", "id": format!("msg_{index}"),
                    "status": "completed", "content": text_parts(msg, "output_text"),
                }));
            }
            if let Some(tool_calls) = &msg.tool_calls {
                items.extend(tool_calls.iter().map(|tc| {
                    let arguments = tc
                        .arguments
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| tc.arguments.to_string());
                    serde_json::json!({"type":"function_call", "call_id":tc.id,
                        "name":sanitize_tool_name(&tc.name), "arguments":arguments})
                }));
            }
            items
        }
        Role::Tool => {
            // The shared preflight rejects absent/empty/unmatched result IDs.
            let call_id = &msg.tool_call_id;
            vec![serde_json::json!({
                "type": "function_call_output",
                "call_id": call_id,
                "output": msg.content,
            })]
        }
    }
}

/// Sanitize a tool name to match the OpenAI Responses API pattern `^[a-zA-Z0-9_-]+$`.
/// Replaces any invalid character (e.g. dots in MCP tool names) with underscores.
pub(crate) fn sanitize_tool_name(name: &str) -> String {
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

/// Convert a `ToolDefinition` to Responses API tool format.
///
/// Applies the shared `tool_schema.rs` shaping entrypoint with the
/// `StrictOpenAi` policy, which performs strict-mode object normalization and
/// the top-level union flatten that the Responses API requires. The flatten
/// can append a hint to the tool description, so we pass an owned clone
/// through and read it back.
pub(crate) fn convert_tool_definition(tool: &ToolDefinition) -> serde_json::Value {
    use crate::tool_schema::{ToolSchemaPolicy, shape_tool_schema};

    let mut description = tool.description.clone();
    let parameters = shape_tool_schema(
        ToolSchemaPolicy::StrictOpenAi,
        &tool.parameters,
        &mut description,
    );

    serde_json::json!({
        "type": "function",
        "name": sanitize_tool_name(&tool.name),
        "description": description,
        "parameters": parameters,
    })
}

#[cfg(all(test, feature = "registry-provider-factory"))]
#[path = "codex_request_tests.rs"]
mod tests;
