//! Concrete embedding-provider implementations and async factory.
//!
//! Moved here from `brassclaw_embeddings` as part of the S10 refactor (§3,
//! revision 17).  `brassclaw_embeddings` is now a pure trait-and-utilities
//! crate (`EmbeddingProvider`, `EmbeddingError`, `CachedEmbeddingProvider`,
//! `url_check`, `default_dimension_for_model`, `MockEmbeddings`).
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
//! | `create_provider(config, deps) -> Option<Arc<dyn EmbeddingProvider>>` | Factory |
//! | `default_dimension_for_model` | Re-exported from `brassclaw_embeddings` |
//!
//! Concrete provider structs are crate-private; downstream code holds
//! `Arc<dyn brassclaw_embeddings::EmbeddingProvider>` only.

#[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
use std::sync::Arc;

pub use brassclaw_embeddings::default_dimension_for_model;
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
    /// Model to use for embeddings.
    pub model: String,
    /// Ollama base URL (for Ollama provider). Defaults to `http://localhost:11434`.
    pub ollama_base_url: String,
    /// Embedding vector dimension. Inferred from the model name when not set explicitly.
    pub dimension: usize,
    /// Custom base URL for OpenAI-compatible embedding providers.
    pub openai_base_url: Option<String>,
    /// Base URL for the NEAR AI embeddings endpoint.
    pub nearai_base_url: String,
    /// Maximum entries in the embedding LRU cache (default 10,000).
    pub cache_size: usize,
}

impl EmbeddingsConfig {
    /// Get the OpenAI API key if configured.
    pub(crate) fn openai_api_key(&self) -> Option<&str> {
        self.openai_api_key.as_ref().map(|s| s.expose_secret())
    }
}

impl Default for EmbeddingsConfig {
    fn default() -> Self {
        let model = "text-embedding-3-small".to_string();
        let dimension = default_dimension_for_model(&model);
        Self {
            enabled: false,
            provider: "openai".to_string(),
            openai_api_key: None,
            model,
            ollama_base_url: "http://localhost:11434".to_string(),
            dimension,
            openai_base_url: None,
            nearai_base_url: "https://api.near.ai".to_string(),
            cache_size: brassclaw_embeddings::DEFAULT_EMBEDDING_CACHE_SIZE,
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
    pub bedrock_setup: Option<BedrockEmbeddingSetup>,
}

/// AWS Bedrock parameters needed by the embedding provider.
///
/// Defined here (not re-using `brassclaw_llm::BedrockConfig`) so the embeddings
/// layer does not couple to LLM-side config types.
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
/// Returns `None` if embeddings are disabled, required credentials are missing,
/// or the URL check fails.  All rejects are logged at debug level.
#[cfg(feature = "root-llm-provider")]
pub(crate) async fn create_provider(
    config: &EmbeddingsConfig,
    deps: ProviderDeps,
) -> Option<Arc<dyn brassclaw_embeddings::EmbeddingProvider>> {
    use brassclaw_embeddings::url_check::check_base_url;

    if !config.enabled {
        tracing::debug!("embeddings disabled");
        return None;
    }

    match config.provider.as_str() {
        "nearai" => {
            if let Err(e) = check_base_url(&config.nearai_base_url, "nearai_base_url") {
                tracing::debug!(error = %e, "refusing to build NEAR AI embeddings");
                return None;
            }
            tracing::debug!(model = %config.model, dim = %config.dimension, "embeddings via NEAR AI");
            Some(Arc::new(NearAiEmbeddings::new(&config.nearai_base_url, deps.session)
                .with_model(&config.model, config.dimension)) as Arc<dyn brassclaw_embeddings::EmbeddingProvider>)
        }
        "bedrock" => {
            let Some(_setup) = deps.bedrock_setup.as_ref() else {
                tracing::debug!("embeddings configured for Bedrock but no Bedrock setup available");
                return None;
            };
            tracing::debug!("bedrock embedding provider not enabled in this build");
            None
        }
        "ollama" => {
            if let Err(e) = check_base_url(&config.ollama_base_url, "ollama_base_url") {
                tracing::debug!(error = %e, "refusing to build Ollama embeddings");
                return None;
            }
            tracing::debug!(model = %config.model, url = %config.ollama_base_url, dim = %config.dimension, "embeddings via Ollama");
            Some(Arc::new(OllamaEmbeddings::new(&config.ollama_base_url)
                .with_model(&config.model, config.dimension)) as Arc<dyn brassclaw_embeddings::EmbeddingProvider>)
        }
        _ => {
            // OpenAI / OpenAI-compatible fallback.
            let Some(api_key) = config.openai_api_key() else {
                tracing::debug!("embeddings configured but API key not set");
                return None;
            };
            let mut provider = OpenAiEmbeddings::with_model(api_key, &config.model, config.dimension);
            if let Some(ref base_url) = config.openai_base_url {
                if let Err(e) = check_base_url(base_url, "openai_base_url") {
                    tracing::debug!(error = %e, "refusing to build OpenAI embeddings");
                    return None;
                }
                tracing::debug!(model = %config.model, base_url = %base_url, dim = %config.dimension, "embeddings via OpenAI-compatible");
                provider = provider.with_base_url(base_url);
            } else {
                tracing::debug!(model = %config.model, dim = %config.dimension, "embeddings via OpenAI");
            }
            Some(Arc::new(provider) as Arc<dyn brassclaw_embeddings::EmbeddingProvider>)
        }
    }
}

// ---------------------------------------------------------------------------
// Concrete provider implementations (crate-private)
// ---------------------------------------------------------------------------

// ── OpenAI / OpenAI-compatible ────────────────────────────────────────────

use async_trait::async_trait;
use brassclaw_embeddings::{EmbeddingError, EmbeddingProvider};
use serde::{Deserialize, Serialize};

const OPENAI_API_BASE_URL: &str = "https://api.openai.com";

struct OpenAiEmbeddings {
    client: reqwest::Client,
    api_key: String,
    model: String,
    dimension: usize,
    base_url: String,
}

impl OpenAiEmbeddings {
    fn with_model(api_key: impl Into<String>, model: impl Into<String>, dimension: usize) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.into(),
            model: model.into(),
            dimension,
            base_url: OPENAI_API_BASE_URL.to_string(),
        }
    }

