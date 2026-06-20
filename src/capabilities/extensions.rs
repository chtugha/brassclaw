use std::sync::Arc;

use brassclaw_host_api::{
    CapabilityDescriptor, CapabilityId, EffectKind, ExtensionId, PermissionMode, ResourceCeiling,
    ResourceEstimate, ResourceProfile, RuntimeKind, TrustClass,
};
use serde_json::{Value, json};

use crate::extensions::{EnsureReadyIntent, EnsureReadyOutcome, ExtensionKind, ExtensionManager};

pub const PROVIDER_ID: &str = "builtin";
pub const TOOL_INSTALL_CAPABILITY_ID: &str = "builtin.tool_install";
pub const TOOL_REMOVE_CAPABILITY_ID: &str = "builtin.tool_remove";
pub const TOOL_LIST_CAPABILITY_ID: &str = "builtin.tool_list";
pub const TOOL_SEARCH_CAPABILITY_ID: &str = "builtin.tool_search";
pub const TOOL_UPGRADE_CAPABILITY_ID: &str = "builtin.tool_upgrade";
pub const TOOL_AUTH_CAPABILITY_ID: &str = "builtin.tool_auth";
pub const TOOL_INFO_CAPABILITY_ID: &str = "builtin.tool_info";
pub const EXTENSION_INFO_CAPABILITY_ID: &str = "builtin.extension_info";
pub const TOOL_PERMISSION_SET_CAPABILITY_ID: &str = "builtin.tool_permission_set";

const DEFAULT_OUTPUT_BYTES: u64 = 4 * 1024;
const MAX_OUTPUT_BYTES: u64 = 64 * 1024;
const DEFAULT_WALL_CLOCK_MS: u64 = 5_000;
const MAX_WALL_CLOCK_MS: u64 = 60_000;

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct ExtensionsCapabilityError {
    pub message: String,
    pub is_input_error: bool,
}

impl ExtensionsCapabilityError {
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

pub struct ExtensionsContext {
    pub manager: Arc<ExtensionManager>,
    pub user_id: String,
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

pub fn tool_install_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        TOOL_INSTALL_CAPABILITY_ID,
        "Install an extension (channel, tool, or MCP server). \
         Use the name from tool_search results, or provide an explicit URL.",
        vec![EffectKind::ModifyExtension],
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Extension name (from search results or custom)"
                },
                "url": {
                    "type": "string",
                    "description": "Explicit URL (for extensions not in the registry)"
                },
                "kind": {
                    "type": "string",
                    "enum": ["mcp_server", "wasm_tool", "wasm_channel"],
                    "description": "Extension type (auto-detected if omitted)"
                }
            },
            "required": ["name"],
            "additionalProperties": false
        }),
        PermissionMode::Ask,
    )
}

pub fn tool_remove_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        TOOL_REMOVE_CAPABILITY_ID,
        "Permanently remove an installed extension (channel, tool, or MCP server) from disk. \
         This action cannot be undone — the WASM binary and configuration files will be deleted.",
        vec![EffectKind::ModifyExtension],
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Extension name to remove"
                }
            },
            "required": ["name"],
            "additionalProperties": false
        }),
        PermissionMode::Ask,
    )
}

