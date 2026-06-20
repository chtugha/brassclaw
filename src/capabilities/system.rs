use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use brassclaw_common::{AppEvent, DynEventPublisher, PlanStepDto};
use brassclaw_host_api::{
    CapabilityDescriptor, CapabilityId, EffectKind, ExtensionId, PermissionMode, ResourceCeiling,
    ResourceEstimate, ResourceProfile, RuntimeKind, TrustClass,
};
use chrono::{DateTime, LocalResult, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use serde_json::{Value, json};
use tokio::sync::RwLock;

pub const PROVIDER_ID: &str = "builtin";
pub const ECHO_CAPABILITY_ID: &str = "builtin.echo";
pub const TIME_CAPABILITY_ID: &str = "builtin.time";
pub const JSON_CAPABILITY_ID: &str = "builtin.json";
pub const PLAN_UPDATE_CAPABILITY_ID: &str = "builtin.plan_update";
pub const RESTART_CAPABILITY_ID: &str = "builtin.restart";
pub const SYSTEM_VERSION_CAPABILITY_ID: &str = "builtin.system_version";
pub const SYSTEM_TOOLS_LIST_CAPABILITY_ID: &str = "builtin.system_tools_list";

const DEFAULT_OUTPUT_BYTES: u64 = 4 * 1024;
const MAX_OUTPUT_BYTES: u64 = 256 * 1024;
const DEFAULT_WALL_CLOCK_MS: u64 = 100;
const MAX_WALL_CLOCK_MS: u64 = 10_000;

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct SystemCapabilityError {
    pub message: String,
    pub is_input_error: bool,
}

impl SystemCapabilityError {
    fn input(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            is_input_error: true,
        }
    }

    fn operation(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            is_input_error: false,
        }
    }
}

pub struct SystemContext {
    pub event_publisher: Option<DynEventPublisher>,
    pub tool_output_stash: Arc<RwLock<HashMap<String, String>>>,
    pub user_timezone: String,
    pub conversation_id: Option<uuid::Uuid>,
    pub registered_capability_names: Vec<(String, String)>,
}

fn resource_profile() -> Option<ResourceProfile> {
    Some(ResourceProfile {
        default_estimate: ResourceEstimate {
            wall_clock_ms: Some(DEFAULT_WALL_CLOCK_MS),
            output_bytes: Some(DEFAULT_OUTPUT_BYTES),
            ..ResourceEstimate::default()
        },
        hard_ceiling: Some(ResourceCeiling {
            max_usd: None,
            max_input_tokens: None,
            max_output_tokens: None,
            max_wall_clock_ms: Some(MAX_WALL_CLOCK_MS),
            max_output_bytes: Some(MAX_OUTPUT_BYTES),
            sandbox: None,
        }),
    })
}

fn make_descriptor(
    id: &str,
    description: &str,
    effects: Vec<EffectKind>,
    parameters_schema: Value,
    default_permission: PermissionMode,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        id: CapabilityId::new(id).expect("valid capability id"),
        provider: ExtensionId::new(PROVIDER_ID).expect("valid provider id"),
        runtime: RuntimeKind::FirstParty,
        trust_ceiling: TrustClass::Sandbox,
        description: description.to_string(),
        parameters_schema,
        effects,
        default_permission,
        runtime_credentials: Vec::new(),
        resource_profile: resource_profile(),
    }
}

