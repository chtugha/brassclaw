//! CAS-only requirement publication. A staged selection is never a completed
//! flow or runtime grant; the ordinary reader requires actual completed custody.
use super::*;
use crate::auth_flow_custody::StoredAuthFlow;
use brassclaw_auth::CredentialRequirementSelection;

impl<F: RootFilesystem + 'static> FilesystemAuthProductServices<F> {
    async fn stage_manual_requirement(
        &self,
        original: &ManualOriginal,
        selected: &CredentialRequirementSelection,
    ) -> Result<(StoredAuthFlow, AuthRecordSnapshot), AuthProductError> {
        let pending = original
            .pending_document()
            .ok_or(AuthProductError::BackendConflict)?;
        let scope = &pending.flow.scope;
        let (current, snapshot) = self
            .read_flow_document(scope, pending.flow.id)
            .await?
            .ok_or(AuthProductError::BackendConflict)?;
        if current.flow.status == AuthFlowStatus::Completed {
            original.verify_completed_document(&current)?;
            return Ok((current, snapshot));
        }
        let mut staged = pending.clone();
        staged.auth_custody.requirement_selection = Some(Box::new(selected.clone()));
        // The first stage is the decision point. Recover its exact proof,
        // including historical absence; never capture current material again.
        staged.auth_custody.requirement_source =
            if let Some(original_selection) = &current.auth_custody.requirement_selection {
                if **original_selection != *selected {
                    return Err(AuthProductError::BackendConflict);
                }
                if let Some(source) = &current.auth_custody.requirement_source {
                    source.verify_selection(selected)?;
                }
                current.auth_custody.requirement_source.clone()
            } else {
                if current != pending
                    || Some(snapshot) != original.flow_entry.map(SelectedEntry::snapshot)
                {
                    return Err(AuthProductError::BackendConflict);
                }
                self.capture_requirement_source(selected).await?
            };
        if current == staged {
            return Ok((current, snapshot));
        }
        if current.flow.status == AuthFlowStatus::Failed
            && current.flow.error == Some(brassclaw_auth::AuthErrorCode::AccountSelectionRequired)
            && current.auth_custody == staged.auth_custody
        {
            return Err(AuthProductError::AccountSelectionRequired);
        }
        if current != pending || Some(snapshot) != original.flow_entry.map(SelectedEntry::snapshot)
        {
            return Err(AuthProductError::BackendConflict);
        }
        match self.write_flow_document(&staged, snapshot.cas()).await {
            Ok(snapshot) => Ok((staged, snapshot)),
            Err(error) => {
                let (after, snapshot) = self
                    .read_flow_document(scope, pending.flow.id)
                    .await?
                    .ok_or(error.clone())?;
                if after.flow.status == AuthFlowStatus::Completed {
                    original.verify_completed_document(&after)?;
                } else if after != staged {
                    return Err(error);
                }
                Ok((after, snapshot))
            }
        }
    }

    pub(super) async fn finish_manual_requirement(
        &self,
        original: &ManualOriginal,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let selected = original
            .requirement_selection()?
            .ok_or(AuthProductError::BackendConflict)?;
        let (mut staged, snapshot) = self.stage_manual_requirement(original, &selected).await?;
        if staged.flow.status == AuthFlowStatus::Completed {
            return Ok(staged.flow);
        }
        if let Err(error) = self
            .publish_requirement_head(
                &staged,
                &original
                    .completed_flow()?
                    .ok_or(AuthProductError::BackendConflict)?,
            )
            .await
        {
            if error != AuthProductError::BackendConflict {
                return Err(error);
            }
            // The account/material publication already happened. Preserve it
            // and expose the losing selection as a failed original requiring
            // deliberate account choice, never as a rolled-back or successful
            // flow. A concurrent successful completion wins its own exact CAS.
            staged.flow.status = AuthFlowStatus::Failed;
            staged.flow.error = Some(brassclaw_auth::AuthErrorCode::AccountSelectionRequired);
            staged.flow.updated_at = Utc::now();
            if let Err(write_error) = self.write_flow_document(&staged, snapshot.cas()).await {
                let (after, _) = self
                    .read_flow_document(&staged.flow.scope, staged.flow.id)
                    .await?
                    .ok_or(write_error.clone())?;
                if after.flow.status == AuthFlowStatus::Completed {
                    original.verify_completed_document(&after)?;
                    return Ok(after.flow);
                }
                if after != staged {
                    return Err(write_error);
                }
            }
            return Err(AuthProductError::AccountSelectionRequired);
        }
        staged.flow = original
            .completed_flow()?
            .ok_or(AuthProductError::BackendConflict)?;
        if let Err(error) = self.write_flow_document(&staged, snapshot.cas()).await {
            let (after, _) = self
                .read_flow_document(&staged.flow.scope, staged.flow.id)
                .await?
                .ok_or(error.clone())?;
            original
                .verify_completed_document(&after)
                .map_err(|_| error)?;
            return Ok(after.flow);
        }
        Ok(staged.flow)
    }
}
