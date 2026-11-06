//! Task-scoped host operations for the global Monty hosting cutover.
//!
//! This adapter owns the admitted handoff, rather than loading an engine Thread.
//! It executes one requested host operation at a time. Recipe selection, model
//! iteration, tool sequencing and completion remain Python's responsibility.
//! The instance service must retain this adapter across waits and fence the
//! attempt before dropping it; ownership alone is not a cancellation handshake.

use parking_lot::Mutex;
use std::sync::Arc;

use crate::monty_attempt_fence::{MontyHostCall, MontyHostCallId, MontyTaskFence};

use brassclaw_turns::{
    LoopMessageRef,
    run_profile::{
        AgentLoopDriverHost, AgentLoopDriverRunRequest, AgentLoopHostError, AgentLoopHostErrorKind,
        AppendCapabilityResultRef, CapabilityInvocation, CapabilityOutcome,
        FinalizeAssistantMessage, LoopModelRequest, LoopModelResponse, LoopPromptBundle,
        LoopPromptBundleRequest, LoopRunContext, MontyTaskAttempt, MontyTaskHandoff,
        PreparedLoopPromptRequest, VisibleCapabilityRequest, VisibleCapabilitySurface,
    },
};

/// Existing reply primitive's explicit dispatch identity. Its Python callable
/// remains host.post_reply; registration/selection must bind them explicitly.
pub const POST_REPLY_CAPABILITY_ID: &str = "host.post_reply";

/// Rust-owned task host. Never serialize this object or its claim identity into
/// Python. The hosting service addresses it using its own task routing token.
pub struct MontyTaskHost {
    origin: TaskOrigin,
    attempt: MontyTaskAttempt,
    fence: MontyTaskFence,
    memory_embeddings:
        std::sync::OnceLock<Arc<dyn brassclaw_host_runtime::RuntimeMemoryEmbeddingPort>>,
    published_reply: Mutex<Option<PublishedReply>>,
    withheld: Mutex<Vec<WithheldTaskPortResult>>,
    model_exchange_count: std::sync::atomic::AtomicU64,
    capability_dispatch_count: std::sync::atomic::AtomicU64,
}

enum TaskOrigin {
    Chat {
        request: Box<AgentLoopDriverRunRequest>,
        claimed_run: Option<Arc<brassclaw_turns::runner::ClaimedTurnRun>>,
        host: Arc<dyn AgentLoopDriverHost + Send + Sync>,
    },
    Private {
        claimed: Arc<brassclaw_turns::private_work::ClaimedPrivateWork>,
    },
}

/// Trusted supervisor evidence. Payloads have no Debug/serde surface and never
/// become ordinary task state. Taking this receipt acknowledges no effect.
pub struct WithheldTaskPortResult {
    pub call_id: MontyHostCallId,
    pub value: WithheldTaskPortValue,
}

pub enum WithheldTaskPortValue {
    VisibleCapabilities(VisibleCapabilitySurface),
    PromptBundle(LoopPromptBundle),
    ModelResponse(LoopModelResponse),
    RecordedModelResponse {
        invocation_id: brassclaw_host_api::InvocationId,
        response_bytes: String,
    },
    CapabilityOutcome(CapabilityOutcome),
    CapabilityResultReference(LoopMessageRef),
    FinalizedReplyReference(LoopMessageRef),
    PublishedReplyContent(String),
    RetainedToolOutcome(brassclaw_host_runtime::RuntimeCapabilityOutcome),
}

// Keep only the current finalized reply; durable transcript storage remains
// authoritative after restart. Never format this private content with Debug.
struct PublishedReply {
    reference: LoopMessageRef,
    content: String,
}

