//! LLM configuration types.
//!
//! These types define the configuration for LLM providers. They are defined
//! here (in the `llm` module) so that the module is self-contained and can be
//! extracted into a standalone crate. Resolution logic (reading env vars,
//! settings) lives in `crate::config::llm`.

use std::path::PathBuf;

use secrecy::SecretString;

use crate::error::LlmConfigError;
use crate::registry::ProviderProtocol;
use crate::session::SessionConfig;
use brassclaw_common::paths::brassclaw_base_dir;

/// Sentinel value used as `api_key` when only an OAuth token is present.
///
/// When we only have an OAuth token the provider factory in `llm/mod.rs`
/// checks for this value and routes to `AnthropicOAuthProvider`, so this
/// sentinel value is never sent over the wire.
pub const OAUTH_PLACEHOLDER: &str = "oauth-placeholder";

/// Prompt cache retention policy for Anthropic.
///
/// Controls Anthropic's automatic prompt caching via a top-level
/// `cache_control` field injected through rig-core's `additional_params`.
/// - `None` — caching disabled, no `cache_control` injected.
/// - `Short` — 5-minute TTL (default), `{"type": "ephemeral"}`, 1.25× write surcharge.
/// - `Long` — 1-hour TTL, `{"type": "ephemeral", "ttl": "1h"}`, 2× write surcharge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CacheRetention {
    /// No prompt caching.
    None,
    /// 5-minute TTL (default). Write cost: 1.25× base input.
    #[default]
    Short,
    /// 1-hour TTL. Write cost: 2× base input.
    Long,
}

impl std::str::FromStr for CacheRetention {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "none" | "off" | "disabled" => Ok(Self::None),
            "short" | "5m" | "ephemeral" => Ok(Self::Short),
            "long" | "1h" => Ok(Self::Long),
            _ => Err(format!(
                "invalid cache retention '{}', expected one of: none, short, long",
                s
            )),
        }
    }
}

impl std::fmt::Display for CacheRetention {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Short => write!(f, "short"),
            Self::Long => write!(f, "long"),
        }
    }
}

/// Resolved configuration for a registry-based provider.
///
/// This single struct replaces what used to be five separate config types
/// (`OpenAiDirectConfig`, `AnthropicDirectConfig`, `OllamaConfig`,
/// `OpenAiCompatibleConfig`, `TinfoilConfig`). The `protocol` field
/// determines which rig-core client constructor to use.
#[derive(Debug, Clone)]
pub struct RegistryProviderConfig {
    /// Which API protocol to use (determines the rig-core client).
    pub protocol: ProviderProtocol,
    /// Provider identifier (e.g., "groq", "openai", "tinfoil").
    pub provider_id: String,
    /// API key (optional for some providers like Ollama).
    /// For Anthropic OAuth, this is set to `OAUTH_PLACEHOLDER`.
    pub api_key: Option<SecretString>,
    /// Base URL for the API endpoint.
    pub base_url: String,
    /// Model identifier.
    pub model: String,
    /// Extra HTTP headers injected into every request.
    pub extra_headers: Vec<(String, String)>,
    /// OAuth token for providers that support Bearer auth (e.g. Anthropic via `claude login`).
    /// When set, the provider factory routes to the OAuth-specific provider implementation.
    pub oauth_token: Option<SecretString>,
    /// When true, route OpenAI-compatible traffic to the Codex ChatGPT
    /// Responses API provider instead of rig-core's Chat Completions path.
    pub is_codex_chatgpt: bool,
    /// OAuth refresh token for Codex ChatGPT token refresh.
    pub refresh_token: Option<SecretString>,
    /// Path to Codex auth.json for persisting refreshed tokens.
    pub auth_path: Option<PathBuf>,
    /// Prompt cache retention (Anthropic-specific).
    pub cache_retention: CacheRetention,
    /// Parameter names that this provider does not support (e.g., `["temperature"]`).
    /// Supported keys: `"temperature"`, `"max_tokens"`, `"stop_sequences"`.
    /// Listed parameters are stripped from requests before sending to avoid 400 errors.
    pub unsupported_params: Vec<String>,
}

