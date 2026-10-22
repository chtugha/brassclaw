//! LLM integration for the agent.
//!
//! Supports multiple backends:
//! - **NEAR AI** (default): Session token or API key auth via Chat Completions API
//! - **OpenAI**: Direct API access with your own key
//! - **Anthropic**: Direct API access with your own key
//! - **Ollama**: Local model inference
//! - **OpenAI-compatible**: Any endpoint that speaks the OpenAI API
//! - **AWS Bedrock**: Native Converse API via aws-sdk-bedrockruntime
#![warn(unreachable_pub)]

mod anthropic_oauth;
mod anthropic_thinking;
pub mod auth;
#[cfg(feature = "bedrock")]
mod bedrock;
pub mod circuit_breaker;
pub(crate) mod codex_auth;
mod codex_chatgpt;
pub mod config;
pub mod costs;
mod embedding_models;
pub mod error;
pub mod failover;
pub(crate) mod gemini_oauth;
mod github_copilot;
pub(crate) mod github_copilot_auth;
pub mod host;
pub mod nearai_chat;
pub mod openai_codex_provider;
pub(crate) mod openai_codex_session;
mod optional_auth_http;
mod provider;
pub mod provider_observation;
mod reasoning;
pub mod recording;
pub mod registry;
#[cfg(feature = "registry-provider-factory")]
mod resolution;
mod response_body;
pub mod response_cache;
mod responses_reasoning;
pub mod retry;
mod rig_adapter;
pub mod role;
pub mod runtime;
pub mod session;
pub mod smart_routing;
mod token_refreshing;
pub mod tool_schema;
pub mod transcription;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

#[cfg(test)]
mod codex_test_helpers;

pub mod image_models;
pub mod model_discovery;
pub mod reasoning_models;
pub mod vision_models;

#[cfg(all(test, feature = "registry-provider-factory"))]
mod single_submission_tests;

pub use circuit_breaker::{CircuitBreakerConfig, CircuitBreakerProvider};
pub use config::{
    BedrockConfig, CacheRetention, GeminiOauthConfig, LlmBackendKind, LlmConfig, NearAiConfig,
    OAUTH_PLACEHOLDER, OpenAiCodexConfig, ProviderRequestPolicy, RegistryProviderConfig,
};
pub use embedding_models::{EmbeddingModelContract, EmbeddingModelContracts};
mod model_token_limits;
pub use error::{LlmConfigError, LlmError};
pub use failover::{CooldownConfig, FailoverProvider};
pub(crate) use gemini_oauth::GeminiOauthProvider;
pub use host::{
    NoopKeyPersistor, NoopSessionRenewer, SessionDb, SessionKeyPersistor, SessionRenewer,
    SessionSecrets, SharedSessionDb, SharedSessionKeyPersistor, SharedSessionRenewer,
    SharedSessionSecrets,
};
pub use model_token_limits::{ModelTokenLimitOverrides, ModelTokenLimits};
pub use nearai_chat::{DEFAULT_MODEL, ModelInfo, NearAiChatProvider, default_models};
pub use openai_codex_provider::OpenAiCodexProvider;
pub use openai_codex_session::{DeviceCodeStart, OpenAiCodexSessionManager};
pub use provider::sanitize_tool_messages;
pub use provider::{
    ChatMessage, CompletionRequest, CompletionResponse, ContentPart, FinishReason, ImageUrl,
    LlmProvider, ModelMetadata, ProviderDispatchPolicy, RetainedLlmProvider, Role, ToolCall,
    ToolCompletionRequest, ToolCompletionResponse, ToolDefinition, ToolResult,
    generate_tool_call_id, normalized_model_override,
};
pub use reasoning::{
    ActionPlan, Reasoning, ReasoningContext, RespondOutput, RespondResult, ResponseAnomaly,
    ResponseMetadata, SILENT_REPLY_TOKEN, TOOL_INTENT_NUDGE, TRUNCATED_TOOL_CALL_NOTICE,
    TokenUsage, ToolSelection, is_silent_reply,
};
pub use reasoning::{
    clean_response, contains_codex_text_tool_call_syntax,
    recover_codex_text_tool_calls_from_content, recover_codex_text_tool_calls_from_tool_names,
    recover_tool_calls_from_content,
};
pub use recording::{MemorySnapshotEntry, RecordingLlm};
pub use registry::{ProviderDefinition, ProviderProtocol, ProviderRegistry, ProviderTokenBudget};
#[cfg(feature = "registry-provider-factory")]
pub use resolution::{
    ProviderResolutionError, ProviderSelection, ResolvedDedicatedProviderConfig,
    ResolvedProviderConfig, build_llm_config_with_provider_policy,
    resolve_provider_config_with_credentials,
};
pub use response_body::{
    ResponseBodyError as ProviderResponseBodyError, read as read_provider_response_body,
};
pub use response_cache::{CachedProvider, ResponseCacheConfig, ResponseCachePolicy};
pub use retry::{RetryConfig, RetryProvider, RetryTimingPolicy};
pub use rig_adapter::{RigAdapter, RigUsageFormat};
pub use role::ProviderRole;
pub use runtime::{LlmReloadHandle, SwappableLlmProvider};
pub use session::{NearWalletSignedMessage, SessionConfig, SessionManager, create_session_manager};
pub use smart_routing::{SmartRoutingConfig, SmartRoutingProvider, TaskComplexity};
pub use token_refreshing::TokenRefreshingProvider;

use std::sync::Arc;

use rig::client::CompletionClient;
use secrecy::ExposeSecret;

// LlmConfig, NearAiConfig, RegistryProviderConfig, and LlmError are
// re-exported via `pub use` above from config and error submodules.

/// Create an LLM provider based on configuration.
///
/// - NearAI backend: Uses session manager for authentication
/// - Registry providers: Looked up by protocol and constructed generically
pub async fn create_llm_provider(
    config: &LlmConfig,
    session: Arc<SessionManager>,
) -> Result<Arc<dyn LlmProvider>, LlmError> {
    let timeout = config.request_timeout_secs;
    if config.max_response_bytes.is_some()
        && matches!(config.backend_kind(), LlmBackendKind::Bedrock)
    {
        return Err(LlmError::RequestFailed {
            provider: config.active_provider_id(),
            reason: "this adapter does not yet support a response-body limit".into(),
        });
    }

    if matches!(config.backend_kind(), LlmBackendKind::NearAi) {
        return create_nearai_bounded(&config.nearai, session, timeout, config.max_response_bytes);
    }

    if matches!(config.backend_kind(), LlmBackendKind::GeminiOauth) {
        return create_gemini_oauth_provider(config);
    }

    // Bedrock uses a native AWS SDK, not the rig-core registry
    if matches!(config.backend_kind(), LlmBackendKind::Bedrock) {
        #[cfg(feature = "bedrock")]
        {
            return create_bedrock_provider(config).await;
        }
        #[cfg(not(feature = "bedrock"))]
        {
            return Err(LlmError::RequestFailed {
                provider: "bedrock".to_string(),
                reason: "Bedrock support not compiled. Rebuild with --features bedrock".to_string(),
            });
        }
    }

    if matches!(config.backend_kind(), LlmBackendKind::OpenAiCodex) {
        return Err(LlmError::RequestFailed {
            provider: "openai_codex".to_string(),
            reason:
                "OpenAI Codex uses a dedicated factory path. Use build_provider_chain() instead of create_llm_provider()."
                    .to_string(),
        });
    }

    let reg_config = config
        .provider
        .as_ref()
        .ok_or_else(|| LlmError::AuthFailed {
            provider: config.backend.clone(),
        })?;

    create_registry_provider_bounded(reg_config, timeout, config.max_response_bytes)
}

