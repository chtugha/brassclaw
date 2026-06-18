use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use brassclaw_host_api::{
    CapabilityDescriptor, CapabilityId, EffectKind, ExtensionId, PermissionMode, ResourceCeiling,
    ResourceEstimate, ResourceProfile, RuntimeKind, TrustClass,
};
use chrono::Utc;
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::agent::routine::{
    NotifyConfig, Routine, RoutineAction, RoutineGuardrails, Trigger, next_cron_fire,
    normalize_cron_expression, reset_routine_verification_state, routine_verification_fingerprint,
    routine_verification_status,
};
use crate::agent::routine_engine::RoutineEngine;
use crate::db::Database;

pub const PROVIDER_ID: &str = "builtin";
pub const ROUTINE_CREATE_CAPABILITY_ID: &str = "builtin.routine_create";
pub const ROUTINE_UPDATE_CAPABILITY_ID: &str = "builtin.routine_update";
pub const ROUTINE_DELETE_CAPABILITY_ID: &str = "builtin.routine_delete";
pub const ROUTINE_LIST_CAPABILITY_ID: &str = "builtin.routine_list";
pub const ROUTINE_HISTORY_CAPABILITY_ID: &str = "builtin.routine_history";
pub const ROUTINE_FIRE_CAPABILITY_ID: &str = "builtin.routine_fire";
pub const EVENT_EMIT_CAPABILITY_ID: &str = "builtin.event_emit";

const DEFAULT_OUTPUT_BYTES: u64 = 4 * 1024;
const MAX_OUTPUT_BYTES: u64 = 64 * 1024;
const DEFAULT_WALL_CLOCK_MS: u64 = 2_000;
const MAX_WALL_CLOCK_MS: u64 = 30_000;

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct RoutinesCapabilityError {
    pub message: String,
    pub is_input_error: bool,
}

impl RoutinesCapabilityError {
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

pub struct RoutinesContext {
    pub store: Arc<dyn Database>,
    pub engine: Arc<RoutineEngine>,
    pub user_id: String,
    pub metadata: Value,
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

pub fn routine_create_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        ROUTINE_CREATE_CAPABILITY_ID,
        "Create a new routine (scheduled or event-driven task). \
         Supports cron schedules, event pattern matching, system events, and manual triggers.",
        vec![EffectKind::DispatchCapability],
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Unique name for the routine (e.g. 'daily-pr-review')."
                },
                "prompt": {
                    "type": "string",
                    "description": "Instructions for what the routine should do when it fires."
                },
                "description": {
                    "type": "string",
                    "description": "Optional human-readable summary of what the routine does."
                },
                "request": {
                    "type": "object",
                    "description": "Canonical trigger config. Set request.kind first, then only fill fields that match that kind.",
                    "properties": {
                        "kind": {
                            "type": "string",
                            "enum": ["cron", "manual", "message_event", "system_event"],
                            "description": "How the routine should start."
                        },
                        "schedule": {
                            "type": "string",
                            "description": "Cron expression for request.kind='cron'. Uses 6-field cron: second minute hour day month weekday."
                        },
                        "timezone": {
                            "type": "string",
                            "description": "IANA timezone for request.kind='cron', such as 'America/New_York'."
                        },
                        "pattern": {
                            "type": "string",
                            "description": "Regex pattern for request.kind='message_event'."
                        },
                        "channel": {
                            "type": "string",
                            "description": "Optional channel filter for request.kind='message_event'."
                        },
                        "source": {
                            "type": "string",
                            "description": "Event source namespace for request.kind='system_event', such as 'github'."
                        },
                        "event_type": {
                            "type": "string",
                            "description": "Event type for request.kind='system_event', such as 'issue.opened'."
                        },
                        "filters": {
                            "type": "object",
                            "properties": {},
                            "additionalProperties": {
                                "type": ["string", "number", "boolean"]
                            },
                            "description": "Optional exact-match filters for request.kind='system_event'."
                        }
                    },
                    "required": ["kind"]
                },
                "execution": {
                    "type": "object",
                    "description": "Optional execution settings. Omit for the default lightweight mode.",
                    "properties": {
                        "mode": {
                            "type": "string",
                            "enum": ["lightweight", "full_job"],
                            "description": "Execution mode. 'lightweight' is the default."
                        },
                        "context_paths": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Workspace paths to preload for lightweight routines."
                        },
                        "use_tools": {
                            "type": "boolean",
                            "default": true,
                            "description": "Only applies to lightweight mode."
                        },
                        "max_tool_rounds": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": crate::agent::routine::MAX_TOOL_ROUNDS_LIMIT,
                            "default": 3,
                            "description": "Only applies when execution.mode='lightweight' and use_tools=true."
                        },
                        "max_iterations": {
                            "type": "integer",
                            "description": "Maximum LLM iterations for the job (default: 25).",
                            "default": 25,
                            "minimum": 1,
                            "maximum": 200
                        }
                    }
                },
                "delivery": {
                    "type": "object",
                    "description": "Optional delivery defaults for notifications and message tool calls inside routine jobs.",
                    "properties": {
                        "channel": {
                            "type": "string",
                            "description": "Default channel for notifications and routine job message calls."
                        },
                        "user": {
                            "type": "string",
                            "description": "Default user or target for notifications and routine job message calls."
                        }
                    }
                },
                "advanced": {
                    "type": "object",
                    "description": "Optional advanced knobs.",
                    "properties": {
                        "cooldown_secs": {
                            "type": "integer",
                            "description": "Minimum seconds between automatic fires."
                        }
                    }
                }
            },
            "required": ["name", "prompt", "request"]
        }),
        PermissionMode::Ask,
    )
}

