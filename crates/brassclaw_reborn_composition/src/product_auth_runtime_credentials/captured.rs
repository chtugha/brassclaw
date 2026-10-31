//! Original account/source metadata selected once for a native task family.
//! The existing catalogue preparation owns capture; no second task registry.
use std::sync::Arc;

use brassclaw_auth::{
    AuthProductError, AuthProductScope, CredentialAccount, CredentialAccountRecordSource,
    CredentialAccountStatus, CredentialRequirementSelection,
};
use brassclaw_host_api::{
    CredentialStageError, RuntimeCredentialAccountRequirement, RuntimeCredentialRequirementSource,
};
use brassclaw_host_runtime::{RuntimeCredentialAccessSecret, RuntimeCredentialAccountRequest};
use brassclaw_secrets::SecretStore;

/// A completed capture always distinguishes absence from a selected unusable
/// account and an unavailable encrypted generation. None is never "not captured".
#[derive(Clone)]
enum CapturedAccount {
    Unselected,
    Missing,
    Present {
        account: Box<CredentialAccount>,
        source: Option<Box<RuntimeCredentialAccessSecret>>,
    },
}

#[derive(Clone)]
pub(crate) struct CapturedAccountUsage {
    requirement: RuntimeCredentialAccountRequirement,
    scope: AuthProductScope,
    // Preserve this even if the selected account is missing, revoked or expired.
    previous: Option<CredentialRequirementSelection>,
    captured: CapturedAccount,
    accounts: Arc<dyn CredentialAccountRecordSource>,
}

impl std::fmt::Debug for CapturedAccountUsage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CapturedAccountUsage")
            .field("requirement", &self.requirement)
            .field("scope", &self.scope)
            .field("selected", &self.previous.is_some())
            .field("source", &"<retained-host-only>")
            .finish()
    }
}

impl CapturedAccountUsage {
    pub(crate) async fn capture(
        accounts: Arc<dyn CredentialAccountRecordSource>,
        secrets: &dyn SecretStore,
        scope: AuthProductScope,
        requirement: RuntimeCredentialAccountRequirement,
    ) -> Result<Self, AuthProductError> {
        // This is the only selection-head read. All later reads address the
        // original account location, even after another task selects account B.
        let previous = accounts.requirement_selection(&scope, &requirement).await?;
        let captured = match &previous {
            None => CapturedAccount::Unselected,
            Some(selection) => {
                selection.verify_requested(&scope, &requirement)?;
                let account = accounts
                    .account_record(selection.original_scope(), selection.account().id)
                    .await?;
                match account {
                    None => CapturedAccount::Missing,
                    Some(account) => {
                        verify_identity(selection, &account)?;
                        // Capture authenticated generation metadata even for a
                        // currently expired/unusable account. Eligibility is a
                        // separate current-use check, not source selection.
                        let source = match &account.access_secret {
                            None => None,
                            Some(handle) => match secrets
                                .record_reference(&account.scope.resource, handle)
                                .await
                                .map_err(|_| AuthProductError::BackendUnavailable)?
                            {
                                None => None,
                                Some(reference) if reference.is_empty() => {
                                    return Err(AuthProductError::BackendUnavailable);
                                }
                                Some(record_reference) => {
                                    Some(Box::new(RuntimeCredentialAccessSecret {
                                        scope: account.scope.resource.clone(),
                                        handle: handle.clone(),
                                        record_reference,
                                        access_expires_at: account.access_expires_at,
                                    }))
                                }
                            },
                        };
                        // Separate async owners are not an atomic transaction.
                        // Reject a metadata race, preserving the already chosen
                        // original rather than silently retrying a new selection.
                        if accounts
                            .account_record(selection.original_scope(), account.id)
                            .await?
                            .as_ref()
                            != Some(&account)
                        {
                            return Err(AuthProductError::BackendConflict);
                        }
                        CapturedAccount::Present {
                            account: Box::new(account),
                            source,
                        }
                    }
                }
            }
        };
        Ok(Self {
            requirement,
            scope,
            previous,
            captured,
            accounts,
        })
    }

    pub(crate) fn previous(&self) -> Option<&CredentialRequirementSelection> {
        self.previous.as_ref()
    }

    pub(crate) fn owns_source(&self, candidate: &RuntimeCredentialAccessSecret) -> bool {
        let CapturedAccount::Present {
            source: Some(original),
            ..
        } = &self.captured
        else {
            return false;
        };
        original.scope == candidate.scope
            && original.handle == candidate.handle
            && original.record_reference == candidate.record_reference
            && match (original.access_expires_at, candidate.access_expires_at) {
                (Some(original), Some(candidate)) => candidate <= original,
                (Some(_), None) => false,
                (None, _) => true,
            }
    }

    /// Called only by the exact invocation-bound, claim-fenced resolver. This
    /// returns the original generation; the real SecretStore still performs
    /// conditional lease/consume and expiry checks. No lease or access is granted.
    pub(crate) async fn resolve(
        &self,
        request: RuntimeCredentialAccountRequest<'_>,
    ) -> Result<RuntimeCredentialAccessSecret, CredentialStageError> {
        if request.requirement != Some(&self.requirement)
            || !self.requirement.matches_dispatch(
                &self.requirement.requirement().handle,
                request.provider,
                request.provider_scopes,
                request.requester_extension,
            )
        {
            return Err(CredentialStageError::Backend);
        }
        let Some(selection) = self.previous() else {
            return Err(CredentialStageError::AuthRequired);
        };
        selection
            .verify_requested(&self.scope, &self.requirement)
            .map_err(|_| CredentialStageError::Backend)?;
        let CapturedAccount::Present {
            account: captured,
            source: Some(source),
        } = &self.captured
        else {
            return Err(CredentialStageError::AuthRequired);
        };
        let current = self
            .accounts
            .account_record(selection.original_scope(), captured.id)
            .await
            .map_err(|_| CredentialStageError::Backend)?
            .ok_or(CredentialStageError::AuthRequired)?;
        verify_identity(selection, &current).map_err(|_| CredentialStageError::Backend)?;
        if current.access_secret.as_ref() != Some(&source.handle)
            || current.effective_status() != CredentialAccountStatus::Configured
            || !current.is_authorized_for_requester(Some(request.requester_extension))
            || !self
                .requirement
                .requirement()
                .provider_scopes
                .iter()
                .all(|required| {
                    current
                        .scopes
                        .iter()
                        .any(|actual| actual.as_str() == required)
                })
            || !super::account_visible_from_runtime_scope(
                &current,
                &AuthProductScope::new(request.scope.clone(), brassclaw_auth::AuthSurface::Api),
            )
        {
            return Err(CredentialStageError::AuthRequired);
        }
        let mut result = source.as_ref().clone();
        result.access_expires_at = match (source.access_expires_at, current.access_expires_at) {
            (Some(original), Some(current)) => Some(original.min(current)),
            (original, current) => original.or(current),
        };
        Ok(result)
    }
}

fn verify_identity(
    selection: &CredentialRequirementSelection,
    account: &CredentialAccount,
) -> Result<(), AuthProductError> {
    let RuntimeCredentialRequirementSource::ProductAuthAccount { provider, .. } =
        &selection.requirement().requirement().source
    else {
        return Err(AuthProductError::BackendConflict);
    };
    if account.id != selection.account().id
        || account.scope != selection.account().scope
        || account.provider.as_str() != provider.as_str()
    {
        return Err(AuthProductError::BackendConflict);
    }
    Ok(())
}

#[cfg(test)]
#[path = "captured_tests.rs"]
pub(crate) mod tests;
