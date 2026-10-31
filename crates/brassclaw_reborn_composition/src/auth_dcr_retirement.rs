//! Authenticated nonmaterial selections/closure receipts for the existing DCR
//! flow owner. Product sequencing stays in its original setup/cleanup path.
use brassclaw_auth::{
    AuthProductError, DcrArtifactExpectation as Expected, DcrArtifactSlot as Slot,
    DcrChallengePublication, DcrMaterialReference, DcrRegistrationAction, DcrRegistrationOutcome,
    DcrRetirementOutcome, DcrRetirementSelection, DcrRetirementUpdate, OAuthAuthorizationUrl,
};
use brassclaw_secrets::{
    SecretCustodyMetadata, SecretCustodySource, SecretCustodyState, SecretIdentityClosure,
    SecretStore,
};

pub(super) fn closure(
    action: &DcrRegistrationAction,
    selection: &DcrRetirementSelection,
) -> SecretIdentityClosure {
    match &selection.expected {
        Expected::Absent { context } => SecretIdentityClosure::Absent {
            context: context.clone(),
        },
        Expected::Original { reference, context } => SecretIdentityClosure::Original {
            reference: reference.clone(),
            context: context.clone(),
        },
        Expected::Retained { reference, source } => SecretIdentityClosure::Retained {
            reference: reference.clone(),
            source: SecretCustodySource {
                scope: action.original.scope.resource.clone(),
                handle: source.handle.clone(),
                reference: source.reference.clone(),
            },
        },
    }
}

fn original_context(
    action: &DcrRegistrationAction,
    slot: Slot,
    reference: &str,
    context: &serde_json::Value,
) -> Result<(), AuthProductError> {
    let expected = match slot {
        Slot::PreparationOriginal if reference == action.original.preparation.reference => {
            action.original.preparation_context()
        }
        Slot::RegistrationResponse => {
            let DcrRegistrationOutcome::Returned {
                status, response, ..
            } = &action.outcome
            else {
                return Err(AuthProductError::BackendConflict);
            };
            if reference != response.reference {
                return Err(AuthProductError::BackendConflict);
            }
            action.response_context(*status)?
        }
        Slot::PkceOriginal | Slot::ClientOriginal => {
            let url = context["authorization_url"]
                .as_str()
                .ok_or(AuthProductError::BackendConflict)?;
            let url =
                OAuthAuthorizationUrl::new(url).map_err(|_| AuthProductError::BackendConflict)?;
            action.output_context(
                if slot == Slot::PkceOriginal {
                    "pkce"
                } else {
                    "client"
                },
                &url,
            )?
        }
        Slot::CleanupManagement => {
            if action
                .cleanup
                .claim()
                .is_none_or(|claim| reference != claim.management.reference)
            {
                return Err(AuthProductError::BackendConflict);
            }
            action.cleanup_management_context()?
        }
        Slot::CleanupResponse => {
            let brassclaw_auth::DcrCleanupState::Returned {
                status,
                empty_body,
                response,
                ..
            } = &action.cleanup
            else {
                return Err(AuthProductError::BackendConflict);
            };
            if reference != response.reference {
                return Err(AuthProductError::BackendConflict);
            }
            action.cleanup_response_context(*status, *empty_body)?
        }
        _ => return Err(AuthProductError::BackendConflict),
    };
    if *context != expected {
        return Err(AuthProductError::BackendConflict);
    }
    Ok(())
}

fn original_parts(
    metadata: &SecretCustodyMetadata,
) -> Result<(&str, &serde_json::Value), AuthProductError> {
    match &metadata.state {
        SecretCustodyState::Original { context } => Ok((&metadata.reference, context)),
        SecretCustodyState::RetiredOriginal {
            original_reference,
            context,
        } => Ok((original_reference, context)),
        _ => Err(AuthProductError::BackendConflict),
    }
}

