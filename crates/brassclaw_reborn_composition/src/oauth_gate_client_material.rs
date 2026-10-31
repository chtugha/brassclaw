//! Exact client selection for the existing Google callback/refresh transport.
//! Refresh originals contain client material only, never the setup PKCE bytes.
use super::*;
use async_trait::async_trait;
use brassclaw_auth::OAuthSetupOriginal;
use brassclaw_secrets::{
    SecretCustodyMetadata, SecretCustodySource, SecretCustodyState, SecretIdentityClosure,
};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};

use crate::oauth_dcr_protocol::StoredDcrClientMaterial as StoredClientMaterial;
use crate::oauth_provider_client::{OAuthClientMaterial, OAuthClientMaterialSource};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RefreshClientContext {
    format: String,
    refresh_secret: SecretHandle,
    setup: OAuthSetupOriginal,
    exchange_use_reference: String,
    predecessor: Option<SecretCustodySource>,
}

fn refresh_client_handle(refresh: &SecretHandle) -> Result<SecretHandle, AuthProductError> {
    SecretHandle::new(format!(
        "google-oauth-refresh-client-{}",
        brassclaw_common::hashing::sha256_hex(refresh.as_str().as_bytes())
    ))
    .map_err(|_| AuthProductError::BackendConflict)
}

fn refresh_read_handle(refresh: &SecretHandle) -> Result<SecretHandle, AuthProductError> {
    SecretHandle::new(format!(
        "google-oauth-refresh-use-{}",
        brassclaw_common::hashing::sha256_hex(refresh.as_str().as_bytes())
    ))
    .map_err(|_| AuthProductError::BackendConflict)
}

/// Existing account-lifecycle custody captures these alongside the refresh
/// token. Public auth records never carry private client bytes.
pub(super) fn owned_refresh_material_handles(
    refresh: &SecretHandle,
) -> Result<Vec<SecretHandle>, AuthProductError> {
    Ok(vec![
        refresh_client_handle(refresh)?,
        refresh_read_handle(refresh)?,
    ])
}

fn source(selected: &SecretCustodyMetadata) -> SecretCustodySource {
    SecretCustodySource {
        scope: selected.scope.clone(),
        handle: selected.handle.clone(),
        reference: selected.reference.clone(),
    }
}

