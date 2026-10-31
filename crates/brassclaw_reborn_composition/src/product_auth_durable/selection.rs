//! Explicit account choice on the existing CAS-only flow/head owner.
//! The selected account snapshot survives waits, lost replies and later edits.
use super::*;
use crate::auth_flow_custody::StoredAuthFlow;
use brassclaw_auth::{AuthFlowStatus, CredentialRequirementSelection, CredentialSelectionInput};

fn completed_choice(
    document: &StoredAuthFlow,
    input: &CredentialSelectionInput,
) -> Result<(), AuthProductError> {
    let mut flow = document.flow.clone();
    super::domain::validate_selection_flow(&mut flow, &document.flow.scope, input, Utc::now())?;
    if flow.status != AuthFlowStatus::Completed {
        return Err(AuthProductError::BackendConflict);
    }
    if let Some(selected) = &document.auth_custody.requirement_selection {
        selected.verify_receipt(
            &flow,
            document
                .auth_custody
                .requirement_setup
                .as_deref()
                .ok_or(AuthProductError::BackendConflict)?,
        )?;
        if let Some(source) = &document.auth_custody.requirement_source {
            source.verify_selection(selected)?;
        }
    } else if document.auth_custody.requirement_source.is_some() {
        return Err(AuthProductError::BackendConflict);
    }
    Ok(())
}

impl<F: RootFilesystem> FilesystemAuthProductServices<F> {
    pub(super) async fn complete_selection_owned(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        input: CredentialSelectionInput,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let lock = self.lock_for(format!("flow:{}", input.flow_id));
        let _guard = lock.lock().await;
        let (mut document, mut snapshot) = self
            .read_flow_document(scope, input.flow_id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        if document.flow.status == AuthFlowStatus::Completed {
            completed_choice(&document, &input)?;
            // Legacy originals stay legacy. No latest account lookup or grant
            // backfill when the original never recorded a requirement receipt.
            return Ok(document.flow);
        }
        if document.auth_custody.oauth_callback.is_some()
            || document.fs_callback_publication.is_some()
        {
            return Err(AuthProductError::BackendConflict);
        }
        let mut completed = document.flow.clone();
        if let Some(selected) = &document.auth_custody.requirement_selection {
            // The stage CAS was the decision point; expiry or current account
            // edits after that point cannot select a different original result.
            completed.status = AuthFlowStatus::Completed;
            completed.error = None;
            let mut proposed = document.clone();
            proposed.flow = completed.clone();
            completed_choice(&proposed, &input)?;
            let setup = document
                .auth_custody
                .requirement_setup
                .as_deref()
                .ok_or(AuthProductError::BackendConflict)?;
            if CredentialRequirementSelection::from_completed_flow(
                scope,
                setup,
                &completed,
                selected.account().clone(),
            )? != **selected
            {
                return Err(AuthProductError::BackendConflict);
            }
            if document.flow.status == AuthFlowStatus::Failed
                && document.flow.error
                    == Some(brassclaw_auth::AuthErrorCode::AccountSelectionRequired)
            {
                return Err(AuthProductError::AccountSelectionRequired);
            }
            if document.flow.status != AuthFlowStatus::AwaitingUser {
                return Err(AuthProductError::BackendConflict);
            }
        } else {
            if document.auth_custody.requirement_source.is_some() {
                return Err(AuthProductError::BackendConflict);
            }
            let now = Utc::now();
            match super::domain::validate_selection_flow(&mut document.flow, scope, &input, now) {
                Ok(()) => {}
                Err(AuthProductError::UnknownOrExpiredFlow) => {
                    self.write_flow_document(&document, snapshot.cas()).await?;
                    return Err(AuthProductError::UnknownOrExpiredFlow);
                }
                Err(error) => return Err(error),
            }
            let account = self
                .read_account(scope, input.credential_account_id)
                .await?
                .ok_or(AuthProductError::CredentialMissing)?
                .0;
            if account.provider != document.flow.provider
                || account.effective_status_at(now) != CredentialAccountStatus::Configured
            {
                return Err(AuthProductError::CrossScopeDenied);
            }
            completed = document.flow.clone();
            completed.status = AuthFlowStatus::Completed;
            completed.error = None;
            completed.credential_account_id = Some(account.id);
            completed.updated_at = now;
            if let Some(setup) = &document.auth_custody.requirement_setup {
                let selected = CredentialRequirementSelection::from_completed_flow(
                    scope, setup, &completed, account,
                )?;
                document.auth_custody.requirement_source =
                    self.capture_requirement_source(&selected).await?;
                document.flow.credential_account_id = Some(input.credential_account_id);
                document.flow.updated_at = now;
                document.auth_custody.requirement_selection = Some(Box::new(selected));
                match self.write_flow_document(&document, snapshot.cas()).await {
                    Ok(version) => snapshot = version,
                    Err(error) => {
                        let (after, version) = self
                            .read_flow_document(scope, input.flow_id)
                            .await?
                            .ok_or(error.clone())?;
                        if after.auth_custody == document.auth_custody
                            && after.flow.status == AuthFlowStatus::Completed
                        {
                            completed_choice(&after, &input)?;
                            return Ok(after.flow);
                        }
                        if after != document {
                            return Err(error);
                        }
                        snapshot = version;
                    }
                }
            }
        }
        if document.auth_custody.requirement_selection.is_some()
            && let Err(error) = self.publish_requirement_head(&document, &completed).await
        {
            if error != AuthProductError::BackendConflict {
                return Err(error);
            }
            // No account mutation belongs to choice; a lost head race requires
            // a new deliberate choice, never overwriting the winning selection.
            document.flow.status = AuthFlowStatus::Failed;
            document.flow.error = Some(brassclaw_auth::AuthErrorCode::AccountSelectionRequired);
            // Keep original selection time for verification on later retries.
            if let Err(write_error) = self.write_flow_document(&document, snapshot.cas()).await {
                let (after, _) = self
                    .read_flow_document(scope, input.flow_id)
                    .await?
                    .ok_or(write_error.clone())?;
                if after.auth_custody == document.auth_custody
                    && after.flow.status == AuthFlowStatus::Completed
                {
                    completed_choice(&after, &input)?;
                    return Ok(after.flow);
                }
                if after != document {
                    return Err(write_error);
                }
            }
            return Err(AuthProductError::AccountSelectionRequired);
        }
        document.flow = completed;
        if let Err(error) = self.write_flow_document(&document, snapshot.cas()).await {
            let (after, _) = self
                .read_flow_document(scope, input.flow_id)
                .await?
                .ok_or(error.clone())?;
            if after.auth_custody != document.auth_custody {
                return Err(error);
            }
            completed_choice(&after, &input).map_err(|_| error)?;
            return Ok(after.flow);
        }
        Ok(document.flow)
    }
}
