//! Captured, acknowledged host resource policy. This supplies no authority.
use async_trait::async_trait;
use brassclaw_host_api::MontyExecutionLimits;

use crate::FirstPartyCapabilityError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostPrimitiveSettings {
    pub revision: u64,
    pub limits: MontyExecutionLimits,
}

#[async_trait]
pub trait HostPrimitiveSettingsSource: Send + Sync + std::fmt::Debug {
    async fn snapshot(&self) -> Result<HostPrimitiveSettings, FirstPartyCapabilityError>;
    async fn capture_for_native_fixture(
        &self,
    ) -> Result<NativePrimitiveSettingsCapture, FirstPartyCapabilityError> {
        Err(FirstPartyCapabilityError::new(
            brassclaw_host_api::RuntimeDispatchErrorKind::UnsupportedRunner,
        ))
    }
}

/// Actual source of one accepted native preparation snapshot; not a Tool grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NativePrimitiveSettingsOrigin {
    ControlledBootstrap,
    AcknowledgedLive,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativePrimitiveSettingsCapture {
    pub settings: HostPrimitiveSettings,
    pub origin: NativePrimitiveSettingsOrigin,
}
