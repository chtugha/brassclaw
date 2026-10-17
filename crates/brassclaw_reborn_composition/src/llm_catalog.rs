//! Provider-catalog resolution for the assembled Reborn runtime.
//!
//! After V047/V048 the source of truth for all providers (builtins and custom)
//! is the `brassclaw_llm_providers` DB table, initialized once by V140 before
//! catalogue consumers start. The active slot selection is stored in
//! `brassclaw_config` via `db_config`.
//!
//! This module resolves a selection into a concrete `brassclaw_llm::LlmConfig`
//! ready to wire through the shared LLM provider chain. Lives behind the
//! `root-llm-provider` feature so the substrate-only composition stays free
//! of `brassclaw_llm`.

use thiserror::Error;

use brassclaw_llm::{
    ProviderRegistry, ProviderResolutionError, ProviderSelection, registry::ProviderDefinition,
};
use brassclaw_reborn_config::{LlmSlotSelection, reject_inline_secret};

/// Errors surfaced when resolving an `LlmSlotSelection` against the
/// coherent active database catalogue.
#[derive(Debug, Error)]
pub enum RebornLlmCatalogError {
    /// The persisted role selection did not name a provider.
    #[error(
        "llm slot selection has no provider_id; configure the provider in Settings → Inference"
    )]
    MissingProviderId,
    /// `provider_id` does not exist in the active catalogue.
    #[error(
        "llm provider id `{requested}` not found in the provider catalog; known ids: [{known}]"
    )]
    UnknownProvider { requested: String, known: String },
    /// A required credential is absent. The retained environment-reference
    /// field is compatibility metadata, not an alternate runtime key source.
    #[error(
        "llm provider `{provider}` requires a credential in the encrypted store; \
         configure it in Settings → Inference"
    )]
    ApiKeyEnvUnset { provider: String, env: String },
    /// Provider says it needs an API key but doesn't expose an
    /// `api_key_env` setting (and the selection didn't override it).
    /// Theoretically impossible in a sane catalog; defensive guard.
    #[error(
        "llm provider `{provider}` requires a credential in the encrypted store; \
         configure its authentication in Settings → Inference"
    )]
    ApiKeyEnvUnconfigured { provider: String },
    /// Provider's API-key env-var name is malformed. Do not echo it:
    /// malformed values may be pasted secret material from the provider catalog.
    #[error(
        "llm provider `{provider}` has an invalid api_key_env value; it must be an env-var name \
         like `OPENAI_API_KEY`, never the secret value"
    )]
    ApiKeyEnvInvalid { provider: String },
    /// Catalog field is malformed or secret-shaped. Do not echo it:
    /// values may be pasted secret material from the provider catalog.
    #[error(
        "llm provider `{provider}` at catalogue index {catalog_index} has an invalid catalog field `{field}`; \
         provider catalog fields must not contain inline secret material"
    )]
    CatalogFieldInvalid {
        provider: String,
        catalog_index: usize,
        field: &'static str,
    },
    /// Provider requires a base URL (e.g. generic OpenAI-compatible) but
    /// neither the catalog nor the selection supplied one.
    #[error(
        "llm provider `{provider}` requires a base_url but neither the catalog entry's \
         `default_base_url` nor the selection's `base_url` override are set"
    )]
    BaseUrlUnconfigured { provider: String },
    /// The shared provider constructor rejected the selected configuration.
    #[error("could not construct the selected LLM provider: {source}")]
    ProviderConstruction {
        #[source]
        source: brassclaw_llm::LlmError,
    },
    /// DB-backed resolution failed (pool error, query error, etc.).
    /// Only raised by `resolve_llm_selection_against_catalog_db`.
    #[error("DB-backed LLM catalog resolution failed: {reason}")]
    DbError { reason: String },
}

/// Resolve the default selection from the authoritative PostgreSQL catalogue.
///
/// Reads the active selection and provider definition from one coherent
/// database snapshot. Used by startup and live reload; no file/ENV catalogue
/// fallback is available.
///
/// If no `provider_id` is set in `brassclaw_config` (first run, no provider
/// configured yet), returns `Ok(None)`.
#[cfg(feature = "postgres")]
pub async fn resolve_llm_selection_against_catalog_db(
    pool: &brassclaw_pg::PgPool,
    tenant_id: &str,
    keys: &crate::LlmKeyStore,
) -> Result<Option<brassclaw_llm::LlmConfig>, RebornLlmCatalogError> {
    Ok(resolve_provider_selection_db(pool, tenant_id, keys)
        .await?
        .map(|(config, _)| config))
}

