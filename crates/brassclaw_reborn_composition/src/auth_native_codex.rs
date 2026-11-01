#![allow(dead_code)] // PRIVATE036: staged native owner APIs; callers land with the full login/refresh/startup wiring.
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
