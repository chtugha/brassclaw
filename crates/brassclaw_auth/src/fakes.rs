#[path = "fakes_dcr.rs"]
mod dcr;
#[path = "fakes_oauth_setup.rs"]
mod oauth_setup;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use brassclaw_host_api::SecretHandle;
use chrono::Utc;
use secrecy::{ExposeSecret, SecretString};
use subtle::ConstantTimeEq;

use crate::{
    AuthChallenge, AuthContinuationEvent, AuthFlowId, AuthFlowManager, AuthFlowRecord,
    AuthFlowRecordSource, AuthFlowStatus, AuthInteractionId, AuthInteractionService,
    AuthProductError, AuthProductScope, AuthProviderClient, CredentialAccount,
    CredentialAccountChoiceRequest, CredentialAccountId, CredentialAccountListPage,
    CredentialAccountListRequest, CredentialAccountLookupRequest, CredentialAccountMutation,
    CredentialAccountOwnerScope, CredentialAccountProjection, CredentialAccountRecordSource,
    CredentialAccountSelectionRequest, CredentialAccountService, CredentialAccountStatus,
    CredentialOwnership, CredentialRecoveryProjection, CredentialRecoveryReason,
    CredentialRecoveryRequest, CredentialRefreshReport, CredentialRefreshRequest,
    CredentialSelectionInput, CredentialSetupService, ManualTokenCompletionInput,
    ManualTokenSetupRequest, NewAuthFlow, NewCredentialAccount, OAuthCallbackClaimRequest,
    OAuthCallbackFailureInput, OAuthCallbackInput, OAuthProviderCallbackRequest,
    OAuthProviderExchange, OAuthProviderExchangeContext, OAuthProviderRefresh,
    OAuthProviderRefreshRequest, ProviderCallbackOutcome, SecretCleanupAction,
    SecretCleanupQuarantine, SecretCleanupQuarantineReason, SecretCleanupReport,
    SecretCleanupRequest, SecretCleanupService, SecretSubmitRequest, SecretSubmitResult, Timestamp,
    TurnGateAuthFlowQuery,
    cleanup::SecretCleanupAction::Deactivate,
    domain::{
        account_is_authorized_for_requester, prepare_callback_flow,
        recovery_projection_for_single_account, recovery_projection_for_unconfigured_accounts,
        update_account_from_request, validate_account_update_target,
        validate_credential_status_transition, validate_flow_update_binding,
        validate_manual_token_flow, validate_manual_token_update_binding,
        validate_new_credential_account, validate_selection_flow,
    },
    flow::credential_status_for_completed_flow,
    flow_matches_turn_gate_query,
    interaction::PendingSecretInteraction,
    provider::validate_provider_callback_request,
    scope_matches,
};

#[derive(Default)]
struct AuthState {
    flows: HashMap<AuthFlowId, AuthFlowRecord>,
    flow_custody: HashMap<AuthFlowId, crate::AuthFlowCustody>,
    interactions: HashMap<AuthInteractionId, PendingSecretInteraction>,
    accounts: HashMap<CredentialAccountId, CredentialAccount>,
    requirement_selections:
        HashMap<crate::CredentialRequirementKey, crate::CredentialRequirementSelection>,
    manual_originals: HashMap<AuthInteractionId, ManualOriginal>,
    manual_update_snapshots: HashMap<AuthInteractionId, CredentialAccount>,
    continuations: Vec<AuthContinuationEvent>,
    refresh_fails: HashSet<CredentialAccountId>,
    refresh_actions: HashMap<(String, crate::CredentialRefreshId), crate::CredentialRefreshAction>,
    refresh_races: HashMap<CredentialAccountId, (SecretHandle, SecretHandle)>,
    quarantines: HashMap<CredentialAccountId, SecretCleanupQuarantineReason>,
}

// Test-only protected memory models the real owners' authenticated original.
// Never serialize, debug, hash or project this one-shot secret material.
struct ManualOriginal {
    scope: crate::AuthProductScope,
    secret: SecretString,
    provider: crate::AuthProviderId,
    result: SecretSubmitResult,
    interaction: PendingSecretInteraction,
    flow: Option<AuthFlowRecord>,
    expected_account: Option<CredentialAccount>,
    complete_flow: bool,
    account: CredentialAccount,
    flow_custody: Option<crate::AuthFlowCustody>,
}

// Pure preparation against actual protected originals. Callers commit the
// resulting account/flow/custody/head together under the existing state mutex.
fn prepare_requirement_publication(
    state: &AuthState,
    flow: &AuthFlowRecord,
    custody: &crate::AuthFlowCustody,
    account: &CredentialAccount,
) -> Result<
    Option<(
        crate::CredentialRequirementKey,
        crate::CredentialRequirementSelection,
    )>,
    AuthProductError,
> {
    let Some(setup) = custody.requirement_setup.as_deref() else {
        if custody.requirement_selection.is_some() {
            return Err(AuthProductError::BackendConflict);
        }
        return Ok(None);
    };
    if custody.requirement_selection.is_some() {
        return Err(AuthProductError::BackendConflict);
    }
    setup.verify_flow(flow)?;
    let selected = crate::CredentialRequirementSelection::from_completed_flow(
        &flow.scope,
        setup,
        flow,
        account.clone(),
    )?;
    let key = selected.key()?;
    let current = state.requirement_selections.get(&key);
    if let Some(current) = current {
        current.verify_requested(&flow.scope, setup.requirement())?;
        let original = state
            .flows
            .get(&current.original_flow())
            .ok_or(AuthProductError::BackendConflict)?;
        let original_custody = state
            .flow_custody
            .get(&current.original_flow())
            .ok_or(AuthProductError::BackendConflict)?;
        if original_custody.requirement_selection.as_deref() != Some(current) {
            return Err(AuthProductError::BackendConflict);
        }
        current.verify_receipt(
            original,
            original_custody
                .requirement_setup
                .as_deref()
                .ok_or(AuthProductError::BackendConflict)?,
        )?;
    }
    if setup.publication(current, &selected)? != crate::CredentialRequirementPublication::Publish {
        return Err(AuthProductError::BackendConflict);
    }
    Ok(Some((key, selected)))
}

