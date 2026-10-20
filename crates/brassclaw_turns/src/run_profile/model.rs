use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::LoopDiagnosticRef;

use super::host::{
    AgentLoopHostError, AgentLoopHostErrorKind, AgentLoopHostErrorReasonKind, LoopModelPort,
    LoopModelRequest, LoopModelResponse, LoopModelResponseCustodian, LoopRunContext,
    LoopSafeSummary, ParentLoopOutput, sanitize_model_visible_text,
};
use super::milestones::{LoopHostMilestoneEmitter, LoopHostMilestoneSink};
use super::model_work::{ModelWorkOutcome, ModelWorkRequest};

/// Outcome passed to [`LoopModelBudgetAccountant::post_model_call`] so the
/// accountant can record usage on success or note the failure kind.
#[derive(Debug, Clone)]
pub enum ModelCallOutcome<'a> {
    /// The model call succeeded; the response is available for inspection.
    Success(&'a LoopModelResponse),
    /// The model call failed with the given gateway error.
    Failure(&'a LoopModelGatewayError),
}

/// Budget/resource accounting boundary invoked around every model call flowing
/// through [`HostManagedLoopModelPort`].
///
/// Implementations may enforce token budgets, call-count limits, cost caps, or
/// any other resource policy. A `pre_model_call` rejection short-circuits the
/// provider call entirely.
#[async_trait]
pub trait LoopModelBudgetAccountant: Send + Sync {
    /// An isolated reservation namespace for host-owned work nested within a
    /// model dispatch. It must share the same resource governor/policy while
    /// retaining the original run identity. Returning None fails setup closed;
    /// reusing a run-keyed reservation would corrupt the enclosing call.
    fn isolated_work_accountant(&self) -> Option<Arc<dyn LoopModelBudgetAccountant>> {
        None
    }

    /// Called **before** any model-backed work dispatches to a provider.
    async fn pre_model_work(
        &self,
        context: &LoopRunContext,
        request: &ModelWorkRequest,
    ) -> Result<(), LoopModelGatewayError>;

    /// Mark the boundary after which external work may have begun. Call this
    /// synchronously before entering the provider/worker future, after admission.
    /// Cancellation or an error after this boundary is not evidence of zero
    /// spend. Accountants retaining reservations must preserve that uncertainty.
    fn model_work_dispatching(
        &self,
        _context: &LoopRunContext,
        _request: &ModelWorkRequest,
    ) -> Result<(), LoopModelGatewayError> {
        Ok(())
    }

    /// Called after model-backed work succeeds or fails.
    async fn post_model_work(
        &self,
        context: &LoopRunContext,
        request: &ModelWorkRequest,
        outcome: ModelWorkOutcome,
    ) -> Result<(), LoopModelGatewayError>;

    /// Reconcile a retained work request with the actual provider response.
    /// Accountants with provider pricing override this to use reported usage.
    async fn post_model_work_result(
        &self,
        context: &LoopRunContext,
        request: &ModelWorkRequest,
        outcome: ModelCallOutcome<'_>,
    ) -> Result<(), LoopModelGatewayError> {
        self.post_model_work(context, request, ModelWorkOutcome::from_model_call(outcome))
            .await
    }

    /// Opt-in reconciliation of an already-observed provider success. Must be
    /// safe after an unknown accounting commit; never dispatch provider work.
    /// None means the retained reservation cannot be reconciled automatically.
    async fn reconcile_retained_model_work(
        &self,
        _context: &LoopRunContext,
        _request: &ModelWorkRequest,
        _response: &LoopModelResponse,
    ) -> Option<Result<(), LoopModelGatewayError>> {
        None
    }

    /// Called **before** dispatching the model request. Return `Err` with
    /// `AgentLoopHostErrorKind::BudgetExceeded` to reject the call.
    async fn pre_model_call(
        &self,
        context: &LoopRunContext,
        request: &LoopModelRequest,
    ) -> Result<(), LoopModelGatewayError> {
        self.pre_model_work(context, &ModelWorkRequest::for_assistant(context, request))
            .await
    }

    /// Called **after** the model call completes (or fails). Implementations
    /// should record success usage and retain reservations for unknown provider
    /// outcomes. A gateway failure alone never proves zero spend. Accounting
    /// failures must be returned so callers fail closed instead of hiding stuck
    /// reservations or missing failed-call accounting behind the provider error.
    async fn post_model_call(
        &self,
        context: &LoopRunContext,
        request: &LoopModelRequest,
        outcome: ModelCallOutcome<'_>,
    ) -> Result<(), LoopModelGatewayError> {
        self.post_model_work(
            context,
            &ModelWorkRequest::for_assistant(context, request),
            ModelWorkOutcome::from_model_call(outcome),
        )
        .await
    }

    /// Synchronous cancellation cleanup. Release only when the accountant can
    /// prove dispatch never began; otherwise retain the original hold and any
    /// observed result for reconciliation. Dropping a future, including during
    /// settlement, does not prove the provider performed no work. The legacy
    /// method name does not require unconditional release. The exact admitted
    /// request fences delayed cleanup from a successor operation in the same run.
    fn release_in_flight(&self, _context: &LoopRunContext, _request: &ModelWorkRequest) {}
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopModelGatewayRequest {
    pub context: LoopRunContext,
    pub request: LoopModelRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[error("loop model gateway {kind:?}: {safe_summary}")]
/// Sanitized model-gateway failure surfaced through the loop-host wire contract.
///
/// `AgentLoopHostErrorKind::CredentialUnavailable` means the host could not
/// provide a scoped, non-reusable credential for the selected provider/model;
/// callers must treat it as a host-owned credential acquisition failure, not as
/// provider output. `AgentLoopHostErrorKind::BudgetExceeded` can also surface
/// after a provider failure when post-call accounting/release fails closed.
pub struct LoopModelGatewayError {
    pub kind: AgentLoopHostErrorKind,
    pub safe_summary: LoopSafeSummary,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_kind: Option<AgentLoopHostErrorReasonKind>,
    pub diagnostic_ref: Option<LoopDiagnosticRef>,
}

impl LoopModelGatewayError {
    pub fn new(
        kind: AgentLoopHostErrorKind,
        safe_summary: impl Into<String>,
    ) -> Result<Self, String> {
        Ok(Self {
            kind,
            safe_summary: LoopSafeSummary::new(safe_summary)?,
            reason_kind: None,
            diagnostic_ref: None,
        })
    }

    pub fn with_reason_kind(mut self, reason_kind: AgentLoopHostErrorReasonKind) -> Self {
        self.reason_kind = Some(reason_kind);
        self
    }

    pub fn with_diagnostic_ref(mut self, diagnostic_ref: LoopDiagnosticRef) -> Self {
        self.diagnostic_ref = Some(diagnostic_ref);
        self
    }

    pub fn into_host_error(self) -> AgentLoopHostError {
        let mut error = AgentLoopHostError::new(self.kind, self.safe_summary.as_str().to_string());
        if let Some(reason_kind) = self.reason_kind {
            error = error.with_reason_kind(reason_kind);
        }
        if let Some(diagnostic_ref) = self.diagnostic_ref {
            error = error.with_diagnostic_ref(diagnostic_ref);
        }
        error
    }
}

#[async_trait]
pub trait PreparedLoopModelCall: Send {
    /// Exact work accounting for the host-resolved bytes retained by this call.
    /// The prepared call is Rust-private; it cannot be supplied by Monty/LLM.
    fn work_request(&self) -> &ModelWorkRequest;
    async fn dispatch(self: Box<Self>) -> Result<LoopModelResponse, LoopModelGatewayError>;
}

#[async_trait]
pub trait LoopModelGateway: Send + Sync {
    /// Selected native usage: unsupported adapters fail before provider I/O.
    async fn prepare_model_call_with_observation(
        &self,
        _request: LoopModelGatewayRequest,
        _custodian: Arc<dyn LoopModelResponseCustodian>,
    ) -> Result<Box<dyn PreparedLoopModelCall>, LoopModelGatewayError> {
        Err(LoopModelGatewayError::new(
            AgentLoopHostErrorKind::Unimplemented,
            "provider response custody is unavailable",
        )
        .expect("classified summary"))
    }

    /// Resolve and retain an authorized prompt before policy/accounting. Legacy
    /// gateways without reference resolution may use the default None path.
    async fn prepare_model_call(
        &self,
        _request: LoopModelGatewayRequest,
    ) -> Result<Option<Box<dyn PreparedLoopModelCall>>, LoopModelGatewayError> {
        Ok(None)
    }

    async fn stream_model(
        &self,
        request: LoopModelGatewayRequest,
    ) -> Result<LoopModelResponse, LoopModelGatewayError>;
}

/// Provider/model policy guard consulted before dispatching a model call.
///
/// Implementations may enforce allow/deny lists for models, providers, or
/// any request-level policy. A denial short-circuits the call before any
/// provider or credential is touched.
#[async_trait]
pub trait LoopModelPolicyGuard: Send + Sync {
    /// Return `Ok(())` to allow model-backed work, or `Err` with
    /// `AgentLoopHostErrorKind::PolicyDenied` and a sanitized summary.
    async fn check_model_work_policy(
        &self,
        context: &LoopRunContext,
        request: &ModelWorkRequest,
    ) -> Result<(), LoopModelGatewayError>;

    /// Return `Ok(())` to allow the call, or `Err` with
    /// `AgentLoopHostErrorKind::PolicyDenied` and a sanitized summary.
    async fn check_model_policy(
        &self,
        context: &LoopRunContext,
        request: &LoopModelRequest,
    ) -> Result<(), LoopModelGatewayError> {
        self.check_model_work_policy(context, &ModelWorkRequest::for_assistant(context, request))
            .await
    }
}

/// A no-op policy guard that allows every model call.
pub struct NoOpPolicyGuard;

#[async_trait]
impl LoopModelPolicyGuard for NoOpPolicyGuard {
    async fn check_model_work_policy(
        &self,
        _context: &LoopRunContext,
        _request: &ModelWorkRequest,
    ) -> Result<(), LoopModelGatewayError> {
        Ok(())
    }
}

/// A no-op budget accountant that approves every call and records nothing.
///
/// Used as the default when no budget policy is configured.
pub struct NoOpBudgetAccountant;

#[async_trait]
impl LoopModelBudgetAccountant for NoOpBudgetAccountant {
    async fn reconcile_retained_model_work(
        &self,
        _context: &LoopRunContext,
        _request: &ModelWorkRequest,
        _response: &LoopModelResponse,
    ) -> Option<Result<(), LoopModelGatewayError>> {
        Some(Ok(()))
    }

    fn isolated_work_accountant(&self) -> Option<Arc<dyn LoopModelBudgetAccountant>> {
        Some(Arc::new(NoOpBudgetAccountant))
    }

    async fn pre_model_work(
        &self,
        _context: &LoopRunContext,
        _request: &ModelWorkRequest,
    ) -> Result<(), LoopModelGatewayError> {
        Ok(())
    }

    async fn post_model_work(
        &self,
        _context: &LoopRunContext,
        _request: &ModelWorkRequest,
        _outcome: ModelWorkOutcome,
    ) -> Result<(), LoopModelGatewayError> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct HostManagedLoopModelPort<G, S>
where
    G: LoopModelGateway + ?Sized,
    S: LoopHostMilestoneSink + ?Sized,
{
    context: LoopRunContext,
    gateway: Arc<G>,
    milestones: LoopHostMilestoneEmitter<S>,
    accountant: Arc<dyn LoopModelBudgetAccountant>,
    policy_guard: Arc<dyn LoopModelPolicyGuard>,
}

impl<G, S> HostManagedLoopModelPort<G, S>
where
    G: LoopModelGateway + ?Sized,
    S: LoopHostMilestoneSink + ?Sized,
{
    pub fn new(context: LoopRunContext, gateway: Arc<G>, milestone_sink: Arc<S>) -> Self {
        let milestones = LoopHostMilestoneEmitter::new(context.clone(), milestone_sink);
        Self {
            context,
            gateway,
            milestones,
            accountant: Arc::new(NoOpBudgetAccountant),
            policy_guard: Arc::new(NoOpPolicyGuard),
        }
    }

    /// Create a port with a custom budget accountant injected.
    pub fn with_accountant(
        context: LoopRunContext,
        gateway: Arc<G>,
        milestone_sink: Arc<S>,
        accountant: Arc<dyn LoopModelBudgetAccountant>,
    ) -> Self {
        let milestones = LoopHostMilestoneEmitter::new(context.clone(), milestone_sink);
        Self {
            context,
            gateway,
            milestones,
            accountant,
            policy_guard: Arc::new(NoOpPolicyGuard),
        }
    }

    /// Create a fully-configured port with policy guard and budget accountant.
    pub fn with_guards(
        context: LoopRunContext,
        gateway: Arc<G>,
        milestone_sink: Arc<S>,
        accountant: Arc<dyn LoopModelBudgetAccountant>,
        policy_guard: Arc<dyn LoopModelPolicyGuard>,
    ) -> Self {
        let milestones = LoopHostMilestoneEmitter::new(context.clone(), milestone_sink);
        Self {
            context,
            gateway,
            milestones,
            accountant,
            policy_guard,
        }
    }
}

impl<G, S> HostManagedLoopModelPort<G, S>
where
    G: LoopModelGateway + ?Sized,
    S: LoopHostMilestoneSink + ?Sized,
{
    async fn stream_model_inner(
        &self,
        request: LoopModelRequest,
        custodian: Option<Arc<dyn LoopModelResponseCustodian>>,
    ) -> Result<LoopModelResponse, AgentLoopHostError> {
        if request.work_id.is_none() {
            return Err(AgentLoopHostError::new(
                AgentLoopHostErrorKind::InvalidInvocation,
                "model work identity is missing",
            ));
        }
        let preparation = LoopModelGatewayRequest {
            context: self.context.clone(),
            request: request.clone(),
        };
        let prepared = if let Some(custodian) = custodian
            .as_ref()
            .filter(|c| c.requires_provider_observation())
        {
            Some(
                self.gateway
                    .prepare_model_call_with_observation(preparation, custodian.clone())
                    .await
                    .map_err(LoopModelGatewayError::into_host_error)?,
            )
        } else {
            self.gateway
                .prepare_model_call(preparation)
                .await
                .map_err(LoopModelGatewayError::into_host_error)?
        };
        let work_request = prepared.as_ref().map_or_else(
            || ModelWorkRequest::for_assistant(&self.context, &request),
            |prepared| prepared.work_request().clone(),
        );
        let is_prepared = prepared.is_some();
        if work_request.work_id != request.work_id {
            return Err(AgentLoopHostError::new(
                AgentLoopHostErrorKind::InvalidInvocation,
                "prepared model work identity differs",
            ));
        }

        // Policy check — rejects before any provider or credential is touched.
        if let Err(policy_error) = self
            .policy_guard
            .check_model_work_policy(&self.context, &work_request)
            .await
        {
            return Err(policy_error.into_host_error());
        }

        // Pre-call budget check — rejects before touching the provider.
        if let Err(budget_error) = self
            .accountant
            .pre_model_work(&self.context, &work_request)
            .await
        {
            return Err(budget_error.into_host_error());
        }

        // The guard can release an unsubmitted reservation. Once dispatch may
        // begin, cancellation must preserve unknown or already completed spend.
        let mut release_guard =
            ReservationReleaseGuard::new(self.accountant.as_ref(), &self.context, &work_request);

        if let Err(error) = self
            .milestones
            .model_started(request.model_preference.clone())
            .await
        {
            tracing::debug!(
                kind = ?error.kind,
                diagnostic_ref = ?error.diagnostic_ref,
                "loop model_started milestone failed before model request"
            );
        }

        self.accountant
            .model_work_dispatching(&self.context, &work_request)
            .map_err(LoopModelGatewayError::into_host_error)?;
        let gateway_result = if let Some(prepared) = prepared {
            prepared.dispatch().await
        } else {
            self.gateway
                .stream_model(LoopModelGatewayRequest {
                    context: self.context.clone(),
                    request: request.clone(),
                })
                .await
        }
        .map(sanitize_model_response);

        // A received response belongs to its original invocation even if
        // accounting subsequently fails. Always account the provider outcome,
        // including when custody persistence itself returns an error.
        let custody_result = match (&gateway_result, custodian) {
            (Ok(response), Some(custodian)) => match custodian.receive_response(response) {
                Ok(()) => custodian.persist_response().await,
                Err(error) => Err(error),
            },
            _ => Ok(()),
        };

        // Post-call accounting fires on BOTH success and failure. The
        // RAII guard stays armed across this await — if the future is
        // cancelled mid-`post_model_call`, cancellation cleanup preserves the
        // original reservation. Successful accounting removes its own entry.
        let outcome = match &gateway_result {
            Ok(response) => ModelCallOutcome::Success(response),
            Err(error) => ModelCallOutcome::Failure(error),
        };
        let post_result = if is_prepared {
            self.accountant
                .post_model_work_result(&self.context, &work_request, outcome)
                .await
        } else {
            self.accountant
                .post_model_call(&self.context, &request, outcome)
                .await
        };
        // Disarm only AFTER post_model_call returns. If we're past this
        // line accounting owns the reconciled or unresolved reservation.
        release_guard.disarm();
        if let Err(post_error) = post_result {
            if let Err(custody_error) = &custody_result {
                tracing::warn!(
                    kind = ?custody_error.kind,
                    diagnostic_ref = ?custody_error.diagnostic_ref,
                    "model response custody and accounting both require reconciliation"
                );
            }
            let host_error = post_error.into_host_error();
            if let Err(milestone_error) = self.milestones.model_failed(host_error.kind).await {
                tracing::debug!(
                    kind = ?milestone_error.kind,
                    diagnostic_ref = ?milestone_error.diagnostic_ref,
                    "loop model_failed milestone failed after post-model accounting error"
                );
            }
            return Err(host_error);
        }
        if let Err(error) = custody_result {
            if let Err(milestone_error) = self.milestones.model_failed(error.kind).await {
                tracing::debug!(
                    kind = ?milestone_error.kind,
                    diagnostic_ref = ?milestone_error.diagnostic_ref,
                    "loop model_failed milestone failed after custody error"
                );
            }
            return Err(error);
        }

        let response = match gateway_result {
            Ok(response) => response,
            Err(error) => {
                let host_error = error.into_host_error();
                if let Err(milestone_error) = self.milestones.model_failed(host_error.kind).await {
                    tracing::debug!(
                        kind = ?milestone_error.kind,
                        diagnostic_ref = ?milestone_error.diagnostic_ref,
                        "loop model_failed milestone failed after model error"
                    );
                }
                return Err(host_error);
            }
        };

        for safe_delta in &response.safe_reasoning_deltas {
            if let Err(error) = self
                .milestones
                .model_reasoning_delta(safe_delta.clone())
                .await
            {
                tracing::debug!(
                    kind = ?error.kind,
                    diagnostic_ref = ?error.diagnostic_ref,
                    "loop model reasoning milestone failed after successful model response"
                );
            }
        }
        if let Err(error) = self
            .milestones
            .model_completed(response.effective_model_profile_id.clone())
            .await
        {
            tracing::debug!(
                kind = ?error.kind,
                diagnostic_ref = ?error.diagnostic_ref,
                "loop model_completed milestone failed after successful model response"
            );
        }
        Ok(response)
    }
}

#[async_trait]
impl<G, S> LoopModelPort for HostManagedLoopModelPort<G, S>
where
    G: LoopModelGateway + ?Sized,
    S: LoopHostMilestoneSink + ?Sized,
{
    async fn stream_model(
        &self,
        request: LoopModelRequest,
    ) -> Result<LoopModelResponse, AgentLoopHostError> {
        self.stream_model_inner(request, None).await
    }

    async fn stream_model_with_custody(
        &self,
        request: LoopModelRequest,
        custodian: Arc<dyn LoopModelResponseCustodian>,
    ) -> Result<LoopModelResponse, AgentLoopHostError> {
        self.stream_model_inner(request, Some(custodian)).await
    }
}

/// Cancellation cleanup delegates to the accountant, which knows whether
/// dispatch could have begun. Keep it armed through settlement: cancellation
/// during a lost acknowledgement must not release completed spend.
struct ReservationReleaseGuard<'a> {
    accountant: &'a dyn LoopModelBudgetAccountant,
    context: &'a LoopRunContext,
    request: &'a ModelWorkRequest,
    armed: bool,
}

impl<'a> ReservationReleaseGuard<'a> {
    fn new(
        accountant: &'a dyn LoopModelBudgetAccountant,
        context: &'a LoopRunContext,
        request: &'a ModelWorkRequest,
    ) -> Self {
        Self {
            accountant,
            context,
            request,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for ReservationReleaseGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.accountant
                .release_in_flight(self.context, self.request);
        }
    }
}

fn sanitize_model_response(mut response: LoopModelResponse) -> LoopModelResponse {
    for chunk in &mut response.chunks {
        chunk.safe_text_delta =
            sanitize_model_visible_text(std::mem::take(&mut chunk.safe_text_delta));
    }
    for safe_delta in &mut response.safe_reasoning_deltas {
        *safe_delta = sanitize_model_visible_text(std::mem::take(safe_delta));
    }
    response
        .safe_reasoning_deltas
        .retain(|safe_delta| !safe_delta.is_empty());
    if let ParentLoopOutput::AssistantReply(reply) = &mut response.output {
        reply.content = sanitize_model_visible_text(std::mem::take(&mut reply.content));
    }
    response
}