pub fn echo_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        ECHO_CAPABILITY_ID,
        "Echoes back the input message. Useful for testing tool execution.",
        vec![],
        json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "The message to echo back"
                }
            },
            "required": ["message"],
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn time_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        TIME_CAPABILITY_ID,
        "Get current time, parse or format timestamps, convert timezones, or calculate time differences.",
        vec![],
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["now", "parse", "convert", "format", "diff"],
                    "description": "The time operation to perform"
                },
                "input": {
                    "type": "string",
                    "description": "Input timestamp. Accepts RFC 3339, or a naive timestamp when timezone/from_timezone is provided."
                },
                "timestamp": {
                    "type": "string",
                    "description": "Alias for input (kept for backward compatibility)."
                },
                "timezone": {
                    "type": "string",
                    "description": "IANA timezone name (e.g. 'America/New_York')."
                },
                "from_timezone": {
                    "type": "string",
                    "description": "Source IANA timezone for naive input timestamps."
                },
                "to_timezone": {
                    "type": "string",
                    "description": "Target IANA timezone for convert."
                },
                "format": {
                    "type": "string",
                    "description": "strftime format string (kept for backward compatibility)."
                },
                "format_string": {
                    "type": "string",
                    "description": "strftime format string for format."
                },
                "timestamp2": {
                    "type": "string",
                    "description": "Second timestamp for diff."
                }
            },
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn json_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        JSON_CAPABILITY_ID,
        "Parse, query, and transform JSON data. Supports JSONPath-like queries. \
         Use `source_tool_call_id` to reference the full output of a previous tool call \
         (avoids truncation issues with large responses).",
        vec![],
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["parse", "query", "stringify", "validate"],
                    "description": "The JSON operation to perform"
                },
                "data": {
                    "description": "JSON input data. Pass a string for parse, or any JSON value otherwise."
                },
                "source_tool_call_id": {
                    "type": "string",
                    "description": "Reference a previous tool call's full output by its ID."
                },
                "path": {
                    "type": "string",
                    "description": "JSONPath-like path for query operation (e.g., 'foo.bar[0].baz')"
                }
            },
            "required": ["operation"]
        }),
        PermissionMode::Allow,
    )
}

pub fn plan_update_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        PLAN_UPDATE_CAPABILITY_ID,
        "Update the plan progress checklist displayed to the user. Call this when creating a \
         plan, starting execution, completing a step, or when the plan fails. The UI renders \
         this as a live checklist. Always send the FULL list of steps (not incremental diffs).",
        vec![],
        json!({
            "type": "object",
            "properties": {
                "plan_id": {
                    "type": "string",
                    "description": "Plan identifier (slug or ID)"
                },
                "title": {
                    "type": "string",
                    "description": "Plan title"
                },
                "status": {
                    "type": "string",
                    "enum": ["draft", "approved", "executing", "completed", "failed"],
                    "description": "Overall plan status"
                },
                "steps": {
                    "type": "array",
                    "description": "Full list of plan steps with their current status",
                    "items": {
                        "type": "object",
                        "properties": {
                            "title": { "type": "string", "description": "Step description" },
                            "status": {
                                "type": "string",
                                "enum": ["pending", "in_progress", "completed", "failed"],
                                "description": "Step status"
                            },
                            "result": { "type": "string", "description": "Step result or error message (optional)" }
                        },
                        "required": ["title", "status"]
                    }
                },
                "mission_id": {
                    "type": "string",
                    "description": "Associated mission ID"
                }
            },
            "required": ["plan_id", "title", "status", "steps"],
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn restart_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        RESTART_CAPABILITY_ID,
        "Restart the BrassClaw agent process. The process exits cleanly (code 0) and the \
         container entrypoint loop restarts it automatically within a few seconds.",
        vec![EffectKind::SpawnProcess],
        json!({
            "type": "object",
            "properties": {
                "delay_secs": {
                    "type": "integer",
                    "description": "Seconds to wait before exiting (default: 2, min: 1, max: 30)",
                    "minimum": 1,
                    "maximum": 30
                }
            },
            "additionalProperties": false
        }),
        PermissionMode::Ask,
    )
}

pub fn system_version_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        SYSTEM_VERSION_CAPABILITY_ID,
        "Get the agent version and build information",
        vec![],
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn system_tools_list_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        SYSTEM_TOOLS_LIST_CAPABILITY_ID,
        "List all registered tools with names and descriptions",
        vec![],
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn descriptors() -> Vec<CapabilityDescriptor> {
    vec![
        echo_descriptor(),
        time_descriptor(),
        json_descriptor(),
        plan_update_descriptor(),
        restart_descriptor(),
        system_version_descriptor(),
        system_tools_list_descriptor(),
    ]
}

