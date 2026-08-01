//! Reborn provider-admin facade.
//!
//! This is the read side of the provider/model administration surface shared
//! by the standalone CLI and product command workflow. It reads the boot
//! `config.toml` for the current Kohai slot selection and the shared provider
//! catalog through `brassclaw_llm`. Writes go through the DB-backed
//! `PgProviderRepo` / `db_config::save_config_key` in the composition root.

use std::{fmt, path::PathBuf};

use brassclaw_reborn_config::{LlmSlotSelection, RebornBootConfig, RebornConfigFile};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct RebornProviderAdmin {
    boot: RebornBootConfig,
}

impl RebornProviderAdmin {
    pub fn new(boot: RebornBootConfig) -> Self {
        Self { boot }
    }

    pub fn list(
        &self,
        provider: Option<&str>,
        verbose: bool,
    ) -> Result<RebornProviderList, RebornProviderAdminError> {
        let home = self.boot.home();
        let config_path = home.path().join("config.toml");
        let registry = self.load_registry()?;
        let config = RebornConfigFile::load(&config_path).map_err(|source| {
            RebornProviderAdminError::LoadConfig {
                path: config_path.clone(),
                source: Box::new(source),
            }
        })?;
        let active = active_llm_selection(config.as_ref(), &registry);

        let providers = if let Some(provider) = provider {
            let def = registry.find(provider).ok_or_else(|| {
                RebornProviderAdminError::UnknownProvider {
                    provider: provider.to_string(),
                    known: known_provider_ids(&registry),
                }
            })?;
            vec![provider_info(def, active.as_ref(), true)]
        } else {
            unique_provider_definitions(&registry)
                .into_iter()
                .map(|def| provider_info(def, active.as_ref(), verbose))
                .collect()
        };

        Ok(RebornProviderList {
            providers,
            v1_state: RebornV1State::NotUsed,
        })
    }

    pub fn status(&self) -> Result<RebornProviderStatus, RebornProviderAdminError> {
        let home = self.boot.home();
        let config_path = home.path().join("config.toml");
        let registry = self.load_registry()?;
        let config = RebornConfigFile::load(&config_path).map_err(|source| {
            RebornProviderAdminError::LoadConfig {
                path: config_path.clone(),
                source: Box::new(source),
            }
        })?;
        let active = active_llm_selection(config.as_ref(), &registry);
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

    fn load_registry(&self) -> Result<brassclaw_llm::ProviderRegistry, RebornProviderAdminError> {
        // Built-ins only; custom providers are now in brassclaw_llm_providers (DB).
        brassclaw_llm::ProviderRegistry::try_load_from_path(None).map_err(|error| {
            RebornProviderAdminError::LoadRegistry {
                reason: error.to_string(),
            }
        })
    }

    /// DB-backed variant of `list()`.
    ///
    /// Reads all active providers (builtin + custom) from `brassclaw_llm_providers`
    /// via `pg_repo.load_all()`.  Used by the CLI `models list` command when a
    /// Postgres pool is available.  The sync `list()` remains for offline use.
    #[cfg(feature = "postgres")]
    pub async fn list_from_db(
        &self,
        pg_repo: &crate::pg_provider_repo::PgProviderRepo,
        provider: Option<&str>,
        verbose: bool,
    ) -> Result<RebornProviderList, RebornProviderAdminError> {
        use brassclaw_llm::registry::ProviderDefinition;

        let all = pg_repo
            .load_all()
            .await
            .map_err(|e| RebornProviderAdminError::LoadRegistry {
                reason: e.to_string(),
            })?;

        // Read the active Kohai selection from config.toml as a fallback for
        // CLi context where DB config may not yet be loaded.  Errors are ignored
        // — the CLI can still list providers without an active selection.
        let home = self.boot.home();
        let config_path = home.path().join("config.toml");
        let config = RebornConfigFile::load(&config_path).ok().flatten();
        // Build a minimal registry from DB rows to pass to active_llm_selection.
        let db_defs: Vec<ProviderDefinition> = all.iter().map(|(def, _)| def.clone()).collect();
        let registry = brassclaw_llm::ProviderRegistry::new(db_defs);
        let active = active_llm_selection(config.as_ref(), &registry);

        let providers: Vec<RebornProviderInfo> = if let Some(provider_id) = provider {
            let Some((def, _)) = all.iter().find(|(d, _)| d.id == provider_id) else {
                return Err(RebornProviderAdminError::UnknownProvider {
                    provider: provider_id.to_string(),
                    known: all.iter().map(|(d, _)| d.id.clone()).collect(),
                });
            };
            vec![provider_info(def, active.as_ref(), true)]
        } else {
            all.iter()
                .map(|(def, _)| provider_info(def, active.as_ref(), verbose))
                .collect()
        };

        Ok(RebornProviderList {
            providers,
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
    #[error("load Reborn config `{}`: {source}", path.display())]
    LoadConfig {
        path: PathBuf,
        source: Box<brassclaw_reborn_config::RebornConfigFileError>,
    },
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

pub(crate) fn provider_protocol_wire_name(protocol: brassclaw_llm::registry::ProviderProtocol) -> String {
    serde_json::to_value(protocol)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}