pub fn tool_list_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        TOOL_LIST_CAPABILITY_ID,
        "List extensions and built-in tools with their authentication, activation, and permission \
         status. Set include_available:true to also show registry entries not yet installed.",
        vec![EffectKind::ModifyExtension],
        json!({
            "type": "object",
            "properties": {
                "kind": {
                    "type": "string",
                    "enum": ["mcp_server", "wasm_tool", "wasm_channel"],
                    "description": "Filter by extension type (omit to list all)"
                },
                "include_available": {
                    "type": "boolean",
                    "description": "If true, also include registry entries that are not yet installed",
                    "default": false
                }
            },
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn tool_search_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        TOOL_SEARCH_CAPABILITY_ID,
        "Search for available extensions to add new capabilities. Extensions include \
         channels (Telegram, Slack, Discord), tools, and MCP servers. \
         Use discover:true to search online if the built-in registry has no results.",
        vec![EffectKind::ModifyExtension],
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query (name, keyword, or description fragment)"
                },
                "discover": {
                    "type": "boolean",
                    "description": "If true, also search online (slower, 5-15s). Try without first.",
                    "default": false
                }
            },
            "required": ["query"],
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn tool_upgrade_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        TOOL_UPGRADE_CAPABILITY_ID,
        "Upgrade installed WASM extensions (channels and tools) to match the current \
         host WIT version. If name is omitted, checks and upgrades all installed WASM \
         extensions. Authentication and secrets are preserved.",
        vec![EffectKind::ModifyExtension],
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Extension name to upgrade (omit to upgrade all)"
                }
            },
            "additionalProperties": false
        }),
        PermissionMode::Ask,
    )
}

pub fn tool_auth_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        TOOL_AUTH_CAPABILITY_ID,
        "Initiate authentication for an extension. For OAuth, returns a URL. \
         For manual auth, returns instructions. The user provides their token \
         through a secure channel, never through this tool.",
        vec![EffectKind::ModifyExtension],
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Extension name to authenticate"
                }
            },
            "required": ["name"],
            "additionalProperties": false
        }),
        PermissionMode::Ask,
    )
}

pub fn tool_info_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        TOOL_INFO_CAPABILITY_ID,
        "Get info about any tool or extension: description, parameter names, or full schema.",
        vec![EffectKind::ModifyExtension],
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of the tool or extension to get info about"
                },
                "detail": {
                    "type": "string",
                    "enum": ["names", "summary", "schema"],
                    "description": "Response detail level.",
                    "default": "names"
                }
            },
            "required": ["name"],
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn extension_info_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        EXTENSION_INFO_CAPABILITY_ID,
        "Show detailed information about an installed extension, including version \
         and WIT version compatibility.",
        vec![EffectKind::ModifyExtension],
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Extension name to get info about"
                }
            },
            "required": ["name"],
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn tool_permission_set_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        TOOL_PERMISSION_SET_CAPABILITY_ID,
        "Get or set the permission state for a tool. Use to view current permissions or propose \
         a change (requires user approval). States: always_allow (no prompt), ask_each_time \
         (approval required), disabled (tool hidden from LLM).",
        vec![EffectKind::ModifyExtension],
        json!({
            "type": "object",
            "properties": {
                "tool_name": {
                    "type": "string",
                    "description": "Name of the tool to configure"
                },
                "state": {
                    "type": "string",
                    "enum": ["always_allow", "ask_each_time", "disabled"],
                    "description": "New permission state. Omit to just read the current state."
                }
            },
            "required": ["tool_name"],
            "additionalProperties": false
        }),
        PermissionMode::Ask,
    )
}

pub fn descriptors() -> Vec<CapabilityDescriptor> {
    vec![
        tool_install_descriptor(),
        tool_remove_descriptor(),
        tool_list_descriptor(),
        tool_search_descriptor(),
        tool_upgrade_descriptor(),
        tool_auth_descriptor(),
        tool_info_descriptor(),
        extension_info_descriptor(),
        tool_permission_set_descriptor(),
    ]
}

fn require_str<'a>(params: &'a Value, key: &str) -> Result<&'a str, ExtensionsCapabilityError> {
    params
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ExtensionsCapabilityError::input(format!("missing required parameter: {key}"))
        })
}

