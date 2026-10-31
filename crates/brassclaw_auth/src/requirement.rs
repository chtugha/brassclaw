//! Original requirement/account selection metadata for the existing Auth owner.
//! Records and their content references are not runtime grants. The storage
//! owner must verify actual original-flow custody, account state and CAS before
//! publication; deserializing or constructing this data proves none of them.
use brassclaw_host_api::{
    AgentId, ProjectId, ResourceScope, RuntimeCredentialAccountRequirement,
    RuntimeCredentialRequirementSource, TenantId, UserId, canonical_json_v1,
};
use serde::{Deserialize, Serialize};

use crate::{
    AuthFlowId, AuthFlowRecord, AuthFlowStatus, AuthProductError, AuthProductScope,
    CredentialAccount, CredentialAccountStatus, CredentialRequirementKey,
    CredentialRequirementSelectionRef,
};

const FORMAT: &str = "credential-requirement-selection/1";

/// A metadata publication decision, not permission to execute or repeat work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialRequirementPublication {
    Publish,
    AlreadyPublished,
}

/// Stable owner independent of the UI surface, setup invocation and task thread.
/// Account-specific thread/session restrictions still apply at every actual use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialRequirementOwner {
    tenant_id: TenantId,
    user_id: UserId,
    agent_id: Option<AgentId>,
    project_id: Option<ProjectId>,
}

fn digest<T: Serialize>(value: &T) -> Result<String, AuthProductError> {
    let value = serde_json::to_value(value).map_err(|_| AuthProductError::BackendUnavailable)?;
    let value = canonical_json_v1(&value).map_err(|_| AuthProductError::BackendUnavailable)?;
    let bytes = serde_json::to_vec(&value).map_err(|_| AuthProductError::BackendUnavailable)?;
    Ok(brassclaw_common::hashing::sha256_hex(&bytes))
}

impl CredentialRequirementOwner {
    pub fn from_scope(scope: &ResourceScope) -> Self {
        Self {
            tenant_id: scope.tenant_id.clone(),
            user_id: scope.user_id.clone(),
            agent_id: scope.agent_id.clone(),
            project_id: scope.project_id.clone(),
        }
    }

    pub fn matches_scope(&self, scope: &ResourceScope) -> bool {
        self == &Self::from_scope(scope)
    }

    /// Canonical location of this owner's one selection for an exact usage.
    /// This key has no mutable labels, enumeration ordinals or credential data.
    pub fn key(
        &self,
        requirement: &RuntimeCredentialAccountRequirement,
    ) -> Result<CredentialRequirementKey, AuthProductError> {
        CredentialRequirementKey::new(digest(&serde_json::json!({
            "format": "credential-requirement-key/1",
            "owner": self,
            "requirement": requirement,
        }))?)
    }
}

/// Immutable original selected account metadata. Account secret values remain
/// solely behind SecretStore; public projections must use account.projection().
/// A new task captures its actual credential generation separately, while live
/// eligibility/revocation remains independent of this retained original.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "SelectionRecord", into = "SelectionRecord")]
pub struct CredentialRequirementSelection {
    owner: CredentialRequirementOwner,
    requirement: RuntimeCredentialAccountRequirement,
    original_flow: AuthFlowId,
    original_scope: AuthProductScope,
    selected_at: crate::Timestamp,
    account: CredentialAccount,
    previous: Option<CredentialRequirementSelectionRef>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionRecord {
    format: String,
    owner: CredentialRequirementOwner,
    requirement: RuntimeCredentialAccountRequirement,
    original_flow: AuthFlowId,
    original_scope: AuthProductScope,
    selected_at: crate::Timestamp,
    account: CredentialAccount,
    previous: Option<CredentialRequirementSelectionRef>,
}

impl CredentialRequirementSelection {
    /// Data validation only. The real owner must read this original completed
    /// flow and its exact requirement/previous-selection custody itself.
    pub fn from_completed_flow(
        scope: &AuthProductScope,
        setup: &CredentialRequirementSetup,
        flow: &AuthFlowRecord,
        account: CredentialAccount,
    ) -> Result<Self, AuthProductError> {
        setup.verify_scope(scope)?;
        if !crate::scope_matches(scope, &flow.scope)
            || flow.kind != crate::AuthFlowKind::IntegrationCredential
            || flow.status != AuthFlowStatus::Completed
            || flow.error.is_some()
            || flow.credential_account_id != Some(account.id)
            || flow.provider != account.provider
            || account.effective_status_at(flow.updated_at) != CredentialAccountStatus::Configured
        {
            return Err(AuthProductError::BackendConflict);
        }
        let result = Self {
            owner: setup.owner.clone(),
            requirement: setup.requirement.clone(),
            original_flow: flow.id,
            original_scope: flow.scope.clone(),
            selected_at: flow.updated_at,
            account,
            previous: setup.previous.as_ref().map(Self::reference).transpose()?,
        };
        result.verify()?;
        Ok(result)
    }

