//! Restricted bootstrap host: real durable transcript and cancellation, no
//! model, Recipe lookup, external transport or general capability surface.
use async_trait::async_trait;
use brassclaw_loop_support::{RunStateLoopCancellationPort, ThreadBackedLoopTranscriptPort};
use brassclaw_threads::PgSessionThreadService;
use brassclaw_turns::{LoopMessageRef, TurnCheckpointId, run_profile::*};
use std::sync::Arc;

pub(super) struct BootstrapReplyHost {
    pub(super) transcript: ThreadBackedLoopTranscriptPort<PgSessionThreadService>,
    pub(super) cancellation: RunStateLoopCancellationPort,
    // Keep the live durable cancellation supervisor alive for this case.
    pub(super) _cancellation_owner: Arc<brassclaw_loop_support::TurnStateRunCancellationFactory>,
}
impl LoopRunInfoPort for BootstrapReplyHost {
    fn run_context(&self) -> &LoopRunContext {
        self.transcript.run_context()
    }
}
#[async_trait]
impl LoopTranscriptPort for BootstrapReplyHost {
    async fn finalize_assistant_message(
        &self,
        request: FinalizeAssistantMessage,
    ) -> Result<LoopMessageRef, AgentLoopHostError> {
        self.transcript.finalize_assistant_message(request).await
    }
}
#[async_trait]
impl LoopCancellationPort for BootstrapReplyHost {
    fn observe_cancellation(&self) -> Option<LoopCancellationSignal> {
        self.cancellation.observe_cancellation()
    }
    async fn cancellation_requested(&self) -> LoopCancellationSignal {
        self.cancellation.cancellation_requested().await
    }
}
fn denied() -> AgentLoopHostError {
    AgentLoopHostError::new(
        AgentLoopHostErrorKind::Unavailable,
        "bootstrap host exposes only the validation transcript",
    )
}
// These are explicit containment denials, never simulated successful ports.
macro_rules! deny_ports {
    ($trait:ident { $( $method:ident ( $( $arg:ident : $ty:ty ),* ) -> $out:ty; )* }) => {
        #[async_trait]
        impl $trait for BootstrapReplyHost {
            $( async fn $method(&self, $( $arg: $ty ),*) -> Result<$out, AgentLoopHostError> {
                $( let _ = $arg; )* Err(denied())
            } )*
        }
    };
}
deny_ports!(LoopContextPort { load_loop_context(request: LoopContextRequest) -> LoopContextBundle; });
deny_ports!(LoopPromptPort { build_prompt_bundle(request: LoopPromptBundleRequest) -> LoopPromptBundle; });
deny_ports!(LoopInputPort { poll_inputs(after: LoopInputCursor, limit: usize) -> LoopInputBatch; ack_inputs(tokens: Vec<LoopInputAckToken>) -> (); });
deny_ports!(LoopModelPort { stream_model(request: LoopModelRequest) -> LoopModelResponse; });
deny_ports!(LoopCapabilityPort { visible_capabilities(request: VisibleCapabilityRequest) -> VisibleCapabilitySurface; invoke_capability(request: CapabilityInvocation) -> CapabilityOutcome; invoke_capability_batch(request: CapabilityBatchInvocation) -> CapabilityBatchOutcome; });
deny_ports!(LoopCheckpointPort { checkpoint(request: LoopCheckpointRequest) -> TurnCheckpointId; });
deny_ports!(LoopProgressPort { emit_loop_progress(event: LoopProgressEvent) -> (); });
#[async_trait]
impl LoopCompactionPort for BootstrapReplyHost {
    async fn compact_loop_context(
        &self,
        _request: LoopCompactionRequest,
    ) -> Result<LoopCompactionOutcome, LoopCompactionError> {
        Err(LoopCompactionError::UnsupportedMode)
    }
}
impl LoopRecipePort for BootstrapReplyHost {
    fn recipe_lookup(&self) -> Option<&dyn RecipeLookup> {
        None
    }
}
impl LoopRetrievalPort for BootstrapReplyHost {
    fn retrieval_lookup(&self) -> Option<&dyn RetrievalLookup> {
        None
    }
}
impl LoopOrchestratorPort for BootstrapReplyHost {
    fn orchestrator_lookup(&self) -> Option<&dyn OrchestratorLookup> {
        None
    }
}
#[async_trait]
impl LoopInterceptorPort for BootstrapReplyHost {
    async fn on_prompt_assembled(
        &self,
        _run: &str,
        _iteration: u32,
        _snapshot: serde_json::Value,
    ) -> Option<InterceptorResult> {
        None
    }
    async fn on_kohai_response(
        &self,
        _packet: &str,
        _response: &str,
        _usage: Option<serde_json::Value>,
    ) {
    }
}
