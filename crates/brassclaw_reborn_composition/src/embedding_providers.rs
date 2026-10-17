//! Concrete embedding-provider implementations and async factory.
//!
//! Moved here from `brassclaw_embeddings` as part of the S10 refactor (§3,
//! revision 17).  `brassclaw_embeddings` is now a pure trait-and-utilities
//! crate (`EmbeddingProvider`, `EmbeddingError`, `CachedEmbeddingProvider`,
//! `url_check`, `MockEmbeddings`).
//!
//! Concrete provider types (`OpenAiEmbeddings`, `NearAiEmbeddings`,
//! `OllamaEmbeddings`, `BedrockEmbeddings`) and the `create_provider` async
//! factory live here because they depend on `brassclaw_llm` runtime objects
//! (`SessionManager`) and DB-backed config that is only available in the
//! composition layer.
//!
//! # Public surface
//!
//! | Symbol | Use |
//! |--------|-----|
//! | `EmbeddingsConfig` | Pure-data provider config shape |
//! | `ProviderDeps` | Runtime wiring (session manager, optional Bedrock setup) |
//! | `create_provider(config, deps) -> Result<Option<Arc<dyn EmbeddingProvider>>, EmbeddingError>` | Factory |
//!
//! Concrete provider structs are crate-private; downstream code holds
//! `Arc<dyn brassclaw_embeddings::EmbeddingProvider>` only.

#[cfg(feature = "root-llm-provider")]
use std::sync::Arc;

use secrecy::{ExposeSecret, SecretString};

// ---------------------------------------------------------------------------
// EmbeddingsConfig
// ---------------------------------------------------------------------------

/// Embedding provider configuration (resolved from DB config + provider registry).
///
/// Pure data — no env-var or file reads at construction time.  The composition
/// factory (`resolve_pg_embedding_provider`) builds this from the DB snapshot.
#[derive(Debug, Clone)]
pub(crate) struct EmbeddingsConfig {
    /// Whether embeddings are enabled.
    pub enabled: bool,
    /// Provider to use: `"openai"`, `"nearai"`, `"ollama"`, or `"bedrock"`.
    pub provider: String,
    /// OpenAI API key (for OpenAI / OpenAI-compatible providers).
    pub openai_api_key: Option<SecretString>,
    /// Whether this selected catalogue definition requires an API key.
    pub api_key_required: bool,
    /// Model to use for embeddings.
    pub model: String,
    /// Exact selected Ollama base URL.
    pub ollama_base_url: String,
    /// Expected vector dimension from the exact database model contract.
    pub dimension: usize,
    /// Custom base URL for OpenAI-compatible embedding providers.
    pub openai_base_url: Option<String>,
    /// Base URL for the NEAR AI embeddings endpoint.
    pub nearai_base_url: String,
    /// Selected model's entry ceiling; zero disables caching.
    pub cache_size: usize,
    /// Captured technical deadline/body limits; embedding calls are not retried.
    pub request_policy: brassclaw_llm::ProviderRequestPolicy,
}

impl EmbeddingsConfig {
    /// Get the OpenAI API key if configured.
    pub(crate) fn openai_api_key(&self) -> Option<&str> {
        self.openai_api_key.as_ref().map(|s| s.expose_secret())
    }
}

impl Default for EmbeddingsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: String::new(),
            openai_api_key: None,
            api_key_required: true,
            model: String::new(),
            ollama_base_url: String::new(),
            dimension: 0,
            openai_base_url: None,
            nearai_base_url: String::new(),
            cache_size: 0,
            request_policy: Default::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// ProviderDeps
// ---------------------------------------------------------------------------

/// Runtime wiring the factory needs that doesn't fit in [`EmbeddingsConfig`].
///
/// `EmbeddingsConfig` is pure data (populated from the DB config snapshot).
/// These are shared runtime objects supplied by the host and consulted only
/// by the matching provider — `session` for `nearai`, `bedrock_setup` for
/// `bedrock`.
#[cfg(feature = "root-llm-provider")]
#[derive(Clone)]
pub(crate) struct ProviderDeps {
    pub session: Arc<brassclaw_llm::SessionManager>,
    /// Bedrock connection parameters. `Some` only when the `bedrock` feature
    /// is enabled and the operator has configured a Bedrock embedding provider.
    #[cfg(feature = "bedrock")]
    pub bedrock_setup: Option<BedrockEmbeddingSetup>,
}