impl MontyTaskHost {
    pub fn new(handoff: MontyTaskHandoff) -> Self {
        let claimed_run = handoff.claimed_run().cloned();
        let (request, attempt, host) = handoff.into_parts();
        Self {
            origin: TaskOrigin::Chat {
                request: Box::new(request),
                claimed_run,
                host,
            },
            attempt,
            fence: MontyTaskFence::new(attempt),
            memory_embeddings: std::sync::OnceLock::new(),
            published_reply: Mutex::new(None),
            withheld: Mutex::new(Vec::new()),
            model_exchange_count: std::sync::atomic::AtomicU64::new(0),
            capability_dispatch_count: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Only the actual PG claim supplies original private identity. The local
    /// cancellation signal is fed by the existing worker heartbeat; durable
    /// admission still rechecks PG before every actual dispatch.
    pub fn new_private(claimed: Arc<brassclaw_turns::private_work::ClaimedPrivateWork>) -> Self {
        let attempt = claimed.attempt();
        Self {
            origin: TaskOrigin::Private { claimed },
            attempt,
            fence: MontyTaskFence::new(attempt),
            memory_embeddings: std::sync::OnceLock::new(),
            published_reply: Mutex::new(None),
            withheld: Mutex::new(Vec::new()),
            model_exchange_count: std::sync::atomic::AtomicU64::new(0),
            capability_dispatch_count: std::sync::atomic::AtomicU64::new(0),
        }
    }

    fn chat_host(&self) -> Result<&Arc<dyn AgentLoopDriverHost + Send + Sync>, AgentLoopHostError> {
        match &self.origin {
            TaskOrigin::Chat { host, .. } => Ok(host),
            TaskOrigin::Private { .. } => Err(AgentLoopHostError::new(
                AgentLoopHostErrorKind::PolicyDenied,
                "operation requires an actual chat origin",
            )),
        }
    }

    pub fn private_claim(&self) -> Option<&Arc<brassclaw_turns::private_work::ClaimedPrivateWork>> {
        match &self.origin {
            TaskOrigin::Private { claimed, .. } => Some(claimed),
            TaskOrigin::Chat { .. } => None,
        }
    }

    /// A typed projection of real identity, with no synthetic thread axis.
    pub fn work_context(&self) -> brassclaw_turns::private_work::PrivateExecutionContext {
        match &self.origin {
            TaskOrigin::Private { claimed, .. } => claimed.request().context().clone(),
            TaskOrigin::Chat { host, .. } => {
                let context = host.run_context();
                brassclaw_turns::private_work::PrivateExecutionContext {
                    tenant_id: context.scope.tenant_id.clone(),
                    agent_id: context.scope.agent_id.clone(),
                    project_id: context.scope.project_id.clone(),
                    actor: context.actor.clone(),
                }
            }
        }
    }

    fn cancellation_observed(&self) -> bool {
        match &self.origin {
            TaskOrigin::Chat { host, .. } => host.observe_cancellation().is_some(),
            TaskOrigin::Private { .. } => self.fence.check_open().is_err(),
        }
    }

    /// Capture before task effects; a parent is supplied only after the native
    /// child owner has established exact original-parent provenance.
    /// This retains metadata, without model preparation or dispatch. It must
    /// finish even after cancellation so the existing pre-submission path can
    /// durably acknowledge that the task never entered the VM. Dispatch and
    /// model preflight retain their separate cancellation/fence checks.
    pub async fn capture_task_model_selection(
        &self,
        parent: Option<Arc<dyn brassclaw_turns::run_profile::LoopTaskModelSelection>>,
        capture: Option<Arc<dyn brassclaw_turns::run_profile::LoopModelCaptureContext>>,
    ) -> Result<
        Option<Arc<dyn brassclaw_turns::run_profile::LoopTaskModelSelection>>,
        AgentLoopHostError,
    > {
        if self.private_claim().is_some() {
            return Ok(None);
        }
        self.chat_host()?
            .capture_task_model_selection(parent, capture.clone())
            .await
    }

    /// Install once from trusted composition, before task execution. This port
    /// retains the original dependency, not a Tool grant or current-role lookup.
    pub fn bind_memory_embeddings(
        &self,
        embeddings: Arc<dyn brassclaw_host_runtime::RuntimeMemoryEmbeddingPort>,
    ) -> Result<(), AgentLoopHostError> {
        self.memory_embeddings.set(embeddings).map_err(|_| {
            AgentLoopHostError::new(
                AgentLoopHostErrorKind::InvalidInvocation,
                "task memory dependency is already bound",
            )
        })
    }

    async fn with_memory_embeddings<T>(
        &self,
        invocation: impl std::future::Future<Output = T>,
    ) -> T {
        brassclaw_host_runtime::with_task_memory_embeddings(
            self.memory_embeddings.get().cloned(),
            invocation,
        )
        .await
    }

    pub async fn preflight_task_model_usage(
        &self,
        input: Option<&brassclaw_turns::run_profile::LoopModelInputReference>,
    ) -> Result<(), AgentLoopHostError> {
        self.check_dispatch_open()?;
        self.chat_host()?.preflight_task_model_usage(input).await?;
        self.check_dispatch_open()
    }

    pub fn task_model_selection(
        &self,
    ) -> Option<Arc<dyn brassclaw_turns::run_profile::LoopTaskModelSelection>> {
        self.chat_host()
            .ok()
            .and_then(|host| host.task_model_selection())
    }

    pub fn selected_model_route(
        &self,
    ) -> Result<brassclaw_turns::run_profile::LoopModelRouteSnapshot, AgentLoopHostError> {
        if let Some(selected) = self.task_model_selection() {
            return selected.model_route();
        }
        // Historical/contained hosts retain their explicit route contract.
        let route = self
            .chat_host()?
            .run_context()
            .resolved_model_route
            .clone()
            .ok_or_else(|| {
                AgentLoopHostError::new(
                    AgentLoopHostErrorKind::Unavailable,
                    "task model selection is unavailable",
                )
            })?;
        route.validate().map_err(|_| {
            AgentLoopHostError::new(
                AgentLoopHostErrorKind::Unavailable,
                "task model selection is invalid",
            )
        })?;
        Ok(route)
    }

    pub fn run_context(&self) -> Option<&LoopRunContext> {
        match &self.origin {
            TaskOrigin::Chat { host, .. } => Some(host.run_context()),
            TaskOrigin::Private { .. } => None,
        }
    }

    /// Transport cleanup observes the same durable cancellation signal as the
    /// task host. This is not a VM acknowledgement or effect reconciliation.
    pub async fn cancellation_requested(&self) {
        tokio::select! {
            _ = async {
                match &self.origin {
                    TaskOrigin::Chat { host, .. } => {
                        let _ = host.cancellation_requested().await;
                    }
                    TaskOrigin::Private { .. } => std::future::pending::<()>().await,
                }
            } => {},
            _ = self.fence.closed() => {},
        }
    }

    /// Host observations, not inferred from potentially missing forensic writes.
    pub fn review_dispatch_counts(&self) -> (u64, u64) {
        use std::sync::atomic::Ordering;
        (
            self.model_exchange_count.load(Ordering::Acquire),
            self.capability_dispatch_count.load(Ordering::Acquire),
        )
    }

    /// Trusted supervisor access only; this contains the Rust-only lease token.
    pub fn attempt(&self) -> MontyTaskAttempt {
        self.attempt
    }

    /// The actual worker claim stays behind the host boundary across waits.
    /// Durable admission must still recheck it; this is not a permission grant.
    pub fn claimed_run(&self) -> Option<&Arc<brassclaw_turns::runner::ClaimedTurnRun>> {
        match &self.origin {
            TaskOrigin::Chat { claimed_run, .. } => claimed_run.as_ref(),
            TaskOrigin::Private { .. } => None,
        }
    }

    /// Completion follows the original host-owned ingress, independently of
    /// whether this workflow may use a model. Durable admission still verifies
    /// the actual source/claim; this observation grants no Tool permission.
    pub fn is_result_only_completion(&self) -> bool {
        self.private_claim().is_some()
            || self
                .run_context()
                .is_some_and(|context| context.trusted_internal_turn)
            || self.claimed_run().is_some_and(|claimed| {
                claimed.state.source_binding_ref.as_str()
                    == brassclaw_turns::run_profile::INTERNAL_MODEL_SOURCE_BINDING
            })
    }

    pub fn request(&self) -> Option<&AgentLoopDriverRunRequest> {
        match &self.origin {
            TaskOrigin::Chat { request, .. } => Some(request.as_ref()),
            TaskOrigin::Private { .. } => None,
        }
    }

    /// Retain this handle before dispatch; the supervisor can fence a dropped
    /// runner future without borrowing or destroying the global Monty VM.
    pub fn fence_handle(&self) -> MontyTaskFence {
        self.fence.clone()
    }

    /// Close this admitted attempt's dispatch immediately. The instance service
    /// keeps started futures/results and still owes bounded VM acknowledgement
    /// and effect reconciliation. This is never a completed-cancellation receipt.
    pub fn fence_dispatch(&self) {
        self.fence.close();
    }

    /// Attempt freshness, separate from the kernel's current Tool permission.
    pub fn check_dispatch_open(&self) -> Result<(), AgentLoopHostError> {
        self.check_cancellation()?;
        self.fence.check_open()
    }

    /// One retained primitive through the existing kernel, under this attempt's
    /// acknowledgement barrier. Late outcomes stay available for reconciliation.
    pub async fn invoke_retained_tool(
        &self,
        capability: &brassclaw_host_runtime::RetainedFirstPartyCapability,
        request: brassclaw_host_runtime::RuntimeCapabilityRequest,
    ) -> Result<brassclaw_host_runtime::RuntimeCapabilityOutcome, AgentLoopHostError> {
        let call = self.begin_call()?;
        let result = self
            .with_memory_embeddings(capability.invoke(request))
            .await
            .map_err(|_| {
                AgentLoopHostError::new(
                    AgentLoopHostErrorKind::Unavailable,
                    "retained kernel invocation failed",
                )
            });
        self.finish_call(call, result, WithheldTaskPortValue::RetainedToolOutcome)
    }

    fn begin_call(&self) -> Result<MontyHostCall, AgentLoopHostError> {
        self.check_cancellation()?;
        self.fence.begin_call()
    }

    fn finish_call<T>(
        &self,
        call: MontyHostCall,
        result: Result<T, AgentLoopHostError>,
        retain: impl FnOnce(T) -> WithheldTaskPortValue,
    ) -> Result<T, AgentLoopHostError> {
        if self.cancellation_observed() {
            self.fence.close();
        }
        call.finish_retaining(result, |call_id, value| {
            // A fence cannot reopen. Only successful calls already active when
            // it closed can enter this receipt list. The global service owns
            // pending-call capacity; delivered successes are not accumulated.
            self.withheld.lock().push(WithheldTaskPortResult {
                call_id,
                value: retain(value),
            });
        })
    }

    /// Transfer actual results withheld after fencing to trusted reconciliation.
    /// This neither restores the attempt nor acknowledges external quiescence.
    pub fn take_withheld_results(&self) -> Vec<WithheldTaskPortResult> {
        std::mem::take(&mut *self.withheld.lock())
    }

    /// Trusted lifecycle observation. Completion may release a task only when
    /// late successful port answers have either been retained or reconciled.
    pub fn has_withheld_results(&self) -> bool {
        !self.withheld.lock().is_empty()
    }

    fn check_cancellation(&self) -> Result<(), AgentLoopHostError> {
        if self.cancellation_observed() {
            self.fence.close();
            return Err(AgentLoopHostError::new(
                AgentLoopHostErrorKind::Cancelled,
                "Monty task cancellation requested",
            ));
        }
        Ok(())
    }

    pub async fn visible_capabilities(
        &self,
    ) -> Result<VisibleCapabilitySurface, AgentLoopHostError> {
        let call = self.begin_call()?;
        let result = self
            .chat_host()?
            .visible_capabilities(VisibleCapabilityRequest)
            .await;
        self.finish_call(call, result, WithheldTaskPortValue::VisibleCapabilities)
    }

    pub async fn build_prompt_bundle(
        &self,
        request: LoopPromptBundleRequest,
    ) -> Result<LoopPromptBundle, AgentLoopHostError> {
        self.check_cancellation()?;
        // Legacy recipe_hint embeds raw component bodies. A Python payload is
        // not an approved component revision. The global manifest adapter must
        // resolve pinned component context behind the host boundary instead.
        if request.recipe_hint.is_some() {
            return Err(AgentLoopHostError::new(
                AgentLoopHostErrorKind::InvalidInvocation,
                "Monty prompt requests cannot supply unverified component bodies",
            ));
        }
        let call = self.begin_call()?;
        let result = self.chat_host()?.build_prompt_bundle(request).await;
        self.finish_call(call, result, WithheldTaskPortValue::PromptBundle)
    }

    /// Host-only preparation for a selected Recipe's complete model input.
    /// There is deliberately no raw-body JSON port in `dispatch_port`: the
    /// retained usage adapter must validate its typed input and selected prefix.
    /// Preparing references never calls the model or grants Tool permission.
    pub async fn build_prepared_prompt_bundle(
        &self,
        request: PreparedLoopPromptRequest,
    ) -> Result<LoopPromptBundle, AgentLoopHostError> {
        let call = self.begin_call()?;
        let result = self
            .chat_host()?
            .build_prepared_prompt_bundle(request)
            .await;
        self.finish_call(call, result, WithheldTaskPortValue::PromptBundle)
    }

    /// Return the structured response unchanged: tool requests are never
    /// converted to assistant text or dispatched by this adapter.
    pub async fn stream_model(
        &self,
        request: LoopModelRequest,
    ) -> Result<LoopModelResponse, AgentLoopHostError> {
        let call = self.begin_model_call(&request)?;
        let result = self.chat_host()?.stream_model(request).await;
        self.finish_call(call, result, WithheldTaskPortValue::ModelResponse)
    }

    fn begin_model_call(
        &self,
        request: &LoopModelRequest,
    ) -> Result<MontyHostCall, AgentLoopHostError> {
        if self.private_claim().is_some()
            || self
                .run_context()
                .is_some_and(|context| context.trusted_internal_turn)
        {
            return Err(AgentLoopHostError::new(
                AgentLoopHostErrorKind::PolicyDenied,
                "trusted internal turns require deterministic execution",
            ));
        }
        self.check_cancellation()?;
        // This field is a trusted Sempai bypass, not a Python prompt API. Monty
        // must use the run-scoped prompt refs issued by build_prompt_bundle.
        if request.resolved_messages.is_some() {
            return Err(AgentLoopHostError::new(
                AgentLoopHostErrorKind::InvalidInvocation,
                "Monty model requests require host-issued prompt references",
            ));
        }
        let call = self.begin_call()?;
        self.model_exchange_count
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        Ok(call)
    }

    /// A selected native usage takes custody of a received response before the
    /// final delivery fence. A late result may be journaled for the original
    /// effect, but can never resume Python or authorize another dispatch.
    pub(crate) async fn stream_model_retaining(
        &self,
        invocation_id: brassclaw_host_api::InvocationId,
        request: LoopModelRequest,
        record: Arc<dyn crate::monty_model_capability::RecipeModelResponseRecord>,
    ) -> Result<String, AgentLoopHostError> {
        if invocation_id.as_uuid().is_nil() {
            return Err(AgentLoopHostError::new(
                AgentLoopHostErrorKind::InvalidInvocation,
                "model invocation identity is invalid",
            ));
        }
        let call = self.begin_model_call(&request)?;
        let custodian = Arc::new(SelectedModelResponseCustodian {
            record,
            bytes: Mutex::new(None),
        });
        let result = async {
            self.chat_host()?
                .stream_model_with_custody(request, custodian.clone())
                .await?;
            // Only bytes durably retained by the original record may be delivered.
            custodian
                .bytes
                .lock()
                .take()
                .ok_or_else(missing_model_custody)
        }
        .await;
        self.finish_call(call, result, |response_bytes| {
            WithheldTaskPortValue::RecordedModelResponse {
                invocation_id,
                response_bytes,
            }
        })
    }

    pub async fn invoke_capability(
        &self,
        request: CapabilityInvocation,
    ) -> Result<CapabilityOutcome, AgentLoopHostError> {
        let call = self.begin_call()?;
        self.capability_dispatch_count
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        let result = self
            .with_memory_embeddings(self.chat_host()?.invoke_capability(request))
            .await;
        self.finish_call(call, result, WithheldTaskPortValue::CapabilityOutcome)
    }

    pub async fn append_capability_result_ref(
        &self,
        request: AppendCapabilityResultRef,
    ) -> Result<LoopMessageRef, AgentLoopHostError> {
        let call = self.begin_call()?;
        let result = self
            .chat_host()?
            .append_capability_result_ref(request)
            .await;
        self.finish_call(
            call,
            result,
            WithheldTaskPortValue::CapabilityResultReference,
        )
    }

    pub async fn finalize_assistant_message(
        &self,
        request: FinalizeAssistantMessage,
    ) -> Result<LoopMessageRef, AgentLoopHostError> {
        if self.is_result_only_completion() {
            return Err(AgentLoopHostError::new(
                AgentLoopHostErrorKind::PolicyDenied,
                "internal result-only work cannot publish an assistant reply",
            ));
        }
        let call = self.begin_call()?;
        let content = request.reply.content.clone();
        let result = self.chat_host()?.finalize_assistant_message(request).await;
        if let Ok(reference) = &result {
            // Retain actual success even if the attempt was fenced while the
            // transcript port awaited persistence. A late success is evidence
            // for reconciliation, never permission to resume Python.
            *self.published_reply.lock() = Some(PublishedReply {
                reference: reference.clone(),
                content,
            });
        }
        self.finish_call(call, result, WithheldTaskPortValue::FinalizedReplyReference)
    }

    /// Task-local reply lookup for Monty's history/completion handoff. Only a
    /// reference actually issued by this task host qualifies. The local fence
    /// is checked as well as durable cancellation; claims are never Python data.
    pub fn published_reply_content(
        &self,
        reference: &LoopMessageRef,
    ) -> Result<String, AgentLoopHostError> {
        let call = self.begin_call()?;
        let result = self
            .published_reply
            .lock()
            .as_ref()
            .filter(|reply| &reply.reference == reference)
            .map(|reply| reply.content.clone())
            .ok_or_else(|| {
                AgentLoopHostError::new(
                    AgentLoopHostErrorKind::ScopeMismatch,
                    "reply reference was not finalized by this Monty task",
                )
            });
        self.finish_call(call, result, WithheldTaskPortValue::PublishedReplyContent)
    }

    /// Read-only settlement evidence; this neither restores dispatch authority
    /// nor performs a transcript operation after cancellation.
    pub fn finalized_reply_evidence(&self) -> Option<(String, String)> {
        self.published_reply
            .lock()
            .as_ref()
            .map(|reply| (reply.reference.as_str().to_owned(), reply.content.clone()))
    }

    /// Trusted supervisor evidence, including success received after fencing.
    /// This does not validate an active lease or authorize task continuation.
    pub fn finalized_reply_ref(&self) -> Option<LoopMessageRef> {
        self.published_reply
            .lock()
            .as_ref()
            .map(|reply| reply.reference.clone())
    }
}

/// Bridge one selected native record into both neutral custody stages. The
/// original record owns pending data independently of dropped futures; neither
/// stage can submit provider work, change an identity or authorize a retry.
struct SelectedModelResponseCustodian {
    record: Arc<dyn crate::monty_model_capability::RecipeModelResponseRecord>,
    bytes: Mutex<Option<String>>,
}

#[async_trait::async_trait]
impl brassclaw_turns::run_profile::LoopModelResponseCustodian for SelectedModelResponseCustodian {
    async fn prepare_related_model_work(
        &self,
        work: &brassclaw_turns::run_profile::ModelWorkRequest,
        request_bytes: &str,
    ) -> Result<Arc<dyn brassclaw_turns::run_profile::LoopModelResponseCustodian>, AgentLoopHostError>
    {
        let record = self
            .record
            .prepare_related_model_work(work, request_bytes)
            .await?;
        Ok(Arc::new(Self {
            record,
            bytes: Mutex::new(None),
        }))
    }

    fn requested_output_tokens(&self) -> Option<std::num::NonZeroU32> {
        self.record.requested_output_tokens()
    }
    fn retained_model_input_reference(
        &self,
    ) -> Option<&brassclaw_turns::run_profile::LoopModelInputReference> {
        Some(self.record.retained_model_input_reference())
    }
    async fn before_provider_submission(&self) -> Result<(), AgentLoopHostError> {
        self.record.before_provider_submission().await
    }
    fn requires_provider_observation(&self) -> bool {
        true
    }
    fn receive_provider_wire_observation(
        &self,
        response: serde_json::Value,
    ) -> Result<(), AgentLoopHostError> {
        self.record.receive_provider_wire_observation(response)
    }
    async fn persist_provider_wire_observation(&self) -> Result<(), AgentLoopHostError> {
        self.record.persist_provider_wire_observation().await
    }
    fn receive_provider_observation(
        &self,
        response: serde_json::Value,
    ) -> Result<(), AgentLoopHostError> {
        self.record.receive_provider_observation(response)
    }
    async fn persist_provider_observation(&self) -> Result<(), AgentLoopHostError> {
        self.record.persist_provider_observation().await
    }
    fn receive_response(&self, response: &LoopModelResponse) -> Result<(), AgentLoopHostError> {
        self.record.receive_response(response.clone())
    }
    async fn persist_response(&self) -> Result<(), AgentLoopHostError> {
        let bytes = self.record.persist_response().await?;
        *self.bytes.lock() = Some(bytes);
        Ok(())
    }
}

fn missing_model_custody() -> AgentLoopHostError {
    AgentLoopHostError::new(
        AgentLoopHostErrorKind::Internal,
        "selected model response custody is incomplete",
    )
}