pub fn routine_update_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        ROUTINE_UPDATE_CAPABILITY_ID,
        "Update an existing routine. Can change prompt, description, enabled state, cron schedule/timezone.",
        vec![EffectKind::DispatchCapability],
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of the routine to update"
                },
                "enabled": {
                    "type": "boolean",
                    "description": "Enable or disable the routine"
                },
                "prompt": {
                    "type": "string",
                    "description": "New prompt/instructions"
                },
                "schedule": {
                    "type": "string",
                    "description": "New cron schedule (for cron triggers)"
                },
                "timezone": {
                    "type": "string",
                    "description": "IANA timezone for cron schedule (e.g. 'America/New_York'). Only valid for cron triggers."
                },
                "description": {
                    "type": "string",
                    "description": "New description"
                },
                "max_iterations": {
                    "type": "integer",
                    "description": "Maximum LLM iterations for full_job routines (1-200).",
                    "minimum": 1,
                    "maximum": 200
                }
            },
            "required": ["name"]
        }),
        PermissionMode::Ask,
    )
}

pub fn routine_delete_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        ROUTINE_DELETE_CAPABILITY_ID,
        "Delete a routine permanently. This also removes all run history.",
        vec![EffectKind::DispatchCapability],
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of the routine to delete"
                }
            },
            "required": ["name"]
        }),
        PermissionMode::Ask,
    )
}

pub fn routine_list_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        ROUTINE_LIST_CAPABILITY_ID,
        "List all routines with their status, trigger info, and next fire time.",
        vec![EffectKind::DispatchCapability],
        json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        PermissionMode::Allow,
    )
}

pub fn routine_history_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        ROUTINE_HISTORY_CAPABILITY_ID,
        "View the execution history of a routine. Shows recent runs with status, duration, and results.",
        vec![EffectKind::DispatchCapability],
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of the routine"
                },
                "limit": {
                    "type": "integer",
                    "description": "Max runs to return (default: 10)",
                    "default": 10
                }
            },
            "required": ["name"]
        }),
        PermissionMode::Allow,
    )
}

pub fn routine_fire_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        ROUTINE_FIRE_CAPABILITY_ID,
        "Manually trigger a routine to run immediately, bypassing schedule, trigger type, and cooldown.",
        vec![EffectKind::DispatchCapability],
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of the routine to fire"
                }
            },
            "required": ["name"]
        }),
        PermissionMode::Ask,
    )
}

pub fn event_emit_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        EVENT_EMIT_CAPABILITY_ID,
        "Emit a structured system event to routines with a system_event trigger.",
        vec![EffectKind::DispatchCapability],
        json!({
            "type": "object",
            "properties": {
                "event_source": {
                    "type": "string",
                    "description": "Canonical event source, such as 'github'."
                },
                "event_type": {
                    "type": "string",
                    "description": "Event type, such as 'issue.opened'."
                },
                "payload": {
                    "type": "object",
                    "properties": {},
                    "description": "Structured event payload."
                }
            },
            "required": ["event_source", "event_type"]
        }),
        PermissionMode::Ask,
    )
}

pub fn descriptors() -> Vec<CapabilityDescriptor> {
    vec![
        routine_create_descriptor(),
        routine_update_descriptor(),
        routine_delete_descriptor(),
        routine_list_descriptor(),
        routine_history_descriptor(),
        routine_fire_descriptor(),
        event_emit_descriptor(),
    ]
}

fn require_str<'a>(params: &'a Value, key: &str) -> Result<&'a str, RoutinesCapabilityError> {
    params
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RoutinesCapabilityError::input(format!("missing required parameter: {key}"))
        })
}

fn nested_object<'a>(params: &'a Value, field: &str) -> Option<&'a Map<String, Value>> {
    params.get(field).and_then(Value::as_object)
}