/// AWS Bedrock parameters needed by the embedding provider.
///
/// Defined here (not re-using `brassclaw_llm::BedrockConfig`) so the embeddings
/// layer does not couple to LLM-side config types.
///
/// Only compiled when the `bedrock` feature is active — the fields are
/// consumed exclusively by `BedrockEmbeddings::new`.
#[cfg(feature = "bedrock")]
#[derive(Debug, Clone)]
pub(crate) struct BedrockEmbeddingSetup {
    pub region: String,
    pub profile: Option<String>,
}

// ---------------------------------------------------------------------------
// create_provider factory
// ---------------------------------------------------------------------------

/// Build the configured embedding provider.
///
/// Only a disabled selection returns `Ok(None)`. Invalid selected configuration,
/// unsupported adapters and initialization failures remain explicit errors.
#[cfg(feature = "root-llm-provider")]
pub(crate) async fn create_provider(
    config: &EmbeddingsConfig,
    deps: ProviderDeps,
) -> Result<Option<Arc<dyn brassclaw_embeddings::EmbeddingProvider>>, EmbeddingError> {
    if !config.enabled {
        tracing::debug!("embeddings disabled");
        return Ok(None);
    }

    config.request_policy.validate().map_err(|_| {
        EmbeddingError::InvalidResponse("embedding request policy is invalid".into())
    })?;

    if config.model.trim().is_empty() || config.dimension == 0 {
        return Err(EmbeddingError::InvalidResponse(
            "embedding model and positive dimension must be configured".into(),
        ));
    }
    if config
        .openai_api_key()
        .is_some_and(|key| key.trim().is_empty())
        || (config.api_key_required && config.openai_api_key.is_none())
    {
        return Err(EmbeddingError::AuthFailed);
    }

    let provider = match config.provider.as_str() {
        "nearai" => {
            check_embedding_base_url(&config.nearai_base_url, "nearai_base_url")?;
            tracing::debug!(model = %config.model, dim = %config.dimension, "embeddings via NEAR AI");
            Ok(Some(Arc::new(
                NearAiEmbeddings::new(
                    &config.nearai_base_url,
                    deps.session,
                    &config.model,
                    config.dimension,
                    config.request_policy,
                )?
                .with_api_key(config.openai_api_key.clone()),
            )
                as Arc<dyn brassclaw_embeddings::EmbeddingProvider>))
        }
        "bedrock" => {
            if config.request_policy.max_response_bytes.is_some() {
                return Err(EmbeddingError::InvalidResponse(
                    "embedding Bedrock adapter does not support a response-body limit".into(),
                ));
            }
            #[cfg(feature = "bedrock")]
            {
                let Some(setup) = deps.bedrock_setup.as_ref() else {
                    return Err(EmbeddingError::InvalidResponse(
                        "embedding Bedrock connection settings are missing".into(),
                    ));
                };
                tracing::debug!(model = %config.model, region = %setup.region, dim = %config.dimension, "embeddings via Bedrock");
                let provider =
                    BedrockEmbeddings::new(setup, &config.model, config.dimension).await?;
                Ok(Some(
                    Arc::new(provider) as Arc<dyn brassclaw_embeddings::EmbeddingProvider>
                ))
            }
            #[cfg(not(feature = "bedrock"))]
            {
                Err(EmbeddingError::InvalidResponse(
                    "embedding Bedrock adapter is unavailable in this build".into(),
                ))
            }
        }
        "ollama" => {
            check_embedding_base_url(&config.ollama_base_url, "ollama_base_url")?;
            tracing::debug!(model = %config.model, url = %config.ollama_base_url, dim = %config.dimension, "embeddings via Ollama");
            Ok(Some(Arc::new(OllamaEmbeddings::new(
                &config.ollama_base_url,
                &config.model,
                config.dimension,
                config.request_policy,
            )?)
                as Arc<dyn brassclaw_embeddings::EmbeddingProvider>))
        }
        "openai" => {
            let base_url = config.openai_base_url.as_deref().ok_or_else(|| {
                EmbeddingError::InvalidResponse("embedding endpoint must be configured".into())
            })?;
            check_embedding_base_url(base_url, "openai_base_url")?;
            let provider = OpenAiEmbeddings::with_model(
                config.openai_api_key.clone(),
                &config.model,
                config.dimension,
                base_url,
                config.request_policy,
            )?;
            tracing::debug!(model = %config.model, dim = %config.dimension, "embeddings via OpenAI-compatible");
            Ok(Some(
                Arc::new(provider) as Arc<dyn brassclaw_embeddings::EmbeddingProvider>
            ))
        }
        _ => Err(EmbeddingError::InvalidResponse(
            "selected embedding protocol has no supported adapter".into(),
        )),
    }?;
    Ok(provider.map(|inner| {
        Arc::new(DeadlineEmbeddingProvider {
            inner,
            timeout: std::time::Duration::from_secs(config.request_policy.timeout_secs),
        }) as Arc<dyn EmbeddingProvider>
    }))
}