#[cfg(feature = "postgres")]
pub(crate) async fn resolve_provider_selection_db(
    pool: &brassclaw_pg::PgPool,
    tenant_id: &str,
    keys: &crate::LlmKeyStore,
) -> Result<Option<(brassclaw_llm::LlmConfig, ProviderDefinition)>, RebornLlmCatalogError> {
    let captured = crate::RebornProviderAdmin::new(pool.clone(), tenant_id)
        .capture_selection(
            crate::provider_admin::ProviderSelectionSource::Role(
                brassclaw_llm::ProviderRole::Kohai,
            ),
            keys,
        )
        .await
        .map_err(|error| RebornLlmCatalogError::DbError {
            reason: error.to_string(),
        })?;
    let Some(captured) = captured else {
        return Ok(None);
    };
    let registry = ProviderRegistry::new(vec![captured.definition.clone()]);
    Ok(Some((
        resolve_against_registry_with_credentials(
            &captured.selection,
            &registry,
            captured.credential,
        )?,
        captured.definition,
    )))
}

/// Resolve a selection against a pre-built registry. Useful in tests
/// where a synthetic registry can be assembled without touching the
/// filesystem.
#[cfg(test)]
fn resolve_against_registry(
    selection: &LlmSlotSelection,
    registry: &ProviderRegistry,
) -> Result<brassclaw_llm::LlmConfig, RebornLlmCatalogError> {
    resolve_registry_selection(selection, registry, None)
}

pub(crate) fn resolve_against_registry_with_credentials(
    selection: &LlmSlotSelection,
    registry: &ProviderRegistry,
    key: Option<secrecy::SecretString>,
) -> Result<brassclaw_llm::LlmConfig, RebornLlmCatalogError> {
    resolve_registry_selection(selection, registry, key)
}

fn resolve_registry_selection(
    selection: &LlmSlotSelection,
    registry: &ProviderRegistry,
    key: Option<secrecy::SecretString>,
) -> Result<brassclaw_llm::LlmConfig, RebornLlmCatalogError> {
    validate_catalog(registry)?;

    let provider_id = selection
        .provider_id
        .as_deref()
        .ok_or(RebornLlmCatalogError::MissingProviderId)?;

    let provider =
        registry
            .find(provider_id)
            .ok_or_else(|| RebornLlmCatalogError::UnknownProvider {
                requested: provider_id.to_string(),
                known: registry
                    .all()
                    .iter()
                    .map(|provider| safe_catalog_display_value("providers.<id>", &provider.id))
                    .collect::<Vec<_>>()
                    .join(", "),
            })?;

    let catalog_index = registry
        .all()
        .iter()
        .position(|candidate| std::ptr::eq(candidate, provider))
        .unwrap_or(0);

    validate_selection(selection, provider, catalog_index)?;

    let selection_config = ProviderSelection {
        provider_id: provider.id.clone(),
        api_key_env: selection.api_key_env.clone(),
        base_url: selection.base_url.clone(),
        model: selection.model.clone(),
    };
    let resolved =
        brassclaw_llm::resolve_provider_config_with_credentials(selection_config, registry, key)
            .map_err(|source| map_selection_resolution_error(source, selection, provider))?;

    validate_catalog_text(
        provider,
        catalog_index,
        "resolved_base_url",
        resolved.base_url(),
    )?;
    validate_catalog_text(provider, catalog_index, "resolved_model", resolved.model())?;

    let result = brassclaw_llm::build_llm_config_with_provider_policy(
        resolved,
        provider.request_policy,
        provider.response_cache,
    );
    result.map_err(|source| RebornLlmCatalogError::ProviderConstruction { source })
}