/// Create an LLM provider from a `NearAiConfig` directly.
///
/// This is useful when constructing additional providers for failover,
/// where only the model name differs from the primary config.
pub fn create_llm_provider_with_config(
    config: &NearAiConfig,
    session: Arc<SessionManager>,
    request_timeout_secs: u64,
) -> Result<Arc<dyn LlmProvider>, LlmError> {
    create_nearai_bounded(config, session, request_timeout_secs, None)
}
fn create_nearai_bounded(
    config: &NearAiConfig,
    session: Arc<SessionManager>,
    request_timeout_secs: u64,
    max_response_bytes: Option<u64>,
) -> Result<Arc<dyn LlmProvider>, LlmError> {
    let auth_mode = if config.api_key.is_some() {
        "API key"
    } else {
        "session token"
    };
    tracing::debug!(
        model = %config.model,
        base_url = %config.base_url,
        auth = auth_mode,
        timeout_secs = request_timeout_secs,
        "Using NEAR AI (Chat Completions API)"
    );
    Ok(Arc::new(NearAiChatProvider::new_with_response_limit(
        config.clone(),
        session,
        true,
        request_timeout_secs,
        max_response_bytes,
    )?))
}

/// Create a provider from a registry-resolved config.
///
/// Dispatches on `RegistryProviderConfig::protocol` to build the appropriate
/// rig-core client. Exposed only for composition roots that already own
/// provider resolution and intentionally opt into the registry factory API;
/// normal callers should use `create_llm_provider` / `build_provider_chain`.
#[cfg(feature = "registry-provider-factory")]
pub fn create_registry_provider(
    config: &RegistryProviderConfig,
    request_timeout_secs: u64,
) -> Result<Arc<dyn LlmProvider>, LlmError> {
    create_registry_provider_inner(config, request_timeout_secs)
}

#[cfg(any(test, feature = "registry-provider-factory"))]
fn create_registry_provider_inner(
    config: &RegistryProviderConfig,
    request_timeout_secs: u64,
) -> Result<Arc<dyn LlmProvider>, LlmError> {
    create_registry_provider_bounded(config, request_timeout_secs, None)
}

fn create_registry_provider_bounded(
    config: &RegistryProviderConfig,
    request_timeout_secs: u64,
    max_response_bytes: Option<u64>,
) -> Result<Arc<dyn LlmProvider>, LlmError> {
    if config.is_codex_chatgpt && max_response_bytes.is_some() {
        return Err(LlmError::RequestFailed {
            provider: config.provider_id.clone(),
            reason: "the legacy Codex ChatGPT adapter does not yet support a response-body limit"
                .into(),
        });
    }
    // Codex ChatGPT mode: use the Responses API provider
    if config.is_codex_chatgpt {
        return create_codex_chatgpt_from_registry(config, request_timeout_secs);
    }

    match config.protocol {
        ProviderProtocol::OpenAiCompletions => {
            create_openai_compat_from_registry(config, request_timeout_secs, max_response_bytes)
        }
        ProviderProtocol::Anthropic => {
            create_anthropic_from_registry(config, request_timeout_secs, max_response_bytes)
        }
        ProviderProtocol::Ollama => {
            create_ollama_from_registry(config, request_timeout_secs, max_response_bytes)
        }
        ProviderProtocol::DeepSeek => {
            create_deepseek_from_registry(config, request_timeout_secs, max_response_bytes)
        }
        ProviderProtocol::Gemini => {
            create_gemini_from_registry(config, request_timeout_secs, max_response_bytes)
        }
        ProviderProtocol::OpenRouter => {
            create_openrouter_from_registry(config, request_timeout_secs, max_response_bytes)
        }
        ProviderProtocol::GithubCopilot => {
            let provider = github_copilot::GithubCopilotProvider::new(
                config,
                request_timeout_secs,
                max_response_bytes,
            )?;
            tracing::debug!(
                provider = %config.provider_id,
                model = %config.model,
                base_url = %config.base_url,
                "Using GitHub Copilot provider (token exchange)"
            );
            Ok(Arc::new(provider))
        }
        // Protocols with a dedicated config slot on `LlmConfig` are
        // dispatched in `create_llm_provider` before this function is
        // reached. They never carry a `RegistryProviderConfig`, so this
        // arm is only reachable as an internal logic bug.
        ProviderProtocol::Bedrock
        | ProviderProtocol::OpenAiCodex
        | ProviderProtocol::GeminiOauth
        | ProviderProtocol::NearAi => Err(LlmError::RequestFailed {
            provider: config.provider_id.clone(),
            reason: format!(
                "Provider '{}' uses a dedicated config slot on LlmConfig and \
                 must be dispatched in create_llm_provider, not via \
                 RegistryProviderConfig.",
                config.provider_id
            ),
        }),
    }
}

fn create_codex_chatgpt_from_registry(
    config: &RegistryProviderConfig,
    request_timeout_secs: u64,
) -> Result<Arc<dyn LlmProvider>, LlmError> {
    let api_key = config
        .api_key
        .as_ref()
        .cloned()
        .ok_or_else(|| LlmError::AuthFailed {
            provider: "codex_chatgpt".to_string(),
        })?;

    tracing::debug!(
        configured_model = %config.model,
        base_url = %config.base_url,
        "Using Codex ChatGPT provider (Responses API) — model detection deferred to first call"
    );

    let provider = codex_chatgpt::CodexChatGptProvider::with_lazy_model(
        &config.base_url,
        api_key,
        &config.model,
        config.refresh_token.clone(),
        config.auth_path.clone(),
        request_timeout_secs,
    )?;

    Ok(Arc::new(provider))
}

#[cfg(feature = "bedrock")]
async fn create_bedrock_provider(config: &LlmConfig) -> Result<Arc<dyn LlmProvider>, LlmError> {
    let br = config
        .bedrock
        .as_ref()
        .ok_or_else(|| LlmError::AuthFailed {
            provider: "bedrock".to_string(),
        })?;

    let provider = bedrock::BedrockProvider::new(br).await?;
    tracing::debug!(
        "Using AWS Bedrock (Converse API, region: {}, model: {})",
        br.region,
        provider.active_model_name(),
    );

    Ok(Arc::new(provider))
}

/// One explicit deadline/redirect policy for registry SDK adapters. Clients
/// remain owned by their provider and are reused across its requests.
fn registry_http_client(provider: &str, timeout_secs: u64) -> Result<reqwest::Client, LlmError> {
    let timeout = std::time::Duration::from_secs(timeout_secs);
    if timeout.is_zero() || std::time::Instant::now().checked_add(timeout).is_none() {
        return Err(LlmError::RequestFailed {
            provider: provider.into(),
            reason: "provider request deadline is invalid".into(),
        });
    }
    reqwest::Client::builder()
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .build()
        .map_err(|_| LlmError::RequestFailed {
            provider: provider.into(),
            reason: "provider HTTP client could not be constructed".into(),
        })
}

fn registry_extra_headers(
    config: &RegistryProviderConfig,
) -> Result<reqwest::header::HeaderMap, LlmError> {
    let mut headers = reqwest::header::HeaderMap::new();
    for (name, value) in &config.extra_headers {
        let invalid = || LlmError::RequestFailed {
            provider: config.provider_id.clone(),
            reason: "configured provider header is invalid".into(),
        };
        let name =
            reqwest::header::HeaderName::from_bytes(name.as_bytes()).map_err(|_| invalid())?;
        let mut value = reqwest::header::HeaderValue::from_str(value).map_err(|_| invalid())?;
        value.set_sensitive(true);
        headers.insert(name, value);
    }
    Ok(headers)
}

