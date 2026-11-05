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
    LoopModelResponse, LoopPromptBundleRequest, LoopPromptMessageSetId, PreparedLoopPromptMessage,
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
    /// Native related work retains its own request/response identity under this
    /// original selected invocation. No caller-controlled observation grants
    /// a new effect or reuses the primary response record.
    async fn prepare_related_model_work(
        &self,
        _work: &brassclaw_turns::run_profile::ModelWorkRequest,
        _request_bytes: &str,
    ) -> Result<Arc<dyn RecipeModelResponseRecord>, AgentLoopHostError> {
        Err(AgentLoopHostError::new(
            AgentLoopHostErrorKind::Unimplemented,
            "related native model response custody is unavailable",
        ))
    }

    /// Read only the explicit output setting captured in this original
    /// invocation's verified arguments. Never refill it from current settings.
    fn requested_output_tokens(&self) -> Option<std::num::NonZeroU32> {
        None
    }

    /// Exact prefix/workflow reference already committed for this occurrence.
    fn retained_model_input_reference(
        &self,
    ) -> &brassclaw_turns::run_profile::LoopModelInputReference;
    /// Verify the original claim/invocation after all preparation awaits and
    /// before its first submission. A successful check authorizes no replay.
    async fn before_provider_submission(&self) -> Result<(), AgentLoopHostError>;
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
    selection: PrefixSelection,
}
#[derive(Clone)]
enum PrefixSelection {
    Inline(ResolvedRecipeModelPrefix),
    FinalBoundary(Arc<dyn RecipeModelPrefixSource>),
}
#[derive(Clone)]
pub struct ResolvedRecipeModelPrefix {
    reference: String,
    input: ResolvedRecipeModelInput,
}

pub use brassclaw_turns::run_profile::RecipePrefixSelection;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecipePrefixInsertion {
    Inline,
    FinalBoundary,
}

#[derive(Clone)]
enum ResolvedRecipeModelInput {
    Prefix {
        selection: RecipePrefixSelection,
        content: Arc<str>,
        insertion: RecipePrefixInsertion,
    },
    PrefixFreeWorkflow,
}

fn invalid_prefix() -> AgentLoopHostError {
    AgentLoopHostError::new(
        AgentLoopHostErrorKind::Unavailable,
        "selected model input binding is unavailable",
    )
}

impl ResolvedRecipeModelPrefix {
    pub fn prefix(
        reference: String,
        content: Arc<str>,
        selection: RecipePrefixSelection,
        insertion: RecipePrefixInsertion,
    ) -> Result<Self, AgentLoopHostError> {
        if !valid_reference(&reference)
            || content.trim().is_empty()
            || selection == RecipePrefixSelection::PrefixFreeWorkflow
        {
            return Err(invalid_prefix());
        }
        Ok(Self {
            reference,
            input: ResolvedRecipeModelInput::Prefix {
                selection,
                content,
                insertion,
            },
        })
    }