    fn with_base_url(mut self, base_url: &str) -> Self {
        let url = base_url.trim();
        let mut url = if !url.starts_with("http://") && !url.starts_with("https://") {
            format!("https://{url}")
        } else {
            url.to_string()
        };
        while url.ends_with('/') {
            url.pop();
        }
        self.base_url = url;
        self
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
    embedding: Vec<f32>,
}

#[async_trait]
impl EmbeddingProvider for OpenAiEmbeddings {
    fn dimension(&self) -> usize { self.dimension }
    fn model_name(&self) -> &str { &self.model }
    fn max_input_length(&self) -> usize { 32_000 }

    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        if text.len() > self.max_input_length() {
            return Err(EmbeddingError::TextTooLong { length: text.len(), max: self.max_input_length() });
        }
        self.embed_batch(&[text.to_string()]).await?
            .into_iter().next()
            .ok_or_else(|| EmbeddingError::InvalidResponse("no embedding returned".into()))
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() { return Ok(Vec::new()); }
        let request = OpenAiEmbeddingRequest { model: &self.model, input: texts };
        let url = format!("{}/v1/embeddings", self.base_url);
        let response = self.client.post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send().await
            .map_err(|e| EmbeddingError::HttpError(e.to_string()))?;
        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(EmbeddingError::AuthFailed);
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = Some(brassclaw_llm::retry::parse_retry_after(
                response.headers().get("retry-after"),
            ));
            return Err(EmbeddingError::RateLimited { retry_after });
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(EmbeddingError::HttpError(format!("Status {status}: {body}")));
        }
        let result: OpenAiEmbeddingResponse = response.json().await
            .map_err(|e| EmbeddingError::InvalidResponse(format!("parse error: {e}")))?;
        Ok(result.data.into_iter().map(|d| d.embedding).collect())
    }
}

// ── NEAR AI ───────────────────────────────────────────────────────────────

#[cfg(feature = "root-llm-provider")]
struct NearAiEmbeddings {
    client: reqwest::Client,
    base_url: String,
    session: Arc<brassclaw_llm::SessionManager>,
    model: String,
    dimension: usize,
}