impl RegistryProviderConfig {
    /// Build a generic registry provider config with provider-specific
    /// optional knobs left at their neutral defaults.
    pub fn generic(
        protocol: ProviderProtocol,
        provider_id: impl Into<String>,
        api_key: Option<SecretString>,
        base_url: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            protocol,
            provider_id: provider_id.into(),
            api_key,
            base_url: base_url.into(),
            model: model.into(),
            extra_headers: Vec::new(),
            oauth_token: None,
            is_codex_chatgpt: false,
            refresh_token: None,
            auth_path: None,
            cache_retention: CacheRetention::None,
            unsupported_params: Vec::new(),
        }
    }

    pub fn with_extra_headers(mut self, extra_headers: Vec<(String, String)>) -> Self {
        self.extra_headers = extra_headers;
        self
    }

    pub fn with_unsupported_params(mut self, unsupported_params: Vec<String>) -> Self {
        self.unsupported_params = unsupported_params;
        self
    }
}

/// Technical policy for each model request, independent of VM compute time,
/// model/context selection and completed-response caching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderRequestPolicy {
    pub timeout_secs: u64,
    /// Additional attempts after the initial model request; zero disables retries.
    pub max_retries: u32,
    /// None disables the circuit breaker. A configured threshold is positive.
    pub circuit_breaker_threshold: Option<u32>,
    pub circuit_breaker_recovery_secs: u64,
    #[serde(default)]
    pub retry_timing: crate::retry::RetryTimingPolicy,
    /// Optional total decoded HTTP body ceiling. None keeps unrestricted bodies.
    #[serde(default)]
    pub max_response_bytes: Option<u64>,
}
impl Default for ProviderRequestPolicy {
    fn default() -> Self {
        Self {
            timeout_secs: 120,
            max_retries: 3,
            circuit_breaker_threshold: None,
            circuit_breaker_recovery_secs: 30,
            retry_timing: Default::default(),
            max_response_bytes: None,
        }
    }
}
impl ProviderRequestPolicy {
    pub fn validate_for_protocol(
        self,
        protocol: crate::ProviderProtocol,
    ) -> Result<(), &'static str> {
        self.validate()?;
        if self.max_response_bytes.is_some() && !protocol.supports_response_body_limit() {
            return Err("this adapter does not yet support a response-body limit");
        }
        Ok(())
    }
    pub fn validate(self) -> Result<(), &'static str> {
        if self.timeout_secs == 0
            || self.circuit_breaker_recovery_secs == 0
            || self.circuit_breaker_threshold == Some(0)
            || [self.timeout_secs, self.circuit_breaker_recovery_secs]
                .into_iter()
                .any(|seconds| {
                    std::time::Instant::now()
                        .checked_add(std::time::Duration::from_secs(seconds))
                        .is_none()
                })
        {
            return Err(
                "request deadlines/recovery must be positive and representable; a configured circuit threshold must be positive",
            );
        }
        if self.max_response_bytes.is_some_and(|bytes| {
            bytes == 0
                || usize::try_from(bytes)
                    .ok()
                    .is_none_or(|bytes| bytes > isize::MAX as usize)
        }) {
            return Err(
                "response bytes must be a positive representable capacity, or null for unrestricted bodies",
            );
        }
        self.retry_timing.validate()?;
        Ok(())
    }
}

