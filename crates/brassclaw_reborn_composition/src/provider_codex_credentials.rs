//! Native instance registration metadata in the existing provider/secret owners.
//! Selected by native login publication and immutable per-request access guards.
use brassclaw_auth::{
    AuthProductScope, AuthSurface, CredentialAccount, CredentialAccountId, CredentialOwnership,
};
use brassclaw_host_api::{ResourceScope, SecretHandle};
use brassclaw_product_workflow::LlmConfigServiceError;
use brassclaw_secrets::{PgSecretStore, SecretMaterial};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub(crate) const LOGIN_KIND: &str = "codex-device-login/1";
const BINDING_KIND: &str = "codex-instance-credential/1";

// One neutral descriptor shared with AuthFlowCustody; its definition and pure
// syntax validation are supplied by the connected private Device-Login owner.
pub(crate) type CodexOAuthClient = brassclaw_auth::NativeDeviceOAuthClient;

/// The real installer scope is provenance; System ownership excludes ordinary
/// extension/user refresh consumers. The global head itself uses System scope.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CodexCredentialBinding {
    format: String,
    namespace: String,
    provider_id: String,
    pub(crate) login_id: Uuid,
    pub(crate) account_id: CredentialAccountId,
    account_scope: AuthProductScope,
    pub(crate) client: CodexOAuthClient,
}

impl CodexCredentialBinding {
    /// Metadata derived from the actual retained native definition. This does
    /// not authenticate its storage or approve dispatch; the native Auth owner
    /// must still verify the original and real account in the caller commit.
    pub(crate) fn from_definition(
        namespace: &str,
        provider_id: &str,
        definition: &brassclaw_auth::NativeDeviceLoginDefinition,
    ) -> Result<Self, LlmConfigServiceError> {
        definition
            .validate()
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        if definition.provider.as_str() != provider_id {
            return Err(LlmConfigServiceError::Unavailable);
        }
        let binding = Self {
            format: BINDING_KIND.into(),
            namespace: namespace.into(),
            provider_id: provider_id.into(),
            login_id: definition.flow_id.as_uuid(),
            account_id: definition.account_id,
            account_scope: definition.scope.clone(),
            client: definition.client.clone(),
        };
        binding.validate(namespace, provider_id)?;
        Ok(binding)
    }

    pub(crate) fn new(
        namespace: &str,
        provider_id: &str,
        login_id: Uuid,
        account: &CredentialAccount,
        client: CodexOAuthClient,
    ) -> Result<Self, LlmConfigServiceError> {
        let binding = Self {
            format: BINDING_KIND.into(),
            namespace: namespace.into(),
            provider_id: provider_id.into(),
            login_id,
            account_id: account.id,
            account_scope: account.scope.clone(),
            client,
        };
        binding.validate(namespace, provider_id)?;
        binding.verify_account(account)?;
        Ok(binding)
    }

    fn validate(&self, namespace: &str, provider_id: &str) -> Result<(), LlmConfigServiceError> {
        let scope = &self.account_scope;
        let resource = &scope.resource;
        if self.format != BINDING_KIND
            || self.namespace != namespace
            || self.provider_id != provider_id
            || provider_id.is_empty()
            || self.login_id.is_nil()
            || resource.is_system()
            || resource.tenant_id.as_str() != namespace
            || resource.agent_id.is_some()
            || resource.project_id.is_some()
            || resource.thread_id.is_some()
            || scope.surface != AuthSurface::SetupAdmin
            || scope.session_id.is_some()
        {
            return Err(LlmConfigServiceError::Unavailable);
        }
        self.client
            .validate()
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        oauth_handle(provider_id)?;
        Ok(())
    }

    pub(crate) fn verify_account(
        &self,
        account: &CredentialAccount,
    ) -> Result<(), LlmConfigServiceError> {
        if account.id != self.account_id
            || account.scope != self.account_scope
            || account.provider.as_str() != self.provider_id
            || account.ownership != CredentialOwnership::System
            || account.owner_extension.is_some()
            || !account.granted_extensions.is_empty()
        {
            return Err(LlmConfigServiceError::Unavailable);
        }
        Ok(())
    }

    pub(crate) fn material(&self) -> Result<SecretMaterial, LlmConfigServiceError> {
        let mut value =
            serde_json::to_value(self).map_err(|_| LlmConfigServiceError::Unavailable)?;
        value.sort_all_objects();
        serde_json::to_string(&value)
            .map(SecretMaterial::from)
            .map_err(|_| LlmConfigServiceError::Unavailable)
    }
}

impl std::fmt::Debug for CodexCredentialBinding {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("CodexCredentialBinding([REDACTED])")
    }
}

pub(crate) fn oauth_handle(provider_id: &str) -> Result<SecretHandle, LlmConfigServiceError> {
    crate::llm_key_store::oauth_handle_for(provider_id)
        .map_err(|_| LlmConfigServiceError::Unavailable)
}

/// One exact metadata capture, with no public serialization or secret value.
pub(crate) struct BoundCodexRegistration {
    pub(crate) binding: CodexCredentialBinding,
    pub(crate) head_reference: String,
}

/// Authenticated metadata for one native account; never a token or permission.
pub(crate) struct BoundCodexAccess {
    pub(crate) registration: BoundCodexRegistration,
    pub(crate) account: CredentialAccount,
    pub(crate) access_handle: SecretHandle,
    pub(crate) access_reference: String,
}

#[derive(Clone, Copy)]
pub(crate) enum AccountRead {
    Snapshot,
    ForUpdate,
}

