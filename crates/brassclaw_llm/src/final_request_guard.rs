//! One immutable wire request, exact host measurement and live technical limits.
//! This is a transport primitive. A caller must resolve its accepted model/prefix
//! binding and actual encoder; neither a DTO estimate nor model output qualifies.
use crate::{
    ModelRequestTokenCounts, ModelTokenCheckError, ProviderDispatchGuard, ProviderDispatchState,
    ProviderRequestFacts,
};
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::sync::Arc;

/// Deliberately sanitized. Request bytes, endpoint/auth and encoder diagnostics
/// never cross this error boundary. A rejection is local and nonretryable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FinalRequestError {
    #[error("final request does not match its retained target")]
    Target,
    #[error("selected transport has no qualified final request hook")]
    UnsupportedTransport,
    #[error("final request has no complete immutable body")]
    Body,
    #[error("exact model input measurement is unavailable")]
    Measurement,
    #[error("measurement does not belong to this exact wire request")]
    Binding,
    #[error("final model policy could not be verified")]
    Policy,
    #[error("model input rejected: {0}")]
    Tokens(ModelTokenCheckError),
}

/// No Debug/Serialize: the bytes may contain private history, tools and data.
/// It borrows the actual immutable reqwest body that will be transmitted.
pub struct FinalRequestInput<'a> {
    pub body: &'a [u8],
    /// Borrowed host-only metadata from the SAME owned request. The native
    /// accepted-pair verifier checks prompt-affecting header semantics without
    /// serializing auth/header values into model inputs, worker data or logs.
    /// Client-added defaults are not exposed here; their qualified transport
    /// contract must ALSO be retained by the accepted native binding.
    pub headers: &'a reqwest::header::HeaderMap,
    pub provider_id: &'a str,
    pub model: &'a str,
}

/// Returned only by the trusted native model/prefix encoding owner. A request's
/// metadata and model-generated token counts must never construct this record.
/// The encoding owner also authenticates generation/design/artifact identity,
/// semantic prefix attribution and provider/default mapping. A matching body
/// digest alone proves none of those properties.
#[derive(Clone)]
pub struct FinalRequestMeasurement {
    pub body_sha256: [u8; 32],
    pub provider_id: String,
    pub model: String,
    pub reserved_output_tokens: u32,
    pub counts: ModelRequestTokenCounts,
}
impl std::fmt::Debug for FinalRequestMeasurement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FinalRequestMeasurement")
            .field("counts", &self.counts)
            .field("reserved_output_tokens", &self.reserved_output_tokens)
            .finish_non_exhaustive()
    }
}

/// Required operation, without an estimate/default or permissive fallback.
/// Implementations use the exact accepted target's contained tokenizer/render
/// owner, including actual schemas/history/framing and effective output values.
#[async_trait]
pub trait FinalRequestMeasurer: Send + Sync {
    async fn measure(
        &self,
        input: FinalRequestInput<'_>,
    ) -> Result<FinalRequestMeasurement, FinalRequestError>;
}

/// Immutable target identity, not another settings or model selection owner.
/// The endpoint is resolved by the existing provider configuration machinery.
pub struct FinalRequestGuard {
    provider_id: String,
    model: String,
    endpoint: reqwest::Url,
    policy: Arc<dyn ProviderDispatchGuard>,
    measurer: Arc<dyn FinalRequestMeasurer>,
}
impl std::fmt::Debug for FinalRequestGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FinalRequestGuard").finish_non_exhaustive()
    }
}
impl FinalRequestGuard {
    pub fn new(
        provider_id: String,
        model: String,
        endpoint: reqwest::Url,
        policy: Arc<dyn ProviderDispatchGuard>,
        measurer: Arc<dyn FinalRequestMeasurer>,
    ) -> Result<Self, FinalRequestError> {
        if provider_id.is_empty()
            || provider_id.trim() != provider_id
            || provider_id.chars().any(char::is_control)
            || model.is_empty()
            || model.trim() != model
            || model.chars().any(char::is_control)
            || !matches!(endpoint.scheme(), "http" | "https")
            || endpoint.host_str().is_none()
            || endpoint.fragment().is_some()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
        {
            return Err(FinalRequestError::Target);
        }
        Ok(Self {
            provider_id,
            model,
            endpoint,
            policy,
            measurer,
        })
    }

    /// This performs no inference/network send. The private returned object owns
    /// the exact original request; there is no public body/policy setter or
    /// conversion into an editable request between validation and execution.
    pub async fn prepare(
        &self,
        request: reqwest::Request,
    ) -> Result<CheckedFinalRequest<'_>, FinalRequestError> {
        if request.method() != reqwest::Method::POST || request.url() != &self.endpoint {
            return Err(FinalRequestError::Target);
        }
        let body = request
            .body()
            .and_then(reqwest::Body::as_bytes)
            .ok_or(FinalRequestError::Body)?;
        let measurement = self
            .measurer
            .measure(FinalRequestInput {
                body,
                headers: request.headers(),
                provider_id: &self.provider_id,
                model: &self.model,
            })
            .await?;
        if measurement.body_sha256 != <[u8; 32]>::from(Sha256::digest(body))
            || measurement.provider_id != self.provider_id
            || measurement.model != self.model
        {
            return Err(FinalRequestError::Binding);
        }
        let checked = CheckedFinalRequest {
            owner: self,
            request,
            measurement,
        };
        // Re-read AFTER potentially long tokenization. Canonical caps apply
        // independently of optional task/slice budgets, which are absent here.
        checked.check_live().await?;
        Ok(checked)
    }
}