fn validate_selection(
    selection: &LlmSlotSelection,
    provider: &ProviderDefinition,
    catalog_index: usize,
) -> Result<(), RebornLlmCatalogError> {
    if let Some(env) = selection.api_key_env.as_deref()
        && (reject_inline_secret("llm.<slot>.api_key_env", env).is_err() || !is_env_var_name(env))
    {
        return Err(RebornLlmCatalogError::ApiKeyEnvInvalid {
            provider: provider.id.clone(),
        });
    }
    if let Some(base_url) = selection.base_url.as_deref() {
        validate_catalog_text(provider, catalog_index, "selection_base_url", base_url)?;
    }
    if let Some(model) = selection.model.as_deref() {
        validate_catalog_text(provider, catalog_index, "selection_model", model)?;
    }
    Ok(())
}

fn map_selection_resolution_error(
    source: ProviderResolutionError,
    selection: &LlmSlotSelection,
    provider: &ProviderDefinition,
) -> RebornLlmCatalogError {
    match source {
        ProviderResolutionError::MissingApiKey {
            provider: error_provider,
        } if error_provider == provider.id => {
            match selection
                .api_key_env
                .clone()
                .or_else(|| provider.api_key_env.clone())
            {
                Some(env) => RebornLlmCatalogError::ApiKeyEnvUnset {
                    provider: provider.id.clone(),
                    env,
                },
                None => RebornLlmCatalogError::ApiKeyEnvUnconfigured {
                    provider: provider.id.clone(),
                },
            }
        }
        ProviderResolutionError::MissingBaseUrl {
            provider: error_provider,
        } if error_provider == provider.id => RebornLlmCatalogError::BaseUrlUnconfigured {
            provider: provider.id.clone(),
        },
        source => RebornLlmCatalogError::ProviderConstruction {
            source: source.into_llm_error(),
        },
    }
}

pub(crate) fn validate_catalog(registry: &ProviderRegistry) -> Result<(), RebornLlmCatalogError> {
    for (catalog_index, provider) in registry.all().iter().enumerate() {
        validate_catalog_provider(provider, catalog_index)?;
    }
    Ok(())
}

fn validate_catalog_provider(
    provider: &ProviderDefinition,
    catalog_index: usize,
) -> Result<(), RebornLlmCatalogError> {
    provider
        .embedding_models
        .validate_for_protocol(provider.protocol)
        .map_err(|_| RebornLlmCatalogError::CatalogFieldInvalid {
            provider: safe_catalog_display_value("providers.<id>", &provider.id),
            catalog_index,
            field: "embedding_models",
        })?;
    provider
        .request_policy
        .validate_for_protocol(provider.protocol)
        .map_err(|_| RebornLlmCatalogError::CatalogFieldInvalid {
            provider: safe_catalog_display_value("providers.<id>", &provider.id),
            catalog_index,
            field: "request_policy",
        })?;
    provider.model_discovery.validate().map_err(|_| {
        RebornLlmCatalogError::CatalogFieldInvalid {
            provider: safe_catalog_display_value("providers.<id>", &provider.id),
            catalog_index,
            field: "model_discovery",
        }
    })?;
    provider
        .response_cache
        .validate()
        .map_err(|_| RebornLlmCatalogError::CatalogFieldInvalid {
            provider: safe_catalog_display_value("providers.<id>", &provider.id),
            catalog_index,
            field: "response_cache",
        })?;
    if provider.context_window_tokens == Some(0) {
        return Err(RebornLlmCatalogError::CatalogFieldInvalid {
            provider: safe_catalog_display_value("providers.<id>", &provider.id),
            catalog_index,
            field: "context_window_tokens",
        });
    }
    if provider
        .cache_retention
        .as_deref()
        .is_some_and(|value| value.parse::<brassclaw_llm::CacheRetention>().is_err())
    {
        return Err(RebornLlmCatalogError::CatalogFieldInvalid {
            provider: safe_catalog_display_value("providers.<id>", &provider.id),
            catalog_index,
            field: "cache_retention",
        });
    }
    validate_catalog_text(provider, catalog_index, "id", &provider.id)?;
    if let Some(base_url) = provider.default_base_url.as_deref() {
        validate_catalog_text(provider, catalog_index, "default_base_url", base_url)?;
    }
    validate_catalog_text(
        provider,
        catalog_index,
        "default_model",
        &provider.default_model,
    )?;
    validate_catalog_env_var(provider, catalog_index, "model_env", &provider.model_env)?;
    if let Some(base_url_env) = provider.base_url_env.as_deref() {
        validate_catalog_env_var(provider, catalog_index, "base_url_env", base_url_env)?;
    }
    if let Some(api_key_env) = provider.api_key_env.as_deref() {
        validate_catalog_env_var(provider, catalog_index, "api_key_env", api_key_env)?;
    }
    if let Some(extra_headers_env) = provider.extra_headers_env.as_deref() {
        validate_catalog_env_var(
            provider,
            catalog_index,
            "extra_headers_env",
            extra_headers_env,
        )?;
    }
    Ok(())
}