fn create_openai_compat_from_registry(
    config: &RegistryProviderConfig,
    request_timeout_secs: u64,
    max_response_bytes: Option<u64>,
) -> Result<Arc<dyn LlmProvider>, LlmError> {
    use rig::providers::openai;

    let extra_headers = registry_extra_headers(config)?;

    let auth_override = config
        .api_key
        .is_none()
        .then(|| extra_headers.get(reqwest::header::AUTHORIZATION).cloned());
    let api_key = config
        .api_key
        .as_ref()
        .map(|k| k.expose_secret().to_string())
        .unwrap_or_default();

    // The SDK requires its bearer-key builder type even for no-auth compatible
    // servers. Remove that generated header before transport when no key exists.
    let mut builder = openai::Client::<reqwest::Client>::builder()
        .api_key(&api_key)
        .http_client(optional_auth_http::OptionalAuthHttp {
            client: registry_http_client(&config.provider_id, request_timeout_secs)?,
            auth_override,
            query_key_header: None,
            max_response_bytes,
        });
    if !config.base_url.is_empty() {
        let base_url = normalize_openai_base_url(&config.base_url);
        builder = builder.base_url(&base_url);
    }
    if !extra_headers.is_empty() {
        builder = builder.http_headers(extra_headers);
    }

    let client: openai::Client<optional_auth_http::OptionalAuthHttp> =
        builder.build().map_err(|e| LlmError::RequestFailed {
            provider: config.provider_id.clone(),
            reason: format!("Failed to create OpenAI-compatible client: {e}"),
        })?;

    // Use CompletionsClient (Chat Completions API) instead of the default
    // Client (Responses API). The Responses API path in rig-core handles
    // tool results differently, which breaks BrassClaw's tool call flow.
    let client = client.completions_api();
    let model = client.completion_model(&config.model);

    tracing::debug!(
        provider = %config.provider_id,
        model = %config.model,
        base_url = %config.base_url,
        "Using OpenAI-compatible provider"
    );

    let adapter = RigAdapter::new(model, &config.model)
        .with_usage_format(crate::rig_adapter::RigUsageFormat::OpenAi)
        .with_request_format(crate::rig_adapter::RigRequestFormat::OpenAiChat)
        .with_unsupported_params(config.unsupported_params.clone());
    Ok(Arc::new(adapter))
}

fn create_anthropic_from_registry(
    config: &RegistryProviderConfig,
    request_timeout_secs: u64,
    max_response_bytes: Option<u64>,
) -> Result<Arc<dyn LlmProvider>, LlmError> {
    // Route to OAuth provider when an OAuth token is present and no real API
    // key was provided. When both are set, the API key takes priority (standard
    // x-api-key auth via rig-core).
    let api_key_is_placeholder = config
        .api_key
        .as_ref()
        .is_some_and(|k| k.expose_secret() == crate::config::OAUTH_PLACEHOLDER);
    if config.oauth_token.is_some() && (config.api_key.is_none() || api_key_is_placeholder) {
        tracing::debug!(
            provider = %config.provider_id,
            model = %config.model,
            base_url = if config.base_url.is_empty() { "default" } else { &config.base_url },
            "Using Anthropic OAuth API"
        );
        let provider = anthropic_oauth::AnthropicOAuthProvider::new(
            config,
            request_timeout_secs,
            max_response_bytes,
        )?;
        return Ok(Arc::new(provider));
    }

    use crate::config::CacheRetention;
    use rig::providers::anthropic;

    let api_key = config
        .api_key
        .as_ref()
        .map(|k| k.expose_secret().to_string())
        .ok_or_else(|| LlmError::AuthFailed {
            provider: config.provider_id.clone(),
        })?;

    let mut builder = anthropic::Client::<reqwest::Client>::builder()
        .api_key(&api_key)
        .http_client(optional_auth_http::OptionalAuthHttp {
            client: registry_http_client(&config.provider_id, request_timeout_secs)?,
            auth_override: None,
            query_key_header: None,
            max_response_bytes,
        });
    if !config.base_url.is_empty() {
        builder = builder.base_url(&config.base_url);
    }
    let client: anthropic::Client<optional_auth_http::OptionalAuthHttp> =
        builder.build().map_err(|_| LlmError::RequestFailed {
            provider: config.provider_id.clone(),
            reason: "provider SDK client could not be constructed".into(),
        })?;

    let cache_retention = config.cache_retention;

    let model = client.completion_model(&config.model);

    if cache_retention != CacheRetention::None {
        tracing::debug!(
            model = %config.model,
            retention = %cache_retention,
            "Anthropic automatic prompt caching enabled"
        );
    }

    tracing::debug!(
        provider = %config.provider_id,
        model = %config.model,
        base_url = if config.base_url.is_empty() { "default" } else { &config.base_url },
        "Using Anthropic provider"
    );

    Ok(Arc::new(
        RigAdapter::new(model, &config.model)
            .with_usage_format(crate::rig_adapter::RigUsageFormat::Anthropic)
            .with_request_format(crate::rig_adapter::RigRequestFormat::Anthropic)
            .with_cache_retention(cache_retention)
            .with_unsupported_params(config.unsupported_params.clone()),
    ))
}

fn create_ollama_from_registry(
    config: &RegistryProviderConfig,
    request_timeout_secs: u64,
    max_response_bytes: Option<u64>,
) -> Result<Arc<dyn LlmProvider>, LlmError> {
    use rig::client::Nothing;
    use rig::providers::ollama;

    let client: ollama::Client<optional_auth_http::OptionalAuthHttp> =
        ollama::Client::<reqwest::Client>::builder()
            .http_client(optional_auth_http::OptionalAuthHttp {
                client: registry_http_client(&config.provider_id, request_timeout_secs)?,
                auth_override: None,
                query_key_header: None,
                max_response_bytes,
            })
            .base_url(&config.base_url)
            .api_key(Nothing)
            .build()
            .map_err(|e| LlmError::RequestFailed {
                provider: config.provider_id.clone(),
                reason: format!("Failed to create Ollama client: {e}"),
            })?;

    let model = client.completion_model(&config.model);

    tracing::debug!(
        provider = %config.provider_id,
        model = %config.model,
        base_url = %config.base_url,
        "Using Ollama provider"
    );

    // Ollama's native /api/chat requires `think: true` to enable extended
    // reasoning for thinking models (Qwen3, DeepSeek-R1, Gemma 4, etc.).
    // Non-thinking models ignore the parameter harmlessly.
    let adapter = RigAdapter::new(model, &config.model)
        .with_usage_format(crate::rig_adapter::RigUsageFormat::Ollama)
        .with_request_format(crate::rig_adapter::RigRequestFormat::Ollama)
        .with_unsupported_params(config.unsupported_params.clone())
        .with_additional_params(serde_json::json!({ "think": true }));
    Ok(Arc::new(adapter))
}

/// Build a DeepSeek provider via rig-core's dedicated DeepSeek client.
///
/// Routing through this client (rather than the generic OpenAI-compat path)
/// is what makes thinking-mode tool calling work: rig-core's DeepSeek
/// implementation captures `reasoning_content` from each response and writes
/// it back onto the assistant message in the next request. Without that
/// round-trip the API rejects the second turn with HTTP 400 ("The
/// reasoning_content in the thinking mode must be passed back to the API").
/// See #3201.
fn create_deepseek_from_registry(
    config: &RegistryProviderConfig,
    request_timeout_secs: u64,
    max_response_bytes: Option<u64>,
) -> Result<Arc<dyn LlmProvider>, LlmError> {
    use rig::providers::deepseek;

    let api_key = config
        .api_key
        .as_ref()
        .map(|k| k.expose_secret().to_string())
        .ok_or_else(|| LlmError::AuthFailed {
            provider: config.provider_id.clone(),
        })?;

    let mut builder = deepseek::Client::<reqwest::Client>::builder()
        .api_key(&api_key)
        .http_client(optional_auth_http::OptionalAuthHttp {
            client: registry_http_client(&config.provider_id, request_timeout_secs)?,
            auth_override: None,
            query_key_header: None,
            max_response_bytes,
        });
    if !config.base_url.is_empty() {
        builder = builder.base_url(&config.base_url);
    }
    let client: deepseek::Client<optional_auth_http::OptionalAuthHttp> =
        builder.build().map_err(|_| LlmError::RequestFailed {
            provider: config.provider_id.clone(),
            reason: "provider SDK client could not be constructed".into(),
        })?;

    let model = client.completion_model(&config.model);

    tracing::debug!(
        provider = %config.provider_id,
        model = %config.model,
        base_url = if config.base_url.is_empty() { "default" } else { &config.base_url },
        "Using DeepSeek provider (preserves reasoning_content across turns)"
    );

    Ok(Arc::new(
        RigAdapter::new(model, &config.model)
            .with_usage_format(crate::rig_adapter::RigUsageFormat::OpenAi)
            .with_request_format(crate::rig_adapter::RigRequestFormat::ChatMaxTokens)
            .with_unsupported_params(config.unsupported_params.clone()),
    ))
}

