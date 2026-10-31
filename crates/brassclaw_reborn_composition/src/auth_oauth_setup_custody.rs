//! Original OAuth setup authentication in the existing encrypted secret owner.
//! This performs no plaintext read, account selection or provider dispatch.
use brassclaw_auth::{AuthProductError, OAuthSetupOriginal};
use brassclaw_secrets::{SecretCustodyState, SecretStore};

pub(super) async fn unsubmitted_preparation(
    store: &dyn SecretStore,
    original: &OAuthSetupOriginal,
) -> Result<(), AuthProductError> {
    original.validate()?;
    let scope = &original.definition.scope.resource;
    let selected = &original.preparation;
    let actual = store
        .custody(scope, &selected.handle)
        .await
        .map_err(|_| AuthProductError::BackendUnavailable)?
        .ok_or(AuthProductError::BackendConflict)?;
    if actual.scope != *scope
        || actual.handle != selected.handle
        || actual.reference != selected.reference
        || actual.state
            != (SecretCustodyState::Original {
                context: original.definition.preparation_context(),
            })
    {
        return Err(AuthProductError::BackendConflict);
    }
    // Even a retired later-use record is not proof that exchange never began.
    // The flow owner must additionally win the existing absent-record CAS.
    if store
        .custody(scope, &original.definition.exchange_use_handle()?)
        .await
        .map_err(|_| AuthProductError::BackendUnavailable)?
        .is_some()
    {
        return Err(AuthProductError::BackendConflict);
    }
    Ok(())
}

#[cfg(all(test, feature = "postgres"))]
#[path = "auth_oauth_setup_custody_tests.rs"]
pub(crate) mod tests;
