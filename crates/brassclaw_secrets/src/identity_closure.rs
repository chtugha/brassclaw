//! Trusted setup identity closure inside the existing secret owner. An absent
//! reservation contains no fabricated material or value-generation reference.
use crate::custody::{CustodyBody, DecodedSecretValue};
use crate::{SecretCustodyMetadata, SecretCustodySource, SecretCustodyState};
use brassclaw_host_api::ResourceScope;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SecretIdentityClosure {
    /// Retire the exact authenticated ordinary generation without inventing
    /// original-value provenance or allowing a future writer to reuse its name.
    Ordinary {
        reference: String,
        context: Value,
    },
    /// Reserve an actually absent identity. This blocks future writers; it
    /// does not claim that a value was deleted or had never existed historically.
    Absent {
        context: Value,
    },
    Original {
        reference: String,
        context: Value,
    },
    Retained {
        reference: String,
        source: SecretCustodySource,
    },
}
impl std::fmt::Debug for SecretIdentityClosure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Absent { .. } => "Absent([REDACTED])",
            Self::Ordinary { .. } => "Ordinary([REDACTED])",
            Self::Original { .. } => "Original([REDACTED])",
            Self::Retained { .. } => "Retained([REDACTED])",
        })
    }
}
pub(crate) enum ClosureDecision {
    Keep,
    Write(Box<CustodyBody>),
    Reject,
}
impl ClosureDecision {
    fn write(body: CustodyBody) -> Self {
        Self::Write(Box::new(body))
    }
}

impl SecretIdentityClosure {
    pub(crate) fn source(&self) -> Option<&SecretCustodySource> {
        match self {
            Self::Retained { source, .. } => Some(source),
            _ => None,
        }
    }
    /// Metadata proof for readback after an unknown write acknowledgement.
    /// Caller must separately verify the expected handle; this grants no lease.
    pub fn accepts(&self, scope: &ResourceScope, metadata: &SecretCustodyMetadata) -> bool {
        if &metadata.scope != scope {
            return false;
        }
        match (self, &metadata.state) {
            (
                Self::Ordinary { reference, context },
                SecretCustodyState::RetiredOrdinary {
                    original_reference,
                    context: stored,
                },
            ) => reference == original_reference && context == stored,
            (Self::Absent { context: a }, SecretCustodyState::ClosedIdentity { context: b }) => {
                a == b
            }
            (
                Self::Original { reference, context },
                SecretCustodyState::RetiredOriginal {
                    original_reference,
                    context: stored,
                },
            ) => reference == original_reference && context == stored,
            (
                Self::Retained { reference, source },
                SecretCustodyState::RetiredRetained {
                    retained_reference,
                    source: stored,
                },
            ) => reference == retained_reference && source == stored,
            _ => false,
        }
    }
    pub(crate) fn decide(
        &self,
        scope: &ResourceScope,
        current: Option<(&DecodedSecretValue, &str)>,
    ) -> ClosureDecision {
        let Some((current, reference)) = current else {
            return match self {
                Self::Absent { context } => ClosureDecision::write(CustodyBody::ClosedIdentity {
                    scope: scope.clone(),
                    context: context.clone(),
                }),
                _ => ClosureDecision::Reject,
            };
        };
        if let Self::Ordinary {
            reference: expected,
            context,
        } = self
            && matches!(current, DecodedSecretValue::Ordinary(_))
            && expected == reference
        {
            return ClosureDecision::write(CustodyBody::RetiredOrdinary {
                scope: scope.clone(),
                original_reference: reference.to_owned(),
                context: context.clone(),
            });
        }
        let Some(body) = current.custody() else {
            return ClosureDecision::Reject;
        };
        if body.scope() != scope {
            return ClosureDecision::Reject;
        }
        match (self, body) {
            (
                Self::Ordinary {
                    reference: expected,
                    context,
                },
                CustodyBody::RetiredOrdinary {
                    original_reference,
                    context: stored,
                    ..
                },
            ) if expected == original_reference && context == stored => ClosureDecision::Keep,
            (Self::Absent { context: a }, CustodyBody::ClosedIdentity { context: b, .. })
                if a == b =>
            {
                ClosureDecision::Keep
            }
            (
                Self::Original {
                    reference: expected,
                    context,
                },
                CustodyBody::Original {
                    context: stored, ..
                },
            ) if expected == reference && context == stored => {
                ClosureDecision::write(CustodyBody::RetiredOriginal {
                    scope: scope.clone(),
                    original_reference: reference.to_owned(),
                    context: context.clone(),
                })
            }
            (
                Self::Retained {
                    reference: expected,
                    source,
                },
                CustodyBody::Retained { source: stored, .. },
            ) if expected == reference && source == stored => {
                ClosureDecision::write(CustodyBody::RetiredRetained {
                    scope: scope.clone(),
                    retained_reference: reference.to_owned(),
                    source: source.clone(),
                })
            }
            (
                Self::Original {
                    reference: expected,
                    context,
                },
                CustodyBody::RetiredOriginal {
                    original_reference,
                    context: stored,
                    ..
                },
            ) if expected == original_reference && context == stored => ClosureDecision::Keep,
            (
                Self::Retained {
                    reference: expected,
                    source,
                },
                CustodyBody::RetiredRetained {
                    retained_reference,
                    source: stored,
                    ..
                },
            ) if expected == retained_reference && source == stored => ClosureDecision::Keep,
            _ => ClosureDecision::Reject,
        }
    }
}