fn require_str<'a>(params: &'a Value, key: &str) -> Result<&'a str, SystemCapabilityError> {
    params
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| SystemCapabilityError::input(format!("missing required parameter: {key}")))
}

pub fn execute_echo(params: &Value) -> Result<Value, SystemCapabilityError> {
    let message = require_str(params, "message")?;
    Ok(Value::String(message.to_string()))
}

pub fn execute_time(
    params: &Value,
    user_timezone: &str,
) -> Result<Value, SystemCapabilityError> {
    let operation = params
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("now");

    match operation {
        "now" => time_now(params, user_timezone),
        "parse" => time_parse(params, user_timezone),
        "convert" => time_convert(params, user_timezone),
        "format" => time_format(params, user_timezone),
        "diff" => time_diff(params, user_timezone),
        _ => Err(SystemCapabilityError::input(format!(
            "unknown operation: {operation}"
        ))),
    }
}

pub async fn execute_json(
    params: &Value,
    ctx: &SystemContext,
) -> Result<Value, SystemCapabilityError> {
    let operation = require_str(params, "operation")?;

    let data_value =
        if let Some(ref_id) = params.get("source_tool_call_id").and_then(|v| v.as_str()) {
            let stash = ctx.tool_output_stash.read().await;
            let full_output = stash.get(ref_id).ok_or_else(|| {
                SystemCapabilityError::input(format!(
                    "no tool output found for call ID '{}'. Available IDs: {:?}",
                    ref_id,
                    stash.keys().collect::<Vec<_>>()
                ))
            })?;
            serde_json::from_str::<Value>(full_output)
                .unwrap_or_else(|_| Value::String(full_output.clone()))
        } else {
            params
                .get("data")
                .ok_or_else(|| SystemCapabilityError::input("missing 'data' parameter"))?
                .clone()
        };

    match operation {
        "parse" => {
            let json_str = data_value.as_str().ok_or_else(|| {
                SystemCapabilityError::input("'data' must be a string for parse operation")
            })?;
            serde_json::from_str(json_str)
                .map_err(|e| SystemCapabilityError::input(format!("invalid JSON: {e}")))
        }
        "stringify" => {
            let value = if data_value.is_string() {
                json_parse_input(&data_value)?
            } else {
                data_value
            };
            let json_str = serde_json::to_string_pretty(&value)
                .map_err(|e| SystemCapabilityError::operation(format!("failed to stringify: {e}")))?;
            Ok(Value::String(json_str))
        }
        "query" => {
            let path = params.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
                SystemCapabilityError::input("missing 'path' parameter for query")
            })?;
            let value = if data_value.is_string() {
                json_parse_input(&data_value)?
            } else {
                data_value
            };
            json_query(&value, path)
        }
        "validate" => {
            let is_valid = data_value
                .as_str()
                .map(|s| serde_json::from_str::<Value>(s).is_ok())
                .unwrap_or(false);
            Ok(json!({ "valid": is_valid }))
        }
        _ => Err(SystemCapabilityError::input(format!(
            "unknown operation: {operation}"
        ))),
    }
}

