//! Versioned, explicit interpretation of final OpenAI text-chat JSON.
//! This is a mechanical projection, not approval that a provider uses it.
//! Qualification binds this exact contract to the target's actual defaults.
use crate::{EncodingError, checksum, context};
use indexmap::IndexMap;
use minijinja::{Value, value::ValueKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const WIRE_MAPPING: &str = "openai-text-chat-context/1";
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireMapping {
    pub format: String,
    /// Exact ordered explicit defaults from the selected model design. No
    /// library/server/model-name guess supplies a missing generation flag.
    pub render_defaults_json: String,
    pub missing_tools: MissingTools,
    pub null_assistant_content: NullContent,
    pub tool_arguments: ToolArguments,
    pub text_parts: TextParts,
    pub forward_tool_choice: bool,
    pub forward_response_format: bool,
    pub allowed_tool_choices: BTreeSet<String>,
    pub allowed_chat_kwargs: BTreeSet<String>,
    pub allowed_message_fields: BTreeSet<String>,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingTools {
    Omit,
    Null,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NullContent {
    Preserve,
    EmptyString,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolArguments {
    Preserve,
    DecodeJson,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum TextParts {
    Reject,
    Preserve,
    Join { separator: String },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireInputObservation {
    pub request_sha256: String,
    pub mapping_sha256: String,
    pub model: String,
    pub reserved_output_tokens: u32,
    pub render_context_sha256: String,
}

fn invalid<T>() -> Result<T, EncodingError> {
    Err(EncodingError::InvalidInput)
}
fn object(value: &Value) -> Result<IndexMap<String, Value>, EncodingError> {
    if value.kind() != ValueKind::Map {
        return invalid();
    }
    value
        .try_iter()
        .map_err(|_| EncodingError::InvalidInput)?
        .map(|key| {
            let name = key.as_str().ok_or(EncodingError::InvalidInput)?.to_owned();
            Ok((
                name,
                value
                    .get_item(&key)
                    .map_err(|_| EncodingError::InvalidInput)?,
            ))
        })
        .collect()
}
fn value(map: IndexMap<String, Value>) -> Value {
    Value::from_object(
        map.into_iter()
            .map(|(k, v)| (Value::from(k), v))
            .collect::<IndexMap<_, _>>(),
    )
}
fn positive_output(value: &Value) -> Result<u32, EncodingError> {
    // Deserialize the original numeric kind, not a float/bool/string cast.
    let bytes = serde_json::to_vec(value).map_err(|_| EncodingError::InvalidInput)?;
    serde_json::from_slice::<u32>(&bytes)
        .ok()
        .filter(|v| *v > 0)
        .ok_or(EncodingError::InvalidInput)
}
fn reserved(name: &str) -> bool {
    matches!(
        name,
        "messages"
            | "tools"
            | "model"
            | "tool_choice"
            | "response_format"
            | "bos_token"
            | "eos_token"
            | "pad_token"
            | "unk_token"
            | "cls_token"
            | "sep_token"
            | "mask_token"
            | "chat_template"
            | "add_special_tokens"
            | "tokenizer"
            | "functions"
            | "documents"
            | "raise_exception"
            | "strftime_now"
            | "dict"
            | "range"
            | "namespace"
    )
}
fn normalize_text_parts(content: &Value, rules: &TextParts) -> Result<Value, EncodingError> {
    if matches!(rules, TextParts::Reject) {
        return invalid();
    }
    let mut text = Vec::new();
    for i in 0..content.len().ok_or(EncodingError::InvalidInput)? {
        let part = object(
            &content
                .get_item(&Value::from(i))
                .map_err(|_| EncodingError::InvalidInput)?,
        )?;
        if part.len() != 2 || part.get("type").and_then(Value::as_str) != Some("text") {
            return invalid();
        }
        text.push(
            part.get("text")
                .and_then(Value::as_str)
                .ok_or(EncodingError::InvalidInput)?
                .to_owned(),
        );
    }
    match rules {
        TextParts::Preserve => Ok(content.clone()),
        TextParts::Join { separator } => {
            let total = text
                .iter()
                .try_fold(0usize, |n, s| n.checked_add(s.len()))
                .and_then(|n| {
                    separator
                        .len()
                        .checked_mul(text.len().saturating_sub(1))
                        .and_then(|extra| n.checked_add(extra))
                })
                .ok_or(EncodingError::Capacity)?;
            if total > crate::MAX_RENDER_BYTES {
                return Err(EncodingError::Capacity);
            }
            Ok(Value::from(text.join(separator)))
        }
        TextParts::Reject => invalid(),
    }
}
fn normalize_messages(messages: &Value, rules: &WireMapping) -> Result<Value, EncodingError> {
    if messages.kind() != ValueKind::Seq {
        return invalid();
    }
    let count = messages.len().ok_or(EncodingError::InvalidInput)?;
    if count == 0 {
        return invalid();
    }
    let mut output = Vec::with_capacity(count);
    for i in 0..count {
        let mut message = object(
            &messages
                .get_item(&Value::from(i))
                .map_err(|_| EncodingError::InvalidInput)?,
        )?;
        if message
            .keys()
            .any(|key| !rules.allowed_message_fields.contains(key))
        {
            return invalid();
        }
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .ok_or(EncodingError::InvalidInput)?
            .to_owned();
        if !matches!(
            role.as_str(),
            "system" | "developer" | "user" | "assistant" | "tool"
        ) {
            return invalid();
        }
        if let Some(content) = message
            .get("content")
            .filter(|v| v.kind() == ValueKind::Seq)
        {
            let normalized = normalize_text_parts(content, &rules.text_parts)?;
            message.insert("content".into(), normalized);
        }
        match message.get("content") {
            Some(content) if content.kind() == ValueKind::String => {}
            Some(content)
                if content.kind() == ValueKind::Seq
                    && matches!(rules.text_parts, TextParts::Preserve) => {}
            Some(content) if content.is_none() && role == "assistant" => {
                if matches!(rules.null_assistant_content, NullContent::EmptyString) {
                    message.insert("content".into(), Value::from(""));
                }
            }
            _ => return invalid(), // No placeholder accounting for media.
        }
        if let Some(calls) = message.get("tool_calls").cloned().filter(|v| !v.is_none()) {
            if role != "assistant" || calls.kind() != ValueKind::Seq {
                return invalid();
            }
            let mut normalized = Vec::new();
            for j in 0..calls.len().ok_or(EncodingError::InvalidInput)? {
                let mut call = object(
                    &calls
                        .get_item(&Value::from(j))
                        .map_err(|_| EncodingError::InvalidInput)?,
                )?;
                if call.get("type").and_then(Value::as_str) != Some("function") {
                    return invalid();
                }
                let mut function =
                    object(call.get("function").ok_or(EncodingError::InvalidInput)?)?;
                let arguments = function
                    .get("arguments")
                    .ok_or(EncodingError::InvalidInput)?;
                if matches!(rules.tool_arguments, ToolArguments::DecodeJson) {
                    let text = arguments.as_str().ok_or(EncodingError::InvalidInput)?;
                    let parsed = context::parse(text)?;
                    if parsed.kind() != ValueKind::Map {
                        return invalid();
                    }
                    function.insert("arguments".into(), parsed);
                }
                call.insert("function".into(), value(function));
                normalized.push(value(call));
            }
            message.insert("tool_calls".into(), Value::from(normalized));
        }
        output.push(value(message));
    }
    Ok(Value::from(output))
}

/// Preserve ordered nested JSON, including Tool-schema key order. Every known
/// non-prompt envelope field is deliberately excluded; unknown fields reject.
/// The worker, not the host orchestration thread, performs this bounded parse.
pub fn project(
    request_json: &str,
    mapping_json: &str,
    expected_model: &str,
) -> Result<(String, WireInputObservation), EncodingError> {
    let rules = parse_mapping(mapping_json)?;
    let body = object(&context::parse(request_json)?)?;
    const KEYS: &[&str] = &[
        "model",
        "messages",
        "tools",
        "tool_choice",
        "max_tokens",
        "max_completion_tokens",
        "temperature",
        "top_p",
        "frequency_penalty",
        "presence_penalty",
        "seed",
        "stop",
        "stream",
        "stream_options",
        "metadata",
        "parallel_tool_calls",
        "response_format",
        "reasoning_effort",
        "chat_template_kwargs",
        "cache_salt",
        "user",
        "logprobs",
        "top_logprobs",
        "n",
    ];
    if body.keys().any(|key| !KEYS.contains(&key.as_str()))
        || body.get("model").and_then(Value::as_str) != Some(expected_model)
        || body
            .get("stream")
            .is_some_and(|v| !matches!(bool::try_from(v.clone()), Ok(false)))
        || body
            .get("n")
            .is_some_and(|v| !matches!(positive_output(v), Ok(1)))
        || ["reasoning_effort", "parallel_tool_calls", "stream_options"]
            .iter()
            .any(|name| body.get(*name).is_some_and(|v| !v.is_none()))
    {
        return invalid();
    }
    let limits = [body.get("max_completion_tokens"), body.get("max_tokens")]
        .into_iter()
        .flatten()
        .filter(|v| !v.is_none())
        .map(positive_output)
        .collect::<Result<Vec<_>, _>>()?;
    let output = *limits.first().ok_or(EncodingError::InvalidInput)?;
    if limits.iter().any(|v| *v != output) {
        return invalid();
    }
    let mut ctx = defaults(&rules)?;
    ctx.insert(
        "messages".into(),
        normalize_messages(
            body.get("messages").ok_or(EncodingError::InvalidInput)?,
            &rules,
        )?,
    );
    let has_tools = body.get("tools").is_some_and(|v| !v.is_none());
    if let Some(tools) = body.get("tools").filter(|v| !v.is_none()) {
        if tools.kind() != ValueKind::Seq || tools.len() == Some(0) {
            return invalid();
        }
        ctx.insert("tools".into(), tools.clone());
    } else if matches!(rules.missing_tools, MissingTools::Null) {
        ctx.insert("tools".into(), Value::from(()));
    }
    let choice = body
        .get("tool_choice")
        .filter(|v| !v.is_none())
        .map(|v| v.as_str().ok_or(EncodingError::InvalidInput))
        .transpose()?;
    if !has_tools && choice.is_some_and(|name| name != "none") {
        return invalid();
    }
    let effective_choice = has_tools.then_some(choice.unwrap_or("auto"));
    if let Some(name) = effective_choice {
        if !rules.allowed_tool_choices.contains(name) {
            return invalid();
        }
        if rules.forward_tool_choice {
            ctx.insert("tool_choice".into(), Value::from(name));
        } else if name != "auto" {
            return invalid();
        }
    } else if rules.forward_tool_choice {
        ctx.insert("tool_choice".into(), Value::from(()));
    }
    if let Some(format) = body.get("response_format").filter(|v| !v.is_none()) {
        if !rules.forward_response_format || format.kind() != ValueKind::Map {
            return invalid();
        }
        ctx.insert("response_format".into(), format.clone());
    } else if rules.forward_response_format {
        ctx.insert("response_format".into(), Value::from(()));
    }
    if let Some(kwargs) = body.get("chat_template_kwargs").filter(|v| !v.is_none()) {
        for (name, value) in object(kwargs)? {
            if !rules.allowed_chat_kwargs.contains(&name)
                || reserved(&name)
                || matches!(
                    name.as_str(),
                    "add_generation_prompt" | "continue_final_message"
                )
            {
                return invalid();
            }
            ctx.insert(name, value);
        }
    }
    let context_json =
        serde_json::to_string(&value(ctx)).map_err(|_| EncodingError::InvalidInput)?;
    let observation = WireInputObservation {
        request_sha256: checksum(request_json.as_bytes()),
        mapping_sha256: checksum(mapping_json.as_bytes()),
        model: expected_model.to_owned(),
        reserved_output_tokens: output,
        render_context_sha256: checksum(context_json.as_bytes()),
    };
    Ok((context_json, observation))
}

fn parse_mapping(mapping_json: &str) -> Result<WireMapping, EncodingError> {
    let _ = context::parse(mapping_json)?; // Duplicate/capacity checks before serde.
    let rules: WireMapping =
        serde_json::from_str(mapping_json).map_err(|_| EncodingError::InvalidInput)?;
    if rules.format != WIRE_MAPPING
        || !rules.allowed_message_fields.contains("role")
        || !rules.allowed_message_fields.contains("content")
        || rules.allowed_chat_kwargs.iter().any(|key| {
            reserved(key)
                || matches!(
                    key.as_str(),
                    "add_generation_prompt" | "continue_final_message"
                )
        })
        || rules
            .allowed_tool_choices
            .iter()
            .any(|v| !matches!(v.as_str(), "auto" | "none" | "required"))
    {
        return invalid();
    }
    defaults(&rules)?;
    Ok(rules)
}

fn defaults(rules: &WireMapping) -> Result<IndexMap<String, Value>, EncodingError> {
    let ctx = object(&context::parse(&rules.render_defaults_json)?)?;
    if ctx.keys().any(|key| reserved(key))
        || ctx
            .get("add_generation_prompt")
            .is_none_or(|v| v.kind() != ValueKind::Bool)
        || ctx
            .get("continue_final_message")
            .is_none_or(|v| !matches!(bool::try_from(v.clone()), Ok(false)))
    {
        return invalid();
    }
    Ok(ctx)
}

/// Structural readiness only; provider/template equivalence needs independent qualification.
pub fn validate_mapping(mapping_json: &str) -> Result<(), EncodingError> {
    parse_mapping(mapping_json).map(|_| ())
}