// ---------------------------------------------------------------------------
// Concrete provider implementations (crate-private)
// ---------------------------------------------------------------------------

// ── OpenAI / OpenAI-compatible ────────────────────────────────────────────

use async_trait::async_trait;
use brassclaw_embeddings::{EmbeddingError, EmbeddingProvider};
use serde::{Deserialize, Serialize};

#[cfg(feature = "root-llm-provider")]
fn check_embedding_base_url(base_url: &str, field: &str) -> Result<(), EmbeddingError> {
    let parsed = url::Url::parse(base_url).map_err(|_| {
        EmbeddingError::InvalidResponse("embedding endpoint is not a valid URL".into())
    })?;
    if !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(EmbeddingError::InvalidResponse(
            "embedding endpoint cannot contain credentials, a query or a fragment".into(),
        ));
    }
    brassclaw_embeddings::url_check::check_base_url(base_url, field)
}

fn embeddings_endpoint(base_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
    if base.ends_with("/v1") {
        format!("{base}/embeddings")
    } else {
        format!("{base}/v1/embeddings")
    }
}

// Provider error bodies can echo the input or authentication material. Keep
// classification/status, without reading or propagating those payloads or URLs.
fn embedding_status_error(response: &reqwest::Response) -> Option<EmbeddingError> {
    let status = response.status();
    if status.is_success() {
        None
    } else if status == reqwest::StatusCode::UNAUTHORIZED {
        Some(EmbeddingError::AuthFailed)
    } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        Some(EmbeddingError::RateLimited {
            retry_after: Some(brassclaw_llm::retry::parse_retry_after(
                response.headers().get("retry-after"),
            )),
        })
    } else {
        Some(EmbeddingError::HttpError(format!(
            "embedding service returned HTTP {}",
            status.as_u16()
        )))
    }
}

fn embedding_transport_error(error: reqwest::Error) -> EmbeddingError {
    let reason = if error.is_timeout() {
        "embedding request timed out"
    } else if error.is_connect() {
        "embedding connection failed"
    } else {
        "embedding transport failed"
    };
    EmbeddingError::HttpError(reason.into())
}

fn embedding_client(
    policy: brassclaw_llm::ProviderRequestPolicy,
) -> Result<reqwest::Client, EmbeddingError> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(policy.timeout_secs))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| EmbeddingError::HttpError("embedding transport could not be created".into()))
}

async fn read_embedding_response<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
    max_bytes: Option<u64>,
) -> Result<T, EmbeddingError> {
    let body = brassclaw_llm::read_provider_response_body(response, max_bytes)
        .await
        .map_err(|error| match error {
            brassclaw_llm::ProviderResponseBodyError::Capacity { limit } => {
                EmbeddingError::InvalidResponse(format!(
                    "embedding response exceeded configured byte capacity ({limit})"
                ))
            }
            brassclaw_llm::ProviderResponseBodyError::Transport(error) => {
                embedding_transport_error(error)
            }
        })?;
    serde_json::from_slice(&body).map_err(|_| {
        EmbeddingError::InvalidResponse("embedding response could not be decoded".into())
    })
}

/// The deadline covers authentication and the complete batch as well as HTTP
/// body reads. Dropping an expired future never starts another request.
struct DeadlineEmbeddingProvider {
    inner: Arc<dyn EmbeddingProvider>,
    timeout: std::time::Duration,
}

