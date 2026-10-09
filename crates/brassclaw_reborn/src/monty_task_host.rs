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
        VisibleCapabilityRequest, VisibleCapabilitySurface,
    },
};

/// Existing reply primitive's explicit dispatch identity. Its Python callable
/// remains host.post_reply; registration/selection must bind them explicitly.
pub const POST_REPLY_CAPABILITY_ID: &str = "host.post_reply";

/// Rust-owned task host. Never serialize this object or its claim identity into
/// Python. The hosting service addresses it using its own task routing token.
pub struct MontyTaskHost {
    request: AgentLoopDriverRunRequest,
    attempt: MontyTaskAttempt,
    host: Arc<dyn AgentLoopDriverHost + Send + Sync>,
    fence: MontyTaskFence,
    published_reply: Mutex<Option<PublishedReply>>,
    withheld: Mutex<Vec<WithheldTaskPortResult>>,
    model_exchange_count: std::sync::atomic::AtomicU64,
    capability_dispatch_count: std::sync::atomic::AtomicU64,
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
        let (request, attempt, host) = handoff.into_parts();
        Self {
            request,
            attempt,
            host,
            fence: MontyTaskFence::new(attempt),
            published_reply: Mutex::new(None),
            withheld: Mutex::new(Vec::new()),
            model_exchange_count: std::sync::atomic::AtomicU64::new(0),
            capability_dispatch_count: std::sync::atomic::AtomicU64::new(0),
        }
    }

    pub fn run_context(&self) -> &LoopRunContext {
        self.host.run_context()
    }

    /// Transport cleanup observes the same durable cancellation signal as the
    /// task host. This is not a VM acknowledgement or effect reconciliation.
    pub async fn cancellation_requested(&self) {
        tokio::select! {
            _ = self.host.cancellation_requested() => {},
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

    pub fn request(&self) -> &AgentLoopDriverRunRequest {
        &self.request
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
        let result = capability.invoke(request).await.map_err(|_| {
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
        if self.host.observe_cancellation().is_some() {
            self.fence.close();
        }
        call.finish_retaining(result, |call_id, value| {
            // A fence cannot reopen, and at most 64 calls were admitted before
            // it closed. Only late successes enter this bounded receipt list;
            // successful calls delivered to Python are not accumulated here.
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
        if self.host.observe_cancellation().is_some() {
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
            .host
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
        let result = self.host.build_prompt_bundle(request).await;
        self.finish_call(call, result, WithheldTaskPortValue::PromptBundle)
    }

    /// Return the structured response unchanged: tool requests are never
    /// converted to assistant text or dispatched by this adapter.
    pub async fn stream_model(
        &self,
        request: LoopModelRequest,
    ) -> Result<LoopModelResponse, AgentLoopHostError> {
        if self.run_context().trusted_internal_turn {
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
        let result = self.host.stream_model(request).await;
        self.finish_call(call, result, WithheldTaskPortValue::ModelResponse)
    }

    pub async fn invoke_capability(
        &self,
        request: CapabilityInvocation,
    ) -> Result<CapabilityOutcome, AgentLoopHostError> {
        let call = self.begin_call()?;
        self.capability_dispatch_count
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        let result = self.host.invoke_capability(request).await;
        self.finish_call(call, result, WithheldTaskPortValue::CapabilityOutcome)
    }

    pub async fn append_capability_result_ref(
        &self,
        request: AppendCapabilityResultRef,
    ) -> Result<LoopMessageRef, AgentLoopHostError> {
        let call = self.begin_call()?;
        let result = self.host.append_capability_result_ref(request).await;
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
        let call = self.begin_call()?;
        let content = request.reply.content.clone();
        let result = self.host.finalize_assistant_message(request).await;
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