fn retained_manual_setup_original(
    state: &AuthState,
    scope: &crate::AuthProductScope,
    identity: crate::ManualTokenSetupIdentity,
    requested: Option<&ManualTokenSetupRequest>,
) -> Result<Option<AuthFlowRecord>, AuthProductError> {
    let retained = state
        .interactions
        .get(&identity.interaction_id)
        .cloned()
        .or_else(|| {
            state
                .manual_originals
                .get(&identity.interaction_id)
                .map(|o| o.interaction.clone())
        });
    if let Some(pending) = retained {
        if !scope_matches(scope, &pending.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if pending.setup_identity != Some(identity) {
            return Err(AuthProductError::BackendConflict);
        }
        let flow = state
            .flows
            .get(&identity.flow_id)
            .ok_or(AuthProductError::BackendConflict)?;
        let original = pending_setup_request(&pending);
        crate::domain::validate_original_manual_setup_flow(
            identity,
            &original,
            requested.unwrap_or(&original),
            flow,
        )?;
        if pending.setup_created_at != Some(flow.created_at) {
            return Err(AuthProductError::BackendConflict);
        }
        let expected = state
            .manual_originals
            .get(&identity.interaction_id)
            .map(|o| o.expected_account.as_ref())
            .unwrap_or_else(|| state.manual_update_snapshots.get(&identity.interaction_id));
        let custody = state
            .flow_custody
            .get(&identity.flow_id)
            .ok_or(AuthProductError::BackendConflict)?;
        if custody.update_account_snapshot.as_ref() != expected
            || custody.requirement_setup != pending.requirement_setup
        {
            return Err(AuthProductError::BackendConflict);
        }
        if pending.canceled_at.is_some() && flow.status != AuthFlowStatus::Canceled {
            return Err(AuthProductError::Canceled);
        }
        if requested.is_some()
            && flow.status == AuthFlowStatus::AwaitingUser
            && flow.expires_at <= Utc::now()
        {
            return Err(AuthProductError::UnknownOrExpiredFlow);
        }
        if flow.status == AuthFlowStatus::AwaitingUser
            && state
                .manual_originals
                .contains_key(&identity.interaction_id)
        {
            return Err(AuthProductError::BackendConflict);
        }
        if flow.status == AuthFlowStatus::Completed {
            confirmed_manual_original(state, scope, identity)?;
        }
        return Ok(Some(flow.clone()));
    }
    if state.flows.contains_key(&identity.flow_id) {
        return Err(AuthProductError::BackendConflict);
    }
    Ok(None)
}

/// In-memory fake implementation of all product-auth service ports.
///
/// This is test support, not production persistence. It intentionally models
/// important fail-closed transitions so downstream code cannot depend on unsafe
/// shortcuts while production stores are still being composed.
#[derive(Default, Clone)]
pub struct InMemoryAuthProductServices {
    state: Arc<Mutex<AuthState>>,
}

impl InMemoryAuthProductServices {
    /// Deterministic fake metadata only; this helper performs no provider/storage call.
    pub fn refresh_preparation_for_tests(
        request: &OAuthProviderRefreshRequest,
    ) -> Result<crate::OAuthProviderRefreshPreparation, AuthProductError> {
        let make = |kind: &str| {
            SecretHandle::new(format!(
                "fake-oauth-{kind}-{}-{}",
                request.account_id, request.refresh_id
            ))
            .map_err(|_| AuthProductError::BackendUnavailable)
        };
        // Synthetic test-service metadata, never a native-store generation proof.
        Ok(crate::OAuthProviderRefreshPreparation {
            destinations: crate::OAuthTokenDestinations {
                access_secret: make("access")?,
                refresh_secret: make("refresh")?,
            },
            source: crate::OAuthRefreshSource {
                scope: request.scope.resource.clone(),
                handle: request.refresh_secret.clone(),
                record_reference: brassclaw_common::hashing::sha256_hex(
                    request.refresh_secret.as_str().as_bytes(),
                ),
            },
        })
    }

    pub fn callback_destinations_for_tests(
        context: &OAuthProviderExchangeContext,
    ) -> Result<crate::OAuthTokenDestinations, AuthProductError> {
        let make = |kind: &str| {
            SecretHandle::new(format!(
                "fake-oauth-{kind}-{}-{}",
                context.flow_id, context.scope.resource.invocation_id
            ))
            .map_err(|_| AuthProductError::BackendUnavailable)
        };
        Ok(crate::OAuthTokenDestinations {
            access_secret: make("access")?,
            refresh_secret: make("refresh")?,
        })
    }

    /// Test adapter for the same atomic metadata pair used by the native owner.
    /// It never calls an issuer or stores a serializable raw-token record.
    pub fn request_manual_setup_original_for_tests(
        &self,
        identity: crate::ManualTokenSetupIdentity,
        request: ManualTokenSetupRequest,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let mut state = self.lock_state();
        if let Some(original) =
            retained_manual_setup_original(&state, &request.scope, identity, Some(&request))?
        {
            return Ok(original);
        }
        let now = Utc::now();
        let flow = crate::domain::build_original_manual_setup_flow(identity, &request, now)?;
        let snapshot = if let Some(binding) = &request.update_binding {
            let account = state
                .accounts
                .get(&binding.account_id)
                .ok_or(AuthProductError::CredentialMissing)?;
            validate_manual_token_update_binding(account, &request, binding)?;
            Some(account.clone())
        } else {
            None
        };
        let pending = PendingSecretInteraction {
            requirement_setup: request.requirement_setup.clone(),
            scope: request.scope,
            provider: request.provider,
            label: request.label,
            continuation: request.continuation,
            update_binding: request.update_binding,
            expires_at: request.expires_at,
            setup_identity: Some(identity),
            setup_created_at: Some(now),
            canceled_at: None,
        };
        if let Some(account) = &snapshot {
            state
                .manual_update_snapshots
                .insert(identity.interaction_id, account.clone());
        }
        state.flow_custody.insert(
            identity.flow_id,
            crate::AuthFlowCustody {
                update_account_snapshot: snapshot,
                oauth_callback: None,
                oauth_setup: None,
                dcr_registration: None,
                dcr_legacy: None,
                requirement_setup: request.requirement_setup,
                requirement_selection: None,
                requirement_source: None,
            },
        );
        state.interactions.insert(identity.interaction_id, pending);
        state.flows.insert(identity.flow_id, flow.clone());
        Ok(flow)
    }

    /// Read only an existing original setup under the same protected test state.
    /// Never mint/recreate setup or select today's account during recovery.
    pub fn recover_manual_setup_original_for_tests(
        &self,
        scope: &crate::AuthProductScope,
        identity: crate::ManualTokenSetupIdentity,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
        retained_manual_setup_original(&self.lock_state(), scope, identity, None)
    }

    /// Read a confirmed original result without accepting a raw token again.
    pub fn continue_manual_setup_original_for_tests(
        &self,
        scope: &crate::AuthProductScope,
        identity: crate::ManualTokenSetupIdentity,
    ) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError> {
        let state = self.lock_state();
        confirmed_manual_original(&state, scope, identity)
    }

    /// Whole-flow fake publication shares its owning state lock with the lower
    /// primitive; the lower operation itself still does not complete a flow.
    pub fn submit_manual_token_flow_for_tests(
        &self,
        scope: &crate::AuthProductScope,
        request: SecretSubmitRequest,
    ) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError> {
        self.submit_manual_token_flow_with_expected(scope, request, None)
    }
    /// Original pair mismatches fail under the same lock, before account writes.
    pub fn submit_original_manual_token_flow_for_tests(
        &self,
        scope: &crate::AuthProductScope,
        identity: crate::ManualTokenSetupIdentity,
        secret: SecretString,
    ) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError> {
        self.submit_manual_token_flow_with_expected(
            scope,
            SecretSubmitRequest {
                interaction_id: identity.interaction_id,
                secret,
            },
            Some(identity.flow_id),
        )
    }
    fn submit_manual_token_flow_with_expected(
        &self,
        scope: &crate::AuthProductScope,
        request: SecretSubmitRequest,
        expected_flow: Option<AuthFlowId>,
    ) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError> {
        request.validate_secret()?;
        let id = request.interaction_id;
        let mut state = self.lock_state();
        let result = submit_manual_token_in_state(
            &mut state,
            scope,
            request,
            true,
            expected_flow,
            Utc::now(),
        )?;
        let selected = state
            .manual_originals
            .get(&id)
            .and_then(|o| o.flow.as_ref())
            .ok_or(AuthProductError::BackendConflict)?;
        let flow = state
            .flows
            .get(&selected.id)
            .cloned()
            .ok_or(AuthProductError::BackendConflict)?;
        Ok((result, flow))
    }

    pub fn new() -> Self {
        Self::default()
    }

    pub fn continuations(&self) -> Vec<AuthContinuationEvent> {
        self.lock_state().continuations.clone()
    }

    pub fn fail_next_refresh_for_tests(&self, account_id: CredentialAccountId) {
        self.lock_state().refresh_fails.insert(account_id);
    }

    pub fn complete_refresh_during_next_provider_call_for_tests(
        &self,
        account_id: CredentialAccountId,
        access_secret: SecretHandle,
        refresh_secret: SecretHandle,
    ) {
        self.lock_state()
            .refresh_races
            .insert(account_id, (access_secret, refresh_secret));
    }

    pub fn quarantine_cleanup_for_tests(
        &self,
        account_id: CredentialAccountId,
        reason: SecretCleanupQuarantineReason,
    ) {
        self.lock_state().quarantines.insert(account_id, reason);
    }

    pub fn flow_records_snapshot(&self) -> Vec<AuthFlowRecord> {
        let mut flows = self
            .lock_state()
            .flows
            .values()
            .cloned()
            .collect::<Vec<_>>();
        flows.sort_by_key(|flow| flow.id.as_uuid());
        flows
    }

    fn lock_state(&self) -> MutexGuard<'_, AuthState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[async_trait]
impl AuthFlowRecordSource for InMemoryAuthProductServices {
    async fn flow_for_turn_gate(
        &self,
        query: TurnGateAuthFlowQuery,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
        let state = self.lock_state();
        Ok(state
            .flows
            .values()
            .find(|flow| flow_matches_turn_gate_query(flow, &query))
            .cloned())
    }

    async fn flows_for_owner(
        &self,
        owner: crate::AuthFlowOwnerScope,
    ) -> Result<Vec<AuthFlowRecord>, AuthProductError> {
        let state = self.lock_state();
        let mut flows = state
            .flows
            .values()
            .filter(|flow| owner.matches(flow))
            .cloned()
            .collect::<Vec<_>>();
        flows.sort_by_key(|flow| flow.id);
        Ok(flows)
    }
}

#[async_trait]
impl AuthFlowManager for InMemoryAuthProductServices {
    async fn requirement_setup(
        &self,
        scope: &AuthProductScope,
        flow: AuthFlowId,
    ) -> Result<Option<crate::CredentialRequirementSetup>, AuthProductError> {
        let state = self.lock_state();
        let original = state
            .flows
            .get(&flow)
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        if !scope_matches(scope, &original.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let setup = state
            .flow_custody
            .get(&flow)
            .and_then(|custody| custody.requirement_setup.as_deref())
            .cloned();
        if let Some(setup) = &setup {
            setup
                .verify_flow(original)
                .map_err(|_| AuthProductError::BackendUnavailable)?;
        }
        Ok(setup)
    }

    async fn claim_dcr_cleanup(
        &self,
        prepared: crate::DcrCleanupPrepared,
    ) -> Result<crate::DcrCleanupAdmission, AuthProductError> {
        self.claim_dcr_cleanup_owned(prepared).await
    }
    async fn record_dcr_cleanup(
        &self,
        scope: &crate::AuthProductScope,
        completion: crate::DcrCleanupCompletion,
    ) -> Result<crate::DcrRegistrationState, AuthProductError> {
        self.record_dcr_cleanup_owned(scope, completion).await
    }
    async fn update_dcr_retirement(
        &self,
        original: &crate::DcrRegistrationOriginal,
        update: crate::DcrRetirementUpdate,
    ) -> Result<crate::DcrRegistrationState, crate::AuthProductError> {
        self.update_dcr_retirement_owned(original, update).await
    }

    async fn retain_legacy_dcr(
        &self,
        selection: crate::DcrLegacySelection,
    ) -> Result<crate::DcrLegacyState, crate::AuthProductError> {
        self.retain_legacy_dcr_owned(selection).await
    }

    async fn mark_dcr_cleanup_unavailable(
        &self,
        original: &crate::DcrRegistrationOriginal,
        issue: crate::DcrCleanupUnavailable,
    ) -> Result<crate::DcrRegistrationState, AuthProductError> {
        self.mark_dcr_cleanup_unavailable_owned(original, issue)
            .await
    }

    async fn prepare_oauth_setup(
        &self,
        original: crate::OAuthSetupOriginal,
        retirement: Option<crate::OAuthSetupRetirement>,
    ) -> Result<crate::OAuthSetupState, crate::AuthProductError> {
        self.prepare_oauth_setup_owned(original, retirement).await
    }

    async fn read_oauth_setup(
        &self,
        scope: &crate::AuthProductScope,
        flow_id: crate::AuthFlowId,
    ) -> Result<Option<crate::OAuthSetupState>, crate::AuthProductError> {
        self.read_oauth_setup_owned(scope, flow_id).await
    }

    async fn prepare_dcr_registration(
        &self,
        original: crate::DcrRegistrationOriginal,
    ) -> Result<crate::DcrRegistrationState, AuthProductError> {
        self.prepare_dcr_original(original).await
    }

    async fn retire_dcr_preparation(
        &self,
        original: crate::DcrRegistrationOriginal,
        reason: crate::DcrCleanupReason,
    ) -> Result<crate::DcrRegistrationState, crate::AuthProductError> {
        self.retire_dcr_preparation_owned(original, reason).await
    }
    async fn read_dcr_registration(
        &self,
        scope: &crate::AuthProductScope,
        id: AuthFlowId,
    ) -> Result<Option<crate::DcrRegistrationState>, AuthProductError> {
        self.read_dcr_original(scope, id).await
    }
    async fn claim_dcr_registration(
        &self,
        original: &crate::DcrRegistrationOriginal,
    ) -> Result<crate::DcrRegistrationAdmission, AuthProductError> {
        self.claim_dcr_original(original).await
    }
    async fn record_dcr_registration(
        &self,
        scope: &crate::AuthProductScope,
        completion: crate::DcrRegistrationCompletion,
    ) -> Result<crate::DcrRegistrationState, AuthProductError> {
        self.record_dcr_original(scope, completion).await
    }
    async fn publish_dcr_challenge(
        &self,
        original: &crate::DcrRegistrationOriginal,
        challenge: crate::DcrRegistrationChallenge,
    ) -> Result<crate::DcrRegistrationState, AuthProductError> {
        self.publish_dcr_original(original, challenge).await
    }

    async fn create_flow(&self, request: NewAuthFlow) -> Result<AuthFlowRecord, AuthProductError> {
        let mut state = self.lock_state();
        let snapshot = if let Some(binding) = &request.update_binding {
            let account = state
                .accounts
                .get(&binding.account_id)
                .ok_or(AuthProductError::CredentialMissing)?;
            validate_flow_update_binding(account, &request)?;
            Some(account.clone())
        } else {
            None
        };
        let id = request.id.unwrap_or_default();
        if state.flows.contains_key(&id) {
            return Err(AuthProductError::BackendUnavailable);
        }
        let now = Utc::now();
        let record = AuthFlowRecord {
            id,
            scope: request.scope,
            kind: request.kind,
            status: AuthFlowStatus::AwaitingUser,
            provider: request.provider,
            challenge: Some(request.challenge),
            continuation: request.continuation,
            credential_account_id: None,
            update_binding: request.update_binding,
            opaque_state_hash: request.opaque_state_hash,
            pkce_verifier_hash: request.pkce_verifier_hash,
            authorization_code_hash: None,
            error: None,
            continuation_emitted_at: None,
            created_at: now,
            updated_at: now,
            expires_at: request.expires_at,
        };
        if let Some(setup) = &request.requirement_setup {
            setup.verify_flow(&record)?;
        }
        state.flow_custody.insert(
            record.id,
            crate::AuthFlowCustody {
                update_account_snapshot: snapshot,
                oauth_callback: None,
                oauth_setup: None,
                dcr_registration: None,
                dcr_legacy: None,
                requirement_setup: request.requirement_setup,
                requirement_selection: None,
                requirement_source: None,
            },
        );
        state.flows.insert(record.id, record.clone());
        Ok(record)
    }

    async fn get_flow(
        &self,
        scope: &crate::AuthProductScope,
        flow_id: AuthFlowId,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
        let state = self.lock_state();
        let Some(record) = state.flows.get(&flow_id) else {
            return Ok(None);
        };
        if !scope_matches(scope, &record.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        Ok(Some(record.clone()))
    }

    async fn read_oauth_callback_original(
        &self,
        scope: &crate::AuthProductScope,
        reference: &crate::OAuthCallbackReference,
    ) -> Result<Option<crate::OAuthCallbackState>, AuthProductError> {
        if !scope_matches(scope, &reference.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let state = self.lock_state();
        let Some(flow) = state.flows.get(&reference.flow_id) else {
            return Ok(None);
        };
        state
            .flow_custody
            .get(&reference.flow_id)
            .cloned()
            .unwrap_or_default()
            .original_state(flow, reference)
    }
    async fn claim_oauth_callback(
        &self,
        scope: &crate::AuthProductScope,
        request: OAuthCallbackClaimRequest,
    ) -> Result<crate::OAuthCallbackAdmission, AuthProductError> {
        if !scope_matches(scope, &request.original.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let mut state = self.lock_state();
        let AuthState {
            flows,
            flow_custody,
            accounts,
            ..
        } = &mut *state;
        let flow = flows
            .get_mut(&request.original.flow_id)
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        let target = flow
            .update_binding
            .as_ref()
            .map(|binding| binding.account_id)
            .unwrap_or_else(|| CredentialAccountId::from_uuid(flow.id.as_uuid()));
        let current = accounts.get(&target);
        flow_custody
            .entry(flow.id)
            .or_default()
            .admit(flow, current, request, Utc::now())
    }
    async fn reconcile_oauth_callback(
        &self,
        scope: &crate::AuthProductScope,
        reference: &crate::OAuthCallbackReference,
    ) -> Result<crate::OAuthCallbackState, AuthProductError> {
        if !scope_matches(scope, &reference.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let mut state = self.lock_state();
        let mut flow = state
            .flows
            .get(&reference.flow_id)
            .cloned()
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        let mut custody = state
            .flow_custody
            .get(&flow.id)
            .cloned()
            .ok_or(AuthProductError::BackendUnavailable)?;
        let original = custody
            .original_state(&flow, reference)?
            .ok_or(AuthProductError::BackendUnavailable)?;
        if !original.action.needs_publication() {
            return Ok(original);
        }
        let action = custody
            .oauth_callback
            .as_mut()
            .ok_or(AuthProductError::BackendUnavailable)?;
        let now = Utc::now();
        let account = action.publish(
            &mut flow,
            state.accounts.get(&action.target_account_id()),
            now,
        )?;
        let result = crate::OAuthCallbackState {
            flow: flow.clone(),
            action: action.clone(),
        };
        let publication = match &account {
            Some(account) => prepare_requirement_publication(&state, &flow, &custody, account)?,
            None => None,
        };
        // No fallible lookup/check remains after the first mutation. A losing
        // head cannot leak a completed flow, account or successful continuation.
        if let Some((key, selected)) = publication {
            custody.requirement_selection = Some(Box::new(selected.clone()));
            state.requirement_selections.insert(key, selected);
        }
        if let Some(account) = account {
            state.accounts.insert(account.id, account);
        }
        state.flows.insert(flow.id, flow.clone());
        state.flow_custody.insert(flow.id, custody);
        if flow.status == AuthFlowStatus::Completed {
            state.continuations.push(AuthContinuationEvent {
                flow_id: flow.id,
                scope: flow.scope.clone(),
                continuation: flow.continuation.clone(),
                credential_account_id: flow.credential_account_id,
                emitted_at: now,
            });
        }
        Ok(result)
    }

    async fn complete_oauth_callback(
        &self,
        scope: &crate::AuthProductScope,
        input: OAuthCallbackInput,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let reference = {
            let mut state = self.lock_state();
            let AuthState {
                flows,
                flow_custody,
                ..
            } = &mut *state;
            let flow = flows
                .get_mut(&input.flow_id)
                .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
            if !scope_matches(scope, &flow.scope)
                || !flow
                    .opaque_state_hash
                    .as_ref()
                    .is_some_and(|hash| hash.constant_time_eq(&input.opaque_state_hash))
            {
                return Err(AuthProductError::CrossScopeDenied);
            }
            match input.outcome {
                ProviderCallbackOutcome::Denied => {
                    if input.dispatched.is_some() {
                        return Err(AuthProductError::MalformedCallback);
                    }
                    prepare_callback_flow(flow, scope, &input.opaque_state_hash, Utc::now())?;
                    if flow_custody
                        .get(&flow.id)
                        .is_some_and(|custody| custody.oauth_callback.is_some())
                    {
                        return Err(AuthProductError::BackendConflict);
                    }
                    if !matches!(flow.challenge, Some(AuthChallenge::OAuthUrl { .. })) {
                        return Err(AuthProductError::MalformedCallback);
                    }
                    flow.status = AuthFlowStatus::Failed;
                    flow.error = Some(crate::AuthErrorCode::ProviderDenied);
                    flow.updated_at = Utc::now();
                    return Err(AuthProductError::ProviderDenied);
                }
                ProviderCallbackOutcome::Authorized { exchange } => {
                    let dispatched = input
                        .dispatched
                        .ok_or(AuthProductError::BackendUnavailable)?;
                    if dispatched.original.flow_id != input.flow_id
                        || !scope_matches(scope, &dispatched.original.scope)
                        || !dispatched
                            .original
                            .opaque_state_hash
                            .constant_time_eq(&input.opaque_state_hash)
                    {
                        return Err(AuthProductError::CrossScopeDenied);
                    }
                    let custody = flow_custody
                        .get_mut(&flow.id)
                        .ok_or(AuthProductError::BackendUnavailable)?;
                    custody
                        .original_state(flow, &dispatched.original.reference())?
                        .ok_or(AuthProductError::BackendUnavailable)?;
                    let action = custody
                        .oauth_callback
                        .as_mut()
                        .ok_or(AuthProductError::BackendUnavailable)?;
                    action.record_result(
                        crate::OAuthCallbackCompletion {
                            dispatched,
                            outcome: crate::OAuthCallbackOutcome::Returned(exchange),
                        },
                        Utc::now(),
                    )?;
                    if flow.status == AuthFlowStatus::CallbackReceived {
                        flow.status = AuthFlowStatus::Completing;
                        flow.updated_at = Utc::now();
                    }
                    action.original.reference()
                }
            }
        };
        self.reconcile_oauth_callback(scope, &reference)
            .await?
            .completed_flow()
    }

    async fn complete_credential_selection(
        &self,
        scope: &crate::AuthProductScope,
        input: CredentialSelectionInput,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let now = Utc::now();
        let mut state = self.lock_state();
        let mut record = state
            .flows
            .get(&input.flow_id)
            .cloned()
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        match validate_selection_flow(&mut record, scope, &input, now) {
            Ok(()) => {}
            Err(AuthProductError::UnknownOrExpiredFlow) => {
                state.flows.insert(record.id, record);
                return Err(AuthProductError::UnknownOrExpiredFlow);
            }
            Err(error) => return Err(error),
        }
        let mut custody = state
            .flow_custody
            .get(&record.id)
            .cloned()
            .ok_or(AuthProductError::BackendUnavailable)?;
        if record.status == AuthFlowStatus::Completed {
            if let Some(selected) = &custody.requirement_selection {
                selected.verify_receipt(
                    &record,
                    custody
                        .requirement_setup
                        .as_deref()
                        .ok_or(AuthProductError::BackendConflict)?,
                )?;
            }
            return Ok(record);
        }
        if custody.oauth_callback.is_some() {
            return Err(AuthProductError::BackendConflict);
        }
        let account = state
            .accounts
            .get(&input.credential_account_id)
            .ok_or(AuthProductError::CredentialMissing)?;
        if !scope_matches(&record.scope, &account.scope) || account.provider != record.provider {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if account.effective_status_at(now) != CredentialAccountStatus::Configured {
            return Err(AuthProductError::CredentialMissing);
        }
        record.status = AuthFlowStatus::Completed;
        record.error = None;
        record.credential_account_id = Some(input.credential_account_id);
        record.updated_at = now;
        let publication = prepare_requirement_publication(&state, &record, &custody, account)?;
        if let Some((key, selected)) = publication {
            custody.requirement_selection = Some(Box::new(selected.clone()));
            state.requirement_selections.insert(key, selected);
        }
        state.flows.insert(record.id, record.clone());
        state.flow_custody.insert(record.id, custody);
        state.continuations.push(AuthContinuationEvent {
            flow_id: record.id,
            scope: record.scope.clone(),
            continuation: record.continuation.clone(),
            credential_account_id: record.credential_account_id,
            emitted_at: now,
        });
        Ok(record)
    }

    async fn complete_manual_token(
        &self,
        scope: &crate::AuthProductScope,
        input: ManualTokenCompletionInput,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let now = Utc::now();
        let mut state = self.lock_state();
        let flow_id = state
            .flows
            .iter()
            .find_map(|(flow_id, record)| {
                let matches_interaction = matches!(
                    &record.challenge,
                    Some(AuthChallenge::ManualTokenRequired { interaction_id, .. })
                        if interaction_id == &input.interaction_id
                );
                matches_interaction.then_some(*flow_id)
            })
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        let (flow_scope, flow_provider) = {
            let record = state
                .flows
                .get_mut(&flow_id)
                .ok_or(AuthProductError::BackendUnavailable)?;
            validate_manual_token_flow(record, scope, &input, now)?;
            if record.status == AuthFlowStatus::Completed {
                return confirmed_manual_original(
                    &state,
                    scope,
                    crate::ManualTokenSetupIdentity {
                        interaction_id: input.interaction_id,
                        flow_id,
                    },
                )
                .map(|(_, flow)| flow);
            }
            (record.scope.clone(), record.provider.clone())
        };
        let original = state
            .manual_originals
            .get(&input.interaction_id)
            .ok_or(AuthProductError::BackendConflict)?;
        if !scope_matches(&flow_scope, &original.scope)
            || original.provider != flow_provider
            || original.result.account_id != input.credential_account_id
            || original.flow.as_ref().map(|flow| flow.id) != Some(flow_id)
        {
            return Err(AuthProductError::BackendConflict);
        }
        let record = state
            .flows
            .get_mut(&flow_id)
            .ok_or(AuthProductError::BackendUnavailable)?;
        record.status = AuthFlowStatus::Completed;
        record.error = None;
        record.credential_account_id = Some(input.credential_account_id);
        record.updated_at = now;
        let completed = record.clone();
        state.continuations.push(AuthContinuationEvent {
            flow_id: completed.id,
            scope: completed.scope.clone(),
            continuation: completed.continuation.clone(),
            credential_account_id: completed.credential_account_id,
            emitted_at: now,
        });
        Ok(completed)
    }

    async fn cancel_manual_token(
        &self,
        scope: &crate::AuthProductScope,
        interaction_id: AuthInteractionId,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
        let now = Utc::now();
        let mut state = self.lock_state();
        let Some(flow_id) = state.flows.iter().find_map(|(flow_id, record)| {
            let matches_interaction = matches!(
                &record.challenge,
                Some(AuthChallenge::ManualTokenRequired { interaction_id: id, .. })
                    if id == &interaction_id
            );
            matches_interaction.then_some(*flow_id)
        }) else {
            return Ok(None);
        };
        let record = state
            .flows
            .get_mut(&flow_id)
            .ok_or(AuthProductError::BackendUnavailable)?;
        if !scope_matches(scope, &record.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if !crate::is_terminal_status(record.status) {
            record.status = AuthFlowStatus::Canceled;
            record.error = Some(crate::AuthErrorCode::Canceled);
            record.updated_at = now;
        }
        Ok(Some(record.clone()))
    }

    async fn fail_oauth_callback(
        &self,
        scope: &crate::AuthProductScope,
        input: OAuthCallbackFailureInput,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        if let Some(dispatched) = input.dispatched {
            let reference = dispatched.original.reference();
            if reference.flow_id != input.flow_id
                || !scope_matches(scope, &reference.scope)
                || !reference
                    .opaque_state_hash
                    .constant_time_eq(&input.opaque_state_hash)
            {
                return Err(AuthProductError::CrossScopeDenied);
            }
            {
                let mut state = self.lock_state();
                let AuthState {
                    flows,
                    flow_custody,
                    ..
                } = &mut *state;
                let flow = flows
                    .get_mut(&input.flow_id)
                    .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
                let custody = flow_custody
                    .get_mut(&flow.id)
                    .ok_or(AuthProductError::BackendUnavailable)?;
                custody
                    .original_state(flow, &reference)?
                    .ok_or(AuthProductError::BackendUnavailable)?;
                let action = custody
                    .oauth_callback
                    .as_mut()
                    .ok_or(AuthProductError::BackendUnavailable)?;
                action.record_result(
                    crate::OAuthCallbackCompletion {
                        dispatched,
                        outcome: crate::OAuthCallbackOutcome::Unknown(input.error),
                    },
                    Utc::now(),
                )?;
                if flow.status == AuthFlowStatus::CallbackReceived {
                    flow.status = AuthFlowStatus::Completing;
                    flow.updated_at = Utc::now();
                }
            }
            return Ok(self.reconcile_oauth_callback(scope, &reference).await?.flow);
        }
        let mut state = self.lock_state();
        let AuthState {
            flows,
            flow_custody,
            ..
        } = &mut *state;
        let flow = flows
            .get_mut(&input.flow_id)
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        prepare_callback_flow(flow, scope, &input.opaque_state_hash, Utc::now())?;
        if flow_custody
            .get(&flow.id)
            .is_some_and(|custody| custody.oauth_callback.is_some())
        {
            return Err(AuthProductError::BackendConflict);
        }
        if !matches!(flow.challenge, Some(AuthChallenge::OAuthUrl { .. })) {
            return Err(AuthProductError::MalformedCallback);
        }
        flow.status = AuthFlowStatus::Failed;
        flow.error = Some(input.error);
        flow.updated_at = Utc::now();
        Ok(flow.clone())
    }

    async fn cancel_flow(
        &self,
        scope: &crate::AuthProductScope,
        flow_id: AuthFlowId,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let now = Utc::now();
        let mut state = self.lock_state();
        let mut record = state
            .flows
            .get(&flow_id)
            .cloned()
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        if !scope_matches(scope, &record.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if crate::is_terminal_status(record.status) {
            return Err(if record.status == AuthFlowStatus::Canceled {
                AuthProductError::Canceled
            } else {
                AuthProductError::FlowAlreadyTerminal
            });
        }
        record.status = AuthFlowStatus::Canceled;
        record.error = Some(crate::AuthErrorCode::Canceled);
        record.updated_at = now;
        if let Some(mut custody) = state.flow_custody.get(&flow_id).cloned() {
            custody.require_dcr_cleanup(&mut record, now)?;
            state.flow_custody.insert(flow_id, custody);
        }
        state.flows.insert(flow_id, record.clone());
        Ok(record)
    }

    async fn mark_continuation_dispatched(
        &self,
        scope: &crate::AuthProductScope,
        flow_id: AuthFlowId,
        emitted_at: Timestamp,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let mut state = self.lock_state();
        let record = state
            .flows
            .get_mut(&flow_id)
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        if !scope_matches(scope, &record.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if record.status != AuthFlowStatus::Completed {
            return Err(AuthProductError::FlowAlreadyTerminal);
        }
        // Idempotent: if already marked by a concurrent caller, return existing record.
        if record.continuation_emitted_at.is_some() {
            return Ok(record.clone());
        }
        record.continuation_emitted_at = Some(emitted_at);
        record.updated_at = emitted_at;
        Ok(record.clone())
    }
}

#[async_trait]
impl CredentialAccountService for InMemoryAuthProductServices {
    async fn create_account(
        &self,
        request: NewCredentialAccount,
    ) -> Result<CredentialAccount, AuthProductError> {
        create_account_in_state(&mut self.lock_state(), request)
    }

    async fn get_account(
        &self,
        request: CredentialAccountLookupRequest,
    ) -> Result<Option<CredentialAccount>, AuthProductError> {
        let state = self.lock_state();
        let Some(account) = state.accounts.get(&request.account_id) else {
            return Ok(None);
        };
        if !scope_matches(&request.scope, &account.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if !account_is_authorized_for_requester(account, request.requester_extension.as_ref()) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        Ok(Some(account.clone()))
    }

    async fn list_accounts(
        &self,
        request: CredentialAccountListRequest,
    ) -> Result<CredentialAccountListPage, AuthProductError> {
        request.validate()?;
        let mut accounts = self
            .lock_state()
            .accounts
            .values()
            .filter(|account| {
                scope_matches(&request.scope, &account.scope)
                    && account.provider == request.provider
                    && request.cursor.is_none_or(|cursor| account.id > cursor)
                    && account_is_authorized_for_requester(
                        account,
                        request.requester_extension.as_ref(),
                    )
            })
            .map(CredentialAccount::projection)
            .collect::<Vec<_>>();
        accounts.sort_by_key(|account| account.id);
        let next_cursor = if accounts.len() > request.limit {
            accounts.truncate(request.limit);
            accounts.last().map(|account| account.id)
        } else {
            None
        };
        Ok(CredentialAccountListPage {
            accounts,
            next_cursor,
        })
    }

    async fn update_status(
        &self,
        scope: &crate::AuthProductScope,
        account_id: CredentialAccountId,
        status: CredentialAccountStatus,
    ) -> Result<CredentialAccount, AuthProductError> {
        let now = Utc::now();
        let mut state = self.lock_state();
        let account = state
            .accounts
            .get_mut(&account_id)
            .ok_or(AuthProductError::CredentialMissing)?;
        if !scope_matches(scope, &account.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        validate_credential_status_transition(account.status, status)?;
        account.status = status;
        account.updated_at = now;
        Ok(account.clone())
    }

    async fn select_unique_configured_account(
        &self,
        request: CredentialAccountSelectionRequest,
    ) -> Result<CredentialAccountProjection, AuthProductError> {
        let state = self.lock_state();
        let configured = state
            .accounts
            .values()
            .filter(|account| {
                scope_matches(&request.scope, &account.scope)
                    && account.provider == request.provider
                    && account.effective_status() == CredentialAccountStatus::Configured
            })
            .collect::<Vec<_>>();
        if configured.is_empty() {
            return Err(AuthProductError::CredentialMissing);
        }
        let selectable = configured
            .iter()
            .copied()
            .filter(|account| {
                account_is_authorized_for_requester(account, request.requester_extension.as_ref())
            })
            .collect::<Vec<_>>();
        match selectable.as_slice() {
            [] => Err(AuthProductError::CrossScopeDenied),
            [account] => Ok(account.projection()),
            _ => Err(AuthProductError::AccountSelectionRequired),
        }
    }

    async fn project_credential_recovery(
        &self,
        request: CredentialRecoveryRequest,
    ) -> Result<CredentialRecoveryProjection, AuthProductError> {
        let state = self.lock_state();
        let mut accounts = state
            .accounts
            .values()
            .filter(|account| {
                scope_matches(&request.scope, &account.scope)
                    && account.provider == request.provider
            })
            .collect::<Vec<_>>();
        accounts.sort_by_key(|account| account.id);

        if accounts.is_empty() {
            return Ok(CredentialRecoveryProjection::setup_required(
                request.provider,
                CredentialRecoveryReason::NoAccount,
                Vec::new(),
            ));
        }

        let authorized = accounts
            .iter()
            .copied()
            .filter(|account| {
                account_is_authorized_for_requester(account, request.requester_extension.as_ref())
            })
            .collect::<Vec<_>>();
        if authorized.is_empty() {
            return Ok(CredentialRecoveryProjection::setup_required(
                request.provider,
                CredentialRecoveryReason::NoAccount,
                Vec::new(),
            ));
        }

        let configured = authorized
            .iter()
            .copied()
            .filter(|account| account.effective_status() == CredentialAccountStatus::Configured)
            .collect::<Vec<_>>();
        match configured.as_slice() {
            [account] => {
                return Ok(CredentialRecoveryProjection::configured(
                    request.provider,
                    account.projection(),
                ));
            }
            [_, ..] => {
                return Ok(CredentialRecoveryProjection::account_selection_required(
                    request.provider,
                    configured
                        .iter()
                        .map(|account| account.projection())
                        .collect(),
                ));
            }
            [] => {}
        }

        if let [account] = authorized.as_slice() {
            return Ok(recovery_projection_for_single_account(
                request.provider,
                account,
            ));
        }

        Ok(recovery_projection_for_unconfigured_accounts(
            request.provider,
            &authorized,
        ))
    }

    async fn select_configured_account(
        &self,
        request: CredentialAccountChoiceRequest,
    ) -> Result<CredentialAccountProjection, AuthProductError> {
        let state = self.lock_state();
        let account = state
            .accounts
            .get(&request.account_id)
            .ok_or(AuthProductError::CredentialMissing)?;
        if !scope_matches(&request.scope, &account.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if account.provider != request.provider {
            return Err(AuthProductError::CredentialMissing);
        }
        if account.effective_status() != CredentialAccountStatus::Configured {
            return Err(AuthProductError::CredentialMissing);
        }
        if !account_is_authorized_for_requester(account, request.requester_extension.as_ref()) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        Ok(account.projection())
    }

    async fn refresh_account(
        &self,
        request: CredentialRefreshRequest,
    ) -> Result<CredentialRefreshReport, AuthProductError> {
        let shared = Arc::new(self.clone());
        crate::ProviderBackedCredentialAccountService::new(shared.clone(), shared.clone(), shared)
            .refresh_account(request)
            .await
    }
}

#[async_trait]
impl CredentialAccountRecordSource for InMemoryAuthProductServices {
    async fn account_record(
        &self,
        scope: &AuthProductScope,
        account: CredentialAccountId,
    ) -> Result<Option<CredentialAccount>, AuthProductError> {
        let record = self.lock_state().accounts.get(&account).cloned();
        if record
            .as_ref()
            .is_some_and(|record| !scope_matches(scope, &record.scope))
        {
            return Err(AuthProductError::CrossScopeDenied);
        }
        Ok(record)
    }

    async fn requirement_selection(
        &self,
        scope: &AuthProductScope,
        requirement: &brassclaw_host_api::RuntimeCredentialAccountRequirement,
    ) -> Result<Option<crate::CredentialRequirementSelection>, AuthProductError> {
        let key =
            crate::CredentialRequirementOwner::from_scope(&scope.resource).key(requirement)?;
        let state = self.lock_state();
        let selected = state.requirement_selections.get(&key).cloned();
        if let Some(selected) = &selected {
            selected
                .verify_requested(scope, requirement)
                .map_err(|_| AuthProductError::BackendUnavailable)?;
            let flow = state
                .flows
                .get(&selected.original_flow())
                .ok_or(AuthProductError::BackendUnavailable)?;
            let custody = state
                .flow_custody
                .get(&selected.original_flow())
                .ok_or(AuthProductError::BackendUnavailable)?;
            let setup = custody
                .requirement_setup
                .as_deref()
                .ok_or(AuthProductError::BackendUnavailable)?;
            if custody.requirement_selection.as_deref() != Some(selected) {
                return Err(AuthProductError::BackendUnavailable);
            }
            selected
                .verify_receipt(flow, setup)
                .map_err(|_| AuthProductError::BackendUnavailable)?;
        }
        Ok(selected)
    }

    async fn requirement_selection_for_flow(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
        requirement: &brassclaw_host_api::RuntimeCredentialAccountRequirement,
    ) -> Result<Option<crate::CredentialRequirementSelection>, AuthProductError> {
        let state = self.lock_state();
        let Some(flow) = state.flows.get(&flow_id) else {
            return Ok(None);
        };
        let custody = state
            .flow_custody
            .get(&flow_id)
            .ok_or(AuthProductError::BackendUnavailable)?;
        custody.original_requirement_selection(scope, flow_id, flow, requirement)
    }

    async fn accounts_for_owner(
        &self,
        scope: &crate::AuthProductScope,
    ) -> Result<Vec<CredentialAccount>, AuthProductError> {
        let owner = CredentialAccountOwnerScope::from_scope(scope);
        let state = self.lock_state();
        let mut accounts = state
            .accounts
            .values()
            .filter(|account| owner.matches(account))
            .cloned()
            .collect::<Vec<_>>();
        accounts.sort_by_key(|account| account.id);
        Ok(accounts)
    }
}

#[async_trait]
impl CredentialSetupService for InMemoryAuthProductServices {
    async fn refresh_action(
        &self,
        request: &CredentialRefreshRequest,
    ) -> Result<Option<crate::CredentialRefreshAction>, AuthProductError> {
        let original = crate::CredentialRefreshOriginal::from(request);
        let key = (original.scope_ref()?, original.id);
        let action = self.lock_state().refresh_actions.get(&key).cloned();
        if let Some(action) = &action {
            action.original.verify(request)?;
        }
        Ok(action)
    }
    async fn begin_refresh(
        &self,
        prepared: crate::CredentialRefreshPrepared,
    ) -> Result<crate::CredentialRefreshAdmission, AuthProductError> {
        let key = (prepared.original.scope_ref()?, prepared.original.id);
        let mut state = self.lock_state();
        if let Some(action) = state.refresh_actions.get(&key) {
            if action.original != prepared.original {
                return Err(AuthProductError::BackendConflict);
            }
            return Ok(crate::CredentialRefreshAdmission::Original(action.clone()));
        }
        let existing = state
            .refresh_actions
            .values()
            .filter(|a| a.original.account_id == prepared.original.account_id)
            .cloned()
            .collect::<Vec<_>>();
        let account = state
            .accounts
            .get_mut(&prepared.original.account_id)
            .ok_or(AuthProductError::CredentialMissing)?;
        let admission =
            crate::CredentialRefreshAction::begin(account, &existing, prepared, Utc::now())?;
        let action = match &admission {
            crate::CredentialRefreshAdmission::Original(action)
            | crate::CredentialRefreshAdmission::Dispatch(action) => action,
        };
        state.refresh_actions.insert(key, action.clone());
        Ok(admission)
    }
    async fn reconcile_refresh(
        &self,
        request: &CredentialRefreshRequest,
    ) -> Result<Option<crate::CredentialRefreshAction>, AuthProductError> {
        let original = crate::CredentialRefreshOriginal::from(request);
        let key = (original.scope_ref()?, original.id);
        let mut state = self.lock_state();
        let AuthState {
            accounts,
            refresh_actions,
            ..
        } = &mut *state;
        let Some(action) = refresh_actions.get_mut(&key) else {
            return Ok(None);
        };
        action.original.verify(request)?;
        let account = accounts
            .get_mut(&request.account_id)
            .ok_or(AuthProductError::CredentialMissing)?;
        action.publish(account, Utc::now())?;
        Ok(Some(action.clone()))
    }

    async fn complete_refresh(
        &self,
        completion: crate::CredentialRefreshCompletion,
    ) -> Result<crate::CredentialRefreshAction, AuthProductError> {
        let original = &completion.dispatched.original;
        let key = (original.scope_ref()?, original.id);
        let mut state = self.lock_state();
        let AuthState {
            accounts,
            refresh_actions,
            ..
        } = &mut *state;
        let account = accounts
            .get_mut(&original.account_id)
            .ok_or(AuthProductError::CredentialMissing)?;
        let action = refresh_actions
            .get_mut(&key)
            .ok_or(AuthProductError::BackendConflict)?;
        action.complete(account, completion, Utc::now())?;
        Ok(action.clone())
    }

    async fn compare_and_update_account(
        &self,
        expected: &CredentialAccount,
        request: NewCredentialAccount,
    ) -> Result<CredentialAccount, AuthProductError> {
        let mut state = self.lock_state();
        let current = state
            .accounts
            .get_mut(&expected.id)
            .ok_or(AuthProductError::CredentialMissing)?;
        if &*current != expected {
            return Err(AuthProductError::BackendConflict);
        }
        update_account_from_request(current, request, Utc::now())
    }

    async fn create_or_update_account(
        &self,
        request: CredentialAccountMutation,
    ) -> Result<CredentialAccount, AuthProductError> {
        let mut state = self.lock_state();
        match request {
            CredentialAccountMutation::Create(account) => {
                create_account_in_state(&mut state, account)
            }
            CredentialAccountMutation::Update(update) => {
                let now = Utc::now();
                let account = state
                    .accounts
                    .get_mut(&update.account_id)
                    .ok_or(AuthProductError::CredentialMissing)?;
                validate_account_update_target(account, &update.account)?;
                update_account_from_request(account, update.account, now)
            }
        }
    }
}

#[async_trait]
impl AuthInteractionService for InMemoryAuthProductServices {
    async fn request_secret_input(
        &self,
        request: ManualTokenSetupRequest,
    ) -> Result<AuthChallenge, AuthProductError> {
        if let Some(setup) = &request.requirement_setup {
            setup.verify_manual(&request.scope, &request.provider)?;
        }
        let interaction_id = AuthInteractionId::new();
        let mut state = self.lock_state();
        if let Some(binding) = &request.update_binding {
            let account = state
                .accounts
                .get(&binding.account_id)
                .ok_or(AuthProductError::CredentialMissing)?;
            validate_manual_token_update_binding(account, &request, binding)?;
            let snapshot = account.clone();
            state
                .manual_update_snapshots
                .insert(interaction_id, snapshot);
        }
        state.interactions.insert(
            interaction_id,
            PendingSecretInteraction {
                requirement_setup: request.requirement_setup,
                scope: request.scope,
                provider: request.provider.clone(),
                label: request.label.clone(),
                continuation: request.continuation,
                update_binding: request.update_binding,
                expires_at: request.expires_at,
                setup_identity: None,
                setup_created_at: None,
                canceled_at: None,
            },
        );
        Ok(AuthChallenge::ManualTokenRequired {
            interaction_id,
            provider: request.provider,
            label: request.label,
            expires_at: request.expires_at,
        })
    }

    async fn submit_manual_token(
        &self,
        scope: &crate::AuthProductScope,
        request: SecretSubmitRequest,
    ) -> Result<SecretSubmitResult, AuthProductError> {
        request.validate_secret()?;
        let mut state = self.lock_state();
        submit_manual_token_in_state(&mut state, scope, request, false, None, Utc::now())
    }

    async fn abandon_manual_token(
        &self,
        scope: &crate::AuthProductScope,
        interaction_id: crate::AuthInteractionId,
    ) -> Result<bool, AuthProductError> {
        let mut state = self.lock_state();
        let Some(mut pending) = state.interactions.get(&interaction_id).cloned() else {
            return Ok(false);
        };
        if !scope_matches(scope, &pending.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if pending.canceled_at.is_some() {
            return Ok(false);
        }
        if let Some(identity) = pending.setup_identity {
            let mut flow = state
                .flows
                .get(&identity.flow_id)
                .cloned()
                .ok_or(AuthProductError::BackendConflict)?;
            let original = pending_setup_request(&pending);
            crate::domain::validate_original_manual_setup_flow(
                identity, &original, &original, &flow,
            )?;
            if crate::is_terminal_status(flow.status) && flow.status != AuthFlowStatus::Canceled {
                return Ok(false);
            }
            if flow.status != AuthFlowStatus::Canceled {
                flow.status = AuthFlowStatus::Canceled;
                flow.error = Some(crate::AuthErrorCode::Canceled);
                flow.updated_at = Utc::now();
            }
            pending.canceled_at = Some(flow.updated_at);
            state.flows.insert(identity.flow_id, flow);
            state.interactions.insert(interaction_id, pending);
            return Ok(true);
        }
        state.manual_update_snapshots.remove(&interaction_id);
        Ok(state.interactions.remove(&interaction_id).is_some())
    }
}

#[async_trait]
impl AuthProviderClient for InMemoryAuthProductServices {
    async fn prepare_callback(
        &self,
        context: &crate::OAuthProviderExchangeContext,
        _request: &crate::OAuthProviderCallbackRequest,
    ) -> Result<crate::OAuthTokenDestinations, AuthProductError> {
        Self::callback_destinations_for_tests(context)
    }

    async fn prepare_refresh(
        &self,
        request: &OAuthProviderRefreshRequest,
    ) -> Result<crate::OAuthProviderRefreshPreparation, AuthProductError> {
        Self::refresh_preparation_for_tests(request)
    }

    async fn exchange_callback(
        &self,
        context: OAuthProviderExchangeContext,
        request: OAuthProviderCallbackRequest,
    ) -> Result<OAuthProviderExchange, AuthProductError> {
        validate_provider_callback_request(&request)?;
        let destinations = Self::callback_destinations_for_tests(&context)?;
        Ok(OAuthProviderExchange {
            provider: request.provider,
            account_label: request.account_label,
            authorization_code_hash: request.authorization_code_hash,
            pkce_verifier_hash: request.pkce_verifier_hash,
            access_secret: destinations.access_secret,
            access_expires_at: None,
            refresh_secret: Some(destinations.refresh_secret),
            scopes: request.scopes,
            account_id: None,
        })
    }

    async fn refresh_token(
        &self,
        request: OAuthProviderRefreshRequest,
        preparation: crate::OAuthProviderRefreshPreparation,
    ) -> Result<OAuthProviderRefresh, AuthProductError> {
        preparation.source.verify(&request)?;
        let destinations = preparation.destinations;
        let should_fail = {
            let mut state = self.lock_state();
            let should_fail = state.refresh_fails.remove(&request.account_id);
            if let Some((access_secret, refresh_secret)) =
                state.refresh_races.remove(&request.account_id)
                && let Some(account) = state.accounts.get_mut(&request.account_id)
            {
                account.access_secret = Some(access_secret);
                account.access_expires_at = None;
                account.refresh_secret = Some(refresh_secret);
                account.status = CredentialAccountStatus::Configured;
                account.updated_at = Utc::now();
            }
            should_fail
        };
        if should_fail {
            return Err(AuthProductError::RefreshFailed);
        }
        Ok(OAuthProviderRefresh {
            provider: request.provider,
            access_secret: destinations.access_secret,
            access_expires_at: None,
            refresh_secret: Some(destinations.refresh_secret),
            scopes: request.scopes,
        })
    }
}

#[async_trait]
impl SecretCleanupService for InMemoryAuthProductServices {
    async fn cleanup_for_lifecycle(
        &self,
        request: SecretCleanupRequest,
    ) -> Result<SecretCleanupReport, AuthProductError> {
        let mut state = self.lock_state();
        let quarantines = state.quarantines.clone();
        let mut report = SecretCleanupReport::default();
        let owner = CredentialAccountOwnerScope::from_scope(&request.scope);
        for account in state.accounts.values_mut() {
            if !owner.matches(account) {
                continue;
            }
            let owns_extension_account = account.owner_extension.as_ref()
                == Some(&request.extension_id)
                && account.ownership == CredentialOwnership::ExtensionOwned;
            let had_grant = account
                .granted_extensions
                .iter()
                .any(|extension| extension == &request.extension_id);
            if !(owns_extension_account || had_grant) {
                continue;
            }
            if let Some(reason) = quarantines.get(&account.id).copied() {
                report.quarantined_accounts.push(SecretCleanupQuarantine {
                    account_id: account.id,
                    reason,
                });
                continue;
            }
            account
                .granted_extensions
                .retain(|extension| extension != &request.extension_id);
            if had_grant {
                report.removed_grants.push(account.id);
            }

            let prior_status = account.status;
            if owns_extension_account {
                match request.action {
                    Deactivate => {
                        if !matches!(
                            account.status,
                            CredentialAccountStatus::Revoked | CredentialAccountStatus::Inactive
                        ) {
                            account.status = CredentialAccountStatus::Inactive;
                        }
                        report.retained_accounts.push(account.id);
                    }
                    SecretCleanupAction::Uninstall => {
                        if account.status != CredentialAccountStatus::Revoked {
                            account.status = CredentialAccountStatus::Revoked;
                            report.revoked_accounts.push(account.id);
                        }
                    }
                }
            } else if had_grant {
                report.retained_accounts.push(account.id);
            }
            if had_grant || account.status != prior_status {
                account.updated_at = Utc::now();
            }
        }
        Ok(report)
    }
}

fn create_account_in_state(
    state: &mut AuthState,
    request: NewCredentialAccount,
) -> Result<CredentialAccount, AuthProductError> {
    validate_new_credential_account(&request)?;
    let account = account_from_request(request, Utc::now());
    state.accounts.insert(account.id, account.clone());
    Ok(account)
}

fn account_from_request(request: NewCredentialAccount, now: Timestamp) -> CredentialAccount {
    CredentialAccount {
        id: CredentialAccountId::new(),
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
    }
}

// Prepare a candidate without changing AuthState. All flow/head checks must
// succeed before the existing owner mutex commits any account or continuation.
fn prepare_manual_token_account(
    state: &AuthState,
    pending: &PendingSecretInteraction,
    now: Timestamp,
) -> Result<CredentialAccount, AuthProductError> {
    let expected = pending
        .update_binding
        .as_ref()
        .map(|binding| {
            state
                .accounts
                .get(&binding.account_id)
                .ok_or(AuthProductError::CredentialMissing)
        })
        .transpose()?;
    let binding = pending.update_binding.as_ref();
    let scopes = if let Some(account) = expected {
        account.scopes.clone()
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
                    .map(crate::ProviderScope::new)
                    .collect::<Result<Vec<_>, AuthProductError>>()
            })
            .transpose()?
            .unwrap_or_default()
    };
    let request = NewCredentialAccount {
        scope: expected.map_or_else(|| pending.scope.clone(), |account| account.scope.clone()),
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
        access_secret: Some(generated_secret_handle("manual-access")?),
        access_expires_at: None,
        refresh_secret: None,
        scopes,
    };
    validate_new_credential_account(&request)?;
    if let Some(account) = expected {
        validate_account_update_target(account, &request)?;
        let mut prepared = account.clone();
        update_account_from_request(&mut prepared, request, now)
    } else {
        Ok(account_from_request(request, now))
    }
}

fn generated_secret_handle(prefix: &str) -> Result<SecretHandle, AuthProductError> {
    SecretHandle::new(format!("{prefix}-{}", CredentialAccountId::new()))
        .map_err(|_| AuthProductError::BackendUnavailable)
}

fn pending_setup_request(pending: &PendingSecretInteraction) -> ManualTokenSetupRequest {
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

fn submit_manual_token_in_state(
    state: &mut AuthState,
    scope: &crate::AuthProductScope,
    request: SecretSubmitRequest,
    complete_flow: bool,
    expected_flow: Option<AuthFlowId>,
    now: Timestamp,
) -> Result<SecretSubmitResult, AuthProductError> {
    if let Some(original) = state.manual_originals.get(&request.interaction_id) {
        if !scope_matches(scope, &original.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if !bool::from(
            original
                .secret
                .expose_secret()
                .as_bytes()
                .ct_eq(request.secret.expose_secret().as_bytes()),
        ) {
            return Err(AuthProductError::BackendConflict);
        }
        if expected_flow.is_some() && expected_flow != original.flow.as_ref().map(|flow| flow.id) {
            return Err(AuthProductError::BackendConflict);
        }
        if complete_flow {
            if !original.complete_flow {
                return Err(AuthProductError::BackendConflict);
            }
            let selected = original
                .flow
                .as_ref()
                .ok_or(AuthProductError::BackendConflict)?;
            return confirmed_manual_original(
                state,
                scope,
                crate::ManualTokenSetupIdentity {
                    interaction_id: request.interaction_id,
                    flow_id: selected.id,
                },
            )
            .map(|(result, _)| result);
        }
        return Ok(original.result.clone());
    }
    let pending = state
        .interactions
        .get(&request.interaction_id)
        .ok_or(AuthProductError::UnknownOrExpiredFlow)?
        .clone();
    if !scope_matches(scope, &pending.scope) {
        return Err(AuthProductError::CrossScopeDenied);
    }
    if pending.canceled_at.is_some() {
        return Err(AuthProductError::Canceled);
    }
    if now > pending.expires_at {
        return Err(AuthProductError::UnknownOrExpiredFlow);
    }
    if pending.requirement_setup.is_some() && !complete_flow {
        return Err(AuthProductError::BackendConflict);
    }
    let expected_account = state
        .manual_update_snapshots
        .get(&request.interaction_id)
        .cloned();
    let mut selected_flow = None;
    for flow in state.flows.values() {
        if matches!(&flow.challenge, Some(AuthChallenge::ManualTokenRequired{interaction_id,..})
                if *interaction_id == request.interaction_id)
        {
            if !scope_matches(scope, &flow.scope) {
                return Err(AuthProductError::CrossScopeDenied);
            }
            if selected_flow.is_some() {
                return Err(AuthProductError::BackendConflict);
            }
            let original = pending_setup_request(&pending);
            crate::domain::validate_original_manual_setup_flow(
                crate::ManualTokenSetupIdentity {
                    interaction_id: request.interaction_id,
                    flow_id: flow.id,
                },
                &original,
                &original,
                flow,
            )?;
            let custody = state
                .flow_custody
                .get(&flow.id)
                .ok_or(AuthProductError::BackendConflict)?;
            if custody.update_account_snapshot.as_ref() != expected_account.as_ref()
                || custody.requirement_setup != pending.requirement_setup
                || custody.requirement_selection.is_some()
            {
                return Err(AuthProductError::BackendConflict);
            }
            if let Some(id) = pending.setup_identity
                && (id.flow_id != flow.id || pending.setup_created_at != Some(flow.created_at))
            {
                return Err(AuthProductError::BackendConflict);
            }
            selected_flow = Some(flow.clone());
            match flow.status {
                AuthFlowStatus::Canceled => return Err(AuthProductError::Canceled),
                AuthFlowStatus::Expired => return Err(AuthProductError::UnknownOrExpiredFlow),
                AuthFlowStatus::Failed => return Err(AuthProductError::FlowAlreadyTerminal),
                AuthFlowStatus::Completed => return Err(AuthProductError::BackendConflict),
                _ => {}
            }
        }
    }
    if expected_flow.is_some() && expected_flow != selected_flow.as_ref().map(|flow| flow.id) {
        return Err(AuthProductError::BackendConflict);
    }
    if complete_flow && selected_flow.is_none() {
        return Err(AuthProductError::UnknownOrExpiredFlow);
    }
    if pending.setup_identity.is_some() && selected_flow.is_none() {
        return Err(AuthProductError::BackendConflict);
    }
    if let Some(binding) = &pending.update_binding {
        let expected = state
            .manual_update_snapshots
            .get(&request.interaction_id)
            .ok_or(AuthProductError::BackendConflict)?;
        if state.accounts.get(&binding.account_id) != Some(expected) {
            return Err(AuthProductError::BackendConflict);
        }
    }
    let continuation = pending.continuation.clone();
    let account = prepare_manual_token_account(state, &pending, now)?;
    let result = SecretSubmitResult {
        account_id: account.id,
        status: account.status,
        continuation,
    };
    let mut selected_custody = selected_flow
        .as_ref()
        .map(|flow| {
            state
                .flow_custody
                .get(&flow.id)
                .cloned()
                .ok_or(AuthProductError::BackendConflict)
        })
        .transpose()?;
    if complete_flow && let Some(flow) = &mut selected_flow {
        flow.status = AuthFlowStatus::Completed;
        flow.credential_account_id = Some(result.account_id);
        flow.error = None;
        flow.updated_at = account.updated_at;
    }
    let publication = match (&selected_flow, &selected_custody) {
        (Some(flow), Some(custody)) => {
            prepare_requirement_publication(state, flow, custody, &account)?
        }
        (None, None) if pending.requirement_setup.is_none() => None,
        _ => return Err(AuthProductError::BackendConflict),
    };
    if let Some((_, selected)) = &publication {
        selected_custody
            .as_mut()
            .ok_or(AuthProductError::BackendConflict)?
            .requirement_selection = Some(Box::new(selected.clone()));
    }
    // Every fallible check precedes these infallible in-memory mutations. The
    // original, head, account and completed flow become visible under one lock.
    state.accounts.insert(account.id, account.clone());
    if complete_flow && let Some(flow) = &selected_flow {
        state.flows.insert(flow.id, flow.clone());
        if let Some(custody) = &selected_custody {
            state.flow_custody.insert(flow.id, custody.clone());
        }
        state.continuations.push(AuthContinuationEvent {
            flow_id: flow.id,
            scope: flow.scope.clone(),
            continuation: flow.continuation.clone(),
            credential_account_id: flow.credential_account_id,
            emitted_at: flow.updated_at,
        });
    }
    if let Some((key, selected)) = publication {
        state.requirement_selections.insert(key, selected);
    }
    state.manual_originals.insert(
        request.interaction_id,
        ManualOriginal {
            scope: scope.clone(),
            secret: request.secret,
            provider: account.provider.clone(),
            result: result.clone(),
            interaction: pending,
            flow: selected_flow,
            expected_account,
            complete_flow,
            account,
            flow_custody: selected_custody,
        },
    );
    state.interactions.remove(&request.interaction_id);
    state
        .manual_update_snapshots
        .remove(&request.interaction_id);
    Ok(result)
}

fn confirmed_manual_original(
    state: &AuthState,
    scope: &crate::AuthProductScope,
    identity: crate::ManualTokenSetupIdentity,
) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError> {
    let original = state
        .manual_originals
        .get(&identity.interaction_id)
        .ok_or(AuthProductError::BackendConflict)?;
    if !scope_matches(scope, &original.scope) {
        return Err(AuthProductError::CrossScopeDenied);
    }
    let mut expected = original
        .flow
        .clone()
        .ok_or(AuthProductError::BackendConflict)?;
    if expected.id != identity.flow_id {
        return Err(AuthProductError::BackendConflict);
    }
    expected.status = AuthFlowStatus::Completed;
    expected.credential_account_id = Some(original.result.account_id);
    expected.error = None;
    let flow = state
        .flows
        .get(&identity.flow_id)
        .ok_or(AuthProductError::BackendConflict)?;
    let mut comparable = flow.clone();
    comparable.updated_at = expected.updated_at;
    comparable.continuation_emitted_at = expected.continuation_emitted_at;
    if comparable != expected {
        return Err(AuthProductError::BackendConflict);
    }
    if let Some(expected_custody) = &original.flow_custody {
        if state.flow_custody.get(&identity.flow_id) != Some(expected_custody) {
            return Err(AuthProductError::BackendConflict);
        }
        if let Some(selected) = &expected_custody.requirement_selection {
            if selected.account() != &original.account {
                return Err(AuthProductError::BackendConflict);
            }
            selected.verify_receipt(
                flow,
                expected_custody
                    .requirement_setup
                    .as_deref()
                    .ok_or(AuthProductError::BackendConflict)?,
            )?;
        }
    }
    Ok((original.result.clone(), flow.clone()))
}
