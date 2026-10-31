//! Explicit cleanup of the retained DCR original. Uses existing AuthFlow claims
//! and host egress; no background queue, replacement registration or retry loop.
use super::private_material::{DcrManagementRecord, DcrPreparationRecord, DcrResponseRecord};
use super::*;
use brassclaw_auth::{
    AuthFlowRecord, AuthFlowStatus, DcrCleanupAdmission, DcrCleanupCompletion, DcrCleanupOutcome,
    DcrCleanupPrepared, DcrCleanupState, DcrCleanupUnavailable, DcrRegistrationOutcome,
    DcrRegistrationState,
};
use brassclaw_secrets::SecretCustodyState;

fn validate_management(
    record: &DcrManagementRecord,
    preparation: &DcrPreparationRecord,
) -> Result<(), AuthProductError> {
    // Never send this bearer to a URL merely supplied in a registration reply.
    // Bind it to the exact retained registration origin and require TLS, with
    // no embedded credentials or fragment. Network/kernel checks still follow.
    let uri = url::Url::parse(record.uri.expose_secret())
        .map_err(|_| AuthProductError::BackendUnavailable)?;
    if uri.scheme() != "https"
        || !uri.username().is_empty()
        || uri.password().is_some()
        || uri.fragment().is_some()
        || record.token.expose_secret().is_empty()
        || record.token.expose_secret().chars().any(char::is_control)
    {
        return Err(AuthProductError::BackendUnavailable);
    }
    validate_endpoint_origin(uri.as_str(), &preparation.registration_endpoint)
}

impl OAuthDcrProvider {
    pub(super) async fn cleanup_original_registration(
        &self,
        manager: &Arc<dyn AuthFlowManager>,
        selected: DcrRegistrationState,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let original = selected.action.original;
        let state = manager
            .read_dcr_registration(&original.scope, original.flow_id)
            .await?
            .ok_or(AuthProductError::BackendUnavailable)?;
        state.action.original.verify(&original)?;
        match self
            .cleanup_original_registration_inner(manager, state)
            .await
        {
            Ok(flow) => Ok(flow),
            Err(error) => {
                let actual = manager
                    .read_dcr_registration(&original.scope, original.flow_id)
                    .await?
                    .ok_or(AuthProductError::BackendUnavailable)?;
                actual.action.original.verify(&original)?;
                // A concurrent owner may have closed raw material after our
                // read. Its confirmed original receipt permits local readback,
                // never another DELETE or a false confirmation of Unknown.
                if actual.action.cleanup.remote_confirmed() {
                    self.finish_cleanup_state(manager, actual).await
                } else {
                    Err(error)
                }
            }
        }
    }

