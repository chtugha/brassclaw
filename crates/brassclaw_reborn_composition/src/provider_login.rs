//! Existing provider publication owner: bind an explicit browser login to its
//! original database context and commit credentials/selection/receipt together.
//! This is setup custody, not an agent workflow or a dispatch grant.
use super::*;
use serde::{Deserialize, Serialize};
use serde_json::json;

const BROWSER_LOGIN: &str = "nearai-browser-login/1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NearAiLoginContext {
    kind: String,
    pub(super) id: uuid::Uuid,
    namespace: String,
    actor: String,
    provider_revision: u64,
    selection_ref: String,
    credential_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target_model: Option<String>,
    auth_provider: String,
    origin: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    wallet: Option<WalletChallenge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WalletChallenge {
    nonce: Vec<u8>,
    message: String,
    recipient: String,
    callback_url: Option<String>,
}

#[path = "provider_wallet_login.rs"]
mod wallet;

impl RebornLlmConfigService {
    pub(super) async fn capture_nearai_login(
        &self,
        caller: &WebUiAuthenticatedCaller,
        origin: &str,
        auth_provider: &str,
    ) -> Result<NearAiLoginContext, LlmConfigServiceError> {
        let mut client = self
            .pg_pool
            .get()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let transaction = tokio_postgres::Client::build_transaction(&mut client)
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let selected = self
            .pg_provider_repo
            .get_revision_from_client(&transaction, "nearai", false)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?
            .ok_or(LlmConfigServiceError::NotFound)?;
        if selected.definition.protocol != ProviderProtocol::NearAi {
            return Err(LlmConfigServiceError::InvalidRequest {
                field: Some("provider".into()),
                reason: "The selected NEAR login provider uses another protocol.".into(),
            });
        }
        // Authentication to the same provider does not request a model edit.
        // A different provider's model must never travel into this selection.
        let roles = crate::db_config::provider_role_selections(&transaction, &self.db_tenant_id)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let selections =
            crate::db_config::model_selection_refs_from_values(&roles, &self.db_tenant_id)
                .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let target_model = roles
            .get("llm.default.provider_id")
            .filter(|provider| provider.as_str() == "nearai")
            .and_then(|_| roles.get("llm.default.model"))
            .filter(|model| !model.is_empty())
            .cloned();
        let credential_ref = self
            .keys
            .inspect_record_reference_in_transaction(&transaction, "nearai")
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let context = NearAiLoginContext {
            kind: BROWSER_LOGIN.into(),
            id: uuid::Uuid::new_v4(),
            namespace: self.db_tenant_id.clone(),
            actor: caller.user_id.as_str().into(),
            provider_revision: selected.revision,
            selection_ref: selections
                .get("kohai")
                .cloned()
                .ok_or(LlmConfigServiceError::Unavailable)?,
            credential_ref,
            target_model,
            auth_provider: auth_provider.into(),
            origin: origin.into(),
            wallet: None,
        };
        transaction
            .commit()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        Ok(context)
    }

    /// The state is one-use for a new commit. After acknowledgement loss, only
    /// the exact committed original may be read/reconciled, even across restart.
    pub(crate) async fn complete_nearai_browser_login(
        &self,
        state: &str,
        token: &str,
    ) -> Result<LlmConfigSnapshot, LlmConfigServiceError> {
        let id = uuid::Uuid::parse_str(state).map_err(|_| LlmConfigServiceError::NotFound)?;
        if id.is_nil()
            || token.trim().is_empty()
            || reqwest::header::HeaderValue::from_str(&format!("Bearer {token}")).is_err()
        {
            return Err(LlmConfigServiceError::NotFound);
        }
        let _publication = self.publication.lock().await;
        let pending = self
            .nearai_login_states
            .consume_matching(state, |context| {
                context.kind == BROWSER_LOGIN && context.wallet.is_none()
            })
            .await;
        let fresh = pending.is_some();
        let context = match pending {
            Some(context) => context,
            None => {
                self.committed_nearai_login_context(id, BROWSER_LOGIN)
                    .await?
            }
        };
        if context.id != id
            || context.namespace != self.db_tenant_id
            || context.kind != BROWSER_LOGIN
        {
            return Err(LlmConfigServiceError::NotFound);
        }
        let credential = SecretString::from(token.to_owned());
        let mutation = crate::provider_mutation::ProviderMutation::new(
            id,
            "upsert",
            json!(context),
            Some(&credential),
        )?;
        let receipt = match self.lookup_provider_mutation(&mutation).await? {
            Some(original) => original,
            None if !fresh => return Err(LlmConfigServiceError::NotFound),
            None => {
                self.prepare_nearai_login(&context, &credential).await?;
                let result = self
                    .commit_nearai_login(&context, &credential, &mutation)
                    .await;
                match result {
                    Ok(receipt) => receipt,
                    Err(error) => self
                        .lookup_provider_mutation(&mutation)
                        .await?
                        .ok_or(error)?,
                }
            }
        };
        self.finish_provider_mutation(receipt).await
    }