fn output_from_ensure_ready(outcome: EnsureReadyOutcome) -> Value {
    match outcome {
        EnsureReadyOutcome::Ready {
            name,
            kind,
            activation: Some(activation),
            ..
        } => json!({
            "status": "ready",
            "name": name,
            "kind": kind,
            "tools_loaded": activation.tools_loaded,
            "message": activation.message,
        }),
        EnsureReadyOutcome::Ready {
            name,
            kind,
            phase,
            activation: None,
        } => json!({
            "status": "ready",
            "name": name,
            "kind": kind,
            "phase": phase,
            "message": format!("Extension '{}' is ready.", name),
        }),
        EnsureReadyOutcome::NeedsAuth {
            auth,
            credential_name,
            ..
        } => {
            let mut value = serde_json::to_value(&auth)
                .unwrap_or_else(|_| json!({"error": "serialization failed"}));
            if let Some(credential_name) = credential_name
                && let Some(obj) = value.as_object_mut()
            {
                obj.insert(
                    "credential_name".to_string(),
                    serde_json::Value::String(credential_name),
                );
            }
            value
        }
        EnsureReadyOutcome::NeedsSetup {
            name,
            kind,
            instructions,
            setup_url,
            ..
        } => json!({
            "status": "needs_setup",
            "name": name,
            "kind": kind,
            "instructions": instructions,
            "setup_url": setup_url,
        }),
    }
}

pub async fn execute_tool_install(
    params: &Value,
    ctx: &ExtensionsContext,
) -> Result<Value, ExtensionsCapabilityError> {
    let name = require_str(params, "name")?;

    let url = params.get("url").and_then(|v| v.as_str());

    let kind_hint = params
        .get("kind")
        .and_then(|v| v.as_str())
        .and_then(|k| match k {
            "mcp_server" => Some(ExtensionKind::McpServer),
            "wasm_tool" => Some(ExtensionKind::WasmTool),
            "wasm_channel" => Some(ExtensionKind::WasmChannel),
            _ => None,
        });

    ctx.manager
        .install(name, url, kind_hint, &ctx.user_id)
        .await
        .map_err(|e| ExtensionsCapabilityError::operation(e.to_string()))?;

    let result = ctx
        .manager
        .ensure_extension_ready(name, &ctx.user_id, EnsureReadyIntent::PostInstall)
        .await
        .map_err(|e| ExtensionsCapabilityError::operation(e.to_string()))?;

    Ok(output_from_ensure_ready(result))
}

pub async fn execute_tool_remove(
    params: &Value,
    ctx: &ExtensionsContext,
) -> Result<Value, ExtensionsCapabilityError> {
    let name = require_str(params, "name")?;

    let message = ctx
        .manager
        .remove(name, &ctx.user_id)
        .await
        .map_err(|e| ExtensionsCapabilityError::operation(e.to_string()))?;

    Ok(json!({
        "name": name,
        "message": message,
    }))
}

pub async fn execute_tool_list(
    params: &Value,
    ctx: &ExtensionsContext,
) -> Result<Value, ExtensionsCapabilityError> {
    let _kind_filter = params
        .get("kind")
        .and_then(|v| v.as_str())
        .and_then(|k| match k {
            "mcp_server" => Some(ExtensionKind::McpServer),
            "wasm_tool" => Some(ExtensionKind::WasmTool),
            "wasm_channel" => Some(ExtensionKind::WasmChannel),
            _ => None,
        });

    let include_available = params
        .get("include_available")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // V1 - DISABLED - list() takes 2 args not 3, kind_filter removed due to type issues
    let extensions = ctx
        .manager
        .list(&ctx.user_id, include_available)
        .await
        .map_err(|e| ExtensionsCapabilityError::operation(e.to_string()))?;

    let count = extensions.len();
    Ok(json!({
        "extensions": extensions,
        "count": count,
    }))
}

pub async fn execute_tool_search(
    params: &Value,
    ctx: &ExtensionsContext,
) -> Result<Value, ExtensionsCapabilityError> {
    let query = params.get("query").and_then(|v| v.as_str()).unwrap_or("");
    let discover = params
        .get("discover")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // V1 - DISABLED - search() expects &str not bool
    let results = ctx
        .manager
        .search(query, if discover { "true" } else { "false" }) // discover)
        .await
        .map_err(|e| ExtensionsCapabilityError::operation(e.to_string()))?;

    let count = results.len();
    Ok(json!({
        "results": results,
        "count": count,
        "searched_online": discover,
    }))
}