pub fn execute_plan_update(
    params: &Value,
    ctx: &SystemContext,
) -> Result<Value, SystemCapabilityError> {
    let plan_id = require_str(params, "plan_id")?;
    let title = require_str(params, "title")?;
    let status = require_str(params, "status")?;

    let steps: Vec<PlanStepDto> = params
        .get("steps")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .enumerate()
                .filter_map(|(i, s)| {
                    Some(PlanStepDto {
                        index: i,
                        title: s.get("title")?.as_str()?.to_string(),
                        status: s.get("status")?.as_str()?.to_string(),
                        result: s
                            .get("result")
                            .and_then(|r| r.as_str())
                            .map(|s| s.to_string()),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let mission_id = params
        .get("mission_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let completed = steps.iter().filter(|s| s.status == "completed").count();
    let total = steps.len();

    if let Some(ref ep) = ctx.event_publisher {
        ep.broadcast(AppEvent::PlanUpdate {
            plan_id: plan_id.to_string(),
            title: title.to_string(),
            status: status.to_string(),
            steps: steps.clone(),
            mission_id: mission_id.clone(),
            thread_id: ctx.conversation_id.map(|id| id.to_string()),
        });
    }

    let summary = format!(
        "Plan '{}' updated: {} ({}/{} steps completed)",
        title, status, completed, total
    );

    Ok(Value::String(summary))
}

pub fn execute_restart(params: &Value) -> Result<Value, SystemCapabilityError> {
    let in_docker = std::env::var("BRASSCLAW_IN_DOCKER")
        .map(|v| v.to_lowercase() == "true")
        .unwrap_or(false);

    if !in_docker {
        return Err(SystemCapabilityError::operation(
            "Restart is only available when running inside the Docker container. \
             For local development, please restart BrassClaw manually.",
        ));
    }

    let delay = params
        .get("delay_secs")
        .and_then(|v| v.as_u64())
        .unwrap_or(2)
        .clamp(1, 30);

    let restart_disabled = std::env::var("BRASSCLAW_DISABLE_RESTART")
        .map(|v| {
            let v = v.to_lowercase();
            v == "1" || v == "true"
        })
        .unwrap_or(false);

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(delay)).await;
        if !restart_disabled {
            std::process::exit(0);
        }
    });

    let msg = format!(
        "Restarting in {delay} second(s). The process will exit cleanly and the \
         entrypoint restart loop will bring BrassClaw back online."
    );
    Ok(Value::String(msg))
}

pub fn execute_system_version() -> Result<Value, SystemCapabilityError> {
    Ok(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "name": env!("CARGO_PKG_NAME"),
    }))
}

pub fn execute_system_tools_list(ctx: &SystemContext) -> Result<Value, SystemCapabilityError> {
    let tools: Vec<Value> = ctx
        .registered_capability_names
        .iter()
        .map(|(name, desc)| {
            json!({
                "name": name,
                "description": desc
            })
        })
        .collect();
    let count = tools.len();
    Ok(json!({ "tools": tools, "count": count }))
}

fn time_now(params: &Value, user_timezone: &str) -> Result<Value, SystemCapabilityError> {
    let now = Utc::now();
    let mut result = json!({
        "iso": now.to_rfc3339(),
        "utc_iso": now.to_rfc3339(),
        "unix": now.timestamp(),
        "unix_millis": now.timestamp_millis()
    });

    if let Some((tz, tz_name)) = resolve_timezone_for_output(params, user_timezone)? {
        let local = now.with_timezone(&tz);
        result["local_iso"] = Value::String(local.to_rfc3339());
        result["timezone"] = Value::String(tz_name);
    }

    Ok(result)
}

fn time_parse(params: &Value, user_timezone: &str) -> Result<Value, SystemCapabilityError> {
    let input = require_input(params)?;
    let parse_tz = resolve_parse_timezone(params, user_timezone)?;
    let dt = parse_timestamp(input, parse_tz.as_ref())?;

    Ok(json!({
        "iso": dt.to_rfc3339(),
        "unix": dt.timestamp(),
        "unix_millis": dt.timestamp_millis()
    }))
}

fn time_convert(params: &Value, user_timezone: &str) -> Result<Value, SystemCapabilityError> {
    let input = require_input(params)?;
    let source_tz = optional_timezone(params, &["from_timezone", "timezone"])?;
    let dt = parse_timestamp(input, source_tz.as_ref())?;

    let target_name = params
        .get("to_timezone")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SystemCapabilityError::input("convert operation requires 'to_timezone'"))?;
    let target_tz = parse_tz(target_name)?;
    let converted = dt.with_timezone(&target_tz);

    let mut result = json!({
        "input": input,
        "utc_iso": dt.to_rfc3339(),
        "output": converted.to_rfc3339(),
        "timezone": target_tz.to_string()
    });

    if let Some((ctx_tz, ctx_tz_name)) = context_timezone(user_timezone)? {
        result["context_timezone"] = Value::String(ctx_tz_name);
        result["context_iso"] = Value::String(dt.with_timezone(&ctx_tz).to_rfc3339());
    }

    Ok(result)
}