fn string_field(params: &Value, group: &str, field: &str, aliases: &[&str]) -> Option<String> {
    nested_object(params, group)
        .and_then(|obj| obj.get(field))
        .and_then(Value::as_str)
        .map(String::from)
        .or_else(|| {
            aliases
                .iter()
                .find_map(|alias| params.get(*alias).and_then(Value::as_str).map(String::from))
        })
}

fn bool_field(params: &Value, group: &str, field: &str, aliases: &[&str]) -> Option<bool> {
    nested_object(params, group)
        .and_then(|obj| obj.get(field))
        .and_then(Value::as_bool)
        .or_else(|| {
            aliases
                .iter()
                .find_map(|alias| params.get(*alias).and_then(Value::as_bool))
        })
}

fn u64_field(params: &Value, group: &str, field: &str, aliases: &[&str]) -> Option<u64> {
    nested_object(params, group)
        .and_then(|obj| obj.get(field))
        .and_then(Value::as_u64)
        .or_else(|| {
            aliases
                .iter()
                .find_map(|alias| params.get(*alias).and_then(Value::as_u64))
        })
}

fn string_array_field(params: &Value, group: &str, field: &str, aliases: &[&str]) -> Vec<String> {
    nested_object(params, group)
        .and_then(|obj| obj.get(field))
        .and_then(Value::as_array)
        .or_else(|| {
            aliases
                .iter()
                .find_map(|alias| params.get(*alias).and_then(Value::as_array))
        })
        .map(|arr| {
            let mut seen = std::collections::HashSet::new();
            arr.iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .filter_map(|value| {
                    if seen.insert(value.to_string()) {
                        Some(value.to_string())
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn object_field(
    params: &Value,
    group: &str,
    field: &str,
    aliases: &[&str],
) -> Option<Map<String, Value>> {
    nested_object(params, group)
        .and_then(|obj| obj.get(field))
        .and_then(Value::as_object)
        .cloned()
        .or_else(|| {
            aliases
                .iter()
                .find_map(|alias| params.get(*alias).and_then(Value::as_object).cloned())
        })
}

fn validate_timezone_param(timezone: Option<String>) -> Result<Option<String>, RoutinesCapabilityError> {
    timezone
        .map(|tz| {
            crate::timezone::parse_timezone(&tz)
                .map(|_| tz.clone())
                .ok_or_else(|| {
                    RoutinesCapabilityError::input(format!("invalid IANA timezone: '{tz}'"))
                })
        })
        .transpose()
}

fn parse_system_event_filters(
    filters: Option<Map<String, Value>>,
) -> Result<HashMap<String, String>, RoutinesCapabilityError> {
    let Some(obj) = filters else {
        return Ok(HashMap::new());
    };

    let mut parsed = HashMap::with_capacity(obj.len());
    for (key, value) in obj {
        let rendered = crate::agent::routine::json_value_as_filter_string(&value).ok_or_else(|| {
            RoutinesCapabilityError::input(format!(
                "system_event filters only support string, number, and boolean values (invalid '{key}')"
            ))
        })?;
        parsed.insert(key, rendered);
    }

    Ok(parsed)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum NormalizedTriggerRequest {
    Cron {
        schedule: String,
        timezone: Option<String>,
    },
    Manual,
    MessageEvent {
        pattern: String,
        channel: Option<String>,
    },
    SystemEvent {
        source: String,
        event_type: String,
        filters: HashMap<String, String>,
    },
    Webhook {
        path: Option<String>,
        secret: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NormalizedExecutionMode {
    Lightweight,
    FullJob,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedExecutionRequest {
    mode: NormalizedExecutionMode,
    context_paths: Vec<String>,
    use_tools: bool,
    max_tool_rounds: u32,
    max_iterations: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedDeliveryRequest {
    channel: Option<String>,
    user: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedRoutineCreateRequest {
    name: String,
    description: String,
    prompt: String,
    trigger: NormalizedTriggerRequest,
    execution: NormalizedExecutionRequest,
    delivery: NormalizedDeliveryRequest,
    cooldown_secs: u64,
}

fn parse_routine_trigger(params: &Value) -> Result<NormalizedTriggerRequest, RoutinesCapabilityError> {
    let kind = string_field(params, "request", "kind", &["trigger_type"])
        .map(|value| match value.as_str() {
            "event" => "message_event".to_string(),
            other => other.to_string(),
        })
        .ok_or_else(|| {
            RoutinesCapabilityError::input(
                "routine_create requires request.kind (canonical) or trigger_type (legacy)"
                    .to_string(),
            )
        })?;

    match kind.as_str() {
        "cron" => {
            let schedule =
                string_field(params, "request", "schedule", &["schedule"]).ok_or_else(|| {
                    RoutinesCapabilityError::input("cron request requires 'schedule'".to_string())
                })?;
            let timezone = validate_timezone_param(string_field(
                params,
                "request",
                "timezone",
                &["timezone"],
            ))?;
            next_cron_fire(&schedule, timezone.as_deref())
                .map_err(|e| RoutinesCapabilityError::input(format!("invalid cron schedule: {e}")))?;
            Ok(NormalizedTriggerRequest::Cron { schedule, timezone })
        }
        "manual" => Ok(NormalizedTriggerRequest::Manual),
        "message_event" => {
            let pattern = string_field(params, "request", "pattern", &["event_pattern"])
                .ok_or_else(|| {
                    RoutinesCapabilityError::input(
                        "message_event request requires 'pattern'".to_string(),
                    )
                })?;
            regex::RegexBuilder::new(&pattern)
                .size_limit(64 * 1024)
                .build()
                .map_err(|e| {
                    RoutinesCapabilityError::input(format!("invalid or too complex regex: {e}"))
                })?;
            let channel = string_field(params, "request", "channel", &["event_channel"]);
            Ok(NormalizedTriggerRequest::MessageEvent { pattern, channel })
        }
        "system_event" => {
            let source =
                string_field(params, "request", "source", &["event_source"]).ok_or_else(|| {
                    RoutinesCapabilityError::input(
                        "system_event request requires 'source'".to_string(),
                    )
                })?;
            let event_type = string_field(params, "request", "event_type", &["event_type"])
                .ok_or_else(|| {
                    RoutinesCapabilityError::input(
                        "system_event request requires 'event_type'".to_string(),
                    )
                })?;
            let filters = parse_system_event_filters(object_field(
                params,
                "request",
                "filters",
                &["event_filters"],
            ))?;
            Ok(NormalizedTriggerRequest::SystemEvent {
                source,
                event_type,
                filters,
            })
        }
        "webhook" => {
            let path = string_field(params, "request", "path", &["webhook_path"]);
            let secret = string_field(params, "request", "secret", &["webhook_secret"]);
            Ok(NormalizedTriggerRequest::Webhook { path, secret })
        }
        other => Err(RoutinesCapabilityError::input(format!(
            "unknown request.kind: {other}"
        ))),
    }
}

fn parse_execution_mode(value: Option<String>) -> Result<NormalizedExecutionMode, RoutinesCapabilityError> {
    match value.as_deref().unwrap_or("lightweight") {
        "lightweight" => Ok(NormalizedExecutionMode::Lightweight),
        "full_job" => Ok(NormalizedExecutionMode::FullJob),
        other => Err(RoutinesCapabilityError::input(format!(
            "unknown execution mode: {other}"
        ))),
    }
}

fn parse_routine_execution(
    params: &Value,
    default_use_tools: bool,
) -> Result<NormalizedExecutionRequest, RoutinesCapabilityError> {
    let mode = parse_execution_mode(string_field(params, "execution", "mode", &["action_type"]))?;
    let context_paths =
        string_array_field(params, "execution", "context_paths", &["context_paths"]);
    let use_tools =
        bool_field(params, "execution", "use_tools", &["use_tools"]).unwrap_or(default_use_tools);
    let max_tool_rounds = u64_field(params, "execution", "max_tool_rounds", &["max_tool_rounds"])
        .unwrap_or(3)
        .clamp(1, crate::agent::routine::MAX_TOOL_ROUNDS_LIMIT as u64)
        as u32;

    let max_iterations = u64_field(params, "execution", "max_iterations", &["max_iterations"])
        .unwrap_or(25)
        .clamp(1, 200) as u32;

    Ok(NormalizedExecutionRequest {
        mode,
        context_paths,
        use_tools,
        max_tool_rounds,
        max_iterations,
    })
}

fn parse_routine_delivery(params: &Value) -> NormalizedDeliveryRequest {
    NormalizedDeliveryRequest {
        channel: string_field(params, "delivery", "channel", &["notify_channel"]),
        user: string_field(params, "delivery", "user", &["notify_user"]),
    }
}

fn parse_routine_create_request(
    params: &Value,
) -> Result<NormalizedRoutineCreateRequest, RoutinesCapabilityError> {
    let name = require_str(params, "name")?.to_string();
    let prompt = require_str(params, "prompt")?.to_string();
    let description = params
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let trigger = parse_routine_trigger(params)?;
    let execution = parse_routine_execution(params, true)?;
    let delivery = parse_routine_delivery(params);
    let cooldown_secs =
        u64_field(params, "advanced", "cooldown_secs", &["cooldown_secs"]).unwrap_or(300);

    Ok(NormalizedRoutineCreateRequest {
        name,
        description,
        prompt,
        trigger,
        execution,
        delivery,
        cooldown_secs,
    })
}

fn build_routine_trigger(trigger: &NormalizedTriggerRequest) -> Trigger {
    match trigger {
        NormalizedTriggerRequest::Cron { schedule, timezone } => Trigger::Cron {
            schedule: normalize_cron_expression(schedule),
            timezone: timezone.clone(),
        },
        NormalizedTriggerRequest::Manual => Trigger::Manual,
        NormalizedTriggerRequest::MessageEvent { pattern, channel } => Trigger::Event {
            channel: channel.clone(),
            pattern: pattern.clone(),
        },
        NormalizedTriggerRequest::SystemEvent {
            source,
            event_type,
            filters,
        } => Trigger::SystemEvent {
            source: source.clone(),
            event_type: event_type.clone(),
            filters: filters.clone(),
        },
        NormalizedTriggerRequest::Webhook { path, secret } => Trigger::Webhook {
            path: path.clone(),
            secret: secret.clone(),
        },
    }
}

fn build_routine_action(
    name: &str,
    prompt: &str,
    execution: &NormalizedExecutionRequest,
) -> RoutineAction {
    match execution.mode {
        NormalizedExecutionMode::Lightweight => RoutineAction::Lightweight {
            prompt: prompt.to_string(),
            context_paths: execution.context_paths.clone(),
            max_tokens: 4096,
            use_tools: execution.use_tools,
            max_tool_rounds: execution.max_tool_rounds,
        },
        NormalizedExecutionMode::FullJob => RoutineAction::FullJob {
            title: name.to_string(),
            description: prompt.to_string(),
            max_iterations: execution.max_iterations,
        },
    }
}

fn verification_result_payload(routine: &Routine, verification_reset: bool) -> Value {
    let verification_status = routine_verification_status(routine);
    json!({
        "verification_status": verification_status.as_str(),
        "verification_reset": verification_reset,
        "verification_hint": if verification_reset {
            "The routine configuration changed and should be re-tested before being treated as reliable."
        } else if verification_status == crate::agent::routine::RoutineVerificationStatus::Verified {
            "The current routine configuration has already been verified with a successful run."
        } else {
            "The routine has been saved, but it has not been verified yet. Offer to test it now."
        }
    })
}

pub async fn execute_routine_create(
    params: &Value,
    ctx: &RoutinesContext,
) -> Result<Value, RoutinesCapabilityError> {
    let normalized = parse_routine_create_request(params)?;
    let trigger = build_routine_trigger(&normalized.trigger);
    let action = build_routine_action(&normalized.name, &normalized.prompt, &normalized.execution);

    let next_fire = if let Trigger::Cron {
        ref schedule,
        ref timezone,
    } = trigger
    {
        next_cron_fire(schedule, timezone.as_deref()).unwrap_or(None)
    } else {
        None
    };

    let mut routine = Routine {
        id: Uuid::new_v4(),
        name: normalized.name.clone(),
        description: normalized.description.clone(),
        user_id: ctx.user_id.clone(),
        enabled: true,
        trigger,
        action,
        guardrails: RoutineGuardrails {
            cooldown: Duration::from_secs(normalized.cooldown_secs),
            max_concurrent: 1,
            dedup_window: None,
        },
        notify: NotifyConfig {
            channel: normalized.delivery.channel.clone().or_else(|| {
                ctx.metadata
                    .get("notify_channel")
                    .and_then(|v| v.as_str())
                    .map(ToOwned::to_owned)
            }),
            user: normalized.delivery.user.clone().or_else(|| {
                ctx.metadata
                    .get("notify_user")
                    .and_then(|v| v.as_str())
                    .filter(|v| *v != "default")
                    .map(ToOwned::to_owned)
            }),
            ..NotifyConfig::default()
        },
        last_run_at: None,
        next_fire_at: next_fire,
        run_count: 0,
        consecutive_failures: 0,
        state: json!({}),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    routine.state = reset_routine_verification_state(
        &routine.state,
        routine_verification_fingerprint(&routine),
    );

    ctx.store
        .create_routine(&routine)
        .await
        .map_err(|e| RoutinesCapabilityError::operation(format!("failed to create routine: {e}")))?;

    if matches!(
        routine.trigger,
        Trigger::Event { .. } | Trigger::SystemEvent { .. }
    ) {
        ctx.engine.refresh_event_cache().await;
    }

    let verification = verification_result_payload(&routine, false);
    Ok(json!({
        "id": routine.id.to_string(),
        "name": routine.name.clone(),
        "trigger_type": routine.trigger.type_tag(),
        "next_fire_at": routine.next_fire_at.map(|t| t.to_rfc3339()),
        "status": "created",
        "verification": verification,
    }))
}

pub async fn execute_routine_update(
    params: &Value,
    ctx: &RoutinesContext,
) -> Result<Value, RoutinesCapabilityError> {
    let name = require_str(params, "name")?;

    let mut routine = ctx
        .store
        .get_routine_by_name(&ctx.user_id, name)
        .await
        .map_err(|e| RoutinesCapabilityError::operation(format!("DB error: {e}")))?
        .ok_or_else(|| RoutinesCapabilityError::operation(format!("routine '{}' not found", name)))?;

    let original_fingerprint = routine_verification_fingerprint(&routine);
    let mut verification_reset = false;

    if let Some(enabled) = params.get("enabled").and_then(|v| v.as_bool()) {
        routine.enabled = enabled;
    }

    if let Some(desc) = params.get("description").and_then(|v| v.as_str()) {
        routine.description = desc.to_string();
    }

    if let Some(prompt) = params.get("prompt").and_then(|v| v.as_str()) {
        match &mut routine.action {
            RoutineAction::Lightweight { prompt: p, .. } => {
                if p != prompt {
                    verification_reset = true;
                    *p = prompt.to_string();
                }
            }
            RoutineAction::FullJob { description: d, .. } => {
                if d != prompt {
                    verification_reset = true;
                    *d = prompt.to_string();
                }
            }
        }
    }

    if let Some(iters) = params.get("max_iterations").and_then(|v| v.as_u64())
        && let RoutineAction::FullJob { max_iterations, .. } = &mut routine.action
    {
        *max_iterations = (iters.clamp(1, 200)) as u32;
    }

    let new_timezone = params
        .get("timezone")
        .and_then(|v| v.as_str())
        .map(|tz| {
            crate::timezone::parse_timezone(tz)
                .map(|_| tz.to_string())
                .ok_or_else(|| {
                    RoutinesCapabilityError::input(format!("invalid IANA timezone: '{tz}'"))
                })
        })
        .transpose()?;

    let new_schedule = params
        .get("schedule")
        .and_then(|v| v.as_str())
        .map(normalize_cron_expression);

    if new_schedule.is_some() || new_timezone.is_some() {
        let existing_cron = match &routine.trigger {
            Trigger::Cron { schedule, timezone } => Some((schedule.clone(), timezone.clone())),
            _ => None,
        };

        if let Some((old_schedule, old_tz)) = existing_cron {
            let effective_schedule = new_schedule.as_deref().unwrap_or(&old_schedule);
            let effective_tz = new_timezone.clone().or(old_tz.clone());
            next_cron_fire(effective_schedule, effective_tz.as_deref()).map_err(|e| {
                RoutinesCapabilityError::input(format!("invalid cron schedule: {e}"))
            })?;

            if effective_schedule != old_schedule || effective_tz != old_tz {
                verification_reset = true;
            }

            routine.trigger = Trigger::Cron {
                schedule: effective_schedule.to_string(),
                timezone: effective_tz.clone(),
            };
            routine.next_fire_at =
                next_cron_fire(effective_schedule, effective_tz.as_deref()).unwrap_or(None);
        } else {
            return Err(RoutinesCapabilityError::input(
                "Cannot update schedule or timezone on a non-cron routine.".to_string(),
            ));
        }
    }

    let updated_fingerprint = routine_verification_fingerprint(&routine);
    if updated_fingerprint != original_fingerprint {
        verification_reset = true;
        routine.state = reset_routine_verification_state(&routine.state, updated_fingerprint);
    }

    ctx.store
        .update_routine(&routine)
        .await
        .map_err(|e| RoutinesCapabilityError::operation(format!("failed to update: {e}")))?;

    ctx.engine.refresh_event_cache().await;

    let verification = verification_result_payload(&routine, verification_reset);
    Ok(json!({
        "name": routine.name.clone(),
        "enabled": routine.enabled,
        "trigger_type": routine.trigger.type_tag(),
        "next_fire_at": routine.next_fire_at.map(|t| t.to_rfc3339()),
        "status": "updated",
        "verification": verification,
    }))
}

pub async fn execute_routine_delete(
    params: &Value,
    ctx: &RoutinesContext,
) -> Result<Value, RoutinesCapabilityError> {
    let name = require_str(params, "name")?;

    let routine = ctx
        .store
        .get_routine_by_name(&ctx.user_id, name)
        .await
        .map_err(|e| RoutinesCapabilityError::operation(format!("DB error: {e}")))?
        .ok_or_else(|| RoutinesCapabilityError::operation(format!("routine '{}' not found", name)))?;

    let deleted = ctx
        .store
        .delete_routine(routine.id)
        .await
        .map_err(|e| RoutinesCapabilityError::operation(format!("failed to delete: {e}")))?;

    ctx.engine.refresh_event_cache().await;

    Ok(json!({
        "name": name,
        "deleted": deleted,
    }))
}

pub async fn execute_routine_list(
    _params: &Value,
    ctx: &RoutinesContext,
) -> Result<Value, RoutinesCapabilityError> {
    let routines = ctx
        .store
        .list_routines(&ctx.user_id)
        .await
        .map_err(|e| RoutinesCapabilityError::operation(format!("failed to list routines: {e}")))?;
    let routine_ids: Vec<Uuid> = routines.iter().map(|r| r.id).collect();
    let last_run_statuses = ctx
        .store
        .batch_get_last_run_status(&routine_ids)
        .await
        .map_err(|e| {
            RoutinesCapabilityError::operation(format!("failed to read routine statuses: {e}"))
        })?;

    let list: Vec<Value> = routines
        .iter()
        .map(|r| {
            let verification_status = routine_verification_status(r);
            let status = crate::agent::routine::routine_display_status_for_verification(
                r,
                verification_status,
                last_run_statuses.get(&r.id).copied(),
            );
            json!({
                "id": r.id.to_string(),
                "name": r.name,
                "description": r.description,
                "enabled": r.enabled,
                "trigger_type": r.trigger.type_tag(),
                "action_type": r.action.type_tag(),
                "last_run_at": r.last_run_at.map(|t| t.to_rfc3339()),
                "next_fire_at": r.next_fire_at.map(|t| t.to_rfc3339()),
                "run_count": r.run_count,
                "consecutive_failures": r.consecutive_failures,
                "status": status.as_str(),
                "verification_status": verification_status.as_str(),
            })
        })
        .collect();

    Ok(json!({
        "count": list.len(),
        "routines": list,
    }))
}

pub async fn execute_routine_history(
    params: &Value,
    ctx: &RoutinesContext,
) -> Result<Value, RoutinesCapabilityError> {
    let name = require_str(params, "name")?;

    let limit = params
        .get("limit")
        .and_then(|v| v.as_i64())
        .unwrap_or(10)
        .min(50);

    let routine = ctx
        .store
        .get_routine_by_name(&ctx.user_id, name)
        .await
        .map_err(|e| RoutinesCapabilityError::operation(format!("DB error: {e}")))?
        .ok_or_else(|| RoutinesCapabilityError::operation(format!("routine '{}' not found", name)))?;

    let runs = ctx
        .store
        .list_routine_runs(routine.id, limit)
        .await
        .map_err(|e| RoutinesCapabilityError::operation(format!("failed to list runs: {e}")))?;

    let run_list: Vec<Value> = runs
        .iter()
        .map(|r| {
            let duration_secs = r
                .completed_at
                .map(|c| c.signed_duration_since(r.started_at).num_seconds());
            json!({
                "id": r.id.to_string(),
                "trigger_type": r.trigger_type,
                "trigger_detail": r.trigger_detail,
                "started_at": r.started_at.to_rfc3339(),
                "completed_at": r.completed_at.map(|t| t.to_rfc3339()),
                "duration_secs": duration_secs,
                "status": r.status.to_string(),
                "result_summary": r.result_summary,
                "tokens_used": r.tokens_used,
            })
        })
        .collect();

    let (conversation_id, recent_output) = match ctx
        .store
        .get_or_create_routine_conversation(routine.id, name, &ctx.user_id)
        .await
    {
        Ok(conv_id) => {
            let messages = ctx
                .store
                .list_conversation_messages_paginated(conv_id, None, limit)
                .await
                .map(|(msgs, _)| msgs)
                .unwrap_or_default();
            let msg_list: Vec<Value> = messages
                .iter()
                .map(|m| {
                    json!({
                        "role": m.role,
                        "content": m.content,
                        "timestamp": m.created_at.to_rfc3339(),
                    })
                })
                .collect();
            (Some(conv_id.to_string()), msg_list)
        }
        Err(_) => (None, Vec::new()),
    };

    Ok(json!({
        "routine": name,
        "total_runs": routine.run_count,
        "conversation_id": conversation_id,
        "runs": run_list,
        "recent_output": recent_output,
    }))
}

pub async fn execute_routine_fire(
    params: &Value,
    ctx: &RoutinesContext,
) -> Result<Value, RoutinesCapabilityError> {
    let name = require_str(params, "name")?;

    let routine = ctx
        .store
        .get_routine_by_name(&ctx.user_id, name)
        .await
        .map_err(|e| RoutinesCapabilityError::operation(format!("DB error: {e}")))?
        .ok_or_else(|| RoutinesCapabilityError::operation(format!("routine '{}' not found", name)))?;

    let run_id = ctx
        .engine
        .fire_manual(routine.id, None)
        .await
        .map_err(|e| {
            RoutinesCapabilityError::operation(format!("failed to fire routine '{}': {e}", name))
        })?;

    Ok(json!({
        "name": name,
        "run_id": run_id.to_string(),
        "status": "fired",
        "note": "Routine is executing asynchronously. Use routine_history to check the result.",
    }))
}

pub async fn execute_event_emit(
    params: &Value,
    ctx: &RoutinesContext,
) -> Result<Value, RoutinesCapabilityError> {
    let source = params
        .get("event_source")
        .and_then(Value::as_str)
        .or_else(|| params.get("source").and_then(Value::as_str))
        .ok_or_else(|| {
            RoutinesCapabilityError::input(
                "event_emit requires 'event_source' (canonical) or 'source' (alias)".to_string(),
            )
        })?
        .to_string();
    let event_type = require_str(params, "event_type")?.to_string();
    let payload = params
        .get("payload")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let fired = ctx
        .engine
        .emit_system_event(&source, &event_type, &payload, Some(&ctx.user_id))
        .await;

    Ok(json!({
        "event_source": &source,
        "event_type": &event_type,
        "user_id": &ctx.user_id,
        "fired_routines": fired,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routine_create_descriptor_is_valid() {
        let desc = routine_create_descriptor();
        assert_eq!(desc.id.as_str(), ROUTINE_CREATE_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::DispatchCapability));
        assert_eq!(desc.default_permission, PermissionMode::Ask);
    }

    #[test]
    fn routine_update_descriptor_is_valid() {
        let desc = routine_update_descriptor();
        assert_eq!(desc.id.as_str(), ROUTINE_UPDATE_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::DispatchCapability));
        assert_eq!(desc.default_permission, PermissionMode::Ask);
    }

    #[test]
    fn routine_delete_descriptor_is_valid() {
        let desc = routine_delete_descriptor();
        assert_eq!(desc.id.as_str(), ROUTINE_DELETE_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::DispatchCapability));
        assert_eq!(desc.default_permission, PermissionMode::Ask);
    }

    #[test]
    fn routine_list_descriptor_is_valid() {
        let desc = routine_list_descriptor();
        assert_eq!(desc.id.as_str(), ROUTINE_LIST_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::DispatchCapability));
        assert_eq!(desc.default_permission, PermissionMode::Allow);
    }

    #[test]
    fn routine_history_descriptor_is_valid() {
        let desc = routine_history_descriptor();
        assert_eq!(desc.id.as_str(), ROUTINE_HISTORY_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::DispatchCapability));
        assert_eq!(desc.default_permission, PermissionMode::Allow);
    }

    #[test]
    fn routine_fire_descriptor_is_valid() {
        let desc = routine_fire_descriptor();
        assert_eq!(desc.id.as_str(), ROUTINE_FIRE_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::DispatchCapability));
        assert_eq!(desc.default_permission, PermissionMode::Ask);
    }

    #[test]
    fn event_emit_descriptor_is_valid() {
        let desc = event_emit_descriptor();
        assert_eq!(desc.id.as_str(), EVENT_EMIT_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::DispatchCapability));
        assert_eq!(desc.default_permission, PermissionMode::Ask);
    }

    #[test]
    fn descriptors_returns_all_seven() {
        let descs = descriptors();
        assert_eq!(descs.len(), 7);
        let ids: Vec<&str> = descs.iter().map(|d| d.id.as_str()).collect();
        assert!(ids.contains(&ROUTINE_CREATE_CAPABILITY_ID));
        assert!(ids.contains(&ROUTINE_UPDATE_CAPABILITY_ID));
        assert!(ids.contains(&ROUTINE_DELETE_CAPABILITY_ID));
        assert!(ids.contains(&ROUTINE_LIST_CAPABILITY_ID));
        assert!(ids.contains(&ROUTINE_HISTORY_CAPABILITY_ID));
        assert!(ids.contains(&ROUTINE_FIRE_CAPABILITY_ID));
        assert!(ids.contains(&EVENT_EMIT_CAPABILITY_ID));
    }

    #[test]
    fn routine_create_schema_has_required_fields() {
        let desc = routine_create_descriptor();
        let required = desc.parameters_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "name"));
        assert!(required.iter().any(|v| v == "prompt"));
        assert!(required.iter().any(|v| v == "request"));
    }

    #[test]
    fn routine_update_schema_requires_name() {
        let desc = routine_update_descriptor();
        let required = desc.parameters_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "name"));
    }

    #[test]
    fn event_emit_schema_requires_event_source_and_type() {
        let desc = event_emit_descriptor();
        let required = desc.parameters_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "event_source"));
        assert!(required.iter().any(|v| v == "event_type"));
    }
}
