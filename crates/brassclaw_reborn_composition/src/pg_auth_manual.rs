//! The native flow transaction owns input, credential, interaction and flow.
//! The lower interaction port publishes only its declared account/interaction.
//! No issuer call or continuation dispatch runs inside this transaction.
use super::*;
use crate::auth_flow_custody::StoredAuthFlow;
use brassclaw_secrets::{SecretCustodyMetadata, SecretCustodyState};

const LEGACY_ORIGINAL_KIND: &str = "native-manual-token-publication/1";
const ORIGINAL_KIND: &str = "native-manual-token-publication/2";

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManualPublication {
    kind: String,
    interaction: StoredManualTokenInteraction,
    expected_account: Option<CredentialAccount>,
    account: CredentialAccount,
    flow: Option<AuthFlowRecord>,
}

fn original_handle(id: AuthInteractionId) -> Result<SecretHandle, AuthProductError> {
    SecretHandle::new(format!("product-auth-manual-original-{id}"))
        .map_err(|_| AuthProductError::BackendUnavailable)
}

fn access_handle(
    account: CredentialAccountId,
    interaction: AuthInteractionId,
) -> Result<SecretHandle, AuthProductError> {
    SecretHandle::new(format!("product-auth-manual-{account}-{interaction}"))
        .map_err(|_| AuthProductError::BackendUnavailable)
}

impl ManualPublication {
    fn verify_flow_custody(&self, document: &StoredAuthFlow) -> Result<(), AuthProductError> {
        if document.auth_custody.requirement_setup != self.interaction.requirement_setup
            || document.auth_custody.update_account_snapshot
                != self.interaction.update_account_snapshot
        {
            return Err(AuthProductError::BackendConflict);
        }
        let expected = self
            .interaction
            .requirement_setup
            .as_deref()
            .filter(|_| {
                // A v1 original may confirm its historical account result but
                // did not publish this binding. Missing selection is not a
                // grant or permission to manufacture one during recovery.
                self.kind != LEGACY_ORIGINAL_KIND
                    || document.auth_custody.requirement_selection.is_some()
            })
            .map(|setup| {
                brassclaw_auth::CredentialRequirementSelection::from_completed_flow(
                    &self.interaction.scope,
                    setup,
                    self.flow
                        .as_ref()
                        .ok_or(AuthProductError::BackendConflict)?,
                    self.account.clone(),
                )
            })
            .transpose()?;
        if document.auth_custody.requirement_selection.as_deref() != expected.as_ref() {
            return Err(AuthProductError::BackendConflict);
        }
        if let Some(source) = &document.auth_custody.requirement_source {
            source.verify_selection(expected.as_ref().ok_or(AuthProductError::BackendConflict)?)?;
        }
        Ok(())
    }