fn time_format(params: &Value, user_timezone: &str) -> Result<Value, SystemCapabilityError> {
    let input = require_input(params)?;
    let output_tz = resolve_timezone_for_output(params, user_timezone)?;
    let source_tz = optional_timezone(params, &["from_timezone"])?
        .or_else(|| output_tz.as_ref().map(|(tz, _)| *tz));
    let dt = parse_timestamp(input, source_tz.as_ref())?;
    let format_string = params
        .get("format_string")
        .and_then(|v| v.as_str())
        .or_else(|| params.get("format").and_then(|v| v.as_str()))
        .unwrap_or("%Y-%m-%d %H:%M:%S %Z");

    let mut result = if let Some((tz, tz_name)) = output_tz {
        json!({
            "formatted": dt.with_timezone(&tz).format(format_string).to_string(),
            "timezone": tz_name
        })
    } else {
        json!({
            "formatted": dt.format(format_string).to_string()
        })
    };

    result["utc_iso"] = Value::String(dt.to_rfc3339());
    Ok(result)
}

fn time_diff(params: &Value, user_timezone: &str) -> Result<Value, SystemCapabilityError> {
    let parse_tz = resolve_parse_timezone(params, user_timezone)?;
    let ts1 = require_input(params)?;
    let ts2 = params
        .get("timestamp2")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SystemCapabilityError::input("diff operation requires 'timestamp2'"))?;

    let dt1 = parse_timestamp(ts1, parse_tz.as_ref())?;
    let dt2 = parse_timestamp(ts2, parse_tz.as_ref())?;
    let diff = dt2.signed_duration_since(dt1);

    Ok(json!({
        "seconds": diff.num_seconds(),
        "minutes": diff.num_minutes(),
        "hours": diff.num_hours(),
        "days": diff.num_days()
    }))
}

fn require_input(params: &Value) -> Result<&str, SystemCapabilityError> {
    params
        .get("input")
        .and_then(|v| v.as_str())
        .or_else(|| params.get("timestamp").and_then(|v| v.as_str()))
        .ok_or_else(|| {
            SystemCapabilityError::input(
                "missing 'input' (or legacy 'timestamp') parameter",
            )
        })
}

fn resolve_parse_timezone(
    params: &Value,
    user_timezone: &str,
) -> Result<Option<Tz>, SystemCapabilityError> {
    if let Some(tz) = optional_timezone(params, &["from_timezone", "timezone"])? {
        return Ok(Some(tz));
    }
    Ok(context_timezone(user_timezone)?.map(|(tz, _)| tz))
}

fn resolve_timezone_for_output(
    params: &Value,
    user_timezone: &str,
) -> Result<Option<(Tz, String)>, SystemCapabilityError> {
    if let Some(name) = params
        .get("timezone")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        let tz = parse_tz(name)?;
        return Ok(Some((tz, tz.to_string())));
    }
    context_timezone(user_timezone)
}

fn context_timezone(user_timezone: &str) -> Result<Option<(Tz, String)>, SystemCapabilityError> {
    if user_timezone != "UTC"
        && !user_timezone.is_empty()
        && let Some(tz) = crate::timezone::parse_timezone(user_timezone)
    {
        return Ok(Some((tz, tz.to_string())));
    }
    Ok(None)
}

fn optional_timezone(
    params: &Value,
    keys: &[&str],
) -> Result<Option<Tz>, SystemCapabilityError> {
    for key in keys {
        if let Some(value) = params
            .get(*key)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            return parse_tz(value).map(Some);
        }
    }
    Ok(None)
}