#[async_trait]
impl EmbeddingProvider for DeadlineEmbeddingProvider {
    fn dimension(&self) -> usize {
        self.inner.dimension()
    }
    fn model_name(&self) -> &str {
        self.inner.model_name()
    }
    fn max_input_length(&self) -> Option<usize> {
        self.inner.max_input_length()
    }
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        tokio::time::timeout(self.timeout, self.inner.embed(text))
            .await
            .map_err(|_| EmbeddingError::HttpError("embedding request timed out".into()))?
    }
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        tokio::time::timeout(self.timeout, self.inner.embed_batch(texts))
            .await
            .map_err(|_| EmbeddingError::HttpError("embedding request timed out".into()))?
    }
}

struct OpenAiEmbeddings {
    client: reqwest::Client,
    api_key: Option<SecretString>,
    model: String,
    dimension: usize,
    base_url: String,
    max_response_bytes: Option<u64>,
}

impl OpenAiEmbeddings {
    fn with_model(
        api_key: Option<SecretString>,
        model: impl Into<String>,
        dimension: usize,
        base_url: &str,
        policy: brassclaw_llm::ProviderRequestPolicy,
    ) -> Result<Self, EmbeddingError> {
        Ok(Self {
            client: embedding_client(policy)?,
            api_key,
            model: model.into(),
            dimension,
            base_url: base_url.trim_end_matches('/').to_string(),
            max_response_bytes: policy.max_response_bytes,
        })
    }
}

#[derive(Debug, Serialize)]
struct OpenAiEmbeddingRequest<'a> {
    model: &'a str,
    input: &'a [String],
}

