//! Captured, acknowledged host resource policy. This supplies no authority.
use async_trait::async_trait;
use brassclaw_host_api::MontyExecutionLimits;

use crate::FirstPartyCapabilityError;

#[derive(Debug, Clone, Copy)]
pub struct HostPrimitiveSettings {
    pub revision: u64,
    pub limits: MontyExecutionLimits,
}

#[async_trait]
pub trait HostPrimitiveSettingsSource: Send + Sync + std::fmt::Debug {
    async fn snapshot(&self) -> Result<HostPrimitiveSettings, FirstPartyCapabilityError>;
}
