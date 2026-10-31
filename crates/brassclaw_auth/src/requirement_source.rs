//! Host/storage-only generation selected with an original Auth-flow decision.
//! This record is not material, live eligibility, family lineage or a grant.
use brassclaw_host_api::RuntimeCredentialAccountRequirement;
use serde::{Deserialize, Serialize};

use crate::{
    AuthFlowId, AuthFlowRecord, AuthFlowStatus, AuthProductError, AuthProductScope,
    CredentialRequirementSelection, CredentialRequirementSelectionRef,
    CredentialSecretGenerationRef,
};

/// Protected original-flow metadata. The real owner captures the authenticated
/// generation at publication/choice; readers must not fill absence from current
/// material. The referenced selection binds the complete usage, original flow,
/// selected account/handle/scope and issuer deadline without copying that record.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "SourceRecord", into = "SourceRecord")]
pub struct CredentialRequirementSource {
    selection: CredentialRequirementSelectionRef,
    generation: CredentialSecretGenerationRef,
}

impl std::fmt::Debug for CredentialRequirementSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CredentialRequirementSource")
            .field("selection", &self.selection)
            .field("generation", &"[REDACTED]")
            .finish()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceRecord {
    format: String,
    selection: CredentialRequirementSelectionRef,
    generation: CredentialSecretGenerationRef,
}

impl CredentialRequirementSource {
    /// Data validation only. Only the actual Auth/Secret owners may publish
    /// this alongside their own original completed/staged decision. Supplied
    /// copies, a digest or a source label cannot establish that provenance.
    pub fn from_selection(
        selected: &CredentialRequirementSelection,
        generation: CredentialSecretGenerationRef,
    ) -> Result<Self, AuthProductError> {
        selected.verify_requested(selected.original_scope(), selected.requirement())?;
        Ok(Self {
            selection: selected.reference()?,
            generation,
        })
    }

    pub fn generation(&self) -> &CredentialSecretGenerationRef {
        &self.generation
    }

    pub fn verify_selection(
        &self,
        selected: &CredentialRequirementSelection,
    ) -> Result<(), AuthProductError> {
        selected.verify_requested(selected.original_scope(), selected.requirement())?;
        if self.selection != selected.reference()? {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(())
    }
}

impl TryFrom<SourceRecord> for CredentialRequirementSource {
    type Error = AuthProductError;
    fn try_from(record: SourceRecord) -> Result<Self, Self::Error> {
        if record.format != "credential-requirement-source/1" {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(Self {
            selection: record.selection,
            generation: record.generation,
        })
    }
}
impl From<CredentialRequirementSource> for SourceRecord {
    fn from(source: CredentialRequirementSource) -> Self {
        Self {
            format: "credential-requirement-source/1".into(),
            selection: source.selection,
            generation: source.generation,
        }
    }
}

impl crate::AuthFlowCustody {
    /// Read only the actual original completed flow. This does not authenticate
    /// a caller-supplied record, lease material or establish its task lineage.
    pub fn original_requirement_source(
        &self,
        scope: &AuthProductScope,
        expected_flow: AuthFlowId,
        flow: &AuthFlowRecord,
        requirement: &RuntimeCredentialAccountRequirement,
    ) -> Result<Option<CredentialRequirementSource>, AuthProductError> {
        let selected =
            self.original_requirement_selection(scope, expected_flow, flow, requirement)?;
        if flow.status != AuthFlowStatus::Completed {
            return Ok(None);
        }
        match (selected, self.requirement_source.as_deref()) {
            (_, None) => Ok(None), // Historical/missing proof is never backfilled.
            (Some(selected), Some(source)) => {
                source
                    .verify_selection(&selected)
                    .map_err(|_| AuthProductError::BackendUnavailable)?;
                Ok(Some(source.clone()))
            }
            (None, Some(_)) => Err(AuthProductError::BackendUnavailable),
        }
    }
}