/// Build an OpenRouter provider via rig-core's dedicated OpenRouter client.
///
/// Routing through this client (rather than the generic OpenAI-compat path)
/// preserves OpenRouter's `reasoning`, `reasoning_details`, and per-tool-call
/// signatures across turns. The generic OpenAI client strips all of them, so
/// any thinking-mode model accessed via OpenRouter (Claude with thinking,
/// OpenAI o-series, DeepSeek-R1, Gemini 2.5+, Qwen QwQ, …) loses its
/// reasoning artifacts on the assistant message and the next request fails
/// the same way as #3201 / #3225.
fn create_openrouter_from_registry(
    config: &RegistryProviderConfig,
    request_timeout_secs: u64,
    max_response_bytes: Option<u64>,
) -> Result<Arc<dyn LlmProvider>, LlmError> {
    use rig::providers::openrouter;

    let api_key = config
        .api_key
        .as_ref()
        .map(|k| k.expose_secret().to_string())
        .ok_or_else(|| LlmError::AuthFailed {
            provider: config.provider_id.clone(),
        })?;

    // OpenRouter attribution headers (`HTTP-Referer`, `X-Title`) and any other
    // user-configured extras must follow the request through. The `http` crate
    // normalizes header names to lowercase internally, so configuring
    // `HTTP-Referer` or `X-Title` (canonical OpenRouter spelling) parses fine.
    let extra_headers = registry_extra_headers(config)?;

    let mut builder = openrouter::Client::<reqwest::Client>::builder()
        .api_key(&api_key)
        .http_client(optional_auth_http::OptionalAuthHttp {
            client: registry_http_client(&config.provider_id, request_timeout_secs)?,
            auth_override: None,
            query_key_header: None,
            max_response_bytes,
        });
    if !config.base_url.is_empty() {
        builder = builder.base_url(&config.base_url);
    }
    if !extra_headers.is_empty() {
        builder = builder.http_headers(extra_headers);
    }

    let client: openrouter::Client<optional_auth_http::OptionalAuthHttp> =
        builder.build().map_err(|e| LlmError::RequestFailed {
            provider: config.provider_id.clone(),
            reason: format!("Failed to create OpenRouter client: {e}"),
        })?;

    let model = client.completion_model(&config.model);

    tracing::debug!(
        provider = %config.provider_id,
        model = %config.model,
        base_url = if config.base_url.is_empty() { "default" } else { &config.base_url },
        "Using OpenRouter provider (preserves reasoning + signatures across turns)"
    );

    Ok(Arc::new(
        RigAdapter::new(model, &config.model)
            .with_usage_format(crate::rig_adapter::RigUsageFormat::OpenAi)
            .with_request_format(crate::rig_adapter::RigRequestFormat::ChatMaxTokens)
            .with_unsupported_params(config.unsupported_params.clone()),
    ))
}

/// Build a Gemini provider via rig-core's dedicated Gemini client.
///
/// Routing through this client (rather than the generic OpenAI-compat path
/// at `/v1beta/openai`) is what makes Gemini thinking-mode tool calling
/// work: rig-core's Gemini implementation round-trips `thought_signature`
/// on each `functionCall`. Without that round-trip the API rejects the
/// next turn with HTTP 400 ("Function call is missing a thought_signature
/// in functionCall parts"). See #3225.
///
/// This is API-key auth only (`GEMINI_API_KEY`). Users on Gemini OAuth go
/// through the separate `gemini_oauth` backend.
fn create_gemini_from_registry(
    config: &RegistryProviderConfig,
    request_timeout_secs: u64,
    max_response_bytes: Option<u64>,
) -> Result<Arc<dyn LlmProvider>, LlmError> {
    use rig::providers::gemini;

    let api_key = config
        .api_key
        .as_ref()
        .map(|k| k.expose_secret().to_string())
        .ok_or_else(|| LlmError::AuthFailed {
            provider: config.provider_id.clone(),
        })?;

    // Adapt a legacy shim path on the same host; never discard an explicitly
    // selected proxy and send its key to the SDK's public default instead.
    let base_url = sanitize_gemini_base_url(&config.base_url).map_err(|_| LlmError::RequestFailed {
        provider: config.provider_id.clone(), reason: "Gemini endpoint must use HTTP(S) without URL credentials/query/fragment; this SDK requires its native v1beta API".into(),
    })?;
    let mut builder = gemini::Client::<reqwest::Client>::builder()
        .api_key(&api_key)
        .http_client(optional_auth_http::OptionalAuthHttp {
            client: registry_http_client(&config.provider_id, request_timeout_secs)?,
            auth_override: None,
            query_key_header: Some("x-goog-api-key"),
            max_response_bytes,
        });
    if !base_url.is_empty() {
        builder = builder.base_url(&base_url);
    }
    let client: gemini::Client<optional_auth_http::OptionalAuthHttp> =
        builder.build().map_err(|_| LlmError::RequestFailed {
            provider: config.provider_id.clone(),
            reason: "Gemini SDK client could not be constructed".into(),
        })?;

    // Rig 0.30's Gemini Capabilities<H> names the default transport model;
    // its public generic constructor preserves our actual HTTP adapter.
    let model = gemini::CompletionModel::new(client, &config.model);

    tracing::debug!(
        provider = %config.provider_id,
        model = %config.model,
        base_url = if base_url.is_empty() { "default" } else { &base_url },
        "Using Gemini provider (preserves thought_signature across turns)"
    );

    Ok(Arc::new(
        RigAdapter::new(model, &config.model)
            .with_usage_format(crate::rig_adapter::RigUsageFormat::Gemini)
            .with_request_format(crate::rig_adapter::RigRequestFormat::Gemini)
            .with_unsupported_params(config.unsupported_params.clone()),
    ))
}

/// Normalize native/shim v1beta paths while preserving the exact authority
/// and proxy prefix. Stable v1 cannot be silently rewritten by this v1beta SDK.
fn sanitize_gemini_base_url(base_url: &str) -> Result<String, ()> {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    let mut url = reqwest::Url::parse(trimmed).map_err(|_| ())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(());
    }
    let path = url.path().trim_end_matches('/');
    let lower = path.to_ascii_lowercase();
    if lower.ends_with("/v1") || lower.ends_with("/v1/openai") {
        return Err(());
    }
    let path = if lower.ends_with("/v1beta/openai") {
        &path[..path.len() - "/v1beta/openai".len()]
    } else if lower.ends_with("/v1beta") {
        &path[..path.len() - "/v1beta".len()]
    } else {
        path
    };
    let path = path.to_owned();
    url.set_path(&path);
    Ok(url.as_str().trim_end_matches('/').to_owned())
}

/// Create an OpenAI Codex provider with OAuth authentication.
///
/// This is async because it needs to ensure authentication before
/// creating the provider (which requires a valid Bearer token).
///
/// Uses the Responses API (`chatgpt.com/backend-api/codex/responses`)
/// instead of the Chat Completions API, matching OpenClaw's approach.
async fn create_openai_codex_provider(
    config: &LlmConfig,
) -> Result<Arc<dyn LlmProvider>, LlmError> {
    let codex = config
        .openai_codex
        .as_ref()
        .ok_or_else(|| LlmError::AuthFailed {
            provider: "openai_codex".to_string(),
        })?;

    let session_mgr = Arc::new(OpenAiCodexSessionManager::new(codex.clone())?);
    session_mgr.ensure_authenticated().await?;

    let token = session_mgr.get_access_token().await?;

    let provider = Arc::new(OpenAiCodexProvider::new_with_response_limit(
        &codex.model,
        &codex.api_base_url,
        token.expose_secret(),
        config.request_timeout_secs,
        config.max_response_bytes,
    )?);

    tracing::debug!(
        "Using OpenAI Codex (Responses API, model: {}, base: {})",
        codex.model,
        codex.api_base_url,
    );

    Ok(Arc::new(TokenRefreshingProvider::new(
        provider,
        session_mgr,
    )))
}