#[cfg(test)]
mod request_capacity_tests {
    use super::*;
    #[test]
    fn response_capacity_is_explicit_and_supported_before_construction() {
        let defaults = ProviderRequestPolicy::default();
        assert_eq!(defaults.max_response_bytes, None);
        for limit in [0, u64::MAX] {
            assert!(
                ProviderRequestPolicy {
                    max_response_bytes: Some(limit),
                    ..defaults
                }
                .validate()
                .is_err()
            );
        }
        let bounded = ProviderRequestPolicy {
            max_response_bytes: Some(1024),
            ..defaults
        };
        for protocol in [
            crate::ProviderProtocol::OpenAiCompletions,
            crate::ProviderProtocol::NearAi,
            crate::ProviderProtocol::GeminiOauth,
            crate::ProviderProtocol::OpenAiCodex,
        ] {
            bounded.validate_for_protocol(protocol).unwrap();
        }
        assert!(
            bounded
                .validate_for_protocol(crate::ProviderProtocol::Bedrock)
                .is_err()
        );
        defaults
            .validate_for_protocol(crate::ProviderProtocol::Bedrock)
            .unwrap();
    }
}

/// Configuration for OpenAI Codex (ChatGPT subscription OAuth).
#[derive(Debug, Clone)]
pub struct OpenAiCodexConfig {
    /// Model to use (default: "gpt-5.5"). Must be a model the ChatGPT account
    /// is entitled to: codex-only slugs like `gpt-5.3-codex` work with an
    /// API-key Codex account but the subscription backend rejects them with
    /// HTTP 400, and this provider is subscription-only.
    pub model: String,
    /// OAuth authorization server (default: "https://auth.openai.com").
    pub auth_endpoint: String,
    /// Responses API base URL (default: "https://chatgpt.com/backend-api/codex").
    pub api_base_url: String,
    /// OAuth client ID (default: OpenAI's public Codex client).
    pub client_id: String,
    /// Path to session file (default: ~/.brassclaw/openai_codex_session.json).
    pub session_path: PathBuf,
    /// Seconds before expiry to proactively refresh (default: 300).
    pub token_refresh_margin_secs: u64,
}

impl Default for OpenAiCodexConfig {
    fn default() -> Self {
        Self {
            model: "gpt-5.5".to_string(),
            auth_endpoint: "https://auth.openai.com".to_string(),
            api_base_url: "https://chatgpt.com/backend-api/codex".to_string(),
            client_id: "app_EMoamEEZ73f0CkXaXp7hrann".to_string(),
            session_path: brassclaw_base_dir().join("openai_codex_session.json"),
            token_refresh_margin_secs: 300,
        }
    }
}

impl OpenAiCodexConfig {
    /// Build a Codex config from already-resolved overrides, falling back to
    /// crate defaults for any field the caller leaves as `None`. Callers
    /// (the binary) own env / settings precedence and SSRF validation; this
    /// helper centralises the default values inside the crate.
    pub fn build(
        model: Option<String>,
        auth_endpoint: Option<String>,
        api_base_url: Option<String>,
        client_id: Option<String>,
        session_path: Option<PathBuf>,
        token_refresh_margin_secs: Option<u64>,
    ) -> Self {
        let defaults = Self::default();
        Self {
            model: model.unwrap_or(defaults.model),
            auth_endpoint: auth_endpoint.unwrap_or(defaults.auth_endpoint),
            api_base_url: api_base_url.unwrap_or(defaults.api_base_url),
            client_id: client_id.unwrap_or(defaults.client_id),
            session_path: session_path.unwrap_or(defaults.session_path),
            token_refresh_margin_secs: token_refresh_margin_secs
                .unwrap_or(defaults.token_refresh_margin_secs),
        }
    }
}

/// Configuration for AWS Bedrock (native Converse API).
#[derive(Debug, Clone)]
pub struct BedrockConfig {
    /// AWS region (e.g. "us-east-1").
    pub region: String,
    /// Bedrock model ID (e.g. "anthropic.claude-opus-4-6-v1").
    pub model: String,
    /// Cross-region inference prefix: "us", "eu", "apac", "global", or None.
    pub cross_region: Option<String>,
    /// AWS named profile (for SSO / assume-role workflows).
    pub profile: Option<String>,
}

