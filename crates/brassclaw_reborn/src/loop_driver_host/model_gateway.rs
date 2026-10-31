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
    pub(super) task_selection:
        tokio::sync::OnceCell<Option<brassclaw_loop_support::HostManagedTaskModelSelection>>,
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
    fn port(
        &self,
        mut context: LoopRunContext,
    ) -> Result<ThreadBackedLoopModelPort<S, G>, AgentLoopHostError> {
        let selected = self.task_selection.get().and_then(Option::as_ref);
        if let Some(selected) = selected {
            context.resolved_model_route = Some(selected.binding.model_route()?);
        }
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
        if let Some(selected) = selected {
            model_port = model_port.with_task_gateway(selected.gateway.clone());
        }
        Ok(model_port)
    }
}

#[async_trait]
impl<S, G> LoopModelGateway for ThreadResolvingLoopModelGateway<S, G>
where
    S: SessionThreadService + ?Sized + Send + Sync + 'static,
    G: HostManagedModelGateway + ?Sized + Send + Sync + 'static,
{
    async fn preflight_task_model_usage(
        &self,
        input: Option<&brassclaw_turns::run_profile::LoopModelInputReference>,
    ) -> Result<(), AgentLoopHostError> {
        if input.is_some() && !matches!(self.task_selection.get(), Some(Some(_))) {
            return Err(AgentLoopHostError::new(
                brassclaw_turns::run_profile::AgentLoopHostErrorKind::Unavailable,
                "original task model input selection is unavailable",
            ));
        }
        if let Some(Some(selected)) = self.task_selection.get() {
            selected
                .gateway
                .preflight_task_model_usage(input)
                .await
                .map_err(|_| {
                    AgentLoopHostError::new(
                        brassclaw_turns::run_profile::AgentLoopHostErrorKind::Unavailable,
                        "selected task model usage is unavailable",
                    )
                })?;
        }
        Ok(())
    }

    async fn capture_task_model_selection(
        &self,
        parent: Option<Arc<dyn brassclaw_turns::run_profile::LoopTaskModelSelection>>,
        capture: Option<Arc<dyn brassclaw_turns::run_profile::LoopModelCaptureContext>>,
    ) -> Result<
        Option<Arc<dyn brassclaw_turns::run_profile::LoopTaskModelSelection>>,
        AgentLoopHostError,
    > {
        let selected = self
            .task_selection
            .get_or_try_init(|| async {
                self.host_gateway
                    .capture_task_model_selection(parent.clone(), capture.clone())
                    .await
                    .map_err(|_| {
                        AgentLoopHostError::new(
                            brassclaw_turns::run_profile::AgentLoopHostErrorKind::Unavailable,
                            "task model selection could not be captured",
                        )
                    })
            })
            .await?;
        if let Some(capture) = capture.as_ref()
            && selected
                .as_ref()
                .is_none_or(|selected| !selected.binding.matches_capture_context(capture.as_ref()))
        {
            return Err(AgentLoopHostError::new(
                brassclaw_turns::run_profile::AgentLoopHostErrorKind::InvalidInvocation,
                "task model capture declaration conflicts",
            ));
        }
        if let Some(parent) = parent.as_ref()
            && selected
                .as_ref()
                .is_none_or(|selected| !Arc::ptr_eq(&selected.binding, parent))
        {
            return Err(AgentLoopHostError::new(
                brassclaw_turns::run_profile::AgentLoopHostErrorKind::InvalidInvocation,
                "task model parent selection conflicts",
            ));
        }
        Ok(selected.as_ref().map(|selected| selected.binding.clone()))
    }

    fn task_model_selection(
        &self,
    ) -> Option<Arc<dyn brassclaw_turns::run_profile::LoopTaskModelSelection>> {
        self.task_selection
            .get()
            .and_then(Option::as_ref)
            .map(|selected| selected.binding.clone())
    }

    async fn prepare_model_call_with_observation(
        &self,
        request: LoopModelGatewayRequest,
        custodian: Arc<dyn brassclaw_turns::run_profile::LoopModelResponseCustodian>,
    ) -> Result<Box<dyn PreparedLoopModelCall>, LoopModelGatewayError> {
        self.port(request.context)
            .map_err(host_error_to_model_gateway_error)?
            .prepare_model_call_with_observation(request.request, custodian)
            .await
            .map_err(host_error_to_model_gateway_error)
    }

    async fn prepare_model_call(
        &self,
        request: LoopModelGatewayRequest,
    ) -> Result<Option<Box<dyn PreparedLoopModelCall>>, LoopModelGatewayError> {
        self.port(request.context)
            .map_err(host_error_to_model_gateway_error)?
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
            .map_err(host_error_to_model_gateway_error)?
            .stream_model(request.request)
            .await
            .map_err(host_error_to_model_gateway_error)
    }
}

pub(super) fn host_error_to_model_gateway_error(
    error: AgentLoopHostError,
) -> LoopModelGatewayError {
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
