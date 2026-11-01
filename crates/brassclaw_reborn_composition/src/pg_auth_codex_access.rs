#![allow(dead_code)] // PRIVATE036: staged access APIs; callers land with the full login/refresh/startup wiring.
//! PRIVATE036: existing native Auth owner, request-local access consumption.
//! Unmounted/uncompiled until login, refresh, all callers and native cases join.
use super::PgAuthProductServices;
use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_auth::{CredentialAccount, CredentialAccountSnapshotRef, CredentialAccountStatus};
use brassclaw_host_api::SecretHandle;
use brassclaw_llm::{
    CodexHostSession, LlmError, ProviderDispatchGuard, ProviderDispatchState, ProviderRequestFacts,
};
use chrono::{DateTime, Utc};

use crate::provider_codex_credentials::{self, AccountRead};

fn changed() -> LlmError {
    LlmError::ConfigurationChanged {
        provider: "native Codex credential".into(),
    }
}

fn require_fresh_account(account: &CredentialAccount) -> Result<(), LlmError> {
    match account.effective_status() {
        CredentialAccountStatus::Configured => Ok(()),
        CredentialAccountStatus::Expired => Err(LlmError::SessionExpired {
            provider: "openai_codex".into(),
        }),
        _ => Err(LlmError::AuthFailed {
            provider: "openai_codex".into(),
        }),
    }
}

/// Only the original token consumer constructs this immutable metadata. No
/// shared mutable credential pointer, refresh permission or secret value lives
/// here. Its account digest and encrypted reference bind the returned token.
struct SelectedAccess {
    namespace: String,
    provider_id: String,
    head_reference: String,
    api_base_url: String,
    account_reference: CredentialAccountSnapshotRef,
    access_handle: SecretHandle,
    access_reference: String,
    account_expires_at: Option<DateTime<Utc>>,
    secret_expires_at: Option<DateTime<Utc>>,
}

struct NativeCodexAccessGuard {
    owner: Arc<PgAuthProductServices>,
    selected: SelectedAccess,
    provider_guard: Arc<dyn ProviderDispatchGuard>,
}

impl std::fmt::Debug for NativeCodexAccessGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeCodexAccessGuard")
            .finish_non_exhaustive()
    }
}

impl NativeCodexAccessGuard {
    fn require_unexpired(&self) -> Result<(), LlmError> {
        if [
            self.selected.account_expires_at,
            self.selected.secret_expires_at,
        ]
        .into_iter()
        .flatten()
        .any(|expiry| expiry <= Utc::now())
        {
            return Err(changed());
        }
        Ok(())
    }

    async fn verify_selected_access(&self) -> Result<(), LlmError> {
        // The encrypted generation includes the secret expiry. A matching
        // reference keeps this captured expiry meaningful without decrypting
        // token material again or issuing another lease during final dispatch.
        self.require_unexpired()?;
        let mut client = self.owner.pool.get().await.map_err(|_| changed())?;
        let transaction = tokio_postgres::Client::build_transaction(&mut client)
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(|_| changed())?;
        let current = provider_codex_credentials::current_access(
            &transaction,
            &self.owner.secret_store,
            &self.selected.namespace,
            &self.selected.provider_id,
            &self.selected.head_reference,
            &self.selected.api_base_url,
            AccountRead::Snapshot,
        )
        .await
        .map_err(|_| changed())?;
        require_fresh_account(&current.account).map_err(|_| changed())?;
        if current.account.snapshot_ref().map_err(|_| changed())? != self.selected.account_reference
            || current.access_handle != self.selected.access_handle
            || current.access_reference != self.selected.access_reference
        {
            return Err(changed());
        }
        transaction.commit().await.map_err(|_| changed())?;
        Ok(())
    }
}

#[async_trait]
impl ProviderDispatchGuard for NativeCodexAccessGuard {
    async fn check(
        &self,
        facts: ProviderRequestFacts<'_>,
    ) -> Result<ProviderDispatchState, LlmError> {
        let state = self.provider_guard.check(facts).await?;
        // Provider reads are effect-free but may wait. Authenticate the exact
        // token generation after that await, not before an arbitrary DB wait.
        self.verify_selected_access().await?;
        self.require_unexpired()?;
        Ok(state)
    }

    async fn check_preparation(
        &self,
        facts: ProviderRequestFacts<'_>,
    ) -> Result<ProviderDispatchState, LlmError> {
        let state = self.provider_guard.check_preparation(facts).await?;
        self.verify_selected_access().await?;
        self.require_unexpired()?;
        Ok(state)
    }
}

impl PgAuthProductServices {
    /// Consume the currently published access generation for this captured
    /// login lineage. The complete source handles an eligible original refresh
    /// separately; this method never refreshes, switches login or uses files.
    pub(crate) async fn take_codex_access_session(
        self: &Arc<Self>,
        namespace: &str,
        provider_id: &str,
        head_reference: &str,
        api_base_url: &str,
        provider_guard: Arc<dyn ProviderDispatchGuard>,
    ) -> Result<CodexHostSession, LlmError> {
        let mut client = self.pool.get().await.map_err(|_| changed())?;
        let transaction = client.transaction().await.map_err(|_| changed())?;
        let current = provider_codex_credentials::current_access(
            &transaction,
            &self.secret_store,
            namespace,
            provider_id,
            head_reference,
            api_base_url,
            AccountRead::ForUpdate,
        )
        .await
        .map_err(|_| changed())?;
        require_fresh_account(&current.account)?;
        let scope = &current.account.scope.resource;
        let lease = self
            .secret_store
            .lease_once_if_record_reference_in_transaction(
                &transaction,
                scope,
                &current.access_handle,
                &current.access_reference,
            )
            .await
            .map_err(|error| match error {
                brassclaw_secrets::SecretStoreError::SecretExpired => LlmError::SessionExpired {
                    provider: "openai_codex".into(),
                },
                _ => changed(),
            })?
            .ok_or_else(changed)?;
        // Leasing may upgrade legacy storage under its row lock. Capture the
        // actual post-upgrade generation, not the pre-upgrade inspection ref.
        let access_reference = self
            .secret_store
            .inspect_record_reference_in_transaction(&transaction, scope, &current.access_handle)
            .await
            .map_err(|_| changed())?
            .ok_or_else(changed)?;
        let account_reference = current.account.snapshot_ref().map_err(|_| changed())?;
        let token = self
            .secret_store
            .consume_in_transaction(&transaction, scope, lease.id)
            .await
            .map_err(|_| changed())?;
        let selected = SelectedAccess {
            namespace: namespace.into(),
            provider_id: provider_id.into(),
            head_reference: current.registration.head_reference,
            api_base_url: api_base_url.into(),
            account_reference,
            access_handle: current.access_handle,
            access_reference,
            account_expires_at: current.account.access_expires_at,
            secret_expires_at: lease.secret_expires_at,
        };
        // No token reaches the SDK after an uncertain lease-consumption commit.
        // A final immutable guard still rechecks this exact source before send.
        transaction.commit().await.map_err(|_| changed())?;
        Ok(CodexHostSession::new(
            token,
            Arc::new(NativeCodexAccessGuard {
                owner: Arc::clone(self),
                selected,
                provider_guard,
            }),
        ))
    }
}
