use std::fmt;

use async_trait::async_trait;
use brassclaw_host_api::SecretHandle;
use secrecy::{ExposeSecret, SecretString};

use crate::{
    AuthFlowId, AuthProductError, AuthProductScope, AuthorizationCodeHash, CredentialAccountId,
    CredentialAccountLabel, PkceVerifierHash, ProviderScope, ids::AuthProviderId,
};

macro_rules! one_shot_secret {
    ($name:ident, $label:literal) => {
        pub struct $name(SecretString);

        impl $name {
            pub fn new(value: SecretString) -> Result<Self, AuthProductError> {
                let exposed = value.expose_secret();
                if exposed.is_empty() {
                    return Err(AuthProductError::invalid_request(format!(
                        "{} must not be empty",
                        $label
                    )));
                }
                if exposed.trim() != exposed {
                    return Err(AuthProductError::invalid_request(format!(
                        "{} must not contain leading or trailing whitespace",
                        $label
                    )));
                }
                if exposed.chars().any(|c| c == '\0' || c.is_control()) {
                    return Err(AuthProductError::invalid_request(format!(
                        "{} must not contain NUL/control characters",
                        $label
                    )));
                }
                Ok(Self(value))
            }

            pub fn expose_secret(&self) -> &str {
                self.0.expose_secret()
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(concat!(stringify!($name), "([REDACTED])"))
            }
        }
    };
}

one_shot_secret!(OAuthAuthorizationCode, "oauth authorization code");
one_shot_secret!(PkceVerifierSecret, "pkce verifier");

/// One-shot provider exchange input. This type intentionally does not implement
/// serde traits because it may carry raw OAuth code and PKCE verifier material.
pub struct OAuthProviderCallbackRequest {
    pub provider: AuthProviderId,
    pub account_label: CredentialAccountLabel,
    pub authorization_code: OAuthAuthorizationCode,
    pub authorization_code_hash: AuthorizationCodeHash,
    pub pkce_verifier: PkceVerifierSecret,
    pub pkce_verifier_hash: PkceVerifierHash,
    pub scopes: Vec<ProviderScope>,
}

impl fmt::Debug for OAuthProviderCallbackRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuthProviderCallbackRequest")
            .field("provider", &self.provider)
            .field("account_label", &self.account_label)
            .field("authorization_code", &"[REDACTED]")
            .field("authorization_code_hash", &self.authorization_code_hash)
            .field("pkce_verifier", &"[REDACTED]")
            .field("pkce_verifier_hash", &self.pkce_verifier_hash)
            .field("scopes", &self.scopes)
            .finish()
    }
}

/// Provider-exchange context claimed by the product-auth flow before raw
/// provider material is exchanged or stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthProviderExchangeContext {
    pub scope: AuthProductScope,
    pub flow_id: AuthFlowId,
}

/// Provider-exchange result safe to store in auth-flow/account records.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OAuthProviderExchange {
    pub provider: AuthProviderId,
    pub account_label: CredentialAccountLabel,
    pub authorization_code_hash: AuthorizationCodeHash,
    pub pkce_verifier_hash: PkceVerifierHash,
    pub access_secret: SecretHandle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_expires_at: Option<crate::Timestamp>,
    pub refresh_secret: Option<SecretHandle>,
    pub scopes: Vec<ProviderScope>,
    pub account_id: Option<CredentialAccountId>,
}

/// One-shot provider refresh input. This type intentionally does not implement
/// serde traits because refresh authority must stay behind host-mediated
/// credential/egress boundaries.
#[derive(Clone, PartialEq, Eq)]
pub struct OAuthProviderRefreshRequest {
    pub refresh_id: crate::CredentialRefreshId,
    pub provider: AuthProviderId,
    pub scope: AuthProductScope,
    pub account_id: CredentialAccountId,
    pub refresh_secret: SecretHandle,
    pub scopes: Vec<ProviderScope>,
}

impl fmt::Debug for OAuthProviderRefreshRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuthProviderRefreshRequest")
            .field("refresh_id", &self.refresh_id)
            .field("provider", &self.provider)
            .field("scope", &self.scope)
            .field("account_id", &self.account_id)
            .field("refresh_secret", &"[REDACTED]")
            .field("scopes", &self.scopes)
            .finish()
    }
}