/// Create a cheap/fast LLM provider for lightweight tasks (heartbeat, routing, evaluation).
///
/// Resolution order:
/// 1. `LLM_CHEAP_MODEL` (generic, works with any backend)
/// 2. `NEARAI_CHEAP_MODEL` (NearAI-only, backward compatibility)
///
/// Returns `None` if no cheap model is configured.
pub fn create_cheap_llm_provider(
    config: &LlmConfig,
    session: Arc<SessionManager>,
) -> Result<Option<Arc<dyn LlmProvider>>, LlmError> {
    let Some(cheap_model) = config.cheap_model_name() else {
        return Ok(None);
    };

    create_cheap_provider_for_backend(config, session, cheap_model)
}

/// Create a cheap provider for a specific backend.
///
/// Handles backend-specific provider construction:
/// - `nearai` — clones NearAiConfig, swaps model, uses `create_llm_provider_with_config`
/// - `bedrock` — returns error (smart routing not yet supported)
/// - All others — clones `RegistryProviderConfig`, swaps model, uses `create_registry_provider`
fn create_cheap_provider_for_backend(
    config: &LlmConfig,
    session: Arc<SessionManager>,
    cheap_model: &str,
) -> Result<Option<Arc<dyn LlmProvider>>, LlmError> {
    if matches!(config.backend_kind(), LlmBackendKind::NearAi) {
        let mut cheap_config = config.nearai.clone();
        cheap_config.model = cheap_model.to_string();
        let provider = create_nearai_bounded(
            &cheap_config,
            session,
            config.request_timeout_secs,
            config.max_response_bytes,
        )?;
        return Ok(Some(provider));
    }

    if matches!(config.backend_kind(), LlmBackendKind::Bedrock) {
        return Err(LlmError::RequestFailed {
            provider: "bedrock".to_string(),
            reason: "Smart routing with cheap model is not supported for Bedrock yet".to_string(),
        });
    }

    if matches!(config.backend_kind(), LlmBackendKind::GeminiOauth) {
        let Some(ref gemini_config) = config.gemini_oauth else {
            return Err(LlmError::RequestFailed {
                provider: "gemini_oauth".to_string(),
                reason: "Gemini OAuth config not available for cheap model".to_string(),
            });
        };
        let mut cheap_gemini_config = gemini_config.clone();
        cheap_gemini_config.model = cheap_model.to_string();
        let provider = GeminiOauthProvider::new(
            cheap_gemini_config,
            config.request_timeout_secs,
            config.max_response_bytes,
        )?;
        return Ok(Some(Arc::new(provider)));
    }

    // Registry-based provider: clone config and swap model
    let reg_config = config.provider.as_ref().ok_or_else(|| LlmError::RequestFailed {
        provider: config.backend.clone(),
        reason: format!(
            "Cannot create cheap provider for backend '{}': no registry provider config available",
            config.backend
        ),
    })?;

    let mut cheap_reg_config = reg_config.clone();
    cheap_reg_config.model = cheap_model.to_string();
    let provider = create_registry_provider_bounded(
        &cheap_reg_config,
        config.request_timeout_secs,
        config.max_response_bytes,
    )?;
    Ok(Some(provider))
}

/// Build the full LLM provider chain with all configured wrappers.
///
/// Applies decorators in this order:
/// 1. Raw provider (from config)
/// 2. RetryProvider (per-provider retry with exponential backoff)
/// 3. SmartRoutingProvider (cheap/primary split when cheap model is configured)
/// 4. FailoverProvider (fallback model when primary fails)
/// 5. CircuitBreakerProvider (fast-fail when backend is degraded)
/// 6. CachedProvider (in-memory response cache)
///
/// Also returns a separate cheap LLM provider for heartbeat/evaluation (not
/// part of the chain — it's a standalone provider for explicitly cheap tasks).
///
/// This is the single source of truth for provider chain construction,
/// called by both `main.rs` and `app.rs`.
///
/// Raw primary + cheap providers as rebuilt from config.
///
/// Used by [`build_provider_chain`] (for startup wiring) and by
/// [`LlmReloadHandle::reload`] (for hot-swap): the latter needs the
/// *unwrapped* primary so it can feed it into the existing
/// [`SwappableLlmProvider`] without stacking another wrapper.
pub(crate) struct ProviderChainComponents {
    pub primary: Arc<dyn LlmProvider>,
    pub cheap: Option<Arc<dyn LlmProvider>>,
}

pub(crate) async fn build_provider_chain_components(
    config: &LlmConfig,
    session: Arc<SessionManager>,
) -> Result<ProviderChainComponents, LlmError> {
    build_provider_chain_components_with_options(config, session, true).await
}

async fn build_provider_chain_components_with_options(
    config: &LlmConfig,
    session: Arc<SessionManager>,
    include_standalone_cheap: bool,
) -> Result<ProviderChainComponents, LlmError> {
    let llm: Arc<dyn LlmProvider> = if matches!(config.backend_kind(), LlmBackendKind::OpenAiCodex)
    {
        create_openai_codex_provider(config).await?
    } else {
        create_llm_provider(config, session.clone()).await?
    };
    tracing::debug!("LLM provider initialized: {}", llm.model_name());

    // 1. Retry — uses top-level LlmConfig fields (resolved from LLM_* env vars
    // with fallback to NEARAI_* for backward compatibility).
    let retry_config = RetryConfig {
        max_retries: config.max_retries,
    };
    let llm: Arc<dyn LlmProvider> = if retry_config.max_retries > 0 {
        tracing::debug!(
            max_retries = retry_config.max_retries,
            "LLM retry wrapper enabled"
        );
        Arc::new(RetryProvider::new_with_timing(
            llm,
            retry_config.clone(),
            config.retry_timing,
        )?)
    } else {
        llm
    };

    // 2. Smart routing (cheap/primary split)
    let llm: Arc<dyn LlmProvider> = if let Some(cheap_model) = config.cheap_model_name() {
        let cheap = create_cheap_provider_for_backend(config, session.clone(), cheap_model)?
            .ok_or_else(|| LlmError::RequestFailed {
                provider: config.backend.clone(),
                reason: format!(
                    "Failed to create cheap provider for model '{cheap_model}' on backend '{}'",
                    config.backend
                ),
            })?;
        let cheap: Arc<dyn LlmProvider> = if retry_config.max_retries > 0 {
            Arc::new(RetryProvider::new_with_timing(
                cheap,
                retry_config.clone(),
                config.retry_timing,
            )?)
        } else {
            cheap
        };
        tracing::debug!(
            primary = %llm.model_name(),
            cheap = %cheap.model_name(),
            "Smart routing enabled"
        );
        Arc::new(SmartRoutingProvider::new(
            llm,
            cheap,
            SmartRoutingConfig {
                cascade_enabled: config.smart_routing_cascade,
                ..SmartRoutingConfig::default()
            },
        ))
    } else {
        llm
    };

    // 3. A NEAR configuration cannot redirect another selected protocol's
    // failed request to a different service or credential owner.
    let nearai_fallback = matches!(config.backend_kind(), LlmBackendKind::NearAi)
        .then_some(config.nearai.fallback_model.as_ref())
        .flatten();
    let llm: Arc<dyn LlmProvider> = if let Some(fallback_model) = nearai_fallback {
        if fallback_model == &config.nearai.model {
            tracing::debug!(
                "fallback_model is the same as primary model, failover may not be effective"
            );
        }
        let mut fallback_config = config.nearai.clone();
        fallback_config.model = fallback_model.clone();
        let fallback = create_nearai_bounded(
            &fallback_config,
            session.clone(),
            config.request_timeout_secs,
            config.max_response_bytes,
        )?;
        tracing::debug!(
            primary = %llm.model_name(),
            fallback = %fallback.model_name(),
            "LLM failover enabled"
        );
        let fallback: Arc<dyn LlmProvider> = if retry_config.max_retries > 0 {
            Arc::new(RetryProvider::new_with_timing(
                fallback,
                retry_config.clone(),
                config.retry_timing,
            )?)
        } else {
            fallback
        };
        let cooldown_config = CooldownConfig {
            cooldown_duration: std::time::Duration::from_secs(config.nearai.failover_cooldown_secs),
            failure_threshold: config.nearai.failover_cooldown_threshold,
        };
        Arc::new(FailoverProvider::with_cooldown(
            vec![llm, fallback],
            cooldown_config,
        )?)
    } else {
        llm
    };

    // 4. Circuit breaker
    let llm: Arc<dyn LlmProvider> = if let Some(threshold) = config.circuit_breaker_threshold {
        let cb_config = CircuitBreakerConfig {
            failure_threshold: threshold,
            recovery_timeout: std::time::Duration::from_secs(config.circuit_breaker_recovery_secs),
            ..CircuitBreakerConfig::default()
        };
        tracing::debug!(
            threshold,
            recovery_secs = config.circuit_breaker_recovery_secs,
            "LLM circuit breaker enabled"
        );
        Arc::new(CircuitBreakerProvider::new(llm, cb_config))
    } else {
        llm
    };

    // 5. Response cache
    let llm: Arc<dyn LlmProvider> = if config.response_cache_enabled {
        let rc_config = ResponseCacheConfig {
            ttl: std::time::Duration::from_secs(config.response_cache_ttl_secs),
            max_entries: config.response_cache_max_entries,
        };
        tracing::debug!(
            ttl_secs = config.response_cache_ttl_secs,
            max_entries = config.response_cache_max_entries,
            "LLM response cache enabled"
        );
        Arc::new(CachedProvider::new(llm, rc_config))
    } else {
        llm
    };

    // Standalone cheap LLM for heartbeat/evaluation (not part of the chain)
    let cheap_llm = if include_standalone_cheap {
        create_cheap_llm_provider(config, session)?
    } else {
        None
    };
    if let Some(ref cheap) = cheap_llm {
        tracing::debug!("Cheap LLM provider initialized: {}", cheap.model_name());
    }

    Ok(ProviderChainComponents {
        primary: llm,
        cheap: cheap_llm,
    })
}

