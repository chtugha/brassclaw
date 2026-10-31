//! Explicit reconciliation of the original's local encrypted material. Each
//! identity's intended closure is saved in the existing flow before mutation.
use super::*;
use brassclaw_auth::{
    AuthFlowRecord, DcrArtifactExpectation, DcrArtifactSlot, DcrRegistrationState,
    DcrRetirementOutcome, DcrRetirementProblem, DcrRetirementUpdate,
};

impl OAuthDcrProvider {
    pub(super) async fn finish_cleanup_state(
        &self,
        manager: &Arc<dyn AuthFlowManager>,
        mut state: DcrRegistrationState,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        if !state.action.cleanup.remote_confirmed() {
            return Ok(state.flow);
        }
        let original = state.action.original.clone();
        if state.action.retirement.is_none() {
            let mut selections = Vec::new();
            for slot in DcrArtifactSlot::ALL {
                selections.push(
                    crate::auth_dcr_retirement::select(
                        self.secret_store.as_ref(),
                        &state.action,
                        slot,
                    )
                    .await?,
                );
            }
            state = manager
                .update_dcr_retirement(&original, DcrRetirementUpdate::Plan(selections))
                .await?;
        }
        for slot in DcrArtifactSlot::ALL {
            let mut selected = state
                .action
                .retirement
                .as_ref()
                .and_then(|plan| {
                    plan.entries
                        .iter()
                        .find(|entry| entry.selection.slot == slot)
                })
                .ok_or(AuthProductError::BackendConflict)?
                .clone();
            if matches!(selected.outcome, DcrRetirementOutcome::Confirmed { .. }) {
                continue;
            }
            if matches!(
                selected.selection.expected,
                DcrArtifactExpectation::Absent { .. }
            ) {
                let current = crate::auth_dcr_retirement::select(
                    self.secret_store.as_ref(),
                    &state.action,
                    slot,
                )
                .await?;
                if current != selected.selection
                    && !matches!(current.expected, DcrArtifactExpectation::Absent { .. })
                {
                    state = manager
                        .update_dcr_retirement(
                            &original,
                            DcrRetirementUpdate::Refine {
                                before: selected.selection,
                                selected: current,
                            },
                        )
                        .await?;
                    selected = state
                        .action
                        .retirement
                        .as_ref()
                        .and_then(|plan| {
                            plan.entries
                                .iter()
                                .find(|entry| entry.selection.slot == slot)
                        })
                        .ok_or(AuthProductError::BackendConflict)?
                        .clone();
                    if matches!(selected.outcome, DcrRetirementOutcome::Confirmed { .. }) {
                        continue;
                    }
                }
            }
            let expected = crate::auth_dcr_retirement::closure(&state.action, &selected.selection);
            let written = self
                .secret_store
                .close_identity(
                    original.scope.resource.clone(),
                    selected.selection.handle.clone(),
                    expected.clone(),
                )
                .await;
            let mut unavailable = written.is_err();
            let actual = match written {
                Ok(Some(actual)) => Some(actual),
                Ok(None) | Err(_) => match self
                    .secret_store
                    .custody(&original.scope.resource, &selected.selection.handle)
                    .await
                {
                    Ok(actual) => actual,
                    Err(_) => {
                        unavailable = true;
                        None
                    }
                },
            };
            let outcome = match actual {
                Some(actual)
                    if actual.handle == selected.selection.handle
                        && expected.accepts(&original.scope.resource, &actual) =>
                {
                    DcrRetirementOutcome::Confirmed {
                        closed_reference: actual.reference,
                    }
                }
                _ => DcrRetirementOutcome::Unresolved {
                    problem: if unavailable {
                        DcrRetirementProblem::StorageUnavailable
                    } else {
                        DcrRetirementProblem::IdentityConflict
                    },
                },
            };
            state = manager
                .update_dcr_retirement(
                    &original,
                    DcrRetirementUpdate::Record {
                        selected: selected.selection,
                        outcome,
                    },
                )
                .await?;
        }
        Ok(state.flow)
    }
}
