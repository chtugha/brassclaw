//! PostgreSQL-backed provider/model administration read facade.
//! Selection and active definitions are read from one repeatable-read snapshot.
//! File configuration and compiled provider catalogues are never fallback sources.

use std::fmt;

use brassclaw_pg::PgPool;
use brassclaw_reborn_config::{LlmSlotSelection, RebornConfigFile};
use serde::Serialize;
use thiserror::Error;

#[derive(Clone)]
pub struct RebornProviderAdmin {
    pool: PgPool,
    tenant_id: String,
}

/// An explicit operator draft or an independently selected persisted role.
/// This is model selection data, never an invocation permission.
pub(crate) enum ProviderSelectionSource {
    Role(brassclaw_llm::ProviderRole),
    Explicit(LlmSlotSelection),
}

pub(crate) struct CapturedProviderSelection {
    pub selection: LlmSlotSelection,
    pub definition: brassclaw_llm::registry::ProviderDefinition,
    pub credential: Option<secrecy::SecretString>,
}

impl RebornProviderAdmin {
    pub fn new(pool: PgPool, tenant_id: impl Into<String>) -> Self {
        Self {
            pool,
            tenant_id: tenant_id.into(),
        }
    }

    pub(crate) async fn snapshot(
        &self,
    ) -> Result<(RebornConfigFile, brassclaw_llm::ProviderRegistry), RebornProviderAdminError> {
        crate::PgProviderRepo::new(self.pool.clone(), &self.tenant_id)
            .initialize()
            .await
            .map_err(|error| RebornProviderAdminError::LoadDatabase {
                reason: error.to_string(),
            })?;
        let mut client =
            self.pool
                .get()
                .await
                .map_err(|error| RebornProviderAdminError::LoadDatabase {
                    reason: error.to_string(),
                })?;
        let transaction = tokio_postgres::Client::build_transaction(&mut client)
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(|error| RebornProviderAdminError::LoadDatabase {
                reason: error.to_string(),
            })?;
        let snapshot = self.snapshot_from_client(&transaction).await?;
        transaction
            .commit()
            .await
            .map_err(|error| RebornProviderAdminError::LoadDatabase {
                reason: error.to_string(),
            })?;
        Ok(snapshot)
    }

    /// Reuse catalogue validation for reads made inside an owning transaction.
    async fn snapshot_from_client<C: tokio_postgres::GenericClient + Sync>(
        &self,
        client: &C,
    ) -> Result<(RebornConfigFile, brassclaw_llm::ProviderRegistry), RebornProviderAdminError> {
        let config = crate::db_config::load_config_snapshot_from_client(client, &self.tenant_id)
            .await
            .map_err(|error| RebornProviderAdminError::LoadDatabase {
                reason: error.to_string(),
            })?;
        let repo =
            crate::pg_provider_repo::PgProviderRepo::new(self.pool.clone(), self.tenant_id.clone());
        let definitions = repo.load_all_from_client(client).await.map_err(|error| {
            RebornProviderAdminError::LoadRegistry {
                reason: error.to_string(),
            }
        })?;
        // Registry lookup accepts aliases, but cannot silently let one active
        // provider hide another provider's canonical identity or alias.
        let mut identities = std::collections::HashMap::new();
        for (definition, _) in &definitions {
            for name in std::iter::once(&definition.id).chain(&definition.aliases) {
                if name.trim().is_empty() {
                    return Err(RebornProviderAdminError::LoadRegistry {
                        reason: "active provider contains an empty identity or alias".into(),
                    });
                }
                if let Some(previous) = identities.insert(name.to_lowercase(), &definition.id)
                    && previous != &definition.id
                {
                    return Err(RebornProviderAdminError::LoadRegistry {
                        reason: "active provider identities or aliases conflict".into(),
                    });
                }
            }
        }
        Ok((
            config,
            brassclaw_llm::ProviderRegistry::new(
                definitions
                    .into_iter()
                    .map(|(definition, _)| definition)
                    .collect(),
            ),
        ))
    }