    fn verify(
        &self,
        scope: &AuthProductScope,
        interaction: AuthInteractionId,
    ) -> Result<(), AuthProductError> {
        if !matches!(self.kind.as_str(), ORIGINAL_KIND | LEGACY_ORIGINAL_KIND)
            || self.interaction.id != interaction
            || !scope_matches(scope, &self.interaction.scope)
            || self.interaction.consumed_at.is_some()
            || self.interaction.canceled_at.is_some()
            || self.account.provider != self.interaction.provider
            || self.account.label != self.interaction.label
            || self.account.status != CredentialAccountStatus::Configured
            || self.account.access_secret.as_ref()
                != Some(&access_handle(self.account.id, interaction)?)
            || !scope_matches(scope, &self.account.scope)
        {
            return Err(AuthProductError::BackendConflict);
        }
        let mut expected = prepare_account(
            &self.interaction,
            self.expected_account.as_ref(),
            self.account.updated_at,
        )?;
        if self.kind == LEGACY_ORIGINAL_KIND {
            // v1 cleared scope metadata, including on bound updates. Verify
            // its retained result with that historical rule, not today's one.
            expected.scopes.clear();
        }
        if expected != self.account {
            return Err(AuthProductError::BackendConflict);
        }
        if let Some(flow) = &self.flow
            && (flow.status != AuthFlowStatus::Completed
                || flow.kind != brassclaw_auth::AuthFlowKind::IntegrationCredential
                || flow.provider != self.interaction.provider
                || flow.continuation_emitted_at.is_some()
                || flow.error.is_some()
                || flow.updated_at != self.account.updated_at
                || !scope_matches(scope, &flow.scope)
                || flow.credential_account_id != Some(self.account.id)
                || flow.continuation != self.interaction.continuation
                || flow.expires_at != self.interaction.expires_at
                || flow.update_binding != self.interaction.update_binding
                || !matches!(&flow.challenge,
                    Some(AuthChallenge::ManualTokenRequired { interaction_id, provider, label, expires_at })
                    if *interaction_id == interaction
                        && *provider == self.interaction.provider
                        && *label == self.interaction.label
                        && *expires_at == self.interaction.expires_at))
        {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(())
    }

    fn result(&self) -> SecretSubmitResult {
        SecretSubmitResult {
            account_id: self.account.id,
            status: self.account.status,
            continuation: self.interaction.continuation.clone(),
        }
    }
}

fn prepare_account(
    pending: &StoredManualTokenInteraction,
    expected: Option<&CredentialAccount>,
    now: brassclaw_auth::Timestamp,
) -> Result<CredentialAccount, AuthProductError> {
    match (&pending.update_binding, expected) {
        (Some(binding), Some(account)) => {
            if account.id != binding.account_id
                || pending.update_account_snapshot.as_ref() != Some(account)
            {
                return Err(AuthProductError::BackendConflict);
            }
            validate_manual_token_update_binding(
                account,
                &ManualTokenSetupRequest {
                    requirement_setup: pending.requirement_setup.clone(),
                    scope: pending.scope.clone(),
                    provider: pending.provider.clone(),
                    label: pending.label.clone(),
                    continuation: pending.continuation.clone(),
                    update_binding: pending.update_binding.clone(),
                    expires_at: pending.expires_at,
                },
                binding,
            )?;
        }
        (None, None) if pending.update_account_snapshot.is_none() => {}
        _ => return Err(AuthProductError::BackendConflict),
    }
    let account_id = expected
        .map(|account| account.id)
        .unwrap_or_else(|| CredentialAccountId::from_uuid(pending.id.as_uuid()));
    let binding = pending.update_binding.as_ref();
    // A bound update preserves its declared scopes. For a new manual account,
    // the exact captured usage supplies requested scope metadata; this is not
    // evidence that the external issuer actually granted those privileges.
    let scopes = if let Some(expected) = expected {
        expected.scopes.clone()
    } else {
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
    };
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
        access_secret: Some(access_handle(account_id, pending.id)?),
        access_expires_at: None,
        refresh_secret: None,
        scopes,
    };
    validate_new_credential_account(&request)?;
    match expected {
        Some(expected) => {
            let mut account = expected.clone();
            update_account_from_request(&mut account, request, now)
        }
        None => Ok(CredentialAccount {
            id: account_id,
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
        }),
    }
}

async fn manual_flow(
    transaction: &tokio_postgres::Transaction<'_>,
    scope: &AuthProductScope,
    interaction: AuthInteractionId,
) -> Result<Option<(StoredAuthFlow, i64)>, AuthProductError> {
    let rows = transaction
        .query(
            "SELECT data,revision FROM brassclaw_product_auth_flows
         WHERE tenant_id=$1 AND user_id=$2
           AND data->'challenge'->>'type'='manual_token_required'
           AND data->'challenge'->>'interaction_id'=$3
         ORDER BY id FOR UPDATE",
            &[
                &scope.resource.tenant_id.as_str(),
                &scope.resource.user_id.as_str(),
                &interaction.to_string(),
            ],
        )
        .await
        .map_err(db_err)?;
    if rows.len() > 1 {
        return Err(AuthProductError::BackendConflict);
    }
    let Some(row) = rows.into_iter().next() else {
        return Ok(None);
    };
    let document: StoredAuthFlow =
        serde_json::from_value(row.get(0)).map_err(|_| AuthProductError::BackendUnavailable)?;
    if !scope_matches(scope, &document.flow.scope) {
        return Err(AuthProductError::CrossScopeDenied);
    }
    Ok(Some((document, row.get(1))))
}