    fn verify(&self) -> Result<(), AuthProductError> {
        let RuntimeCredentialRequirementSource::ProductAuthAccount { provider, .. } =
            &self.requirement.requirement().source
        else {
            return Err(AuthProductError::BackendConflict);
        };
        if !self.owner.matches_scope(&self.account.scope.resource)
            || !self.owner.matches_scope(&self.original_scope.resource)
            || self.original_flow.as_uuid().is_nil()
            || self.account.id.as_uuid().is_nil()
            || provider.as_str() != self.account.provider.as_str()
            || self.account.access_secret.is_none()
            || self.account.status != CredentialAccountStatus::Configured
            || self.account.effective_status_at(self.selected_at)
                != CredentialAccountStatus::Configured
            || !self
                .requirement
                .requirement()
                .provider_scopes
                .iter()
                .all(|scope| {
                    self.account
                        .scopes
                        .iter()
                        .any(|actual| actual.as_str() == scope)
                })
            || !self
                .account
                .is_authorized_for_requester(Some(self.requirement.requester_extension()))
        {
            return Err(AuthProductError::CrossScopeDenied);
        }
        Ok(())
    }

    pub fn key(&self) -> Result<CredentialRequirementKey, AuthProductError> {
        self.owner.key(&self.requirement)
    }

    pub fn reference(&self) -> Result<CredentialRequirementSelectionRef, AuthProductError> {
        CredentialRequirementSelectionRef::new(digest(self)?)
    }

    pub fn original_flow(&self) -> AuthFlowId {
        self.original_flow
    }
    pub fn original_scope(&self) -> &AuthProductScope {
        &self.original_scope
    }
    pub fn account(&self) -> &CredentialAccount {
        &self.account
    }
    pub fn requirement(&self) -> &RuntimeCredentialAccountRequirement {
        &self.requirement
    }
    pub fn previous(&self) -> Option<&CredentialRequirementSelectionRef> {
        self.previous.as_ref()
    }

    pub fn verify_requested(
        &self,
        scope: &AuthProductScope,
        requirement: &RuntimeCredentialAccountRequirement,
    ) -> Result<(), AuthProductError> {
        self.verify()?;
        if !self.owner.matches_scope(&scope.resource) || &self.requirement != requirement {
            return Err(AuthProductError::CrossScopeDenied);
        }
        Ok(())
    }