    async fn cleanup_original_registration_inner(
        &self,
        manager: &Arc<dyn AuthFlowManager>,
        mut state: DcrRegistrationState,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        // A completed account still owns its registration for future refresh.
        if state.flow.status == AuthFlowStatus::Completed
            || (!brassclaw_auth::is_terminal_status(state.flow.status)
                && state.flow.expires_at > Utc::now())
        {
            return Ok(state.flow);
        }
        state = self.reconcile_original_response(manager, state).await?;
        if matches!(state.action.cleanup, DcrCleanupState::Retained) {
            state = manager
                .mark_dcr_cleanup_unavailable(
                    &state.action.original,
                    DcrCleanupUnavailable::RegistrationUnconfirmed,
                )
                .await?;
        }
        if matches!(state.action.cleanup, DcrCleanupState::Submitted { .. }) {
            let state = self.reconcile_cleanup_response(manager, state).await?;
            return self.finish_cleanup_state(manager, state).await;
        }
        if !matches!(
            state.action.cleanup,
            DcrCleanupState::Required { .. } | DcrCleanupState::Unavailable { .. }
        ) {
            return self.finish_cleanup_state(manager, state).await;
        }
        let original = state.action.original.clone();
        let DcrRegistrationOutcome::Returned {
            status: 200..=299,
            response,
            ..
        } = &state.action.outcome
        else {
            let state = manager
                .mark_dcr_cleanup_unavailable(
                    &original,
                    DcrCleanupUnavailable::RegistrationUnconfirmed,
                )
                .await?;
            return self.finish_cleanup_state(manager, state).await;
        };
        let registration_response = response.clone();
        let raw = self
            .read_dcr_material(
                &original,
                registration_response.clone(),
                "registration-response-read",
            )
            .await?;
        let response_record = DcrResponseRecord::decode(&raw)?;
        let DcrRegistrationOutcome::Returned { status, .. } = state.action.outcome else {
            return Err(AuthProductError::BackendConflict);
        };
        if response_record.status != status {
            return Err(AuthProductError::BackendConflict);
        }
        let body = response_record.body()?;
        let management = match DcrManagementRecord::from_registration(body.expose_secret()) {
            Ok(Some(management)) => management,
            result => {
                let issue = if result.is_err() {
                    DcrCleanupUnavailable::RegistrationResponseUnusable
                } else {
                    DcrCleanupUnavailable::ManagementUnavailable
                };
                return manager
                    .mark_dcr_cleanup_unavailable(&original, issue)
                    .await
                    .map(|state| state.flow);
            }
        };
        let preparation = self
            .read_dcr_material(&original, original.preparation.clone(), "preparation-read")
            .await?;
        let preparation = DcrPreparationRecord::decode(&preparation)?;
        if validate_management(&management, &preparation).is_err() {
            return manager
                .mark_dcr_cleanup_unavailable(
                    &original,
                    DcrCleanupUnavailable::InvalidManagementEndpoint,
                )
                .await
                .map(|state| state.flow);
        }
        let management_ref = self
            .retain_dcr_original(
                &original.scope.resource,
                flow_secret_handle(&self.spec, original.flow_id, "cleanup-management")?,
                management.encode()?,
                state.action.cleanup_management_context()?,
            )
            .await?;
        // Capture before consuming the one durable cleanup dispatch. Original
        // submitted/terminal readback above never depends on current settings.
        let http_limits = self.capture_http_limits().await?;
        let dispatched = match manager
            .claim_dcr_cleanup(DcrCleanupPrepared {
                original: original.clone(),
                registration_response,
                management: management_ref,
            })
            .await?
        {
            DcrCleanupAdmission::Dispatch(state) => state,
            DcrCleanupAdmission::Original(state) => {
                let state = self.reconcile_cleanup_response(manager, state).await?;
                return self.finish_cleanup_state(manager, state).await;
            }
        };
        // Only a newly acknowledged durable claim authorizes this one DELETE.
        // Unknown outcomes and readback of our own claim never reach this branch.
        let claim = dispatched
            .action
            .cleanup
            .claim()
            .ok_or(AuthProductError::BackendConflict)?
            .clone();
        let response = self
            .execute_request(
                original.scope.resource.clone(),
                NetworkMethod::Delete,
                management.uri.expose_secret(),
                Vec::new(),
                http_limits,
                Some(&management.token),
            )
            .await;
        let outcome = match response {
            Ok(response) => {
                let status = response.status;
                let empty_body = response.body.is_empty();
                let retained = self
                    .retain_dcr_original(
                        &original.scope.resource,
                        flow_secret_handle(&self.spec, original.flow_id, "cleanup-response")?,
                        DcrResponseRecord::capture(response).encode()?,
                        dispatched
                            .action
                            .cleanup_response_context(status, empty_body)?,
                    )
                    .await?;
                DcrCleanupOutcome::Returned {
                    status,
                    empty_body,
                    response: retained,
                }
            }
            Err(error) => {
                manager
                    .record_dcr_cleanup(
                        &original.scope,
                        DcrCleanupCompletion {
                            dispatched: claim,
                            outcome: DcrCleanupOutcome::Unknown(error.code()),
                        },
                    )
                    .await?;
                return Err(error);
            }
        };
        let state = manager
            .record_dcr_cleanup(
                &original.scope,
                DcrCleanupCompletion {
                    dispatched: claim,
                    outcome,
                },
            )
            .await?;
        self.finish_cleanup_state(manager, state).await
    }

    async fn reconcile_cleanup_response(
        &self,
        manager: &Arc<dyn AuthFlowManager>,
        state: DcrRegistrationState,
    ) -> Result<DcrRegistrationState, AuthProductError> {
        let DcrCleanupState::Submitted { claim } = &state.action.cleanup else {
            return Ok(state);
        };
        let original = &state.action.original;
        let handle = flow_secret_handle(&self.spec, original.flow_id, "cleanup-response")?;
        let Some(metadata) = self
            .secret_store
            .custody(&original.scope.resource, &handle)
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
        else {
            return Ok(state);
        };
        let SecretCustodyState::Original { context } = &metadata.state else {
            return Err(AuthProductError::BackendConflict);
        };
        let status = context["status"]
            .as_u64()
            .and_then(|n| u16::try_from(n).ok())
            .ok_or(AuthProductError::BackendConflict)?;
        let empty_body = context["empty_body"]
            .as_bool()
            .ok_or(AuthProductError::BackendConflict)?;
        if metadata.scope != original.scope.resource
            || metadata.handle != handle
            || *context != state.action.cleanup_response_context(status, empty_body)?
        {
            return Err(AuthProductError::BackendConflict);
        }
        // The owner authenticates the captured complete return and exact claim.
        // It does not infer success from missing material or elapsed time.
        manager
            .record_dcr_cleanup(
                &original.scope,
                DcrCleanupCompletion {
                    dispatched: claim.clone(),
                    outcome: DcrCleanupOutcome::Returned {
                        status,
                        empty_body,
                        response: brassclaw_auth::DcrMaterialReference {
                            handle: metadata.handle,
                            reference: metadata.reference,
                        },
                    },
                },
            )
            .await
    }
}