impl BedrockConfig {
    /// Default region used when none is configured.
    pub const DEFAULT_REGION: &'static str = "us-east-1";

    /// Valid cross-region inference prefixes accepted by Bedrock.
    pub const VALID_CROSS_REGION_PREFIXES: &'static [&'static str] =
        &["us", "eu", "apac", "global"];

    /// Build a Bedrock config from already-resolved overrides.
    ///
    /// - `region` falls back to [`Self::DEFAULT_REGION`] when `None`.
    /// - `model` is required (returns [`LlmConfigError::MissingRequired`] when `None`).
    /// - `cross_region`, when set, is validated against
    ///   [`Self::VALID_CROSS_REGION_PREFIXES`].
    pub fn build(
        region: Option<String>,
        model: Option<String>,
        cross_region: Option<String>,
        profile: Option<String>,
    ) -> Result<Self, LlmConfigError> {
        let region = region.unwrap_or_else(|| Self::DEFAULT_REGION.to_string());
        let model = model.ok_or_else(|| LlmConfigError::MissingRequired {
            key: "BEDROCK_MODEL".to_string(),
            hint: "Set BEDROCK_MODEL or selected_model when LLM_BACKEND=bedrock".to_string(),
        })?;
        if let Some(ref cr) = cross_region
            && !Self::VALID_CROSS_REGION_PREFIXES.contains(&cr.as_str())
        {
            return Err(LlmConfigError::InvalidValue {
                key: "BEDROCK_CROSS_REGION".to_string(),
                message: format!(
                    "'{}' is not valid, expected one of: {}",
                    cr,
                    Self::VALID_CROSS_REGION_PREFIXES.join(", ")
                ),
            });
        }
        Ok(Self {
            region,
            model,
            cross_region,
            profile,
        })
    }
}

