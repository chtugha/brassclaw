//! Original task preparation, captured once from the catalogue's acknowledged
//! settings view. This is provenance, not a live VM/heap override or Tool grant.
use crate::MontyExecutionLimits;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreparationOrigin {
    AcknowledgedStartup,
    AcknowledgedLive,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    format: String,
    // Retained /1 provenance; neither a dispatch distinction nor authority.
    origin: PreparationOrigin,
    revision: u64,
    execution_limits_bytes: String,
    execution_limits_checksum: String,
}

/// Exact original preparation bytes. Encoding/restoration checks integrity,
/// not acknowledgement, Work ownership, approval or Tool permission.
#[derive(Clone)]
pub struct RetainedPreparation {
    revision: u64,
    policy: MontyExecutionLimits,
    bytes: Arc<str>,
    checksum: Arc<str>,
}

fn checksum(bytes: &str) -> String {
    Sha256::digest(bytes.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

impl RetainedPreparation {
    /// Encode integrity/provenance data only. The calling application owner
    /// establishes acknowledgement; this value grants no admission or authority.
    pub fn encode(
        revision: u64,
        origin: PreparationOrigin,
        policy: MontyExecutionLimits,
    ) -> Result<Self, &'static str> {
        if revision == 0 {
            return Err("preparation revision is invalid");
        }
        policy.validate()?;
        let execution_limits_bytes =
            serde_json::to_string(&policy).map_err(|_| "preparation limits encoding failed")?;
        let wire = Wire {
            format: "task-preparation-capture/1".into(),
            origin,
            revision,
            execution_limits_checksum: checksum(&execution_limits_bytes),
            execution_limits_bytes,
        };
        let bytes =
            serde_json::to_string(&wire).map_err(|_| "preparation capture encoding failed")?;
        Ok(Self {
            revision,
            policy,
            checksum: checksum(&bytes).into(),
            bytes: bytes.into(),
        })
    }

    /// Read the exact retained original; never consult current settings. This
    /// verifies provenance bytes, not their Work ownership, claim or approval.
    pub fn restore_original(bytes: &str, expected_checksum: &str) -> Result<Self, &'static str> {
        if checksum(bytes) != expected_checksum {
            return Err("preparation capture checksum mismatch");
        }
        let wire: Wire =
            serde_json::from_str(bytes).map_err(|_| "preparation capture format is invalid")?;
        if wire.format != "task-preparation-capture/1" || wire.revision == 0 {
            return Err("preparation capture identity is invalid");
        }
        if checksum(&wire.execution_limits_bytes) != wire.execution_limits_checksum {
            return Err("preparation limits checksum mismatch");
        }
        let policy: MontyExecutionLimits = serde_json::from_str(&wire.execution_limits_bytes)
            .map_err(|_| "preparation limits format is invalid")?;
        // MontyExecutionLimits has legacy defaulted fields for settings upgrades.
        // An original capture must contain every field of its exact /1 codec;
        // missing fields may not acquire today's defaults on restore.
        if serde_json::to_string(&policy).map_err(|_| "preparation limits encoding failed")?
            != wire.execution_limits_bytes
        {
            return Err("preparation limits are not a complete original capture");
        }
        policy.validate()?;
        Ok(Self {
            revision: wire.revision,
            policy,
            bytes: bytes.into(),
            checksum: expected_checksum.into(),
        })
    }

    /// Intrinsic maximum encoding size of this numeric-only /1 schema, not a
    /// resource quota/default. Every field is bounded by unsigned64 decimal
    /// width; key escaping and outer framing use the actual existing serializer.
    pub fn maximum_canonical_bytes() -> Result<usize, &'static str> {
        let mut fields = serde_json::to_value(MontyExecutionLimits::default())
            .map_err(|_| "preparation schema encoding failed")?;
        let object = fields
            .as_object_mut()
            .ok_or("preparation schema is invalid")?;
        for value in object.values_mut() {
            if !value.is_u64() {
                return Err("preparation schema is not numeric");
            }
            *value = serde_json::Value::from(u64::MAX);
        }
        let inner =
            serde_json::to_string(&fields).map_err(|_| "preparation schema encoding failed")?;
        let wire = Wire {
            format: "task-preparation-capture/1".into(),
            origin: PreparationOrigin::AcknowledgedStartup,
            revision: u64::MAX,
            execution_limits_bytes: inner,
            execution_limits_checksum: "0".repeat(64),
        };
        Ok(serde_json::to_string(&wire)
            .map_err(|_| "preparation schema encoding failed")?
            .len())
    }
    /// Fresh /4 owner records retain the encoder's exact canonical capture.
    /// Legacy /1 capture restoration itself keeps its old outer-whitespace
    /// acceptance; this additional check belongs only to the new owner format.
    pub fn require_canonical(&self) -> Result<(), &'static str> {
        let wire: Wire = serde_json::from_str(self.bytes())
            .map_err(|_| "preparation capture format is invalid")?;
        if serde_json::to_string(&wire).map_err(|_| "preparation capture encoding failed")?
            != self.bytes()
        {
            return Err("new preparation capture is not canonical");
        }
        Ok(())
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn policy(&self) -> MontyExecutionLimits {
        self.policy
    }
    pub fn bytes(&self) -> &str {
        &self.bytes
    }
    pub fn checksum(&self) -> &str {
        &self.checksum
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Literal /1 fixtures follow the original serializer's declaration order.
    // The unchanged legacy Wire encoder is independently checked before the new
    // public codec: tests do not generate expected bytes with that new encoder.
    #[test]
    fn original_preparation_v1_goldens_preserve_legacy_bytes_and_strictness() {
        for (bytes, digest, origin, revision, count) in [
            (
                include_str!("testdata/preparation-v1-startup.json"),
                include_str!("testdata/preparation-v1-startup.sha256"),
                PreparationOrigin::AcknowledgedStartup,
                19,
                5001,
            ),
            (
                include_str!("testdata/preparation-v1-live.json"),
                include_str!("testdata/preparation-v1-live.sha256"),
                PreparationOrigin::AcknowledgedLive,
                23,
                5002,
            ),
        ] {
            let mut policy = MontyExecutionLimits {
                max_catalogue_components_per_recipe: count,
                max_catalogue_bytes: 134217728,
                ..MontyExecutionLimits::default()
            };
            policy.max_value_nodes += 7;
            let inner = serde_json::to_string(&policy).unwrap();
            let legacy_wire = Wire {
                format: "task-preparation-capture/1".into(),
                origin,
                revision,
                execution_limits_checksum: checksum(&inner),
                execution_limits_bytes: inner,
            };
            assert_eq!(serde_json::to_string(&legacy_wire).unwrap(), bytes);
            assert_eq!(checksum(bytes), digest);
            let fresh = RetainedPreparation::encode(revision, origin, policy).unwrap();
            assert_eq!(fresh.bytes(), bytes);
            assert_eq!(fresh.checksum(), digest);
            let original = RetainedPreparation::restore_original(bytes, digest).unwrap();
            assert_eq!(original.revision(), revision);
            assert_eq!(original.policy(), policy);
            // Old outer whitespace acceptance must retain the actual original.
            let spaced = format!(" {bytes} ");
            assert_eq!(
                RetainedPreparation::restore_original(&spaced, &checksum(&spaced))
                    .unwrap()
                    .bytes(),
                spaced
            );
            assert!(RetainedPreparation::restore_original(bytes, "wrong").is_err());
            let mut wire: Wire = serde_json::from_str(bytes).unwrap();
            wire.execution_limits_bytes =
                wire.execution_limits_bytes
                    .replacen("\"max_value_nodes\":1048583,", "", 1);
            wire.execution_limits_checksum = checksum(&wire.execution_limits_bytes);
            let missing = serde_json::to_string(&wire).unwrap();
            assert!(RetainedPreparation::restore_original(&missing, &checksum(&missing)).is_err());
            let duplicate = bytes.replacen("\"revision\":", "\"revision\":1,\"revision\":", 1);
            assert!(
                RetainedPreparation::restore_original(&duplicate, &checksum(&duplicate)).is_err()
            );
        }
    }
}
