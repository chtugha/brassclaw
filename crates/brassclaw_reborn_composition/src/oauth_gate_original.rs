//! First Google client/PKCE capture inside the existing encrypted owner.
//! This is neither callback admission nor authority to call the issuer.
use super::*;
use brassclaw_auth::{
    CredentialAccount, CredentialAccountUpdateBinding, CredentialRequirementSetup,
    OAuthSetupDefinition, OAuthSetupMaterialReference, OAuthSetupOriginal, OAuthSetupRetirement,
    OAuthSetupState,
};
use brassclaw_secrets::{
    SecretCustodyMetadata, SecretCustodySource, SecretCustodyState, SecretIdentityClosure,
};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};

use crate::oauth_dcr_protocol::StoredDcrClientMaterial as StoredClientMaterial;

/// Host-captured setup inputs, never raw browser/provider material. A repeated
/// start compares these with the retained first definition before new work.
#[derive(Clone, Copy)]
pub(crate) enum GoogleSetupDeadline {
    Exact(brassclaw_auth::Timestamp),
    GateDefault,
}

#[derive(Clone)]
pub(crate) struct GoogleSetupRequest {
    pub(crate) scope: AuthProductScope,
    pub(crate) flow_id: AuthFlowId,
    pub(crate) account_label: CredentialAccountLabel,
    pub(crate) requested_scopes: Vec<ProviderScope>,
    pub(crate) continuation: AuthContinuationRef,
    pub(crate) requirement_setup: Option<Box<CredentialRequirementSetup>>,
    pub(crate) update_binding: Option<CredentialAccountUpdateBinding>,
    pub(crate) update_account_snapshot: Option<CredentialAccount>,
    pub(crate) deadline: GoogleSetupDeadline,
}

impl GoogleSetupRequest {
    fn verify(&self, original: &OAuthSetupOriginal) -> Result<(), AuthProductError> {
        original.validate()?;
        let definition = &original.definition;
        if self.scope != definition.scope || self.flow_id != definition.flow_id {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if definition.provider.as_str() != brassclaw_auth::GOOGLE_PROVIDER_ID
            || self.account_label != definition.account_label
            || self.requested_scopes != definition.requested_scopes
            || self.continuation != definition.continuation
            || self.requirement_setup != definition.requirement_setup
            || self.update_binding != definition.update_binding
            || self.update_account_snapshot != definition.update_account_snapshot
            || matches!(self.deadline, GoogleSetupDeadline::Exact(expiry) if expiry != definition.expires_at)
        {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(())
    }
}

// Serialized only directly into encrypted SecretMaterial. Never Debug, a public
// DTO or generic request JSON. Client and PKCE enter the SAME immutable write.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GooglePreparation {
    format: String,
    #[serde(with = "secret_text")]
    pub(super) pkce: SecretString,
    pub(super) client: StoredClientMaterial,
    hosted_domain_hint: Option<String>,
}

impl GooglePreparation {
    fn encode(&self) -> Result<SecretString, AuthProductError> {
        serde_json::to_string(self)
            .map(SecretString::from)
            .map_err(|_| AuthProductError::BackendUnavailable)
    }

    fn decode(
        raw: &SecretString,
        definition: &OAuthSetupDefinition,
    ) -> Result<Self, AuthProductError> {
        let value: Self = serde_json::from_str(raw.expose_secret())
            .map_err(|_| AuthProductError::BackendConflict)?;
        if value.format != "google-private-preparation/1"
            || value.client.token_endpoint != brassclaw_auth::GOOGLE_TOKEN_ENDPOINT
        {
            return Err(AuthProductError::BackendConflict);
        }
        let verifier = PkceVerifierSecret::new(value.pkce.clone())?;
        if !pkce_verifier_hash(&verifier)?.constant_time_eq(&definition.pkce_verifier_hash) {
            return Err(AuthProductError::BackendConflict);
        }
        // Rebuild exactly from the retained first state/client, never current
        // provider configuration. This checks the complete challenge partition.
        let url = url::Url::parse(definition.authorization_url.as_str())
            .map_err(|_| AuthProductError::BackendConflict)?;
        let states = url
            .query_pairs()
            .filter(|(name, _)| name == "state")
            .map(|(_, value)| value.into_owned())
            .collect::<Vec<_>>();
        let [state] = states.as_slice() else {
            return Err(AuthProductError::BackendConflict);
        };
        let state_metadata = GoogleOAuthCallbackState::decode(state)?;
        if state_metadata.flow_id() != definition.flow_id
            || state_metadata.scope().resource != definition.scope.resource
            || state_metadata.scope().session_id != definition.scope.session_id
            || state_metadata.account_label() != &definition.account_label
            || state_metadata.requested_scopes() != definition.requested_scopes
            || !opaque_state_hash(state)?.constant_time_eq(&definition.opaque_state_hash)
            || build_google_authorization_url(
                &value.client.client_id,
                &value.client.redirect_uri,
                state,
                &pkce_s256_challenge(&verifier),
                &definition.requested_scopes,
                value.hosted_domain_hint.as_deref(),
            )? != definition.authorization_url
        {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(value)
    }
}

impl GoogleOAuthGateProvider {
    pub(crate) async fn retained_setup_original(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
    ) -> Result<Option<OAuthSetupOriginal>, AuthProductError> {
        self.retained_setup_original_for_resource(&scope.resource, flow_id)
            .await?
            .map(|original| {
                if original.definition.scope != *scope {
                    return Err(AuthProductError::CrossScopeDenied);
                }
                Ok(original)
            })
            .transpose()
    }