    /// Structural carrier only. The native workflow owner must qualify this
    /// exact reference and the final guard must verify the same binding.
    /// Construction supplies no approval, provider access or guard bypass.
    pub fn prefix_free_workflow(reference: String) -> Result<Self, AgentLoopHostError> {
        if !valid_reference(&reference) {
            return Err(invalid_prefix());
        }
        Ok(Self {
            reference,
            input: ResolvedRecipeModelInput::PrefixFreeWorkflow,
        })
    }
    pub fn reference(&self) -> &str {
        &self.reference
    }
    pub fn input_reference(&self) -> brassclaw_turns::run_profile::LoopModelInputReference {
        brassclaw_turns::run_profile::LoopModelInputReference {
            selection: self.selection(),
            reference: self.reference.clone(),
        }
    }
    pub fn selection(&self) -> RecipePrefixSelection {
        match &self.input {
            ResolvedRecipeModelInput::Prefix { selection, .. } => *selection,
            ResolvedRecipeModelInput::PrefixFreeWorkflow => {
                RecipePrefixSelection::PrefixFreeWorkflow
            }
        }
    }
    pub fn prefix_content(&self) -> Option<&Arc<str>> {
        match &self.input {
            ResolvedRecipeModelInput::Prefix { content, .. } => Some(content),
            ResolvedRecipeModelInput::PrefixFreeWorkflow => None,
        }
    }
    fn inline_content(&self) -> Option<&Arc<str>> {
        match &self.input {
            ResolvedRecipeModelInput::Prefix {
                content,
                insertion: RecipePrefixInsertion::Inline,
                ..
            } => Some(content),
            _ => None,
        }
    }
}
fn valid_reference(reference: &str) -> bool {
    !reference.trim().is_empty() && !reference.chars().any(char::is_control)
}
#[async_trait]
pub trait RecipeModelPrefixSource: Send + Sync {
    /// Native, task-captured immutable selection. Never an operator string or
    /// current-role lookup from inside a running Recipe.
    async fn resolve(&self) -> Result<ResolvedRecipeModelPrefix, AgentLoopHostError>;
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
        Ok(Self {
            selection: PrefixSelection::Inline(ResolvedRecipeModelPrefix::prefix(
                reference,
                content,
                RecipePrefixSelection::Selected,
                RecipePrefixInsertion::Inline,
            )?),
        })
    }
    pub fn at_final_boundary(source: Arc<dyn RecipeModelPrefixSource>) -> Self {
        Self {
            selection: PrefixSelection::FinalBoundary(source),
        }
    }
    pub async fn resolve(&self) -> Result<ResolvedRecipeModelPrefix, AgentLoopHostError> {
        let resolved = match &self.selection {
            PrefixSelection::Inline(prefix) => prefix.clone(),
            PrefixSelection::FinalBoundary(source) => source.resolve().await?,
        };
        if matches!(&self.selection, PrefixSelection::FinalBoundary(_))
            && resolved.inline_content().is_some()
        {
            return Err(invalid_prefix());
        }
        Ok(resolved)
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
    #[serde(default, deserialize_with = "positive_output_reservation")]
    max_output_tokens: Option<std::num::NonZeroU32>,
}
fn positive_output_reservation<'de, D>(
    deserializer: D,
) -> Result<Option<std::num::NonZeroU32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // Called only for a present field: explicit null is not missing.
    std::num::NonZeroU32::deserialize(deserializer).map(Some)
}