/// Build a primary provider chain for composition roots that do not own
/// hot-reload or standalone cheap-provider lifecycle handles.
pub async fn build_static_provider_chain(
    config: &LlmConfig,
    session: Arc<SessionManager>,
) -> Result<Arc<dyn LlmProvider>, LlmError> {
    let components = build_provider_chain_components_with_options(config, session, false).await?;
    let primary = components.primary;
    let recording_handle = RecordingLlm::from_env(primary.clone());
    Ok(if let Some(recorder) = recording_handle {
        recorder as Arc<dyn LlmProvider>
    } else {
        primary
    })
}

/// Apply a [`CacheRetention`] override to an `Arc<dyn LlmProvider>` produced
/// from registry-driven providers.
///
/// Composition roots can't reach inside `RigAdapter<M>` to call
/// `.with_cache_retention(...)` themselves — the generic `M: CompletionModel`
/// parameter is erased behind a trait object. This helper returns the
/// provider unchanged and logs the override request: the RigAdapter reads
/// `cache_retention` once at construction, so post-construction overrides
/// require routing through the settings hot-reload path (`LlmReloadHandle`).
/// Providers whose model does not support prompt caching (Ollama, vLLM,
/// OpenAI-compat) are returned unchanged — `with_cache_retention` already
/// disables caching for them, so the no-op is safe.
pub fn apply_cache_retention(
    provider: Arc<dyn LlmProvider>,
    retention: CacheRetention,
) -> Arc<dyn LlmProvider> {
    use rig_adapter::supports_prompt_cache;

    let model_name = provider.model_name().to_string();
    if !supports_prompt_cache(&model_name) {
        tracing::debug!(
            model = %model_name,
            retention = %retention,
            "apply_cache_retention: model does not support prompt caching; no-op"
        );
        return provider;
    }
    tracing::debug!(
        provider = %model_name,
        retention = %retention,
        "cache_retention override recorded; effective on next settings-driven provider reload"
    );
    provider
}

/// Build the full provider chain and wrap the primary (and cheap, if any)
/// in hot-swap capable [`SwappableLlmProvider`] handles. The returned
/// [`LlmReloadHandle`] can rebuild the chain later from a fresh config.
///
/// This is the single source of truth for provider chain construction,
/// called by both `main.rs` and `app.rs`.
#[allow(clippy::type_complexity)]
pub async fn build_provider_chain(
    config: &LlmConfig,
    session: Arc<SessionManager>,
) -> Result<
    (
        Arc<dyn LlmProvider>,
        Option<Arc<dyn LlmProvider>>,
        Option<Arc<RecordingLlm>>,
        Arc<LlmReloadHandle>,
    ),
    LlmError,
> {
    let components = build_provider_chain_components(config, session).await?;

    let primary_swappable = Arc::new(SwappableLlmProvider::new(components.primary));
    let cheap_swappable = components
        .cheap
        .map(|cheap| Arc::new(SwappableLlmProvider::new(cheap)));
    let reload_handle = Arc::new(LlmReloadHandle::new(
        Arc::clone(&primary_swappable),
        cheap_swappable.clone(),
    ));

    // 6. Recording (trace capture for replay testing) wraps the swappable
    // wrapper so traces follow the active inner provider across swaps.
    let primary: Arc<dyn LlmProvider> = primary_swappable;
    let recording_handle = RecordingLlm::from_env(primary.clone());
    let primary: Arc<dyn LlmProvider> = if let Some(ref recorder) = recording_handle {
        Arc::clone(recorder) as Arc<dyn LlmProvider>
    } else {
        primary
    };

    let cheap: Option<Arc<dyn LlmProvider>> =
        cheap_swappable.map(|handle| handle as Arc<dyn LlmProvider>);

    Ok((primary, cheap, recording_handle, reload_handle))
}

pub fn create_gemini_oauth_provider(config: &LlmConfig) -> Result<Arc<dyn LlmProvider>, LlmError> {
    let gemini_config = config
        .gemini_oauth
        .clone()
        .ok_or_else(|| LlmError::AuthFailed {
            provider: "gemini_oauth".to_string(),
        })?;
    let provider = gemini_oauth::GeminiOauthProvider::new(
        gemini_config,
        config.request_timeout_secs,
        config.max_response_bytes,
    )?;
    Ok(Arc::new(provider))
}