impl GoogleOAuthGateProvider {
    async fn retain_refresh_client(
        &self,
        scope: &ResourceScope,
        refresh: &SecretHandle,
        client: StoredClientMaterial,
        context: RefreshClientContext,
    ) -> Result<(), AuthProductError> {
        let handle = refresh_client_handle(refresh)?;
        if context.format != "google-refresh-client/1"
            || context.refresh_secret != *refresh
            || context.setup.definition.scope.resource != *scope
            || context.exchange_use_reference.is_empty()
            || client.token_endpoint != brassclaw_auth::GOOGLE_TOKEN_ENDPOINT
        {
            return Err(AuthProductError::BackendConflict);
        }
        context.setup.validate()?;
        let context =
            serde_json::to_value(context).map_err(|_| AuthProductError::BackendUnavailable)?;
        let material = SecretString::from(
            serde_json::to_string(&client).map_err(|_| AuthProductError::BackendUnavailable)?,
        );
        let written = self
            .secret_store
            .put_original(
                scope.clone(),
                handle.clone(),
                material.clone(),
                context.clone(),
            )
            .await;
        let selected = match written {
            Ok(Some(metadata)) => metadata,
            Ok(None) | Err(_) => {
                let metadata = self
                    .secret_store
                    .custody(scope, &handle)
                    .await
                    .map_err(|_| AuthProductError::BackendUnavailable)?
                    .ok_or(AuthProductError::BackendUnavailable)?;
                if !self
                    .secret_store
                    .matches(scope, &handle, &material)
                    .await
                    .map_err(|_| AuthProductError::BackendUnavailable)?
                {
                    return Err(AuthProductError::BackendConflict);
                }
                metadata
            }
        };
        if selected.scope != *scope
            || selected.handle != handle
            || selected.state != (SecretCustodyState::Original { context })
        {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(())
    }

    async fn refresh_client_original(
        &self,
        scope: &ResourceScope,
        refresh: &SecretHandle,
    ) -> Result<(SecretCustodyMetadata, RefreshClientContext), AuthProductError> {
        let handle = refresh_client_handle(refresh)?;
        let selected = self
            .secret_store
            .custody(scope, &handle)
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
            .ok_or(AuthProductError::CredentialMissing)?;
        let context = match &selected.state {
            SecretCustodyState::Original { context }
            | SecretCustodyState::RetiredOriginal { context, .. } => context,
            _ => return Err(AuthProductError::BackendConflict),
        };
        let context: RefreshClientContext = serde_json::from_value(context.clone())
            .map_err(|_| AuthProductError::BackendConflict)?;
        if selected.scope != *scope
            || selected.handle != handle
            || context.format != "google-refresh-client/1"
            || context.refresh_secret != *refresh
            || context.exchange_use_reference.is_empty()
            || context.setup.definition.scope.resource != *scope
            || context.predecessor.as_ref().is_some_and(|previous| {
                previous.scope != *scope
                    || previous.reference.is_empty()
                    || previous.handle == handle
            })
        {
            return Err(AuthProductError::BackendConflict);
        }
        let setup = self
            .retained_setup_original_for_resource(scope, context.setup.definition.flow_id)
            .await?
            .ok_or(AuthProductError::BackendConflict)?;
        setup.verify(&context.setup)?;
        let marker_handle = setup.definition.exchange_use_handle()?;
        let marker = self
            .secret_store
            .custody(scope, &marker_handle)
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
            .ok_or(AuthProductError::BackendConflict)?;
        if marker.scope != *scope || marker.handle != marker_handle {
            return Err(AuthProductError::BackendConflict);
        }
        let expected_source = SecretCustodySource {
            scope: scope.clone(),
            handle: setup.preparation.handle,
            reference: setup.preparation.reference,
        };
        match &marker.state {
            SecretCustodyState::Retained { source }
                if source == &expected_source
                    && marker.reference == context.exchange_use_reference => {}
            SecretCustodyState::RetiredRetained {
                source,
                retained_reference,
            } if source == &expected_source
                && retained_reference == &context.exchange_use_reference => {}
            _ => return Err(AuthProductError::BackendConflict),
        }
        Ok((selected, context))
    }

    async fn read_refresh_client(
        &self,
        scope: &ResourceScope,
        refresh: &SecretHandle,
    ) -> Result<
        (
            SecretCustodyMetadata,
            RefreshClientContext,
            StoredClientMaterial,
        ),
        AuthProductError,
    > {
        let (original, context) = self.refresh_client_original(scope, refresh).await?;
        if !matches!(original.state, SecretCustodyState::Original { .. }) {
            return Err(AuthProductError::BackendConflict);
        }
        let source = source(&original);
        let target = refresh_read_handle(refresh)?;
        let written = self
            .secret_store
            .put_retained_from_original(scope.clone(), target.clone(), source.clone())
            .await;
        let selected = match written {
            Ok(Some(metadata)) => metadata,
            Ok(None) | Err(_) => self
                .secret_store
                .custody(scope, &target)
                .await
                .map_err(|_| AuthProductError::BackendUnavailable)?
                .ok_or(AuthProductError::BackendUnavailable)?,
        };
        if selected.scope != *scope
            || selected.handle != target
            || selected.state != (SecretCustodyState::Retained { source })
        {
            return Err(AuthProductError::BackendConflict);
        }
        let raw = self.read_selected(&selected).await?;
        let client: StoredClientMaterial = serde_json::from_str(raw.expose_secret())
            .map_err(|_| AuthProductError::BackendConflict)?;
        if client.token_endpoint != brassclaw_auth::GOOGLE_TOKEN_ENDPOINT {
            return Err(AuthProductError::BackendConflict);
        }
        Ok((original, context, client))
    }

    /// Only the existing Auth owner may call this after confirmed completion or
    /// terminal setup cancellation; uncertain callbacks retain their material.
    pub(crate) async fn retire_setup_material(
        &self,
        scope: &ResourceScope,
        flow_id: AuthFlowId,
    ) -> Result<(), AuthProductError> {
        let Some(original) = self
            .retained_setup_original_for_resource(scope, flow_id)
            .await?
        else {
            return Err(AuthProductError::CredentialMissing);
        };
        let selected = SecretCustodySource {
            scope: scope.clone(),
            handle: original.preparation.handle.clone(),
            reference: original.preparation.reference.clone(),
        };
        self.close_retained_slot(
            scope,
            &original.definition.exchange_use_handle()?,
            selected,
            serde_json::json!({"format":"google-setup-use-closed/1", "setup":&original}),
        )
        .await?;
        let before = self
            .secret_store
            .custody(scope, &original.preparation.handle)
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
            .ok_or(AuthProductError::BackendConflict)?;
        let valid = match &before.state {
            SecretCustodyState::Original { context } => {
                before.reference == original.preparation.reference
                    && *context == original.definition.preparation_context()
            }
            SecretCustodyState::RetiredOriginal {
                original_reference,
                context,
            } => {
                original_reference == &original.preparation.reference
                    && *context == original.definition.preparation_context()
            }
            _ => false,
        };
        if before.scope != *scope || before.handle != original.preparation.handle || !valid {
            return Err(AuthProductError::BackendConflict);
        }
        self.retire_custody_selection(&before).await?;
        Ok(())
    }
}

#[async_trait]
impl OAuthClientMaterialSource for GoogleOAuthGateProvider {
    async fn exchange_material(
        &self,
        scope: &ResourceScope,
        flow_id: AuthFlowId,
    ) -> Result<OAuthClientMaterial, AuthProductError> {
        let (original, payload) = self.original_private_material(scope, flow_id).await?;
        if original.definition.expires_at <= Utc::now() {
            return Err(AuthProductError::UnknownOrExpiredFlow);
        }
        payload.client.into_client_material()
    }

