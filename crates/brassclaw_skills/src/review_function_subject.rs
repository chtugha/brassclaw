//! The partner of one authored function review, not another queue or approval.
//! Changed roots are staged together; unchanged partners are exact context only.

use super::{Reference, SubmissionStoreError};
use crate::{
    association_contract::ComponentRevisionRef,
    component_revision::{ComponentRevisionDraft, RetainedComponentSnapshot},
};
use serde::{Deserialize, Serialize};

/// Untrusted operator proposal. Retention verifies identity and the actual Skill
/// association; it does not assert that an unchanged partner is approved.
pub enum FunctionReviewPartnerDraft {
    Changed {
        candidate: Box<ComponentRevisionDraft>,
        base: Option<ComponentRevisionRef>,
        expected_previous_version: u64,
    },
    Context {
        reference: ComponentRevisionRef,
    },
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "role", deny_unknown_fields)]
pub(super) enum FunctionPartner {
    #[serde(rename = "changed")]
    Changed {
        candidate: Reference,
        candidate_bytes: String,
        base: Option<Reference>,
        expected_previous_version: u64,
    },
    #[serde(rename = "context")]
    Context { reference: Reference },
}

impl FunctionPartner {
    pub(super) fn from_draft(
        value: FunctionReviewPartnerDraft,
    ) -> Result<Self, SubmissionStoreError> {
        Ok(match value {
            FunctionReviewPartnerDraft::Changed {
                candidate,
                base,
                expected_previous_version,
            } => Self::Changed {
                candidate: super::candidate_reference(&candidate, base, expected_previous_version)?
                    .into(),
                candidate_bytes: candidate.exact_bytes().to_owned(),
                base: base.map(Into::into),
                expected_previous_version,
            },
            FunctionReviewPartnerDraft::Context { reference } => {
                Reference::from(reference).parsed()?;
                Self::Context {
                    reference: reference.into(),
                }
            }
        })
    }

    pub(super) fn reference(&self) -> Result<ComponentRevisionRef, SubmissionStoreError> {
        match self {
            Self::Changed { candidate, .. } => candidate.parsed(),
            Self::Context { reference } => reference.parsed(),
        }
    }

    pub(super) fn to_draft(&self) -> Result<FunctionReviewPartnerDraft, SubmissionStoreError> {
        Ok(match self {
            Self::Changed {
                candidate,
                candidate_bytes,
                base,
                expected_previous_version,
            } => {
                let draft = ComponentRevisionDraft::from_json(candidate_bytes)
                    .map_err(crate::revision_store::RevisionStoreError::from)?;
                let base = base.as_ref().map(Reference::parsed).transpose()?;
                if super::candidate_reference(&draft, base, *expected_previous_version)?
                    != candidate.parsed()?
                {
                    return Err(SubmissionStoreError::Invalid);
                }
                FunctionReviewPartnerDraft::Changed {
                    candidate: Box::new(draft),
                    base,
                    expected_previous_version: *expected_previous_version,
                }
            }
            Self::Context { reference } => FunctionReviewPartnerDraft::Context {
                reference: reference.parsed()?,
            },
        })
    }

    pub(super) fn is_changed(&self) -> bool {
        matches!(self, Self::Changed { .. })
    }

    /// Resolve the real retained association rather than inferring a partner
    /// from a name, prose example, dependency membership or caller's role label.
    pub(super) fn require_pair(
        &self,
        primary: ComponentRevisionRef,
        snapshot: &RetainedComponentSnapshot,
    ) -> Result<(), SubmissionStoreError> {
        let partner = self.reference()?;
        let (skill, code) = match (primary.class_code, partner.class_code) {
            (1..=3, 22) => (primary, partner),
            (22, 1..=3) => (partner, primary),
            _ => return Err(SubmissionStoreError::Invalid),
        };
        if snapshot.revisions()[&skill.uuid]
            .draft()
            .association()
            .is_none_or(|association| association.python_code_uuid() != code.uuid)
        {
            return Err(SubmissionStoreError::Invalid);
        }
        Ok(())
    }
}