pub(super) async fn select(
    store: &dyn SecretStore,
    action: &DcrRegistrationAction,
    slot: Slot,
) -> Result<DcrRetirementSelection, AuthProductError> {
    let original = &action.original;
    let scope = &original.scope.resource;
    let handle = original.artifact_handle(slot)?;
    let mut metadata = store
        .custody(scope, &handle)
        .await
        .map_err(|_| AuthProductError::BackendUnavailable)?;
    if metadata.is_none()
        && store
            .record_reference(scope, &handle)
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
            .is_some()
    {
        // A racing custody writer may have committed between these reads. One
        // authenticated readback distinguishes it from ordinary live material.
        metadata = Some(
            store
                .custody(scope, &handle)
                .await
                .map_err(|_| AuthProductError::BackendUnavailable)?
                .ok_or(AuthProductError::BackendConflict)?,
        );
    }
    let Some(metadata) = metadata else {
        return Ok(DcrRetirementSelection {
            slot,
            handle,
            expected: Expected::Absent {
                context: original.absent_artifact_context(slot),
            },
        });
    };
    if metadata.scope != *scope || metadata.handle != handle {
        return Err(AuthProductError::CrossScopeDenied);
    }
    let expected = match &metadata.state {
        SecretCustodyState::ClosedIdentity { context }
            if *context == original.absent_artifact_context(slot) =>
        {
            Expected::Absent {
                context: context.clone(),
            }
        }
        SecretCustodyState::Original { .. } | SecretCustodyState::RetiredOriginal { .. } => {
            let (reference, context) = original_parts(&metadata)?;
            original_context(action, slot, reference, context)?;
            Expected::Original {
                reference: reference.to_owned(),
                context: context.clone(),
            }
        }
        SecretCustodyState::Retained { source }
        | SecretCustodyState::RetiredRetained { source, .. } => {
            let source_slot = slot
                .source_slot()
                .ok_or(AuthProductError::BackendConflict)?;
            if source.scope != *scope || source.handle != original.artifact_handle(source_slot)? {
                return Err(AuthProductError::BackendConflict);
            }
            let source_metadata = store
                .custody(scope, &source.handle)
                .await
                .map_err(|_| AuthProductError::BackendUnavailable)?
                .ok_or(AuthProductError::BackendConflict)?;
            if source_metadata.scope != *scope || source_metadata.handle != source.handle {
                return Err(AuthProductError::CrossScopeDenied);
            }
            let (reference, context) = original_parts(&source_metadata)?;
            if reference != source.reference {
                return Err(AuthProductError::BackendConflict);
            }
            original_context(action, source_slot, reference, context)?;
            let reference = match &metadata.state {
                SecretCustodyState::Retained { .. } => metadata.reference.clone(),
                SecretCustodyState::RetiredRetained {
                    retained_reference, ..
                } => retained_reference.clone(),
                _ => unreachable!("same authenticated metadata variant"),
            };
            if let DcrChallengePublication::Ready(challenge) = &action.publication {
                let selected = match slot {
                    Slot::Pkce => Some(&challenge.pkce),
                    Slot::Client => Some(&challenge.client),
                    _ => None,
                };
                if selected.is_some_and(|selected| {
                    selected.handle != handle || selected.reference != reference
                }) {
                    return Err(AuthProductError::BackendConflict);
                }
            }
            Expected::Retained {
                reference,
                source: DcrMaterialReference {
                    handle: source.handle.clone(),
                    reference: source.reference.clone(),
                },
            }
        }
        _ => return Err(AuthProductError::BackendConflict),
    };
    let selected = DcrRetirementSelection {
        slot,
        handle,
        expected,
    };
    selected.validate(original)?;
    Ok(selected)
}

pub(super) async fn validate_update(
    store: &dyn SecretStore,
    action: &DcrRegistrationAction,
    update: &DcrRetirementUpdate,
) -> Result<(), AuthProductError> {
    match update {
        DcrRetirementUpdate::Plan(selections) => {
            // A racing writer may already have refined an originally absent
            // slot. Re-reading that exact persisted plan is not a new selection
            // and cannot require the old absence to become true again. The
            // domain transition still validates complete/unique slot coverage.
            if update.satisfied_by(action) {
                return Ok(());
            }
            for selected in selections {
                if select(store, action, selected.slot).await? != *selected {
                    return Err(AuthProductError::BackendConflict);
                }
            }
        }
        DcrRetirementUpdate::Refine { selected, .. } => {
            if select(store, action, selected.slot).await? != *selected {
                return Err(AuthProductError::BackendConflict);
            }
        }
        DcrRetirementUpdate::Record {
            selected,
            outcome: DcrRetirementOutcome::Confirmed { closed_reference },
        } => {
            let metadata = store
                .custody(&action.original.scope.resource, &selected.handle)
                .await
                .map_err(|_| AuthProductError::BackendUnavailable)?
                .ok_or(AuthProductError::BackendConflict)?;
            if metadata.handle != selected.handle
                || metadata.reference != *closed_reference
                || !closure(action, selected).accepts(&action.original.scope.resource, &metadata)
            {
                return Err(AuthProductError::BackendConflict);
            }
        }
        DcrRetirementUpdate::Record { .. } => {}
    }
    Ok(())
}
