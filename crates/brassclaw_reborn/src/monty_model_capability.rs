//! One selected model operation behind the kernel. The caller owns catalogue
//! qualification and immutable prefix selection; this adapter owns neither a
//! Recipe loop nor tool-request execution. There is no root JSON control port.
use std::{sync::Arc, time::Instant};

use async_trait::async_trait;
use brassclaw_host_api::{InvocationId, ResourceUsage, RuntimeDispatchErrorKind};
use brassclaw_host_runtime::{
    FirstPartyCapabilityError, FirstPartyCapabilityHandler, FirstPartyCapabilityRequest,
    FirstPartyCapabilityResult,
};
use brassclaw_turns::run_profile::{
    AgentLoopHostError, AgentLoopHostErrorKind, LoopInlineMessageRole, LoopModelRequest,
    LoopModelResponse, LoopPromptBundleRequest, PreparedLoopPromptMessage,
    PreparedLoopPromptRequest, PromptMode,
};
use serde::Deserialize;
use serde_json::Value;

use crate::monty_task_host::MontyTaskHost;

pub const KOHAI_COMPLETE_CAPABILITY_ID: &str = "host.kohai_complete";

/// Trusted durable owner of the original dispatch intent. The ID is the kernel
/// ResourceScope identity already committed by the selected Tool dispatcher,
/// never an argument, grant or fresh retry identity supplied by Python.
#[async_trait]
pub trait RecipeModelResponseStore: Send + Sync {
    /// Supervisor settlement must retain the task owner while this is true.
    fn has_uncommitted_results(&self) -> bool;

    /// Trusted supervisor-only persistence of an already received occurrence.
    /// No response body, new effect, retry identity or continuation is accepted.
    async fn reconcile_received_response(
        &self,
        invocation: InvocationId,
    ) -> Result<(), AgentLoopHostError>;

    /// Verify the exact committed invocation, arguments, prefix and live attempt
    /// before preparation/provider I/O. Return an opaque owner of that occurrence.
    async fn prepare(
        &self,
        invocation: InvocationId,
        arguments: &Value,
        prefix_reference: &str,
    ) -> Result<Box<dyn RecipeModelResponseRecord>, AgentLoopHostError>;
}

#[async_trait]
pub trait RecipeModelResponseRecord: Send + Sync {
    fn receive_provider_wire_observation(&self, response: Value) -> Result<(), AgentLoopHostError>;
    async fn persist_provider_wire_observation(&self) -> Result<(), AgentLoopHostError>;

    /// Own the original provider adapter response before interpretation, including
    /// length/policy/validation failures. No fabricated LoopModelResponse.
    fn receive_provider_observation(&self, response: Value) -> Result<(), AgentLoopHostError>;
    async fn persist_provider_observation(&self) -> Result<(), AgentLoopHostError>;

    /// Take custody of the complete actual response, including proposals and
    /// unknown usage, synchronously before any persistence await. The owner must
    /// keep the actual response for reconciliation; never manufacture success
    /// or authorize another provider call. Memory alone is no crash checkpoint.
    fn receive_response(&self, response: LoopModelResponse) -> Result<(), AgentLoopHostError>;

    /// Persist the synchronously owned observation. Cancelling this future
    /// retains it with the record's owner for supervised reconciliation. Return
    /// exact encoded response bytes only after durable retention.
    async fn persist_response(&self) -> Result<String, AgentLoopHostError>;
}

/// Captured catalogue data. Construction does not approve or activate a Skill.
/// No Debug/serde surface can accidentally expose the full prefix in a log.
#[derive(Clone)]
pub struct RecipeModelPrefix {
    reference: String,
    content: Arc<str>,
}
impl RecipeModelPrefix {
    pub fn new(reference: String, content: Arc<str>) -> Result<Self, AgentLoopHostError> {
        if reference.is_empty()
            || reference.chars().any(char::is_control)
            || content.trim().is_empty()
        {
            return Err(AgentLoopHostError::new(
                AgentLoopHostErrorKind::InvalidInvocation,
                "selected model prefix is unavailable",
            ));
        }
        Ok(Self { reference, content })
    }
}

