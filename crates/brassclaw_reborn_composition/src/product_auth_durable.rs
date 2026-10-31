use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};

use futures::{StreamExt as _, TryStreamExt as _, stream};

use brassclaw_filesystem::{
    CasExpectation, ContentType, Entry, EntryBodyChecksum, FileType, FilesystemError,
    RecordVersion, RootFilesystem, ScopedFilesystem,
};
use brassclaw_host_api::{ResourceScope, ScopedPath};
use brassclaw_secrets::SecretStore;
use chrono::Utc;
use serde::{Serialize, de::DeserializeOwned};

use brassclaw_auth::{
    AuthFlowId, AuthFlowOwnerScope, AuthFlowRecord, AuthProductError, AuthSessionId, AuthSurface,
    CredentialAccount, CredentialAccountId, CredentialAccountOwnerScope,
    CredentialAccountSelectionRequest, CredentialAccountStatus, NewCredentialAccount,
};

use self::domain::validate_new_credential_account;
use self::paths::{
    account_path, account_root, flow_path, flow_root, fs_error, join_scoped, surface_sessions_root,
};

mod accounts;
mod callback;
mod cleanup;
mod dcr;
mod domain;
mod flows;
mod interactions;
mod manual;
mod manual_setup;
mod oauth_setup;
mod paths;
mod provider;
mod refresh;
mod requirement;
mod selection;
#[cfg(test)]
mod tests;

/// Exact entry identity captured together, never reconstructed from a later read.
/// Auth metadata contains handles only; this is not a secret-value fingerprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AuthRecordSnapshot {
    version: RecordVersion,
    body_checksum: EntryBodyChecksum,
}

impl AuthRecordSnapshot {
    fn cas(self) -> CasExpectation {
        CasExpectation::VersionAndBody {
            version: self.version,
            body_checksum: self.body_checksum,
        }
    }
}

pub(crate) use provider::UnavailableAuthProviderClient;

/// Durable production implementation of the product-auth ports.
///
/// Records live under the caller's scoped `/secrets/product-auth` tree. Raw
/// provider tokens and manual token values are stored only through
/// [`SecretStore`] and represented here by opaque secret handles.
// Product-auth UX metadata and runtime broker grants have different contracts.
// The existing ProductAuthRuntimeCredentialResolver selects authenticated account
// metadata for host-mediated staging; a copied row or snapshot hash is no grant.
// Keep dedicated original-grant/expiry/final-use migration in Simplified K3's
// existing owner instead of reviving the retired projection-plan placeholder.
pub(crate) struct FilesystemAuthProductServices<F>
where
    F: RootFilesystem,
{
    filesystem: Arc<ScopedFilesystem<F>>,
    secret_store: Arc<dyn SecretStore>,
    locks: Mutex<HashMap<String, Weak<tokio::sync::Mutex<()>>>>,
}

