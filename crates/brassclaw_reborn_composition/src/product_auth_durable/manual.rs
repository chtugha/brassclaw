//! Original manual input/selection data: only the encrypted SecretStore retains
//! this context. A planned result is NOT proof of account publication.
use super::interactions::StoredManualTokenInteraction;
use super::*;
use brassclaw_auth::{
    AuthChallenge, AuthFlowKind, AuthFlowStatus, CredentialAccountUpdateBinding,
    CredentialOwnership, ManualTokenCompletionInput, ManualTokenSetupRequest, SecretSubmitResult,
};
use brassclaw_secrets::SecretCustodySource;
use serde::{Deserialize, Serialize};

const LEGACY_ORIGINAL_KIND: &str = "filesystem-manual-token-original/1";
const ORIGINAL_KIND: &str = "filesystem-manual-token-original/2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SelectedEntry {
    version: RecordVersion,
    body_checksum: EntryBodyChecksum,
}
impl From<AuthRecordSnapshot> for SelectedEntry {
    fn from(value: AuthRecordSnapshot) -> Self {
        Self {
            version: value.version,
            body_checksum: value.body_checksum,
        }
    }
}
impl SelectedEntry {
    pub(super) fn snapshot(self) -> AuthRecordSnapshot {
        AuthRecordSnapshot {
            version: self.version,
            body_checksum: self.body_checksum,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum ManualDecision {
    Selected {
        original: SecretCustodySource,
        selected_at: brassclaw_auth::Timestamp,
    },
    Published {
        original: SecretCustodySource,
    },
    Superseded {
        original: SecretCustodySource,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SelectionKind {
    New,
    Bound,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManualOriginal {
    kind: String,
    pub(super) interaction: StoredManualTokenInteraction,
    pub(super) selection: SelectionKind,
    pub(super) complete_flow: bool,
    pub(super) expected_account: Option<CredentialAccount>,
    pub(super) account_entry: Option<SelectedEntry>,
    pub(super) account: CredentialAccount,
    pub(super) pending_flow: Option<AuthFlowRecord>,
    pub(super) flow_entry: Option<SelectedEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pending_custody: Option<Box<brassclaw_auth::AuthFlowCustody>>,
}

fn binding_request(pending: &StoredManualTokenInteraction) -> ManualTokenSetupRequest {
    ManualTokenSetupRequest {
        requirement_setup: pending.requirement_setup.clone(),
        scope: pending.scope.clone(),
        provider: pending.provider.clone(),
        label: pending.label.clone(),
        continuation: pending.continuation.clone(),
        update_binding: pending.update_binding.clone(),
        expires_at: pending.expires_at,
    }
}

fn prepare_account(
    pending: &StoredManualTokenInteraction,
    selection: SelectionKind,
    expected: Option<&CredentialAccount>,
    now: brassclaw_auth::Timestamp,
    current_format: bool,
) -> Result<CredentialAccount, AuthProductError> {
    if now > pending.expires_at {
        return Err(AuthProductError::UnknownOrExpiredFlow);
    }
    if pending.consumed_at.is_some()
        || pending.publication.is_some()
        || pending.canceled_at.is_some()
    {
        return Err(AuthProductError::BackendConflict);
    }
    match (selection, &pending.update_binding, expected) {
        (SelectionKind::New, None, None) if pending.update_account_snapshot.is_none() => {}
        (SelectionKind::Bound, Some(binding), Some(account)) => {
            if pending.update_account_snapshot.as_ref() != Some(account) {
                return Err(AuthProductError::BackendConflict);
            }
            super::domain::validate_manual_token_update_binding(
                account,
                &binding_request(pending),
                binding,
            )?;
        }
        _ => return Err(AuthProductError::BackendConflict),
    }
    let id = expected
        .map(|account| account.id)
        .unwrap_or_else(|| CredentialAccountId::from_uuid(pending.id.as_uuid()));
    let binding: Option<&CredentialAccountUpdateBinding> = pending.update_binding.as_ref();
    let request = NewCredentialAccount {
        scope: expected
            .map(|account| account.scope.clone())
            .unwrap_or_else(|| pending.scope.clone()),
        provider: pending.provider.clone(),
        label: pending.label.clone(),
        status: credential_status_for_completed_flow(),
        ownership: binding.map_or(CredentialOwnership::UserReusable, |binding| {
            binding.ownership
        }),
        owner_extension: binding.and_then(|binding| binding.owner_extension.clone()),
        granted_extensions: binding
            .map(|binding| binding.granted_extensions.clone())
            .unwrap_or_default(),
        access_secret: Some(super::paths::manual_token_secret_handle(id, pending.id)?),
        access_expires_at: None,
        refresh_secret: None,
        scopes: if !current_format {
            // Immutable /1 originals retain their actual historical output.
            vec![]
        } else if let Some(expected) = expected {
            expected.scopes.clone()
        } else {
            // Requested metadata, not proof of external issuer privileges.
            pending
                .requirement_setup
                .as_ref()
                .map(|setup| {
                    setup.verify_manual(&pending.scope, &pending.provider)?;
                    setup
                        .requirement()
                        .requirement()
                        .provider_scopes
                        .iter()
                        .cloned()
                        .map(brassclaw_auth::ProviderScope::new)
                        .collect::<Result<Vec<_>, AuthProductError>>()
                })
                .transpose()?
                .unwrap_or_default()
        },
    };
    super::domain::validate_new_credential_account(&request)?;
    if let Some(expected) = expected {
        super::domain::validate_account_update_target(expected, &request)?;
        let mut account = expected.clone();
        return super::domain::update_account_from_request(&mut account, request, now);
    }
    Ok(CredentialAccount {
        id,
        scope: request.scope,
        provider: request.provider,
        label: request.label,
        status: request.status,
        ownership: request.ownership,
        owner_extension: request.owner_extension,
        granted_extensions: request.granted_extensions,
        access_secret: request.access_secret,
        access_expires_at: request.access_expires_at,
        refresh_secret: request.refresh_secret,
        scopes: request.scopes,
        created_at: now,
        updated_at: now,
    })
}

impl ManualOriginal {
    pub(super) fn prepare(
        interaction: StoredManualTokenInteraction,
        selection: SelectionKind,
        complete_flow: bool,
        selected: Option<(CredentialAccount, AuthRecordSnapshot)>,
        flow: Option<(crate::auth_flow_custody::StoredAuthFlow, AuthRecordSnapshot)>,
        now: brassclaw_auth::Timestamp,
    ) -> Result<Self, AuthProductError> {
        let account = prepare_account(
            &interaction,
            selection,
            selected.as_ref().map(|(account, _)| account),
            now,
            true,
        )?;
        let (expected_account, account_entry) = match selected {
            Some((account, snapshot)) => (Some(account), Some(snapshot.into())),
            None => (None, None),
        };
        let (pending_flow, flow_entry, pending_custody) = match flow {
            Some((document, snapshot)) if document.fs_callback_publication.is_none() => (
                Some(document.flow),
                Some(snapshot.into()),
                Some(Box::new(document.auth_custody)),
            ),
            Some(_) => return Err(AuthProductError::BackendConflict),
            None => (None, None, None),
        };
        let original = Self {
            kind: ORIGINAL_KIND.into(),
            interaction,
            selection,
            complete_flow,
            expected_account,
            account_entry,
            account,
            pending_flow,
            flow_entry,
            pending_custody,
        };
        original.verify()?;
        Ok(original)
    }
    pub(super) fn verify(&self) -> Result<(), AuthProductError> {
        match (
            self.interaction.setup_flow_id,
            self.interaction.setup_created_at,
            self.pending_flow.as_ref(),
        ) {
            (None, None, _) => {}
            (Some(id), Some(created_at), Some(flow))
                if flow.id == id && flow.created_at == created_at => {}
            _ => return Err(AuthProductError::BackendConflict),
        }
        if (self.complete_flow && self.pending_flow.is_none())
            || !matches!(self.kind.as_str(), ORIGINAL_KIND | LEGACY_ORIGINAL_KIND)
            || (self.kind == LEGACY_ORIGINAL_KIND && self.pending_custody.is_some())
            || (self.kind == ORIGINAL_KIND
                && self.pending_custody.is_some() != self.pending_flow.is_some())
            || self.expected_account.is_some() != self.account_entry.is_some()
            || self.pending_flow.is_some() != self.flow_entry.is_some()
            || prepare_account(
                &self.interaction,
                self.selection,
                self.expected_account.as_ref(),
                self.account.updated_at,
                self.kind == ORIGINAL_KIND,
            )? != self.account
        {
            return Err(AuthProductError::BackendConflict);
        }
        if let Some(custody) = &self.pending_custody {
            if custody.requirement_setup != self.interaction.requirement_setup
                || custody.update_account_snapshot != self.interaction.update_account_snapshot
                || custody.requirement_selection.is_some()
                || custody.requirement_source.is_some()
                || custody.oauth_callback.is_some()
            {
                return Err(AuthProductError::BackendConflict);
            }
            if custody.requirement_setup.is_some() && !self.complete_flow {
                return Err(AuthProductError::BackendConflict);
            }
        }
        self.completed_flow()?;
        self.requirement_selection()?;
        Ok(())
    }

    fn pending_document(&self) -> Option<crate::auth_flow_custody::StoredAuthFlow> {
        Some(crate::auth_flow_custody::StoredAuthFlow {
            flow: self.pending_flow.clone()?,
            auth_custody: *self.pending_custody.clone()?,
            fs_callback_publication: None,
        })
    }

    fn requirement_selection(
        &self,
    ) -> Result<Option<brassclaw_auth::CredentialRequirementSelection>, AuthProductError> {
        if self.kind == LEGACY_ORIGINAL_KIND {
            // Recover the historical result without manufacturing a new binding.
            return Ok(None);
        }
        self.interaction
            .requirement_setup
            .as_ref()
            .map(|setup| {
                brassclaw_auth::CredentialRequirementSelection::from_completed_flow(
                    &self.interaction.scope,
                    setup,
                    &self
                        .completed_flow()?
                        .ok_or(AuthProductError::BackendConflict)?,
                    self.account.clone(),
                )
            })
            .transpose()
    }

    fn verify_completed_document(
        &self,
        document: &crate::auth_flow_custody::StoredAuthFlow,
    ) -> Result<(), AuthProductError> {
        let expected = self
            .completed_flow()?
            .ok_or(AuthProductError::BackendConflict)?;
        if !runtime::final_flow_matches(&document.flow, &expected) {
            return Err(AuthProductError::BackendConflict);
        }
        if let Some(mut pending) = self.pending_document() {
            pending.auth_custody.requirement_selection =
                self.requirement_selection()?.map(Box::new);
            if let Some(source) = &document.auth_custody.requirement_source {
                source.verify_selection(
                    pending
                        .auth_custody
                        .requirement_selection
                        .as_deref()
                        .ok_or(AuthProductError::BackendConflict)?,
                )?;
                pending.auth_custody.requirement_source = Some(source.clone());
            }
            if pending.auth_custody != document.auth_custody
                || document.fs_callback_publication.is_some()
            {
                return Err(AuthProductError::BackendConflict);
            }
        }
        Ok(())
    }
    pub(super) fn completed_flow(&self) -> Result<Option<AuthFlowRecord>, AuthProductError> {
        let Some(mut flow) = self.pending_flow.clone() else {
            return Ok(None);
        };
        if !scope_matches(&self.interaction.scope, &flow.scope)
            || flow.kind != AuthFlowKind::IntegrationCredential
            || flow.status != AuthFlowStatus::AwaitingUser
            || flow.provider != self.interaction.provider
            || flow.continuation != self.interaction.continuation
            || flow.update_binding != self.interaction.update_binding
            || flow.error.is_some()
            || flow.credential_account_id.is_some()
            || flow.continuation_emitted_at.is_some()
            || flow.opaque_state_hash.is_some()
            || flow.pkce_verifier_hash.is_some()
            || flow.authorization_code_hash.is_some()
            || flow.expires_at != self.interaction.expires_at
            || !matches!(&flow.challenge,Some(AuthChallenge::ManualTokenRequired{interaction_id,provider,label,expires_at})
                if *interaction_id==self.interaction.id && *provider==self.interaction.provider
                && *label==self.interaction.label && *expires_at==self.interaction.expires_at)
        {
            return Err(AuthProductError::BackendConflict);
        }
        super::domain::validate_manual_token_flow(
            &mut flow,
            &self.interaction.scope,
            &ManualTokenCompletionInput {
                interaction_id: self.interaction.id,
                credential_account_id: self.account.id,
            },
            self.account.updated_at,
        )?;
        flow.status = AuthFlowStatus::Completed;
        flow.error = None;
        flow.credential_account_id = Some(self.account.id);
        flow.updated_at = self.account.updated_at;
        Ok(Some(flow))
    }
    pub(super) fn result(&self) -> SecretSubmitResult {
        SecretSubmitResult {
            account_id: self.account.id,
            status: self.account.status,
            continuation: self.interaction.continuation.clone(),
        }
    }
}

fn original_handle(
    interaction: brassclaw_auth::AuthInteractionId,
) -> Result<brassclaw_host_api::SecretHandle, AuthProductError> {
    brassclaw_host_api::SecretHandle::new(format!("product-auth-manual-original-{interaction}"))
        .map_err(|_| AuthProductError::BackendUnavailable)
}

fn custody_error(error: brassclaw_secrets::SecretStoreError) -> AuthProductError {
    match error {
        brassclaw_secrets::SecretStoreError::UnknownSecret { .. }
        | brassclaw_secrets::SecretStoreError::SecretExpired => AuthProductError::BackendConflict,
        _ => AuthProductError::BackendUnavailable,
    }
}

fn decode_original(
    scope: &brassclaw_auth::AuthProductScope,
    interaction: brassclaw_auth::AuthInteractionId,
    metadata: brassclaw_secrets::SecretCustodyMetadata,
) -> Result<(ManualOriginal, SecretCustodySource), AuthProductError> {
    if metadata.scope != scope.resource || metadata.handle != original_handle(interaction)? {
        return Err(AuthProductError::CrossScopeDenied);
    }
    let brassclaw_secrets::SecretCustodyState::Original { context } = metadata.state else {
        return Err(AuthProductError::BackendConflict);
    };
    let original: ManualOriginal =
        serde_json::from_value(context).map_err(|_| AuthProductError::BackendConflict)?;
    original.verify()?;
    if original.interaction.id != interaction || !scope_matches(scope, &original.interaction.scope)
    {
        return Err(AuthProductError::CrossScopeDenied);
    }
    Ok((
        original,
        SecretCustodySource {
            scope: metadata.scope,
            handle: metadata.handle,
            reference: metadata.reference,
        },
    ))
}

impl<F: RootFilesystem> FilesystemAuthProductServices<F> {
    /// Internal metadata read, never raw-token acceptance or publication proof.
    /// Callers that receive a new raw input must additionally prove it below.
    pub(super) async fn read_manual_original(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        interaction: brassclaw_auth::AuthInteractionId,
    ) -> Result<Option<(ManualOriginal, SecretCustodySource)>, AuthProductError> {
        let handle = original_handle(interaction)?;
        let Some(metadata) = self
            .secret_store
            .custody(&scope.resource, &handle)
            .await
            .map_err(custody_error)?
        else {
            return Ok(None);
        };
        decode_original(scope, interaction, metadata).map(Some)
    }

    pub(super) async fn prove_manual_original(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        request: &brassclaw_auth::SecretSubmitRequest,
    ) -> Result<Option<(ManualOriginal, SecretCustodySource)>, AuthProductError> {
        let Some(original) = self
            .read_manual_original(scope, request.interaction_id)
            .await?
        else {
            return Ok(None);
        };
        if !self
            .secret_store
            .matches(&scope.resource, &original.1.handle, &request.secret)
            .await
            .map_err(custody_error)?
        {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(Some(original))
    }

    /// First input selects its immutable context inside the existing secret
    /// owner. A same-input concurrent loser recovers the WINNING context; it
    /// cannot substitute its own later account choice or result timestamp.
    pub(super) async fn retain_manual_original(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        request: brassclaw_auth::SecretSubmitRequest,
        proposed: ManualOriginal,
    ) -> Result<(ManualOriginal, SecretCustodySource), AuthProductError> {
        if let Some(original) = self.prove_manual_original(scope, &request).await? {
            return Ok(original);
        }
        proposed.verify()?;
        if proposed.interaction.id != request.interaction_id
            || !scope_matches(scope, &proposed.interaction.scope)
        {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let context =
            serde_json::to_value(&proposed).map_err(|_| AuthProductError::BackendUnavailable)?;
        // Keep the candidate only through uncertain-write/race readback. It is
        // never serialized outside the encrypted owner or logged/fingerprinted.
        let result = self
            .secret_store
            .put_original(
                scope.resource.clone(),
                original_handle(request.interaction_id)?,
                request.secret.clone(),
                context,
            )
            .await;
        let write_error = match result {
            Ok(Some(metadata)) => {
                let original = decode_original(scope, request.interaction_id, metadata)?;
                if original.0 != proposed {
                    return Err(AuthProductError::BackendConflict);
                }
                return Ok(original);
            }
            Ok(None) => AuthProductError::BackendConflict,
            Err(error) => custody_error(error),
        };
        // An error can still mean the original committed. Read/prove the same
        // identity before answering; no compensating delete or replacement.
        self.prove_manual_original(scope, &request)
            .await?
            .ok_or(write_error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManualReceipt {
    original: SecretCustodySource,
    account: CredentialAccount,
    result: SecretSubmitResult,
}
impl ManualReceipt {
    fn new(original: &ManualOriginal, source: &SecretCustodySource) -> Self {
        Self {
            original: source.clone(),
            account: original.account.clone(),
            result: original.result(),
        }
    }
}
impl ManualDecision {
    fn original(&self) -> &SecretCustodySource {
        match self {
            Self::Selected { original, .. }
            | Self::Published { original }
            | Self::Superseded { original } => original,
        }
    }
}

#[path = "manual_runtime.rs"]
mod runtime;

#[path = "manual_requirement.rs"]
mod requirement;
