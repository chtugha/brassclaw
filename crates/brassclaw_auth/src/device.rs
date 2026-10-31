//! Native device-login metadata in the existing AuthFlow owner. This is not a
//! provider grant. Actual HTTP/secret originals are authenticated by the host.
use crate::{
    AuthContinuationRef, AuthErrorCode, AuthFlowId, AuthFlowKind, AuthFlowRecord, AuthFlowStatus,
    AuthProductError, AuthProductScope, AuthProviderId, AuthSurface, AuthorizationCodeHash,
    CredentialAccountId, CredentialAccountLabel, OAuthSetupMaterialReference,
    OAuthTokenDestinations, PkceVerifierHash, Timestamp,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Shared with the existing native provider binding. These exact JSON fields
/// identify the first client; a parsed descriptor is neither authority nor egress.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeDeviceOAuthClient {
    pub client_id: String,
    pub auth_endpoint: String,
    pub refresh_endpoint: String,
    pub api_base_url: String,
}
impl std::fmt::Debug for NativeDeviceOAuthClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeDeviceOAuthClient([REDACTED])")
    }
}
impl NativeDeviceOAuthClient {
    /// Syntax only. No current provider selection, original-store proof, network
    /// permission or generic System/Extension credential access is established.
    pub fn validate(&self) -> Result<(), AuthProductError> {
        if self.client_id.is_empty()
            || self.client_id.trim() != self.client_id
            || self.client_id.chars().any(char::is_control)
        {
            return Err(AuthProductError::MalformedConfig);
        }
        for endpoint in [
            &self.auth_endpoint,
            &self.refresh_endpoint,
            &self.api_base_url,
        ] {
            let url = url::Url::parse(endpoint).map_err(|_| AuthProductError::MalformedConfig)?;
            if !matches!(url.scheme(), "http" | "https")
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return Err(AuthProductError::MalformedConfig);
            }
        }
        Ok(())
    }
}

