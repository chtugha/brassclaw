//! Host resource consumers read the same acknowledged snapshot as Monty.
//! Installation qualification receives the exact selected startup snapshot.
//! Before ingress, that private bootstrap state transitions once to the live
//! acknowledged view. Neither state grants Tool permission.
use async_trait::async_trait;
use brassclaw_host_api::RuntimeDispatchErrorKind;
use brassclaw_host_runtime::{
    FirstPartyCapabilityError, HostPrimitiveSettings, HostPrimitiveSettingsSource,
};
use brassclaw_product_workflow::MontyVmSettingsStore;
use std::sync::{Arc, RwLock};

pub(crate) struct AcknowledgedPrimitiveSettings {
    source: RwLock<PrimitiveSettingsState>,
}
enum PrimitiveSettingsState {
    Unbound,
    Bootstrap(Box<HostPrimitiveSettings>),
    Live(Arc<dyn MontyVmSettingsStore>),
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
            source: RwLock::new(PrimitiveSettingsState::Unbound),
        }
    }
    pub(crate) fn prepare_bootstrap(
        &self,
        settings: HostPrimitiveSettings,
    ) -> Result<(), crate::RebornBuildError> {
        let invalid = || crate::RebornBuildError::InvalidConfig {
            reason: "host primitive bootstrap settings failed".into(),
        };
        if settings.revision == 0 {
            return Err(invalid());
        }
        settings.limits.validate().map_err(|_| invalid())?;
        let mut source = self.source.write().map_err(|_| invalid())?;
        if !matches!(*source, PrimitiveSettingsState::Unbound) {
            return Err(invalid());
        }
        *source = PrimitiveSettingsState::Bootstrap(Box::new(settings));
        Ok(())
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
        let PrimitiveSettingsState::Bootstrap(bootstrap) = &*bound else {
            return Err(invalid());
        };
        if settings.revision < bootstrap.revision
            || (settings.revision == bootstrap.revision
                && settings.execution_limits != bootstrap.limits)
        {
            return Err(invalid());
        }
        *bound = PrimitiveSettingsState::Live(source);
        Ok(())
    }
}
#[async_trait]
impl HostPrimitiveSettingsSource for AcknowledgedPrimitiveSettings {
    async fn snapshot(&self) -> Result<HostPrimitiveSettings, FirstPartyCapabilityError> {
        let failure = || FirstPartyCapabilityError::new(RuntimeDispatchErrorKind::Resource);
        let source = {
            let bound = self.source.read().map_err(|_| failure())?;
            match &*bound {
                PrimitiveSettingsState::Unbound => return Err(failure()),
                PrimitiveSettingsState::Bootstrap(settings) => return Ok(**settings),
                PrimitiveSettingsState::Live(source) => source.clone(),
            }
        };
        let settings = source
            .get("default", "default")
            .await
            .map_err(|_| failure())?;
        settings
            .execution_limits
            .validate()
            .map_err(|_| failure())?;
        if settings.revision == 0 {
            return Err(failure());
        }
        Ok(HostPrimitiveSettings {
            revision: settings.revision,
            limits: settings.execution_limits,
        })
    }
}