#[derive(Debug, Deserialize)]
struct OpenAiEmbeddingResponse {
    data: Vec<OpenAiEmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct OpenAiEmbeddingData {
    index: usize,
    embedding: Vec<f32>,
}

fn validate_embedding_inputs(
    texts: &[String],
    max_bytes: Option<usize>,
) -> Result<(), EmbeddingError> {
    if let Some(max_bytes) = max_bytes
        && let Some(text) = texts.iter().find(|text| text.len() > max_bytes)
    {
        return Err(EmbeddingError::TextTooLong {
            length: text.len(),
            max: max_bytes,
        });
    }
    Ok(())
}

fn validate_indexed_embeddings(
    response: OpenAiEmbeddingResponse,
    expected_count: usize,
    dimension: usize,
) -> Result<Vec<Vec<f32>>, EmbeddingError> {
    if response.data.len() != expected_count {
        return Err(EmbeddingError::InvalidResponse(
            "embedding response count differs from input count".into(),
        ));
    }
    let mut ordered = vec![None; expected_count];
    for item in response.data {
        let Some(slot) = ordered.get_mut(item.index) else {
            return Err(EmbeddingError::InvalidResponse(
                "embedding response index is out of bounds".into(),
            ));
        };
        if slot.replace(item.embedding).is_some() {
            return Err(EmbeddingError::InvalidResponse(
                "embedding response repeats an input index".into(),
            ));
        }
    }
    let vectors = ordered
        .into_iter()
        .map(|item| {
            item.ok_or_else(|| {
                EmbeddingError::InvalidResponse("embedding response omits an input index".into())
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    validate_embeddings(vectors, expected_count, dimension)
}

fn validate_embeddings(
    vectors: Vec<Vec<f32>>,
    expected_count: usize,
    dimension: usize,
) -> Result<Vec<Vec<f32>>, EmbeddingError> {
    if vectors.len() != expected_count
        || vectors.iter().any(|vector| {
            vector.len() != dimension || vector.iter().any(|value| !value.is_finite())
        })
    {
        return Err(EmbeddingError::InvalidResponse(
            "embedding response count, dimension or numeric values differ from the selected contract".into(),
        ));
    }
    Ok(vectors)
}

#[async_trait]
impl EmbeddingProvider for OpenAiEmbeddings {
    fn dimension(&self) -> usize {
        self.dimension
    }
    fn model_name(&self) -> &str {
        &self.model
    }
    fn max_input_length(&self) -> Option<usize> {
        None
    }

    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        if let Some(max) = self.max_input_length()
            && text.len() > max
        {
            return Err(EmbeddingError::TextTooLong {
                length: text.len(),
                max,
            });
        }
        self.embed_batch(&[text.to_string()])
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| EmbeddingError::InvalidResponse("no embedding returned".into()))
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        validate_embedding_inputs(texts, self.max_input_length())?;
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let request = OpenAiEmbeddingRequest {
            model: &self.model,
            input: texts,
        };
        let url = embeddings_endpoint(&self.base_url);
        let mut request_builder = self.client.post(&url);
        if let Some(key) = self.api_key.as_ref() {
            request_builder = request_builder.bearer_auth(key.expose_secret());
        }
        let response = request_builder
            .json(&request)
            .send()
            .await
            .map_err(embedding_transport_error)?;
        if let Some(error) = embedding_status_error(&response) {
            return Err(error);
        }
        let result: OpenAiEmbeddingResponse =
            read_embedding_response(response, self.max_response_bytes).await?;
        validate_indexed_embeddings(result, texts.len(), self.dimension)
    }
}

// ── NEAR AI ───────────────────────────────────────────────────────────────

#[cfg(feature = "root-llm-provider")]
struct NearAiEmbeddings {
    client: reqwest::Client,
    base_url: String,
    session: Arc<brassclaw_llm::SessionManager>,
    api_key: Option<SecretString>,
    model: String,
    dimension: usize,
    max_response_bytes: Option<u64>,
}

#[cfg(feature = "root-llm-provider")]
impl NearAiEmbeddings {
    fn new(
        base_url: impl Into<String>,
        session: Arc<brassclaw_llm::SessionManager>,
        model: impl Into<String>,
        dimension: usize,
        policy: brassclaw_llm::ProviderRequestPolicy,
    ) -> Result<Self, EmbeddingError> {
        Ok(Self {
            client: embedding_client(policy)?,
            base_url: base_url.into(),
            session,
            api_key: None,
            model: model.into(),
            dimension,
            max_response_bytes: policy.max_response_bytes,
        })
    }

    fn with_api_key(mut self, api_key: Option<SecretString>) -> Self {
        self.api_key = api_key;
        self
    }
}

#[cfg(feature = "root-llm-provider")]
#[derive(Debug, Serialize)]
struct NearAiEmbeddingRequest<'a> {
    model: &'a str,
    input: &'a [String],
}

#[cfg(feature = "root-llm-provider")]
#[async_trait]
impl EmbeddingProvider for NearAiEmbeddings {
    fn dimension(&self) -> usize {
        self.dimension
    }
    fn model_name(&self) -> &str {
        &self.model
    }
    fn max_input_length(&self) -> Option<usize> {
        None
    }

    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        if let Some(max) = self.max_input_length()
            && text.len() > max
        {
            return Err(EmbeddingError::TextTooLong {
                length: text.len(),
                max,
            });
        }
        self.embed_batch(&[text.to_string()])
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| EmbeddingError::InvalidResponse("no embedding returned".into()))
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        use secrecy::ExposeSecret as _;
        validate_embedding_inputs(texts, self.max_input_length())?;
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let request = NearAiEmbeddingRequest {
            model: &self.model,
            input: texts,
        };
        let token = match self.api_key.as_ref() {
            Some(key) => key.clone(),
            None => self
                .session
                .get_token()
                .await
                .map_err(|_| EmbeddingError::AuthFailed)?,
        };
        let url = embeddings_endpoint(&self.base_url);
        let response = self
            .client
            .post(&url)
            .bearer_auth(token.expose_secret())
            .json(&request)
            .send()
            .await
            .map_err(embedding_transport_error)?;
        if let Some(error) = embedding_status_error(&response) {
            return Err(error);
        }
        let result: OpenAiEmbeddingResponse =
            read_embedding_response(response, self.max_response_bytes).await?;
        validate_indexed_embeddings(result, texts.len(), self.dimension)
    }
}

// ── Ollama ────────────────────────────────────────────────────────────────

struct OllamaEmbeddings {
    client: reqwest::Client,
    base_url: String,
    model: String,
    dimension: usize,
    max_response_bytes: Option<u64>,
}

impl OllamaEmbeddings {
    fn new(
        base_url: impl Into<String>,
        model: impl Into<String>,
        dimension: usize,
        policy: brassclaw_llm::ProviderRequestPolicy,
    ) -> Result<Self, EmbeddingError> {
        Ok(Self {
            client: embedding_client(policy)?,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            model: model.into(),
            dimension,
            max_response_bytes: policy.max_response_bytes,
        })
    }
}

#[derive(Debug, Serialize)]
struct OllamaEmbedRequest<'a> {
    model: &'a str,
    input: &'a [String],
    // Oversized model inputs must fail explicitly rather than be truncated.
    truncate: bool,
}