    async fn refresh_material(
        &self,
        scope: &ResourceScope,
        refresh_secret: &SecretHandle,
    ) -> Result<OAuthClientMaterial, AuthProductError> {
        let (_, _, client) = self.read_refresh_client(scope, refresh_secret).await?;
        client.into_client_material()
    }

    async fn bind_refresh_material(
        &self,
        scope: &ResourceScope,
        flow_id: AuthFlowId,
        refresh_secret: &SecretHandle,
    ) -> Result<(), AuthProductError> {
        let (original, payload) = self.original_private_material(scope, flow_id).await?;
        let selected = self.selected_preparation(&original).await?;
        self.retain_refresh_client(
            scope,
            refresh_secret,
            payload.client,
            RefreshClientContext {
                format: "google-refresh-client/1".into(),
                refresh_secret: refresh_secret.clone(),
                setup: original,
                exchange_use_reference: selected.reference,
                predecessor: None,
            },
        )
        .await
    }

    async fn bind_rotated_refresh_material(
        &self,
        scope: &ResourceScope,
        previous: &SecretHandle,
        replacement: &SecretHandle,
    ) -> Result<(), AuthProductError> {
        if previous == replacement {
            return Err(AuthProductError::BackendConflict);
        }
        let (original, mut context, client) = self.read_refresh_client(scope, previous).await?;
        context.predecessor = Some(source(&original));
        context.refresh_secret = replacement.clone();
        self.retain_refresh_client(scope, replacement, client, context)
            .await
    }

    async fn cleanup_refresh_material(
        &self,
        scope: &ResourceScope,
        refresh_secret: &SecretHandle,
    ) -> Result<(), AuthProductError> {
        // First close an actually absent binding. This fences a delayed initial
        // publication without inventing a client generation. If that publication
        // won the one CAS race, authenticate and retire its exact value below.
        let handle = refresh_client_handle(refresh_secret)?;
        let absent_context = serde_json::json!({
            "format":"google-refresh-binding-closed/1", "refresh_secret":refresh_secret,
        });
        let before = self
            .secret_store
            .custody(scope, &handle)
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?;
        let already_closed = before.as_ref().is_some_and(|selected| {
            selected.handle == handle
                && selected.scope == *scope
                && selected.state
                    == (SecretCustodyState::ClosedIdentity {
                        context: absent_context.clone(),
                    })
        });
        let closed_absent = if before.is_none() {
            self.close_exact_identity(
                scope,
                &handle,
                SecretIdentityClosure::Absent {
                    context: absent_context.clone(),
                },
            )
            .await
            .is_ok()
        } else {
            already_closed
        };
        if closed_absent {
            self.close_exact_identity(
                scope,
                &refresh_read_handle(refresh_secret)?,
                SecretIdentityClosure::Absent {
                    context: absent_context,
                },
            )
            .await?;
            return Ok(());
        }
        // Retire only this binding's client-only bytes. A rotated successor owns
        // its own immutable copy and never depends on these live private bytes.
        let (original, _) = self.refresh_client_original(scope, refresh_secret).await?;
        let original_reference = match &original.state {
            SecretCustodyState::Original { .. } => &original.reference,
            SecretCustodyState::RetiredOriginal {
                original_reference, ..
            } => original_reference,
            _ => return Err(AuthProductError::BackendConflict),
        };
        let expected_source = SecretCustodySource {
            scope: scope.clone(),
            handle: original.handle.clone(),
            reference: original_reference.clone(),
        };
        self.close_retained_slot(
            scope,
            &refresh_read_handle(refresh_secret)?,
            expected_source.clone(),
            serde_json::json!({"format":"google-refresh-use-closed/1", "source":expected_source}),
        )
        .await?;
        self.retire_custody_selection(&original).await?;
        Ok(())
    }

    async fn cleanup_exchange_material(
        &self,
        scope: &ResourceScope,
        flow_id: AuthFlowId,
    ) -> Result<(), AuthProductError> {
        self.retire_setup_material(scope, flow_id).await
    }
}