async fn selected_account(
    transaction: &tokio_postgres::Transaction<'_>,
    pending: &StoredManualTokenInteraction,
) -> Result<Option<(CredentialAccount, i64)>, AuthProductError> {
    if let Some(binding) = &pending.update_binding {
        let selected = callback::locked_account(transaction, &pending.scope, binding.account_id)
            .await?
            .ok_or(AuthProductError::CredentialMissing)?;
        if pending.update_account_snapshot.as_ref() != Some(&selected.0) {
            return Err(AuthProductError::BackendConflict);
        }
        return Ok(Some(selected));
    }
    // An unbound native setup selects its own interaction-derived account.
    // It cannot reinterpret another account as an authorized update target.
    let target = CredentialAccountId::from_uuid(pending.id.as_uuid());
    if callback::locked_account(transaction, &pending.scope, target)
        .await?
        .is_some()
    {
        return Err(AuthProductError::BackendConflict);
    }
    Ok(None)
}

impl PgAuthProductServices {
    pub(super) async fn continue_native_manual_original(
        &self,
        scope: &AuthProductScope,
        identity: ManualTokenSetupIdentity,
    ) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError> {
        let handle = original_handle(identity.interaction_id)?;
        let metadata = self
            .secret_store
            .custody(&scope.resource, &handle)
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
            .ok_or(AuthProductError::BackendConflict)?;
        if metadata.scope != scope.resource || metadata.handle != handle {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let context = match metadata.state {
            SecretCustodyState::Original { context }
            | SecretCustodyState::RetiredOriginal { context, .. } => context,
            _ => return Err(AuthProductError::BackendConflict),
        };
        let publication: ManualPublication =
            serde_json::from_value(context).map_err(|_| AuthProductError::BackendConflict)?;
        publication.verify(scope, identity.interaction_id)?;
        let original = publication
            .flow
            .as_ref()
            .ok_or(AuthProductError::BackendConflict)?;
        if original.id != identity.flow_id {
            return Err(AuthProductError::BackendConflict);
        }
        let document = self
            .read_flow_document(scope, identity.flow_id)
            .await?
            .ok_or(AuthProductError::BackendConflict)?
            .0;
        publication.verify_flow_custody(&document)?;
        let current = document.flow;
        let mut comparable = current.clone();
        comparable.continuation_emitted_at = original.continuation_emitted_at;
        comparable.updated_at = original.updated_at;
        if &comparable != original {
            return Err(AuthProductError::BackendConflict);
        }
        // No raw acceptance, current account lookup, publication or issuer call.
        Ok((publication.result(), current))
    }

    async fn verified_manual_original(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        scope: &AuthProductScope,
        request: SecretSubmitRequest,
        metadata: SecretCustodyMetadata,
        require_flow: bool,
        expected_flow: Option<AuthFlowId>,
    ) -> Result<(SecretSubmitResult, Option<AuthFlowRecord>), AuthProductError> {
        let SecretCustodyState::Original { context } = metadata.state else {
            return Err(AuthProductError::BackendConflict);
        };
        if metadata.scope != scope.resource
            || metadata.handle != original_handle(request.interaction_id)?
        {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let publication: ManualPublication = serde_json::from_value(context.clone())
            .map_err(|_| AuthProductError::BackendConflict)?;
        publication.verify(scope, request.interaction_id)?;
        if expected_flow.is_some() && expected_flow != publication.flow.as_ref().map(|flow| flow.id)
        {
            return Err(AuthProductError::BackendConflict);
        }
        if require_flow && publication.flow.is_none() {
            return Err(AuthProductError::BackendConflict);
        }
        let proof = self
            .secret_store
            .put_original_in_transaction(
                transaction,
                scope.resource.clone(),
                metadata.handle,
                request.secret,
                context,
            )
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
            .ok_or(AuthProductError::BackendConflict)?;
        if proof.reference != metadata.reference {
            return Err(AuthProductError::BackendConflict);
        }
        let flow = match &publication.flow {
            Some(original) => {
                let row = transaction.query_opt(
                    "SELECT data FROM brassclaw_product_auth_flows WHERE tenant_id=$1 AND id=$2",
                    &[&scope.resource.tenant_id.as_str(), &original.id.to_string()],
                ).await.map_err(db_err)?.ok_or(AuthProductError::BackendConflict)?;
                let document: StoredAuthFlow = serde_json::from_value(row.get(0))
                    .map_err(|_| AuthProductError::BackendUnavailable)?;
                publication.verify_flow_custody(&document)?;
                let current = document.flow;
                // Only continuation acknowledgement may advance the published
                // flow. No account lookup can substitute its current state.
                let mut comparable = current.clone();
                comparable.continuation_emitted_at = original.continuation_emitted_at;
                comparable.updated_at = original.updated_at;
                if &comparable != original {
                    return Err(AuthProductError::BackendConflict);
                }
                Some(current)
            }
            None => None,
        };
        Ok((publication.result(), flow))
    }

    pub(super) async fn submit_manual_native(
        &self,
        scope: &AuthProductScope,
        request: SecretSubmitRequest,
        require_flow: bool,
        expected_flow: Option<AuthFlowId>,
    ) -> Result<(SecretSubmitResult, Option<AuthFlowRecord>), AuthProductError> {
        validate_secret(&request)?;
        let original = original_handle(request.interaction_id)?;
        let mut client = self.pool.get().await.map_err(pool_err)?;
        let transaction = client.transaction().await.map_err(db_err)?;
        if let Some(metadata) = self
            .secret_store
            .custody_in_transaction(&transaction, &scope.resource, &original)
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
        {
            let result = self
                .verified_manual_original(
                    &transaction,
                    scope,
                    request,
                    metadata,
                    require_flow,
                    expected_flow,
                )
                .await?;
            transaction.commit().await.map_err(db_err)?;
            return Ok(result);
        }
        // Match callback publication's lock order: flow before account. The
        // interaction row serializes independent owners, not a process mutex.
        let mut flow = manual_flow(&transaction, scope, request.interaction_id).await?;
        if expected_flow.is_some()
            && expected_flow != flow.as_ref().map(|(document, _)| document.flow.id)
        {
            return Err(AuthProductError::BackendConflict);
        }
        if require_flow && flow.is_none() {
            return Err(AuthProductError::UnknownOrExpiredFlow);
        }
        let row = transaction
            .query_opt(
                "SELECT data,revision FROM brassclaw_product_auth_interactions
             WHERE tenant_id=$1 AND id=$2 FOR UPDATE",
                &[
                    &scope.resource.tenant_id.as_str(),
                    &request.interaction_id.to_string(),
                ],
            )
            .await
            .map_err(db_err)?;
        // A competing successful transaction may have committed while this
        // caller waited. Read its original before consumed/expiry decisions.
        if let Some(metadata) = self
            .secret_store
            .custody_in_transaction(&transaction, &scope.resource, &original)
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
        {
            let result = self
                .verified_manual_original(
                    &transaction,
                    scope,
                    request,
                    metadata,
                    require_flow,
                    expected_flow,
                )
                .await?;
            transaction.commit().await.map_err(db_err)?;
            return Ok(result);
        }
        if let Some((document, _)) = &flow {
            match document.flow.status {
                // Legacy completed flows have no raw-input proof. Do not enter
                // the facade's latest-account recovery on unverified input.
                AuthFlowStatus::Completed => return Err(AuthProductError::BackendConflict),
                AuthFlowStatus::Canceled => return Err(AuthProductError::Canceled),
                AuthFlowStatus::Expired => return Err(AuthProductError::UnknownOrExpiredFlow),
                AuthFlowStatus::Failed => return Err(AuthProductError::FlowAlreadyTerminal),
                _ => {}
            }
        }
        let row = row.ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        let mut pending: StoredManualTokenInteraction =
            serde_json::from_value(row.get(0)).map_err(|_| AuthProductError::BackendUnavailable)?;
        let revision: i64 = row.get(1);
        if pending.id != request.interaction_id || !scope_matches(scope, &pending.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if pending.requirement_setup.is_some() && !require_flow {
            // A bare interaction cannot establish original completed-flow
            // custody or publish the usage/account selection independently.
            return Err(AuthProductError::BackendConflict);
        }
        let now = Utc::now();
        if pending.setup_flow_id.is_some()
            && pending.setup_flow_id != flow.as_ref().map(|(document, _)| document.flow.id)
        {
            return Err(AuthProductError::BackendConflict);
        }
        if pending.canceled_at.is_some() {
            return Err(AuthProductError::Canceled);
        }
        if pending.consumed_at.is_some() || now > pending.expires_at {
            return Err(AuthProductError::UnknownOrExpiredFlow);
        }
        if pending.update_binding.is_some() && pending.update_account_snapshot.is_none() {
            // Earlier supported setups retained this original snapshot in their
            // flow envelope. Reuse that evidence; never reconstruct it from the
            // current account. Truly unproven legacy bindings remain conflicts.
            pending.update_account_snapshot = flow
                .as_ref()
                .and_then(|(document, _)| document.auth_custody.update_account_snapshot.clone());
        }
        let selected = selected_account(&transaction, &pending).await?;
        let expected_account = selected.as_ref().map(|(account, _)| account.clone());
        let account = prepare_account(&pending, expected_account.as_ref(), now)?;
        let target = access_handle(account.id, pending.id)?;
        if let Some((document, _)) = &mut flow {
            if document.flow.provider != pending.provider
                || document.flow.continuation != pending.continuation
                || document.flow.update_binding != pending.update_binding
                || document.auth_custody.update_account_snapshot != pending.update_account_snapshot
                || document.auth_custody.requirement_setup != pending.requirement_setup
            {
                return Err(AuthProductError::BackendConflict);
            }
            validate_manual_token_flow(
                &mut document.flow,
                scope,
                &brassclaw_auth::ManualTokenCompletionInput {
                    interaction_id: pending.id,
                    credential_account_id: account.id,
                },
                now,
            )?;
            if require_flow {
                document.flow.status = AuthFlowStatus::Completed;
                document.flow.error = None;
                document.flow.credential_account_id = Some(account.id);
                document.flow.updated_at = now;
            }
        }
        let publication = ManualPublication {
            kind: ORIGINAL_KIND.into(),
            interaction: pending.clone(),
            expected_account,
            account: account.clone(),
            flow: if require_flow {
                flow.as_ref().map(|(document, _)| document.flow.clone())
            } else {
                None
            },
        };
        publication.verify(scope, pending.id)?;
        self.secret_store
            .put_original_and_retained_in_transaction(
                &transaction,
                (scope.resource.clone(), original),
                request.secret,
                serde_json::to_value(&publication)
                    .map_err(|_| AuthProductError::BackendUnavailable)?,
                (account.scope.resource.clone(), target),
            )
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
            .ok_or(AuthProductError::BackendConflict)?;
        // The selection and its generation proof must observe the retained
        // output just written above, inside this same transaction.
        if require_flow && let Some((document, _)) = &mut flow {
            self.publish_requirement_in_transaction(&transaction, document, &account)
                .await?;
        }
        callback::save_account(
            &transaction,
            &account,
            selected.as_ref().map(|(_, revision)| *revision),
        )
        .await?;
        let mut consumed = pending;
        consumed.consumed_at = Some(now);
        let changed = transaction.execute(
            "UPDATE brassclaw_product_auth_interactions SET data=$3,revision=revision+1,updated_at=NOW()
             WHERE tenant_id=$1 AND id=$2 AND revision=$4",
            &[&scope.resource.tenant_id.as_str(), &consumed.id.to_string(),
                &serde_json::to_value(&consumed).map_err(|_| AuthProductError::BackendUnavailable)?, &revision],
        ).await.map_err(db_err)?;
        if changed != 1 {
            return Err(AuthProductError::BackendConflict);
        }
        if require_flow && let Some((document, revision)) = &flow {
            callback::save_flow(&transaction, document, *revision).await?;
        }
        transaction.commit().await.map_err(db_err)?;
        // Never clean a new or preceding secret after an unknown commit. Both
        // exact generations remain retained for their existing owner references.
        Ok((publication.result(), publication.flow))
    }
}

#[cfg(test)]
#[path = "pg_auth_manual_tests.rs"]
mod tests;
