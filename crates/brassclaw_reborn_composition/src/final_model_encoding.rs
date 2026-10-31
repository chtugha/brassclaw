//! Final SDK-body counter over the existing contained model-encoding backend.
//! The binding verifier is REQUIRED and supplied by the canonical Prefix
//! acceptance owner. This module cannot turn a package, mapping or observation
//! into an approved design. No provider calls, template overrides or cap store.
//! This native composition API is not an HTTP/operator approval endpoint; ordinary
//! role attachment still requires the canonical acceptance owner.
use async_trait::async_trait;
use brassclaw_host_api::{ResourceEstimate, ResourceReservationId, ResourceScope, ResourceUsage};
use brassclaw_host_runtime::ModelEncodingBackend;
use brassclaw_llm::ModelRequestTokenCounts;
use brassclaw_llm::final_request_guard::{
    FinalRequestError, FinalRequestInput, FinalRequestMeasurement, FinalRequestMeasurer,
};
use brassclaw_model_encoding::{EncodingReport, Probe, checksum};
use brassclaw_resources::ResourceGovernor;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

/// Captured immutable references, never editable model settings. In particular
/// no token limits are copied here: the guard uses the canonical LIVE policy.
/// Pair acceptance is shared across roles; the profile acceptance and current
/// generation qualification are distinct. A design's evaluation generation is
/// NOT an equality constraint on every subsequent regeneration.
pub struct FinalEncodingSelection {
    pub specification: String,
    pub artifact_set: String,
    pub pair_acceptance: Uuid,
    pub profile_acceptance: Uuid,
    pub generation_qualification: Uuid,
    pub profile: String,
    pub generation: String,
    pub provider_id: String,
    pub model: String,
    pub prefix: Arc<str>,
    pub prefix_token_ids_sha256: String,
    pub mapping_json: Arc<str>,
    pub tokenizer_sha256: String,
    pub tokenizer_config_sha256: String,
    pub template_sha256: String,
    pub worker_sha256: String,
    pub preparation_revision: u64,
}
/// Immutable request-encoding inputs for an intentionally prefix-free qualified
/// workflow. No prefix generation or acceptance IDs exist in this variant.
/// `workflow_reference` identifies the retained native workflow/instruction
/// binding; the mandatory verifier must establish its actual provenance and
/// target/default-SDK contract. Construction alone establishes none of those.
pub struct WorkflowFinalEncodingSelection {
    pub workflow_reference: String,
    pub specification: String,
    pub artifact_set: String,
    pub provider_id: String,
    pub model: String,
    pub mapping_json: Arc<str>,
    pub tokenizer_sha256: String,
    pub tokenizer_config_sha256: String,
    pub template_sha256: String,
    pub worker_sha256: String,
    pub preparation_revision: u64,
}

// One counter, one encoder and one accounting path for both typed choices.
// These references are encoding identity, never another provider/limit store.
enum BoundFinalEncoding {
    Prefix(Arc<FinalEncodingSelection>),
    Workflow(Arc<WorkflowFinalEncodingSelection>),
}
struct EncodingMetadata<'a> {
    specification: &'a str,
    artifact_set: &'a str,
    provider_id: &'a str,
    model: &'a str,
    mapping_json: &'a str,
    tokenizer_sha256: &'a str,
    tokenizer_config_sha256: &'a str,
    template_sha256: &'a str,
    worker_sha256: &'a str,
    preparation_revision: u64,
}
impl BoundFinalEncoding {
    fn metadata(&self) -> EncodingMetadata<'_> {
        macro_rules! metadata {
            ($s:expr) => {
                EncodingMetadata {
                    specification: &$s.specification,
                    artifact_set: &$s.artifact_set,
                    provider_id: &$s.provider_id,
                    model: &$s.model,
                    mapping_json: &$s.mapping_json,
                    tokenizer_sha256: &$s.tokenizer_sha256,
                    tokenizer_config_sha256: &$s.tokenizer_config_sha256,
                    template_sha256: &$s.template_sha256,
                    worker_sha256: &$s.worker_sha256,
                    preparation_revision: $s.preparation_revision,
                }
            };
        }
        match self {
            Self::Prefix(s) => metadata!(s),
            Self::Workflow(s) => metadata!(s),
        }
    }
    fn leading_bytes(&self) -> &str {
        match self {
            Self::Prefix(s) => &s.prefix,
            // Physical zero-byte partition in the existing encoder, derived
            // from the typed workflow choice; not an empty selection sentinel.
            Self::Workflow(_) => "",
        }
    }
    fn leading_token_digest(&self) -> std::borrow::Cow<'_, str> {
        match self {
            Self::Prefix(s) => std::borrow::Cow::Borrowed(&s.prefix_token_ids_sha256),
            Self::Workflow(_) => std::borrow::Cow::Owned(checksum(&[])),
        }
    }
}