fn validate_catalog_text(
    provider: &ProviderDefinition,
    catalog_index: usize,
    field: &'static str,
    value: &str,
) -> Result<(), RebornLlmCatalogError> {
    if reject_inline_secret("provider catalog field", value).is_err() {
        return Err(RebornLlmCatalogError::CatalogFieldInvalid {
            provider: safe_catalog_display_value("providers.<id>", &provider.id),
            catalog_index,
            field,
        });
    }
    Ok(())
}

fn validate_catalog_env_var(
    provider: &ProviderDefinition,
    catalog_index: usize,
    field: &'static str,
    value: &str,
) -> Result<(), RebornLlmCatalogError> {
    if reject_inline_secret("provider catalog env-var field", value).is_err()
        || !is_env_var_name(value)
    {
        return Err(RebornLlmCatalogError::CatalogFieldInvalid {
            provider: safe_catalog_display_value("providers.<id>", &provider.id),
            catalog_index,
            field,
        });
    }
    Ok(())
}

fn safe_catalog_display_value(label: &'static str, value: &str) -> String {
    if reject_inline_secret(label, value).is_err() {
        "<redacted>".to_string()
    } else {
        value.to_string()
    }
}

fn is_env_var_name(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_llm::registry::{ProviderProtocol, ProviderRegistry};

    #[test]
    fn invalid_context_and_cache_properties_fail_before_resolution() {
        for (context, cache, field) in [
            (Some(0), None, "context_window_tokens"),
            (Some(131072), Some("invalid-retention"), "cache_retention"),
        ] {
            let mut definition = provider_no_key_required("alpha");
            definition.context_window_tokens = context;
            definition.cache_retention = cache.map(str::to_owned);
            let registry = ProviderRegistry::new(vec![definition]);
            let selection = LlmSlotSelection {
                provider_id: Some("alpha".into()),
                ..Default::default()
            };
            assert!(
                matches!(resolve_against_registry_with_credentials(&selection, &registry, None),
                Err(RebornLlmCatalogError::CatalogFieldInvalid { field: actual, .. }) if actual == field)
            );
        }
    }

    fn provider_with_required_key(id: &str, env: &str) -> ProviderDefinition {
        ProviderDefinition {
            embedding_models: Default::default(),
            model_discovery: Default::default(),
            request_policy: Default::default(),
            id: id.to_string(),
            aliases: Vec::new(),
            protocol: ProviderProtocol::OpenAiCompletions,
            default_base_url: Some("https://example.test/v1".to_string()),
            base_url_env: None,
            base_url_required: false,
            api_key_env: Some(env.to_string()),
            api_key_required: true,
            model_env: "TEST_MODEL".to_string(),
            default_model: "test-model".to_string(),
            description: "test".to_string(),
            extra_headers_env: None,
            unsupported_params: Vec::new(),
            setup: None,
            token_budget: None,
            context_window_tokens: None,
            cache_retention: None,
            response_cache: Default::default(),
        }
    }

    fn provider_no_key_required(id: &str) -> ProviderDefinition {
        ProviderDefinition {
            embedding_models: Default::default(),
            model_discovery: Default::default(),
            request_policy: Default::default(),
            id: id.to_string(),
            aliases: Vec::new(),
            protocol: ProviderProtocol::Ollama,
            default_base_url: Some("http://localhost:11434".to_string()),
            base_url_env: None,
            base_url_required: false,
            api_key_env: None,
            api_key_required: false,
            model_env: "TEST_MODEL".to_string(),
            default_model: "llama3".to_string(),
            description: "test (no key)".to_string(),
            extra_headers_env: None,
            unsupported_params: Vec::new(),
            setup: None,
            token_budget: None,
            context_window_tokens: None,
            cache_retention: None,
            response_cache: Default::default(),
        }
    }

    fn provider_no_base_url_required(id: &str) -> ProviderDefinition {
        ProviderDefinition {
            embedding_models: Default::default(),
            model_discovery: Default::default(),
            request_policy: Default::default(),
            id: id.to_string(),
            aliases: Vec::new(),
            protocol: ProviderProtocol::OpenAiCompletions,
            default_base_url: None,
            base_url_env: Some("REBORN_TEST_OPTIONAL_BASE_URL_UNSET_DO_NOT_SET_9c13".to_string()),
            base_url_required: false,
            api_key_env: None,
            api_key_required: false,
            model_env: "REBORN_TEST_MODEL_UNSET_DO_NOT_SET_9c13".to_string(),
            default_model: "default-model".to_string(),
            description: "test (client-default base url)".to_string(),
            extra_headers_env: None,
            unsupported_params: Vec::new(),
            setup: None,
            token_budget: None,
            context_window_tokens: None,
            cache_retention: None,
            response_cache: Default::default(),
        }
    }

    fn provider_base_url_required(id: &str) -> ProviderDefinition {
        ProviderDefinition {
            base_url_required: true,
            ..provider_no_base_url_required(id)
        }
    }

    fn provider_github_copilot_no_key_required(id: &str) -> ProviderDefinition {
        ProviderDefinition {
            embedding_models: Default::default(),
            model_discovery: Default::default(),
            request_policy: Default::default(),
            id: id.to_string(),
            aliases: Vec::new(),
            protocol: ProviderProtocol::GithubCopilot,
            default_base_url: Some("https://api.githubcopilot.com".to_string()),
            base_url_env: None,
            base_url_required: false,
            api_key_env: None,
            api_key_required: false,
            model_env: "REBORN_TEST_GITHUB_COPILOT_MODEL_UNSET_DO_NOT_SET_9c13".to_string(),
            default_model: "gpt-4o".to_string(),
            description: "test github copilot".to_string(),
            extra_headers_env: None,
            unsupported_params: Vec::new(),
            setup: None,
            token_budget: None,
            context_window_tokens: None,
            cache_retention: None,
            response_cache: Default::default(),
        }
    }

    fn provider_with_protocol(id: &str, protocol: ProviderProtocol) -> ProviderDefinition {
        ProviderDefinition {
            embedding_models: Default::default(),
            model_discovery: Default::default(),
            request_policy: Default::default(),
            id: id.to_string(),
            aliases: Vec::new(),
            protocol,
            default_base_url: None,
            base_url_env: None,
            base_url_required: false,
            api_key_env: None,
            api_key_required: false,
            model_env: "REBORN_TEST_DEDICATED_MODEL_UNSET_DO_NOT_SET_9c13".to_string(),
            default_model: "dedicated-default-model".to_string(),
            description: "test dedicated provider".to_string(),
            extra_headers_env: None,
            unsupported_params: Vec::new(),
            setup: None,
            token_budget: None,
            context_window_tokens: None,
            cache_retention: None,
            response_cache: Default::default(),
        }
    }

    #[test]
    fn unknown_provider_lists_known_ids() {
        let registry =
            ProviderRegistry::new(vec![provider_with_required_key("alpha", "ALPHA_KEY")]);
        let selection = LlmSlotSelection {
            provider_id: Some("does-not-exist".to_string()),
            ..Default::default()
        };
        let err = resolve_against_registry(&selection, &registry).expect_err("must error");
        match err {
            RebornLlmCatalogError::UnknownProvider { requested, known } => {
                assert_eq!(requested, "does-not-exist");
                assert!(known.contains("alpha"), "known list: {known}");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn missing_required_api_key_env_fails_closed() {
        // Use a uniquely-named env var that we never set; no `set_var`
        // / `remove_var` calls (forbidden under `forbid(unsafe_code)`
        // post-edition-2024). The unique suffix means even if the test
        // environment happens to pre-set similar names, this one is
        // free.
        let env_name = "REBORN_TEST_UNSET_API_KEY_DO_NOT_SET_8a3f1c2e";
        debug_assert!(
            std::env::var(env_name).is_err(),
            "test depends on `{env_name}` being unset"
        );
        let registry = ProviderRegistry::new(vec![provider_with_required_key("alpha", env_name)]);
        let selection = LlmSlotSelection {
            provider_id: Some("alpha".to_string()),
            ..Default::default()
        };
        let err = resolve_against_registry(&selection, &registry).expect_err("must error");
        assert!(matches!(err, RebornLlmCatalogError::ApiKeyEnvUnset { .. }));
    }

    #[test]
    fn malformed_api_key_env_fails_without_echoing_value() {
        let pasted_secret = format!("{}{}", "s", "k-proj-1234567890abcdef1234567890");
        let registry =
            ProviderRegistry::new(vec![provider_with_required_key("alpha", &pasted_secret)]);
        let selection = LlmSlotSelection {
            provider_id: Some("alpha".to_string()),
            ..Default::default()
        };

        let err = resolve_against_registry(&selection, &registry).expect_err("must error");
        assert!(matches!(
            err,
            RebornLlmCatalogError::CatalogFieldInvalid {
                field: "api_key_env",
                ..
            }
        ));
        let rendered = err.to_string();
        assert!(
            !rendered.contains(&pasted_secret),
            "error must not echo pasted secret: {rendered}"
        );
    }

    #[test]
    fn secret_shaped_env_name_fails_without_echoing_value() {
        // `github_pat_...` is syntactically a valid env-var name, so the
        // inline-secret guard must run before missing-env errors can echo it.
        let pasted_secret = format!("{}{}", "github_", "pat_abcdef1234567890abcdef1234567890");
        let registry =
            ProviderRegistry::new(vec![provider_with_required_key("alpha", &pasted_secret)]);
        let selection = LlmSlotSelection {
            provider_id: Some("alpha".to_string()),
            ..Default::default()
        };

        let err = resolve_against_registry(&selection, &registry).expect_err("must error");
        assert!(matches!(
            err,
            RebornLlmCatalogError::CatalogFieldInvalid {
                field: "api_key_env",
                ..
            }
        ));
        let rendered = err.to_string();
        assert!(
            !rendered.contains(&pasted_secret),
            "error must not echo pasted secret: {rendered}"
        );
    }

    #[test]
    fn secret_shaped_default_base_url_fails_without_echoing_value() {
        let pasted_secret = format!(
            "https://proxy.example/v1?key={}{}",
            "s", "k-proj-1234567890abcdef1234567890"
        );
        let provider = ProviderDefinition {
            default_base_url: Some(pasted_secret.clone()),
            ..provider_no_key_required("alpha")
        };
        let registry = ProviderRegistry::new(vec![provider]);
        let selection = LlmSlotSelection {
            provider_id: Some("alpha".to_string()),
            ..Default::default()
        };

        let err = resolve_against_registry(&selection, &registry).expect_err("must error");
        assert!(matches!(
            err,
            RebornLlmCatalogError::CatalogFieldInvalid {
                field: "default_base_url",
                ..
            }
        ));
        let rendered = err.to_string();
        assert!(
            !rendered.contains(&pasted_secret),
            "error must not echo pasted secret: {rendered}"
        );
    }

    #[test]
    fn unused_secret_shaped_catalog_entry_fails_closed() {
        let pasted_secret = format!(
            "https://proxy.example/v1?key={}{}",
            "s", "k-proj-1234567890abcdef1234567890"
        );
        let unused_provider = ProviderDefinition {
            default_base_url: Some(pasted_secret.clone()),
            ..provider_no_key_required("unused")
        };
        let registry =
            ProviderRegistry::new(vec![unused_provider, provider_no_key_required("selected")]);
        let selection = LlmSlotSelection {
            provider_id: Some("selected".to_string()),
            ..Default::default()
        };

        let err = resolve_against_registry(&selection, &registry)
            .expect_err("unused secret-shaped catalog entry must fail closed");
        assert!(matches!(
            err,
            RebornLlmCatalogError::CatalogFieldInvalid {
                ref provider,
                catalog_index: 0,
                field: "default_base_url",
            } if provider == "unused"
        ));
        let rendered = err.to_string();
        assert!(
            !rendered.contains(&pasted_secret),
            "error must not echo pasted secret: {rendered}"
        );
    }

    #[test]
    fn unknown_provider_redacts_secret_shaped_catalog_ids() {
        let pasted_secret = format!("{}{}", "github_", "pat_abcdef1234567890abcdef1234567890");
        let registry = ProviderRegistry::new(vec![provider_no_key_required(&pasted_secret)]);
        let selection = LlmSlotSelection {
            provider_id: Some("missing".to_string()),
            ..Default::default()
        };

        let err = resolve_against_registry(&selection, &registry).expect_err("must error");
        let rendered = err.to_string();
        assert!(rendered.contains("<redacted>"), "error: {rendered}");
        assert!(
            !rendered.contains(&pasted_secret),
            "error must not echo pasted secret: {rendered}"
        );
    }

    #[test]
    fn happy_path_no_key_required_uses_catalog_default_model_and_base_url() {
        let registry = ProviderRegistry::new(vec![provider_no_key_required("alpha")]);
        let selection = LlmSlotSelection {
            provider_id: Some("alpha".to_string()),
            ..Default::default()
        };

        let config = resolve_against_registry(&selection, &registry).expect("must resolve");
        let provider = config.provider.as_ref().expect("registry provider");
        assert_eq!(provider.provider_id, "alpha");
        assert_eq!(provider.model, "llama3"); // catalog default
        assert_eq!(provider.base_url, "http://localhost:11434"); // catalog default
        assert_eq!(provider.protocol, ProviderProtocol::Ollama);
        assert!(provider.api_key.is_none());
    }

    #[test]
    fn optional_missing_base_url_resolves_to_client_default() {
        let registry = ProviderRegistry::new(vec![provider_no_base_url_required("alpha")]);
        let selection = LlmSlotSelection {
            provider_id: Some("alpha".to_string()),
            ..Default::default()
        };

        let config = resolve_against_registry(&selection, &registry).expect("must resolve");
        let provider = config.provider.as_ref().expect("registry provider");
        assert_eq!(provider.base_url, "");
        assert_eq!(provider.model, "default-model");
    }

    #[test]
    fn missing_required_base_url_fails_closed() {
        let registry = ProviderRegistry::new(vec![provider_base_url_required("alpha")]);
        let selection = LlmSlotSelection {
            provider_id: Some("alpha".to_string()),
            ..Default::default()
        };

        let err = resolve_against_registry(&selection, &registry).expect_err("must error");
        assert!(matches!(
            err,
            RebornLlmCatalogError::BaseUrlUnconfigured { .. }
        ));
    }

    #[test]
    fn selection_overrides_take_precedence_over_catalog() {
        let registry = ProviderRegistry::new(vec![provider_no_key_required("alpha")]);
        let selection = LlmSlotSelection {
            provider_id: Some("alpha".to_string()),
            model: Some("custom-model".to_string()),
            base_url: Some("https://override.test/v1".to_string()),
            api_key_env: None,
        };

        let config = resolve_against_registry(&selection, &registry).expect("must resolve");
        let provider = config.provider.as_ref().expect("registry provider");
        assert_eq!(provider.model, "custom-model");
        assert_eq!(provider.base_url, "https://override.test/v1");
    }

    #[test]
    fn github_copilot_protocol_carries_default_headers() {
        let registry = ProviderRegistry::new(vec![provider_github_copilot_no_key_required(
            "tenant_copilot",
        )]);
        let selection = LlmSlotSelection {
            provider_id: Some("tenant_copilot".to_string()),
            ..Default::default()
        };

        let config = resolve_against_registry(&selection, &registry).expect("must resolve");
        assert!(
            config
                .provider
                .as_ref()
                .expect("registry provider")
                .extra_headers
                .iter()
                .any(|(key, _)| key == "Editor-Version"),
            "headers: {:?}",
            config.provider.as_ref().unwrap().extra_headers
        );
        assert!(
            config
                .provider
                .as_ref()
                .expect("registry provider")
                .extra_headers
                .iter()
                .any(|(key, _)| key == "Copilot-Integration-Id"),
            "headers: {:?}",
            config.provider.as_ref().unwrap().extra_headers
        );
    }

    #[test]
    fn nearai_catalog_selection_resolves_to_full_dedicated_llm_config() {
        let registry = ProviderRegistry::new(vec![provider_with_protocol(
            "nearai",
            ProviderProtocol::NearAi,
        )]);
        let selection = LlmSlotSelection {
            provider_id: Some("nearai".to_string()),
            model: Some("nearai/test-model".to_string()),
            base_url: Some("https://private.near.ai".to_string()),
            api_key_env: None,
        };

        let config = resolve_against_registry(&selection, &registry).expect("must resolve");
        assert_eq!(config.backend, "nearai");
        assert_eq!(config.nearai.model, "nearai/test-model");
        assert_eq!(config.nearai.base_url, "https://private.near.ai");
        assert!(config.provider.is_none());
    }

    #[test]
    fn openai_codex_catalog_selection_resolves_to_full_dedicated_llm_config() {
        let registry = ProviderRegistry::new(vec![provider_with_protocol(
            "openai_codex",
            ProviderProtocol::OpenAiCodex,
        )]);
        let selection = LlmSlotSelection {
            provider_id: Some("openai_codex".to_string()),
            model: Some("gpt-test-codex".to_string()),
            ..Default::default()
        };

        let config = resolve_against_registry(&selection, &registry).expect("must resolve");
        assert_eq!(config.backend, "openai_codex");
        assert_eq!(
            config.openai_codex.as_ref().expect("codex config").model,
            "gpt-test-codex"
        );
        assert!(config.provider.is_none());
    }

    #[test]
    fn gemini_oauth_catalog_selection_resolves_to_full_dedicated_llm_config() {
        let registry = ProviderRegistry::new(vec![provider_with_protocol(
            "gemini_oauth",
            ProviderProtocol::GeminiOauth,
        )]);
        let selection = LlmSlotSelection {
            provider_id: Some("gemini_oauth".to_string()),
            model: Some("gemini-test".to_string()),
            ..Default::default()
        };

        let config = resolve_against_registry(&selection, &registry).expect("must resolve");
        assert_eq!(config.backend, "gemini_oauth");
        assert_eq!(
            config
                .gemini_oauth
                .as_ref()
                .expect("gemini oauth config")
                .model,
            "gemini-test"
        );
        assert!(config.provider.is_none());
    }

    #[test]
    fn secret_shaped_extra_headers_env_name_fails_before_value_lookup() {
        let mut provider = provider_no_key_required("alpha");
        provider.extra_headers_env = Some("sk-proj-1234567890abcdef".to_string());

        let err = validate_catalog_provider(&provider, 7).expect_err("secret-shaped env must fail");
        let rendered = err.to_string();
        assert!(matches!(
            err,
            RebornLlmCatalogError::CatalogFieldInvalid { .. }
        ));
        assert!(
            !rendered.contains("sk-proj-1234567890abcdef"),
            "error must not echo secret-shaped env name: {rendered}"
        );
        assert!(
            rendered.contains("catalogue index 7"),
            "error must identify catalog index: {rendered}"
        );
    }

    #[test]
    fn stored_api_key_resolves_registry_provider() {
        use secrecy::ExposeSecret as _;

        let registry = ProviderRegistry::new(vec![provider_no_key_required("alpha")]);
        let selection = LlmSlotSelection {
            provider_id: Some("alpha".to_string()),
            ..Default::default()
        };
        let config = resolve_against_registry_with_credentials(
            &selection,
            &registry,
            Some(secrecy::SecretString::from("sk-stored")),
        )
        .expect("credential-aware resolve");

        let stored = config
            .provider
            .as_ref()
            .expect("provider")
            .api_key
            .as_ref()
            .expect("api key");
        assert_eq!(stored.expose_secret(), "sk-stored");
    }

    #[test]
    fn stored_api_key_resolves_nearai_dedicated_config() {
        use secrecy::ExposeSecret as _;

        let registry = ProviderRegistry::new(vec![provider_with_protocol(
            "nearai",
            ProviderProtocol::NearAi,
        )]);
        let selection = LlmSlotSelection {
            provider_id: Some("nearai".to_string()),
            model: Some("nearai/test-model".to_string()),
            base_url: Some("https://private.near.ai".into()),
            ..Default::default()
        };
        let config = resolve_against_registry_with_credentials(
            &selection,
            &registry,
            Some(secrecy::SecretString::from("sk-nearai")),
        )
        .expect("credential-aware resolve");
        assert!(config.provider.is_none());

        assert_eq!(
            config
                .nearai
                .api_key
                .as_ref()
                .expect("nearai key")
                .expose_secret(),
            "sk-nearai"
        );
    }
}