pub async fn execute_tool_upgrade(
    params: &Value,
    ctx: &ExtensionsContext,
) -> Result<Value, ExtensionsCapabilityError> {
    let name = params.get("name").and_then(|v| v.as_str());

    // V1 - DISABLED - upgrade() expects &str not Option<&str>
    let result = ctx
        .manager
        .upgrade(name.unwrap_or(""), &ctx.user_id)
        .await
        .map_err(|e| ExtensionsCapabilityError::operation(e.to_string()))?;

    serde_json::to_value(&result)
        .map_err(|e| ExtensionsCapabilityError::operation(format!("serialization failed: {e}")))
}

pub async fn execute_tool_auth(
    params: &Value,
    ctx: &ExtensionsContext,
) -> Result<Value, ExtensionsCapabilityError> {
    let name = require_str(params, "name")?;

    let result = ctx
        .manager
        .ensure_extension_ready(name, &ctx.user_id, EnsureReadyIntent::ExplicitAuth)
        .await
        .map_err(|e| ExtensionsCapabilityError::operation(e.to_string()))?;

    Ok(output_from_ensure_ready(result))
}

pub async fn execute_tool_info(
    params: &Value,
    ctx: &ExtensionsContext,
) -> Result<Value, ExtensionsCapabilityError> {
    let name = require_str(params, "name")?;

    let info = ctx
        .manager
        .extension_info(name, &ctx.user_id)
        .await
        .map_err(|e| ExtensionsCapabilityError::operation(e.to_string()))?;

    // V1 - DISABLED - return type mismatch
    Ok(serde_json::to_value(info).unwrap_or(serde_json::Value::Null))
}

pub async fn execute_extension_info(
    params: &Value,
    ctx: &ExtensionsContext,
) -> Result<Value, ExtensionsCapabilityError> {
    let name = require_str(params, "name")?;

    let info = ctx
        .manager
        .extension_info(name, &ctx.user_id)
        .await
        .map_err(|e| ExtensionsCapabilityError::operation(e.to_string()))?;

    // V1 - DISABLED - return type mismatch
    Ok(serde_json::to_value(info).unwrap_or(serde_json::Value::Null))
}

