//! In-memory transition model. Actual encrypted provenance belongs to native
//! storage tests, never to this fake or a copied reference.
use super::*;

fn selected(
    state: &AuthState,
    scope: &AuthProductScope,
    id: AuthFlowId,
) -> Result<Option<crate::OAuthSetupState>, AuthProductError> {
    let Some(flow) = state.flows.get(&id) else {
        return Ok(None);
    };
    if flow.scope != *scope {
        return Err(AuthProductError::CrossScopeDenied);
    }
    match state.flow_custody.get(&id) {
        Some(custody) => custody.oauth_setup_state(flow),
        None => Ok(None),
    }
}

impl InMemoryAuthProductServices {
    pub(super) async fn read_oauth_setup_owned(
        &self,
        scope: &AuthProductScope,
        id: AuthFlowId,
    ) -> Result<Option<crate::OAuthSetupState>, AuthProductError> {
        selected(&self.lock_state(), scope, id)
    }

    pub(super) async fn prepare_oauth_setup_owned(
        &self,
        original: crate::OAuthSetupOriginal,
        retirement: Option<crate::OAuthSetupRetirement>,
    ) -> Result<crate::OAuthSetupState, AuthProductError> {
        original.validate()?;
        let definition = &original.definition;
        let mut state = self.lock_state();
        if let Some(existing) = selected(&state, &definition.scope, definition.flow_id)? {
            existing.original.verify(&original)?;
            return Ok(existing);
        }
        if state.flows.contains_key(&definition.flow_id) {
            return Err(AuthProductError::BackendConflict);
        }
        let (flow, custody) =
            crate::AuthFlowCustody::prepared_oauth_setup(original, retirement, Utc::now())?;
        let result = custody
            .oauth_setup_state(&flow)?
            .ok_or(AuthProductError::BackendConflict)?;
        state.flow_custody.insert(flow.id, custody);
        state.flows.insert(flow.id, flow);
        Ok(result)
    }
}
