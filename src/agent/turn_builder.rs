use serde::{Serialize, Deserialize};
use uuid::Uuid;
use brassclaw_common::truncate_preview;
use crate::generated_images::GeneratedImageSentinel;
use crate::history::ConversationMessage;

pub const MAX_HISTORY_IMAGE_DATA_URL_BYTES_PER_IMAGE: usize = 512 * 1024;
pub const MAX_HISTORY_IMAGE_DATA_URL_BYTES_PER_RESPONSE: usize = 1024 * 1024;
pub const MAX_TOOL_RESULT_DISPLAY_BYTES: usize = 1000;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TurnInfo {
    pub turn_number: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_message_id: Option<Uuid>,
    pub user_input: String,
    pub response: Option<String>,
    pub state: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub tool_calls: Vec<ToolCallInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub generated_images: Vec<GeneratedImageInfo>,
    /// Agent's reasoning narrative for this turn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub narrative: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ToolCallInfo {
    pub name: String,
    pub has_result: bool,
    pub has_error: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Agent's reasoning for choosing this tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct GeneratedImageInfo {
    pub event_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// Convert stored tool errors into plain text suitable for UI display.
pub fn tool_error_for_display(error: &str) -> String {
    brassclaw_safety::SafetyLayer::unwrap_tool_output(error).unwrap_or_else(|| error.to_string())
}

/// Convert stored tool results into plain text suitable for UI display.
pub fn tool_result_for_display(result: &serde_json::Value) -> Option<String> {
    if result.is_null() {
        return None;
    }

    if GeneratedImageSentinel::from_value(result).is_some() {
        return Some("Generated image".to_string());
    }

    let content = match result {
        serde_json::Value::String(s) => {
            brassclaw_safety::SafetyLayer::unwrap_tool_output(s).unwrap_or_else(|| s.clone())
        }
        other => other.to_string(),
    };

    if content.is_empty() {
        return None;
    }

    Some(truncate_preview(&content, MAX_TOOL_RESULT_DISPLAY_BYTES))
}

/// Parse tool call summary JSON objects into `ToolCallInfo` structs.
fn parse_tool_call_infos(calls: &[serde_json::Value]) -> Vec<ToolCallInfo> {
    calls
        .iter()
        .map(|c| {
            let result_preview = c.get("result_preview").and_then(tool_result_for_display);
            let result = c.get("result").and_then(tool_result_for_display);
            ToolCallInfo {
                name: c["name"].as_str().unwrap_or("unknown").to_string(),
                has_result: c.get("result").is_some_and(|v| !v.is_null())
                    || c.get("result_preview").is_some_and(|v| !v.is_null()),
                has_error: c.get("error").is_some_and(|v| !v.is_null()),
                call_id: c
                    .get("tool_call_id")
                    .or_else(|| c.get("call_id"))
                    .and_then(|v| v.as_str())
                    .map(String::from),
                result,
                result_preview,
                error: c["error"].as_str().map(tool_error_for_display),
                rationale: c["rationale"].as_str().map(String::from),
            }
        })
        .collect()
}

fn generated_image_event_id(
    turn_number: usize,
    result_index: usize,
    preferred_id: Option<&str>,
) -> String {
    preferred_id
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("turn-{turn_number}-image-{result_index}"))
}

fn parse_image_generated_sentinel_from_value(
    value: &serde_json::Value,
    event_id: String,
) -> Option<GeneratedImageInfo> {
    let sentinel = GeneratedImageSentinel::from_value(value)?;
    let data_url = sentinel
        .data_url()
        .filter(|data_url| !data_url.is_empty())
        .map(str::to_string);
    let path = sentinel.path().map(String::from);
    Some(GeneratedImageInfo {
        event_id,
        data_url,
        path,
    })
}

pub fn collect_generated_images_from_tool_results<'a>(
    turn_number: usize,
    tool_results: impl IntoIterator<Item = (Option<&'a str>, Option<&'a serde_json::Value>)>,
) -> Vec<GeneratedImageInfo> {
    tool_results
        .into_iter()
        .enumerate()
        .filter_map(|(result_index, (event_id, result))| {
            parse_image_generated_sentinel_from_value(
                result?,
                generated_image_event_id(turn_number, result_index, event_id),
            )
        })
        .collect()
}