#[derive(Debug, Deserialize)]
struct OllamaEmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

#[async_trait]
impl EmbeddingProvider for OllamaEmbeddings {
    fn dimension(&self) -> usize {
        self.dimension
    }
    fn model_name(&self) -> &str {
        &self.model
    }
    fn max_input_length(&self) -> Option<usize> {
        None
    }

    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        if let Some(max) = self.max_input_length()
            && text.len() > max
        {
            return Err(EmbeddingError::TextTooLong {
                length: text.len(),
                max,
            });
        }
        self.embed_batch(&[text.to_string()])
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| EmbeddingError::InvalidResponse("no embedding returned".into()))
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        validate_embedding_inputs(texts, self.max_input_length())?;
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let request = OllamaEmbedRequest {
            model: &self.model,
            input: texts,
            truncate: false,
        };
        let url = format!("{}/api/embed", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(embedding_transport_error)?;
        if let Some(error) = embedding_status_error(&response) {
            return Err(error);
        }
        let result: OllamaEmbedResponse =
            read_embedding_response(response, self.max_response_bytes).await?;
        validate_embeddings(result.embeddings, texts.len(), self.dimension)
    }
}

// ── AWS Bedrock (Titan Text Embeddings V2) ────────────────────────────────

#[cfg(feature = "bedrock")]
struct BedrockEmbeddings {
    client: aws_sdk_bedrockruntime::Client,
    model: String,
    dimension: usize,
}

#[cfg(feature = "bedrock")]
impl BedrockEmbeddings {
    async fn new(
        setup: &BedrockEmbeddingSetup,
        model: impl Into<String>,
        dimension: usize,
    ) -> Result<Self, EmbeddingError> {
        let mut builder = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(aws_config::Region::new(setup.region.clone()));
        if let Some(ref profile) = setup.profile {
            builder = builder.profile_name(profile);
        }
        let sdk_config = builder.load().await;
        Ok(Self {
            client: aws_sdk_bedrockruntime::Client::new(&sdk_config),
            model: model.into(),
            dimension,
        })
    }
}

#[cfg(feature = "bedrock")]
#[derive(Debug, Serialize)]
struct BedrockTitanEmbeddingRequest<'a> {
    #[serde(rename = "inputText")]
    input_text: &'a str,
    dimensions: usize,
    normalize: bool,
}

#[cfg(feature = "bedrock")]
#[derive(Debug, Deserialize)]
struct BedrockTitanEmbeddingResponse {
    embedding: Vec<f32>,
}

#[cfg(feature = "bedrock")]
fn map_bedrock_error<R: std::fmt::Debug>(
    error: &aws_sdk_bedrockruntime::error::SdkError<
        aws_sdk_bedrockruntime::operation::invoke_model::InvokeModelError,
        R,
    >,
) -> EmbeddingError {
    use aws_sdk_bedrockruntime::error::SdkError;
    use aws_sdk_bedrockruntime::operation::invoke_model::InvokeModelError;
    match error {
        SdkError::ServiceError(svc) => match svc.err() {
            InvokeModelError::ThrottlingException(_) => {
                EmbeddingError::RateLimited { retry_after: None }
            }
            InvokeModelError::AccessDeniedException(_) => EmbeddingError::AuthFailed,
            InvokeModelError::ValidationException(_) => {
                EmbeddingError::InvalidResponse("Bedrock validation error".into())
            }
            InvokeModelError::ModelNotReadyException(_) => {
                EmbeddingError::HttpError("Bedrock model not ready".into())
            }
            _ => EmbeddingError::HttpError("Bedrock service error".into()),
        },
        SdkError::TimeoutError(_) => {
            EmbeddingError::HttpError("Bedrock request timed out".to_string())
        }
        _ => EmbeddingError::HttpError("Bedrock request failed".into()),
    }
}