/// LLM provider configuration.
///
/// NearAI remains the default backend with its own config struct (session auth).
/// All other providers are resolved through the provider registry, producing
/// a generic `RegistryProviderConfig`.
#[derive(Debug, Clone)]
pub struct LlmConfig {
    /// Canonical selected provider identifier; it does not select its protocol.
    pub backend: String,
    /// Protocol selected from the exact resolved definition. The provider ID
    /// is metadata, not an adapter selector. None is legacy manual input.
    pub resolved_protocol: Option<ProviderProtocol>,
    /// Session manager configuration (auth URL, token persistence path).
    /// Used by the NearAI provider for OAuth/session-token auth.
    pub session: SessionConfig,
    /// NEAR AI config (always populated, also used for embeddings).
    pub nearai: NearAiConfig,
    /// Resolved provider config for registry-based providers.
    /// None for dedicated protocols.
    pub provider: Option<RegistryProviderConfig>,
    /// AWS Bedrock config (populated for its protocol, requires --features bedrock).
    pub bedrock: Option<BedrockConfig>,
    /// Gemini OAuth config (populated for its protocol).
    pub gemini_oauth: Option<GeminiOauthConfig>,
    /// OpenAI Codex config (populated for its protocol).
    pub openai_codex: Option<OpenAiCodexConfig>,
    /// Selected model retry delays, independent of VM task duration.
    pub retry_timing: crate::retry::RetryTimingPolicy,
    /// Optional decoded HTTP response capacity; None leaves it unrestricted.
    pub max_response_bytes: Option<u64>,
    /// HTTP request timeout in seconds for LLM API calls.
    /// Default: 120. Increase for local LLMs that need more evaluation time.
    pub request_timeout_secs: u64,
    /// Generic cheap/fast model for lightweight tasks (heartbeat, routing, evaluation).
    /// Works with any backend. Set via `LLM_CHEAP_MODEL` env var.
    /// When set, takes priority over the NearAI-specific `NEARAI_CHEAP_MODEL`.
    pub cheap_model: Option<String>,
    /// Enable cascade mode for smart routing (retry with primary if cheap model
    /// response seems uncertain). Default: true. Set via `SMART_ROUTING_CASCADE`.
    pub smart_routing_cascade: bool,
    /// Maximum number of retries for transient LLM errors.
    /// Set via `LLM_MAX_RETRIES` (falls back to `NEARAI_MAX_RETRIES`). Default: 3.
    pub max_retries: u32,
    /// Consecutive failures before circuit breaker opens. None = disabled.
    /// Set via `LLM_CIRCUIT_BREAKER_THRESHOLD` (falls back to `CIRCUIT_BREAKER_THRESHOLD`).
    pub circuit_breaker_threshold: Option<u32>,
    /// Seconds the circuit stays open before probing. Default: 30.
    /// Set via `LLM_CIRCUIT_BREAKER_RECOVERY_SECS` (falls back to `CIRCUIT_BREAKER_RECOVERY_SECS`).
    pub circuit_breaker_recovery_secs: u64,
    /// Enable in-memory response caching. Default: false.
    /// Set via `LLM_RESPONSE_CACHE_ENABLED` (falls back to `RESPONSE_CACHE_ENABLED`).
    pub response_cache_enabled: bool,
    /// TTL in seconds for cached responses. Default: 3600.
    /// Set via `LLM_RESPONSE_CACHE_TTL_SECS` (falls back to `RESPONSE_CACHE_TTL_SECS`).
    pub response_cache_ttl_secs: u64,
    /// Max cached responses before LRU eviction. Default: 1000.
    /// Set via `LLM_RESPONSE_CACHE_MAX_ENTRIES` (falls back to `RESPONSE_CACHE_MAX_ENTRIES`).
    pub response_cache_max_entries: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmBackendKind {
    NearAi,
    Bedrock,
    GeminiOauth,
    OpenAiCodex,
    Registry(String),
}

impl LlmBackendKind {
    pub fn from_backend_id(backend: &str) -> Self {
        match backend {
            "nearai" | "near_ai" | "near" => Self::NearAi,
            "bedrock" | "aws_bedrock" | "aws" => Self::Bedrock,
            "gemini_oauth" | "gemini-oauth" => Self::GeminiOauth,
            "openai_codex" | "openai-codex" | "codex" => Self::OpenAiCodex,
            other => Self::Registry(other.to_string()),
        }
    }

    pub fn provider_id(&self, registry_provider: Option<&RegistryProviderConfig>) -> String {
        match self {
            Self::NearAi => "nearai".to_string(),
            Self::Bedrock => "bedrock".to_string(),
            Self::GeminiOauth => "gemini_oauth".to_string(),
            Self::OpenAiCodex => "openai_codex".to_string(),
            Self::Registry(backend) => registry_provider
                .map(|provider| provider.provider_id.clone())
                .unwrap_or_else(|| backend.clone()),
        }
    }
}

impl LlmConfig {
    pub fn backend_kind(&self) -> LlmBackendKind {
        match self.resolved_protocol {
            Some(ProviderProtocol::NearAi) => LlmBackendKind::NearAi,
            Some(ProviderProtocol::Bedrock) => LlmBackendKind::Bedrock,
            Some(ProviderProtocol::GeminiOauth) => LlmBackendKind::GeminiOauth,
            Some(ProviderProtocol::OpenAiCodex) => LlmBackendKind::OpenAiCodex,
            Some(_) => LlmBackendKind::Registry(self.backend.clone()),
            None => self
                .provider
                .as_ref()
                .map(|provider| LlmBackendKind::Registry(provider.provider_id.clone()))
                .unwrap_or_else(|| LlmBackendKind::from_backend_id(&self.backend)),
        }
    }

    pub fn active_provider_id(&self) -> String {
        if self.resolved_protocol.is_some() {
            self.backend.clone()
        } else {
            self.backend_kind().provider_id(self.provider.as_ref())
        }
    }