/// A checked request is invocation-local and single-use. It cannot be cloned,
/// serialized, used for another model or turned into an editable HTTP request.
pub struct CheckedFinalRequest<'a> {
    owner: &'a FinalRequestGuard,
    request: reqwest::Request,
    measurement: FinalRequestMeasurement,
}
impl CheckedFinalRequest<'_> {
    async fn check_live(&self) -> Result<ProviderDispatchState, FinalRequestError> {
        let state = self
            .owner
            .policy
            .check(ProviderRequestFacts {
                model: &self.owner.model,
                max_output_tokens: Some(self.measurement.reserved_output_tokens),
            })
            .await
            .map_err(|_| FinalRequestError::Policy)?;
        state
            .check_configured_token_counts(
                &self.owner.provider_id,
                &self.owner.model,
                self.measurement.counts,
            )
            .map_err(FinalRequestError::Tokens)?;
        Ok(state)
    }

    /// No reserialization or DTO reconstruction. Repeat the mandatory policy
    /// check if the caller waited after prepare, then hand the same owned request
    /// to reqwest exactly once. Every deliberate retry needs a new preparation.
    pub async fn send(
        self,
        transport: &FinalRequestTransport,
    ) -> Result<reqwest::Response, FinalRequestSendError> {
        self.send_observed(transport, None).await
    }

    pub(crate) async fn send_observed(
        self,
        transport: &FinalRequestTransport,
        observer: Option<Arc<dyn crate::provider_observation::ProviderWireObserver>>,
    ) -> Result<reqwest::Response, FinalRequestSendError> {
        if let Some(observer) = &observer {
            observer.before_submission().await.map_err(|_| {
                observer.submission_rejected();
                FinalRequestSendError::Rejected(FinalRequestError::Policy)
            })?;
        }
        self.check_live().await.map_err(|error| {
            if let Some(observer) = &observer {
                observer.submission_rejected();
            }
            FinalRequestSendError::Rejected(error)
        })?;
        if let Some(observer) = observer {
            observer.submission_started();
        }
        transport
            .client
            .execute(self.request)
            .await
            .map_err(FinalRequestSendError::Transport)
    }
}

pub enum FinalRequestSendError {
    Rejected(FinalRequestError),
    Transport(reqwest::Error),
}
impl std::fmt::Debug for FinalRequestSendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(error) => f.debug_tuple("Rejected").field(error).finish(),
            Self::Transport(_) => f.write_str("Transport"),
        }
    }
}
impl std::fmt::Display for FinalRequestSendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(error) => std::fmt::Display::fmt(error, f),
            Self::Transport(_) => f.write_str("provider transport failed"),
        }
    }
}
impl std::error::Error for FinalRequestSendError {}

/// The one selected HTTP client, built from the owning provider's explicit
/// technical builder. Hidden redirects/retries must not bypass remeasurement,
/// live policy checks or durable attempt accounting. This is not a provider pool.
#[derive(Clone)]
pub struct FinalRequestTransport {
    client: reqwest::Client,
}
impl FinalRequestTransport {
    pub fn new(builder: reqwest::ClientBuilder) -> Result<Self, reqwest::Error> {
        builder
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .build()
            .map(|client| Self { client })
    }

    // Only the existing registry SDK transport uses this constructor. Its one
    // client was created above with redirects/retries disabled. Cloning reqwest
    // shares that same selected client; it creates no second provider client.
    pub(crate) fn from_registry_client(client: reqwest::Client) -> Self {
        Self { client }
    }

    pub(crate) fn into_client(self) -> reqwest::Client {
        self.client
    }
}

tokio::task_local! {
    static CURRENT: Arc<FinalRequestGuard>;
}

pub(crate) fn current() -> Option<Arc<FinalRequestGuard>> {
    CURRENT.try_with(Arc::clone).ok()
}

/// Native request-local selection only. This does not certify counts or grant
/// submission: the actual immutable SDK body still must pass this guard's
/// measurement, live policy and single-use transport barrier.
pub fn has_selected_target(provider_id: &str, model: &str) -> bool {
    current().is_some_and(|guard| guard.provider_id == provider_id && guard.model == model)
}

impl FinalRequestGuard {
    fn validate_provider<P: crate::LlmProvider + ?Sized>(
        &self,
        provider: &P,
        model: Option<&str>,
    ) -> Result<(), crate::LlmError> {
        let reason = if !provider.supports_final_request_guard() {
            Some(FinalRequestError::UnsupportedTransport)
        } else if provider.model_name().as_ref() != self.model.as_str()
            || provider.effective_model_name(model) != self.model
            || current().is_some()
        {
            Some(FinalRequestError::Target)
        } else {
            None
        };
        match reason {
            Some(reason) => {
                if let Some(observer) = crate::provider_observation::current() {
                    observer.submission_rejected();
                }
                Err(crate::LlmError::FinalRequestRejected { reason })
            }
            None => Ok(()),
        }
    }

    /// Scope to one already retained supported provider call. Neither a shared
    /// client nor another task can replace its model/prefix measurement owner.
    pub async fn complete<P: crate::LlmProvider + ?Sized>(
        self: Arc<Self>,
        provider: &P,
        request: crate::CompletionRequest,
    ) -> Result<crate::CompletionResponse, crate::LlmError> {
        self.validate_provider(provider, request.model.as_deref())?;
        CURRENT.scope(self, provider.complete(request)).await
    }

    pub async fn complete_with_tools<P: crate::LlmProvider + ?Sized>(
        self: Arc<Self>,
        provider: &P,
        request: crate::ToolCompletionRequest,
    ) -> Result<crate::ToolCompletionResponse, crate::LlmError> {
        self.validate_provider(provider, request.model.as_deref())?;
        CURRENT
            .scope(self, provider.complete_with_tools(request))
            .await
    }
}

#[cfg(test)]
#[path = "final_request_guard_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "final_request_sdk_tests.rs"]
mod sdk_tests;
