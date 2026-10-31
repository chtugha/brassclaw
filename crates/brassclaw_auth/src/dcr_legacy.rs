//! Exact selection for a pre-custody DCR flow. No registration request/response,
//! management token or approval evidence is manufactured by this migration.
use crate::{
    AuthChallenge, AuthContinuationRef, AuthFlowCustody, AuthFlowId, AuthFlowKind, AuthFlowRecord,
    AuthFlowStatus, AuthProductError, AuthProductScope, AuthProviderId, CredentialAccountLabel,
    CredentialAccountUpdateBinding, DcrCleanupProjection, DcrCleanupUnavailable,
    DcrLocalCleanupStatus, DcrMaterialReference, DcrRemoteCleanupStatus, OAuthAuthorizationUrl,
    OpaqueStateHash, PkceVerifierHash, ProviderScope, Timestamp, is_terminal_status,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DcrLegacyOriginal {
    pub scope: AuthProductScope,
    pub flow_id: AuthFlowId,
    pub provider: AuthProviderId,
    pub kind: AuthFlowKind,
    pub account_label: CredentialAccountLabel,
    pub requested_scopes: Vec<ProviderScope>,
    pub authorization_url: OAuthAuthorizationUrl,
    pub opaque_state_hash: OpaqueStateHash,
    pub pkce_verifier_hash: PkceVerifierHash,
    pub expires_at: Timestamp,
    pub created_at: Timestamp,
    pub continuation: AuthContinuationRef,
    pub update_binding: Option<CredentialAccountUpdateBinding>,
}
impl DcrLegacyOriginal {
    pub fn from_flow(
        flow: &AuthFlowRecord,
        label: CredentialAccountLabel,
        scopes: Vec<ProviderScope>,
    ) -> Result<Self, AuthProductError> {
        if flow.kind != AuthFlowKind::IntegrationCredential {
            return Err(AuthProductError::BackendConflict);
        }
        let Some(AuthChallenge::OAuthUrl {
            authorization_url,
            expires_at,
        }) = &flow.challenge
        else {
            return Err(AuthProductError::BackendConflict);
        };
        if *expires_at != flow.expires_at {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(Self {
            scope: flow.scope.clone(),
            flow_id: flow.id,
            provider: flow.provider.clone(),
            kind: flow.kind,
            account_label: label,
            requested_scopes: scopes,
            authorization_url: authorization_url.clone(),
            opaque_state_hash: flow
                .opaque_state_hash
                .clone()
                .ok_or(AuthProductError::BackendConflict)?,
            pkce_verifier_hash: flow
                .pkce_verifier_hash
                .clone()
                .ok_or(AuthProductError::BackendConflict)?,
            expires_at: flow.expires_at,
            created_at: flow.created_at,
            continuation: flow.continuation.clone(),
            update_binding: flow.update_binding.clone(),
        })
    }
    pub fn verify_flow(&self, flow: &AuthFlowRecord) -> Result<(), AuthProductError> {
        if self.scope != flow.scope || self.flow_id != flow.id {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if *self
            != Self::from_flow(
                flow,
                self.account_label.clone(),
                self.requested_scopes.clone(),
            )?
        {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(())
    }
    pub fn cleanup_projection(
        &self,
        flow: &AuthFlowRecord,
    ) -> Result<DcrCleanupProjection, AuthProductError> {
        self.verify_flow(flow)?;
        // Successful account publication retains its client registration. Only
        // unsuccessful terminal flows need cleanup which old schema cannot prove.
        let unavailable =
            is_terminal_status(flow.status) && flow.status != AuthFlowStatus::Completed;
        Ok(DcrCleanupProjection {
            remote: if unavailable {
                DcrRemoteCleanupStatus::Unavailable
            } else {
                DcrRemoteCleanupStatus::Retained
            },
            local: DcrLocalCleanupStatus::Retained,
            issue: unavailable.then_some(DcrCleanupUnavailable::ManagementUnavailable),
            http_status: None,
            error: None,
            can_reconcile: false,
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DcrLegacySelection {
    pub original: DcrLegacyOriginal,
    pub legacy_pkce: DcrMaterialReference,
    pub legacy_client: DcrMaterialReference,
    pub retained: DcrMaterialReference,
}
impl DcrLegacySelection {
    pub fn context(&self) -> serde_json::Value {
        Self::retention_context(&self.original, &self.legacy_pkce, &self.legacy_client)
    }
    pub fn retention_context(
        original: &DcrLegacyOriginal,
        pkce: &DcrMaterialReference,
        client: &DcrMaterialReference,
    ) -> serde_json::Value {
        serde_json::json!({"format":"legacy-dcr-selection/1","original":original,
            "legacy_pkce":pkce,"legacy_client":client})
    }
    pub fn validate(&self) -> Result<(), AuthProductError> {
        self.legacy_pkce.validate()?;
        self.legacy_client.validate()?;
        self.retained.validate()?;
        let suffix = format!(
            "-oauth-dcr-flow-legacy-selection-original-{}",
            self.original.flow_id
        );
        let prefix = self
            .retained
            .handle
            .as_str()
            .strip_suffix(&suffix)
            .filter(|prefix| !prefix.is_empty())
            .ok_or(AuthProductError::BackendConflict)?;
        if self.legacy_pkce.handle.as_str()
            != format!("{prefix}-oauth-dcr-flow-pkce-{}", self.original.flow_id)
            || self.legacy_client.handle.as_str()
                != format!("{prefix}-oauth-dcr-flow-client-{}", self.original.flow_id)
        {
            return Err(AuthProductError::BackendConflict);
        }
        if self.legacy_pkce.handle == self.legacy_client.handle
            || self.retained.handle == self.legacy_pkce.handle
            || self.retained.handle == self.legacy_client.handle
        {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DcrLegacyState {
    pub flow: AuthFlowRecord,
    pub selection: DcrLegacySelection,
}
impl AuthFlowCustody {
    pub fn retain_legacy_dcr(
        &mut self,
        flow: &AuthFlowRecord,
        selection: DcrLegacySelection,
    ) -> Result<DcrLegacyState, AuthProductError> {
        selection.validate()?;
        selection.original.verify_flow(flow)?;
        if self.dcr_registration.is_some() || self.oauth_setup.is_some() {
            return Err(AuthProductError::BackendConflict);
        }
        if let Some(existing) = &self.dcr_legacy {
            if *existing != selection {
                return Err(AuthProductError::BackendConflict);
            }
        } else {
            self.dcr_legacy = Some(selection.clone());
        }
        Ok(DcrLegacyState {
            flow: flow.clone(),
            selection,
        })
    }
}