    /// Resolve the effective cheap model name.
    ///
    /// Resolution order:
    /// 1. `LLM_CHEAP_MODEL` (generic, works with any backend)
    /// 2. `NEARAI_CHEAP_MODEL` (NearAI-only, backward compatibility)
    pub fn cheap_model_name(&self) -> Option<&str> {
        self.cheap_model.as_deref().or_else(|| {
            if matches!(self.backend_kind(), LlmBackendKind::NearAi) {
                self.nearai.cheap_model.as_deref()
            } else {
                None
            }
        })
    }

    /// Resolve the model name to show in status/UI after a hot-reload.
    ///
    /// This is used by the gateway status handler to refresh
    /// `ActiveConfigSnapshot.llm_model` when the provider chain is swapped
    /// without touching an active provider instance (e.g. before the first
    /// request lands on the new chain).
    pub fn active_model_name(&self) -> String {
        match self.backend_kind() {
            LlmBackendKind::NearAi => self.nearai.model.clone(),
            LlmBackendKind::Bedrock => self
                .bedrock
                .as_ref()
                .map(|cfg| cfg.model.clone())
                .unwrap_or_else(|| self.nearai.model.clone()),
            LlmBackendKind::GeminiOauth => self
                .gemini_oauth
                .as_ref()
                .map(|cfg| cfg.model.clone())
                .unwrap_or_else(|| self.nearai.model.clone()),
            LlmBackendKind::OpenAiCodex => self
                .openai_codex
                .as_ref()
                .map(|cfg| cfg.model.clone())
                .unwrap_or_else(|| "gpt-5.5".to_string()),
            _ => self
                .provider
                .as_ref()
                .map(|cfg| cfg.model.clone())
                .unwrap_or_else(|| self.nearai.model.clone()),
        }
    }
}

/// NEAR AI configuration.
#[derive(Debug, Clone)]
pub struct NearAiConfig {
    /// Model to use (e.g., "claude-3-5-sonnet-20241022", "gpt-4o")
    pub model: String,
    /// Cheap/fast model for lightweight tasks (heartbeat, routing, evaluation).
    pub cheap_model: Option<String>,
    /// Base URL for the NEAR AI API.
    pub base_url: String,
    /// API key for NEAR AI Cloud.
    pub api_key: Option<SecretString>,
    /// Optional fallback model for failover.
    pub fallback_model: Option<String>,
    /// Maximum number of retries for transient errors (default: 3).
    pub max_retries: u32,
    /// Consecutive failures before circuit breaker opens. None = disabled.
    pub circuit_breaker_threshold: Option<u32>,
    /// Seconds the circuit stays open before probing (default: 30).
    pub circuit_breaker_recovery_secs: u64,
    /// Enable in-memory response caching. Default: false.
    pub response_cache_enabled: bool,
    /// TTL in seconds for cached responses (default: 3600).
    pub response_cache_ttl_secs: u64,
    /// Max cached responses before LRU eviction (default: 1000).
    pub response_cache_max_entries: usize,
    /// Cooldown duration in seconds for failover (default: 300).
    pub failover_cooldown_secs: u64,
    /// Consecutive failures before failover cooldown (default: 3).
    pub failover_cooldown_threshold: u32,
    /// Enable cascade mode for smart routing. Default: true.
    pub smart_routing_cascade: bool,
}

/// Configuration for the selected Gemini OAuth integration. Request options
/// are explicit; this adapter performs no process-environment overrides.
#[derive(Debug, Clone)]
pub struct GeminiOauthConfig {
    pub model: String,
    pub credentials_path: PathBuf,
}

impl GeminiOauthConfig {
    /// Default model used when none is configured.
    pub const DEFAULT_MODEL: &'static str = "gemini-2.5-flash";

