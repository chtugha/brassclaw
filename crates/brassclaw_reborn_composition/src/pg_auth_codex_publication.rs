#![allow(dead_code)] // PRIVATE036: staged publication APIs; callers land with the full login/startup wiring.
//! PRIVATE036 publication fragment in the existing native Auth owner.
//! Caller owns provider mutation/definition/role locks and the single COMMIT.
//! Not mounted/compiled; actual native Flow finish and callers remain required.
use super::PgAuthProductServices;
use std::sync::Arc;

use brassclaw_auth::{CredentialAccount, CredentialAccountStatus, CredentialOwnership};
use brassclaw_host_api::ResourceScope;
use brassclaw_product_workflow::LlmConfigServiceError;
use chrono::Utc;
use secrecy::ExposeSecret as _;

use crate::provider_codex_credentials::{CodexCredentialBinding, oauth_handle};

pub(crate) struct PublishedCodexAccount {
    pub(crate) account: CredentialAccount,
    pub(crate) binding: CodexCredentialBinding,
    pub(crate) head_reference: String,
}

impl PgAuthProductServices {
    /// Process-local duplicate-driver suppression in the existing Auth lock
    /// cache. This is not issuer authority: every actual request still needs
    /// its acknowledged durable claim and authenticated original.
    pub(crate) fn native_device_driver_lock(
        &self,
        original: &brassclaw_auth::NativeDeviceLoginOriginal,
    ) -> Result<Arc<tokio::sync::Mutex<()>>, brassclaw_auth::AuthProductError> {
        original.validate()?;
        let key = serde_json::to_string(&(
            "native-codex-driver",
            &original.definition.scope.resource.tenant_id,
            original.definition.flow_id,
        ))
        .map_err(|_| brassclaw_auth::AuthProductError::BackendUnavailable)?;
        Ok(self.lock_for(key))
    }

    /// Reuse the composed host transport, encrypted store and acknowledged
    /// policy. The ordinary callback provider registry does not register this
    /// native protocol client. Every native URL comes from its retained client
    /// descriptor and is checked by the shared host HTTP pipeline.
    pub(crate) fn native_codex_transport(
        &self,
        ports: &brassclaw_host_runtime::ProductAuthProviderRuntimePorts,
        client: &brassclaw_auth::NativeDeviceOAuthClient,
        settings: Option<Arc<dyn brassclaw_host_runtime::HostPrimitiveSettingsSource>>,
    ) -> Result<
        crate::oauth_provider_client::HostOAuthProviderClient,
        brassclaw_auth::AuthProductError,
    > {
        use crate::oauth_provider_client::{
            ExchangeScopePolicy, HostOAuthProviderClient, HostOAuthProviderSpec,
        };
        use brassclaw_auth::{OAuthClientId, OAuthRedirectUri};

        client.validate()?;
        let spec = HostOAuthProviderSpec {
            provider_id: "openai_codex",
            capability_id: "brassclaw_auth.openai_codex_oauth",
            token_endpoint: "https://auth.openai.com/oauth/token",
            secret_handle_prefix: "openai-codex",
            resource: None,
            exchange_scope_policy: ExchangeScopePolicy::FallbackToRequested,
        };
        HostOAuthProviderClient::new(
            spec,
            ports.runtime_http_egress(),
            self.secret_store.clone(),
            ports.obligation_handler(),
            OAuthClientId::new(client.client_id.clone())?,
            OAuthRedirectUri::new(format!(
                "{}/deviceauth/callback",
                client.auth_endpoint.trim_end_matches('/')
            ))?,
        )
        .map(|transport| {
            transport
                .with_original_egress(ports.original_http_egress())
                .with_primitive_settings_source(settings)
        })
    }

    /// No issuer call and no implicit retry. The ready record authenticates the
    /// exact retained response; account, derived tokens and global head join the
    /// caller's provider/role/receipt commit. A rollback consumes nothing.
    pub(crate) async fn publish_codex_account_and_head_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        ready: &super::device::NativeDeviceReady,
        namespace: &str,
        provider_id: &str,
        expected_head_reference: Option<&str>,
    ) -> Result<PublishedCodexAccount, LlmConfigServiceError> {
        let definition = &ready.original.definition;
        definition
            .validate()
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        if definition.scope.resource.tenant_id.as_str() != namespace
            || definition.provider.as_str() != provider_id
        {
            return Err(LlmConfigServiceError::Unavailable);
        }
        // Preserve the native Flow -> Account ordering. The token consumer
        // reauthenticates this exact ready snapshot under the same flow lock.
        self.native_device_ready_in_transaction(transaction, &ready.original)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        if super::callback::locked_account(transaction, &definition.scope, definition.account_id)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?
            .is_some()
        {
            return Err(LlmConfigServiceError::Conflict {
                reason: "Native login account already exists; inspect the original publication."
                    .into(),
            });
        }
        let tokens = self
            .native_device_tokens_in_transaction(transaction, ready)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        if tokens.expires_at != ready.access_expires_at || tokens.expires_at <= Utc::now() {
            return Err(LlmConfigServiceError::Unavailable);
        }
        let now = Utc::now();
        let has_refresh_token = !tokens.refresh_token.expose_secret().is_empty();
        let account = CredentialAccount {
            id: definition.account_id,
            scope: definition.scope.clone(),
            provider: definition.provider.clone(),
            label: definition.account_label.clone(),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::System,
            owner_extension: None,
            granted_extensions: vec![],
            access_secret: Some(definition.destinations.access_secret.clone()),
            access_expires_at: Some(tokens.expires_at),
            refresh_secret: has_refresh_token
                .then(|| definition.destinations.refresh_secret.clone()),
            scopes: vec![],
            created_at: now,
            updated_at: now,
        };
        let binding = CodexCredentialBinding::new(
            namespace,
            provider_id,
            definition.flow_id.as_uuid(),
            &account,
            definition.client.clone(),
        )?;
        // Create-only destinations are retained before the issuer request. Use
        // the store's actual atomic absent-reference fence, not read-then-put.
        if self
            .secret_store
            .put_with_expiry_if_record_reference_in_transaction(
                transaction,
                account.scope.resource.clone(),
                definition.destinations.access_secret.clone(),
                tokens.access_token,
                account.access_expires_at,
                None,
            )
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?
            .is_none()
        {
            return Err(LlmConfigServiceError::Unavailable);
        }
        if has_refresh_token
            && self
                .secret_store
                .put_if_record_reference_in_transaction(
                    transaction,
                    account.scope.resource.clone(),
                    definition.destinations.refresh_secret.clone(),
                    tokens.refresh_token,
                    None,
                )
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable)?
                .is_none()
        {
            return Err(LlmConfigServiceError::Unavailable);
        }
        super::callback::save_account(transaction, &account, None)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let handle = oauth_handle(provider_id)?;
        if self
            .secret_store
            .put_if_record_reference_in_transaction(
                transaction,
                ResourceScope::system(),
                handle.clone(),
                binding.material()?,
                expected_head_reference,
            )
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?
            .is_none()
        {
            return Err(LlmConfigServiceError::Conflict {
                reason: "Credentials changed during login; inspect the original publication."
                    .into(),
            });
        }
        let head_reference = self
            .secret_store
            .inspect_record_reference_in_transaction(transaction, &ResourceScope::system(), &handle)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?
            .ok_or(LlmConfigServiceError::Unavailable)?;
        Ok(PublishedCodexAccount {
            account,
            binding,
            head_reference,
        })
    }
}
