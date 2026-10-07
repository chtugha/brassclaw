use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_loop_support::{
    HostIdentityContextSource, HostManagedModelGateway, ThreadBackedLoopModelPort,
};
use brassclaw_threads::{SessionThreadService, ThreadScope};
use brassclaw_turns::run_profile::{
    AgentLoopHostError, InstructionMaterializationStore, LoopCapabilityPort, LoopModelGateway,
    LoopModelGatewayError, LoopModelGatewayRequest, LoopModelPort, LoopModelResponse,
    LoopPromptBundleAuthority, LoopRunContext, LoopSafeSummary, PreparedLoopModelCall,
};

pub(super) struct ThreadResolvingLoopModelGateway<S, G>
where
    S: SessionThreadService + ?Sized,
    G: HostManagedModelGateway + ?Sized,
{
    pub(super) thread_service: Arc<S>,
    pub(super) thread_scope: ThreadScope,
    pub(super) host_gateway: Arc<G>,
    pub(super) max_messages: usize,
    pub(super) identity_context_source: Option<Arc<dyn HostIdentityContextSource>>,
    pub(super) instruction_materialization_store: Option<Arc<dyn InstructionMaterializationStore>>,
    pub(super) capabilities: Option<Arc<dyn LoopCapabilityPort>>,
    pub(super) prompt_authority: LoopPromptBundleAuthority,
    pub(super) tool_result_source: Option<Arc<dyn brassclaw_loop_support::ToolResultPayloadSource>>,
}

impl<S, G> ThreadResolvingLoopModelGateway<S, G>
where
    S: SessionThreadService + ?Sized + Send + Sync + 'static,
    G: HostManagedModelGateway + ?Sized + Send + Sync + 'static,
{
    fn port(&self, context: LoopRunContext) -> ThreadBackedLoopModelPort<S, G> {
        let mut model_port = ThreadBackedLoopModelPort::new(
            Arc::clone(&self.thread_service),
            self.thread_scope.clone(),
            context,
            Arc::clone(&self.host_gateway),
            self.max_messages,
        )
        .with_prompt_bundle_authority(self.prompt_authority.clone());
        if let Some(source) = self.identity_context_source.as_ref() {
            model_port = model_port.with_identity_context_source(source.clone());
        }
        if let Some(store) = self.instruction_materialization_store.as_ref() {
            model_port = model_port.with_instruction_materialization_store(Arc::clone(store));
        }
        if let Some(capabilities) = self.capabilities.as_ref() {
            model_port = model_port.with_capability_port(Arc::clone(capabilities));
        }
        if let Some(source) = self.tool_result_source.as_ref() {
            model_port = model_port.with_tool_result_source(Arc::clone(source));
        }
        model_port
    }
}

#[async_trait]
impl<S, G> LoopModelGateway for ThreadResolvingLoopModelGateway<S, G>
where
    S: SessionThreadService + ?Sized + Send + Sync + 'static,
    G: HostManagedModelGateway + ?Sized + Send + Sync + 'static,
{
    async fn prepare_model_call(
        &self,
        request: LoopModelGatewayRequest,
    ) -> Result<Option<Box<dyn PreparedLoopModelCall>>, LoopModelGatewayError> {
        self.port(request.context)
            .prepare_model_call(request.request)
            .await
            .map(Some)
            .map_err(host_error_to_model_gateway_error)
    }

    async fn stream_model(
        &self,
        request: LoopModelGatewayRequest,
    ) -> Result<LoopModelResponse, LoopModelGatewayError> {
        self.port(request.context)
            .stream_model(request.request)
            .await
            .map_err(host_error_to_model_gateway_error)
    }
}

fn host_error_to_model_gateway_error(error: AgentLoopHostError) -> LoopModelGatewayError {
    let diagnostic_ref = error.diagnostic_ref;
    let reason_kind = error.reason_kind;
    let mut converted = match LoopModelGatewayError::new(error.kind, error.safe_summary) {
        Ok(error) => error,
        Err(_) => LoopModelGatewayError {
            kind: error.kind,
            safe_summary: LoopSafeSummary::model_gateway_failed(),
            reason_kind: None,
            diagnostic_ref: None,
        },
    };
    if let Some(reason_kind) = reason_kind {
        converted = converted.with_reason_kind(reason_kind);
    }
    if let Some(diagnostic_ref) = diagnostic_ref {
        converted = converted.with_diagnostic_ref(diagnostic_ref);
    }
    converted
}