    pub fn default_credentials_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".gemini")
            .join("oauth_creds.json")
    }

    /// Build a Gemini OAuth config from already-resolved overrides.
    ///
    /// Falls back to [`Self::DEFAULT_MODEL`] and
    /// [`Self::default_credentials_path`] when their respective overrides
    /// are absent.
    pub fn build(model: Option<String>, credentials_path: Option<PathBuf>) -> Self {
        Self {
            model: model.unwrap_or_else(|| Self::DEFAULT_MODEL.to_string()),
            credentials_path: credentials_path.unwrap_or_else(Self::default_credentials_path),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bedrock_build_applies_default_region() {
        let cfg = BedrockConfig::build(None, Some("model-x".to_string()), None, None)
            .expect("model is set");
        assert_eq!(cfg.region, BedrockConfig::DEFAULT_REGION);
        assert_eq!(cfg.model, "model-x");
        assert!(cfg.cross_region.is_none());
        assert!(cfg.profile.is_none());
    }

    #[test]
    fn bedrock_build_requires_model() {
        let err = BedrockConfig::build(Some("us-west-2".into()), None, None, None)
            .expect_err("model is required");
        assert!(matches!(
            err,
            LlmConfigError::MissingRequired { ref key, .. } if key == "BEDROCK_MODEL"
        ));
    }

    #[test]
    fn bedrock_build_validates_cross_region() {
        for ok in BedrockConfig::VALID_CROSS_REGION_PREFIXES {
            let cfg =
                BedrockConfig::build(None, Some("model".into()), Some((*ok).to_string()), None)
                    .expect("valid prefix");
            assert_eq!(cfg.cross_region.as_deref(), Some(*ok));
        }

        let err = BedrockConfig::build(None, Some("model".into()), Some("ap".to_string()), None)
            .expect_err("'ap' is not a valid prefix");
        assert!(matches!(
            err,
            LlmConfigError::InvalidValue { ref key, .. } if key == "BEDROCK_CROSS_REGION"
        ));
    }

    #[test]
    fn gemini_oauth_build_applies_defaults() {
        let cfg = GeminiOauthConfig::build(None, None);
        assert_eq!(cfg.model, GeminiOauthConfig::DEFAULT_MODEL);
        assert_eq!(
            cfg.credentials_path,
            GeminiOauthConfig::default_credentials_path()
        );

        let cfg = GeminiOauthConfig::build(
            Some("gemini-foo".into()),
            Some(PathBuf::from("/tmp/creds.json")),
        );
        assert_eq!(cfg.model, "gemini-foo");
        assert_eq!(cfg.credentials_path, PathBuf::from("/tmp/creds.json"));
    }

    #[test]
    fn openai_codex_build_applies_defaults() {
        let cfg = OpenAiCodexConfig::build(None, None, None, None, None, None);
        let defaults = OpenAiCodexConfig::default();
        assert_eq!(cfg.model, defaults.model);
        assert_eq!(cfg.auth_endpoint, defaults.auth_endpoint);
        assert_eq!(cfg.api_base_url, defaults.api_base_url);
        assert_eq!(cfg.client_id, defaults.client_id);
        assert_eq!(cfg.session_path, defaults.session_path);
        assert_eq!(
            cfg.token_refresh_margin_secs,
            defaults.token_refresh_margin_secs
        );
    }

    #[test]
    fn openai_codex_build_overrides_take_precedence() {
        let cfg = OpenAiCodexConfig::build(
            Some("gpt-overridden".into()),
            Some("https://auth.example".into()),
            Some("https://api.example".into()),
            Some("client-z".into()),
            Some(PathBuf::from("/tmp/sess.json")),
            Some(60),
        );
        assert_eq!(cfg.model, "gpt-overridden");
        assert_eq!(cfg.auth_endpoint, "https://auth.example");
        assert_eq!(cfg.api_base_url, "https://api.example");
        assert_eq!(cfg.client_id, "client-z");
        assert_eq!(cfg.session_path, PathBuf::from("/tmp/sess.json"));
        assert_eq!(cfg.token_refresh_margin_secs, 60);
    }
}
