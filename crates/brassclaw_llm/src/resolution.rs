//! Explicit LLM configuration from a captured database catalogue selection.
//! Provider definitions, endpoints, models and technical policies are supplied
//! by composition. Credentials are resolved separately by its secret broker.
//! This module reads no provider files or process environment.

use crate::{
    auth,
    config::{
        BedrockConfig, CacheRetention, GeminiOauthConfig, LlmConfig, NearAiConfig,
        OpenAiCodexConfig, RegistryProviderConfig,
    },
    error::LlmError,
    registry::{ProviderDefinition, ProviderProtocol, ProviderRegistry},
    session::SessionConfig,
};
use secrecy::{ExposeSecret, SecretString};

/// Already-resolved input from one captured database catalogue selection.
#[derive(Debug, Clone)]
pub enum ResolvedProviderConfig {
    Registry(RegistryProviderConfig),
    Dedicated(ResolvedDedicatedProviderConfig),
}

/// Resolved input for providers represented by dedicated `LlmConfig` slots.
#[derive(Debug, Clone)]
pub struct ResolvedDedicatedProviderConfig {
    pub protocol: ProviderProtocol,
    pub provider_id: String,
    pub api_key: Option<SecretString>,
    pub base_url: String,
    pub model: String,
}

impl ResolvedProviderConfig {
    pub fn protocol(&self) -> ProviderProtocol {
        match self {
            Self::Registry(config) => config.protocol,
            Self::Dedicated(config) => config.protocol,
        }
    }

    pub fn provider_id(&self) -> &str {
        match self {
            Self::Registry(config) => &config.provider_id,
            Self::Dedicated(config) => &config.provider_id,
        }
    }

    pub fn base_url(&self) -> &str {
        match self {
            Self::Registry(config) => &config.base_url,
            Self::Dedicated(config) => &config.base_url,
        }
    }

    pub fn model(&self) -> &str {
        match self {
            Self::Registry(config) => &config.model,
            Self::Dedicated(config) => &config.model,
        }
    }
}