    /// Verify against the owner's actual original flow and captured setup.
    /// The owner also compares its protected stored selection receipt with this
    /// exact record; a copied selection row alone is insufficient evidence.
    pub fn verify_receipt(
        &self,
        flow: &AuthFlowRecord,
        setup: &CredentialRequirementSetup,
    ) -> Result<(), AuthProductError> {
        self.verify()?;
        setup.verify_scope(&flow.scope)?;
        if flow.id != self.original_flow
            || flow.scope != self.original_scope
            || flow.kind != crate::AuthFlowKind::IntegrationCredential
            || flow.status != AuthFlowStatus::Completed
            || flow.error.is_some()
            || flow.credential_account_id != Some(self.account.id)
            || flow.provider != self.account.provider
            || setup.owner != self.owner
            || setup.requirement != self.requirement
            || setup.previous.as_ref().map(Self::reference).transpose()? != self.previous
        {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(())
    }
}

impl TryFrom<SelectionRecord> for CredentialRequirementSelection {
    type Error = AuthProductError;
    fn try_from(record: SelectionRecord) -> Result<Self, Self::Error> {
        if record.format != FORMAT {
            return Err(AuthProductError::BackendConflict);
        }
        let selected = Self {
            owner: record.owner,
            requirement: record.requirement,
            original_flow: record.original_flow,
            original_scope: record.original_scope,
            selected_at: record.selected_at,
            account: record.account,
            previous: record.previous,
        };
        selected.verify()?;
        Ok(selected)
    }
}

impl From<CredentialRequirementSelection> for SelectionRecord {
    fn from(value: CredentialRequirementSelection) -> Self {
        Self {
            format: FORMAT.to_owned(),
            owner: value.owner,
            requirement: value.requirement,
            original_flow: value.original_flow,
            original_scope: value.original_scope,
            selected_at: value.selected_at,
            account: value.account,
            previous: value.previous,
        }
    }
}

/// Capture before the original challenge. Repeating that setup retains this
/// same precondition even after another operator changes the current selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "SetupRecord", into = "SetupRecord")]
pub struct CredentialRequirementSetup {
    owner: CredentialRequirementOwner,
    requirement: RuntimeCredentialAccountRequirement,
    previous: Option<CredentialRequirementSelection>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SetupRecord {
    owner: CredentialRequirementOwner,
    requirement: RuntimeCredentialAccountRequirement,
    previous: Option<CredentialRequirementSelection>,
}

impl CredentialRequirementSetup {
    pub fn new(
        scope: &AuthProductScope,
        requirement: RuntimeCredentialAccountRequirement,
        previous: Option<CredentialRequirementSelection>,
    ) -> Result<Self, AuthProductError> {
        if let Some(previous) = &previous {
            previous.verify_requested(scope, &requirement)?;
        }
        Ok(Self {
            owner: CredentialRequirementOwner::from_scope(&scope.resource),
            requirement,
            previous,
        })
    }

    pub fn requirement(&self) -> &RuntimeCredentialAccountRequirement {
        &self.requirement
    }
    pub fn previous(&self) -> Option<&CredentialRequirementSelection> {
        self.previous.as_ref()
    }
    pub fn key(&self) -> Result<CredentialRequirementKey, AuthProductError> {
        self.owner.key(&self.requirement)
    }

    pub fn verify_scope(&self, scope: &AuthProductScope) -> Result<(), AuthProductError> {
        if !self.owner.matches_scope(&scope.resource) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if let Some(previous) = &self.previous {
            previous.verify_requested(scope, &self.requirement)?;
        }
        Ok(())
    }