fn parse_tz(value: &str) -> Result<Tz, SystemCapabilityError> {
    value.parse::<Tz>().map_err(|_| {
        SystemCapabilityError::input(format!(
            "Unknown timezone '{}'. Use IANA names like 'America/New_York' or 'Europe/London'.",
            value
        ))
    })
}

fn parse_timestamp(
    input: &str,
    fallback_tz: Option<&Tz>,
) -> Result<DateTime<Utc>, SystemCapabilityError> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(input) {
        return Ok(dt.with_timezone(&Utc));
    }

    if let Some(naive) = parse_naive_datetime(input) {
        return localize_naive_datetime(naive, fallback_tz, input);
    }

    Err(SystemCapabilityError::input(format!(
        "invalid timestamp '{}': expected RFC 3339 or a naive timestamp with timezone/from_timezone",
        input
    )))
}

fn parse_naive_datetime(input: &str) -> Option<NaiveDateTime> {
    const DATETIME_FORMATS: &[&str] = &[
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%dT%H:%M",
    ];
    const DATE_FORMATS: &[&str] = &["%Y-%m-%d"];

    for format in DATETIME_FORMATS {
        if let Ok(value) = NaiveDateTime::parse_from_str(input, format) {
            return Some(value);
        }
    }

    for format in DATE_FORMATS {
        if let Ok(date) = NaiveDate::parse_from_str(input, format) {
            return date.and_hms_opt(0, 0, 0);
        }
    }

    None
}

fn localize_naive_datetime(
    naive: NaiveDateTime,
    fallback_tz: Option<&Tz>,
    original_input: &str,
) -> Result<DateTime<Utc>, SystemCapabilityError> {
    let tz = fallback_tz.ok_or_else(|| {
        SystemCapabilityError::input(format!(
            "timestamp '{}' has no UTC offset; provide 'timezone' or 'from_timezone'",
            original_input
        ))
    })?;

    match tz.from_local_datetime(&naive) {
        LocalResult::Single(dt) => Ok(dt.with_timezone(&Utc)),
        LocalResult::Ambiguous(_, _) => Err(SystemCapabilityError::input(format!(
            "timestamp '{}' is ambiguous in timezone '{}'; include an explicit UTC offset instead",
            original_input, tz
        ))),
        LocalResult::None => Err(SystemCapabilityError::input(format!(
            "timestamp '{}' does not exist in timezone '{}'",
            original_input, tz
        ))),
    }
}

fn json_parse_input(data: &Value) -> Result<Value, SystemCapabilityError> {
    let json_str = data
        .as_str()
        .ok_or_else(|| SystemCapabilityError::input("'data' must be a JSON string"))?;
    serde_json::from_str(json_str)
        .map_err(|e| SystemCapabilityError::input(format!("invalid JSON input: {e}")))
}