/// Selection captured by composition from its database role and definition.
#[derive(Debug, Clone)]
pub struct ProviderSelection {
    pub provider_id: String,
    /// Retained compatibility metadata; never consulted for a credential.
    pub api_key_env: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderResolutionError {
    #[error("provider `{provider}` is not in the provider registry")]
    UnknownProvider { provider: String },
    #[error("provider `{provider}` requires an API key")]
    MissingApiKey { provider: String },
    #[error("provider `{provider}` requires a base URL")]
    MissingBaseUrl { provider: String },
    #[error(transparent)]
    Llm(#[from] LlmError),
}

impl ProviderResolutionError {
    pub fn into_llm_error(self) -> LlmError {
        match self {
            Self::UnknownProvider { provider } | Self::MissingApiKey { provider } => {
                LlmError::AuthFailed { provider }
            }
            Self::MissingBaseUrl { provider } => LlmError::RequestFailed {
                provider,
                reason: "base URL is required but the catalogue selection has no endpoint"
                    .to_string(),
            },
            Self::Llm(source) => source,
        }
    }
}

/// Resolve a catalogue selection using its explicitly supplied credential.
pub fn resolve_provider_config_with_credentials(
    selection: ProviderSelection,
    registry: &ProviderRegistry,
    api_key: Option<SecretString>,
) -> Result<ResolvedProviderConfig, ProviderResolutionError> {
    let provider = registry.find(&selection.provider_id).ok_or_else(|| {
        ProviderResolutionError::UnknownProvider {
            provider: selection.provider_id.clone(),
        }
    })?;
    resolve_provider_definition(provider, selection.base_url, selection.model, api_key)
}

/// Build the selected catalogue chain from explicit technical policies.
/// Generic model-routing/retry/cache environment overrides never apply here.
pub fn build_llm_config_with_provider_policy(
    resolved: ResolvedProviderConfig,
    request: crate::ProviderRequestPolicy,
    cache: crate::ResponseCachePolicy,
) -> Result<LlmConfig, LlmError> {
    request
        .validate_for_protocol(resolved.protocol())
        .map_err(|reason| LlmError::RequestFailed {
            provider: "request_policy".into(),
            reason: reason.into(),
        })?;
    cache.validate().map_err(|reason| LlmError::RequestFailed {
        provider: "response_cache".into(),
        reason: reason.into(),
    })?;
    let max_entries = usize::try_from(cache.max_entries).map_err(|_| LlmError::RequestFailed {
        provider: "response_cache".into(),
        reason: "cache capacity cannot be represented on this host".into(),
    })?;
    build_llm_config_with_chain(
        resolved,
        ChainSettings {
            request_timeout_secs: request.timeout_secs,
            retry_timing: request.retry_timing,
            max_response_bytes: request.max_response_bytes,
            max_retries: request.max_retries,
            circuit_breaker_threshold: request.circuit_breaker_threshold,
            circuit_breaker_recovery_secs: request.circuit_breaker_recovery_secs,
            response_cache_enabled: cache.enabled,
            response_cache_ttl_secs: cache.ttl_secs,
            response_cache_max_entries: max_entries,
            cheap_model: None,
            smart_routing_cascade: false,
        },
    )
}

fn build_llm_config_with_chain(
    resolved: ResolvedProviderConfig,
    chain: ChainSettings,
) -> Result<LlmConfig, LlmError> {
    let backend = resolved.provider_id().to_string();
    let resolved_protocol = Some(resolved.protocol());
    // Unselected dedicated slots carry no alternate model or credential.
    let mut nearai = build_nearai_config(String::new(), String::new(), None, &chain);
    let mut provider = None;
    let mut bedrock = None;
    let mut gemini_oauth = None;
    let mut openai_codex = None;
    match resolved {
        ResolvedProviderConfig::Registry(config) => provider = Some(config),
        ResolvedProviderConfig::Dedicated(config) => match config.protocol {
            ProviderProtocol::NearAi => {
                if config.base_url.is_empty() {
                    return Err(LlmError::RequestFailed {
                        provider: config.provider_id,
                        reason:
                            "configure the selected NEAR API endpoint in the provider definition"
                                .into(),
                    });
                }
                nearai = build_nearai_config(config.model, config.base_url, config.api_key, &chain);
            }
            ProviderProtocol::Bedrock => {
                bedrock = Some(
                    BedrockConfig::build(None, Some(config.model), None, None).map_err(
                        |error| LlmError::RequestFailed {
                            provider: config.provider_id,
                            reason: error.to_string(),
                        },
                    )?,
                );
            }
            ProviderProtocol::GeminiOauth => {
                gemini_oauth = Some(GeminiOauthConfig::build(Some(config.model), None));
            }
            ProviderProtocol::OpenAiCodex => {
                openai_codex = Some(OpenAiCodexConfig::build(
                    Some(config.model),
                    None,
                    (!config.base_url.is_empty()).then_some(config.base_url),
                    None,
                    None,
                    None,
                ));
            }
            _ => {
                return Err(LlmError::RequestFailed {
                    provider: config.provider_id,
                    reason: "registry provider protocol resolved as dedicated config".into(),
                });
            }
        },
    }
    Ok(LlmConfig {
        backend,
        resolved_protocol,
        session: SessionConfig::default(),
        nearai,
        provider,
        bedrock,
        gemini_oauth,
        openai_codex,
        request_timeout_secs: chain.request_timeout_secs,
        retry_timing: chain.retry_timing,
        max_response_bytes: chain.max_response_bytes,
        cheap_model: chain.cheap_model,
        smart_routing_cascade: chain.smart_routing_cascade,
        max_retries: chain.max_retries,
        circuit_breaker_threshold: chain.circuit_breaker_threshold,
        circuit_breaker_recovery_secs: chain.circuit_breaker_recovery_secs,
        response_cache_enabled: chain.response_cache_enabled,
        response_cache_ttl_secs: chain.response_cache_ttl_secs,
        response_cache_max_entries: chain.response_cache_max_entries,
    })
}

fn resolve_provider_definition(
    provider: &ProviderDefinition,
    base_url_override: Option<String>,
    model_override: Option<String>,
    stored_api_key: Option<SecretString>,
) -> Result<ResolvedProviderConfig, ProviderResolutionError> {
    let api_key = match stored_api_key {
        Some(value) if !value.expose_secret().trim().is_empty() => Some(value),
        Some(_) => {
            return Err(ProviderResolutionError::MissingApiKey {
                provider: provider.id.clone(),
            });
        }
        None if provider.api_key_required => {
            return Err(ProviderResolutionError::MissingApiKey {
                provider: provider.id.clone(),
            });
        }
        None => None,
    };
    let base_url = base_url_override
        .or_else(|| provider.default_base_url.clone())
        .unwrap_or_default();
    if provider.base_url_required && base_url.is_empty() {
        return Err(ProviderResolutionError::MissingBaseUrl {
            provider: provider.id.clone(),
        });
    }
    let model = model_override.unwrap_or_else(|| provider.default_model.clone());
    if provider.protocol.has_dedicated_config() {
        return Ok(ResolvedProviderConfig::Dedicated(
            ResolvedDedicatedProviderConfig {
                protocol: provider.protocol,
                provider_id: provider.id.clone(),
                api_key,
                base_url,
                model,
            },
        ));
    }
    let extra_headers = if provider.protocol == ProviderProtocol::GithubCopilot {
        auth::default_headers(auth::AuthBackend::GithubCopilot)
    } else {
        Vec::new()
    };
    let mut config = RegistryProviderConfig::generic(
        provider.protocol,
        provider.id.clone(),
        api_key,
        base_url,
        model,
    )
    .with_extra_headers(extra_headers)
    .with_unsupported_params(provider.unsupported_params.clone());
    config.cache_retention = provider
        .cache_retention
        .as_deref()
        .map(str::parse::<CacheRetention>)
        .transpose()
        .map_err(|_| {
            ProviderResolutionError::Llm(LlmError::RequestFailed {
                provider: provider.id.clone(),
                reason: "stored cache retention is invalid".into(),
            })
        })?
        .unwrap_or_default();
    Ok(ResolvedProviderConfig::Registry(config))
}

fn build_nearai_config(
    model: String,
    base_url: String,
    api_key: Option<SecretString>,
    chain: &ChainSettings,
) -> NearAiConfig {
    NearAiConfig {
        model,
        base_url,
        api_key,
        cheap_model: None,
        fallback_model: None,
        max_retries: chain.max_retries,
        circuit_breaker_threshold: chain.circuit_breaker_threshold,
        circuit_breaker_recovery_secs: chain.circuit_breaker_recovery_secs,
        response_cache_enabled: chain.response_cache_enabled,
        response_cache_ttl_secs: chain.response_cache_ttl_secs,
        response_cache_max_entries: chain.response_cache_max_entries,
        failover_cooldown_secs: 300,
        failover_cooldown_threshold: 3,
        smart_routing_cascade: chain.smart_routing_cascade,
    }
}

#[derive(Debug, Clone)]
struct ChainSettings {
    retry_timing: crate::RetryTimingPolicy,
    max_response_bytes: Option<u64>,
    request_timeout_secs: u64,
    cheap_model: Option<String>,
    smart_routing_cascade: bool,
    max_retries: u32,
    circuit_breaker_threshold: Option<u32>,
    circuit_breaker_recovery_secs: u64,
    response_cache_enabled: bool,
    response_cache_ttl_secs: u64,
    response_cache_max_entries: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHAIN_ENV_VARS: &[&str] = &[
        "LLM_REQUEST_TIMEOUT_SECS",
        "LLM_CHEAP_MODEL",
        "LLM_BACKEND",
        "SMART_ROUTING_CASCADE",
        "CODEX_AUTH_PATH",
        "LLM_USE_CODEX_AUTH",
        "LLM_MAX_RETRIES",
        "NEARAI_MAX_RETRIES",
        "LLM_CIRCUIT_BREAKER_THRESHOLD",
        "CIRCUIT_BREAKER_THRESHOLD",
        "LLM_CIRCUIT_BREAKER_RECOVERY_SECS",
        "CIRCUIT_BREAKER_RECOVERY_SECS",
        "LLM_RESPONSE_CACHE_ENABLED",
        "RESPONSE_CACHE_ENABLED",
        "LLM_RESPONSE_CACHE_TTL_SECS",
        "RESPONSE_CACHE_TTL_SECS",
        "LLM_RESPONSE_CACHE_MAX_ENTRIES",
        "RESPONSE_CACHE_MAX_ENTRIES",
        "NEARAI_API_KEY",
        "NEARAI_BASE_URL",
        "NEARAI_MODEL",
        "NEARAI_CHEAP_MODEL",
        "NEARAI_FALLBACK_MODEL",
    ];

    struct EnvGuard {
        saved: Vec<(&'static str, Option<String>)>,
    }

    impl EnvGuard {
        fn clear(names: &[&'static str]) -> Self {
            let saved = names
                .iter()
                .map(|name| (*name, std::env::var(name).ok()))
                .collect();
            for name in names {
                unsafe {
                    std::env::remove_var(name);
                }
            }
            Self { saved }
        }

        fn set(&self, name: &str, value: &str) {
            unsafe {
                std::env::set_var(name, value);
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (name, value) in self.saved.drain(..) {
                unsafe {
                    match value {
                        Some(value) => std::env::set_var(name, value),
                        None => std::env::remove_var(name),
                    }
                }
            }
        }
    }

    fn registry_resolved_provider() -> ResolvedProviderConfig {
        ResolvedProviderConfig::Registry(RegistryProviderConfig::generic(
            ProviderProtocol::OpenAiCompletions,
            "openai",
            None,
            "https://api.openai.com/v1",
            "gpt-test",
        ))
    }

    #[test]
    fn explicit_policy_excludes_retired_chain_environment() {
        let _env_lock = brassclaw_common::env_helpers::lock_env();
        let env = EnvGuard::clear(CHAIN_ENV_VARS);
        env.set("NEARAI_MAX_RETRIES", "7");
        env.set("CIRCUIT_BREAKER_THRESHOLD", "11");
        env.set("CIRCUIT_BREAKER_RECOVERY_SECS", "19");
        env.set("RESPONSE_CACHE_ENABLED", "true");
        env.set("RESPONSE_CACHE_TTL_SECS", "23");
        env.set("RESPONSE_CACHE_MAX_ENTRIES", "29");
        let policy = crate::ProviderRequestPolicy::default();
        let cache = crate::ResponseCachePolicy::default();
        let config =
            build_llm_config_with_provider_policy(registry_resolved_provider(), policy, cache)
                .unwrap();
        assert_eq!(config.max_retries, policy.max_retries);
        assert_eq!(
            config.circuit_breaker_threshold,
            policy.circuit_breaker_threshold
        );
        assert_eq!(
            config.circuit_breaker_recovery_secs,
            policy.circuit_breaker_recovery_secs
        );
        assert_eq!(config.response_cache_enabled, cache.enabled);
        assert_eq!(config.response_cache_ttl_secs, cache.ttl_secs);
        assert_eq!(config.response_cache_max_entries as u64, cache.max_entries);
    }

    #[test]
    fn explicit_policy_excludes_retired_boolean_environment() {
        let _env_lock = brassclaw_common::env_helpers::lock_env();
        let env = EnvGuard::clear(CHAIN_ENV_VARS);
        env.set("SMART_ROUTING_CASCADE", "invalid-unselected-value");
        env.set("LLM_RESPONSE_CACHE_ENABLED", "invalid-unselected-value");
        let config = build_llm_config_with_provider_policy(
            registry_resolved_provider(),
            Default::default(),
            Default::default(),
        )
        .unwrap();
        assert!(!config.smart_routing_cascade);
        assert!(!config.response_cache_enabled);
    }

    #[test]
    fn selected_codex_does_not_resolve_ambient_cli_auth_file() {
        let _env_lock = brassclaw_common::env_helpers::lock_env();
        let env = EnvGuard::clear(CHAIN_ENV_VARS);
        env.set("LLM_BACKEND", "unselected-provider");
        env.set("CODEX_AUTH_PATH", "/tmp/brassclaw-missing-codex-auth.json");
        let registry = ProviderRegistry::new(
            crate::testing::provider_seed::installation_provider_definitions(),
        );
        let selected = resolve_provider_config_with_credentials(
            ProviderSelection {
                provider_id: "openai_codex".into(),
                api_key_env: None,
                base_url: Some("https://selected.example/codex".into()),
                model: Some("selected-model".into()),
            },
            &registry,
            None,
        )
        .unwrap();
        let config =
            build_llm_config_with_provider_policy(selected, Default::default(), Default::default())
                .unwrap();
        assert_eq!(config.active_provider_id(), "openai_codex");
        let codex = config.openai_codex.unwrap();
        assert_eq!(codex.model, "selected-model");
        assert_eq!(codex.api_base_url, "https://selected.example/codex");
    }

    #[test]
    fn explicit_catalogue_cache_policy_is_not_overridden_by_environment() {
        let _lock = brassclaw_common::env_helpers::lock_env();
        let env = EnvGuard::clear(&[
            "ANTHROPIC_CACHE_RETENTION",
            "ANTHROPIC_OAUTH_TOKEN",
            "ANTHROPIC_API_KEY",
        ]);
        env.set(
            "ANTHROPIC_CACHE_RETENTION",
            "invalid-environment-cache-policy",
        );
        env.set("ANTHROPIC_API_KEY", "legacy-test-credential");
        let mut provider = ProviderRegistry::new(
            crate::testing::provider_seed::installation_provider_definitions(),
        )
        .find("anthropic")
        .unwrap()
        .clone();
        provider.cache_retention = Some("long".into());
        let registry = ProviderRegistry::new(vec![provider.clone()]);
        let selection = || ProviderSelection {
            provider_id: "anthropic".into(),
            api_key_env: None,
            base_url: None,
            model: Some("selected-anthropic-model".into()),
        };
        let ResolvedProviderConfig::Registry(config) = resolve_provider_config_with_credentials(
            selection(),
            &registry,
            Some(SecretString::from("stored-credential")),
        )
        .unwrap() else {
            panic!("registry protocol expected");
        };
        assert_eq!(config.cache_retention, CacheRetention::Long);
        assert!(
            matches!(
                resolve_provider_config_with_credentials(selection(), &registry, None),
                Err(ProviderResolutionError::MissingApiKey { .. })
            ),
            "ambient credentials cannot satisfy the stored credential contract"
        );
        provider.cache_retention = Some("invalid-stored-cache-policy".into());
        assert!(
            resolve_provider_config_with_credentials(
                selection(),
                &ProviderRegistry::new(vec![provider]),
                Some(SecretString::from("stored-credential"))
            )
            .is_err()
        );
    }

    #[test]
    fn explicit_request_policy_excludes_generic_environment_routing_and_limits() {
        let _lock = brassclaw_common::env_helpers::lock_env();
        let env = EnvGuard::clear(CHAIN_ENV_VARS);
        env.set("LLM_REQUEST_TIMEOUT_SECS", "invalid-environment-timeout");
        env.set("LLM_MAX_RETRIES", "invalid-environment-retries");
        env.set(
            "LLM_CIRCUIT_BREAKER_THRESHOLD",
            "invalid-environment-threshold",
        );
        env.set("LLM_CHEAP_MODEL", "unselected-cheap-model");
        env.set("NEARAI_FALLBACK_MODEL", "unselected-fallback-model");
        env.set("NEARAI_API_KEY", "unselected-near-credential");
        let registry = ProviderRegistry::new(
            crate::testing::provider_seed::installation_provider_definitions(),
        );
        let resolved = || {
            resolve_provider_config_with_credentials(
                ProviderSelection {
                    provider_id: "openai".into(),
                    api_key_env: None,
                    base_url: Some("http://127.0.0.1:8000/v1".into()),
                    model: Some("selected-request-model".into()),
                },
                &registry,
                Some(SecretString::from("selected-request-credential")),
            )
            .unwrap()
        };
        let policy = crate::ProviderRequestPolicy {
            timeout_secs: 3,
            max_retries: 2,
            circuit_breaker_threshold: Some(4),
            circuit_breaker_recovery_secs: 5,
            retry_timing: Default::default(),
            max_response_bytes: None,
        };
        let config =
            build_llm_config_with_provider_policy(resolved(), policy, Default::default()).unwrap();
        assert_eq!(config.request_timeout_secs, 3);
        assert_eq!(config.max_retries, 2);
        assert_eq!(config.circuit_breaker_threshold, Some(4));
        assert_eq!(config.circuit_breaker_recovery_secs, 5);
        assert!(
            config.cheap_model.is_none()
                && config.nearai.cheap_model.is_none()
                && config.nearai.fallback_model.is_none()
                && config.nearai.api_key.is_none()
        );
        assert!(
            build_llm_config_with_provider_policy(
                resolved(),
                crate::ProviderRequestPolicy {
                    timeout_secs: 0,
                    ..policy
                },
                Default::default()
            )
            .is_err()
        );
        assert!(
            build_llm_config_with_provider_policy(
                resolved(),
                crate::ProviderRequestPolicy {
                    circuit_breaker_threshold: Some(0),
                    ..policy
                },
                Default::default()
            )
            .is_err()
        );
    }

    #[test]
    fn catalogue_protocol_preserves_renamed_dedicated_provider_identity() {
        let _lock = brassclaw_common::env_helpers::lock_env();
        let _env = EnvGuard::clear(CHAIN_ENV_VARS);
        let dedicated_env = EnvGuard::clear(&[
            "BEDROCK_REGION",
            "BEDROCK_CROSS_REGION",
            "AWS_PROFILE",
            "GEMINI_CREDENTIALS_PATH",
            "OPENAI_CODEX_AUTH_URL",
            "OPENAI_CODEX_API_URL",
            "OPENAI_CODEX_CLIENT_ID",
            "OPENAI_CODEX_SESSION_PATH",
            "OPENAI_CODEX_REFRESH_MARGIN_SECS",
            "NEARAI_AUTH_URL",
            "NEARAI_SESSION_PATH",
            "LLM_FAILOVER_COOLDOWN_SECS",
            "LLM_FAILOVER_THRESHOLD",
        ]);
        for name in [
            "BEDROCK_REGION",
            "BEDROCK_CROSS_REGION",
            "AWS_PROFILE",
            "GEMINI_CREDENTIALS_PATH",
            "OPENAI_CODEX_AUTH_URL",
            "OPENAI_CODEX_API_URL",
            "OPENAI_CODEX_CLIENT_ID",
            "OPENAI_CODEX_SESSION_PATH",
            "OPENAI_CODEX_REFRESH_MARGIN_SECS",
        ] {
            dedicated_env.set(name, "unselected-invalid-environment-value");
        }
        let registry = ProviderRegistry::new(
            crate::testing::provider_seed::installation_provider_definitions(),
        );
        for (protocol, expected) in [
            (ProviderProtocol::NearAi, crate::LlmBackendKind::NearAi),
            (ProviderProtocol::Bedrock, crate::LlmBackendKind::Bedrock),
            (
                ProviderProtocol::GeminiOauth,
                crate::LlmBackendKind::GeminiOauth,
            ),
            (
                ProviderProtocol::OpenAiCodex,
                crate::LlmBackendKind::OpenAiCodex,
            ),
        ] {
            let mut definition = registry.find("nearai").unwrap().clone();
            definition.id = "renamed-provider".into();
            definition.aliases.clear();
            definition.protocol = protocol;
            definition.default_base_url = Some("http://127.0.0.1:8000/v1".into());
            definition.default_model = "exact-selected-model".into();
            definition.api_key_required = false;
            definition.api_key_env = None;
            let resolved = resolve_provider_config_with_credentials(
                ProviderSelection {
                    provider_id: definition.id.clone(),
                    model: None,
                    api_key_env: None,
                    base_url: None,
                },
                &ProviderRegistry::new(vec![definition]),
                None,
            )
            .unwrap();
            let config = build_llm_config_with_provider_policy(
                resolved,
                Default::default(),
                Default::default(),
            )
            .unwrap();
            assert_eq!(config.backend_kind(), expected);
            assert_eq!(config.active_provider_id(), "renamed-provider");
            assert_eq!(config.active_model_name(), "exact-selected-model");
            if let Some(bedrock) = &config.bedrock {
                assert_eq!(bedrock.region, BedrockConfig::DEFAULT_REGION);
                assert!(bedrock.profile.is_none());
                assert!(bedrock.cross_region.is_none());
            }
            if let Some(gemini) = &config.gemini_oauth {
                assert_eq!(
                    gemini.credentials_path,
                    GeminiOauthConfig::default_credentials_path()
                );
            }
            if let Some(codex) = &config.openai_codex {
                assert_eq!(codex.api_base_url, "http://127.0.0.1:8000/v1");
                let defaults = OpenAiCodexConfig::default();
                assert_eq!(codex.auth_endpoint, defaults.auth_endpoint);
                assert_eq!(codex.client_id, defaults.client_id);
                assert_eq!(codex.session_path, defaults.session_path);
                assert_eq!(
                    codex.token_refresh_margin_secs,
                    defaults.token_refresh_margin_secs
                );
            }
        }
    }

    #[test]
    fn catalogue_credentials_satisfy_required_key_without_environment_overrides() {
        let _lock = brassclaw_common::env_helpers::lock_env();
        let env = EnvGuard::clear(&[
            "OPENAI_API_KEY",
            "OPENAI_MODEL",
            "OPENAI_BASE_URL",
            "LLM_EXTRA_HEADERS",
            "ANTHROPIC_OAUTH_TOKEN",
        ]);
        env.set("OPENAI_MODEL", "environment-model");
        env.set("OPENAI_BASE_URL", "https://unselected.example/v1");
        env.set("OPENAI_API_KEY", "unselected-environment-credential");
        env.set(
            "LLM_EXTRA_HEADERS",
            "Authorization: Bearer unselected-header-credential",
        );
        env.set("ANTHROPIC_OAUTH_TOKEN", "unselected-oauth-credential");
        let loaded = ProviderRegistry::new(
            crate::testing::provider_seed::installation_provider_definitions(),
        );
        let mut definition = loaded.find("openai").unwrap().clone();
        definition.extra_headers_env = Some("LLM_EXTRA_HEADERS".into());
        let registry = ProviderRegistry::new(vec![definition]);
        let selection = || ProviderSelection {
            provider_id: "openai".into(),
            api_key_env: None,
            base_url: Some("https://selected.example/v1".into()),
            model: Some("selected-model".into()),
        };
        let resolved = resolve_provider_config_with_credentials(
            selection(),
            &registry,
            Some(SecretString::from("stored-test-credential")),
        )
        .unwrap();
        assert_eq!(resolved.model(), "selected-model");
        assert_eq!(resolved.base_url(), "https://selected.example/v1");
        let ResolvedProviderConfig::Registry(config) = resolved else {
            panic!("registry protocol expected")
        };
        assert!(config.extra_headers.is_empty());
        assert_eq!(
            config.api_key.unwrap().expose_secret(),
            "stored-test-credential"
        );
        assert!(matches!(
            resolve_provider_config_with_credentials(selection(), &registry, None),
            Err(ProviderResolutionError::MissingApiKey { .. })
        ));
        let mut optional = registry.find("openai").unwrap().clone();
        optional.api_key_required = false;
        let ResolvedProviderConfig::Registry(optional) = resolve_provider_config_with_credentials(
            selection(),
            &ProviderRegistry::new(vec![optional]),
            None,
        )
        .unwrap() else {
            panic!("registry protocol expected")
        };
        assert!(
            optional.api_key.is_none(),
            "an optional stored key cannot inherit a process credential"
        );
        let mut anthropic = loaded.find("anthropic").unwrap().clone();
        anthropic.api_key_required = false;
        let mut anthropic_selection = selection();
        anthropic_selection.provider_id = anthropic.id.clone();
        let ResolvedProviderConfig::Registry(anthropic) = resolve_provider_config_with_credentials(
            anthropic_selection,
            &ProviderRegistry::new(vec![anthropic]),
            None,
        )
        .unwrap() else {
            panic!("registry protocol expected")
        };
        assert!(anthropic.oauth_token.is_none() && anthropic.api_key.is_none());
        assert!(matches!(
            resolve_provider_config_with_credentials(
                selection(),
                &registry,
                Some(SecretString::from(" "))
            ),
            Err(ProviderResolutionError::MissingApiKey { .. })
        ));
    }
}