impl<F> FilesystemAuthProductServices<F>
where
    F: RootFilesystem,
{
    pub(crate) fn new(
        filesystem: Arc<ScopedFilesystem<F>>,
        secret_store: Arc<dyn SecretStore>,
    ) -> Self {
        Self {
            filesystem,
            secret_store,
            locks: Mutex::new(HashMap::new()),
        }
    }

    fn lock_for(&self, key: String) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self
            .locks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        locks.retain(|_, lock| lock.strong_count() > 0);
        if let Some(lock) = locks.get(&key).and_then(Weak::upgrade) {
            return lock;
        }
        let lock = Arc::new(tokio::sync::Mutex::new(()));
        locks.insert(key, Arc::downgrade(&lock));
        lock
    }

    async fn read_record<T>(
        &self,
        scope: &ResourceScope,
        path: &ScopedPath,
    ) -> Result<Option<(T, AuthRecordSnapshot)>, AuthProductError>
    where
        T: DeserializeOwned,
    {
        let Some(versioned) = self.filesystem.get(scope, path).await.map_err(fs_error)? else {
            return Ok(None);
        };
        let value = serde_json::from_slice(&versioned.entry.body)
            .map_err(|_| AuthProductError::BackendUnavailable)?;
        Ok(Some((
            value,
            AuthRecordSnapshot {
                version: versioned.version,
                body_checksum: EntryBodyChecksum::for_body(&versioned.entry.body),
            },
        )))
    }

    async fn read_account_record_for_scan(
        &self,
        scope: &ResourceScope,
        path: &ScopedPath,
    ) -> Result<Option<CredentialAccount>, AuthProductError> {
        let Some(versioned) = self.filesystem.get(scope, path).await.map_err(fs_error)? else {
            return Ok(None);
        };
        serde_json::from_slice(&versioned.entry.body)
            .map(Some)
            .map_err(|_| AuthProductError::BackendUnavailable)
    }

    async fn write_record<T>(
        &self,
        scope: &ResourceScope,
        path: &ScopedPath,
        value: &T,
        cas: CasExpectation,
    ) -> Result<AuthRecordSnapshot, AuthProductError>
    where
        T: Serialize,
    {
        if matches!(cas, CasExpectation::Any | CasExpectation::Version(_)) {
            return Err(AuthProductError::BackendConflict);
        }
        let body = serde_json::to_vec(value).map_err(|_| AuthProductError::BackendUnavailable)?;
        let body_checksum = EntryBodyChecksum::for_body(&body);
        let entry = Entry::bytes(body).with_content_type(ContentType::json());
        let version = self
            .filesystem
            .put(scope, path, entry, cas)
            .await
            .map_err(fs_error)?;
        Ok(AuthRecordSnapshot {
            version,
            body_checksum,
        })
    }

    async fn read_flow_document(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        flow_id: AuthFlowId,
    ) -> Result<
        Option<(crate::auth_flow_custody::StoredAuthFlow, AuthRecordSnapshot)>,
        AuthProductError,
    > {
        let document: Option<(crate::auth_flow_custody::StoredAuthFlow, AuthRecordSnapshot)> = self
            .read_record(&scope.resource, &flow_path(scope, flow_id))
            .await?;
        if let Some((document, _)) = &document
            && (!scope_matches(scope, &document.flow.scope) || document.flow.id != flow_id)
        {
            return Err(AuthProductError::CrossScopeDenied);
        }
        Ok(document)
    }
    async fn read_flow(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        flow_id: AuthFlowId,
    ) -> Result<Option<(AuthFlowRecord, AuthRecordSnapshot)>, AuthProductError> {
        Ok(self
            .read_flow_document(scope, flow_id)
            .await?
            .map(|(doc, version)| (doc.flow, version)))
    }
    async fn write_flow_document(
        &self,
        document: &crate::auth_flow_custody::StoredAuthFlow,
        cas: CasExpectation,
    ) -> Result<AuthRecordSnapshot, AuthProductError> {
        self.write_record(
            &document.flow.scope.resource,
            &flow_path(&document.flow.scope, document.flow.id),
            document,
            cas,
        )
        .await
    }
    async fn write_new_flow(
        &self,
        flow: &AuthFlowRecord,
        snapshot: Option<CredentialAccount>,
        setup: Option<Box<brassclaw_auth::CredentialRequirementSetup>>,
    ) -> Result<AuthRecordSnapshot, AuthProductError> {
        let document = crate::auth_flow_custody::StoredAuthFlow::new(flow.clone(), snapshot)?
            .with_requirement_setup(setup)?;
        self.write_flow_document(&document, CasExpectation::Absent)
            .await
    }
    async fn write_flow(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        flow: &AuthFlowRecord,
        cas: CasExpectation,
    ) -> Result<AuthRecordSnapshot, AuthProductError> {
        if !scope_matches(scope, &flow.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let custody = match cas {
            CasExpectation::VersionAndBody { .. } => {
                let (document, version) = self
                    .read_flow_document(scope, flow.id)
                    .await?
                    .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
                if version.cas() != cas || document.fs_callback_publication.is_some() {
                    return Err(AuthProductError::BackendConflict);
                }
                document.auth_custody
            }
            CasExpectation::Absent => {
                if flow.update_binding.is_some() {
                    return Err(AuthProductError::BackendConflict);
                }
                brassclaw_auth::AuthFlowCustody::default()
            }
            CasExpectation::Any | CasExpectation::Version(_) => {
                return Err(AuthProductError::BackendConflict);
            }
        };
        self.write_flow_document(
            &crate::auth_flow_custody::StoredAuthFlow {
                flow: flow.clone(),
                auth_custody: custody,
                fs_callback_publication: None,
            },
            cas,
        )
        .await
    }

    async fn flows_for_scope(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
    ) -> Result<Vec<(AuthFlowRecord, AuthRecordSnapshot)>, AuthProductError> {
        let mut flows = self.flow_records_under_scope_root(scope).await?;
        flows.retain(|(flow, _)| scope_matches(scope, &flow.scope));
        flows.sort_by_key(|(flow, _)| flow.id);
        Ok(flows)
    }

    async fn flow_records_under_scope_root(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
    ) -> Result<Vec<(AuthFlowRecord, AuthRecordSnapshot)>, AuthProductError> {
        let root = flow_root(scope);
        let entries = match self.filesystem.list_dir(&scope.resource, &root).await {
            Ok(entries) => entries,
            Err(FilesystemError::NotFound { .. }) => return Ok(Vec::new()),
            Err(error) => return Err(fs_error(error)),
        };
        const MAX_CONCURRENT_READS: usize = 16;
        let mut flows: Vec<(AuthFlowRecord, AuthRecordSnapshot)> = stream::iter(
            entries
                .into_iter()
                .filter(|e| e.name.ends_with(".json"))
                .map(|entry| {
                    let path = join_scoped(&root, &entry.name);
                    async move {
                        let path = path?;
                        self.read_record::<AuthFlowRecord>(&scope.resource, &path)
                            .await
                    }
                }),
        )
        .buffer_unordered(MAX_CONCURRENT_READS)
        .try_collect::<Vec<_>>()
        .await?
        .into_iter()
        .flatten()
        .collect();
        flows.sort_by_key(|(flow, _)| flow.id);
        Ok(flows)
    }

    async fn flow_records_for_owner(
        &self,
        owner: &AuthFlowOwnerScope,
    ) -> Result<Vec<AuthFlowRecord>, AuthProductError> {
        let resource = ResourceScope {
            tenant_id: owner.tenant_id.clone(),
            user_id: owner.user_id.clone(),
            agent_id: owner.agent_id.clone(),
            project_id: owner.project_id.clone(),
            thread_id: Some(owner.thread_id.clone()),
            invocation_id: brassclaw_host_api::InvocationId::new(),
        };
        let mut flows = Vec::new();
        for surface in AuthSurface::ALL {
            let scope = brassclaw_auth::AuthProductScope::new(resource.clone(), surface);
            flows.extend(
                self.flow_records_under_scope_root(&scope)
                    .await?
                    .into_iter()
                    .map(|(flow, _)| flow)
                    .filter(|flow| owner.matches(flow)),
            );
            let sessions_root = surface_sessions_root(&resource, surface);
            let mut entries = match self.filesystem.list_dir(&resource, &sessions_root).await {
                Ok(entries) => entries,
                Err(FilesystemError::NotFound { .. }) => continue,
                Err(error) => return Err(fs_error(error)),
            };
            entries.sort_by(|left, right| left.name.cmp(&right.name));
            for entry in entries {
                if entry.file_type != FileType::Directory {
                    continue;
                }
                let Ok(session_id) = AuthSessionId::new(entry.name) else {
                    continue;
                };
                let mut session_scope =
                    brassclaw_auth::AuthProductScope::new(resource.clone(), surface);
                session_scope.session_id = Some(session_id);
                flows.extend(
                    self.flow_records_under_scope_root(&session_scope)
                        .await?
                        .into_iter()
                        .map(|(flow, _)| flow)
                        .filter(|flow| owner.matches(flow)),
                );
            }
        }
        flows.sort_by_key(|flow| flow.id);
        flows.dedup_by_key(|flow| flow.id);
        Ok(flows)
    }

    async fn read_account(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        account_id: CredentialAccountId,
    ) -> Result<Option<(CredentialAccount, AuthRecordSnapshot)>, AuthProductError> {
        Ok(self
            .read_account_document(scope, account_id)
            .await?
            .map(|(document, version)| (document.account, version)))
    }

    async fn write_account(
        &self,
        account: &CredentialAccount,
        cas: CasExpectation,
    ) -> Result<AuthRecordSnapshot, AuthProductError> {
        let (refresh_actions, callback_actions, manual_actions, lifecycle_cleanup) = match cas {
            CasExpectation::Absent => (
                std::collections::BTreeMap::new(),
                std::collections::BTreeMap::new(),
                std::collections::BTreeMap::new(),
                Vec::new(),
            ),
            CasExpectation::VersionAndBody { .. } => {
                let (document, version) = self
                    .read_account_document(&account.scope, account.id)
                    .await?
                    .ok_or(AuthProductError::CredentialMissing)?;
                if version.cas() != cas {
                    return Err(AuthProductError::BackendConflict);
                }
                (
                    document.auth_refresh_actions,
                    document.auth_callback_actions,
                    document.auth_manual_actions,
                    document.auth_lifecycle_cleanup,
                )
            }
            CasExpectation::Any | CasExpectation::Version(_) => {
                return Err(AuthProductError::InvalidRequest {
                    reason: "auth account mutation requires absence or the original entry snapshot"
                        .into(),
                });
            }
        };
        self.write_account_document(
            &refresh::StoredCredentialAccount {
                account: account.clone(),
                auth_refresh_actions: refresh_actions,
                auth_callback_actions: callback_actions,
                auth_manual_actions: manual_actions,
                auth_lifecycle_cleanup: lifecycle_cleanup,
            },
            cas,
        )
        .await
    }

    /// Returns all credential accounts for `scope`, reading records concurrently.
    async fn accounts_for_scope(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
    ) -> Result<Vec<CredentialAccount>, AuthProductError> {
        let mut accounts = self
            .account_records_under_scope_root(scope)
            .await?
            .into_iter()
            .filter(|account| scope_matches(scope, &account.scope))
            .collect::<Vec<_>>();
        accounts.sort_by_key(|account| account.id);
        Ok(accounts)
    }

    /// Returns all credential accounts stored under `scope`'s durable root.
    ///
    /// Normal product-auth lookups still apply exact `AuthProductScope`
    /// filtering through `accounts_for_scope`; runtime credential selection uses
    /// this lower-level scan because setup and runtime invocations necessarily
    /// carry different invocation ids.
    async fn account_records_under_scope_root(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
    ) -> Result<Vec<CredentialAccount>, AuthProductError> {
        self.account_records_under_scope_root_with_limit(scope, None)
            .await
    }

    async fn account_records_under_scope_root_with_limit(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        max_records: Option<usize>,
    ) -> Result<Vec<CredentialAccount>, AuthProductError> {
        let root = account_root(scope);
        let entries = match max_records {
            Some(max_records) => {
                self.filesystem
                    .list_dir_bounded(&scope.resource, &root, max_records.saturating_add(1))
                    .await
            }
            None => self.filesystem.list_dir(&scope.resource, &root).await,
        };
        let entries = match entries {
            Ok(entries) => entries,
            Err(FilesystemError::NotFound { .. }) => return Ok(Vec::new()),
            Err(error) => return Err(fs_error(error)),
        };
        if max_records.is_some_and(|max_records| entries.len() > max_records) {
            return Err(AuthProductError::BackendUnavailable);
        }
        // Read records concurrently, capped at 16 in-flight ops to avoid
        // exhausting file-descriptor or connection limits on large scopes.
        const MAX_CONCURRENT_READS: usize = 16;
        let mut accounts: Vec<CredentialAccount> = stream::iter(
            entries
                .into_iter()
                .filter(|e| e.name.ends_with(".json"))
                .map(|entry| {
                    let path = join_scoped(&root, &entry.name);
                    async move {
                        let path = path?;
                        self.read_account_record_for_scan(&scope.resource, &path)
                            .await
                    }
                }),
        )
        .buffer_unordered(MAX_CONCURRENT_READS)
        .try_collect::<Vec<_>>()
        .await?
        .into_iter()
        .flatten()
        .collect();
        accounts.sort_by_key(|account| account.id);
        Ok(accounts)
    }

    async fn account_scopes_for_owner(
        &self,
        owner: &CredentialAccountOwnerScope,
    ) -> Result<Vec<brassclaw_auth::AuthProductScope>, AuthProductError> {
        let resource = ResourceScope {
            tenant_id: owner.tenant_id.clone(),
            user_id: owner.user_id.clone(),
            agent_id: owner.agent_id.clone(),
            project_id: owner.project_id.clone(),
            thread_id: owner.thread_id.clone(),
            invocation_id: brassclaw_host_api::InvocationId::new(),
        };
        let mut scopes = Vec::new();
        for surface in AuthSurface::ALL {
            scopes.push(brassclaw_auth::AuthProductScope::new(
                resource.clone(),
                surface,
            ));
            if let Some(session_id) = &owner.session_id {
                scopes.push(
                    brassclaw_auth::AuthProductScope::new(resource.clone(), surface)
                        .with_session_id(session_id.clone()),
                );
                continue;
            }
            let sessions_root = surface_sessions_root(&resource, surface);
            let mut entries = match self.filesystem.list_dir(&resource, &sessions_root).await {
                Ok(entries) => entries,
                Err(FilesystemError::NotFound { .. }) => continue,
                Err(error) => return Err(fs_error(error)),
            };
            entries.sort_by(|left, right| left.name.cmp(&right.name));
            for entry in entries {
                if entry.file_type != FileType::Directory {
                    continue;
                }
                let Ok(session_id) = AuthSessionId::new(entry.name) else {
                    continue;
                };
                scopes.push(
                    brassclaw_auth::AuthProductScope::new(resource.clone(), surface)
                        .with_session_id(session_id),
                );
            }
        }
        Ok(scopes)
    }

    async fn account_records_for_owner(
        &self,
        owner: &CredentialAccountOwnerScope,
    ) -> Result<Vec<CredentialAccount>, AuthProductError> {
        let mut accounts = Vec::new();
        for scope in self.account_scopes_for_owner(owner).await? {
            accounts.extend(
                self.account_records_under_scope_root_with_limit(&scope, None)
                    .await?
                    .into_iter()
                    .filter(|account| owner.matches(account)),
            );
        }
        accounts.sort_by_key(|account| account.id);
        if accounts.windows(2).any(|pair| pair[0].id == pair[1].id) {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(accounts)
    }

    async fn select_configured_account_for_owner(
        &self,
        request: CredentialAccountSelectionRequest,
    ) -> Result<CredentialAccount, AuthProductError> {
        let owner = CredentialAccountOwnerScope::from_scope(&request.scope);
        let mut saw_configured = false;
        let mut selected = None;
        for scope in self.account_scopes_for_owner(&owner).await? {
            for account in self
                .account_records_under_scope_root_with_limit(&scope, None)
                .await?
            {
                if !owner.matches(&account)
                    || account.provider != request.provider
                    || account.effective_status() != CredentialAccountStatus::Configured
                {
                    continue;
                }
                saw_configured = true;
                if !account.is_authorized_for_requester(request.requester_extension.as_ref()) {
                    continue;
                }
                if selected.is_some() {
                    return Err(AuthProductError::AccountSelectionRequired);
                }
                selected = Some(account);
            }
        }
        match (selected, saw_configured) {
            (Some(account), _) => Ok(account),
            (None, true) => Err(AuthProductError::CrossScopeDenied),
            (None, false) => Err(AuthProductError::CredentialMissing),
        }
    }

    async fn create_account_with_id(
        &self,
        account_id: CredentialAccountId,
        request: NewCredentialAccount,
        cas: CasExpectation,
    ) -> Result<CredentialAccount, AuthProductError> {
        validate_new_credential_account(&request)?;
        let now = Utc::now();
        let account = CredentialAccount {
            id: account_id,
            scope: request.scope,
            provider: request.provider,
            label: request.label,
            status: request.status,
            ownership: request.ownership,
            owner_extension: request.owner_extension,
            granted_extensions: request.granted_extensions,
            access_secret: request.access_secret,
            access_expires_at: request.access_expires_at,
            refresh_secret: request.refresh_secret,
            scopes: request.scopes,
            created_at: now,
            updated_at: now,
        };
        self.write_account(&account, cas).await?;
        Ok(account)
    }
}

use brassclaw_auth::{credential_status_for_completed_flow, is_terminal_status, scope_matches};