#[cfg(feature = "bedrock")]
#[async_trait]
impl EmbeddingProvider for BedrockEmbeddings {
    fn dimension(&self) -> usize {
        self.dimension
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    fn max_input_length(&self) -> Option<usize> {
        None
    }

    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        if let Some(max) = self.max_input_length()
            && text.len() > max
        {
            return Err(EmbeddingError::TextTooLong {
                length: text.len(),
                max,
            });
        }

        let request = BedrockTitanEmbeddingRequest {
            input_text: text,
            dimensions: self.dimension,
            normalize: true,
        };

        let body = serde_json::to_vec(&request).map_err(|e| {
            EmbeddingError::InvalidResponse(format!("failed to serialize request: {e}"))
        })?;

        let response = self
            .client
            .invoke_model()
            .model_id(&self.model)
            .content_type("application/json")
            .accept("application/json")
            .body(aws_smithy_types::Blob::new(body))
            .send()
            .await
            .map_err(|e| map_bedrock_error(&e))?;

        let result: BedrockTitanEmbeddingResponse = serde_json::from_slice(response.body.as_ref())
            .map_err(|_| {
                EmbeddingError::InvalidResponse("Bedrock response could not be decoded".into())
            })?;

        validate_embeddings(vec![result.embedding], 1, self.dimension)?
            .pop()
            .ok_or_else(|| EmbeddingError::InvalidResponse("no embedding returned".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_embedding_response_preserves_inputs_and_rejects_invalid_contracts() {
        let response = |items: &[(usize, Vec<f32>)]| OpenAiEmbeddingResponse {
            data: items
                .iter()
                .map(|(index, embedding)| OpenAiEmbeddingData {
                    index: *index,
                    embedding: embedding.clone(),
                })
                .collect(),
        };
        assert_eq!(
            validate_indexed_embeddings(
                response(&[(1, vec![2.0, 3.0]), (0, vec![0.0, 1.0])]),
                2,
                2
            )
            .unwrap(),
            vec![vec![0.0, 1.0], vec![2.0, 3.0]]
        );
        for items in [
            vec![(0, vec![0.0, 1.0])],
            vec![(0, vec![0.0, 1.0]), (0, vec![2.0, 3.0])],
            vec![(0, vec![0.0, 1.0]), (2, vec![2.0, 3.0])],
            vec![(0, vec![0.0]), (1, vec![2.0, 3.0])],
            vec![(0, vec![f32::INFINITY, 1.0]), (1, vec![2.0, 3.0])],
        ] {
            assert!(validate_indexed_embeddings(response(&items), 2, 2).is_err());
        }
        assert!(validate_embeddings(vec![], 1, 2).is_err());
        assert!(validate_embedding_inputs(&["ä".into()], Some(1)).is_err());
        assert!(validate_embedding_inputs(&["Ü λ🦀".repeat(5000)], None).is_ok());
    }

    #[cfg(feature = "root-llm-provider")]
    #[tokio::test]
    async fn configured_embedding_errors_are_distinct_from_disabled() {
        let deps = || ProviderDeps {
            session: Arc::new(brassclaw_llm::SessionManager::new(
                brassclaw_llm::SessionConfig::default(),
            )),
            #[cfg(feature = "bedrock")]
            bedrock_setup: None,
        };
        assert!(
            create_provider(&EmbeddingsConfig::default(), deps())
                .await
                .unwrap()
                .is_none()
        );
        let mut config = EmbeddingsConfig {
            enabled: true,
            provider: "openai".into(),
            model: "unit-vector-model".into(),
            dimension: 3,
            ..EmbeddingsConfig::default()
        };
        assert!(matches!(
            create_provider(&config, deps()).await,
            Err(EmbeddingError::AuthFailed)
        ));
        config.api_key_required = false;
        config.openai_base_url = Some("http://127.0.0.1:8000/v1/".into());
        assert_eq!(
            create_provider(&config, deps())
                .await
                .unwrap()
                .unwrap()
                .max_input_length(),
            None
        );
        for endpoint in [
            "file:///tmp/private",
            "https://secret:password@example.com/v1",
            "https://example.com/v1?key=private",
            "https://example.com/v1#fragment",
            "http://169.254.169.254/v1",
        ] {
            config.openai_base_url = Some(endpoint.into());
            assert!(create_provider(&config, deps()).await.is_err());
        }
        config.provider = "unknown".into();
        assert!(create_provider(&config, deps()).await.is_err());
        assert_eq!(
            embeddings_endpoint("http://127.0.0.1:8000/v1/"),
            "http://127.0.0.1:8000/v1/embeddings"
        );
        assert_eq!(
            embeddings_endpoint("https://example.com/custom/"),
            "https://example.com/custom/v1/embeddings"
        );
    }
}