pub fn tool_result_preview(result: Option<&serde_json::Value>) -> Option<String> {
    let result = result?;
    tool_result_for_display(result)
}

/// Build TurnInfo pairs from flat DB messages (user/tool_calls/assistant triples).
pub fn build_turns_from_db_messages(
    messages: &[ConversationMessage],
) -> Vec<TurnInfo> {
    let mut turns = Vec::new();
    let mut turn_number = 0;
    let mut iter = messages.iter().peekable();

    while let Some(msg) = iter.next() {
        if msg.role == "user" {
            let mut turn = TurnInfo {
                turn_number,
                user_message_id: Some(msg.id),
                user_input: msg.content.clone(),
                response: None,
                state: "Completed".to_string(),
                started_at: msg.created_at.to_rfc3339(),
                completed_at: None,
                tool_calls: Vec::new(),
                generated_images: Vec::new(),
                narrative: None,
            };

            // Check if next message is a tool_calls record
            if let Some(next) = iter.peek()
                && next.role == "tool_calls"
            {
                let tc_msg = iter.next().expect("peeked");
                match serde_json::from_str::<serde_json::Value>(&tc_msg.content) {
                    Ok(serde_json::Value::Array(calls)) => {
                        turn.tool_calls = parse_tool_call_infos(&calls);
                        turn.generated_images = collect_generated_images_from_tool_results(
                            turn_number,
                            calls.iter().map(|call| {
                                (
                                    call.get("tool_call_id")
                                        .or_else(|| call.get("call_id"))
                                        .and_then(|v| v.as_str()),
                                    call.get("result"),
                                )
                            }),
                        );
                    }
                    Ok(serde_json::Value::Object(obj)) => {
                        turn.narrative = obj
                            .get("narrative")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        if let Some(serde_json::Value::Array(calls)) = obj.get("calls") {
                            turn.tool_calls = parse_tool_call_infos(calls);
                            turn.generated_images = collect_generated_images_from_tool_results(
                                turn_number,
                                calls.iter().map(|call| {
                                    (
                                        call.get("tool_call_id")
                                            .or_else(|| call.get("call_id"))
                                            .and_then(|v| v.as_str()),
                                        call.get("result"),
                                    )
                                }),
                            );
                        }
                    }
                    Ok(_) => {
                        tracing::warn!(
                            message_id = %tc_msg.id,
                            "Unexpected tool_calls JSON shape in DB, skipping"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            message_id = %tc_msg.id,
                            "Malformed tool_calls JSON in DB, skipping: {e}"
                        );
                    }
                }
            }

            // Check if next message is an assistant response
            if let Some(next) = iter.peek()
                && next.role == "assistant"
            {
                let assistant_msg = iter.next().expect("peeked");
                turn.response = Some(assistant_msg.content.clone());
                turn.completed_at = Some(assistant_msg.created_at.to_rfc3339());
            }

            // Incomplete turn (user message without response)
            if turn.response.is_none() {
                turn.state = "Failed".to_string();
            }

            turns.push(turn);
            turn_number += 1;
        } else if msg.role == "assistant" {
            turns.push(TurnInfo {
                turn_number,
                user_message_id: None,
                user_input: String::new(),
                response: Some(msg.content.clone()),
                state: "Completed".to_string(),
                started_at: msg.created_at.to_rfc3339(),
                completed_at: Some(msg.created_at.to_rfc3339()),
                tool_calls: Vec::new(),
                generated_images: Vec::new(),
                narrative: None,
            });
            turn_number += 1;
        }
    }

    turns
}

pub fn enforce_generated_image_history_budget(turns: &mut [TurnInfo]) {
    let mut remaining_bytes = MAX_HISTORY_IMAGE_DATA_URL_BYTES_PER_RESPONSE;
    for turn in turns.iter_mut().rev() {
        for image in turn.generated_images.iter_mut().rev() {
            let Some(data_url) = image.data_url.as_ref() else {
                continue;
            };
            let data_url_bytes = data_url.len();
            if data_url_bytes > MAX_HISTORY_IMAGE_DATA_URL_BYTES_PER_IMAGE
                || data_url_bytes > remaining_bytes
            {
                image.data_url = None;
                continue;
            }
            remaining_bytes -= data_url_bytes;
        }
    }
}
