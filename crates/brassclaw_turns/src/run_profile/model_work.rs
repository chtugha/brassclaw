use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    host::{
        AgentLoopHostErrorKind, LoopModelRequest, LoopModelRouteSnapshot, LoopModelUsage,
        LoopRunContext,
    },
    model::{LoopModelGatewayError, ModelCallOutcome},
    refs::ModelProfileId,
    system_inference::{
        SystemInferenceError, SystemInferenceRequest, SystemInferenceResponse,
        SystemInferenceTaskId, SystemPromptSource, SystemTaskKind,
    },
};

const MODEL_WORK_ESTIMATED_CHARS_PER_TOKEN: u64 = 4;

/// Host-owned identity of one logical model operation, never a Tool grant.
/// Keep it through preparation, waits, accounting and reconciliation. A new
/// admitted operation receives a new identity even when its prompt is equal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct ModelWorkId(Uuid);

impl ModelWorkId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
    pub fn as_uuid(self) -> Uuid {
        self.0
    }
}
impl Default for ModelWorkId {
    fn default() -> Self {
        Self::new()
    }
}
impl TryFrom<Uuid> for ModelWorkId {
    type Error = &'static str;
    fn try_from(id: Uuid) -> Result<Self, Self::Error> {
        if id.is_nil() {
            return Err("model work identity must not be nil");
        }
        Ok(Self(id))
    }
}
impl From<SystemInferenceTaskId> for ModelWorkId {
    fn from(id: SystemInferenceTaskId) -> Self {
        Self(id.as_uuid())
    }
}
impl<'de> Deserialize<'de> for ModelWorkId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::try_from(Uuid::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// Model-backed unit of work that needs the host policy/accounting envelope.
///
/// This is deliberately narrower than [`LoopModelRequest`]: assistant dispatch
/// and host-owned system inference share spend/policy handling, but keep their
/// execution semantics separate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelWorkRequest {
    /// Absent only in legacy metadata. Admission must reject it, never invent a
    /// new identity while restoring or settling an existing operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_id: Option<ModelWorkId>,
    pub kind: ModelWorkKind,
    pub model_profile_id: ModelProfileId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_model_route: Option<LoopModelRouteSnapshot>,
    pub estimated_input_tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_output_tokens: Option<u64>,
}

impl ModelWorkRequest {
    pub fn for_assistant(context: &LoopRunContext, request: &LoopModelRequest) -> Self {
        let model_profile_id = request
            .model_preference
            .as_ref()
            .unwrap_or(&context.resolved_run_profile.model_profile_id)
            .clone();
        let estimated_input_tokens = request
            .messages
            .iter()
            .map(|message| {
                (message.content_ref.as_str().len() as u64) / MODEL_WORK_ESTIMATED_CHARS_PER_TOKEN
            })
            .sum::<u64>()
            .max(64);
        Self {
            work_id: request.work_id,
            kind: ModelWorkKind::Assistant,
            model_profile_id,
            resolved_model_route: context.resolved_model_route.clone(),
            estimated_input_tokens,
            estimated_output_tokens: None,
        }
    }