    pub fn verify_flow(&self, flow: &AuthFlowRecord) -> Result<(), AuthProductError> {
        self.verify_provider(&flow.scope, &flow.provider)?;
        if flow.kind != crate::AuthFlowKind::IntegrationCredential {
            return Err(AuthProductError::BackendConflict);
        }
        if let Some(crate::AuthChallenge::ManualTokenRequired { provider, .. }) = &flow.challenge {
            if provider != &flow.provider {
                return Err(AuthProductError::BackendConflict);
            }
            self.verify_manual(&flow.scope, provider)?;
        }
        if matches!(&flow.challenge, Some(crate::AuthChallenge::OAuthUrl { .. }))
            && !matches!(
                &self.requirement.requirement().source,
                RuntimeCredentialRequirementSource::ProductAuthAccount {
                    setup: brassclaw_host_api::RuntimeCredentialAccountSetup::OAuth { .. },
                    ..
                }
            )
        {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(())
    }

    /// Validate the captured usage before publishing any challenge metadata.
    pub fn verify_provider(
        &self,
        scope: &AuthProductScope,
        provider: &crate::AuthProviderId,
    ) -> Result<(), AuthProductError> {
        self.verify_scope(scope)?;
        if !matches!(
            &self.requirement.requirement().source,
            RuntimeCredentialRequirementSource::ProductAuthAccount { provider: expected, .. }
                if expected.as_str() == provider.as_str()
        ) {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(())
    }

    /// Exact OAuth usage agreement, independently of account selection or grants.
    pub fn verify_oauth(
        &self,
        scope: &AuthProductScope,
        provider: &crate::AuthProviderId,
        scopes: &[crate::ProviderScope],
    ) -> Result<(), AuthProductError> {
        self.verify_provider(scope, provider)?;
        match &self.requirement.requirement().source {
            RuntimeCredentialRequirementSource::ProductAuthAccount {
                setup: brassclaw_host_api::RuntimeCredentialAccountSetup::OAuth { scopes: expected },
                ..
            } if expected.len() == scopes.len()
                && expected.iter().zip(scopes).all(|(a, b)| a == b.as_str()) =>
            {
                Ok(())
            }
            _ => Err(AuthProductError::BackendConflict),
        }
    }

    pub fn verify_manual(
        &self,
        scope: &AuthProductScope,
        provider: &crate::AuthProviderId,
    ) -> Result<(), AuthProductError> {
        self.verify_provider(scope, provider)?;
        if !matches!(
            &self.requirement.requirement().source,
            RuntimeCredentialRequirementSource::ProductAuthAccount {
                setup: brassclaw_host_api::RuntimeCredentialAccountSetup::ManualToken,
                ..
            }
        ) {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(())
    }

    /// Check the retained prechallenge selection against the actual current
    /// record. Owners still perform their real atomic CAS/transaction. An old
    /// receipt must be recovered from original-flow custody before this check;
    /// this method cannot reinterpret a later selection as that receipt.
    pub fn publication(
        &self,
        current: Option<&CredentialRequirementSelection>,
        selected: &CredentialRequirementSelection,
    ) -> Result<CredentialRequirementPublication, AuthProductError> {
        selected.verify()?;
        if self.owner != selected.owner
            || self.requirement != selected.requirement
            || selected.previous
                != self
                    .previous
                    .as_ref()
                    .map(CredentialRequirementSelection::reference)
                    .transpose()?
        {
            return Err(AuthProductError::BackendConflict);
        }
        if current == Some(selected) {
            return Ok(CredentialRequirementPublication::AlreadyPublished);
        }
        match (&self.previous, current) {
            (None, None) => Ok(CredentialRequirementPublication::Publish),
            (Some(expected), Some(actual)) if expected == actual => {
                Ok(CredentialRequirementPublication::Publish)
            }
            _ => Err(AuthProductError::BackendConflict),
        }
    }
}

impl TryFrom<SetupRecord> for CredentialRequirementSetup {
    type Error = AuthProductError;
    fn try_from(record: SetupRecord) -> Result<Self, Self::Error> {
        if let Some(previous) = &record.previous
            && (previous.owner != record.owner || previous.requirement != record.requirement)
        {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(Self {
            owner: record.owner,
            requirement: record.requirement,
            previous: record.previous,
        })
    }
}

impl From<CredentialRequirementSetup> for SetupRecord {
    fn from(value: CredentialRequirementSetup) -> Self {
        Self {
            owner: value.owner,
            requirement: value.requirement,
            previous: value.previous,
        }
    }
}

impl crate::AuthFlowCustody {
    /// Data validation for the actual storage owner. The owner must read the
    /// original flow/custody itself; supplied copies grant no source access.
    /// Never read a mutable requirement head to recover a completed original.
    pub fn original_requirement_selection(
        &self,
        scope: &AuthProductScope,
        expected_flow: crate::AuthFlowId,
        flow: &AuthFlowRecord,
        requirement: &RuntimeCredentialAccountRequirement,
    ) -> Result<Option<CredentialRequirementSelection>, AuthProductError> {
        if flow.id != expected_flow || !crate::scope_matches(scope, &flow.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if flow.status != AuthFlowStatus::Completed {
            // A staged FS candidate is not a completed publication. Cancelled,
            // failed and pending flows never advance a runtime credential view.
            return Ok(None);
        }
        match (
            self.requirement_setup.as_deref(),
            self.requirement_selection.as_deref(),
        ) {
            // Historical unrelated flows carry no exact-usage evidence.
            (None, None) if self.requirement_source.is_none() => Ok(None),
            (Some(setup), Some(selected)) => {
                selected.verify_requested(scope, requirement)?;
                selected
                    .verify_receipt(flow, setup)
                    .map_err(|_| AuthProductError::BackendUnavailable)?;
                if let Some(source) = &self.requirement_source {
                    source
                        .verify_selection(selected)
                        .map_err(|_| AuthProductError::BackendUnavailable)?;
                }
                Ok(Some(selected.clone()))
            }
            _ => Err(AuthProductError::BackendUnavailable),
        }
    }
}