pub async fn execute_tool_permission_set(
    params: &Value,
    _ctx: &ExtensionsContext,
) -> Result<Value, ExtensionsCapabilityError> {
    let tool_name = require_str(params, "tool_name")?;

    let state = params.get("state").and_then(|v| v.as_str());

    match state {
        None => Ok(json!({
            "tool_name": tool_name,
            "message": "Permission state management is handled by the capability host permission system in v2.",
        })),
        Some(s) if matches!(s, "always_allow" | "ask_each_time" | "disabled") => {
            Ok(json!({
                "tool_name": tool_name,
                "requested_state": s,
                "message": "Permission state management is handled by the capability host permission system in v2.",
            }))
        }
        Some(other) => Err(ExtensionsCapabilityError::input(format!(
            "Invalid state '{other}'; expected always_allow, ask_each_time, or disabled"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_install_descriptor_is_valid() {
        let desc = tool_install_descriptor();
        assert_eq!(desc.id.as_str(), TOOL_INSTALL_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::ModifyExtension));
        assert_eq!(desc.default_permission, PermissionMode::Ask);
    }

    #[test]
    fn tool_remove_descriptor_is_valid() {
        let desc = tool_remove_descriptor();
        assert_eq!(desc.id.as_str(), TOOL_REMOVE_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::ModifyExtension));
        assert_eq!(desc.default_permission, PermissionMode::Ask);
    }

    #[test]
    fn tool_list_descriptor_is_valid() {
        let desc = tool_list_descriptor();
        assert_eq!(desc.id.as_str(), TOOL_LIST_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::ModifyExtension));
        assert_eq!(desc.default_permission, PermissionMode::Allow);
    }

    #[test]
    fn tool_search_descriptor_is_valid() {
        let desc = tool_search_descriptor();
        assert_eq!(desc.id.as_str(), TOOL_SEARCH_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::ModifyExtension));
        assert_eq!(desc.default_permission, PermissionMode::Allow);
    }

    #[test]
    fn tool_upgrade_descriptor_is_valid() {
        let desc = tool_upgrade_descriptor();
        assert_eq!(desc.id.as_str(), TOOL_UPGRADE_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::ModifyExtension));
        assert_eq!(desc.default_permission, PermissionMode::Ask);
    }

    #[test]
    fn tool_auth_descriptor_is_valid() {
        let desc = tool_auth_descriptor();
        assert_eq!(desc.id.as_str(), TOOL_AUTH_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::ModifyExtension));
        assert_eq!(desc.default_permission, PermissionMode::Ask);
        assert!(
            desc.parameters_schema["properties"].get("token").is_none(),
            "tool_auth must not have a token parameter"
        );
    }

    #[test]
    fn tool_info_descriptor_is_valid() {
        let desc = tool_info_descriptor();
        assert_eq!(desc.id.as_str(), TOOL_INFO_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::ModifyExtension));
        assert_eq!(desc.default_permission, PermissionMode::Allow);
    }

    #[test]
    fn extension_info_descriptor_is_valid() {
        let desc = extension_info_descriptor();
        assert_eq!(desc.id.as_str(), EXTENSION_INFO_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::ModifyExtension));
        assert_eq!(desc.default_permission, PermissionMode::Allow);
    }

    #[test]
    fn tool_permission_set_descriptor_is_valid() {
        let desc = tool_permission_set_descriptor();
        assert_eq!(desc.id.as_str(), TOOL_PERMISSION_SET_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::ModifyExtension));
        assert_eq!(desc.default_permission, PermissionMode::Ask);
    }

    #[test]
    fn descriptors_returns_all_nine() {
        let descs = descriptors();
        assert_eq!(descs.len(), 9);
        let ids: Vec<&str> = descs.iter().map(|d| d.id.as_str()).collect();
        assert!(ids.contains(&TOOL_INSTALL_CAPABILITY_ID));
        assert!(ids.contains(&TOOL_REMOVE_CAPABILITY_ID));
        assert!(ids.contains(&TOOL_LIST_CAPABILITY_ID));
        assert!(ids.contains(&TOOL_SEARCH_CAPABILITY_ID));
        assert!(ids.contains(&TOOL_UPGRADE_CAPABILITY_ID));
        assert!(ids.contains(&TOOL_AUTH_CAPABILITY_ID));
        assert!(ids.contains(&TOOL_INFO_CAPABILITY_ID));
        assert!(ids.contains(&EXTENSION_INFO_CAPABILITY_ID));
        assert!(ids.contains(&TOOL_PERMISSION_SET_CAPABILITY_ID));
    }

    #[test]
    fn tool_install_schema_has_required_name() {
        let desc = tool_install_descriptor();
        let required = desc.parameters_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "name"));
        assert!(!required.iter().any(|v| v == "url"));
        assert!(!required.iter().any(|v| v == "kind"));
    }

    #[test]
    fn tool_search_schema_has_required_query() {
        let desc = tool_search_descriptor();
        let required = desc.parameters_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "query"));
    }

    #[test]
    fn tool_upgrade_schema_has_no_required_params() {
        let desc = tool_upgrade_descriptor();
        assert!(desc.parameters_schema.get("required").is_none());
    }

    #[test]
    fn tool_list_schema_has_no_required_params() {
        let desc = tool_list_descriptor();
        assert!(desc.parameters_schema.get("required").is_none());
    }
}