    /// Capture role/model, exact definition and authenticated one-shot secret
    /// together. Never retain a pool connection during provider/network work.
    pub(crate) async fn capture_selection(
        &self,
        source: ProviderSelectionSource,
        keys: &crate::LlmKeyStore,
    ) -> Result<Option<CapturedProviderSelection>, RebornProviderAdminError> {
        crate::PgProviderRepo::new(self.pool.clone(), &self.tenant_id)
            .initialize()
            .await
            .map_err(|_| RebornProviderAdminError::LoadDatabase {
                reason: "provider initialization is unavailable".into(),
            })?;
        let mut client =
            self.pool
                .get()
                .await
                .map_err(|_| RebornProviderAdminError::LoadDatabase {
                    reason: "provider snapshot is unavailable".into(),
                })?;
        // Secret lease consumption and authenticated legacy upgrades are writes.
        let transaction = tokio_postgres::Client::build_transaction(&mut client)
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .start()
            .await
            .map_err(|_| RebornProviderAdminError::LoadDatabase {
                reason: "provider snapshot is unavailable".into(),
            })?;
        let (config, registry) = self.snapshot_from_client(&transaction).await?;
        let selection = match source {
            ProviderSelectionSource::Explicit(selection) => Some(selection),
            ProviderSelectionSource::Role(brassclaw_llm::ProviderRole::Kohai) => {
                config.default_llm_slot().cloned()
            }
            ProviderSelectionSource::Role(brassclaw_llm::ProviderRole::Sempai) => config
                .llm
                .as_ref()
                .and_then(|roles| roles.get("sempai"))
                .cloned(),
            ProviderSelectionSource::Role(brassclaw_llm::ProviderRole::Embedding) => {
                config.embedding.map(|embedding| LlmSlotSelection {
                    provider_id: embedding.provider_id,
                    model: embedding.model,
                    api_key_env: None,
                    base_url: None,
                })
            }
        };
        let captured = match selection.filter(|selection| {
            selection
                .provider_id
                .as_deref()
                .is_some_and(|id| !id.is_empty())
        }) {
            None => None,
            Some(mut selection) => {
                crate::llm_catalog::validate_catalog(&registry).map_err(|_| {
                    RebornProviderAdminError::LoadRegistry {
                        reason: "provider catalogue is invalid".into(),
                    }
                })?;
                let requested = selection.provider_id.as_deref().unwrap_or_default();
                let definition = registry.find(requested).cloned().ok_or_else(|| {
                    RebornProviderAdminError::UnknownProvider {
                        provider: requested.into(),
                        known: known_provider_ids(&registry),
                    }
                })?;
                let credential = keys
                    .read_in_transaction(&transaction, &definition.id)
                    .await
                    .map_err(|_| RebornProviderAdminError::LoadDatabase {
                        reason: "provider credential snapshot is unavailable".into(),
                    })?;
                selection.provider_id = Some(definition.id.clone());
                Some(CapturedProviderSelection {
                    selection,
                    definition,
                    credential,
                })
            }
        };
        transaction
            .commit()
            .await
            .map_err(|_| RebornProviderAdminError::LoadDatabase {
                reason: "provider credential snapshot could not be committed".into(),
            })?;
        drop(client);
        Ok(captured)
    }

    pub async fn list(
        &self,
        provider: Option<&str>,
        verbose: bool,
    ) -> Result<RebornProviderList, RebornProviderAdminError> {
        let (config, registry) = self.snapshot().await?;
        let active = active_llm_selection(Some(&config), &registry);
        let providers = if let Some(provider) = provider {
            let definition = registry.find(provider).ok_or_else(|| {
                RebornProviderAdminError::UnknownProvider {
                    provider: provider.to_string(),
                    known: known_provider_ids(&registry),
                }
            })?;
            vec![provider_info(definition, active.as_ref(), true)]
        } else {
            unique_provider_definitions(&registry)
                .into_iter()
                .map(|definition| provider_info(definition, active.as_ref(), verbose))
                .collect()
        };
        Ok(RebornProviderList {
            providers,
            v1_state: RebornV1State::NotUsed,
        })
    }