/// Parse the same complete finite native prompt profile used by the model
/// capability. Returns data only, never permission or workflow qualification.
pub fn model_output_reservation(
    arguments: &Value,
) -> Result<Option<std::num::NonZeroU32>, FirstPartyCapabilityError> {
    let Arguments { prompt } = Arguments::deserialize(arguments).map_err(|_| invalid_input())?;
    Ok(prompt.max_output_tokens)
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
fn model_host_failure(stage: &'static str, error: AgentLoopHostError) -> AgentLoopHostError {
    tracing::warn!(stage,kind=?error.kind,safe_summary=%error.safe_summary,"selected model host preparation failed");
    error
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
        let context = self.host.run_context().ok_or_else(|| {
            FirstPartyCapabilityError::new(RuntimeDispatchErrorKind::PolicyDenied)
        })?;
        if context.trusted_internal_turn || self.host.selected_model_route().is_err() {
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
        let prefix = self
            .prefix
            .resolve()
            .await
            .map_err(|e| host_error(e, ResourceUsage::default()))?;
        let response_owner: Arc<dyn RecipeModelResponseRecord> = Arc::from(
            self.responses
                .prepare(
                    request.scope.invocation_id,
                    &request.input,
                    prefix.reference(),
                )
                .await
                .map_err(|e| {
                    host_error(
                        model_host_failure("response-owner", e),
                        ResourceUsage::default(),
                    )
                })?,
        );
        let invocation = request.scope.invocation_id;
        let work_id = brassclaw_turns::run_profile::ModelWorkId::try_from(invocation.as_uuid())
            .map_err(|_| invalid_input())?;
        drop(request);
        let started = Instant::now();
        let result = async {
            let surface = self
                .host
                .visible_capabilities()
                .await
                .map_err(|e| model_host_failure("capability-surface", e))?;
            let count = prompt
                .chat_history
                .len()
                .checked_add(2)
                .ok_or_else(capacity)?;
            let mut messages = Vec::new();
            messages.try_reserve_exact(count).map_err(|_| capacity())?;
            if let Some(content) = prefix.inline_content() {
                messages.push(PreparedLoopPromptMessage {
                    role: LoopInlineMessageRole::System,
                    content: content.to_string(),
                });
            }
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
                    message_set_id: LoopPromptMessageSetId::from_uuid(invocation.as_uuid()),
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
                .await
                .map_err(|e| model_host_failure("prompt-materialization", e))?;
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
                .map_err(|e| model_host_failure("host-model-operation", e))
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

#[cfg(test)]
mod prefix_selection_tests {
    use super::*;
    struct Source(ResolvedRecipeModelPrefix);
    #[async_trait]
    impl RecipeModelPrefixSource for Source {
        async fn resolve(&self) -> Result<ResolvedRecipeModelPrefix, AgentLoopHostError> {
            Ok(self.0.clone())
        }
    }
    #[tokio::test]
    async fn prefix_semantics_and_physical_boundary_remain_independent() {
        for selection in [
            RecipePrefixSelection::Default,
            RecipePrefixSelection::Selected,
        ] {
            let prefix = ResolvedRecipeModelPrefix::prefix(
                "exact-retained-prefix".into(),
                "content".into(),
                selection,
                RecipePrefixInsertion::FinalBoundary,
            )
            .unwrap();
            let bound = RecipeModelPrefix::at_final_boundary(Arc::new(Source(prefix)))
                .resolve()
                .await
                .unwrap();
            assert_eq!(bound.input_reference().selection, selection);
            assert_eq!(bound.prefix_content().map(|s| s.as_ref()), Some("content"));
            assert!(bound.inline_content().is_none());
        }
        let free =
            ResolvedRecipeModelPrefix::prefix_free_workflow("exact-retained-workflow".into())
                .unwrap();
        let bound = RecipeModelPrefix::at_final_boundary(Arc::new(Source(free)))
            .resolve()
            .await
            .unwrap();
        assert_eq!(bound.selection(), RecipePrefixSelection::PrefixFreeWorkflow);
        assert_eq!(bound.reference(), "exact-retained-workflow");
        assert!(bound.prefix_content().is_none());
        assert!(bound.inline_content().is_none());
        let inline = ResolvedRecipeModelPrefix::prefix(
            "historical-inline".into(),
            "content".into(),
            RecipePrefixSelection::Selected,
            RecipePrefixInsertion::Inline,
        )
        .unwrap();
        assert!(
            RecipeModelPrefix::at_final_boundary(Arc::new(Source(inline)))
                .resolve()
                .await
                .is_err()
        );
    }
    #[test]
    fn missing_or_empty_present_prefix_cannot_become_intentional_absence() {
        for reference in ["", " ", "bad\nreference"] {
            assert!(ResolvedRecipeModelPrefix::prefix_free_workflow(reference.into()).is_err());
        }
        for content in ["", " "] {
            assert!(
                ResolvedRecipeModelPrefix::prefix(
                    "retained".into(),
                    content.into(),
                    RecipePrefixSelection::Default,
                    RecipePrefixInsertion::FinalBoundary,
                )
                .is_err()
            );
        }
        assert!(
            ResolvedRecipeModelPrefix::prefix(
                "retained".into(),
                "nonempty".into(),
                RecipePrefixSelection::PrefixFreeWorkflow,
                RecipePrefixInsertion::FinalBoundary,
            )
            .is_err()
        );
        let legacy = RecipeModelPrefix::new("historical".into(), "original".into()).unwrap();
        let PrefixSelection::Inline(legacy) = legacy.selection else {
            panic!("legacy placement changed")
        };
        assert_eq!(legacy.selection(), RecipePrefixSelection::Selected);
        assert_eq!(
            legacy.inline_content().map(|s| s.as_ref()),
            Some("original")
        );
    }
}

#[cfg(test)]
mod output_reservation_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn output_setting_is_optional_but_present_values_use_the_whole_native_prompt_contract() {
        let legacy = json!({"prompt":{"user_query":"data","chat_history":[]}});
        assert_eq!(model_output_reservation(&legacy).unwrap(), None);
        for count in [1, 37, u32::MAX] {
            let value =
                json!({"prompt":{"user_query":"data","chat_history":[],"max_output_tokens":count}});
            assert_eq!(
                model_output_reservation(&value)
                    .unwrap()
                    .map(std::num::NonZeroU32::get),
                Some(count)
            );
        }
        for invalid in [
            json!(null),
            json!(0),
            json!(-1),
            json!(true),
            json!(1.0),
            json!("37"),
            json!(u64::from(u32::MAX) + 1),
        ] {
            let value = json!({"prompt":{"user_query":"data","chat_history":[],"max_output_tokens":invalid}});
            assert!(model_output_reservation(&value).is_err());
        }
        for invalid in [
            json!({"prompt":{"chat_history":[],"max_output_tokens":37}}),
            json!({"prompt":{"user_query":"data","chat_history":null,"max_output_tokens":37}}),
            json!({"prompt":{"user_query":"data","chat_history":[],"max_output_tokens":37,"extra":true}}),
            json!({"prompt":{"user_query":"data","chat_history":[{"role":"system","content":"fixed","extra":true}],"max_output_tokens":37}}),
        ] {
            assert!(model_output_reservation(&invalid).is_err());
        }
    }
}
