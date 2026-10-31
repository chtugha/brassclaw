//! One-use signed challenge at the existing NEAR/provider publication owner.
//! Expired, consumed or lost RAM challenges cannot initiate another exchange.
use super::*;
use brassclaw_llm::NearWalletSignedMessage;
use brassclaw_product_workflow::{NearAiWalletLoginStart, NearAiWalletLoginStartRequest};

const WALLET_LOGIN: &str = "nearai-wallet-login/1";

fn wallet_request_matches(
    context: &NearAiLoginContext,
    request: &NearAiWalletLoginRequest,
) -> bool {
    context.kind == WALLET_LOGIN
        && context.id == request.mutation_id
        && context.wallet.as_ref().is_some_and(|challenge| {
            challenge.nonce == request.nonce
                && challenge.message == request.message
                && challenge.recipient == request.recipient
                && challenge.callback_url == request.callback_url
        })
}
fn wallet_proof(request: &NearAiWalletLoginRequest) -> SecretString {
    use sha2::{Digest, Sha256};
    // Hash typed, length-delimited fields directly. Do not clone raw signatures
    // into a JSON value. This fingerprint itself stays in the encrypted proof.
    let mut digest = Sha256::new();
    digest.update(b"brassclaw-nearai-wallet-request/1\0");
    for bytes in [
        request.account_id.as_bytes(),
        request.public_key.as_bytes(),
        request.signature.as_bytes(),
        request.message.as_bytes(),
        request.recipient.as_bytes(),
        request.nonce.as_slice(),
    ] {
        digest.update((bytes.len() as u64).to_be_bytes());
        digest.update(bytes);
    }
    digest.update([u8::from(request.callback_url.is_some())]);
    if let Some(callback) = &request.callback_url {
        digest.update((callback.len() as u64).to_be_bytes());
        digest.update(callback.as_bytes());
    }
    SecretString::from(format!("{:x}", digest.finalize()))
}

impl RebornLlmConfigService {
    pub(in crate::llm_config_service) async fn begin_nearai_wallet_login(
        &self,
        caller: &WebUiAuthenticatedCaller,
        request: NearAiWalletLoginStartRequest,
    ) -> Result<NearAiWalletLoginStart, LlmConfigServiceError> {
        self.nearai_session
            .as_ref()
            .ok_or(LlmConfigServiceError::Unavailable)?
            .near_wallet_auth_endpoint()
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let callback_url = match request.callback_url {
            None => None,
            Some(value) => {
                let parsed = url::Url::parse(&value).map_err(|_| invalid_callback())?;
                if !matches!(parsed.scheme(), "http" | "https")
                    || parsed.host_str().is_none()
                    || !parsed.username().is_empty()
                    || parsed.password().is_some()
                    || parsed.query().is_some()
                    || parsed.fragment().is_some()
                {
                    return Err(invalid_callback());
                }
                Some(parsed.to_string())
            }
        };
        let mut context = self.capture_nearai_login(caller, "", "near-wallet").await?;
        context.kind = WALLET_LOGIN.into();
        self.wallet_exchange_policy(&context).await?;
        let timestamp = u64::try_from(chrono::Utc::now().timestamp_millis())
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let mut nonce = Vec::with_capacity(32);
        nonce.extend_from_slice(&timestamp.to_be_bytes());
        nonce.extend_from_slice(context.id.as_bytes());
        nonce.extend_from_slice(&uuid::Uuid::new_v4().as_bytes()[..8]);
        let challenge = WalletChallenge {
            nonce,
            message: brassclaw_llm::session::NEAR_WALLET_LOGIN_MESSAGE.into(),
            recipient: brassclaw_llm::session::NEAR_WALLET_LOGIN_RECIPIENT.into(),
            callback_url,
        };
        let response = NearAiWalletLoginStart {
            mutation_id: context.id,
            message: challenge.message.clone(),
            recipient: challenge.recipient.clone(),
            nonce: challenge.nonce.clone(),
            callback_url: challenge.callback_url.clone(),
        };
        context.wallet = Some(challenge);
        self.nearai_login_states
            .issue_for(
                context,
                Duration::from_secs(brassclaw_llm::session::NEAR_WALLET_NONCE_LIFETIME_SECS),
            )
            .await;
        Ok(response)
    }

