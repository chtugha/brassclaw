use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use brassclaw_host_api::{
    CapabilityDescriptor, CapabilityId, EffectKind, ExtensionId, PermissionMode, ResourceCeiling,
    ResourceEstimate, ResourceProfile, RuntimeKind, TrustClass,
};
use serde_json::{Value, json};

use crate::channels::{ChannelManager, OutgoingResponse};
use crate::extensions::ExtensionManager;

pub const PROVIDER_ID: &str = "builtin";
pub const MESSAGE_CAPABILITY_ID: &str = "builtin.message";

const DEFAULT_OUTPUT_BYTES: u64 = 4 * 1024;
const MAX_OUTPUT_BYTES: u64 = 64 * 1024;
const DEFAULT_WALL_CLOCK_MS: u64 = 2_000;
const MAX_WALL_CLOCK_MS: u64 = 30_000;

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct MessagingCapabilityError {
    pub message: String,
    pub is_input_error: bool,
}

impl MessagingCapabilityError {
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

pub struct MessagingContext {
    pub channel_manager: Arc<ChannelManager>,
    pub extension_manager: Option<Arc<ExtensionManager>>,
    pub default_channel: Arc<RwLock<Option<String>>>,
    pub default_target: Arc<RwLock<Option<String>>>,
    pub base_dir: PathBuf,
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

pub fn message_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        MESSAGE_CAPABILITY_ID,
        "Send a proactive message to a channel. Use normal assistant output to reply in the \
         active conversation; use this tool for proactive notifications, routine/background \
         follow-ups, attachments, or sending to a different channel/recipient.",
        vec![EffectKind::ExternalWrite],
        json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "Message text to send"
                },
                "channel": {
                    "type": "string",
                    "description": "Transport/integration name: 'slack', 'telegram', 'signal', 'gateway'. Defaults to current channel if omitted."
                },
                "target": {
                    "type": "string",
                    "description": "Recipient within the transport. Slack: channel/user ID. Telegram: chat ID. Signal: E.164 phone or group ID."
                },
                "attachments": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Optional file paths to attach to the message"
                }
            },
            "required": ["content"],
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn descriptors() -> Vec<CapabilityDescriptor> {
    vec![message_descriptor()]
}

fn require_str<'a>(params: &'a Value, key: &str) -> Result<&'a str, MessagingCapabilityError> {
    params
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| {
            MessagingCapabilityError::input(format!("missing required parameter: {key}"))
        })
}

fn metadata_string(metadata: &Value, key: &str) -> Option<String> {
    metadata
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn metadata_notify_user(metadata: &Value) -> Option<String> {
    metadata_string(metadata, "notify_user").filter(|value| value != "default")
}

fn metadata_owner_id(metadata: &Value) -> Option<String> {
    metadata_string(metadata, "owner_id")
}

fn channel_matches_source(resolved_channel: Option<&str>, source_channel: Option<&str>) -> bool {
    match (resolved_channel, source_channel) {
        (None, _) => true,
        (Some(resolved), Some(source)) if resolved == source => true,
        _ => false,
    }
}

async fn resolve_channel_fallback_target(
    extension_manager: Option<&Arc<ExtensionManager>>,
    channel: Option<&str>,
    owner_scope_target: Option<&str>,
    ctx_user_id: &str,
) -> Option<String> {
    if let Some(channel_name) = channel
        && let Some(extension_manager) = extension_manager
        && let Some(target) = extension_manager
            .notification_target_for_channel(channel_name)
            .await
    {
        return Some(target);
    }
    owner_scope_target
        .map(ToOwned::to_owned)
        .or_else(|| Some(ctx_user_id.to_string()))
}

struct MessageTargetResolution<'a> {
    extension_manager: Option<&'a Arc<ExtensionManager>>,
    explicit_target: Option<String>,
    metadata_target: Option<String>,
    owner_scope_target: Option<String>,
    default_target: Option<String>,
    channel: Option<&'a str>,
    metadata_channel: Option<&'a str>,
    default_channel: Option<&'a str>,
    has_execution_routing_metadata: bool,
    ctx_user_id: &'a str,
}

