//! PRIVATE036: reuse the concretely composed Auth/HTTP owners, never reconstruct
//! stores from a downcast or select a file/environment session as a fallback.
use super::RebornProductAuthServices;
use std::sync::Arc;

use brassclaw_auth::{AuthProductError, NativeDeviceOAuthClient};
use brassclaw_host_runtime::ProductAuthProviderRuntimePorts;

#[derive(Clone, Default)]
pub(super) enum NativeCodexOwners {
    #[default]
    Unavailable,
    Composed {
        auth: Arc<crate::pg_auth_product_services::PgAuthProductServices>,
        ports: ProductAuthProviderRuntimePorts,
    },
}

impl RebornProductAuthServices {
    pub(crate) fn with_native_codex_owners(
        mut self,
        auth: Arc<crate::pg_auth_product_services::PgAuthProductServices>,
        ports: ProductAuthProviderRuntimePorts,
    ) -> Result<Self, AuthProductError> {
        if !matches!(self.native_codex, NativeCodexOwners::Unavailable) {
            return Err(AuthProductError::BackendConflict);
        }
        self.native_codex = NativeCodexOwners::Composed { auth, ports };
        Ok(self)
    }

    pub(crate) fn native_codex_owner(
        &self,
    ) -> Result<Arc<crate::pg_auth_product_services::PgAuthProductServices>, AuthProductError> {
        match &self.native_codex {
            NativeCodexOwners::Composed { auth, .. } => Ok(auth.clone()),
            NativeCodexOwners::Unavailable => Err(AuthProductError::BackendUnavailable),
        }
    }

    pub(crate) fn native_codex_transport(
        &self,
        client: &NativeDeviceOAuthClient,
    ) -> Result<crate::oauth_provider_client::HostOAuthProviderClient, AuthProductError> {
        match &self.native_codex {
            NativeCodexOwners::Composed { auth, ports } => {
                auth.native_codex_transport(ports, client, self.primitive_settings.clone())
            }
            NativeCodexOwners::Unavailable => Err(AuthProductError::BackendUnavailable),
        }
    }

    pub(crate) async fn native_codex_limits(
        &self,
    ) -> Result<brassclaw_host_api::MontyExecutionLimits, AuthProductError> {
        crate::oauth_provider_client::acknowledged_oauth_limits(self.primitive_settings.as_ref())
            .await
    }
}

/// Captured internal preparation capability. Carries the original composed host
/// ports alongside Auth; model consumers never reconstruct storage or egress.
#[derive(Clone)]
pub(crate) struct NativeCodexSessionOwner {
    pub(crate) auth: Arc<crate::pg_auth_product_services::PgAuthProductServices>,
    pub(crate) ports: ProductAuthProviderRuntimePorts,
    pub(crate) settings: Option<Arc<dyn brassclaw_host_runtime::HostPrimitiveSettingsSource>>,
}
impl RebornProductAuthServices {
    pub(crate) fn native_codex_session_owner(
        &self,
    ) -> Result<NativeCodexSessionOwner, AuthProductError> {
        match &self.native_codex {
            NativeCodexOwners::Composed { auth, ports } => Ok(NativeCodexSessionOwner {
                auth: auth.clone(),
                ports: ports.clone(),
                settings: self.primitive_settings.clone(),
            }),
            NativeCodexOwners::Unavailable => Err(AuthProductError::BackendUnavailable),
        }
    }
}