/// The existing acceptance owner must read the exact trusted immutable records,
/// scope/spec/artifact/proposal/package bindings and behavioral evidence. No
/// permissive implementation or source/status-label fallback exists here.
/// Verify the selected SDK/client default-header contract as well as the
/// explicit request headers. Borrowed headers alone do not expose defaults
/// that reqwest may apply only at execute time.
#[async_trait]
pub trait FinalEncodingBindingVerifier: Send + Sync {
    async fn verify(
        &self,
        selection: &FinalEncodingSelection,
        scope: &ResourceScope,
        headers: &reqwest::header::HeaderMap,
        body: &[u8],
    ) -> Result<(), FinalRequestError>;

    /// Verify the actual retained qualified workflow, instructions, original
    /// route/assets and supported SDK defaults, without requiring an accepted
    /// generated prefix. Existing prefix-only owners fail closed by default.
    /// A purpose label, echoed hash or empty generation cannot establish this.
    async fn verify_workflow(
        &self,
        _selection: &WorkflowFinalEncodingSelection,
        _scope: &ResourceScope,
        _headers: &reqwest::header::HeaderMap,
        _body: &[u8],
    ) -> Result<(), FinalRequestError> {
        Err(FinalRequestError::Binding)
    }
}

pub struct ContainedFinalRequestMeasurer {
    selection: BoundFinalEncoding,
    scope: ResourceScope,
    backend: Arc<dyn ModelEncodingBackend>,
    verifier: Arc<dyn FinalEncodingBindingVerifier>,
    governor: Arc<dyn ResourceGovernor>,
}
fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn valid_metadata(selection: EncodingMetadata<'_>) -> bool {
    [
        selection.specification,
        selection.artifact_set,
        selection.tokenizer_sha256,
        selection.tokenizer_config_sha256,
        selection.template_sha256,
        selection.worker_sha256,
    ]
    .into_iter()
    .all(hash)
        && [selection.provider_id, selection.model]
            .into_iter()
            .all(|value| {
                !value.is_empty() && value.trim() == value && !value.chars().any(char::is_control)
            })
        && selection.preparation_revision > 0
        && !selection.mapping_json.is_empty()
}
impl ContainedFinalRequestMeasurer {
    /// Native composition only; a successful constructor proves structural
    /// readiness, NOT approval. The mandatory verifier runs before each encode.
    pub fn new(
        selection: Arc<FinalEncodingSelection>,
        scope: ResourceScope,
        backend: Arc<dyn ModelEncodingBackend>,
        verifier: Arc<dyn FinalEncodingBindingVerifier>,
        governor: Arc<dyn ResourceGovernor>,
    ) -> Result<Self, FinalRequestError> {
        let bound = BoundFinalEncoding::Prefix(selection.clone());
        if !valid_metadata(bound.metadata())
            || [&selection.generation, &selection.prefix_token_ids_sha256]
                .iter()
                .any(|v| !hash(v))
            || selection.pair_acceptance.is_nil()
            || selection.profile_acceptance.is_nil()
            || selection.generation_qualification.is_nil()
            || selection.profile.is_empty()
            || selection.profile.trim() != selection.profile
            || selection.profile.chars().any(char::is_control)
            || selection.prefix.is_empty()
        {
            return Err(FinalRequestError::Binding);
        }
        Ok(Self {
            selection: bound,
            scope,
            backend,
            verifier,
            governor,
        })
    }