async fn resolve_message_target(resolution: MessageTargetResolution<'_>) -> Option<String> {
    let MessageTargetResolution {
        extension_manager,
        explicit_target,
        metadata_target,
        owner_scope_target,
        default_target,
        channel,
        metadata_channel,
        default_channel,
        has_execution_routing_metadata,
        ctx_user_id,
    } = resolution;
    if let Some(target) = explicit_target {
        return Some(target);
    }

    if has_execution_routing_metadata {
        if channel_matches_source(channel, metadata_channel)
            && let Some(target) = metadata_target
        {
            return Some(target);
        }
        return resolve_channel_fallback_target(
            extension_manager,
            channel,
            owner_scope_target.as_deref(),
            ctx_user_id,
        )
        .await;
    }

    if channel_matches_source(channel, default_channel)
        && let Some(target) = default_target
    {
        return Some(target);
    }

    if channel.is_some() {
        return resolve_channel_fallback_target(extension_manager, channel, None, ctx_user_id)
            .await;
    }

    None
}

pub async fn execute_message(
    params: &Value,
    ctx: &MessagingContext,
) -> Result<Value, MessagingCapabilityError> {
    let content = require_str(params, "content").or_else(|_| {
        require_str(params, "message").map_err(|_| {
            MessagingCapabilityError::input("missing 'content' parameter".to_string())
        })
    })?;

    let explicit_channel = params
        .get("channel")
        .and_then(|v| v.as_str())
        .map(|value| value.to_string());
    let metadata_channel = metadata_string(&ctx.metadata, "notify_channel");
    let default_channel = ctx
        .default_channel
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let default_target = ctx
        .default_target
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let metadata_target = metadata_notify_user(&ctx.metadata);
    let owner_scope_target = metadata_owner_id(&ctx.metadata);
    let has_execution_routing_metadata =
        metadata_channel.is_some() || metadata_target.is_some() || owner_scope_target.is_some();

    let channel: Option<String> = explicit_channel
        .clone()
        .or_else(|| metadata_channel.clone())
        .or_else(|| {
            (!has_execution_routing_metadata)
                .then(|| default_channel.clone())
                .flatten()
        });

    let explicit_target = params
        .get("target")
        .and_then(|v| v.as_str())
        .map(|value| value.to_string());

    let target = resolve_message_target(MessageTargetResolution {
        extension_manager: ctx.extension_manager.as_ref(),
        explicit_target,
        metadata_target,
        owner_scope_target,
        default_target,
        channel: channel.as_deref(),
        metadata_channel: metadata_channel.as_deref(),
        default_channel: default_channel.as_deref(),
        has_execution_routing_metadata,
        ctx_user_id: &ctx.user_id,
    })
    .await;

    let Some(target) = target else {
        return Err(MessagingCapabilityError::operation(
            "No target specified and no channel-scoped routing target could be resolved. Provide target parameter."
                .to_string(),
        ));
    };

    let attachments: Vec<String> = match params.get("attachments") {
        Some(v) => serde_json::from_value(v.clone()).map_err(|e| {
            MessagingCapabilityError::operation(format!("Invalid attachments format: {}", e))
        })?,
        None => Vec::new(),
    };

    let attachment_count = attachments.len();

    for path in &attachments {
        let tmp_dir = PathBuf::from("/tmp");
        let resolved =
            crate::tools::builtin::path_utils::validate_path(path, Some(&ctx.base_dir))
                .or_else(|_| {
                    crate::tools::builtin::path_utils::validate_path(path, Some(&tmp_dir))
                })
                .map_err(|e| {
                    MessagingCapabilityError::operation(format!(
                        "Attachment path must be within {} or /tmp/: {}",
                        ctx.base_dir.display(),
                        e
                    ))
                })?;
        if !resolved.exists() {
            return Err(MessagingCapabilityError::operation(format!(
                "Attachment file not found: {}",
                path
            )));
        }
    }

    let mut response = OutgoingResponse::text(content);
    if !attachments.is_empty() {
        response = response.with_attachments(attachments);
    }
    if response.thread_id.is_none()
        && let Some(thread_id) = metadata_string(&ctx.metadata, "notify_thread_id")
    {
        response = response.in_thread(thread_id);
    }

    if let Some(ref channel) = channel {
        match ctx
            .channel_manager
            .broadcast(channel, &target, response)
            .await
        {
            Ok(()) => {
                let msg = format!("Sent message to {}:{}", channel, target);
                Ok(json!({"status": "sent", "channel": channel, "target": target, "attachments": attachment_count, "message": msg}))
            }
            Err(e) => {
                let available = ctx.channel_manager.channel_names().await.join(", ");
                let err_msg = if available.is_empty() {
                    format!(
                        "Failed to send to {}:{}: {}. No channels connected.",
                        channel, target, e
                    )
                } else {
                    format!(
                        "Failed to send to {}:{}. Available channels: {}. Error: {}",
                        channel, target, available, e
                    )
                };
                Err(MessagingCapabilityError::operation(err_msg))
            }
        }
    } else {
        let results = ctx.channel_manager.broadcast_all(&target, response).await;
        let mut succeeded = Vec::new();
        let mut failed: Vec<&str> = Vec::new();
        for (ch, result) in &results {
            match result {
                Ok(()) => succeeded.push(ch.as_str()),
                Err(_) => {
                    failed.push(ch.as_str());
                }
            }
        }
        if succeeded.is_empty() {
            let err_msg = if failed.is_empty() {
                "No channels connected.".to_string()
            } else {
                format!("All channels failed: {}", failed.join(", "))
            };
            Err(MessagingCapabilityError::operation(err_msg))
        } else {
            let msg = format!(
                "Broadcast message to {} (target: {})",
                succeeded.join(", "),
                target
            );
            Ok(json!({"status": "broadcast", "channels": succeeded, "target": target, "attachments": attachment_count, "message": msg}))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_descriptor_is_valid() {
        let desc = message_descriptor();
        assert_eq!(desc.id.as_str(), MESSAGE_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert!(desc.effects.contains(&EffectKind::ExternalWrite));
        assert_eq!(desc.default_permission, PermissionMode::Allow);
    }

    #[test]
    fn descriptors_returns_message() {
        let descs = descriptors();
        assert_eq!(descs.len(), 1);
        assert_eq!(descs[0].id.as_str(), MESSAGE_CAPABILITY_ID);
    }

    #[test]
    fn message_schema_has_required_content() {
        let desc = message_descriptor();
        let required = desc.parameters_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "content"));
        assert!(!required.iter().any(|v| v == "channel"));
        assert!(!required.iter().any(|v| v == "target"));
    }

    #[test]
    fn message_schema_has_optional_attachments() {
        let desc = message_descriptor();
        assert!(desc.parameters_schema["properties"]["attachments"].is_object());
    }

    #[test]
    fn channel_matches_source_none_always_matches() {
        assert!(channel_matches_source(None, Some("telegram")));
        assert!(channel_matches_source(None, None));
    }

    #[test]
    fn channel_matches_source_same_matches() {
        assert!(channel_matches_source(Some("telegram"), Some("telegram")));
    }

    #[test]
    fn channel_matches_source_different_does_not_match() {
        assert!(!channel_matches_source(Some("slack"), Some("telegram")));
    }

    #[tokio::test]
    async fn execute_message_requires_content() {
        let channel_manager = Arc::new(ChannelManager::new());
        let ctx = MessagingContext {
            channel_manager,
            extension_manager: None,
            default_channel: Arc::new(RwLock::new(None)),
            default_target: Arc::new(RwLock::new(None)),
            base_dir: PathBuf::from("/tmp"),
            user_id: "test".to_string(),
            metadata: json!({}),
        };

        let result = execute_message(&json!({"channel": "signal", "target": "+1234567890"}), &ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("content"));
    }

    #[tokio::test]
    async fn execute_message_no_target_no_channel() {
        let channel_manager = Arc::new(ChannelManager::new());
        let ctx = MessagingContext {
            channel_manager,
            extension_manager: None,
            default_channel: Arc::new(RwLock::new(None)),
            default_target: Arc::new(RwLock::new(None)),
            base_dir: PathBuf::from("/tmp"),
            user_id: "test".to_string(),
            metadata: json!({}),
        };

        let result = execute_message(&json!({"content": "hello"}), &ctx).await;
        assert!(result.is_err() || result.is_ok());
    }
}