/// No raw user/device code, verifier, token or fake callback state is stored here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeDeviceLoginDefinition {
    pub scope: AuthProductScope,
    pub flow_id: AuthFlowId,
    pub account_id: CredentialAccountId,
    pub provider: AuthProviderId,
    pub account_label: CredentialAccountLabel,
    pub client: NativeDeviceOAuthClient,
    pub destinations: OAuthTokenDestinations,
    pub created_at: Timestamp,
    pub expires_at: Timestamp,
}
impl NativeDeviceLoginDefinition {
    pub fn validate(&self) -> Result<(), AuthProductError> {
        let resource = &self.scope.resource;
        if self.flow_id.as_uuid().is_nil()
            || self.account_id.as_uuid().is_nil()
            || self.expires_at <= self.created_at
            || self.destinations.access_secret == self.destinations.refresh_secret
        {
            return Err(AuthProductError::BackendConflict);
        }
        // Real authenticated installer provenance. This does not relax generic
        // System ownership/requester rules or create a reserved/fake user scope.
        if resource.is_system()
            || self.scope.surface != AuthSurface::SetupAdmin
            || self.scope.session_id.is_some()
            || resource.agent_id.is_some()
            || resource.project_id.is_some()
            || resource.thread_id.is_some()
        {
            return Err(AuthProductError::CrossScopeDenied);
        }
        self.client.validate()
    }
    pub fn preparation_context(&self) -> Result<serde_json::Value, AuthProductError> {
        self.validate()?;
        Ok(serde_json::json!({"kind":"native-device-preparation/1","definition":self}))
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeDeviceLoginOriginal {
    pub definition: NativeDeviceLoginDefinition,
    pub preparation: OAuthSetupMaterialReference,
}
impl NativeDeviceLoginOriginal {
    pub fn validate(&self) -> Result<(), AuthProductError> {
        self.definition.validate()?;
        if self.preparation.reference.is_empty() {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(())
    }
    pub fn awaiting_flow(&self) -> Result<AuthFlowRecord, AuthProductError> {
        self.validate()?;
        let d = &self.definition;
        Ok(AuthFlowRecord {
            id: d.flow_id,
            scope: d.scope.clone(),
            kind: AuthFlowKind::IntegrationCredential,
            status: AuthFlowStatus::Pending,
            provider: d.provider.clone(),
            challenge: None,
            continuation: AuthContinuationRef::SetupOnly,
            credential_account_id: None,
            update_binding: None,
            opaque_state_hash: None,
            pkce_verifier_hash: None,
            authorization_code_hash: None,
            error: None,
            continuation_emitted_at: None,
            created_at: d.created_at,
            updated_at: d.created_at,
            expires_at: d.expires_at,
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeDeviceRequestKind {
    UserCode,
    Poll,
    TokenExchange,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeDeviceRequest {
    pub id: Uuid,
    pub sequence: u64,
    pub kind: NativeDeviceRequestKind,
    pub submitted_at: Timestamp,
    pub source: OAuthSetupMaterialReference,
    pub outcome: NativeDeviceOutcome,
    pub destinations: Option<OAuthTokenDestinations>,
    pub authorization_code_hash: Option<AuthorizationCodeHash>,
    pub pkce_verifier_hash: Option<PkceVerifierHash>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum NativeDeviceOutcome {
    Submitted,
    Challenge {
        response: OAuthSetupMaterialReference,
        issuer_expires_at: Option<Timestamp>,
        expires_at: Timestamp,
        next_poll_at: Timestamp,
        interval_secs: u64,
    },
    Pending {
        response: OAuthSetupMaterialReference,
        next_poll_at: Timestamp,
        interval_secs: u64,
    },
    Authorized {
        response: OAuthSetupMaterialReference,
        authorization_code_hash: AuthorizationCodeHash,
        pkce_verifier_hash: PkceVerifierHash,
    },
    Tokens {
        response: OAuthSetupMaterialReference,
        request_started_at: Timestamp,
        access_expires_at: Timestamp,
        has_refresh_token: bool,
    },
    Rejected {
        response: OAuthSetupMaterialReference,
        error: AuthErrorCode,
    },
    Unknown {
        response: Option<OAuthSetupMaterialReference>,
        error: AuthErrorCode,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeDeviceTransportOriginal {
    pub request_id: Uuid,
    pub source: OAuthSetupMaterialReference,
    pub request_started_at: Timestamp,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeDeviceLoginAction {
    pub original: NativeDeviceLoginOriginal,
    pub requests: Vec<NativeDeviceRequest>,
    #[serde(default)]
    pub transport_originals: Vec<NativeDeviceTransportOriginal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_account: Option<crate::CredentialAccountSnapshotRef>,
}
impl NativeDeviceLoginAction {
    pub fn new(original: NativeDeviceLoginOriginal) -> Result<Self, AuthProductError> {
        original.validate()?;
        Ok(Self {
            original,
            requests: Vec::new(),
            transport_originals: Vec::new(),
            published_account: None,
        })
    }
    pub fn verify_flow(&self, flow: &AuthFlowRecord) -> Result<(), AuthProductError> {
        self.original.validate()?;
        let expected = self.original.awaiting_flow()?;
        if flow.id != expected.id
            || flow.scope != expected.scope
            || flow.kind != expected.kind
            || flow.provider != expected.provider
            || flow.created_at != expected.created_at
            || flow.expires_at != self.expiry()
            || flow.opaque_state_hash.is_some()
            || flow.update_binding.is_some()
            || flow.challenge.is_some()
            || flow.continuation != AuthContinuationRef::SetupOnly
        {
            return Err(AuthProductError::BackendConflict);
        }
        if (flow.status == AuthFlowStatus::Completed) != self.published_account.is_some() {
            return Err(AuthProductError::BackendConflict);
        }
        if self.published_account.is_some() {
            let last = self
                .requests
                .last()
                .ok_or(AuthProductError::BackendConflict)?;
            if !matches!(last.outcome, NativeDeviceOutcome::Tokens { .. })
                || flow.credential_account_id != Some(self.original.definition.account_id)
                || flow.authorization_code_hash != last.authorization_code_hash
                || flow.pkce_verifier_hash != last.pkce_verifier_hash
            {
                return Err(AuthProductError::BackendConflict);
            }
        } else if flow.credential_account_id.is_some()
            || flow.authorization_code_hash.is_some()
            || flow.pkce_verifier_hash.is_some()
        {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(())
    }
    /// The storage owner must authenticate Ready/raw custody and the actual
    /// account row in its caller-owned publication transaction first.
    pub fn finish(
        &mut self,
        flow: &mut AuthFlowRecord,
        account: &crate::CredentialAccount,
        now: Timestamp,
    ) -> Result<(), AuthProductError> {
        self.verify_flow(flow)?;
        if self.published_account.is_some() || flow.status != AuthFlowStatus::Completing {
            return Err(AuthProductError::BackendConflict);
        }
        let last = self
            .requests
            .last()
            .ok_or(AuthProductError::BackendConflict)?;
        let NativeDeviceOutcome::Tokens {
            access_expires_at,
            has_refresh_token,
            ..
        } = &last.outcome
        else {
            return Err(AuthProductError::BackendConflict);
        };
        if last.submitted_at >= self.expiry() {
            return Err(AuthProductError::BackendConflict);
        }
        let definition = &self.original.definition;
        let expected_refresh =
            has_refresh_token.then(|| definition.destinations.refresh_secret.clone());
        if account.id != definition.account_id
            || account.scope != definition.scope
            || account.provider != definition.provider
            || account.label != definition.account_label
            || account.ownership != crate::CredentialOwnership::System
            || account.status != crate::CredentialAccountStatus::Configured
            || account.owner_extension.is_some()
            || !account.granted_extensions.is_empty()
            || !account.scopes.is_empty()
            || account.access_secret.as_ref() != Some(&definition.destinations.access_secret)
            || account.refresh_secret != expected_refresh
            || account.access_expires_at != Some(*access_expires_at)
            || *access_expires_at <= now
        {
            return Err(AuthProductError::BackendConflict);
        }
        let account_ref = account.snapshot_ref()?;
        let code = last
            .authorization_code_hash
            .clone()
            .ok_or(AuthProductError::BackendConflict)?;
        let verifier = last
            .pkce_verifier_hash
            .clone()
            .ok_or(AuthProductError::BackendConflict)?;
        self.published_account = Some(account_ref);
        crate::flow::complete_credential_flow(flow, account.id, code, verifier, now);
        Ok(())
    }
    pub fn expiry(&self) -> Timestamp {
        self.requests
            .iter()
            .find_map(|request| match &request.outcome {
                NativeDeviceOutcome::Challenge { expires_at, .. } => Some(*expires_at),
                _ => None,
            })
            .unwrap_or(self.original.definition.expires_at)
            .min(self.original.definition.expires_at)
    }
    /// A successful durable claim is necessary but not sufficient for HTTP:
    /// the concrete host must also authenticate its exact encrypted source.
    /// Existing Submitted/Unknown outcomes NEVER authorize another request.
    pub fn claim(
        &mut self,
        id: Uuid,
        now: Timestamp,
    ) -> Result<Option<NativeDeviceRequest>, AuthProductError> {
        if id.is_nil() || now < self.original.definition.created_at {
            return Err(AuthProductError::BackendConflict);
        }
        if self.requests.iter().any(|r| r.id == id) {
            return Ok(None);
        }
        if now >= self.expiry() {
            return Err(AuthProductError::UnknownOrExpiredFlow);
        }
        let selected = match self.requests.last().map(|r| &r.outcome) {
            None => Some((
                NativeDeviceRequestKind::UserCode,
                self.original.preparation.clone(),
            )),
            Some(NativeDeviceOutcome::Challenge { next_poll_at, .. })
            | Some(NativeDeviceOutcome::Pending { next_poll_at, .. })
                if now >= *next_poll_at =>
            {
                let source = self
                    .requests
                    .iter()
                    .find_map(|r| match &r.outcome {
                        NativeDeviceOutcome::Challenge { response, .. } => Some(response.clone()),
                        _ => None,
                    })
                    .ok_or(AuthProductError::BackendConflict)?;
                Some((NativeDeviceRequestKind::Poll, source))
            }
            Some(NativeDeviceOutcome::Authorized { response, .. }) => {
                Some((NativeDeviceRequestKind::TokenExchange, response.clone()))
            }
            _ => None,
        };
        let Some((kind, source)) = selected else {
            return Ok(None);
        };
        let (destinations, code, verifier) = if kind == NativeDeviceRequestKind::TokenExchange {
            let Some(NativeDeviceOutcome::Authorized {
                authorization_code_hash,
                pkce_verifier_hash,
                ..
            }) = self.requests.last().map(|r| &r.outcome)
            else {
                return Err(AuthProductError::BackendConflict);
            };
            (
                Some(self.original.definition.destinations.clone()),
                Some(authorization_code_hash.clone()),
                Some(pkce_verifier_hash.clone()),
            )
        } else {
            (None, None, None)
        };
        let request = NativeDeviceRequest {
            id,
            sequence: u64::try_from(self.requests.len())
                .map_err(|_| AuthProductError::BackendConflict)?,
            kind,
            submitted_at: now,
            source,
            outcome: NativeDeviceOutcome::Submitted,
            destinations,
            authorization_code_hash: code,
            pkce_verifier_hash: verifier,
        };
        self.requests.push(request.clone());
        Ok(Some(request))
    }
    pub fn record(
        &mut self,
        dispatched: &NativeDeviceRequest,
        outcome: NativeDeviceOutcome,
    ) -> Result<(), AuthProductError> {
        if !matches!(dispatched.outcome, NativeDeviceOutcome::Submitted)
            || matches!(outcome, NativeDeviceOutcome::Submitted)
        {
            return Err(AuthProductError::BackendConflict);
        }
        let index =
            usize::try_from(dispatched.sequence).map_err(|_| AuthProductError::BackendConflict)?;
        let actual = self
            .requests
            .get_mut(index)
            .ok_or(AuthProductError::BackendConflict)?;
        let mut expected = actual.clone();
        expected.outcome = NativeDeviceOutcome::Submitted;
        if expected != *dispatched {
            return Err(AuthProductError::BackendConflict);
        }
        if !matches!(actual.outcome, NativeDeviceOutcome::Submitted) {
            return if actual.outcome == outcome {
                Ok(())
            } else {
                Err(AuthProductError::BackendConflict)
            };
        }
        let valid = matches!(
            (&dispatched.kind, &outcome),
            (
                NativeDeviceRequestKind::UserCode,
                NativeDeviceOutcome::Challenge { .. }
            ) | (
                NativeDeviceRequestKind::Poll,
                NativeDeviceOutcome::Pending { .. }
            ) | (
                NativeDeviceRequestKind::Poll,
                NativeDeviceOutcome::Authorized { .. }
            ) | (
                NativeDeviceRequestKind::TokenExchange,
                NativeDeviceOutcome::Tokens { .. }
            ) | (_, NativeDeviceOutcome::Rejected { .. })
                | (_, NativeDeviceOutcome::Unknown { .. })
        );
        if !valid {
            return Err(AuthProductError::BackendConflict);
        }
        actual.outcome = outcome;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_native_client_preserves_existing_wire_fields_and_redacts_diagnostics() {
        let client = NativeDeviceOAuthClient {
            client_id: "public-client-marker".into(),
            auth_endpoint: "https://issuer.example".into(),
            refresh_endpoint: "https://issuer.example/oauth/token".into(),
            api_base_url: "https://api.example/backend".into(),
        };
        client.validate().unwrap();
        let value = serde_json::to_value(&client).unwrap();
        assert_eq!(
            value,
            serde_json::json!({"client_id":"public-client-marker","auth_endpoint":"https://issuer.example",
            "refresh_endpoint":"https://issuer.example/oauth/token","api_base_url":"https://api.example/backend"})
        );
        let diagnostic = format!("{client:?}");
        assert!(!diagnostic.contains("issuer.example"));
        assert!(!diagnostic.contains("public-client-marker"));
        for invalid in ["", " padded", "control\n"] {
            let mut invalid_client = client.clone();
            invalid_client.client_id = invalid.into();
            assert!(invalid_client.validate().is_err());
        }
        for invalid in [
            "ftp://issuer.example",
            "https://user:pass@issuer.example",
            "https://issuer.example?code=x",
            "https://issuer.example#fragment",
            "not-a-url",
        ] {
            for field in 0..3 {
                let mut invalid_client = client.clone();
                match field {
                    0 => invalid_client.auth_endpoint = invalid.into(),
                    1 => invalid_client.refresh_endpoint = invalid.into(),
                    _ => invalid_client.api_base_url = invalid.into(),
                }
                assert!(invalid_client.validate().is_err());
            }
        }
    }
}
