//! Immutable registered-source reads and unqualified observation retention.
//! No fetching, classification, review, job scheduling or availability publication.
use crate::FirstPartyCapabilityError;
use async_trait::async_trait;
use brassclaw_extensions::{CapabilityManifest, ExtensionError};
use brassclaw_host_api::{EffectKind, PermissionMode, ResourceScope, RuntimeDispatchErrorKind};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

pub const PREFIX_SOURCE_CAPABILITY_ID: &str = "builtin.prefix_source";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrefixSourceIdentity {
    pub profile: String,
    pub source_id: Uuid,
    pub revision: u64,
    pub registration_sha256: String,
}
impl PrefixSourceIdentity {
    pub fn validate(&self) -> bool {
        !self.profile.is_empty()
            && self.profile.len() <= 64
            && self.profile.bytes().enumerate().all(|(i, b)| {
                b.is_ascii_lowercase() || (i > 0 && (b.is_ascii_digit() || b == b'-'))
            })
            && !self.source_id.is_nil()
            && self.revision > 0
            && self.revision <= 9_007_199_254_740_991
            && self.registration_sha256.len() == 64
            && self
                .registration_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }
}
#[derive(Debug, thiserror::Error)]
pub enum PrefixSourceBackendError {
    #[error("invalid source storage request")]
    Invalid,
    #[error("source storage request conflicts with retained data")]
    Conflict,
    #[error("exact registered source not found")]
    NotFound,
    #[error("source storage unavailable")]
    Unavailable,
}
#[async_trait]
pub trait PrefixSourceBackend: Send + Sync {
    async fn read(
        &self,
        source: &PrefixSourceIdentity,
        scope: &ResourceScope,
    ) -> Result<Value, PrefixSourceBackendError>;
    /// Append exact diagnostic bytes as unqualified evidence. Storage success
    /// cannot authorize an Available indicator or claim successful checking.
    async fn retain_observation(
        &self,
        source: &PrefixSourceIdentity,
        diagnostic_bytes: &str,
        scope: &ResourceScope,
    ) -> Result<Value, PrefixSourceBackendError>;
}
#[derive(Default)]
pub(super) struct PrefixSourceState {
    backend: Option<Arc<dyn PrefixSourceBackend>>,
}
impl std::fmt::Debug for PrefixSourceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrefixSourceState")
            .field("wired", &self.backend.is_some())
            .finish()
    }
}
impl PrefixSourceState {
    pub(super) fn new(backend: Arc<dyn PrefixSourceBackend>) -> Self {
        Self {
            backend: Some(backend),
        }
    }
}
pub(super) fn manifest() -> Result<CapabilityManifest, ExtensionError> {
    super::first_party_capability_manifest(
        PREFIX_SOURCE_CAPABILITY_ID,
        "Read an exact registered source or retain unqualified immutable diagnostic bytes; never fetch, approve, activate or publish availability.",
        vec![EffectKind::ExternalWrite],
        PermissionMode::Allow,
        super::resource_profile(),
    )
}
pub(super) async fn dispatch(
    state: &PrefixSourceState,
    input: &Value,
    scope: &ResourceScope,
) -> Result<Value, FirstPartyCapabilityError> {
    let object = input.as_object().ok_or_else(super::input_error)?;
    let source: PrefixSourceIdentity =
        serde_json::from_value(input["source"].clone()).map_err(|_| super::input_error())?;
    if !source.validate() {
        return Err(super::input_error());
    }
    let operation = input["operation"].as_str().ok_or_else(super::input_error)?;
    let diagnostic = match operation {
        "read" if object.len() == 2 => None,
        "retain_observation" if object.len() == 3 => Some(
            input["diagnostic_bytes"]
                .as_str()
                .filter(|v| !v.is_empty() && v.len() <= 65536)
                .ok_or_else(super::input_error)?,
        ),
        _ => return Err(super::input_error()),
    };
    let backend = state.backend.as_ref().ok_or_else(super::operation_error)?;
    let result = if let Some(bytes) = diagnostic {
        backend.retain_observation(&source, bytes, scope).await
    } else {
        backend.read(&source, scope).await
    };
    result.map_err(|error| {
        FirstPartyCapabilityError::with_safe_summary(
            match error {
                PrefixSourceBackendError::Invalid => RuntimeDispatchErrorKind::InputEncode,
                PrefixSourceBackendError::Conflict | PrefixSourceBackendError::NotFound => {
                    RuntimeDispatchErrorKind::OperationFailed
                }
                PrefixSourceBackendError::Unavailable => RuntimeDispatchErrorKind::Backend,
            },
            error.to_string(),
        )
    })
}
