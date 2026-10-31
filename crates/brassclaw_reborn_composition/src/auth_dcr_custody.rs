//! Authenticate DCR generations using the existing encrypted SecretStore owner.
//! No raw decrypt port, registration dispatcher or additional persistence owner.
use brassclaw_auth::{
    AuthProductError, DcrMaterialReference, DcrRegistrationAction, DcrRegistrationChallenge,
    DcrRegistrationOriginal, DcrRegistrationOutcome,
};
use brassclaw_host_api::ResourceScope;
use brassclaw_secrets::{SecretCustodyState, SecretStore};

async fn original(
    store: &dyn SecretStore,
    scope: &ResourceScope,
    selected: &DcrMaterialReference,
    context: &serde_json::Value,
) -> Result<(), AuthProductError> {
    let metadata = store
        .custody(scope, &selected.handle)
        .await
        .map_err(|_| AuthProductError::BackendUnavailable)?
        .ok_or(AuthProductError::BackendConflict)?;
    if metadata.scope != *scope
        || metadata.handle != selected.handle
        || metadata.reference != selected.reference
        || metadata.state
            != (SecretCustodyState::Original {
                context: context.clone(),
            })
    {
        return Err(AuthProductError::BackendConflict);
    }
    Ok(())
}

pub(super) async fn preparation(
    store: &dyn SecretStore,
    selected: &DcrRegistrationOriginal,
) -> Result<(), AuthProductError> {
    original(
        store,
        &selected.scope.resource,
        &selected.preparation,
        &selected.preparation_context(),
    )
    .await
}

/// A surviving preparation alone is not a no-effect proof. Require its exact
/// original custody and absence of every subsequent private artifact. The flow
/// owner must still win an absent-record CAS; a concurrent live original wins.
pub(super) async fn unsubmitted_preparation(
    store: &dyn SecretStore,
    selected: &DcrRegistrationOriginal,
) -> Result<(), AuthProductError> {
    preparation(store, selected).await?;
    for slot in brassclaw_auth::DcrArtifactSlot::ALL {
        if slot == brassclaw_auth::DcrArtifactSlot::PreparationOriginal {
            continue;
        }
        if store
            .custody(&selected.scope.resource, &selected.artifact_handle(slot)?)
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
            .is_some()
        {
            return Err(AuthProductError::BackendConflict);
        }
    }
    Ok(())
}

pub(super) async fn response(
    store: &dyn SecretStore,
    action: &DcrRegistrationAction,
    outcome: &DcrRegistrationOutcome,
) -> Result<(), AuthProductError> {
    if let DcrRegistrationOutcome::Returned {
        status, response, ..
    } = outcome
    {
        original(
            store,
            &action.original.scope.resource,
            response,
            &action.response_context(*status)?,
        )
        .await?;
    }
    Ok(())
}

pub(super) async fn outputs(
    store: &dyn SecretStore,
    action: &DcrRegistrationAction,
    challenge: &DcrRegistrationChallenge,
) -> Result<(), AuthProductError> {
    let scope = &action.original.scope.resource;
    for (usage, selected) in [("pkce", &challenge.pkce), ("client", &challenge.client)] {
        let metadata = store
            .custody(scope, &selected.handle)
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
            .ok_or(AuthProductError::BackendConflict)?;
        if metadata.scope != *scope
            || metadata.handle != selected.handle
            || metadata.reference != selected.reference
        {
            return Err(AuthProductError::BackendConflict);
        }
        let SecretCustodyState::Retained { source } = metadata.state else {
            return Err(AuthProductError::BackendConflict);
        };
        if source.scope != *scope {
            return Err(AuthProductError::CrossScopeDenied);
        }
        original(
            store,
            scope,
            &DcrMaterialReference {
                handle: source.handle,
                reference: source.reference,
            },
            &action.output_context(usage, &challenge.authorization_url)?,
        )
        .await?;
    }
    Ok(())
}

pub(super) async fn cleanup_management(
    store: &dyn SecretStore,
    action: &DcrRegistrationAction,
    prepared: &brassclaw_auth::DcrCleanupPrepared,
) -> Result<(), AuthProductError> {
    action.original.verify(&prepared.original)?;
    original(
        store,
        &action.original.scope.resource,
        &prepared.management,
        &action.cleanup_management_context()?,
    )
    .await
}

pub(super) async fn cleanup_response(
    store: &dyn SecretStore,
    action: &DcrRegistrationAction,
    completion: &brassclaw_auth::DcrCleanupCompletion,
) -> Result<(), AuthProductError> {
    if action.cleanup.claim() != Some(&completion.dispatched) {
        return Err(AuthProductError::BackendConflict);
    }
    if let brassclaw_auth::DcrCleanupOutcome::Returned {
        status,
        empty_body,
        response,
    } = &completion.outcome
    {
        original(
            store,
            &action.original.scope.resource,
            response,
            &action.cleanup_response_context(*status, *empty_body)?,
        )
        .await?;
    }
    Ok(())
}

pub(super) async fn legacy_selection(
    store: &dyn SecretStore,
    selected: &brassclaw_auth::DcrLegacySelection,
) -> Result<(), AuthProductError> {
    selected.validate()?;
    original(
        store,
        &selected.original.scope.resource,
        &selected.retained,
        &selected.context(),
    )
    .await
}
