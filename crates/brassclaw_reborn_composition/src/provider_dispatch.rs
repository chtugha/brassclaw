//! Live compatibility of a retained provider; never selects another target.
use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_llm::{LlmError, ProviderDispatchGuard, ProviderDispatchState, ProviderRequestFacts};

use crate::provider_admin::{CapturedProviderSelection, ProviderSelectionSource};

pub(crate) struct PgProviderDispatchGuard {
    admin: crate::RebornProviderAdmin,
    keys: crate::LlmKeyStore,
    selection: brassclaw_reborn_config::LlmSlotSelection,
    definition: brassclaw_llm::ProviderDefinition,
    credential_ref: Option<String>,
    endpoint: String,
}

impl std::fmt::Debug for PgProviderDispatchGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PgProviderDispatchGuard")
            .field("provider", &self.definition.id)
            .finish_non_exhaustive()
    }
}

impl PgProviderDispatchGuard {
    pub(crate) fn from_capture(
        pool: brassclaw_pg::PgPool,
        tenant: &str,
        keys: crate::LlmKeyStore,
        captured: &CapturedProviderSelection,
    ) -> Result<Arc<Self>, LlmError> {
        Self::from_admin(
            crate::RebornProviderAdmin::new(pool, tenant),
            keys,
            captured,
        )
    }

    pub(crate) fn from_admin(
        admin: crate::RebornProviderAdmin,
        keys: crate::LlmKeyStore,
        captured: &CapturedProviderSelection,
    ) -> Result<Arc<Self>, LlmError> {
        let registry = brassclaw_llm::ProviderRegistry::new(vec![captured.definition.clone()]);
        let resolved = brassclaw_llm::resolve_provider_config_with_credentials(
            brassclaw_llm::ProviderSelection {
                provider_id: captured.definition.id.clone(),
                api_key_env: captured.selection.api_key_env.clone(),
                base_url: captured.selection.base_url.clone(),
                model: captured.selection.model.clone(),
            },
            &registry,
            captured.credential.clone(),
        )
        .map_err(|_| changed())?;
        // Preserve the actual selected model even if a later edit changes the
        // provider's default. A role switch alone does not invalidate old work.
        let mut selection = captured.selection.clone();
        selection.model = Some(resolved.model().to_owned());
        Ok(Arc::new(Self {
            admin,
            keys,
            selection,
            definition: captured.definition.clone(),
            credential_ref: captured.credential_ref.clone(),
            endpoint: resolved.base_url().to_owned(),
        }))
    }
}

fn changed() -> LlmError {
    LlmError::ConfigurationChanged {
        provider: "retained database provider".into(),
    }
}

