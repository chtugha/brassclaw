use std::sync::Arc;

use brassclaw_host_api::{
    CapabilityDescriptor, CapabilityId, EffectKind, ExtensionId, PermissionMode, ResourceCeiling,
    ResourceEstimate, ResourceProfile, RuntimeKind, TrustClass,
};
use serde_json::{Value, json};

use crate::ownership::{UserId, UserRole};
use crate::pairing::PairingStore;

pub const PROVIDER_ID: &str = "builtin";
pub const PAIRING_APPROVE_CAPABILITY_ID: &str = "builtin.pairing_approve";

const CHANNEL: &str = "slack-relay";

const DEFAULT_OUTPUT_BYTES: u64 = 4 * 1024;
const MAX_OUTPUT_BYTES: u64 = 64 * 1024;
const DEFAULT_WALL_CLOCK_MS: u64 = 2_000;
const MAX_WALL_CLOCK_MS: u64 = 30_000;

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct PairingCapabilityError {
    pub message: String,
    pub is_input_error: bool,
}

impl PairingCapabilityError {
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

pub struct PairingContext {
    pub store: Arc<PairingStore>,
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

pub fn pairing_approve_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        PAIRING_APPROVE_CAPABILITY_ID,
        "Approve a Slack pairing code to bind the user's Slack account to their BrassClaw user. \
         The user receives the code in Slack and provides it here.",
        vec![EffectKind::ModifyApproval],
        json!({
            "type": "object",
            "properties": {
                "code": {
                    "type": "string",
                    "description": "The pairing code received in Slack (e.g. WZG8LQAB)"
                }
            },
            "required": ["code"],
            "additionalProperties": false
        }),
        PermissionMode::Ask,
    )
}

pub fn descriptors() -> Vec<CapabilityDescriptor> {
    vec![pairing_approve_descriptor()]
}

fn require_str<'a>(params: &'a Value, key: &str) -> Result<&'a str, PairingCapabilityError> {
    params
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| PairingCapabilityError::input(format!("missing required parameter: {key}")))
}

pub async fn execute_pairing_approve(
    params: &Value,
    ctx: &PairingContext,
) -> Result<Value, PairingCapabilityError> {
    let code = require_str(params, "code")?;

    let user_id = UserId::new(&ctx.user_id, UserRole::Regular)
        .map_err(|e| PairingCapabilityError::operation(format!("invalid user_id: {e}")))?;

    match ctx.store.approve(CHANNEL, code, &user_id).await {
        Ok(approval) => {
            let msg = format!(
                "Pairing approved! Your {} account (external ID: {}) is now linked to your BrassClaw user.",
                approval.channel, approval.external_id
            );
            Ok(json!({ "status": "approved", "message": msg }))
        }
        Err(e) => {
            let msg = format!(
                "Pairing failed: {e}. Make sure the code is correct and hasn't expired."
            );
            Ok(json!({ "status": "failed", "message": msg }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_approve_descriptor_is_valid() {
        let desc = pairing_approve_descriptor();
        assert_eq!(desc.id.as_str(), PAIRING_APPROVE_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert_eq!(desc.effects, vec![EffectKind::ModifyApproval]);
        assert_eq!(desc.default_permission, PermissionMode::Ask);
    }

    #[test]
    fn descriptors_returns_all() {
        let descs = descriptors();
        assert_eq!(descs.len(), 1);
    }

    #[test]
    fn descriptor_has_code_property() {
        let desc = pairing_approve_descriptor();
        assert!(desc.parameters_schema["properties"]["code"].is_object());
    }

    #[test]
    fn channel_is_slack_relay() {
        assert_eq!(CHANNEL, "slack-relay");
    }

    #[tokio::test]
    async fn test_pairing_approve_missing_code() {
        let store = Arc::new(PairingStore::new_noop());
        let ctx = PairingContext {
            store,
            user_id: "test".to_string(),
        };
        let result = execute_pairing_approve(&json!({}), &ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().is_input_error);
    }

    #[tokio::test]
    async fn test_pairing_approve_noop_store() {
        let store = Arc::new(PairingStore::new_noop());
        let ctx = PairingContext {
            store,
            user_id: "test".to_string(),
        };
        let result = execute_pairing_approve(&json!({"code": "ABC123"}), &ctx).await;
        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["status"], "failed");
    }
}