    pub fn for_system_inference(
        context: &LoopRunContext,
        request: &SystemInferenceRequest,
    ) -> Self {
        let estimated_chars = request
            .identity
            .system_prompt
            .len()
            .saturating_add(request.input_text.len()) as u64;
        Self {
            work_id: Some(request.task_id.into()),
            kind: ModelWorkKind::SystemInference {
                task_kind: request.identity.task_kind,
                prompt_source: request.identity.prompt_source.clone(),
            },
            model_profile_id: context.resolved_run_profile.model_profile_id.clone(),
            resolved_model_route: context.resolved_model_route.clone(),
            estimated_input_tokens: (estimated_chars / MODEL_WORK_ESTIMATED_CHARS_PER_TOKEN)
                .max(64),
            estimated_output_tokens: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ModelWorkKind {
    Assistant,
    /// Host-side review of the resolved primary-model prompt. This is separately
    /// authorized/accounted provider work, not an assistant run or Tool grant.
    SempaiReview,
    SystemInference {
        task_kind: SystemTaskKind,
        prompt_source: SystemPromptSource,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelWorkOutcome {
    Success(ModelWorkUsage),
    Failure(AgentLoopHostErrorKind),
}

impl ModelWorkOutcome {
    pub fn from_model_call(outcome: ModelCallOutcome<'_>) -> Self {
        match outcome {
            ModelCallOutcome::Success(response) => Self::Success(ModelWorkUsage {
                // Transport chunking is independent of tokenization. Preserve
                // the provider observation, including zero and unknown usage.
                token_usage: response.usage,
                output_bytes: response
                    .chunks
                    .iter()
                    .map(|chunk| chunk.safe_text_delta.len() as u64)
                    .sum(),
                wall_clock_ms: 0,
            }),
            ModelCallOutcome::Failure(error) => Self::from_gateway_error(error),
        }
    }

    pub fn from_gateway_error(error: &LoopModelGatewayError) -> Self {
        Self::Failure(error.kind)
    }

    pub fn from_system_inference_result(
        result: &Result<SystemInferenceResponse, SystemInferenceError>,
    ) -> Self {
        match result {
            Ok(response) => Self::Success(ModelWorkUsage {
                token_usage: response.usage,
                output_bytes: response.output_text.len() as u64,
                wall_clock_ms: response.elapsed_ms,
            }),
            Err(SystemInferenceError::Cancelled) => {
                Self::Failure(AgentLoopHostErrorKind::Cancelled)
            }
            Err(SystemInferenceError::InputTooLarge) => {
                Self::Failure(AgentLoopHostErrorKind::InvalidInvocation)
            }
            Err(SystemInferenceError::Timeout) => {
                Self::Failure(AgentLoopHostErrorKind::Unavailable)
            }
            Err(SystemInferenceError::Failed { .. }) => {
                Self::Failure(AgentLoopHostErrorKind::Unavailable)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelWorkUsage {
    /// Complete original provider meter. Unknown is distinct from reported zero;
    /// input/cache counters must survive assistant and system-work handoffs.
    pub token_usage: Option<LoopModelUsage>,
    pub output_bytes: u64,
    pub wall_clock_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_profile::{
        AssistantReply, LoopModelResponse, LoopModelUsage, ModelStreamChunk, ParentLoopOutput,
    };

    #[test]
    fn work_identity_survives_wire_roundtrip_and_legacy_data_stays_unidentified() {
        let original = LoopModelRequest::default();
        let next = LoopModelRequest::default();
        assert_ne!(original.work_id, next.work_id);
        let mut wire = serde_json::to_value(&original).unwrap();
        assert_eq!(
            serde_json::from_value::<LoopModelRequest>(wire.clone()).unwrap(),
            original
        );
        wire.as_object_mut().unwrap().remove("work_id");
        assert_eq!(
            serde_json::from_value::<LoopModelRequest>(wire.clone())
                .unwrap()
                .work_id,
            None
        );
        wire["work_id"] = serde_json::json!(Uuid::nil());
        assert!(serde_json::from_value::<LoopModelRequest>(wire).is_err());
        assert!(
            serde_json::from_value::<SystemInferenceTaskId>(serde_json::json!(Uuid::nil()))
                .is_err()
        );
        let system = SystemInferenceTaskId::new();
        let id = ModelWorkId::from(system);
        assert_eq!(id.as_uuid(), system.as_uuid());
        assert_eq!(
            serde_json::from_str::<ModelWorkId>(&serde_json::to_string(&id).unwrap()).unwrap(),
            id
        );
    }

    #[test]
    fn model_work_token_observation_is_independent_of_transport_chunking() {
        let mut response = LoopModelResponse {
            chunks: vec![],
            safe_reasoning_deltas: vec![],
            output: ParentLoopOutput::AssistantReply(AssistantReply {
                content: "same answer".into(),
            }),
            effective_model_profile_id: ModelProfileId::new("selected-model").unwrap(),
            usage: None,
        };
        for chunks in [vec![], vec!["same answer"], vec!["same ", "answer"]] {
            response.chunks = chunks
                .into_iter()
                .map(|text| ModelStreamChunk {
                    safe_text_delta: text.into(),
                })
                .collect();
            for tokens in [None, Some(0), Some(37)] {
                response.usage = tokens.map(|output_tokens| LoopModelUsage {
                    input_tokens: 12,
                    output_tokens,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                });
                let ModelWorkOutcome::Success(observed) =
                    ModelWorkOutcome::from_model_call(ModelCallOutcome::Success(&response))
                else {
                    panic!("successful observed response")
                };
                assert_eq!(observed.token_usage, response.usage);
                assert_eq!(
                    observed.token_usage.map(|usage| usage.output_tokens),
                    tokens
                );
            }
        }
    }

    #[test]
    fn system_work_preserves_optional_complete_meter_and_legacy_response_decoding() {
        use crate::run_profile::SystemInferenceTaskId;
        for usage in [
            None,
            Some(LoopModelUsage::default()),
            Some(LoopModelUsage {
                input_tokens: 17,
                output_tokens: 3,
                cache_read_input_tokens: u32::MAX,
                cache_creation_input_tokens: 11,
            }),
        ] {
            let response = SystemInferenceResponse {
                task_id: SystemInferenceTaskId::new(),
                output_text: "'ü\n{{vars.data}}".into(),
                elapsed_ms: 23,
                usage,
            };
            let wire = serde_json::to_value(&response).unwrap();
            assert_eq!(wire.get("usage").is_some(), usage.is_some());
            assert_eq!(
                serde_json::from_value::<SystemInferenceResponse>(wire).unwrap(),
                response
            );
            let ModelWorkOutcome::Success(observed) =
                ModelWorkOutcome::from_system_inference_result(&Ok(response.clone()))
            else {
                panic!("successful system observation required");
            };
            assert_eq!(observed.token_usage, usage);
            assert_eq!(observed.output_bytes, response.output_text.len() as u64);
            assert_eq!(observed.wall_clock_ms, 23);
        }
    }
}
