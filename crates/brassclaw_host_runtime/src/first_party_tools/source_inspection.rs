//! One mechanical, nonexecuting source inspection operation. The backend owns
//! contained parsing and acknowledged technical bounds, never validation policy.
use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_extensions::{CapabilityManifest, ExtensionError};
use brassclaw_host_api::{EffectKind, PermissionMode};
use serde_json::Value;

use crate::{FirstPartyCapabilityError, FirstPartyCapabilityResult};

pub const SOURCE_INVENTORY_INPUT_INVALID_REASON: &str = "source inspection input invalid";

fn input_error() -> FirstPartyCapabilityError {
    FirstPartyCapabilityError::with_safe_summary(
        brassclaw_host_api::RuntimeDispatchErrorKind::InputEncode,
        SOURCE_INVENTORY_INPUT_INVALID_REASON,
    )
    .with_usage(brassclaw_host_api::ResourceUsage::default())
}

pub const SOURCE_INSPECTION_CAPABILITY_ID: &str = "builtin.source_inspection";

#[async_trait]
pub trait SourceInspectionBackend: Send + Sync {
    /// Return exact JSON fact bytes and actual resource observations. Neither a
    /// successful parse nor this result is a component-validation verdict.
    async fn inspect(
        &self,
        source: &str,
    ) -> Result<FirstPartyCapabilityResult, FirstPartyCapabilityError>;

    /// One complete named inventory under a shared preparation budget. Values
    /// remain borrowed until the implementation has checked current limits.
    async fn inspect_sources(
        &self,
        sources: &serde_json::Map<String, Value>,
    ) -> Result<FirstPartyCapabilityResult, FirstPartyCapabilityError>;
    async fn capture_fixture_preparation(
        &self,
    ) -> Result<crate::SourceInspectionPreparationFacts, FirstPartyCapabilityError> {
        Err(FirstPartyCapabilityError::new(
            brassclaw_host_api::RuntimeDispatchErrorKind::UnsupportedRunner,
        ))
    }
    async fn inspect_sources_with_fixture(
        &self,
        _: &serde_json::Map<String, Value>,
        _: &crate::SourceInspectionFixtureProfile,
    ) -> Result<FirstPartyCapabilityResult, FirstPartyCapabilityError> {
        Err(FirstPartyCapabilityError::new(
            brassclaw_host_api::RuntimeDispatchErrorKind::UnsupportedRunner,
        ))
    }
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
    pub(super) fn retained_backend(&self) -> Option<Arc<dyn SourceInspectionBackend>> {
        self.backend.clone()
    }
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
        "Inspect Python text or one complete named source inventory without executing it; return compiler/parser facts, never approval or a validation verdict.",
        vec![EffectKind::DispatchCapability],
        PermissionMode::Allow,
        None,
    )
}

pub(super) async fn dispatch(
    state: &SourceInspectionState,
    input: &Value,
    fixture: Option<&crate::SourceInspectionFixtureProfile>,
) -> Result<FirstPartyCapabilityResult, FirstPartyCapabilityError> {
    if fixture.is_some_and(|fixture| fixture.arguments() != input) {
        return Err(super::operation_error());
    }
    let object = input
        .as_object()
        .filter(|object| object.len() == 1)
        .ok_or_else(input_error)?;
    let backend = state.backend.as_ref().ok_or_else(super::operation_error)?;
    let result = match (object.get("source"), object.get("sources")) {
        (Some(Value::String(source)), None) if fixture.is_none() => backend.inspect(source).await?,
        (None, Some(Value::Object(sources)))
            if sources
                .iter()
                .all(|(id, source)| !id.is_empty() && source.is_string()) =>
        {
            match fixture {
                Some(fixture) => {
                    backend
                        .inspect_sources_with_fixture(sources, fixture)
                        .await?
                }
                None => backend.inspect_sources(sources).await?,
            }
        }
        _ => return Err(input_error()),
    };
    if !result.output.is_string() || result.display_preview.is_some() {
        return Err(super::operation_error());
    }
    Ok(result)
}