fn json_query(data: &Value, path: &str) -> Result<Value, SystemCapabilityError> {
    let mut current = data;

    for segment in path.split('.') {
        if segment.is_empty() {
            continue;
        }

        if let Some((field, index_str)) = segment.split_once('[') {
            if !field.is_empty() {
                current = current.get(field).ok_or_else(|| {
                    SystemCapabilityError::operation(format!("field not found: {field}"))
                })?;
            }

            let index_str = index_str.trim_end_matches(']');
            let index: usize = index_str.parse().map_err(|_| {
                SystemCapabilityError::input(format!("invalid array index: {index_str}"))
            })?;

            current = current.get(index).ok_or_else(|| {
                SystemCapabilityError::operation(format!("array index out of bounds: {index}"))
            })?;
        } else {
            current = current.get(segment).ok_or_else(|| {
                SystemCapabilityError::operation(format!("field not found: {segment}"))
            })?;
        }
    }

    Ok(current.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echo_descriptor_is_valid() {
        let desc = echo_descriptor();
        assert_eq!(desc.id.as_str(), ECHO_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.is_empty());
        assert_eq!(desc.default_permission, PermissionMode::Allow);
    }

    #[test]
    fn time_descriptor_is_valid() {
        let desc = time_descriptor();
        assert_eq!(desc.id.as_str(), TIME_CAPABILITY_ID);
        assert!(desc.effects.is_empty());
    }

    #[test]
    fn json_descriptor_is_valid() {
        let desc = json_descriptor();
        assert_eq!(desc.id.as_str(), JSON_CAPABILITY_ID);
        assert!(desc.effects.is_empty());
    }

    #[test]
    fn plan_update_descriptor_is_valid() {
        let desc = plan_update_descriptor();
        assert_eq!(desc.id.as_str(), PLAN_UPDATE_CAPABILITY_ID);
    }

    #[test]
    fn restart_descriptor_is_valid() {
        let desc = restart_descriptor();
        assert_eq!(desc.id.as_str(), RESTART_CAPABILITY_ID);
        assert!(desc.effects.contains(&EffectKind::SpawnProcess));
        assert_eq!(desc.default_permission, PermissionMode::Ask);
    }

    #[test]
    fn system_version_descriptor_is_valid() {
        let desc = system_version_descriptor();
        assert_eq!(desc.id.as_str(), SYSTEM_VERSION_CAPABILITY_ID);
    }

    #[test]
    fn system_tools_list_descriptor_is_valid() {
        let desc = system_tools_list_descriptor();
        assert_eq!(desc.id.as_str(), SYSTEM_TOOLS_LIST_CAPABILITY_ID);
    }

    #[test]
    fn descriptors_returns_all() {
        let descs = descriptors();
        assert_eq!(descs.len(), 7);
    }

    #[test]
    fn test_execute_echo() {
        let result = execute_echo(&json!({"message": "hello"})).unwrap();
        assert_eq!(result.as_str(), Some("hello"));
    }

    #[test]
    fn test_execute_echo_missing_message() {
        let result = execute_echo(&json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().is_input_error);
    }

    #[test]
    fn test_time_now_utc() {
        let result = execute_time(&json!({"operation": "now"}), "UTC").unwrap();
        assert!(result.get("iso").is_some());
        assert!(result.get("utc_iso").is_some());
        assert!(result.get("unix").is_some());
    }

    #[test]
    fn test_time_now_with_timezone() {
        let result = execute_time(
            &json!({"operation": "now", "timezone": "America/New_York"}),
            "UTC",
        )
        .unwrap();
        assert_eq!(result["timezone"].as_str(), Some("America/New_York"));
        assert!(result.get("local_iso").is_some());
    }

    #[test]
    fn test_time_now_with_user_timezone() {
        let result = execute_time(&json!({"operation": "now"}), "America/New_York").unwrap();
        assert_eq!(result["timezone"].as_str(), Some("America/New_York"));
        assert!(result.get("local_iso").is_some());
    }

    #[test]
    fn test_time_convert() {
        let result = execute_time(
            &json!({
                "operation": "convert",
                "input": "2026-03-08T07:30:00Z",
                "to_timezone": "America/New_York"
            }),
            "UTC",
        )
        .unwrap();
        assert_eq!(result["timezone"].as_str(), Some("America/New_York"));
        assert_eq!(
            result["output"].as_str(),
            Some("2026-03-08T03:30:00-04:00")
        );
    }

    #[test]
    fn test_time_format() {
        let result = execute_time(
            &json!({
                "operation": "format",
                "input": "2026-03-08T07:30:00Z",
                "timezone": "America/New_York",
                "format_string": "%Y-%m-%d %H:%M:%S %Z"
            }),
            "UTC",
        )
        .unwrap();
        assert_eq!(
            result["formatted"].as_str(),
            Some("2026-03-08 03:30:00 EDT")
        );
    }

    #[test]
    fn test_time_invalid_timezone() {
        let result = execute_time(
            &json!({"operation": "now", "timezone": "Mars/Olympus"}),
            "UTC",
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("Unknown timezone"));
    }

    #[test]
    fn test_time_empty_timezone_string() {
        let result = execute_time(
            &json!({"operation": "now", "timezone": ""}),
            "UTC",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_json_query() {
        let data = json!({
            "foo": {
                "bar": [1, 2, 3],
                "baz": "hello"
            }
        });
        assert_eq!(
            json_query(&data, "foo.baz").unwrap(),
            json!("hello")
        );
        assert_eq!(
            json_query(&data, "foo.bar[0]").unwrap(),
            json!(1)
        );
        assert_eq!(
            json_query(&data, "foo.bar[2]").unwrap(),
            json!(3)
        );
    }

    #[test]
    fn test_json_parse_input() {
        let input = json!("{\"ok\":true}");
        let parsed = json_parse_input(&input).unwrap();
        assert_eq!(parsed, json!({"ok": true}));
    }

    #[test]
    fn test_json_parse_input_invalid() {
        let input = json!("{not valid json}");
        let err = json_parse_input(&input).unwrap_err();
        assert!(err.message.contains("invalid JSON input"));
    }

    #[tokio::test]
    async fn test_execute_json_query() {
        let ctx = SystemContext {
            event_publisher: None,
            tool_output_stash: Arc::new(RwLock::new(HashMap::new())),
            user_timezone: "UTC".to_string(),
            conversation_id: None,
            registered_capability_names: vec![],
        };
        let params = json!({
            "operation": "query",
            "data": {"foo": {"bar": 42}},
            "path": "foo.bar"
        });
        let result = execute_json(&params, &ctx).await.unwrap();
        assert_eq!(result, json!(42));
    }

    #[tokio::test]
    async fn test_execute_json_with_stash() {
        let stash = Arc::new(RwLock::new(HashMap::new()));
        stash
            .write()
            .await
            .insert(
                "call_01".to_string(),
                r#"{"key": "value"}"#.to_string(),
            );
        let ctx = SystemContext {
            event_publisher: None,
            tool_output_stash: stash,
            user_timezone: "UTC".to_string(),
            conversation_id: None,
            registered_capability_names: vec![],
        };
        let params = json!({
            "operation": "query",
            "source_tool_call_id": "call_01",
            "path": "key"
        });
        let result = execute_json(&params, &ctx).await.unwrap();
        assert_eq!(result, json!("value"));
    }

    #[test]
    fn test_execute_system_version() {
        let result = execute_system_version().unwrap();
        assert!(result.get("version").is_some());
        assert!(result.get("name").is_some());
    }

    #[test]
    fn test_execute_system_tools_list() {
        let ctx = SystemContext {
            event_publisher: None,
            tool_output_stash: Arc::new(RwLock::new(HashMap::new())),
            user_timezone: "UTC".to_string(),
            conversation_id: None,
            registered_capability_names: vec![
                ("echo".to_string(), "Echo tool".to_string()),
                ("time".to_string(), "Time tool".to_string()),
            ],
        };
        let result = execute_system_tools_list(&ctx).unwrap();
        assert_eq!(result["count"], 2);
        assert_eq!(result["tools"][0]["name"], "echo");
    }

    #[test]
    fn test_execute_plan_update() {
        let ctx = SystemContext {
            event_publisher: None,
            tool_output_stash: Arc::new(RwLock::new(HashMap::new())),
            user_timezone: "UTC".to_string(),
            conversation_id: None,
            registered_capability_names: vec![],
        };
        let params = json!({
            "plan_id": "test-plan",
            "title": "Test Plan",
            "status": "executing",
            "steps": [
                {"title": "Step 1", "status": "completed"},
                {"title": "Step 2", "status": "in_progress"}
            ]
        });
        let result = execute_plan_update(&params, &ctx).unwrap();
        let text = result.as_str().unwrap();
        assert!(text.contains("Test Plan"));
        assert!(text.contains("1/2 steps completed"));
    }

    #[test]
    fn test_parse_naive_timestamp_with_timezone() {
        let dt = parse_timestamp("2026-03-08 03:30:00", Some(&chrono_tz::America::New_York))
            .expect("parse timestamp");
        assert_eq!(dt.to_rfc3339(), "2026-03-08T07:30:00+00:00");
    }
}