/// Normalize an OpenAI-compatible base URL by appending `/v1` when the URL
/// contains no path (bare `scheme://host[:port]`).
///
/// rig-core's `openai::Client` does not auto-append `/v1/` to the base URL,
/// so local model servers (MLX, vLLM, llama.cpp) using bare URLs like
/// `http://localhost:8080` get 404s. This mirrors the old
/// `NearAiChatProvider::api_url()` behavior.
///
/// URLs that already carry a path — including non-`/v1` versioned paths such
/// as Zai's `/api/paas/v4` or Gemini's `/v1beta/openai` — are returned
/// unchanged so we don't corrupt provider-specific endpoints.
///
/// **Note:** This is intentionally applied only to `OpenAiCompletions`-protocol
/// providers. Ollama uses `/api/chat` (not `/v1/chat/completions`) and its
/// rig-core client handles the path internally, so normalization is not needed.
fn normalize_openai_base_url(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    if trimmed.to_ascii_lowercase().ends_with("/v1") {
        return trimmed.to_string();
    }
    match url::Url::parse(trimmed) {
        Ok(parsed) if parsed.path().is_empty() || parsed.path() == "/" => {
            format!("{trimmed}/v1")
        }
        _ => trimmed.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NearAiConfig;

    fn test_nearai_config() -> NearAiConfig {
        NearAiConfig {
            model: "test-model".to_string(),
            cheap_model: None,
            base_url: "https://api.near.ai".to_string(),
            api_key: None,
            fallback_model: None,
            max_retries: 3,
            circuit_breaker_threshold: None,
            circuit_breaker_recovery_secs: 30,
            response_cache_enabled: false,
            response_cache_ttl_secs: 3600,
            response_cache_max_entries: 1000,
            failover_cooldown_secs: 300,
            failover_cooldown_threshold: 3,
            smart_routing_cascade: true,
        }
    }

    fn test_llm_config() -> LlmConfig {
        LlmConfig {
            backend: "nearai".to_string(),
            resolved_protocol: None,
            session: SessionConfig::default(),
            nearai: test_nearai_config(),
            provider: None,
            bedrock: None,
            gemini_oauth: None,
            request_timeout_secs: 120,
            retry_timing: Default::default(),
            max_response_bytes: None,
            cheap_model: None,
            smart_routing_cascade: true,
            openai_codex: None,
            max_retries: 3,
            circuit_breaker_threshold: None,
            circuit_breaker_recovery_secs: 30,
            response_cache_enabled: false,
            response_cache_ttl_secs: 3600,
            response_cache_max_entries: 1000,
        }
    }

    #[test]
    fn test_create_cheap_llm_provider_returns_none_when_not_configured() {
        let config = test_llm_config();
        let session = Arc::new(SessionManager::new(SessionConfig::default()));

        let result = create_cheap_llm_provider(&config, session);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_create_cheap_llm_provider_creates_provider_with_nearai_cheap_model() {
        let mut config = test_llm_config();
        config.nearai.cheap_model = Some("cheap-test-model".to_string());

        let session = Arc::new(SessionManager::new(SessionConfig::default()));
        let result = create_cheap_llm_provider(&config, session);

        assert!(result.is_ok());
        let provider = result.unwrap();
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().model_name(), "cheap-test-model");
    }

    #[test]
    fn test_create_cheap_llm_provider_generic_overrides_nearai() {
        let mut config = test_llm_config();
        config.nearai.cheap_model = Some("nearai-cheap".to_string());
        config.cheap_model = Some("generic-cheap".to_string());

        let session = Arc::new(SessionManager::new(SessionConfig::default()));
        let result = create_cheap_llm_provider(&config, session);

        assert!(result.is_ok());
        let provider = result.unwrap();
        assert!(provider.is_some());
        assert_eq!(
            provider.unwrap().model_name(),
            "generic-cheap",
            "LLM_CHEAP_MODEL should take priority over NEARAI_CHEAP_MODEL"
        );
    }

    #[test]
    fn test_create_cheap_llm_provider_nearai_cheap_ignored_for_non_nearai_backend() {
        let mut config = test_llm_config();
        config.backend = "openai".to_string();
        config.nearai.cheap_model = Some("cheap-test-model".to_string());

        let session = Arc::new(SessionManager::new(SessionConfig::default()));
        let result = create_cheap_llm_provider(&config, session);

        assert!(result.is_ok());
        assert!(
            result.unwrap().is_none(),
            "NEARAI_CHEAP_MODEL should be ignored when backend is not nearai"
        );
    }

    #[test]
    fn test_create_cheap_llm_provider_bedrock_returns_error() {
        let mut config = test_llm_config();
        config.backend = "bedrock".to_string();
        config.cheap_model = Some("cheap-model".to_string());

        let session = Arc::new(SessionManager::new(SessionConfig::default()));
        let result = create_cheap_llm_provider(&config, session);

        assert!(
            result.is_err(),
            "Bedrock should return an error for cheap model"
        );
    }

    #[test]
    fn test_create_cheap_llm_provider_gemini_oauth_creates_provider() {
        let mut config = test_llm_config();
        config.backend = "gemini_oauth".to_string();
        config.cheap_model = Some("gemini-2.5-flash-lite".to_string());
        config.gemini_oauth = Some(crate::config::GeminiOauthConfig {
            model: "gemini-2.5-pro".to_string(),
            credentials_path: std::path::PathBuf::from("/tmp/nonexistent-creds.json"),
        });

        let session = Arc::new(SessionManager::new(SessionConfig::default()));
        let result = create_cheap_llm_provider(&config, session);

        // Should succeed and return a provider (credentials validation is deferred
        // until the first LLM call, not at construction time).
        let provider = result.expect("gemini_oauth cheap provider should succeed");
        assert!(provider.is_some(), "Should return Some(provider)");
        assert_eq!(
            provider.unwrap().model_name(),
            "gemini-2.5-flash-lite",
            "Cheap provider should use the overridden model name"
        );
    }

    #[test]
    fn test_cheap_model_name_resolution() {
        // Generic takes priority
        let mut config = test_llm_config();
        config.cheap_model = Some("generic".to_string());
        config.nearai.cheap_model = Some("nearai".to_string());
        assert_eq!(config.cheap_model_name(), Some("generic"));

        // NearAI fallback when backend is nearai
        let mut config = test_llm_config();
        config.nearai.cheap_model = Some("nearai".to_string());
        assert_eq!(config.cheap_model_name(), Some("nearai"));

        // NearAI ignored for non-nearai backend
        let mut config = test_llm_config();
        config.backend = "openai".to_string();
        config.nearai.cheap_model = Some("nearai".to_string());
        assert_eq!(config.cheap_model_name(), None);

        // None when nothing configured
        let config = test_llm_config();
        assert_eq!(config.cheap_model_name(), None);
    }

    /// Exercise the `LlmReloadHandle::reload` path end-to-end: build an
    /// initial chain from a NEAR AI config, call `reload()` with a config
    /// that has a different model, and verify the wrapper now reports the
    /// new model. This is the caller-side coverage for the hot-reload
    /// feature — a unit test on `SwappableLlmProvider::swap` alone does not
    /// catch regressions where `reload()` fails to rebuild the chain.
    #[tokio::test]
    async fn llm_reload_handle_swaps_primary_model_on_reload() {
        let session = Arc::new(SessionManager::new(SessionConfig::default()));

        let mut initial = test_llm_config();
        initial.nearai.model = "model-a".to_string();
        let (primary, _cheap, _recording, reload_handle) =
            build_provider_chain(&initial, Arc::clone(&session))
                .await
                .expect("initial build_provider_chain");
        assert_eq!(primary.model_name(), "model-a");

        let mut updated = test_llm_config();
        updated.nearai.model = "model-b".to_string();
        reload_handle
            .reload(&updated, Arc::clone(&session))
            .await
            .expect("reload should succeed");

        // The primary handle returned from the first build must observe the
        // new model after the swap — callers hold on to this Arc across
        // reloads, so the wrapper identity is preserved.
        assert_eq!(primary.model_name(), "model-b");
        assert_eq!(
            reload_handle.primary_provider().model_name(),
            "model-b",
            "handle should also report the new model",
        );
    }

    /// When `build_provider_chain_components` fails (e.g. backend changed to
    /// one whose credentials are missing), `reload()` must leave the primary
    /// wrapper pointing at the *old* chain. Partial reloads where the
    /// wrapper reports the new model but uses the old inner would be much
    /// worse than a 500 on the setting-write call.
    #[tokio::test]
    async fn llm_reload_handle_preserves_old_chain_on_build_failure() {
        let session = Arc::new(SessionManager::new(SessionConfig::default()));

        let mut initial = test_llm_config();
        initial.nearai.model = "still-good".to_string();
        let (primary, _cheap, _recording, reload_handle) =
            build_provider_chain(&initial, Arc::clone(&session))
                .await
                .expect("initial chain");
        assert_eq!(primary.model_name(), "still-good");

        // Switch to a registry backend without a provider config — this is
        // a deterministic failure path in `create_llm_provider` (returns
        // `AuthFailed`). See `create_llm_provider` above.
        let mut broken = test_llm_config();
        broken.backend = "openai".to_string();
        broken.provider = None;
        let reload_err = reload_handle
            .reload(&broken, Arc::clone(&session))
            .await
            .expect_err("reload must surface the build failure");
        assert!(
            matches!(reload_err, LlmError::AuthFailed { .. }),
            "expected AuthFailed, got {reload_err:?}",
        );

        // The wrapper must still observe the old chain — any other answer
        // would mean callers holding this Arc silently start talking to a
        // half-built provider.
        assert_eq!(primary.model_name(), "still-good");
    }

    /// When the new config omits a cheap model that was present at startup,
    /// `reload()` must fall back to the primary provider rather than leave
    /// the cheap wrapper dangling. This covers the reload asymmetry that
    /// motivated the review feedback.
    #[tokio::test]
    async fn llm_reload_handle_falls_back_to_primary_when_cheap_disappears() {
        let session = Arc::new(SessionManager::new(SessionConfig::default()));

        let mut initial = test_llm_config();
        initial.nearai.model = "primary-a".to_string();
        initial.nearai.cheap_model = Some("cheap-a".to_string());
        let (_primary, cheap, _recording, reload_handle) =
            build_provider_chain(&initial, Arc::clone(&session))
                .await
                .expect("initial build_provider_chain");
        let cheap = cheap.expect("cheap provider wired at startup");
        assert_eq!(cheap.model_name(), "cheap-a");

        let mut updated = test_llm_config();
        updated.nearai.model = "primary-b".to_string();
        updated.nearai.cheap_model = None;
        reload_handle
            .reload(&updated, Arc::clone(&session))
            .await
            .expect("reload should succeed");

        // The cheap wrapper now reflects the primary — not left stale at
        // "cheap-a" — so the chain stays consistent.
        assert_eq!(cheap.model_name(), "primary-b");
    }

    #[test]
    fn test_normalize_openai_base_url_appends_v1_for_bare_hosts() {
        assert_eq!(
            normalize_openai_base_url("http://localhost:8080"),
            "http://localhost:8080/v1"
        );
        assert_eq!(
            normalize_openai_base_url("http://localhost:8080/"),
            "http://localhost:8080/v1"
        );
        assert_eq!(
            normalize_openai_base_url("https://my-server.example.com"),
            "https://my-server.example.com/v1"
        );
    }

    #[test]
    fn test_normalize_openai_base_url_leaves_v1_alone() {
        assert_eq!(
            normalize_openai_base_url("http://localhost:8080/v1"),
            "http://localhost:8080/v1"
        );
        assert_eq!(
            normalize_openai_base_url("http://localhost:8080/v1/"),
            "http://localhost:8080/v1"
        );
        assert_eq!(
            normalize_openai_base_url("https://api.openai.com/v1"),
            "https://api.openai.com/v1"
        );
        // Case-insensitive: /V1 should not get double-suffixed
        assert_eq!(
            normalize_openai_base_url("http://localhost:8080/V1"),
            "http://localhost:8080/V1"
        );
    }

    #[test]
    fn test_normalize_openai_base_url_preserves_existing_paths() {
        // Non-/v1 versioned paths from real providers must stay unchanged
        assert_eq!(
            normalize_openai_base_url("https://api.z.ai/api/paas/v4"),
            "https://api.z.ai/api/paas/v4"
        );
        assert_eq!(
            normalize_openai_base_url("https://generativelanguage.googleapis.com/v1beta/openai"),
            "https://generativelanguage.googleapis.com/v1beta/openai"
        );
        // Custom subpaths should also stay unchanged
        assert_eq!(
            normalize_openai_base_url("https://api.example.com/custom"),
            "https://api.example.com/custom"
        );
    }

    /// Regression for #3225: pre-PR, the configure UI/setup default for
    /// Gemini was the OpenAI shim URL ending in `/v1beta/openai`. Once
    #[tokio::test]
    async fn actual_registry_http_preserves_proxy_credentials_and_request_deadline() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let mut observations = Vec::new();
            for index in 0..3 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                let mut buffer = [0u8; 4096];
                let header_end = loop {
                    let n = socket.read(&mut buffer).await.unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&buffer[..n]);
                    assert!(bytes.len() < 65536);
                    if let Some(end) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                        break end + 4;
                    }
                };
                let headers = String::from_utf8(bytes[..header_end].to_vec()).unwrap();
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .and_then(|v| v.parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                while bytes.len() < header_end + length {
                    let n = socket.read(&mut buffer).await.unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&buffer[..n]);
                    assert!(bytes.len() < 65536);
                }
                observations.push(headers);
                if index == 2 {
                    tokio::time::sleep(std::time::Duration::from_millis(3000)).await;
                    break;
                }
                if index == 1 {
                    socket.write_all(b"HTTP/1.1 302 Found\r\nLocation: /must-not-follow\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap();
                } else {
                    let body = r#"{"responseId":"native-gemini-response","candidates":[{"content":{"role":"model","parts":[{"text":"native proxy answer"}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":1,"candidatesTokenCount":2,"totalTokenCount":3}}"#;
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    socket.write_all(response.as_bytes()).await.unwrap();
                }
            }
            observations
        });
        let gemini = RegistryProviderConfig::generic(
            ProviderProtocol::Gemini,
            "local-gemini",
            Some(secrecy::SecretString::from("native-gemini-fixture-key")),
            format!("http://{address}/proxy/v1beta/openai"),
            "native-gemini-model",
        );
        let mut chain = crate::testing::nearai_test_config("unselected-near-model");
        chain.backend = gemini.provider_id.clone();
        chain.resolved_protocol = Some(ProviderProtocol::Gemini);
        chain.provider = Some(gemini.clone());
        chain.request_timeout_secs = 1;
        chain.nearai.base_url = format!("http://{address}/must-not-fail-over");
        chain.nearai.api_key = Some(secrecy::SecretString::from("unselected-near-fixture-key"));
        chain.nearai.fallback_model = Some("unselected-fallback-model".into());
        let session_dir = tempfile::tempdir().unwrap();
        let session = Arc::new(SessionManager::new(SessionConfig {
            auth_base_url: format!("http://{address}/unused-auth"),
            session_path: session_dir.path().join("session.json"),
        }));
        let provider = build_static_provider_chain(&chain, session).await.unwrap();
        let request = || {
            CompletionRequest::new(vec![ChatMessage::user("native transport check")])
                .with_max_tokens(16)
        };
        assert_eq!(
            provider.complete(request()).await.unwrap().content,
            "native proxy answer"
        );
        let rejected = provider
            .complete(request())
            .await
            .expect_err("redirect must remain an error");
        assert!(!rejected.to_string().contains("native-gemini-fixture-key"));
        let openai = RegistryProviderConfig::generic(
            ProviderProtocol::OpenAiCompletions,
            "local-openai",
            None,
            format!("http://{address}/slow/v1"),
            "native-openai-model",
        );
        let provider = create_registry_provider_inner(&openai, 1).unwrap();
        let started = std::time::Instant::now();
        assert!(provider.complete(request()).await.is_err());
        assert!(
            started.elapsed() < std::time::Duration::from_millis(2500),
            "configured one-second deadline must reach the SDK client"
        );
        let observed = server.await.unwrap();
        assert_eq!(observed.len(), 3);
        for request in &observed[..2] {
            assert!(request.starts_with(
                "POST /proxy/v1beta/models/native-gemini-model:generateContent HTTP/1.1"
            ));
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("x-goog-api-key: native-gemini-fixture-key")
            );
            assert!(!request.contains("?key=") && !request.contains("authorization:"));
        }
        assert!(observed[2].starts_with("POST /slow/v1/chat/completions HTTP/1.1"));
        assert!(!observed[2].contains("authorization:"));
        let mut invalid = openai;
        invalid
            .extra_headers
            .push(("x-invalid".into(), "invalid\r\nvalue".into()));
        assert!(
            create_registry_provider_inner(&invalid, 1).is_err(),
            "malformed configured headers cannot be silently dropped"
        );
        assert!(create_registry_provider_inner(&gemini, 0).is_err());
    }

    #[test]
    fn sanitize_gemini_base_url_retains_authority_and_proxy_prefix() {
        for endpoint in [
            "https://proxy.example/proxy/v1beta/openai",
            "https://proxy.example/proxy/v1beta",
            "https://proxy.example/proxy/v1beta/openai/",
        ] {
            assert_eq!(
                sanitize_gemini_base_url(endpoint).unwrap(),
                "https://proxy.example/proxy"
            );
        }
        assert_eq!(
            sanitize_gemini_base_url("https://Generativelanguage.googleapis.com/V1beta/OpenAI")
                .unwrap(),
            "https://generativelanguage.googleapis.com"
        );
        assert_eq!(
            sanitize_gemini_base_url("https://proxy.example").unwrap(),
            "https://proxy.example"
        );
        assert_eq!(sanitize_gemini_base_url("").unwrap(), "");
        for invalid in [
            "https://proxy.example/v1/openai",
            "https://proxy.example/v1",
            "https://user:credential@proxy.example",
            "https://proxy.example?key=credential",
            "file:///private/provider",
        ] {
            assert!(sanitize_gemini_base_url(invalid).is_err());
        }
    }
}