/// Exact authenticated encrypted source selected before durable refresh admission.
/// It is metadata, not a lease, secret value or dispatch/authorization grant.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OAuthRefreshSource {
    pub scope: brassclaw_host_api::ResourceScope,
    pub handle: SecretHandle,
    pub record_reference: String,
}
impl OAuthRefreshSource {
    pub fn verify(&self, request: &OAuthProviderRefreshRequest) -> Result<(), AuthProductError> {
        if self.scope != request.scope.resource
            || self.handle != request.refresh_secret
            || self.scope.is_system()
        {
            return Err(AuthProductError::CrossScopeDenied);
        }
        // Backend-owned opaque identity: do not normalize or assume a hash
        // encoding. Actual native/lease owners authenticate the exact value.
        if self.record_reference.is_empty() {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(())
    }
}
impl fmt::Debug for OAuthRefreshSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("OAuthRefreshSource([REDACTED])")
    }
}

/// Read-only provider preparation: deterministic destinations and the actual
/// encrypted input generation. Callback preparation has no refresh-source field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthProviderRefreshPreparation {
    pub destinations: OAuthTokenDestinations,
    pub source: OAuthRefreshSource,
}

/// Provider refresh result safe to store back into credential-account records.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OAuthProviderRefresh {
    pub provider: AuthProviderId,
    pub access_secret: SecretHandle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_expires_at: Option<crate::Timestamp>,
    pub refresh_secret: Option<SecretHandle>,
    pub scopes: Vec<ProviderScope>,
}

/// Actual provider-owned storage identities for one original refresh action.
/// Preparing them performs no issuer request and grants no authority.
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OAuthTokenDestinations {
    pub access_secret: SecretHandle,
    pub refresh_secret: SecretHandle,
}

#[async_trait]
pub trait AuthProviderClient: Send + Sync {
    /// Actual deterministic destinations only; no issuer/storage side effects.
    async fn prepare_callback(
        &self,
        context: &OAuthProviderExchangeContext,
        request: &OAuthProviderCallbackRequest,
    ) -> Result<OAuthTokenDestinations, AuthProductError>;

    /// Capture the exact encrypted input generation without issuing a lease or
    /// contacting the issuer. Native admission and use independently recheck it.
    async fn prepare_refresh(
        &self,
        request: &OAuthProviderRefreshRequest,
    ) -> Result<OAuthProviderRefreshPreparation, AuthProductError>;

    async fn exchange_callback(
        &self,
        context: OAuthProviderExchangeContext,
        request: OAuthProviderCallbackRequest,
    ) -> Result<OAuthProviderExchange, AuthProductError>;

    async fn refresh_token(
        &self,
        request: OAuthProviderRefreshRequest,
        preparation: OAuthProviderRefreshPreparation,
    ) -> Result<OAuthProviderRefresh, AuthProductError>;

    async fn cleanup_exchange(
        &self,
        _context: OAuthProviderExchangeContext,
        _exchange: &OAuthProviderExchange,
    ) -> Result<(), AuthProductError> {
        Ok(())
    }
}

pub fn validate_provider_callback_request(
    request: &OAuthProviderCallbackRequest,
) -> Result<(), AuthProductError> {
    if request.authorization_code.expose_secret().trim().is_empty()
        || request.pkce_verifier.expose_secret().trim().is_empty()
    {
        return Err(AuthProductError::MalformedCallback);
    }
    let code = crate::authorization_code_hash(&request.authorization_code)?;
    let verifier = crate::pkce_verifier_hash(&request.pkce_verifier)?;
    if !code.constant_time_eq(&request.authorization_code_hash)
        || !verifier.constant_time_eq(&request.pkce_verifier_hash)
    {
        return Err(AuthProductError::MalformedCallback);
    }
    Ok(())
}

impl fmt::Debug for OAuthTokenDestinations {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuthTokenDestinations")
            .field("access_secret", &"[REDACTED]")
            .field("refresh_secret", &"[REDACTED]")
            .finish()
    }
}
impl fmt::Debug for OAuthProviderRefresh {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuthProviderRefresh")
            .field("provider", &self.provider)
            .field("access_secret", &"[REDACTED]")
            .field(
                "refresh_secret",
                &self.refresh_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .field("scopes", &self.scopes)
            .finish()
    }
}
impl fmt::Debug for OAuthProviderExchange {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuthProviderExchange")
            .field("provider", &self.provider)
            .field("account_label", &self.account_label)
            .field("authorization_code_hash", &self.authorization_code_hash)
            .field("pkce_verifier_hash", &self.pkce_verifier_hash)
            .field("access_secret", &"[REDACTED]")
            .field(
                "refresh_secret",
                &self.refresh_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .field("scopes", &self.scopes)
            .field("account_id", &self.account_id)
            .finish()
    }
}
