//! Immutable task-selected provider data, not a provider registry or authority.
//! The existing admission/child owner retains this capture. Preparation reads
//! exact definition history and checks live credentials; it never selects a role
//! again. No credential value has a serialization or diagnostic surface here.
use std::{collections::BTreeMap, sync::Arc};

use brassclaw_llm::{ProviderRole, ProviderSelectionRevision};
use brassclaw_reborn_config::LlmSlotSelection;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use super::{CapturedProviderSelection, RebornProviderAdmin, RebornProviderAdminError};

const FORMAT: &str = "brassclaw-task-providers/1";
const ROLES: [(&str, ProviderRole); 3] = [
    ("kohai", ProviderRole::Kohai),
    ("sempai", ProviderRole::Sempai),
    ("embedding", ProviderRole::Embedding),
];

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TaskProviderSelections {
    format: String,
    namespace: String,
    roles: BTreeMap<String, RoleSelection>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
enum RoleSelection {
    Unconfigured {
        observed_selection_ref: String,
    },
    Unavailable {
        observed_selection_ref: String,
        reason: UnavailableReason,
    },
    Captured {
        binding: Box<SelectedProvider>,
    },
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum UnavailableReason {
    ProviderCatalogue,
    UnknownProvider,
    InvalidSelection,
    CredentialMetadata,
}

#[derive(Clone)]
enum CredentialMetadata {
    Verified(Option<String>),
    Unavailable,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectedProvider {
    observed_selection_ref: String,
    selection: LlmSlotSelection,
    revision: ProviderSelectionRevision,
    definition_checksum: String,
    credential_ref: Option<String>,
}

/// Shared only within the selected task/child family. In particular, a trusted
/// legacy-envelope upgrade may change ciphertext identity without selecting a
/// different credential. Related roles retain the authenticated original and
/// actual live identities, rather than fetching a replacement key by name.
pub(crate) struct TaskCredentialSnapshot {
    namespace: String,
    provider_id: String,
    origin: Option<String>,
    live: Option<String>,
    value: Option<secrecy::SecretString>,
}

pub(crate) struct PreparedTaskProvider {
    pub(crate) captured: CapturedProviderSelection,
    pub(crate) credential: Arc<TaskCredentialSnapshot>,
}

fn unavailable() -> RebornProviderAdminError {
    RebornProviderAdminError::LoadDatabase {
        reason: "retained task provider selection is unavailable".into(),
    }
}

fn reference(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

impl TaskProviderSelections {
    /// Content identity only. Admission/claim provenance is verified by the
    /// existing task owner, not inferred from this digest or JSON object.
    pub(crate) fn checksum(&self) -> Result<String, RebornProviderAdminError> {
        let mut value = serde_json::to_value(self).map_err(|_| unavailable())?;
        value.sort_all_objects();
        let bytes = serde_json::to_vec(&value).map_err(|_| unavailable())?;
        Ok(format!("{:x}", Sha256::digest(bytes)))
    }

    pub(crate) fn validate(&self, namespace: &str) -> Result<(), RebornProviderAdminError> {
        if self.format != FORMAT || self.namespace != namespace || self.roles.len() != ROLES.len() {
            return Err(unavailable());
        }
        for (key, _) in ROLES {
            let role = self.roles.get(key).ok_or_else(unavailable)?;
            let observed = match role {
                RoleSelection::Unconfigured {
                    observed_selection_ref,
                }
                | RoleSelection::Unavailable {
                    observed_selection_ref,
                    ..
                } => observed_selection_ref,
                RoleSelection::Captured { binding } => {
                    if !reference(&binding.definition_checksum)
                        || !reference(&binding.revision.configuration_ref)
                        || binding
                            .credential_ref
                            .as_deref()
                            .is_some_and(|value| !reference(value))
                        || binding.revision.definition_revision == 0
                        || binding.revision.provider_id.is_empty()
                        || binding.selection.provider_id.as_deref()
                            != Some(binding.revision.provider_id.as_str())
                    {
                        return Err(unavailable());
                    }
                    &binding.observed_selection_ref
                }
            };
            if !reference(observed) {
                return Err(unavailable());
            }
        }
        Ok(())
    }

    pub(crate) fn provider_id(&self, role: ProviderRole) -> Option<&str> {
        self.selected(role)
            .ok()
            .map(|selected| selected.revision.provider_id.as_str())
    }

    /// Metadata only, including an original target whose later preparation may
    /// fail. Absence/invalidity never manufactures an effective publication.
    pub(crate) fn role_revision(&self, role: ProviderRole) -> Option<&ProviderSelectionRevision> {
        self.selected(role).ok().map(|selected| &selected.revision)
    }

    pub(crate) fn role_configured(&self, role: &str) -> bool {
        matches!(
            self.roles.get(role),
            Some(RoleSelection::Captured { .. } | RoleSelection::Unavailable { .. })
        )
    }

    pub(crate) fn model_route(
        &self,
    ) -> Result<brassclaw_turns::run_profile::LoopModelRouteSnapshot, RebornProviderAdminError>
    {
        self.role_model_route(ProviderRole::Kohai)
    }

    pub(crate) fn role_model_route(
        &self,
        role: ProviderRole,
    ) -> Result<brassclaw_turns::run_profile::LoopModelRouteSnapshot, RebornProviderAdminError>
    {
        let selected = self.selected(role)?;
        brassclaw_turns::run_profile::LoopModelRouteSnapshot::try_new(
            &selected.revision.provider_id,
            &selected.revision.model,
            &selected.revision.configuration_ref,
            selected
                .credential_ref
                .as_deref()
                .unwrap_or("no-credential"),
        )
        .map_err(|_| unavailable())
    }

    fn selected(&self, role: ProviderRole) -> Result<&SelectedProvider, RebornProviderAdminError> {
        let key = ROLES
            .iter()
            .find(|(_, candidate)| *candidate == role)
            .map(|(key, _)| *key)
            .ok_or_else(unavailable)?;
        match self.roles.get(key) {
            Some(RoleSelection::Captured { binding }) => Ok(binding),
            // Captured absence/unavailability stays absent. No live role lookup.
            _ => Err(unavailable()),
        }
    }
}

impl RebornProviderAdmin {
    /// Capture optional roles without model/network work, leases, row locks or
    /// credential migration. Bad optional provider data is explicit task data;
    /// it does not prevent independent deterministic work from being admitted.
    pub(crate) async fn capture_task_provider_selections(
        &self,
        keys: &crate::LlmKeyStore,
    ) -> Result<TaskProviderSelections, RebornProviderAdminError> {
        let mut client = self.pool.get().await.map_err(|_| unavailable())?;
        let transaction = tokio_postgres::Client::build_transaction(&mut client)
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(|_| unavailable())?;
        let config =
            crate::db_config::load_config_snapshot_from_client(&transaction, &self.tenant_id)
                .await
                .map_err(|_| unavailable())?;
        let observed = crate::db_config::model_selection_refs(&transaction, &self.tenant_id)
            .await
            .map_err(|_| unavailable())?;
        let repo = crate::PgProviderRepo::new(self.pool.clone(), &self.tenant_id);
        let definitions = match repo.load_active_revisions_from_client(&transaction).await {
            Ok(rows) => Some(rows),
            Err(crate::pg_provider_repo::PgProviderRepoError::Parse { .. }) => None,
            Err(_) => return Err(unavailable()),
        };
        let registry = definitions.as_ref().and_then(|rows| {
            super::validated_registry(
                rows.iter()
                    .filter(|row| !row.inactive)
                    .map(|row| row.definition.clone())
                    .collect(),
            )
            .ok()
        });
        let valid = registry
            .as_ref()
            .is_some_and(|registry| crate::llm_catalog::validate_catalog(registry).is_ok());
        let mut roles = BTreeMap::new();
        let mut credentials: BTreeMap<String, CredentialMetadata> = BTreeMap::new();
        for (key, role) in ROLES {
            let observed_selection_ref = observed.get(key).cloned().ok_or_else(unavailable)?;
            let selection = super::stored_role_selection(&config, role).filter(|selection| {
                selection
                    .provider_id
                    .as_deref()
                    .is_some_and(|id| !id.is_empty())
            });
            let Some(mut selection) = selection else {
                roles.insert(
                    key.into(),
                    RoleSelection::Unconfigured {
                        observed_selection_ref,
                    },
                );
                continue;
            };
            let reason = if !valid {
                Some(UnavailableReason::ProviderCatalogue)
            } else {
                None
            };
            if let Some(reason) = reason {
                roles.insert(
                    key.into(),
                    RoleSelection::Unavailable {
                        observed_selection_ref,
                        reason,
                    },
                );
                continue;
            }
            let definition = registry.as_ref().and_then(|registry| {
                registry.find(selection.provider_id.as_deref().unwrap_or_default())
            });
            let Some(definition) = definition else {
                roles.insert(
                    key.into(),
                    RoleSelection::Unavailable {
                        observed_selection_ref,
                        reason: UnavailableReason::UnknownProvider,
                    },
                );
                continue;
            };
            if crate::llm_catalog::validate_selection(&selection, definition, 0).is_err() {
                roles.insert(
                    key.into(),
                    RoleSelection::Unavailable {
                        observed_selection_ref,
                        reason: UnavailableReason::InvalidSelection,
                    },
                );
                continue;
            }
            let stored = definitions
                .as_ref()
                .and_then(|rows| {
                    rows.iter()
                        .find(|row| row.definition.id == definition.id && !row.inactive)
                })
                .ok_or_else(unavailable)?;
            selection.provider_id = Some(definition.id.clone());
            if !credentials.contains_key(&definition.id) {
                let inspected = match keys
                    .inspect_usage_credential_reference_in_transaction(&transaction, definition)
                    .await
                {
                    Ok(reference) => CredentialMetadata::Verified(reference),
                    Err(_) => CredentialMetadata::Unavailable,
                };
                credentials.insert(definition.id.clone(), inspected);
            }
            let CredentialMetadata::Verified(credential_ref) = credentials[&definition.id].clone()
            else {
                roles.insert(
                    key.into(),
                    RoleSelection::Unavailable {
                        observed_selection_ref,
                        reason: UnavailableReason::CredentialMetadata,
                    },
                );
                continue;
            };
            let revision = super::selection_revision(
                &self.tenant_id,
                &selection,
                definition,
                stored.revision,
                credential_ref.as_deref(),
            )
            .map_err(|_| unavailable())?;
            roles.insert(
                key.into(),
                RoleSelection::Captured {
                    binding: Box::new(SelectedProvider {
                        observed_selection_ref,
                        selection,
                        revision,
                        definition_checksum: stored.definition_checksum.clone(),
                        credential_ref,
                    }),
                },
            );
        }
        transaction.commit().await.map_err(|_| unavailable())?;
        let snapshot = TaskProviderSelections {
            format: FORMAT.into(),
            namespace: self.tenant_id.clone(),
            roles,
        };
        snapshot.validate(&self.tenant_id)?;
        Ok(snapshot)
    }

    /// Resolve one original role from immutable history. Shared credentials may
    /// come only from an authenticated earlier resolution of this same capture.
    /// A replacement or absence is never repaired by reading a live role.
    pub(crate) async fn prepare_task_provider(
        &self,
        snapshot: &TaskProviderSelections,
        role: ProviderRole,
        keys: &crate::LlmKeyStore,
        credential: Option<Arc<TaskCredentialSnapshot>>,
    ) -> Result<PreparedTaskProvider, RebornProviderAdminError> {
        snapshot.validate(&self.tenant_id)?;
        let binding = snapshot.selected(role)?;
        let mut client = self.pool.get().await.map_err(|_| unavailable())?;
        let transaction = tokio_postgres::Client::build_transaction(&mut client)
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .start()
            .await
            .map_err(|_| unavailable())?;
        let repo = crate::PgProviderRepo::new(self.pool.clone(), &self.tenant_id);
        let retained = repo
            .read_retained_revision(
                &transaction,
                &binding.revision.provider_id,
                binding.revision.definition_revision,
            )
            .await
            .map_err(|_| unavailable())?
            .ok_or_else(unavailable)?;
        if retained.definition_checksum != binding.definition_checksum
            || retained.inactive
            || super::selection_revision(
                &self.tenant_id,
                &binding.selection,
                &retained.definition,
                retained.revision,
                binding.credential_ref.as_deref(),
            )
            .map_err(|_| unavailable())?
                != binding.revision
            || repo
                .get_revision_from_client(&transaction, &binding.revision.provider_id, false)
                .await
                .map_err(|_| unavailable())?
                .is_none()
        {
            return Err(unavailable());
        }
        crate::llm_catalog::validate_catalog(&brassclaw_llm::ProviderRegistry::new(vec![
            retained.definition.clone(),
        ]))
        .map_err(|_| unavailable())?;
        crate::llm_catalog::validate_selection(&binding.selection, &retained.definition, 0)
            .map_err(|_| unavailable())?;
        let observed = keys
            .inspect_usage_credential_reference_in_transaction(&transaction, &retained.definition)
            .await
            .map_err(|_| unavailable())?;
        let credential = match credential {
            Some(credential)
                if credential.namespace == self.tenant_id
                    && credential.provider_id == binding.revision.provider_id
                    && credential.origin == binding.credential_ref
                    && credential.live == observed =>
            {
                credential
            }
            Some(_) => return Err(unavailable()),
            None => {
                if observed != binding.credential_ref {
                    return Err(unavailable());
                }
                let value = if observed.is_some() {
                    keys.read_usage_credential_in_transaction(&transaction, &retained.definition)
                        .await
                        .map_err(|_| unavailable())?
                } else {
                    None
                };
                let live = keys
                    .inspect_usage_credential_reference_in_transaction(
                        &transaction,
                        &retained.definition,
                    )
                    .await
                    .map_err(|_| unavailable())?;
                Arc::new(TaskCredentialSnapshot {
                    namespace: self.tenant_id.clone(),
                    provider_id: binding.revision.provider_id.clone(),
                    origin: binding.credential_ref.clone(),
                    live,
                    value,
                })
            }
        };
        let revision = super::selection_revision(
            &self.tenant_id,
            &binding.selection,
            &retained.definition,
            retained.revision,
            credential.live.as_deref(),
        )
        .map_err(|_| unavailable())?;
        transaction.commit().await.map_err(|_| unavailable())?;
        Ok(PreparedTaskProvider {
            captured: CapturedProviderSelection {
                selection: binding.selection.clone(),
                definition: retained.definition,
                credential: credential.value.clone(),
                revision,
                credential_ref: credential.live.clone(),
            },
            credential,
        })
    }
}