    async fn committed_nearai_login_context(
        &self,
        id: uuid::Uuid,
        kind: &str,
    ) -> Result<NearAiLoginContext, LlmConfigServiceError> {
        let client = self
            .pg_pool
            .get()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let row=client.query_opt("SELECT request FROM brassclaw_llm_provider_mutations WHERE tenant_id=$1 AND mutation_id=$2 AND operation='upsert' AND request->>'kind'=$3",&[&self.db_tenant_id,&id,&kind]).await
            .map_err(|_|LlmConfigServiceError::Unavailable)?.ok_or(LlmConfigServiceError::NotFound)?;
        let context: NearAiLoginContext =
            serde_json::from_value(row.get(0)).map_err(|_| LlmConfigServiceError::Unavailable)?;
        if context.id != id || context.namespace != self.db_tenant_id || context.kind != kind {
            return Err(LlmConfigServiceError::Unavailable);
        }
        Ok(context)
    }

    async fn prepare_nearai_login(
        &self,
        context: &NearAiLoginContext,
        credential: &SecretString,
    ) -> Result<(), LlmConfigServiceError> {
        let client = self
            .pg_pool
            .get()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let retained = self
            .pg_provider_repo
            .read_retained_revision(&**client, "nearai", context.provider_revision)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?
            .ok_or(LlmConfigServiceError::NotFound)?;
        drop(client);
        let registry = ProviderRegistry::new(vec![retained.definition]);
        crate::llm_catalog::validate_catalog(&registry)
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let selection = LlmSlotSelection {
            provider_id: Some("nearai".into()),
            model: context.target_model.clone(),
            base_url: None,
            api_key_env: None,
        };
        let config = resolve_against_registry_with_credentials(
            &selection,
            &registry,
            Some(credential.clone()),
        )
        .map_err(|_| LlmConfigServiceError::InvalidRequest {
            field: Some("provider".into()),
            reason: "Complete the provider endpoint, model and authentication before login.".into(),
        })?;
        let session = self
            .nearai_session
            .clone()
            .ok_or(LlmConfigServiceError::Unavailable)?;
        drop(
            brassclaw_llm::build_static_provider_chain(&config, session)
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable)?,
        );
        Ok(())
    }

    async fn commit_nearai_login(
        &self,
        context: &NearAiLoginContext,
        credential: &SecretString,
        mutation: &crate::provider_mutation::ProviderMutation,
    ) -> Result<brassclaw_product_workflow::ProviderMutationReceipt, LlmConfigServiceError> {
        let mut client = self
            .pg_pool
            .get()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let transaction = client
            .transaction()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        mutation.lock(&transaction, &self.db_tenant_id).await?;
        if let Some(original) = mutation
            .original(&transaction, &self.db_tenant_id, &self.keys)
            .await?
        {
            transaction
                .commit()
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable)?;
            return Ok(original);
        }
        let definition = self
            .pg_provider_repo
            .lock_active_revision(&transaction, "nearai")
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?
            .ok_or(LlmConfigServiceError::NotFound)?;
        if definition.revision != context.provider_revision
            || definition.definition.protocol != ProviderProtocol::NearAi
        {
            return Err(LlmConfigServiceError::Conflict {
                reason: "Provider changed during login; start a new login from current settings."
                    .into(),
            });
        }
        crate::db_config::save_model_selection_in_transaction(
            &transaction,
            &self.db_tenant_id,
            "kohai",
            &context.selection_ref,
            Some("nearai"),
            context.target_model.as_deref(),
        )
        .await
        .map_err(map_selection_edit)?;
        if !self
            .keys
            .put_if_record_reference_in_transaction(
                &transaction,
                "nearai",
                credential.clone(),
                context.credential_ref.as_deref(),
            )
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?
        {
            return Err(LlmConfigServiceError::Conflict {
                reason:
                    "Credentials changed during login; start a new login from current settings."
                        .into(),
            });
        }
        let receipt = mutation
            .record(
                &transaction,
                &self.db_tenant_id,
                &context.actor,
                Some(("nearai", definition.revision)),
                &self.keys,
            )
            .await?;
        transaction
            .commit()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        Ok(receipt)
    }
}

#[cfg(all(test, feature = "skills-db"))]
#[path = "provider_login_tests.rs"]
mod tests;