    pub(in crate::llm_config_service) async fn finish_nearai_wallet_login(
        &self,
        request: NearAiWalletLoginRequest,
    ) -> Result<NearAiWalletLoginResult, LlmConfigServiceError> {
        if request.mutation_id.is_nil() {
            return Err(LlmConfigServiceError::InvalidRequest {
                field: Some("mutation_id".into()),
                reason: "Use the original server wallet challenge.".into(),
            });
        }
        if request.account_id.trim().is_empty()
            || request.public_key.trim().is_empty()
            || request.signature.trim().is_empty()
        {
            return Err(LlmConfigServiceError::InvalidRequest {
                field: Some("signature".into()),
                reason: "Provide the complete signed wallet response.".into(),
            });
        }
        let pending = self
            .nearai_login_states
            .consume_matching(&request.mutation_id.to_string(), |context| {
                wallet_request_matches(context, &request)
            })
            .await;
        let fresh = pending.is_some();
        let context=match pending {
            Some(context)=>context,
            None=>self.committed_nearai_login_context(request.mutation_id,WALLET_LOGIN).await.map_err(|error|match error {
                LlmConfigServiceError::NotFound=>LlmConfigServiceError::Conflict {reason:"Wallet challenge is expired, already used or unavailable. Read the original outcome; start a fresh signed challenge only as a new explicit login.".into()},
                other=>other,
            })?,
        };
        if !wallet_request_matches(&context, &request) {
            return Err(LlmConfigServiceError::Conflict {
                reason: "Wallet response differs from its original challenge.".into(),
            });
        }
        let proof = wallet_proof(&request);
        let mutation = crate::provider_mutation::ProviderMutation::new(
            context.id,
            "upsert",
            json!(context),
            Some(&proof),
        )?;
        {
            let _publication = self.publication.lock().await;
            if let Some(original) = self.lookup_provider_mutation(&mutation).await? {
                let snapshot = self.finish_provider_mutation(original).await?;
                return wallet_result(snapshot);
            }
            if !fresh {
                return Err(LlmConfigServiceError::Unavailable);
            }
        }
        // Freshness is the exchange's linearization check. No database connection
        // or settings publication lock is held during network I/O. Later edits
        // are independently fenced by the final credential/role transaction.
        let policy = self.wallet_exchange_policy(&context).await?;
        let session = self
            .nearai_session
            .as_ref()
            .ok_or(LlmConfigServiceError::Unavailable)?;
        let signed = NearWalletSignedMessage {
            account_id: request.account_id,
            public_key: request.public_key,
            signature: request.signature,
            message: request.message,
            recipient: request.recipient,
            nonce: request.nonce,
            callback_url: request.callback_url,
        };
        let token=session.near_wallet_login_with_policy(&signed,policy).await.map_err(|error| {
            tracing::debug!(%error,"Wallet exchange did not produce a publishable credential; no automatic replay");
            LlmConfigServiceError::Unavailable
        })?;
        let credential = SecretString::from(token);
        self.wallet_exchange_policy(&context).await?;
        self.prepare_nearai_login(&context, &credential).await?;
        let _publication = self.publication.lock().await;
        let receipt = match self
            .commit_nearai_login(&context, &credential, &mutation)
            .await
        {
            Ok(receipt) => receipt,
            Err(error) => self
                .lookup_provider_mutation(&mutation)
                .await?
                .ok_or(error)?,
        };
        wallet_result(self.finish_provider_mutation(receipt).await?)
    }

    async fn wallet_exchange_policy(
        &self,
        context: &NearAiLoginContext,
    ) -> Result<brassclaw_llm::ProviderRequestPolicy, LlmConfigServiceError> {
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
        let roles = crate::db_config::model_selection_refs(&transaction, &self.db_tenant_id)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let key = self
            .keys
            .inspect_record_reference_in_transaction(&transaction, "nearai")
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        if context.namespace != self.db_tenant_id
            || selected.revision != context.provider_revision
            || selected.definition.protocol != ProviderProtocol::NearAi
            || roles.get("kohai") != Some(&context.selection_ref)
            || key != context.credential_ref
        {
            return Err(LlmConfigServiceError::Conflict {reason:"Provider settings or credentials changed while signing; use a fresh wallet challenge.".into()});
        }
        crate::llm_catalog::validate_catalog(&ProviderRegistry::new(vec![
            selected.definition.clone(),
        ]))
        .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let policy = selected.definition.request_policy;
        transaction
            .commit()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        Ok(policy)
    }
}
fn invalid_callback() -> LlmConfigServiceError {
    LlmConfigServiceError::InvalidRequest {
        field: Some("callback_url".into()),
        reason: "Use an absolute HTTP(S) callback URL without credentials, query or fragment."
            .into(),
    }
}
fn wallet_result(
    snapshot: LlmConfigSnapshot,
) -> Result<NearAiWalletLoginResult, LlmConfigServiceError> {
    Ok(NearAiWalletLoginResult {
        active: snapshot
            .kohai_active
            .as_ref()
            .is_some_and(|selection| selection.provider_id == "nearai"),
        mutation_receipt: snapshot
            .mutation_receipt
            .ok_or(LlmConfigServiceError::Unavailable)?,
    })
}

#[cfg(all(test, feature = "skills-db"))]
#[path = "provider_wallet_login_tests.rs"]
mod tests;