    pub async fn status(&self) -> Result<RebornProviderStatus, RebornProviderAdminError> {
        let (config, registry) = self.snapshot().await?;
        let active = active_llm_selection(Some(&config), &registry);
        Ok(RebornProviderStatus {
            routes: if active.is_some() {
                RebornModelRoutesState::Configured
            } else {
                RebornModelRoutesState::NotConfigured
            },
            default: active.map(|selection| RebornProviderSelection {
                provider_id: selection.provider_id,
                provider_known: selection.canonical_provider_id.is_some(),
                model: selection.model,
                api_key_env: selection.api_key_env,
                base_url: selection.base_url,
            }),
            v1_state: RebornV1State::NotUsed,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RebornProviderList {
    pub providers: Vec<RebornProviderInfo>,
    pub v1_state: RebornV1State,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RebornProviderInfo {
    pub id: String,
    pub description: String,
    pub default_model: String,
    pub active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<RebornProviderMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RebornProviderMetadata {
    pub aliases: Vec<String>,
    pub protocol: String,
    pub model_env: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    pub api_key_required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_kind: Option<&'static str>,
    pub accepts_api_key: bool,
    pub can_list_models: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RebornProviderStatus {
    pub routes: RebornModelRoutesState,
    pub default: Option<RebornProviderSelection>,
    pub v1_state: RebornV1State,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RebornProviderSelection {
    pub provider_id: Option<String>,
    pub provider_known: bool,
    pub model: Option<String>,
    pub api_key_env: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RebornV1State {
    #[serde(rename = "not-used")]
    NotUsed,
}

impl RebornV1State {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotUsed => "not-used",
        }
    }
}

impl fmt::Display for RebornV1State {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RebornModelRoutesState {
    #[serde(rename = "configured")]
    Configured,
    #[serde(rename = "not-configured")]
    NotConfigured,
}

impl RebornModelRoutesState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Configured => "configured",
            Self::NotConfigured => "not-configured",
        }
    }
}

impl fmt::Display for RebornModelRoutesState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Error)]
pub enum RebornProviderAdminError {
    #[error("load Reborn provider catalog: {reason}")]
    LoadRegistry { reason: String },
    #[error("load Reborn provider/config database: {reason}")]
    LoadDatabase { reason: String },
    #[error("unknown Reborn LLM provider `{provider}`; available providers: {}", known.join(", "))]
    UnknownProvider {
        provider: String,
        known: Vec<String>,
    },
    #[error("{reason}")]
    InvalidRequest { reason: String },
}

#[derive(Debug, Clone)]
struct ActiveLlmSelection {
    provider_id: Option<String>,
    canonical_provider_id: Option<String>,
    model: Option<String>,
    api_key_env: Option<String>,
    base_url: Option<String>,
}

fn active_llm_selection(
    config: Option<&RebornConfigFile>,
    registry: &brassclaw_llm::ProviderRegistry,
) -> Option<ActiveLlmSelection> {
    let selection = config.and_then(RebornConfigFile::default_llm_slot)?;
    Some(active_selection_from_slot(selection, registry))
}

fn active_selection_from_slot(
    selection: &LlmSlotSelection,
    registry: &brassclaw_llm::ProviderRegistry,
) -> ActiveLlmSelection {
    let canonical_provider_id = selection
        .provider_id
        .as_deref()
        .and_then(|provider_id| registry.find(provider_id))
        .map(|def| def.id.clone());
    ActiveLlmSelection {
        provider_id: selection.provider_id.clone(),
        canonical_provider_id,
        model: selection.model.clone(),
        api_key_env: selection.api_key_env.clone(),
        base_url: selection.base_url.clone(),
    }
}

fn unique_provider_definitions(
    registry: &brassclaw_llm::ProviderRegistry,
) -> Vec<&brassclaw_llm::registry::ProviderDefinition> {
    let mut emitted = std::collections::HashSet::new();
    registry
        .all()
        .iter()
        .filter_map(|candidate| {
            let final_def = registry.find(&candidate.id)?;
            if emitted.insert(final_def.id.as_str()) {
                Some(final_def)
            } else {
                None
            }
        })
        .collect()
}

fn known_provider_ids(registry: &brassclaw_llm::ProviderRegistry) -> Vec<String> {
    unique_provider_definitions(registry)
        .into_iter()
        .map(|def| def.id.clone())
        .collect()
}

fn provider_info(
    def: &brassclaw_llm::registry::ProviderDefinition,
    active: Option<&ActiveLlmSelection>,
    verbose: bool,
) -> RebornProviderInfo {
    let active_for_provider = active
        .and_then(|selection| selection.canonical_provider_id.as_deref())
        .is_some_and(|provider_id| provider_id.eq_ignore_ascii_case(&def.id));
    let active_model = active_for_provider.then(|| {
        active
            .and_then(|selection| selection.model.clone())
            .unwrap_or_else(|| def.default_model.clone())
    });
    let resolved_base_url = if active_for_provider {
        active
            .and_then(|s| s.base_url.clone())
            .or_else(|| def.default_base_url.clone())
    } else {
        def.default_base_url.clone()
    };
    RebornProviderInfo {
        id: def.id.clone(),
        description: def.description.clone(),
        default_model: def.default_model.clone(),
        active: active_for_provider,
        active_model,
        metadata: verbose.then(|| RebornProviderMetadata {
            aliases: def.aliases.clone(),
            protocol: provider_protocol_wire_name(def.protocol),
            model_env: def.model_env.clone(),
            api_key_env: def.api_key_env.clone(),
            api_key_required: def.api_key_required,
            base_url: resolved_base_url,
            credential_kind: def.setup.as_ref().map(|setup| setup.kind()),
            accepts_api_key: def.api_key_env.is_some()
                || def
                    .setup
                    .as_ref()
                    .is_some_and(brassclaw_llm::registry::SetupHint::accepts_api_key),
            can_list_models: def
                .setup
                .as_ref()
                .is_some_and(brassclaw_llm::registry::SetupHint::can_list_models),
        }),
    }
}

pub(crate) fn provider_protocol_wire_name(
    protocol: brassclaw_llm::registry::ProviderProtocol,
) -> String {
    serde_json::to_value(protocol)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}
