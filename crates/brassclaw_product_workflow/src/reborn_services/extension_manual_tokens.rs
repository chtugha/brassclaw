//! Typed per-requirement management handoff. Raw material has a dedicated,
//! non-serializable boundary; lifecycle payloads never carry credential values.
use brassclaw_auth::{
    AuthContinuationRef, AuthFlowStatus, AuthProviderId, CredentialAccountId,
    CredentialAccountLabel, CredentialAccountStatus, ManualTokenSetupIdentity, Timestamp,
};
use brassclaw_host_api::{ExtensionId, InvocationId};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

use super::{ExtensionCredentialSetupService, ExtensionCredentialSubmitRequest, lifecycle_setup};
use crate::{
    LifecycleExtensionCredentialSetup, LifecyclePackageKind, LifecyclePackageRef,
    LifecycleProductContext, LifecycleProductFacade, LifecycleProductPayload,
    LifecycleProductSurfaceContext, RebornServicesError, WebUiAuthenticatedCaller,
    WebUiInboundValidationCode,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExtensionCredentialTarget {
    New {},
    Existing { account_id: CredentialAccountId },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebUiExtensionManualTokenSetupRequest {
    pub requirement: String,
    pub identity: ManualTokenSetupIdentity,
    pub invocation_id: InvocationId,
    pub target: ExtensionCredentialTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebUiExtensionManualTokenReferenceRequest {
    pub requirement: String,
    pub identity: ManualTokenSetupIdentity,
    pub invocation_id: InvocationId,
}

/// No Serialize/Deserialize/Clone: only the dedicated HTTP secret body constructs
/// this value. Debug is explicitly redacted independently of SecretString's impl.
pub struct WebUiExtensionManualTokenSubmitRequest {
    pub requirement: String,
    pub identity: ManualTokenSetupIdentity,
    pub invocation_id: InvocationId,
    pub secret: SecretString,
}
impl std::fmt::Debug for WebUiExtensionManualTokenSubmitRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebUiExtensionManualTokenSubmitRequest")
            .field("requirement", &self.requirement)
            .field("identity", &self.identity)
            .field("invocation_id", &self.invocation_id)
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionCredentialPrepareRequest {
    pub scope: brassclaw_auth::AuthProductScope,
    pub requester_extension: ExtensionId,
    pub requirement_name: String,
    /// Current server-resolved candidate for a NEW setup only. An existing
    /// original keeps its stored requirement even after package replacement.
    pub candidate_requirement: Option<brassclaw_host_api::RuntimeCredentialAccountRequirement>,
    pub identity: ManualTokenSetupIdentity,
    pub target: ExtensionCredentialTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionCredentialReferenceRequest {
    pub scope: brassclaw_auth::AuthProductScope,
    pub requester_extension: ExtensionId,
    pub requirement_name: String,
    pub identity: ManualTokenSetupIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionManualTokenSetupReceipt {
    pub identity: ManualTokenSetupIdentity,
    pub invocation_id: InvocationId,
    pub status: AuthFlowStatus,
    pub provider: AuthProviderId,
    pub label: CredentialAccountLabel,
    pub expires_at: Timestamp,
}

/// Recovered original metadata. This is neither a new setup nor a grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionManualTokenOriginal {
    pub receipt: ExtensionManualTokenSetupReceipt,
    pub target: ExtensionCredentialTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionManualTokenCompletion {
    pub identity: ManualTokenSetupIdentity,
    pub invocation_id: InvocationId,
    pub credential_ref: CredentialAccountId,
    pub status: CredentialAccountStatus,
    pub continuation: AuthContinuationRef,
}

/// Stable presentation label; the exact stored usage owns identity/authority.
pub fn manual_label(package_id: &str, provider: &str, requirement: &str) -> String {
    // Every manual requirement is distinct, including same-provider usages.
    // OAuth retains its own existing label contract.
    if requirement.starts_with("sha256:") {
        // Pair identifiers may each be long. The complete reference is already
        // unique and fits the existing account-label contract without truncation.
        format!("Credential {requirement}")
    } else {
        // Preserve old original labels and explicitly named legacy fixtures.
        format!("{package_id} {provider} {requirement}")
    }
}

fn context(
    caller: &WebUiAuthenticatedCaller,
    package: &LifecyclePackageRef,
    requirement_name: &str,
    invocation: InvocationId,
    identity: ManualTokenSetupIdentity,
) -> Result<ExtensionCredentialReferenceRequest, RebornServicesError> {
    let scope = credential_scope(caller, package, requirement_name, invocation)?;
    Ok(ExtensionCredentialReferenceRequest {
        scope,
        requester_extension: ExtensionId::new(package.id.as_str())
            .map_err(|_| RebornServicesError::internal_invariant())?,
        requirement_name: requirement_name.into(),
        identity,
    })
}

fn credential_scope(
    caller: &WebUiAuthenticatedCaller,
    package: &LifecyclePackageRef,
    requirement_name: &str,
    invocation: InvocationId,
) -> Result<brassclaw_auth::AuthProductScope, RebornServicesError> {
    package
        .require_kind(LifecyclePackageKind::Extension)
        .map_err(lifecycle_setup::map_lifecycle_error)?;
    let scope = lifecycle_setup::setup_scope(caller, package);
    if invocation != scope.resource.invocation_id {
        return Err(lifecycle_setup::validation_error(
            "invocation_id",
            WebUiInboundValidationCode::InvalidValue,
        ));
    }
    if requirement_name.is_empty() || requirement_name.trim() != requirement_name {
        return Err(lifecycle_setup::validation_error(
            "requirement",
            WebUiInboundValidationCode::InvalidValue,
        ));
    }
    Ok(scope)
}

#[derive(Clone, Copy)]
enum SetupKind {
    Manual,
    OAuth,
}

/// Only a NEW OAuth original may resolve a current declaration. The caller
/// recovers accepted originals before invoking this server-owned resolver.
pub(super) async fn resolve_oauth_requirement(
    facade: &dyn LifecycleProductFacade,
    caller: WebUiAuthenticatedCaller,
    package: LifecyclePackageRef,
    requirement_name: &str,
    invocation: InvocationId,
) -> Result<brassclaw_host_api::RuntimeCredentialAccountRequirement, RebornServicesError> {
    credential_scope(&caller, &package, requirement_name, invocation)?;
    candidate_requirement(facade, caller, package, requirement_name, SetupKind::OAuth).await
}

async fn candidate_requirement(
    facade: &dyn LifecycleProductFacade,
    caller: WebUiAuthenticatedCaller,
    package: LifecyclePackageRef,
    requirement_name: &str,
    setup_kind: SetupKind,
) -> Result<brassclaw_host_api::RuntimeCredentialAccountRequirement, RebornServicesError> {
    let surface = LifecycleProductContext::Surface(LifecycleProductSurfaceContext {
        tenant_id: caller.tenant_id,
        user_id: caller.user_id,
        agent_id: caller.agent_id,
        project_id: caller.project_id,
    });
    let lifecycle = lifecycle_setup::project_package(facade, surface, package.clone()).await?;
    if lifecycle.package_ref.as_ref() != Some(&package) {
        return Err(RebornServicesError::internal_invariant());
    }
    let Some(LifecycleProductPayload::ExtensionList { extensions, .. }) = lifecycle.payload else {
        return Err(RebornServicesError::internal_invariant());
    };
    let mut selected = extensions
        .iter()
        .filter(|extension| extension.summary.package_ref == package);
    let extension = selected
        .next()
        .ok_or_else(RebornServicesError::internal_invariant)?;
    if selected.next().is_some() {
        return Err(RebornServicesError::internal_invariant());
    }
    let mut requirements = extension
        .summary
        .credential_requirements
        .iter()
        .filter(|requirement| requirement.name == requirement_name);
    let requirement = requirements.next().ok_or_else(|| {
        lifecycle_setup::validation_error("requirement", WebUiInboundValidationCode::InvalidValue)
    })?;
    if requirements.next().is_some()
        || !matches!(
            (&requirement.setup, setup_kind),
            (
                LifecycleExtensionCredentialSetup::ManualToken,
                SetupKind::Manual
            ) | (
                LifecycleExtensionCredentialSetup::OAuth { .. },
                SetupKind::OAuth
            )
        )
    {
        return Err(lifecycle_setup::validation_error(
            "requirement",
            WebUiInboundValidationCode::InvalidValue,
        ));
    }
    super::extension_setup_credentials::exact_requirement(
        requirement,
        &ExtensionId::new(package.id.as_str())
            .map_err(|_| RebornServicesError::internal_invariant())?,
    )?
    .ok_or_else(|| RebornServicesError::service_unavailable(false))
}

fn check_setup(
    receipt: &ExtensionManualTokenSetupReceipt,
    request: &ExtensionCredentialReferenceRequest,
) -> Result<(), RebornServicesError> {
    if receipt.identity != request.identity
        || receipt.invocation_id != request.scope.resource.invocation_id
        || receipt.label.as_str()
            != manual_label(
                request.requester_extension.as_str(),
                receipt.provider.as_str(),
                &request.requirement_name,
            )
        || !matches!(
            receipt.status,
            AuthFlowStatus::AwaitingUser
                | AuthFlowStatus::Completed
                | AuthFlowStatus::Canceled
                | AuthFlowStatus::Expired
                | AuthFlowStatus::Failed
        )
    {
        return Err(RebornServicesError::internal_invariant());
    }
    Ok(())
}

fn check_completion(
    receipt: &ExtensionManualTokenCompletion,
    request: &ExtensionCredentialReferenceRequest,
) -> Result<(), RebornServicesError> {
    if receipt.identity != request.identity
        || receipt.invocation_id != request.scope.resource.invocation_id
        || receipt.status != CredentialAccountStatus::Configured
        || receipt.continuation != AuthContinuationRef::SetupOnly
    {
        return Err(RebornServicesError::internal_invariant());
    }
    Ok(())
}

pub(super) async fn prepare(
    facade: &dyn LifecycleProductFacade,
    service: Option<&dyn ExtensionCredentialSetupService>,
    caller: WebUiAuthenticatedCaller,
    package: LifecyclePackageRef,
    request: WebUiExtensionManualTokenSetupRequest,
) -> Result<ExtensionManualTokenSetupReceipt, RebornServicesError> {
    let service = service.ok_or_else(|| RebornServicesError::service_unavailable(true))?;
    let context = context(
        &caller,
        &package,
        &request.requirement,
        request.invocation_id,
        request.identity,
    )?;
    // Read the original BEFORE a mutable package lookup. A replaced/removed
    // manifest cannot reinterpret an accepted name/identity or extend its TTL.
    if let Some(original) = service.recover_manual_token(context.clone()).await? {
        if original.target != request.target {
            return Err(RebornServicesError {
                code: super::RebornServicesErrorCode::Conflict,
                kind: super::RebornServicesErrorKind::BlockedAuthentication,
                status_code: 409,
                retryable: false,
                field: None,
                validation_code: None,
            });
        }
        check_setup(&original.receipt, &context)?;
        return Ok(original.receipt);
    }
    let candidate = candidate_requirement(
        facade,
        caller,
        package,
        &request.requirement,
        SetupKind::Manual,
    )
    .await?;
    let receipt = service
        .prepare_manual_token(ExtensionCredentialPrepareRequest {
            scope: context.scope.clone(),
            requester_extension: context.requester_extension.clone(),
            requirement_name: context.requirement_name.clone(),
            identity: context.identity,
            candidate_requirement: Some(candidate),
            target: request.target,
        })
        .await?;
    check_setup(&receipt, &context)?;
    Ok(receipt)
}

pub(super) async fn submit(
    service: Option<&dyn ExtensionCredentialSetupService>,
    caller: WebUiAuthenticatedCaller,
    package: LifecyclePackageRef,
    request: WebUiExtensionManualTokenSubmitRequest,
) -> Result<ExtensionManualTokenCompletion, RebornServicesError> {
    let service = service.ok_or_else(|| RebornServicesError::service_unavailable(true))?;
    let context = context(
        &caller,
        &package,
        &request.requirement,
        request.invocation_id,
        request.identity,
    )?;
    if request.secret.expose_secret().trim().is_empty() {
        return Err(lifecycle_setup::validation_error(
            "token",
            WebUiInboundValidationCode::Blank,
        ));
    }
    let receipt = service
        .submit_manual_token(ExtensionCredentialSubmitRequest {
            scope: context.scope.clone(),
            requirement_name: context.requirement_name.clone(),
            requester_extension: context.requester_extension.clone(),
            identity: context.identity,
            secret: request.secret,
        })
        .await?;
    check_completion(&receipt, &context)?;
    Ok(receipt)
}

pub(super) async fn continue_saved(
    service: Option<&dyn ExtensionCredentialSetupService>,
    caller: WebUiAuthenticatedCaller,
    package: LifecyclePackageRef,
    request: WebUiExtensionManualTokenReferenceRequest,
) -> Result<ExtensionManualTokenCompletion, RebornServicesError> {
    let service = service.ok_or_else(|| RebornServicesError::service_unavailable(true))?;
    let context = context(
        &caller,
        &package,
        &request.requirement,
        request.invocation_id,
        request.identity,
    )?;
    let receipt = service.continue_manual_token(context.clone()).await?;
    check_completion(&receipt, &context)?;
    Ok(receipt)
}

pub(super) async fn cancel(
    service: Option<&dyn ExtensionCredentialSetupService>,
    caller: WebUiAuthenticatedCaller,
    package: LifecyclePackageRef,
    request: WebUiExtensionManualTokenReferenceRequest,
) -> Result<ExtensionManualTokenSetupReceipt, RebornServicesError> {
    let service = service.ok_or_else(|| RebornServicesError::service_unavailable(true))?;
    let context = context(
        &caller,
        &package,
        &request.requirement,
        request.invocation_id,
        request.identity,
    )?;
    let receipt = service.cancel_manual_token(context.clone()).await?;
    check_setup(&receipt, &context)?;
    Ok(receipt)
}
