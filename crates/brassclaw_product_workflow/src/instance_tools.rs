//! Instance-wide Tool policy management, independent of component review and
//! task/run authority. Callers authenticate at the product boundary; scope IDs
//! are data addresses, never permission keys for this port.
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Supported per-Tool technical limits. This complete object is required on
/// edits, so older clients cannot erase limits while toggling allow/block.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ToolResourceLimits {
    /// Null removes this additional ceiling. Positive byte values are quoted
    /// decimals on the wire, preserving the full unsigned integer exactly.
    #[serde(with = "output_bytes_wire")]
    pub max_output_bytes: Option<u64>,
}

impl ToolResourceLimits {
    pub fn validate(&self) -> Result<(), InstanceToolSettingsError> {
        if self.max_output_bytes == Some(0) {
            return Err(InstanceToolSettingsError::Invalid);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstanceToolSetting {
    pub tool_id: Uuid,
    /// Retained declared aliases are display/identity information, not grants.
    pub capability_ids: Vec<String>,
    pub enabled: bool,
    pub limits: ToolResourceLimits,
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
    pub limits: ToolResourceLimits,
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

mod output_bytes_wire {
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    pub fn serialize<S: Serializer>(value: &Option<u64>, serializer: S) -> Result<S::Ok, S::Error> {
        match value {
            Some(value) => serializer.serialize_some(&value.to_string()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<u64>, D::Error> {
        let value = Option::<String>::deserialize(deserializer)?;
        value
            .map(|text| {
                let value = text.parse::<u64>().map_err(|_| {
                    D::Error::custom(
                        "output bytes must be a positive canonical decimal string or null",
                    )
                })?;
                if value == 0 || value.to_string() != text {
                    return Err(D::Error::custom(
                        "output bytes must be a positive canonical decimal string or null",
                    ));
                }
                Ok(value)
            })
            .transpose()
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
            limits: ToolResourceLimits::default(),
            revision: 9_007_199_254_740_993,
        };
        let encoded = serde_json::to_value(&value).unwrap();
        assert_eq!(encoded["revision"], "9007199254740993");
        assert_eq!(
            serde_json::from_value::<InstanceToolSetting>(encoded).unwrap(),
            value
        );
        let decoded: UpdateInstanceToolSetting =
            serde_json::from_str(r#"{"expected_revision":"9007199254740993","enabled":true,"limits":{"max_output_bytes":null}}"#)
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
                    serde_json::json!({"expected_revision":revision,"enabled":true,"limits":{"max_output_bytes":null}})
                )
                .is_err()
            );
        }
        assert!(
            serde_json::from_str::<UpdateInstanceToolSetting>(
                r#"{"expected_revision":"1","enabled":true,"limits":{"max_output_bytes":null},"permission_mode":"ask"}"#
            )
            .is_err()
        );
    }

    #[test]
    fn tool_limits_preserve_exact_bytes_and_reject_missing_or_ambiguous_edits() {
        for limit in [None, Some(1), Some(9_007_199_254_740_993), Some(u64::MAX)] {
            let limits = ToolResourceLimits {
                max_output_bytes: limit,
            };
            let encoded = serde_json::to_value(&limits).unwrap();
            assert_eq!(
                encoded["max_output_bytes"],
                limit
                    .map(|value| serde_json::json!(value.to_string()))
                    .unwrap_or(serde_json::Value::Null)
            );
            assert_eq!(
                serde_json::from_value::<ToolResourceLimits>(encoded).unwrap(),
                limits
            );
        }
        for value in [
            serde_json::json!(0),
            serde_json::json!(1),
            serde_json::json!(true),
            serde_json::json!("0"),
            serde_json::json!("01"),
            serde_json::json!("+1"),
            serde_json::json!(" 1"),
            serde_json::json!("1.0"),
            serde_json::json!("1.5"),
            serde_json::json!("18446744073709551616"),
        ] {
            assert!(serde_json::from_value::<UpdateInstanceToolSetting>(serde_json::json!({"expected_revision":"1","enabled":true,"limits":{"max_output_bytes":value}})).is_err());
        }
        for limits in [
            serde_json::json!({}),
            serde_json::json!(null),
            serde_json::json!({"max_output_bytes":null,"max_wall_clock_ms":"1"}),
        ] {
            assert!(
                serde_json::from_value::<UpdateInstanceToolSetting>(
                    serde_json::json!({"expected_revision":"1","enabled":false,"limits":limits})
                )
                .is_err()
            );
        }
        assert!(
            serde_json::from_str::<UpdateInstanceToolSetting>(
                r#"{"expected_revision":"1","enabled":false}"#
            )
            .is_err()
        );
        assert!(
            ToolResourceLimits {
                max_output_bytes: Some(0)
            }
            .validate()
            .is_err()
        );
    }
}