/// The caller owns the transaction and selects its real isolation. Dispatch
/// inspection uses one read-only repeatable snapshot; token consumption takes
/// the existing account lock before acquiring a secret lease.
pub(crate) async fn current_access(
    transaction: &tokio_postgres::Transaction<'_>,
    secrets: &PgSecretStore,
    namespace: &str,
    provider_id: &str,
    expected_head_reference: &str,
    expected_api_base_url: &str,
    mode: AccountRead,
) -> Result<BoundCodexAccess, LlmConfigServiceError> {
    let registration = original_registration(
        transaction,
        secrets,
        namespace,
        provider_id,
        expected_head_reference,
    )
    .await?;
    if crate::llm_config_service::normalized_endpoint(Some(
        &registration.binding.client.api_base_url,
    )) != crate::llm_config_service::normalized_endpoint(Some(expected_api_base_url))
    {
        return Err(LlmConfigServiceError::Unavailable);
    }
    let query = match mode {
        AccountRead::Snapshot => {
            "SELECT data FROM brassclaw_product_auth_accounts WHERE tenant_id=$1 AND id=$2"
        }
        AccountRead::ForUpdate => {
            "SELECT data FROM brassclaw_product_auth_accounts WHERE tenant_id=$1 AND id=$2 FOR UPDATE"
        }
    };
    let row = transaction
        .query_opt(
            query,
            &[&namespace, &registration.binding.account_id.to_string()],
        )
        .await
        .map_err(|_| LlmConfigServiceError::Unavailable)?
        .ok_or(LlmConfigServiceError::Unavailable)?;
    let account: CredentialAccount =
        serde_json::from_value(row.get(0)).map_err(|_| LlmConfigServiceError::Unavailable)?;
    registration.binding.verify_account(&account)?;
    // Under ReadCommitted, acquiring the account lock may have waited. Check
    // the actual original head again rather than trusting the earlier read.
    if matches!(mode, AccountRead::ForUpdate) {
        original_registration(
            transaction,
            secrets,
            namespace,
            provider_id,
            expected_head_reference,
        )
        .await?;
    }
    let access_handle = account
        .access_secret
        .clone()
        .ok_or(LlmConfigServiceError::Unavailable)?;
    let access_reference = secrets
        .inspect_record_reference_in_transaction(
            transaction,
            &account.scope.resource,
            &access_handle,
        )
        .await
        .map_err(|_| LlmConfigServiceError::Unavailable)?
        .ok_or(LlmConfigServiceError::Unavailable)?;
    Ok(BoundCodexAccess {
        registration,
        account,
        access_handle,
        access_reference,
    })
}

/// The SAME provider mutation/secret snapshot, never a latest-role fallback.
/// Native login publishes its actual immutable mutation's encrypted-head link
/// with this binding. An unchanged provider definition need not gain a revision;
/// neither revision order nor timestamps identify a login. A failed binding
/// comparison never searches older receipts.
pub(crate) async fn original_registration(
    transaction: &tokio_postgres::Transaction<'_>,
    secrets: &PgSecretStore,
    namespace: &str,
    provider_id: &str,
    expected_head_reference: &str,
) -> Result<BoundCodexRegistration, LlmConfigServiceError> {
    match inspect_registration(
        transaction,
        secrets,
        namespace,
        provider_id,
        expected_head_reference,
    )
    .await?
    {
        RegistrationInspection::Bound(registration) => Ok(registration),
        RegistrationInspection::Invalid => Err(LlmConfigServiceError::Unavailable),
    }
}

pub(crate) enum RegistrationInspection {
    Bound(BoundCodexRegistration),
    Invalid,
}

/// Storage errors propagate before any further SQL; malformed local binding is
/// a separate disposition. A present head without its original is invalid.
pub(crate) async fn inspect_registration(
    transaction: &tokio_postgres::Transaction<'_>,
    secrets: &PgSecretStore,
    namespace: &str,
    provider_id: &str,
    expected_head_reference: &str,
) -> Result<RegistrationInspection, LlmConfigServiceError> {
    let row = transaction
        .query_opt(
            "SELECT mutation_id,request FROM brassclaw_llm_provider_mutations
         WHERE tenant_id=$1 AND provider_id=$2 AND operation='upsert'
           AND request->>'kind'=$3 AND provider_revision IS NOT NULL
           AND credential_head_ref=$4",
            &[
                &namespace,
                &provider_id,
                &LOGIN_KIND,
                &expected_head_reference,
            ],
        )
        .await
        .map_err(|_| LlmConfigServiceError::Unavailable)?;
    let Some(row) = row else {
        return Ok(RegistrationInspection::Invalid);
    };
    let decode = || -> Result<CodexCredentialBinding, LlmConfigServiceError> {
        let original_id: Uuid = row
            .try_get(0)
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let request: serde_json::Value = row
            .try_get(1)
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let binding: CodexCredentialBinding = serde_json::from_value(
            request
                .get("credential_binding")
                .cloned()
                .ok_or(LlmConfigServiceError::Unavailable)?,
        )
        .map_err(|_| LlmConfigServiceError::Unavailable)?;
        binding.validate(namespace, provider_id)?;
        if binding.login_id != original_id {
            return Err(LlmConfigServiceError::Unavailable);
        }
        Ok(binding)
    };
    let binding = match decode() {
        Ok(binding) => binding,
        Err(_) => return Ok(RegistrationInspection::Invalid),
    };
    if !secrets
        .inspect_matches_if_record_reference_in_transaction(
            transaction,
            &ResourceScope::system(),
            &oauth_handle(provider_id)?,
            expected_head_reference,
            &binding.material()?,
        )
        .await
        .map_err(|_| LlmConfigServiceError::Unavailable)?
    {
        return Ok(RegistrationInspection::Invalid);
    }
    Ok(RegistrationInspection::Bound(BoundCodexRegistration {
        binding,
        head_reference: expected_head_reference.into(),
    }))
}
