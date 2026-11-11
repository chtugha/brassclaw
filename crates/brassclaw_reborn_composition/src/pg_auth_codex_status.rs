//! Native status reads only existing authenticated originals in the caller's
//! READ ONLY snapshot. No pool checkout, account lock, lease or network call.
use super::PgAuthProductServices;
use crate::provider_codex_credentials::{RegistrationInspection, inspect_registration};
use brassclaw_auth::{CredentialAccount, CredentialAccountStatus};
use brassclaw_product_workflow::{
    LlmConfigServiceError, NativeAccessLifetime, NativeAccessStatus, NativeAccountStatus,
    NativeProviderAuthStatus, NativeRefreshStatus, NativeRegistrationStatus,
};

impl PgAuthProductServices {
    pub(crate) async fn inspect_native_codex_status(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        namespace: &str,
        provider_id: &str,
        endpoint: &str,
        head: Option<&str>,
        mut status: NativeProviderAuthStatus,
    ) -> Result<NativeProviderAuthStatus, LlmConfigServiceError> {
        let Some(head) = head else {
            status.registration = NativeRegistrationStatus::Absent;
            status.access = NativeAccessStatus::Absent;
            status.refresh = NativeRefreshStatus::Absent;
            status.recovery = Some(brassclaw_auth::CredentialRecoveryReason::NoAccount);
            return Ok(status);
        };
        let registration = match inspect_registration(
            transaction,
            &self.secret_store,
            namespace,
            provider_id,
            head,
        )
        .await?
        {
            RegistrationInspection::Bound(value) => value,
            RegistrationInspection::Invalid => {
                status.registration = NativeRegistrationStatus::InvalidBinding;
                return Ok(status);
            }
        };
        if crate::llm_config_service::normalized_endpoint(Some(
            &registration.binding.client.api_base_url,
        )) != crate::llm_config_service::normalized_endpoint(Some(endpoint))
        {
            status.registration = NativeRegistrationStatus::InvalidBinding;
            return Ok(status);
        }
        status.login_mutation_id = Some(registration.binding.login_id);
        let row = transaction.query_opt(
            "SELECT data,user_id FROM brassclaw_product_auth_accounts WHERE tenant_id=$1 AND id=$2",
            &[&namespace, &registration.binding.account_id.to_string()],
        ).await.map_err(|_| LlmConfigServiceError::Unavailable)?;
        let Some(row) = row else {
            status.registration = NativeRegistrationStatus::AccountMissing;
            status.access = NativeAccessStatus::Absent;
            status.recovery = Some(brassclaw_auth::CredentialRecoveryReason::AccountMissing);
            return Ok(status);
        };
        let account = serde_json::from_value::<CredentialAccount>(row.get(0));
        let account = match account {
            Ok(account)
                if registration.binding.verify_account(&account).is_ok()
                    && row.get::<_, String>(1) == account.scope.resource.user_id.as_str() =>
            {
                account
            }
            _ => {
                status.registration = NativeRegistrationStatus::InvalidBinding;
                return Ok(status);
            }
        };
        status.registration = NativeRegistrationStatus::Bound;
        status.account = Some(NativeAccountStatus {
            id: account.id,
            status: account.effective_status_at(status.observed_at),
        });
        status.account_expires_at = account.access_expires_at;
        status.recovery = Some(
            brassclaw_auth::domain::recovery_projection_for_single_account_at(
                account.provider.clone(),
                &account,
                status.observed_at,
            )
            .reason,
        );
        status.access = match &account.access_secret {
            None => NativeAccessStatus::Absent,
            Some(handle) => {
                let metadata = self
                    .secret_store
                    .inspect_runtime_metadata_in_transaction(
                        transaction,
                        &account.scope.resource,
                        handle,
                    )
                    .await
                    .map_err(|_| LlmConfigServiceError::Unavailable)?;
                match metadata {
                    None => NativeAccessStatus::Absent,
                    Some(metadata) => {
                        status.secret_expires_at = metadata.expires_at;
                        let deadline = account
                            .access_expires_at
                            .into_iter()
                            .chain(metadata.expires_at)
                            .min();
                        status.lifetime = if deadline.is_some() {
                            NativeAccessLifetime::FiniteDeadline
                        } else {
                            NativeAccessLifetime::Unknown
                        };
                        if !metadata.runtime_eligible
                            || account.effective_status_at(status.observed_at)
                                != CredentialAccountStatus::Configured
                        {
                            NativeAccessStatus::Ineligible
                        } else if deadline.is_some_and(|expiry| expiry <= status.observed_at) {
                            NativeAccessStatus::Expired
                        } else if deadline.is_none() {
                            NativeAccessStatus::UnknownLifetime
                        } else {
                            NativeAccessStatus::Current
                        }
                    }
                }
            }
        };
        // Account expiry is separate from secret purpose and takes precedence
        // over the generic ineligible category for an otherwise configured account.
        if account.status == CredentialAccountStatus::Configured
            && account
                .access_expires_at
                .is_some_and(|expiry| expiry <= status.observed_at)
            && status.access != NativeAccessStatus::Absent
        {
            status.access = NativeAccessStatus::Expired;
        }
        status.refresh = match &account.refresh_secret {
            None => NativeRefreshStatus::Absent,
            Some(handle) => {
                let metadata = self
                    .secret_store
                    .inspect_runtime_metadata_in_transaction(
                        transaction,
                        &account.scope.resource,
                        handle,
                    )
                    .await
                    .map_err(|_| LlmConfigServiceError::Unavailable)?;
                let blocked = self
                    .inspect_blocking_refresh_in_transaction(transaction, &account, handle)
                    .await?;
                status.blocking_refresh = blocked;
                if status.blocking_refresh.is_some() {
                    NativeRefreshStatus::BlockedOriginal
                } else if metadata.as_ref().is_some_and(|value| {
                    value.runtime_eligible
                        && !value
                            .expires_at
                            .is_some_and(|expiry| expiry <= status.observed_at)
                }) {
                    NativeRefreshStatus::Present
                } else if metadata.is_none() {
                    NativeRefreshStatus::Absent
                } else {
                    NativeRefreshStatus::Unavailable
                }
            }
        };
        Ok(status)
    }
}
