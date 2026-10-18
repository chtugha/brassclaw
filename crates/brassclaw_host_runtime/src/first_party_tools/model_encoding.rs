//! One offline rendering/tokenization primitive over exact retained model assets.
//! It does not choose prefix content/design, accept a model or activate a prefix.
use crate::{FirstPartyCapabilityError, FirstPartyCapabilityResult};
use async_trait::async_trait;
use brassclaw_extensions::{CapabilityManifest, ExtensionError};
use brassclaw_host_api::{EffectKind, PermissionMode, ResourceScope};
use serde_json::Value;
use std::{sync::Arc, time::Instant};

pub const MODEL_ENCODING_CAPABILITY_ID: &str = "builtin.model_encoding";
#[async_trait]
pub trait ModelEncodingBackend: Send + Sync {
    async fn encode(
        &self,
        specification: &str,
        artifact_set: &str,
        probes_json: &str,
        template_utf8: Option<&str>,
        scope: &ResourceScope,
    ) -> Result<FirstPartyCapabilityResult, FirstPartyCapabilityError>;
}
#[derive(Default)]
pub(super) struct ModelEncodingState {
    backend: Option<Arc<dyn ModelEncodingBackend>>,
}
impl std::fmt::Debug for ModelEncodingState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelEncodingState")
            .field("wired", &self.backend.is_some())
            .finish()
    }
}
impl ModelEncodingState {
    pub(super) fn new(backend: Arc<dyn ModelEncodingBackend>) -> Self {
        Self {
            backend: Some(backend),
        }
    }
}
pub(super) fn manifest() -> Result<CapabilityManifest, ExtensionError> {
    super::first_party_capability_manifest(
        MODEL_ENCODING_CAPABILITY_ID,
        "Render and encode exact supplied probes using immutable model assets in a contained offline worker; return token/byte hashes, never prefix design, quality approval or cache acceptance.",
        vec![EffectKind::DispatchCapability],
        PermissionMode::Allow,
        None,
    )
}
pub(super) async fn dispatch(
    state: &ModelEncodingState,
    input: &Value,
    scope: &ResourceScope,
) -> Result<FirstPartyCapabilityResult, FirstPartyCapabilityError> {
    let object = input
        .as_object()
        .filter(|value| {
            (3..=4).contains(&value.len())
                && value.keys().all(|key| {
                    matches!(
                        key.as_str(),
                        "specification" | "artifact_set" | "probes_json" | "template_utf8"
                    )
                })
        })
        .ok_or_else(super::input_error)?;
    let hash = |name| -> Result<&str, FirstPartyCapabilityError> {
        object
            .get(name)
            .and_then(Value::as_str)
            .filter(|value| {
                value.len() == 64
                    && value
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            })
            .ok_or_else(super::input_error)
    };
    let specification = hash("specification")?;
    let artifact_set = hash("artifact_set")?;
    let probes = object
        .get("probes_json")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(super::input_error)?;
    let template = match object.get("template_utf8") {
        None => None,
        Some(Value::String(value)) => Some(value.as_str()),
        Some(_) => return Err(super::input_error()),
    };
    let backend = state.backend.as_ref().ok_or_else(super::operation_error)?;
    let started = Instant::now();
    let elapsed = || u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let mut result = backend
        .encode(specification, artifact_set, probes, template, scope)
        .await
        .map_err(|error| {
            let mut usage = error.usage().cloned().unwrap_or_default();
            usage.wall_clock_ms = usage.wall_clock_ms.max(elapsed());
            error.with_usage(usage)
        })?;
    result.usage.wall_clock_ms = result.usage.wall_clock_ms.max(elapsed());
    if !result.output.is_string() || result.display_preview.is_some() {
        return Err(super::operation_error().with_usage(result.usage));
    }
    Ok(result)
}
