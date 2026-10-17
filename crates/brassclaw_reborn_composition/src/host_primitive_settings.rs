//! Host resource consumers read the same acknowledged snapshot as Monty.
//! The view is bound once before ingress; missing/closed owners fail closed.
use async_trait::async_trait;
use brassclaw_host_api::RuntimeDispatchErrorKind;
use brassclaw_host_runtime::{
    FirstPartyCapabilityError, HostPrimitiveSettings, HostPrimitiveSettingsSource,
};
use brassclaw_product_workflow::MontyVmSettingsStore;
use std::sync::{Arc, RwLock};

pub(crate) struct AcknowledgedPrimitiveSettings {
    source: RwLock<Option<Arc<dyn MontyVmSettingsStore>>>,
}
impl std::fmt::Debug for AcknowledgedPrimitiveSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AcknowledgedPrimitiveSettings")
            .finish_non_exhaustive()
    }
}
impl AcknowledgedPrimitiveSettings {
    pub(crate) fn new() -> Self {
        Self {
            source: RwLock::new(None),
        }
    }
    pub(crate) async fn bind(
        &self,
        source: Arc<dyn MontyVmSettingsStore>,
    ) -> Result<(), crate::RebornBuildError> {
        let invalid = || crate::RebornBuildError::InvalidConfig {
            reason: "host primitive settings binding failed".into(),
        };
        let settings = source
            .get("default", "default")
            .await
            .map_err(|_| invalid())?;
        settings
            .execution_limits
            .validate()
            .map_err(|_| invalid())?;
        let mut bound = self.source.write().map_err(|_| invalid())?;
        if bound.is_some() {
            return Err(invalid());
        }
        *bound = Some(source);
        Ok(())
    }
}
#[async_trait]
impl HostPrimitiveSettingsSource for AcknowledgedPrimitiveSettings {
    async fn snapshot(&self) -> Result<HostPrimitiveSettings, FirstPartyCapabilityError> {
        let failure = || FirstPartyCapabilityError::new(RuntimeDispatchErrorKind::Resource);
        let source = self
            .source
            .read()
            .map_err(|_| failure())?
            .clone()
            .ok_or_else(failure)?;
        let settings = source
            .get("default", "default")
            .await
            .map_err(|_| failure())?;
        settings
            .execution_limits
            .validate()
            .map_err(|_| failure())?;
        Ok(HostPrimitiveSettings {
            revision: settings.revision,
            limits: settings.execution_limits,
        })
    }
}
