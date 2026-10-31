//! Durable local DCR cleanup alongside the existing original flow action.
//! These are nonmaterial selections and receipts, never raw secret values.
use crate::{
    AuthFlowCustody, AuthFlowRecord, AuthProductError, DcrMaterialReference,
    DcrRegistrationOriginal, DcrRegistrationState, Timestamp,
};
use brassclaw_host_api::SecretHandle;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DcrArtifactSlot {
    PreparationOriginal,
    PreparationRead,
    RegistrationResponse,
    RegistrationResponseRead,
    PkceOriginal,
    Pkce,
    ClientOriginal,
    Client,
    CleanupManagement,
    CleanupResponse,
    CleanupResponseRead,
}
impl DcrArtifactSlot {
    pub const ALL: [Self; 11] = [
        Self::PreparationOriginal,
        Self::PreparationRead,
        Self::RegistrationResponse,
        Self::RegistrationResponseRead,
        Self::PkceOriginal,
        Self::Pkce,
        Self::ClientOriginal,
        Self::Client,
        Self::CleanupManagement,
        Self::CleanupResponse,
        Self::CleanupResponseRead,
    ];
    pub fn usage(self) -> &'static str {
        match self {
            Self::PreparationOriginal => "preparation-original",
            Self::PreparationRead => "preparation-read",
            Self::RegistrationResponse => "registration-response",
            Self::RegistrationResponseRead => "registration-response-read",
            Self::PkceOriginal => "pkce-original",
            Self::Pkce => "pkce",
            Self::ClientOriginal => "client-original",
            Self::Client => "client",
            Self::CleanupManagement => "cleanup-management",
            Self::CleanupResponse => "cleanup-response",
            Self::CleanupResponseRead => "cleanup-response-read",
        }
    }
    pub fn source_slot(self) -> Option<Self> {
        match self {
            Self::PreparationRead => Some(Self::PreparationOriginal),
            Self::RegistrationResponseRead => Some(Self::RegistrationResponse),
            Self::Pkce => Some(Self::PkceOriginal),
            Self::Client => Some(Self::ClientOriginal),
            Self::CleanupResponseRead => Some(Self::CleanupResponse),
            _ => None,
        }
    }
}
impl DcrRegistrationOriginal {
    /// Derive this original's already selected private namespace, never the
    /// latest provider configuration. No value-generation reference is invented.
    pub fn artifact_handle(&self, slot: DcrArtifactSlot) -> Result<SecretHandle, AuthProductError> {
        let suffix = format!("-oauth-dcr-flow-preparation-original-{}", self.flow_id);
        let prefix = self
            .preparation
            .handle
            .as_str()
            .strip_suffix(&suffix)
            .filter(|prefix| !prefix.is_empty())
            .ok_or(AuthProductError::BackendConflict)?;
        SecretHandle::new(format!(
            "{prefix}-oauth-dcr-flow-{}-{}",
            slot.usage(),
            self.flow_id
        ))
        .map_err(|_| AuthProductError::BackendConflict)
    }
    pub fn absent_artifact_context(&self, slot: DcrArtifactSlot) -> Value {
        serde_json::json!({"format":"dcr-unused-artifact/1","original":self,"usage":slot.usage()})
    }
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DcrArtifactExpectation {
    Absent {
        context: Value,
    },
    Original {
        reference: String,
        context: Value,
    },
    Retained {
        reference: String,
        source: DcrMaterialReference,
    },
}
impl std::fmt::Debug for DcrArtifactExpectation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DcrArtifactExpectation([REDACTED])")
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DcrRetirementSelection {
    pub slot: DcrArtifactSlot,
    pub handle: SecretHandle,
    pub expected: DcrArtifactExpectation,
}
impl DcrRetirementSelection {
    pub fn validate(&self, original: &DcrRegistrationOriginal) -> Result<(), AuthProductError> {
        if self.handle != original.artifact_handle(self.slot)? {
            return Err(AuthProductError::BackendConflict);
        }
        match &self.expected {
            DcrArtifactExpectation::Absent { context }
                if *context == original.absent_artifact_context(self.slot) =>
            {
                Ok(())
            }
            DcrArtifactExpectation::Original { reference, .. }
                if self.slot.source_slot().is_none() && !reference.is_empty() =>
            {
                Ok(())
            }
            DcrArtifactExpectation::Retained { reference, source } if !reference.is_empty() => {
                let slot = self
                    .slot
                    .source_slot()
                    .ok_or(AuthProductError::BackendConflict)?;
                source.validate()?;
                if source.handle != original.artifact_handle(slot)? {
                    return Err(AuthProductError::BackendConflict);
                }
                Ok(())
            }
            _ => Err(AuthProductError::BackendConflict),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DcrRetirementProblem {
    StorageUnavailable,
    IdentityConflict,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DcrRetirementOutcome {
    #[default]
    Pending,
    Confirmed {
        closed_reference: String,
    },
    Unresolved {
        problem: DcrRetirementProblem,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DcrRetirementEntry {
    pub selection: DcrRetirementSelection,
    pub outcome: DcrRetirementOutcome,
    /// Preserve a formerly absent selection if a racing original writer won.
    /// This records reconciliation of the same slot, not a new task selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prior_absence: Option<DcrRetirementSelection>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DcrRetirement {
    pub entries: Vec<DcrRetirementEntry>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
impl DcrRetirement {
    pub fn validate(&self, original: &DcrRegistrationOriginal) -> Result<(), AuthProductError> {
        let slots = self
            .entries
            .iter()
            .map(|entry| entry.selection.slot)
            .collect::<std::collections::BTreeSet<_>>();
        let handles = self
            .entries
            .iter()
            .map(|entry| entry.selection.handle.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        if self.entries.len() != DcrArtifactSlot::ALL.len()
            || handles.len() != self.entries.len()
            || slots != DcrArtifactSlot::ALL.into_iter().collect()
        {
            return Err(AuthProductError::BackendConflict);
        }
        for entry in &self.entries {
            entry.selection.validate(original)?;
            if let Some(prior) = &entry.prior_absence {
                prior.validate(original)?;
                if prior.slot != entry.selection.slot
                    || prior.handle != entry.selection.handle
                    || !matches!(prior.expected, DcrArtifactExpectation::Absent { .. })
                    || matches!(
                        entry.selection.expected,
                        DcrArtifactExpectation::Absent { .. }
                    )
                {
                    return Err(AuthProductError::BackendConflict);
                }
            }
            if matches!(&entry.outcome,DcrRetirementOutcome::Confirmed {closed_reference} if closed_reference.is_empty())
            {
                return Err(AuthProductError::BackendConflict);
            }
        }
        Ok(())
    }
    pub fn confirmed(&self) -> bool {
        self.entries.len() == DcrArtifactSlot::ALL.len()
            && self
                .entries
                .iter()
                .map(|entry| entry.selection.slot)
                .collect::<std::collections::BTreeSet<_>>()
                == DcrArtifactSlot::ALL.into_iter().collect()
            && self
                .entries
                .iter()
                .all(|entry| matches!(entry.outcome, DcrRetirementOutcome::Confirmed { .. }))
    }
}
#[derive(Debug, Clone)]
pub enum DcrRetirementUpdate {
    Plan(Vec<DcrRetirementSelection>),
    Refine {
        before: DcrRetirementSelection,
        selected: DcrRetirementSelection,
    },
    Record {
        selected: DcrRetirementSelection,
        outcome: DcrRetirementOutcome,
    },
}
impl DcrRetirementUpdate {
    /// Original-operation readback may include another slot's newer receipt.
    /// Compare the requested selection/result rather than the whole document.
    pub fn satisfied_by(&self, action: &crate::DcrRegistrationAction) -> bool {
        let Some(retirement) = &action.retirement else {
            return false;
        };
        match self {
            Self::Plan(selections) => {
                selections.len() == DcrArtifactSlot::ALL.len()
                    && selections.iter().all(|selected| {
                        retirement.entries.iter().any(|entry| {
                            entry.selection == *selected
                                || entry.prior_absence.as_ref() == Some(selected)
                        })
                    })
            }
            Self::Refine { before, selected } => retirement.entries.iter().any(|entry| {
                entry.selection == *selected && entry.prior_absence.as_ref() == Some(before)
            }),
            Self::Record { selected, outcome } => retirement
                .entries
                .iter()
                .any(|entry| entry.selection == *selected && entry.outcome == *outcome),
        }
    }
}
impl AuthFlowCustody {
    /// The concrete owner authenticates selections/confirmed receipts against
    /// its actual SecretStore before saving this same-record transition.
    pub fn update_dcr_retirement(
        &mut self,
        flow: &AuthFlowRecord,
        original: &DcrRegistrationOriginal,
        update: DcrRetirementUpdate,
        now: Timestamp,
    ) -> Result<DcrRegistrationState, AuthProductError> {
        let state = self
            .dcr_original_state(flow, original)?
            .ok_or(AuthProductError::BackendConflict)?;
        if !state.action.cleanup.remote_confirmed()
            || flow.credential_account_id.is_some()
            || !matches!(
                flow.status,
                crate::AuthFlowStatus::Canceled
                    | crate::AuthFlowStatus::Expired
                    | crate::AuthFlowStatus::Failed
            )
        {
            return Err(AuthProductError::BackendConflict);
        }
        let mut action = state.action;
        match update {
            DcrRetirementUpdate::Plan(selections) => {
                for selection in &selections {
                    selection.validate(original)?;
                }
                let slots = selections
                    .iter()
                    .map(|s| s.slot)
                    .collect::<std::collections::BTreeSet<_>>();
                let handles = selections
                    .iter()
                    .map(|s| s.handle.as_str())
                    .collect::<std::collections::BTreeSet<_>>();
                if selections.len() != DcrArtifactSlot::ALL.len()
                    || handles.len() != selections.len()
                    || slots != DcrArtifactSlot::ALL.into_iter().collect()
                {
                    return Err(AuthProductError::BackendConflict);
                }
                if let Some(existing) = &action.retirement {
                    if selections.iter().any(|selected| {
                        !existing.entries.iter().any(|entry| {
                            entry.selection == *selected
                                || entry.prior_absence.as_ref() == Some(selected)
                        })
                    }) {
                        return Err(AuthProductError::BackendConflict);
                    }
                    return Ok(DcrRegistrationState {
                        flow: flow.clone(),
                        action,
                    });
                }
                action.retirement = Some(DcrRetirement {
                    entries: selections
                        .into_iter()
                        .map(|selection| DcrRetirementEntry {
                            selection,
                            outcome: DcrRetirementOutcome::Pending,
                            prior_absence: None,
                        })
                        .collect(),
                    created_at: now,
                    updated_at: now,
                });
            }
            DcrRetirementUpdate::Refine { before, selected } => {
                before.validate(original)?;
                selected.validate(original)?;
                if before.slot != selected.slot
                    || before.handle != selected.handle
                    || !matches!(before.expected, DcrArtifactExpectation::Absent { .. })
                    || matches!(selected.expected, DcrArtifactExpectation::Absent { .. })
                {
                    return Err(AuthProductError::BackendConflict);
                }
                let retirement = action
                    .retirement
                    .as_mut()
                    .ok_or(AuthProductError::BackendConflict)?;
                let entry = retirement
                    .entries
                    .iter_mut()
                    .find(|e| e.selection.slot == before.slot)
                    .ok_or(AuthProductError::BackendConflict)?;
                if entry.selection == selected && entry.prior_absence.as_ref() == Some(&before) {
                    return Ok(DcrRegistrationState {
                        flow: flow.clone(),
                        action,
                    });
                }
                if entry.selection != before
                    || entry.prior_absence.is_some()
                    || matches!(entry.outcome, DcrRetirementOutcome::Confirmed { .. })
                {
                    return Err(AuthProductError::BackendConflict);
                }
                entry.prior_absence = Some(before);
                entry.selection = selected;
                entry.outcome = DcrRetirementOutcome::Pending;
                retirement.updated_at = now;
            }
            DcrRetirementUpdate::Record { selected, outcome } => {
                if matches!(outcome, DcrRetirementOutcome::Pending)
                    || matches!(&outcome,DcrRetirementOutcome::Confirmed {closed_reference} if closed_reference.is_empty())
                {
                    return Err(AuthProductError::BackendConflict);
                }
                let retirement = action
                    .retirement
                    .as_mut()
                    .ok_or(AuthProductError::BackendConflict)?;
                let entry = retirement
                    .entries
                    .iter_mut()
                    .find(|e| e.selection == selected)
                    .ok_or(AuthProductError::BackendConflict)?;
                if entry.outcome == outcome {
                    return Ok(DcrRegistrationState {
                        flow: flow.clone(),
                        action,
                    });
                }
                if matches!(entry.outcome, DcrRetirementOutcome::Confirmed { .. }) {
                    return Err(AuthProductError::BackendConflict);
                }
                entry.outcome = outcome;
                retirement.updated_at = now;
            }
        }
        action.updated_at = now;
        self.dcr_registration = Some(action.clone());
        Ok(DcrRegistrationState {
            flow: flow.clone(),
            action,
        })
    }
}
