//! Explicit account choice joins the existing flow/account/head transaction.
use super::callback::{locked_account, locked_flow, save_flow};
use super::*;

impl PgAuthProductServices {
    pub(super) async fn complete_selection_owned(
        &self,
        scope: &AuthProductScope,
        input: brassclaw_auth::CredentialSelectionInput,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let mut client = self.pool.get().await.map_err(pool_err)?;
        let transaction = client.transaction().await.map_err(db_err)?;
        let (mut document, revision) = locked_flow(&transaction, scope, input.flow_id).await?;
        match validate_selection_flow(&mut document.flow, scope, &input, Utc::now()) {
            Ok(()) => {}
            Err(AuthProductError::UnknownOrExpiredFlow) => {
                save_flow(&transaction, &document, revision).await?;
                transaction.commit().await.map_err(db_err)?;
                return Err(AuthProductError::UnknownOrExpiredFlow);
            }
            Err(error) => return Err(error),
        }
        if document.flow.status == AuthFlowStatus::Completed {
            // Recover the original even if its account was later revoked or
            // another head chosen. Never reconstruct it from today's account.
            if let Some(selected) = &document.auth_custody.requirement_selection {
                selected.verify_receipt(
                    &document.flow,
                    document
                        .auth_custody
                        .requirement_setup
                        .as_deref()
                        .ok_or(AuthProductError::BackendConflict)?,
                )?;
            }
            transaction.rollback().await.map_err(db_err)?;
            return Ok(document.flow);
        }
        if document.auth_custody.oauth_callback.is_some()
            || document.fs_callback_publication.is_some()
        {
            return Err(AuthProductError::BackendConflict);
        }
        let (account, _) = locked_account(&transaction, scope, input.credential_account_id)
            .await?
            .ok_or(AuthProductError::CredentialMissing)?;
        let now = Utc::now();
        if account.provider != document.flow.provider
            || account.effective_status_at(now) != CredentialAccountStatus::Configured
        {
            return Err(AuthProductError::CrossScopeDenied);
        }
        document.flow.status = AuthFlowStatus::Completed;
        document.flow.error = None;
        document.flow.credential_account_id = Some(account.id);
        document.flow.updated_at = now;
        self.publish_requirement_in_transaction(&transaction, &mut document, &account)
            .await?;
        save_flow(&transaction, &document, revision).await?;
        transaction.commit().await.map_err(db_err)?;
        Ok(document.flow)
    }
}
