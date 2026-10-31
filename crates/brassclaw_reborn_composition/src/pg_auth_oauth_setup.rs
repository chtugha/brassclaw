//! Original OAuth setup within the existing PostgreSQL flow/CAS owner.
use super::*;
use crate::auth_flow_custody::StoredAuthFlow;
use brassclaw_auth::{OAuthSetupOriginal, OAuthSetupRetirement, OAuthSetupState};

impl PgAuthProductServices {
    pub(super) async fn read_oauth_setup_owned(
        &self,
        scope: &AuthProductScope,
        id: AuthFlowId,
    ) -> Result<Option<OAuthSetupState>, AuthProductError> {
        let Some((document, _)) = self.read_flow_document(scope, id).await? else {
            return Ok(None);
        };
        document.auth_custody.oauth_setup_state(&document.flow)
    }

    pub(super) async fn prepare_oauth_setup_owned(
        &self,
        original: OAuthSetupOriginal,
        retirement: Option<OAuthSetupRetirement>,
    ) -> Result<OAuthSetupState, AuthProductError> {
        original.validate()?;
        let definition = &original.definition;
        if let Some(existing) = self
            .read_oauth_setup_owned(&definition.scope, definition.flow_id)
            .await?
        {
            existing.original.verify(&original)?;
            return Ok(existing);
        }
        crate::auth_oauth_setup_custody::unsubmitted_preparation(
            self.secret_store.as_ref(),
            &original,
        )
        .await?;
        let (flow, auth_custody) = brassclaw_auth::AuthFlowCustody::prepared_oauth_setup(
            original.clone(),
            retirement,
            Utc::now(),
        )?;
        let document = StoredAuthFlow {
            flow,
            auth_custody,
            fs_callback_publication: None,
        };
        if let Err(error) = self.write_flow_document(&document, None).await {
            return match self
                .read_oauth_setup_owned(&definition.scope, definition.flow_id)
                .await?
            {
                Some(actual) => {
                    actual.original.verify(&original)?;
                    Ok(actual)
                }
                None => Err(error),
            };
        }
        document
            .auth_custody
            .oauth_setup_state(&document.flow)?
            .ok_or(AuthProductError::BackendConflict)
    }
}
