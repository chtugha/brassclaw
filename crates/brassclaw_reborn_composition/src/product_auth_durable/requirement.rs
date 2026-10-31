//! One CAS-only requirement-head owner for manual, OAuth and explicit choice.
//! Staging is not a grant: readers require the original flow's completed custody.
use super::*;
use crate::auth_flow_custody::StoredAuthFlow;
use brassclaw_auth::{CredentialRequirementPublication, CredentialRequirementSelection};

impl<F: RootFilesystem> FilesystemAuthProductServices<F> {
    pub(super) async fn capture_requirement_source(
        &self,
        selected: &CredentialRequirementSelection,
    ) -> Result<Option<Box<brassclaw_auth::CredentialRequirementSource>>, AuthProductError> {
        let account = selected.account();
        let Some(handle) = &account.access_secret else {
            return Ok(None);
        };
        self.secret_store
            .record_reference(&account.scope.resource, handle)
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
            .map(|reference| {
                brassclaw_auth::CredentialRequirementSource::from_selection(
                    selected,
                    brassclaw_auth::CredentialSecretGenerationRef::new(reference)
                        .map_err(|_| AuthProductError::BackendUnavailable)?,
                )
                .map(Box::new)
            })
            .transpose()
    }

    async fn verify_requirement_head(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        selected: &CredentialRequirementSelection,
        requirement: &brassclaw_host_api::RuntimeCredentialAccountRequirement,
    ) -> Result<(), AuthProductError> {
        selected.verify_requested(scope, requirement)?;
        let (origin, _) = self
            .read_flow_document(selected.original_scope(), selected.original_flow())
            .await?
            .ok_or(AuthProductError::BackendConflict)?;
        let setup = origin
            .auth_custody
            .requirement_setup
            .as_deref()
            .ok_or(AuthProductError::BackendConflict)?;
        if origin.auth_custody.requirement_selection.as_deref() != Some(selected) {
            return Err(AuthProductError::BackendConflict);
        }
        selected.verify_receipt(&origin.flow, setup)?;
        if let Some(source) = &origin.auth_custody.requirement_source {
            source.verify_selection(selected)?;
        }
        Ok(())
    }

    pub(super) async fn publish_requirement_head(
        &self,
        staged: &StoredAuthFlow,
        completed: &AuthFlowRecord,
    ) -> Result<(), AuthProductError> {
        let scope = &staged.flow.scope;
        let setup = staged
            .auth_custody
            .requirement_setup
            .as_deref()
            .ok_or(AuthProductError::BackendConflict)?;
        let selected = staged
            .auth_custody
            .requirement_selection
            .as_deref()
            .ok_or(AuthProductError::BackendConflict)?;
        // Only actual exact staged custody can publish/recover its head. A
        // selection row or a caller's proposed completed flow is not proof.
        selected.verify_receipt(completed, setup)?;
        if let Some(source) = &staged.auth_custody.requirement_source {
            source.verify_selection(selected)?;
        }
        if completed.id != staged.flow.id
            || completed.scope != staged.flow.scope
            || is_terminal_status(staged.flow.status)
        {
            return Err(AuthProductError::BackendConflict);
        }
        let (actual, _) = self
            .read_flow_document(scope, staged.flow.id)
            .await?
            .ok_or(AuthProductError::BackendConflict)?;
        if actual != *staged {
            // Another owner may have completed this same original. Recover it
            // without touching today's head, which could already be newer.
            if actual.flow.status == brassclaw_auth::AuthFlowStatus::Completed
                && actual.auth_custody == staged.auth_custody
                && actual.fs_callback_publication.is_none()
            {
                selected.verify_receipt(&actual.flow, setup)?;
                return Ok(());
            }
            return Err(AuthProductError::BackendConflict);
        }
        let path = super::paths::requirement_selection_path(scope, &selected.key()?);
        let current: Option<(CredentialRequirementSelection, AuthRecordSnapshot)> =
            self.read_record(&scope.resource, &path).await?;
        // Exact original custody was verified before this idempotent readback.
        if current.as_ref().is_some_and(|(head, _)| head == selected) {
            return Ok(());
        }
        if let Some((head, _)) = &current {
            self.verify_requirement_head(scope, head, setup.requirement())
                .await?;
        }
        if setup.publication(current.as_ref().map(|(head, _)| head), selected)?
            != CredentialRequirementPublication::Publish
        {
            return Err(AuthProductError::BackendConflict);
        }
        let cas = current
            .as_ref()
            .map_or(CasExpectation::Absent, |(_, snapshot)| snapshot.cas());
        if let Err(error) = self
            .write_record(&scope.resource, &path, selected, cas)
            .await
        {
            let after: Option<(CredentialRequirementSelection, AuthRecordSnapshot)> =
                self.read_record(&scope.resource, &path).await?;
            if after.as_ref().is_some_and(|(head, _)| head == selected) {
                return Ok(());
            }
            // Unknown backend failure with an unchanged head remains retryable
            // at this original. A CAS loser never writes a replacement head.
            return Err(error);
        }
        Ok(())
    }
}