impl PgProviderDispatchGuard {
    async fn current(
        &self,
        facts: ProviderRequestFacts<'_>,
        text_bounds: bool,
    ) -> Result<(brassclaw_llm::ProviderDefinition, u64), LlmError> {
        if facts.model.is_empty()
            || facts.model.trim() != facts.model
            || facts.model.chars().any(char::is_control)
        {
            return Err(changed());
        }
        let mut selection = self.selection.clone();
        selection.model = Some(facts.model.to_owned());
        let current = self
            .admin
            .capture_selection(ProviderSelectionSource::Explicit(selection), &self.keys)
            .await
            .map_err(|_| changed())?
            .ok_or_else(changed)?;
        let definition = &current.definition;
        if definition.id != self.definition.id || definition.protocol != self.definition.protocol
            || current.credential_ref != self.credential_ref
            || definition.api_key_required != self.definition.api_key_required
            || definition.unsupported_params != self.definition.unsupported_params
            // These transport/retry/circuit settings are captured by the real
            // chain. Never report a new policy while dispatching with the old one.
            || definition.request_policy != self.definition.request_policy
            || definition.cache_retention != self.definition.cache_retention
        {
            return Err(changed());
        }
        let registry = brassclaw_llm::ProviderRegistry::new(vec![definition.clone()]);
        let resolved = brassclaw_llm::resolve_provider_config_with_credentials(
            brassclaw_llm::ProviderSelection {
                provider_id: definition.id.clone(),
                api_key_env: current.selection.api_key_env,
                base_url: current.selection.base_url,
                model: Some(facts.model.to_owned()),
            },
            &registry,
            current.credential,
        )
        .map_err(|_| changed())?;
        if resolved.base_url() != self.endpoint || resolved.model() != facts.model {
            return Err(changed());
        }

        // Unmeasured callers retain the provisional tightening fence. A selected
        // native final guard instead reads these same current caps after full
        // encoding and again at the single-use transport barrier. It cannot
        // submit via the old limits or fall back to an unchecked HTTP client.
        let final_encoding_selected = text_bounds
            && brassclaw_llm::final_request_guard::has_selected_target(&definition.id, facts.model);
        if text_bounds && !final_encoding_selected {
            let previous = self.definition.model_token_limits.0.get(facts.model);
            let current = definition.model_token_limits.0.get(facts.model);
            let changed_limits = match (previous, current) {
                (Some(_), None) | (None, Some(_)) => true,
                (Some(old), Some(limits)) => {
                    limits.max_prefix_tokens < old.max_prefix_tokens
                        || limits.max_prompt_tokens < old.max_prompt_tokens
                }
                (None, None) => false,
            };
            if changed_limits {
                return Err(changed());
            }
        }
        if text_bounds
            && !final_encoding_selected
            && facts.model == definition.default_model
            && let Some(context) = definition.context_window_tokens
            && (facts
                .max_output_tokens
                .is_some_and(|tokens| tokens > context)
                || (facts.model == self.definition.default_model
                    && self
                        .definition
                        .context_window_tokens
                        .is_none_or(|old| context < old)))
        {
            return Err(changed());
        }
        Ok((current.definition, current.revision.definition_revision))
    }
}

impl PgProviderDispatchGuard {
    async fn dispatch_state(
        &self,
        facts: ProviderRequestFacts<'_>,
        text_bounds: bool,
    ) -> Result<ProviderDispatchState, LlmError> {
        let (definition, revision) = self.current(facts, text_bounds).await?;
        Ok(ProviderDispatchState {
            definition_revision: revision,
            response_cache: definition.response_cache,
            provider_id: definition.id.clone(),
            model: facts.model.to_owned(),
            model_token_limits: definition.model_token_limits.0.get(facts.model).copied(),
            configured_context_window_tokens: if facts.model == definition.default_model {
                definition.context_window_tokens
            } else {
                None
            },
            requested_output_tokens: facts.max_output_tokens,
        })
    }
}
#[async_trait]
impl ProviderDispatchGuard for PgProviderDispatchGuard {
    async fn check(
        &self,
        facts: ProviderRequestFacts<'_>,
    ) -> Result<ProviderDispatchState, LlmError> {
        self.dispatch_state(facts, true).await
    }
    async fn check_preparation(
        &self,
        facts: ProviderRequestFacts<'_>,
    ) -> Result<ProviderDispatchState, LlmError> {
        self.dispatch_state(facts, false).await
    }
}

#[async_trait]
impl brassclaw_embeddings::EmbeddingDispatchGuard for PgProviderDispatchGuard {
    async fn check(
        &self,
    ) -> Result<brassclaw_embeddings::EmbeddingDispatchState, brassclaw_embeddings::EmbeddingError>
    {
        let unavailable = || {
            brassclaw_embeddings::EmbeddingError::InvalidResponse(
                "retained embedding configuration changed or is unavailable".into(),
            )
        };
        let model = self.selection.model.as_deref().ok_or_else(unavailable)?;
        let (definition, revision) = self
            .current(
                ProviderRequestFacts {
                    model,
                    max_output_tokens: None,
                },
                false,
            )
            .await
            .map_err(|_| unavailable())?;
        let old = self
            .definition
            .embedding_models
            .0
            .get(model)
            .ok_or_else(unavailable)?;
        let current = definition
            .embedding_models
            .0
            .get(model)
            .ok_or_else(unavailable)?;
        if old.dimension != current.dimension || old.vector_revision != current.vector_revision {
            return Err(unavailable());
        }
        Ok(brassclaw_embeddings::EmbeddingDispatchState {
            definition_revision: revision,
            cache_max_entries: usize::try_from(current.cache_max_entries)
                .map_err(|_| unavailable())?,
        })
    }
}
