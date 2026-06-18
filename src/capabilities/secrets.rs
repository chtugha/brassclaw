use std::sync::Arc;

use brassclaw_host_api::{
    CapabilityDescriptor, CapabilityId, EffectKind, ExtensionId, PermissionMode, ResourceCeiling,
    ResourceEstimate, ResourceProfile, RuntimeKind, TrustClass,
};
use serde_json::{Value, json};

use crate::secrets::SecretsStore;

pub const PROVIDER_ID: &str = "builtin";
pub const SECRET_LIST_CAPABILITY_ID: &str = "builtin.secret_list";
pub const SECRET_DELETE_CAPABILITY_ID: &str = "builtin.secret_delete";

const DEFAULT_OUTPUT_BYTES: u64 = 4 * 1024;
const MAX_OUTPUT_BYTES: u64 = 64 * 1024;
const DEFAULT_WALL_CLOCK_MS: u64 = 1_000;
const MAX_WALL_CLOCK_MS: u64 = 10_000;

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct SecretsCapabilityError {
    pub message: String,
    pub is_input_error: bool,
}

impl SecretsCapabilityError {
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

pub struct SecretsContext {
    pub store: Arc<dyn SecretsStore + Send + Sync>,
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

pub fn secret_list_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        SECRET_LIST_CAPABILITY_ID,
        "List all stored secrets by name. Never returns values — only names and \
         optional provider metadata. Use this to check what credentials are available \
         before attempting a task that requires them.",
        vec![EffectKind::UseSecret],
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }),
        PermissionMode::Allow,
    )
}

pub fn secret_delete_descriptor() -> CapabilityDescriptor {
    make_descriptor(
        SECRET_DELETE_CAPABILITY_ID,
        "Permanently delete a stored secret by name. This cannot be undone.",
        vec![EffectKind::UseSecret],
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of the secret to delete."
                }
            },
            "required": ["name"],
            "additionalProperties": false
        }),
        PermissionMode::Ask,
    )
}

pub fn descriptors() -> Vec<CapabilityDescriptor> {
    vec![secret_list_descriptor(), secret_delete_descriptor()]
}

fn require_str<'a>(params: &'a Value, key: &str) -> Result<&'a str, SecretsCapabilityError> {
    params
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| SecretsCapabilityError::input(format!("missing required parameter: {key}")))
}

pub async fn execute_secret_list(
    _params: &Value,
    ctx: &SecretsContext,
) -> Result<Value, SecretsCapabilityError> {
    let refs = ctx
        .store
        .list(&ctx.user_id)
        .await
        .map_err(|e| SecretsCapabilityError::operation(e.to_string()))?;

    let secrets: Vec<Value> = refs
        .into_iter()
        .map(|r| {
            json!({
                "name": r.name,
                "provider": r.provider,
            })
        })
        .collect();

    let count = secrets.len();
    Ok(json!({
        "secrets": secrets,
        "count": count,
    }))
}

pub async fn execute_secret_delete(
    params: &Value,
    ctx: &SecretsContext,
) -> Result<Value, SecretsCapabilityError> {
    let name = require_str(params, "name")?;

    let deleted = ctx
        .store
        .delete(&ctx.user_id, name)
        .await
        .map_err(|e| SecretsCapabilityError::operation(e.to_string()))?;

    if deleted {
        Ok(json!({
            "status": "deleted",
            "name": name,
        }))
    } else {
        Ok(json!({
            "status": "not_found",
            "name": name,
            "message": format!("No secret named '{}' found.", name),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::CreateSecretParams;
    use crate::testing::credentials::{TEST_OPENAI_API_KEY_SHORT, test_secrets_store};

    fn test_store() -> Arc<crate::secrets::InMemorySecretsStore> {
        Arc::new(test_secrets_store())
    }

    fn test_ctx(store: Arc<dyn SecretsStore + Send + Sync>) -> SecretsContext {
        SecretsContext {
            store,
            user_id: "test".to_string(),
        }
    }

    #[test]
    fn secret_list_descriptor_is_valid() {
        let desc = secret_list_descriptor();
        assert_eq!(desc.id.as_str(), SECRET_LIST_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert_eq!(desc.effects, vec![EffectKind::UseSecret]);
        assert_eq!(desc.default_permission, PermissionMode::Allow);
    }

    #[test]
    fn secret_delete_descriptor_is_valid() {
        let desc = secret_delete_descriptor();
        assert_eq!(desc.id.as_str(), SECRET_DELETE_CAPABILITY_ID);
        assert_eq!(desc.provider.as_str(), PROVIDER_ID);
        assert_eq!(desc.effects, vec![EffectKind::UseSecret]);
        assert_eq!(desc.default_permission, PermissionMode::Ask);
    }

    #[test]
    fn descriptors_returns_all() {
        let descs = descriptors();
        assert_eq!(descs.len(), 2);
    }

    #[tokio::test]
    async fn test_secret_list() {
        let store = test_store();
        store
            .create(
                "test",
                CreateSecretParams::new("openai_key", TEST_OPENAI_API_KEY_SHORT),
            )
            .await
            .unwrap();

        let ctx = test_ctx(Arc::clone(&store) as Arc<dyn SecretsStore + Send + Sync>);
        let result = execute_secret_list(&json!({}), &ctx).await.unwrap();
        assert_eq!(result["count"], 1);
        assert_eq!(result["secrets"][0]["name"], "openai_key");
        assert!(result["secrets"][0].get("value").is_none());
    }

    #[tokio::test]
    async fn test_secret_delete() {
        let store = test_store();
        store
            .create("test", CreateSecretParams::new("to_delete", "secret"))
            .await
            .unwrap();

        let ctx = test_ctx(Arc::clone(&store) as Arc<dyn SecretsStore + Send + Sync>);
        let result = execute_secret_delete(&json!({"name": "to_delete"}), &ctx)
            .await
            .unwrap();
        assert_eq!(result["status"], "deleted");

        let result2 = execute_secret_delete(&json!({"name": "to_delete"}), &ctx)
            .await
            .unwrap();
        assert_eq!(result2["status"], "not_found");
    }

    #[tokio::test]
    async fn test_secret_delete_missing_name() {
        let store = test_store();
        let ctx = test_ctx(Arc::clone(&store) as Arc<dyn SecretsStore + Send + Sync>);
        let result = execute_secret_delete(&json!({}), &ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().is_input_error);
    }
}