    /// Same mandatory final counter for a native prefix-free workflow. No
    /// synthetic generation/acceptance, alternate template, cap or provider.
    /// A verifier without explicit workflow support cannot measure or dispatch.
    pub fn for_workflow(
        selection: Arc<WorkflowFinalEncodingSelection>,
        scope: ResourceScope,
        backend: Arc<dyn ModelEncodingBackend>,
        verifier: Arc<dyn FinalEncodingBindingVerifier>,
        governor: Arc<dyn ResourceGovernor>,
    ) -> Result<Self, FinalRequestError> {
        let bound = BoundFinalEncoding::Workflow(selection.clone());
        if !valid_metadata(bound.metadata())
            || selection.workflow_reference.trim().is_empty()
            || selection.workflow_reference.trim() != selection.workflow_reference
            || selection.workflow_reference.chars().any(char::is_control)
        {
            return Err(FinalRequestError::Binding);
        }
        Ok(Self {
            selection: bound,
            scope,
            backend,
            verifier,
            governor,
        })
    }
}
// One reservation with the existing governor. Cancellation cannot leave a live
// reservation behind; there is no independent accounting ledger here.
pub(crate) struct EncodingReservation {
    governor: Arc<dyn ResourceGovernor>,
    id: Option<ResourceReservationId>,
}
impl EncodingReservation {
    pub(crate) fn reserve(
        governor: Arc<dyn ResourceGovernor>,
        scope: ResourceScope,
    ) -> Result<Self, FinalRequestError> {
        let reserved = governor
            .reserve(
                scope,
                ResourceEstimate {
                    process_count: Some(1),
                    ..ResourceEstimate::default()
                },
            )
            .map_err(|_| FinalRequestError::Measurement)?;
        Ok(Self {
            governor,
            id: Some(reserved.id),
        })
    }
    pub(crate) fn finish(mut self, usage: Option<&ResourceUsage>) -> Result<(), FinalRequestError> {
        let id = self.id.ok_or(FinalRequestError::Measurement)?;
        if let Some(usage) = usage {
            self.governor
                .reconcile(id, usage.clone())
                .map_err(|_| FinalRequestError::Measurement)?;
        } else {
            self.governor
                .release(id)
                .map_err(|_| FinalRequestError::Measurement)?;
        }
        self.id = None;
        Ok(())
    }
}
impl Drop for EncodingReservation {
    fn drop(&mut self) {
        if self
            .id
            .take()
            .is_some_and(|id| self.governor.release(id).is_err())
        {
            tracing::error!("final model encoding reservation release failed");
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    format: String,
    specification: String,
    artifact_set: String,
    preparation_revision: u64,
    model_chat_template_sha256: String,
    template_origin: String,
    worker_checksum: String,
    report: EncodingReport,
    qualification: String,
}
fn measurement(
    binding: &BoundFinalEncoding,
    input: FinalRequestInput<'_>,
    bytes: &str,
) -> Result<FinalRequestMeasurement, FinalRequestError> {
    let selection = binding.metadata();
    let prefix = binding.leading_bytes();
    let value = brassclaw_skills::value_contract::strict_json(
        bytes,
        brassclaw_skills::value_contract::EXECUTION_DATA_LIMITS,
    )
    .map_err(|_| FinalRequestError::Measurement)?;
    let observed: Observation =
        serde_json::from_value(value).map_err(|_| FinalRequestError::Measurement)?;
    let report = &observed.report;
    if observed.format != "brassclaw-model-encoding-observation/1"
        || observed.qualification != "offline-measurement-only"
        || observed.specification != selection.specification
        || observed.artifact_set != selection.artifact_set
        || observed.preparation_revision != selection.preparation_revision
        || observed.worker_checksum != selection.worker_sha256
        || observed.template_origin != "model-artifact-template"
        || observed.model_chat_template_sha256 != selection.template_sha256
        || report.format != "brassclaw-model-encoding/3"
        || !report.truncation_disabled
        || !report.padding_disabled
        || report.tokenizer_sha256 != selection.tokenizer_sha256
        || report.tokenizer_config_sha256 != selection.tokenizer_config_sha256
        || report.chat_template_sha256 != selection.template_sha256
        || report.probes.len() != 1
    {
        return Err(FinalRequestError::Binding);
    }
    let probe = &report.probes[0];
    let split = probe.partition.as_ref().ok_or(FinalRequestError::Binding)?;
    let wire = probe
        .wire_input
        .as_ref()
        .ok_or(FinalRequestError::Binding)?;
    let body_sha256: [u8; 32] = Sha256::digest(input.body).into();
    if input.provider_id != selection.provider_id
        || input.model != selection.model
        || wire.request_sha256 != hex::encode(body_sha256)
        || wire.mapping_sha256 != checksum(selection.mapping_json.as_bytes())
        || wire.model != selection.model
        || wire.reserved_output_tokens == 0
        || split.prefix_start != 0
        || split.prefix_end != prefix.len()
        || split.prefix_bytes_sha256 != checksum(prefix.as_bytes())
        || split.prefix_token_ids_sha256 != binding.leading_token_digest()
        || matches!(binding, BoundFinalEncoding::Workflow(_)) && split.prefix_tokens != 0
        || split.qualification != "verified-leading-render-bytes-only"
        || split.render_context_sha256.as_ref() != Some(&wire.render_context_sha256)
        || split.prefix_tokens.checked_add(split.prompt_tokens) != Some(probe.token_count)
        || probe.rendered_bytes < prefix.len() as u64
        || report.common_prefix_tokens != probe.token_count
        || report.common_prefix_sha256 != probe.token_ids_sha256
        || [
            &wire.render_context_sha256,
            &probe.rendered_sha256,
            &probe.token_ids_sha256,
            &split.prefix_token_ids_sha256,
            &split.prompt_token_ids_sha256,
        ]
        .iter()
        .any(|v| !hash(v))
    {
        return Err(FinalRequestError::Binding);
    }
    Ok(FinalRequestMeasurement {
        body_sha256,
        provider_id: selection.provider_id.to_owned(),
        model: selection.model.to_owned(),
        reserved_output_tokens: wire.reserved_output_tokens,
        counts: ModelRequestTokenCounts {
            prefix_tokens: split.prefix_tokens,
            prompt_tokens: split.prompt_tokens,
            input_tokens: probe.token_count,
        },
    })
}
#[async_trait]
impl FinalRequestMeasurer for ContainedFinalRequestMeasurer {
    async fn measure(
        &self,
        input: FinalRequestInput<'_>,
    ) -> Result<FinalRequestMeasurement, FinalRequestError> {
        let selection = self.selection.metadata();
        if input.provider_id != selection.provider_id || input.model != selection.model {
            return Err(FinalRequestError::Target);
        }
        match &self.selection {
            BoundFinalEncoding::Prefix(s) => {
                self.verifier
                    .verify(s, &self.scope, input.headers, input.body)
                    .await?
            }
            BoundFinalEncoding::Workflow(s) => {
                self.verifier
                    .verify_workflow(s, &self.scope, input.headers, input.body)
                    .await?
            }
        }
        let size = input
            .body
            .len()
            .checked_add(selection.mapping_json.len())
            .and_then(|n| n.checked_add(self.selection.leading_bytes().len()))
            .and_then(|n| n.checked_add(selection.model.len()))
            .ok_or(FinalRequestError::Measurement)?;
        if size > brassclaw_model_encoding::MAX_INPUT_BYTES {
            return Err(FinalRequestError::Measurement);
        }
        let body = std::str::from_utf8(input.body).map_err(|_| FinalRequestError::Body)?;
        let probes = [Probe::WireChat {
            request_json: body.to_owned(),
            mapping_json: selection.mapping_json.to_owned(),
            expected_model: selection.model.to_owned(),
            leading_prefix: self.selection.leading_bytes().to_owned(),
        }];
        let probes = serde_json::to_string(&probes).map_err(|_| FinalRequestError::Measurement)?;
        let reservation = EncodingReservation::reserve(self.governor.clone(), self.scope.clone())?;
        // None is mandatory: the target's retained DEFAULT template is used.
        let result = self
            .backend
            .encode(
                selection.specification,
                selection.artifact_set,
                &probes,
                None,
                &self.scope,
            )
            .await;
        let output = match result {
            Ok(output) => {
                reservation.finish(Some(&output.usage))?;
                output.output
            }
            Err(error) => {
                reservation.finish(error.usage())?;
                return Err(FinalRequestError::Measurement);
            }
        };
        measurement(
            &self.selection,
            input,
            output.as_str().ok_or(FinalRequestError::Measurement)?,
        )
    }
}

#[cfg(all(test, feature = "root-llm-provider", feature = "skills-db"))]
#[path = "final_model_encoding_tests.rs"]
mod tests;
