//! One immutable job target. Pair checks have no role/profile revision or copied
//! model settings. These data contracts grant no workflow approval or admission.
use super::Error;
use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    Profile {
        name: String,
        revision: u64,
    },
    Pair {
        specification: String,
        proposal: String,
    },
}
impl Target {
    pub(crate) fn profile(&self) -> Option<(&str, u64)> {
        match self {
            Self::Profile { name, revision } => Some((name, *revision)),
            Self::Pair { .. } => None,
        }
    }
    pub(crate) fn from_columns(
        profile: Option<String>,
        revision: Option<i64>,
        operation: &str,
        mode: &str,
        request: &Value,
    ) -> Result<Self, Error> {
        let target = match (profile, revision) {
            (Some(name), Some(revision)) => Self::Profile {
                name,
                revision: u64::try_from(revision).map_err(|_| Error::Invalid)?,
            },
            (None, None) => Self::Pair {
                specification: request["specification"]
                    .as_str()
                    .ok_or(Error::Invalid)?
                    .into(),
                proposal: request["proposal"].as_str().ok_or(Error::Invalid)?.into(),
            },
            _ => return Err(Error::Invalid),
        };
        target.columns(operation, mode, request)?;
        Ok(target)
    }
    pub(super) fn columns(
        &self,
        operation: &str,
        mode: &str,
        request: &Value,
    ) -> Result<(Option<&str>, Option<i64>), Error> {
        if !matches!(mode, "source_only" | "target") {
            return Err(Error::Invalid);
        }
        match self {
            Self::Profile { name, revision } => {
                if name.is_empty()
                    || !matches!(
                        operation,
                        "generate" | "redesign" | "model_design" | "create" | "measure"
                    )
                {
                    return Err(Error::Invalid);
                }
                let revision = i64::try_from(*revision)
                    .ok()
                    .filter(|revision| *revision > 0)
                    .ok_or(Error::Invalid)?;
                Ok((Some(name), Some(revision)))
            }
            Self::Pair {
                specification,
                proposal,
            } => {
                crate::prefix_model_designs::hash(specification).map_err(|_| Error::Invalid)?;
                crate::prefix_model_designs::hash(proposal).map_err(|_| Error::Invalid)?;
                if operation != "check_cases"
                    || mode != "target"
                    || *request != self.pair_request().ok_or(Error::Invalid)?
                {
                    return Err(Error::Invalid);
                }
                Ok((None, None))
            }
        }
    }
    pub(crate) fn pair_request(&self) -> Option<Value> {
        match self {
            Self::Pair {
                specification,
                proposal,
            } => Some(
                json!({"operation":"check_cases","specification":specification,"proposal":proposal}),
            ),
            Self::Profile { .. } => None,
        }
    }
}
