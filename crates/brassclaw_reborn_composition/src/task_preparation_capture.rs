//! Original task preparation, captured once from the catalogue's acknowledged
//! settings view. This is provenance, not a live VM/heap override or Tool grant.
use brassclaw_host_api::MontyExecutionLimits;
use brassclaw_monty_host::utility::UtilityLimits;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PreparationOrigin {
    AcknowledgedStartup,
    AcknowledgedLive,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    format: String,
    origin: PreparationOrigin,
    revision: u64,
    execution_limits_bytes: String,
    execution_limits_checksum: String,
}

/// Immutable original bytes and their checked projections. No public constructor
/// from separate revision/limits claims; the catalogue owns fresh capture.
#[derive(Clone)]
pub(crate) struct PreparationCapture {
    origin: PreparationOrigin,
    revision: u64,
    policy: MontyExecutionLimits,
    limits: UtilityLimits,
    bytes: Arc<str>,
    checksum: Arc<str>,
}

fn checksum(bytes: &str) -> String {
    hex::encode(Sha256::digest(bytes.as_bytes()))
}

impl PreparationCapture {
    pub(super) fn capture(
        settings: &brassclaw_product_workflow::MontyVmSettings,
        origin: PreparationOrigin,
    ) -> Result<Self, &'static str> {
        if settings.revision == 0 {
            return Err("preparation revision is invalid");
        }
        let policy = settings.execution_limits;
        let limits = UtilityLimits::from_execution(policy)?;
        let execution_limits_bytes =
            serde_json::to_string(&policy).map_err(|_| "preparation limits encoding failed")?;
        let wire = Wire {
            format: "task-preparation-capture/1".into(),
            origin,
            revision: settings.revision,
            execution_limits_checksum: checksum(&execution_limits_bytes),
            execution_limits_bytes,
        };
        let bytes =
            serde_json::to_string(&wire).map_err(|_| "preparation capture encoding failed")?;
        Ok(Self {
            origin,
            revision: settings.revision,
            policy,
            limits,
            checksum: checksum(&bytes).into(),
            bytes: bytes.into(),
        })
    }

    /// Read the exact retained original; never consult current settings. This
    /// verifies provenance bytes, not their Work ownership, claim or approval.
    pub(crate) fn restore_original(
        bytes: &str,
        expected_checksum: &str,
    ) -> Result<Self, &'static str> {
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
        let limits = UtilityLimits::from_execution(policy)?;
        Ok(Self {
            origin: wire.origin,
            revision: wire.revision,
            policy,
            limits,
            bytes: bytes.into(),
            checksum: expected_checksum.into(),
        })
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }
    pub(crate) fn origin(&self) -> PreparationOrigin {
        self.origin
    }
    pub(crate) fn policy(&self) -> MontyExecutionLimits {
        self.policy
    }
    pub(crate) fn limits(&self) -> UtilityLimits {
        self.limits
    }
    pub(crate) fn bytes(&self) -> &str {
        &self.bytes
    }
    pub(crate) fn checksum(&self) -> &str {
        &self.checksum
    }
}