struct TaskModelCapability {
    host: Arc<MontyTaskHost>,
    prefix: RecipeModelPrefix,
    responses: Arc<dyn RecipeModelResponseStore>,
}
impl MontyTaskHost {
    /// Bind one task's model usage to its captured prefix and durable response
    /// owner. Only register this behind the actual retained kernel and qualified
    /// exact usage; this constructor itself supplies no component approval.
    pub fn model_capability(
        self: &Arc<Self>,
        prefix: RecipeModelPrefix,
        responses: Arc<dyn RecipeModelResponseStore>,
    ) -> Arc<impl FirstPartyCapabilityHandler + 'static + use<>> {
        Arc::new(TaskModelCapability {
            host: self.clone(),
            prefix,
            responses,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    prompt: Prompt,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Prompt {
    user_query: String,
    chat_history: Vec<Message>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Message {
    role: LoopInlineMessageRole,
    content: String,
}

fn invalid_input() -> FirstPartyCapabilityError {
    FirstPartyCapabilityError::new(RuntimeDispatchErrorKind::InputEncode)
}
fn host_error(error: AgentLoopHostError, usage: ResourceUsage) -> FirstPartyCapabilityError {
    if let Some(diagnostic_ref) = &error.diagnostic_ref {
        tracing::warn!(
            kind = ?error.kind,
            ?diagnostic_ref,
            "selected model host failure retains its private diagnostic reference"
        );
    }
    FirstPartyCapabilityError::with_safe_summary(
        RuntimeDispatchErrorKind::OperationFailed,
        error.kind.as_str(),
    )
    .with_usage(usage)
}

#[async_trait]
impl FirstPartyCapabilityHandler for TaskModelCapability {
    async fn dispatch(
        &self,
        request: FirstPartyCapabilityRequest,
    ) -> Result<FirstPartyCapabilityResult, FirstPartyCapabilityError> {
        let context = self.host.run_context();
        if context.trusted_internal_turn
            || context
                .resolved_model_route
                .as_ref()
                .is_none_or(|route| route.validate().is_err())
        {
            return Err(FirstPartyCapabilityError::new(
                RuntimeDispatchErrorKind::PolicyDenied,
            ));
        }
        if request.capability_id.as_str() != KOHAI_COMPLETE_CAPABILITY_ID
            || request.scope.tenant_id != context.scope.tenant_id
            || request.scope.agent_id != context.scope.agent_id
            || request.scope.project_id != context.scope.project_id
            || request.scope.thread_id.as_ref() != Some(&context.thread_id)
            || context
                .actor
                .as_ref()
                .is_none_or(|actor| actor.user_id != request.scope.user_id)
            || request.scope.invocation_id.as_uuid().is_nil()
        {
            return Err(FirstPartyCapabilityError::new(
                RuntimeDispatchErrorKind::PolicyDenied,
            ));
        }
        self.host
            .check_dispatch_open()
            .map_err(|e| host_error(e, ResourceUsage::default()))?;
        // This finite profile is complete data, not the obsolete permissive
        // placeholder/empty-default bridge. Unknown roles/fields fail as a whole.
        let Arguments { prompt } =
            Arguments::deserialize(&request.input).map_err(|_| invalid_input())?;
        let response_owner: Arc<dyn RecipeModelResponseRecord> = Arc::from(
            self.responses
                .prepare(
                    request.scope.invocation_id,
                    &request.input,
                    &self.prefix.reference,
                )
                .await
                .map_err(|e| host_error(e, ResourceUsage::default()))?,
        );
        let invocation = request.scope.invocation_id;
        let work_id = brassclaw_turns::run_profile::ModelWorkId::try_from(invocation.as_uuid())
            .map_err(|_| invalid_input())?;
        drop(request);
        let started = Instant::now();
        let result = async {
            let surface = self.host.visible_capabilities().await?;
            let count = prompt
                .chat_history
                .len()
                .checked_add(2)
                .ok_or_else(capacity)?;
            let mut messages = Vec::new();
            messages.try_reserve_exact(count).map_err(|_| capacity())?;
            messages.push(PreparedLoopPromptMessage {
                role: LoopInlineMessageRole::System,
                content: self.prefix.content.to_string(),
            });
            messages.extend(prompt.chat_history.into_iter().map(|message| {
                PreparedLoopPromptMessage {
                    role: message.role,
                    content: message.content,
                }
            }));
            messages.push(PreparedLoopPromptMessage {
                role: LoopInlineMessageRole::User,
                content: prompt.user_query,
            });
            let bundle = self
                .host
                .build_prepared_prompt_bundle(PreparedLoopPromptRequest {
                    invocation_id: invocation,
                    metadata: LoopPromptBundleRequest {
                        mode: PromptMode::TextOnly,
                        context_cursor: None,
                        surface_version: Some(surface.version),
                        checkpoint_state_ref: None,
                        max_messages: None,
                        capability_view: None,
                        inline_messages: vec![],
                        recipe_hint: None,
                    },
                    messages,
                })
                .await?;
            // Existing host mediation retains hooks, provider/model policy,
            // accounting, interceptor, prompt authority and cancellation fences.
            // Proposals remain data; this adapter executes no requested Tools.
            self.host
                .stream_model_retaining(
                    invocation,
                    LoopModelRequest {
                        work_id: Some(work_id),
                        dispatch_policy:
                            brassclaw_turns::run_profile::LoopModelDispatchPolicy::SingleSubmission,
                        messages: bundle.messages,
                        surface_version: bundle.surface_version,
                        model_preference: None,
                        capability_view: None,
                        resolved_messages: None,
                    },
                    response_owner,
                )
                .await
        }
        .await;
        let usage = ResourceUsage {
            wall_clock_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            ..ResourceUsage::default()
        };
        result
            .map(|bytes| FirstPartyCapabilityResult::new(Value::String(bytes), usage.clone()))
            .map_err(|error| host_error(error, usage))
    }
}
fn capacity() -> AgentLoopHostError {
    AgentLoopHostError::new(
        AgentLoopHostErrorKind::Unavailable,
        "selected model prompt capacity unavailable",
    )
}
