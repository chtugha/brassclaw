//! Instance-wide Tool policy management, independent of component review and
//! task/run authority. Callers authenticate at the product boundary; scope IDs
//! are data addresses, never permission keys for this port.
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstanceToolSetting {
    pub tool_id: Uuid,
    /// Retained declared aliases are display/identity information, not grants.
    pub capability_ids: Vec<String>,
    pub enabled: bool,
    /// Quoted decimal on the wire, preserving revisions beyond JS safe integers.
    #[serde(with = "revision_wire")]
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateInstanceToolSetting {
    #[serde(with = "revision_wire")]
    pub expected_revision: u64,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum InstanceToolSettingsError {
    #[error("invalid instance Tool setting")]
    Invalid,
    #[error("instance Tool setting not found")]
    NotFound,
    #[error("instance Tool setting revision conflict")]
    Conflict,
    #[error("instance Tool settings unavailable")]
    Unavailable,
}

/// Managing an existing policy row never creates/activates a component, loads
/// an implementation or authorizes an invocation. The kernel reads the current
/// policy again before every actual dispatch, across retained revisions/aliases.
#[async_trait]
pub trait InstanceToolSettings: Send + Sync {
    async fn list(&self) -> Result<Vec<InstanceToolSetting>, InstanceToolSettingsError>;
    async fn update(
        &self,
        tool_id: Uuid,
        request: &UpdateInstanceToolSetting,
    ) -> Result<InstanceToolSetting, InstanceToolSettingsError>;
}

mod revision_wire {
    use serde::{Deserialize, Deserializer, Serializer, de::Error};
    pub fn serialize<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        let text = String::deserialize(deserializer)?;
        if text.is_empty()
            || !text.bytes().all(|byte| byte.is_ascii_digit())
            || (text.len() > 1 && text.starts_with('0'))
        {
            return Err(D::Error::custom(
                "revision must be a canonical decimal string",
            ));
        }
        text.parse()
            .map_err(|_| D::Error::custom("revision exceeds its integer representation"))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn policy_revision_wire_preserves_exact_large_values_and_rejects_ambiguous_inputs() {
        let value = InstanceToolSetting {
            tool_id: Uuid::new_v4(),
            capability_ids: vec!["host.post_reply".into()],
            enabled: false,
            revision: 9_007_199_254_740_993,
        };
        let encoded = serde_json::to_value(&value).unwrap();
        assert_eq!(encoded["revision"], "9007199254740993");
        assert_eq!(
            serde_json::from_value::<InstanceToolSetting>(encoded).unwrap(),
            value
        );
        let decoded: UpdateInstanceToolSetting =
            serde_json::from_str(r#"{"expected_revision":"9007199254740993","enabled":true}"#)
                .unwrap();
        assert_eq!(decoded.expected_revision, value.revision);
        for revision in [
            serde_json::json!(1),
            serde_json::json!("01"),
            serde_json::json!("+1"),
            serde_json::json!("-1"),
            serde_json::json!("18446744073709551616"),
        ] {
            assert!(
                serde_json::from_value::<UpdateInstanceToolSetting>(
                    serde_json::json!({"expected_revision":revision,"enabled":true})
                )
                .is_err()
            );
        }
        assert!(
            serde_json::from_str::<UpdateInstanceToolSetting>(
                r#"{"expected_revision":"1","enabled":true,"permission_mode":"ask"}"#
            )
            .is_err()
        );
    }
}
