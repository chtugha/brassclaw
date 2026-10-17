//! One mechanical, nonexecuting source inspection operation. The backend owns
//! contained parsing and acknowledged technical bounds, never validation policy.
use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_extensions::{CapabilityManifest, ExtensionError};
use brassclaw_host_api::{EffectKind, PermissionMode};
use serde_json::Value;

use crate::{FirstPartyCapabilityError, FirstPartyCapabilityResult};

pub const SOURCE_INSPECTION_CAPABILITY_ID: &str = "builtin.source_inspection";

#[async_trait]
pub trait SourceInspectionBackend: Send + Sync {
    /// Return exact JSON fact bytes and actual resource observations. Neither a
    /// successful parse nor this result is a component-validation verdict.
    async fn inspect(
        &self,
        source: &str,
    ) -> Result<FirstPartyCapabilityResult, FirstPartyCapabilityError>;
}

#[derive(Default)]
pub(super) struct SourceInspectionState {
    backend: Option<Arc<dyn SourceInspectionBackend>>,
}
impl std::fmt::Debug for SourceInspectionState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SourceInspectionState")
            .field("wired", &self.backend.is_some())
            .finish()
    }
}
impl SourceInspectionState {
    pub(super) fn new(backend: Arc<dyn SourceInspectionBackend>) -> Self {
        Self {
            backend: Some(backend),
        }
    }
}

pub(super) fn manifest() -> Result<CapabilityManifest, ExtensionError> {
    // Actual preparation policy bounds the worker/output. The shared static
    // builtin defaults must not impose another hidden wall-time/output ceiling.
    super::first_party_capability_manifest(
        SOURCE_INSPECTION_CAPABILITY_ID,
        "Inspect Python text without executing it; return parser facts, never approval or a validation verdict.",
        vec![EffectKind::DispatchCapability],
        PermissionMode::Allow,
        None,
    )
}

pub(super) async fn dispatch(
    state: &SourceInspectionState,
    input: &Value,
) -> Result<FirstPartyCapabilityResult, FirstPartyCapabilityError> {
    let object = input
        .as_object()
        .filter(|object| object.len() == 1)
        .ok_or_else(super::input_error)?;
    let source = object
        .get("source")
        .and_then(Value::as_str)
        .ok_or_else(super::input_error)?;
    let backend = state.backend.as_ref().ok_or_else(super::operation_error)?;
    let result = backend.inspect(source).await?;
    if !result.output.is_string() || result.display_preview.is_some() {
        return Err(super::operation_error());
    }
    Ok(result)
}