    pub(super) async fn retained_setup_original_for_resource(
        &self,
        scope: &ResourceScope,
        flow_id: AuthFlowId,
    ) -> Result<Option<OAuthSetupOriginal>, AuthProductError> {
        let handle = preparation_handle(flow_id)?;
        let Some(metadata) = self
            .secret_store
            .custody(scope, &handle)
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
        else {
            return Ok(None);
        };
        if metadata.scope != *scope || metadata.handle != handle {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let (reference, context) = match &metadata.state {
            SecretCustodyState::Original { context } => (&metadata.reference, context),
            SecretCustodyState::RetiredOriginal {
                original_reference,
                context,
            } => (original_reference, context),
            _ => return Err(AuthProductError::BackendConflict),
        };
        let definition: OAuthSetupDefinition =
            serde_json::from_value(context["definition"].clone())
                .map_err(|_| AuthProductError::BackendConflict)?;
        if definition.preparation_context() != *context
            || definition.scope.resource != *scope
            || definition.flow_id != flow_id
            || definition.provider.as_str() != brassclaw_auth::GOOGLE_PROVIDER_ID
        {
            return Err(AuthProductError::BackendConflict);
        }
        let original = OAuthSetupOriginal {
            definition,
            preparation: OAuthSetupMaterialReference {
                handle,
                reference: reference.clone(),
            },
        };
        original.validate()?;
        Ok(Some(original))
    }

    pub(crate) async fn prepare_setup_original(
        &self,
        request: GoogleSetupRequest,
    ) -> Result<OAuthSetupOriginal, AuthProductError> {
        if matches!(request.deadline, GoogleSetupDeadline::GateDefault)
            && !matches!(
                request.continuation,
                AuthContinuationRef::TurnGateResume { .. }
            )
        {
            return Err(AuthProductError::BackendConflict);
        }
        if let Some(original) = self
            .retained_setup_original(&request.scope, request.flow_id)
            .await?
        {
            request.verify(&original)?;
            return Ok(original);
        }
        let now = Utc::now();
        let expires_at = match request.deadline {
            GoogleSetupDeadline::Exact(expiry) => expiry,
            GoogleSetupDeadline::GateDefault => {
                now + ChronoDuration::seconds(GATE_FLOW_TTL_SECONDS)
            }
        };
        if expires_at <= now {
            return Err(AuthProductError::UnknownOrExpiredFlow);
        }
        let state = GoogleOAuthCallbackState::new(
            request.flow_id,
            request.scope.clone(),
            request.account_label.clone(),
            request.requested_scopes.clone(),
        )?
        .encode()?;
        let pkce = SecretString::from(brassclaw_common::pkce::generate_code_verifier());
        let verifier = PkceVerifierSecret::new(pkce.clone())?;
        let definition = OAuthSetupDefinition {
            scope: request.scope.clone(),
            flow_id: request.flow_id,
            provider: AuthProviderId::new(self.provider_id())?,
            account_label: request.account_label.clone(),
            requested_scopes: request.requested_scopes.clone(),
            continuation: request.continuation.clone(),
            requirement_setup: request.requirement_setup.clone(),
            update_binding: request.update_binding.clone(),
            update_account_snapshot: request.update_account_snapshot.clone(),
            authorization_url: build_google_authorization_url(
                self.client.client_id.as_str(),
                self.client.redirect_uri.as_str(),
                state.as_str(),
                &pkce_s256_challenge(&verifier),
                &request.requested_scopes,
                self.client.hosted_domain_hint.as_deref(),
            )?,
            opaque_state_hash: opaque_state_hash(state.as_str())?,
            pkce_verifier_hash: pkce_verifier_hash(&verifier)?,
            created_at: now,
            expires_at,
        };
        definition.validate()?;
        let payload = GooglePreparation {
            format: "google-private-preparation/1".into(),
            pkce,
            client: StoredClientMaterial {
                client_id: self.client.client_id.as_str().into(),
                client_secret: self.client.client_secret.clone(),
                redirect_uri: self.client.redirect_uri.as_str().into(),
                token_endpoint: brassclaw_auth::GOOGLE_TOKEN_ENDPOINT.into(),
            },
            hosted_domain_hint: self.client.hosted_domain_hint.clone(),
        };
        let handle = preparation_handle(request.flow_id)?;
        let result = self
            .secret_store
            .put_original(
                request.scope.resource.clone(),
                handle.clone(),
                payload.encode()?,
                definition.preparation_context(),
            )
            .await;
        // A concurrent first writer can own different random state/PKCE. Read
        // that authenticated original, compare the requested semantic inputs and
        // use its exact bytes. A lost write reply never causes cleanup/rewrite.
        let original = self
            .retained_setup_original(&request.scope, request.flow_id)
            .await?
            .ok_or(AuthProductError::BackendUnavailable)?;
        request.verify(&original)?;
        if let Ok(Some(metadata)) = result
            && (metadata.scope != request.scope.resource
                || metadata.handle != handle
                || metadata.reference != original.preparation.reference
                || metadata.state
                    != (SecretCustodyState::Original {
                        context: original.definition.preparation_context(),
                    }))
        {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(original)
    }

    pub(crate) async fn restore_setup_original(
        &self,
        flow_manager: &Arc<dyn AuthFlowManager>,
        original: OAuthSetupOriginal,
        retirement: Option<OAuthSetupRetirement>,
    ) -> Result<OAuthSetupState, AuthProductError> {
        let authenticated = self
            .retained_setup_original(&original.definition.scope, original.definition.flow_id)
            .await?
            .ok_or(AuthProductError::BackendConflict)?;
        authenticated.verify(&original)?;
        flow_manager.prepare_oauth_setup(original, retirement).await
    }

    /// A retained alias is the durable later-use marker. Even after retirement,
    /// its occupied identity prevents rebuilding a missing first flow. This
    /// method grants no issuer dispatch; AuthFlowManager owns that claim.
    pub(super) async fn selected_preparation(
        &self,
        original: &OAuthSetupOriginal,
    ) -> Result<SecretCustodyMetadata, AuthProductError> {
        let scope = &original.definition.scope.resource;
        let target = original.definition.exchange_use_handle()?;
        let source = SecretCustodySource {
            scope: scope.clone(),
            handle: original.preparation.handle.clone(),
            reference: original.preparation.reference.clone(),
        };
        let written = self
            .secret_store
            .put_retained_from_original(scope.clone(), target.clone(), source.clone())
            .await;
        let metadata = match written {
            Ok(Some(metadata)) => metadata,
            Ok(None) | Err(_) => self
                .secret_store
                .custody(scope, &target)
                .await
                .map_err(|_| AuthProductError::BackendUnavailable)?
                .ok_or(AuthProductError::BackendUnavailable)?,
        };
        if metadata.scope != *scope
            || metadata.handle != target
            || metadata.state != (SecretCustodyState::Retained { source })
        {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(metadata)
    }

    pub(super) async fn read_selected(
        &self,
        selected: &SecretCustodyMetadata,
    ) -> Result<SecretString, AuthProductError> {
        let lease = self
            .secret_store
            .lease_once_if_record_reference(&selected.scope, &selected.handle, &selected.reference)
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
            .ok_or(AuthProductError::BackendConflict)?;
        self.secret_store
            .consume(&selected.scope, lease.id)
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)
    }

    /// Caller first authenticates this slot's exact source/context. Immutable
    /// occupied identities cannot be replaced while retirement is reconciled.
    pub(super) async fn retire_custody_selection(
        &self,
        before: &SecretCustodyMetadata,
    ) -> Result<(), AuthProductError> {
        let expected = match &before.state {
            SecretCustodyState::Original { context } => SecretIdentityClosure::Original {
                reference: before.reference.clone(),
                context: context.clone(),
            },
            SecretCustodyState::RetiredOriginal {
                original_reference,
                context,
            } => SecretIdentityClosure::Original {
                reference: original_reference.clone(),
                context: context.clone(),
            },
            SecretCustodyState::Retained { source } => SecretIdentityClosure::Retained {
                reference: before.reference.clone(),
                source: source.clone(),
            },
            SecretCustodyState::RetiredRetained {
                source,
                retained_reference,
            } => SecretIdentityClosure::Retained {
                reference: retained_reference.clone(),
                source: source.clone(),
            },
            _ => return Err(AuthProductError::BackendConflict),
        };
        self.close_exact_identity(&before.scope, &before.handle, expected)
            .await
    }

    pub(super) async fn close_exact_identity(
        &self,
        scope: &ResourceScope,
        handle: &SecretHandle,
        expected: SecretIdentityClosure,
    ) -> Result<(), AuthProductError> {
        let written = self
            .secret_store
            .close_identity(scope.clone(), handle.clone(), expected.clone())
            .await;
        let after = match written {
            Ok(Some(metadata)) => metadata,
            Ok(None) | Err(_) => self
                .secret_store
                .custody(scope, handle)
                .await
                .map_err(|_| AuthProductError::BackendUnavailable)?
                .ok_or(AuthProductError::BackendUnavailable)?,
        };
        if after.handle != *handle || !expected.accepts(scope, &after) {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(())
    }

    /// Reserve an absent output identity or retire its one exact captured
    /// source. The absent CAS and a concurrent first output can race only once:
    /// whichever wins occupies the immutable identity; no late output can escape
    /// a confirmed closure. This also fences CAS-only encrypted store owners.
    pub(super) async fn close_retained_slot(
        &self,
        scope: &ResourceScope,
        target: &SecretHandle,
        source: SecretCustodySource,
        absent_context: serde_json::Value,
    ) -> Result<(), AuthProductError> {
        let before = self
            .secret_store
            .custody(scope, target)
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?;
        if before.is_none() {
            let expected = SecretIdentityClosure::Absent {
                context: absent_context.clone(),
            };
            let written = self
                .secret_store
                .close_identity(scope.clone(), target.clone(), expected.clone())
                .await;
            if let Ok(Some(selected)) = &written
                && selected.handle == *target
                && expected.accepts(scope, selected)
            {
                return Ok(());
            }
        }
        let actual = self
            .secret_store
            .custody(scope, target)
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
            .ok_or(AuthProductError::BackendUnavailable)?;
        if actual.scope != *scope || actual.handle != *target {
            return Err(AuthProductError::BackendConflict);
        }
        match &actual.state {
            SecretCustodyState::ClosedIdentity { context } if context == &absent_context => Ok(()),
            SecretCustodyState::Retained { source: selected }
            | SecretCustodyState::RetiredRetained {
                source: selected, ..
            } if selected == &source => self.retire_custody_selection(&actual).await,
            _ => Err(AuthProductError::BackendConflict),
        }
    }

    pub(super) async fn original_private_material(
        &self,
        scope: &ResourceScope,
        flow_id: AuthFlowId,
    ) -> Result<(OAuthSetupOriginal, GooglePreparation), AuthProductError> {
        let original = self
            .retained_setup_original_for_resource(scope, flow_id)
            .await?
            .ok_or(AuthProductError::CredentialMissing)?;
        let selected = self.selected_preparation(&original).await?;
        let raw = self.read_selected(&selected).await?;
        let payload = GooglePreparation::decode(&raw, &original.definition)?;
        Ok((original, payload))
    }
}

fn preparation_handle(flow_id: AuthFlowId) -> Result<SecretHandle, AuthProductError> {
    SecretHandle::new(format!("google-oauth-setup-original-{flow_id}"))
        .map_err(|_| AuthProductError::BackendConflict)
}

mod secret_text {
    use super::*;
    pub(super) fn serialize<S: serde::Serializer>(
        value: &SecretString,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(value.expose_secret())
    }
    pub(super) fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<SecretString, D::Error> {
        SecretString::deserialize(deserializer)
    }
}

#[cfg(all(test, feature = "postgres"))]
#[path = "oauth_gate_original_tests.rs"]
pub(crate) mod tests;