#[cfg(feature = "root-llm-provider")]
impl NearAiEmbeddings {
    fn new(base_url: impl Into<String>, session: Arc<brassclaw_llm::SessionManager>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.into(),
            session,
            model: "text-embedding-3-small".to_string(),
            dimension: 1536,
        }
    }

    fn with_model(mut self, model: impl Into<String>, dimension: usize) -> Self {
        self.model = model.into();
        self.dimension = dimension;
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
#[derive(Debug, Deserialize)]
struct NearAiEmbeddingResponse {
    data: Vec<NearAiEmbeddingData>,
}

#[cfg(feature = "root-llm-provider")]
#[derive(Debug, Deserialize)]
struct NearAiEmbeddingData {
    embedding: Vec<f32>,
}

#[cfg(feature = "root-llm-provider")]
#[async_trait]
impl EmbeddingProvider for NearAiEmbeddings {
    fn dimension(&self) -> usize { self.dimension }
    fn model_name(&self) -> &str { &self.model }
    fn max_input_length(&self) -> usize { 32_000 }

    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        if text.len() > self.max_input_length() {
            return Err(EmbeddingError::TextTooLong { length: text.len(), max: self.max_input_length() });
        }
        self.embed_batch(&[text.to_string()]).await?
            .into_iter().next()
            .ok_or_else(|| EmbeddingError::InvalidResponse("no embedding returned".into()))
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        use secrecy::ExposeSecret as _;
        if texts.is_empty() { return Ok(Vec::new()); }
        let request = NearAiEmbeddingRequest { model: &self.model, input: texts };
        let token = self.session.get_token().await
            .map_err(|_| EmbeddingError::AuthFailed)?;
        let url = format!("{}/v1/embeddings", self.base_url);
        let response = self.client.post(&url)
            .header("Authorization", format!("Bearer {}", token.expose_secret()))
            .json(&request)
            .send().await
            .map_err(|e| EmbeddingError::HttpError(e.to_string()))?;
        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(EmbeddingError::AuthFailed);
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = Some(brassclaw_llm::retry::parse_retry_after(
                response.headers().get("retry-after"),
            ));
            return Err(EmbeddingError::RateLimited { retry_after });
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(EmbeddingError::HttpError(format!("Status {status}: {body}")));
        }
        let result: NearAiEmbeddingResponse = response.json().await
            .map_err(|e| EmbeddingError::InvalidResponse(format!("parse error: {e}")))?;
        Ok(result.data.into_iter().map(|d| d.embedding).collect())
    }
}

// ── Ollama ────────────────────────────────────────────────────────────────

struct OllamaEmbeddings {
    client: reqwest::Client,
    base_url: String,
    model: String,
    dimension: usize,
}

impl OllamaEmbeddings {
    fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.into(),
            model: "nomic-embed-text".to_string(),
            dimension: 768,
        }
    }

    fn with_model(mut self, model: impl Into<String>, dimension: usize) -> Self {
        self.model = model.into();
        self.dimension = dimension;
        self
    }
}

#[derive(Debug, Serialize)]
struct OllamaEmbedRequest<'a> {
    model: &'a str,
    input: &'a [String],
}

#[derive(Debug, Deserialize)]
struct OllamaEmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

#[async_trait]
impl EmbeddingProvider for OllamaEmbeddings {
    fn dimension(&self) -> usize { self.dimension }
    fn model_name(&self) -> &str { &self.model }
    fn max_input_length(&self) -> usize { 32_000 }

    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        if text.len() > self.max_input_length() {
            return Err(EmbeddingError::TextTooLong { length: text.len(), max: self.max_input_length() });
        }
        self.embed_batch(&[text.to_string()]).await?
            .into_iter().next()
            .ok_or_else(|| EmbeddingError::InvalidResponse("no embedding returned".into()))
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() { return Ok(Vec::new()); }
        let request = OllamaEmbedRequest { model: &self.model, input: texts };
        let url = format!("{}/api/embed", self.base_url);
        let response = self.client.post(&url).json(&request).send().await
            .map_err(|e| EmbeddingError::HttpError(e.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(EmbeddingError::HttpError(format!("Ollama {status}: {body}")));
        }
        let result: OllamaEmbedResponse = response.json().await
            .map_err(|e| EmbeddingError::InvalidResponse(format!("parse error: {e}")))?;
        for (i, emb) in result.embeddings.iter().enumerate() {
            if emb.len() != self.dimension {
                return Err(EmbeddingError::InvalidResponse(format!(
                    "dimension mismatch at index {i}: got {}, expected {}",
                    emb.len(), self.dimension
                )));
            }
        }
        Ok(result.embeddings)
    }
}
