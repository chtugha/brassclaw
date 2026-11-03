//! Native Codex refresh uses the existing account refresh journal and host
//! original custody. An acknowledged admission is the only path to issuer I/O.
//! Reconciliation never repeats transport, including after a lost acknowledgement.
use super::{PgAuthProductServices, db_err, pool_err};
use std::sync::Arc;

use brassclaw_auth::{
    AuthProductError, CredentialAccountStatus, CredentialRefreshAction, CredentialRefreshAdmission,
    CredentialRefreshCompletion, CredentialRefreshId, CredentialRefreshOriginal,
    CredentialRefreshOutcome, CredentialRefreshPrepared, OAuthProviderRefresh,
    OAuthProviderRefreshPreparation, OAuthRefreshSource, OAuthTokenDestinations,
};
use brassclaw_host_api::{ResourceScope, RuntimeHttpEgressRequest, SecretHandle};
use brassclaw_host_runtime::{
    HostHttpOriginalCustody, HostHttpOriginalCustodyError, HostPrimitiveSettingsSource,
    ProductAuthProviderRuntimePorts,
};
use brassclaw_llm::LlmError;
use brassclaw_secrets::{SecretCustodyMetadata, SecretCustodySource, SecretCustodyState};
use chrono::{DateTime, Utc};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::provider_codex_credentials::{self, AccountRead, BoundCodexAccess};

pub(crate) struct CodexAccessSelection<'a> {
    pub(crate) namespace: &'a str,
    pub(crate) provider_id: &'a str,
    pub(crate) head: &'a str,
    pub(crate) api_base_url: &'a str,
    pub(crate) margin_secs: u64,
}

/// Created only after the original journal COMMIT is acknowledged. Fields are
/// private: a recovered action or a caller flag cannot create transport custody.
pub(super) struct AdmittedCodexRefresh {
    current: BoundCodexAccess,
    action: CredentialRefreshAction,
    dispatch: bool,
}

fn unavailable() -> AuthProductError {
    AuthProductError::BackendUnavailable
}
fn recovery() -> LlmError {
    LlmError::SessionRenewalFailed {
        provider: "openai_codex".into(),
        reason:
            "Native credential refresh requires original outcome reconciliation or authorization"
                .into(),
    }
}
fn handle(id: CredentialRefreshId, suffix: &str) -> Result<SecretHandle, AuthProductError> {
    SecretHandle::new(format!("codex-refresh-{id}-{suffix}")).map_err(|_| unavailable())
}
fn context(action: &CredentialRefreshAction, head: &str) -> serde_json::Value {
    serde_json::json!({"kind":"native-codex-refresh/1","action":action.original,
        "account":action.expected_account,"source":action.source,"destinations":action.destinations,"head":head})
}
fn generation_id(
    current: &BoundCodexAccess,
    refresh_reference: &str,
) -> Result<CredentialRefreshId, AuthProductError> {
    // Full generation, including access/account, avoids reusing an applied action
    // when the issuer legitimately retains the same refresh token.
    let bytes = serde_json::to_vec(&serde_json::json!({
        "kind":"native-codex-refresh-generation/1", "binding":current.registration.binding,
        "head":current.registration.head_reference, "account":current.account,
        "access":current.access_reference, "refresh":refresh_reference,
    }))
    .map_err(|_| unavailable())?;
    Ok(CredentialRefreshId::from_uuid(Uuid::new_v5(
        &Uuid::NAMESPACE_OID,
        &bytes,
    )))
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestOriginal {
    started_at: DateTime<Utc>,
    request: RuntimeHttpEgressRequest,
}
#[derive(Deserialize)]
struct ResponseOriginal {
    status: u16,
    body: Vec<u8>,
}

impl PgAuthProductServices {
    /// Prepare a NEW model request only. This never repairs/replays a request
    /// already submitted to the model. Each successful take pins a fresh guard.
    pub(crate) async fn prepare_codex_access(
        self: &Arc<Self>,
        ports: &ProductAuthProviderRuntimePorts,
        settings: Option<Arc<dyn HostPrimitiveSettingsSource>>,
        selected: CodexAccessSelection<'_>,
    ) -> Result<(), LlmError> {
        self.prepare_codex_access_inner(ports, settings, &selected)
            .await
            .map_err(|_| recovery())
    }

    async fn prepare_codex_access_inner(
        self: &Arc<Self>,
        ports: &ProductAuthProviderRuntimePorts,
        settings: Option<Arc<dyn HostPrimitiveSettingsSource>>,
        selected: &CodexAccessSelection<'_>,
    ) -> Result<(), AuthProductError> {
        let Some(admitted) = self.admit_codex_refresh(selected).await? else {
            return Ok(());
        };
        let transport = self.native_codex_transport(
            ports,
            &admitted.current.registration.binding.client,
            settings,
        )?;
        self.execute_codex_refresh(&transport, &admitted, selected)
            .await
    }

    pub(super) async fn admit_codex_refresh(
        &self,
        selected: &CodexAccessSelection<'_>,
    ) -> Result<Option<AdmittedCodexRefresh>, AuthProductError> {
        let CodexAccessSelection {
            namespace,
            provider_id,
            head,
            api_base_url,
            margin_secs,
        } = *selected;
        let mut client = self.pool.get().await.map_err(pool_err)?;
        let tx = client.transaction().await.map_err(db_err)?;
        let current = provider_codex_credentials::current_access(
            &tx,
            &self.secret_store,
            namespace,
            provider_id,
            head,
            api_base_url,
            AccountRead::ForUpdate,
        )
        .await
        .map_err(|_| unavailable())?;
        if !matches!(
            current.account.status,
            CredentialAccountStatus::Configured | CredentialAccountStatus::Expired
        ) {
            return Err(AuthProductError::RefreshFailed);
        }
        let expiry = current
            .account
            .access_expires_at
            .ok_or(AuthProductError::RefreshFailed)?;
        let margin = i64::try_from(margin_secs)
            .ok()
            .and_then(chrono::Duration::try_seconds)
            .ok_or_else(unavailable)?;
        let refresh_at = expiry.checked_sub_signed(margin).ok_or_else(unavailable)?;
        if Utc::now() < refresh_at {
            tx.rollback().await.map_err(db_err)?;
            return Ok(None);
        }
        let refresh = current
            .account
            .refresh_secret
            .clone()
            .ok_or(AuthProductError::CredentialMissing)?;
        let refresh_reference = self
            .secret_store
            .inspect_record_reference_in_transaction(&tx, &current.account.scope.resource, &refresh)
            .await
            .map_err(|_| unavailable())?
            .ok_or(AuthProductError::CredentialMissing)?;
        let id = generation_id(&current, &refresh_reference)?;
        let prepared = CredentialRefreshPrepared {
            original: CredentialRefreshOriginal {
                id,
                scope: current.account.scope.clone(),
                provider: current.account.provider.clone(),
                account_id: current.account.id,
                requester_extension: None,
            },
            expected_account: current.account.clone(),
            preparation: Some(OAuthProviderRefreshPreparation {
                destinations: OAuthTokenDestinations {
                    access_secret: handle(id, "access")?,
                    refresh_secret: handle(id, "refresh")?,
                },
                source: OAuthRefreshSource {
                    scope: current.account.scope.resource.clone(),
                    handle: refresh,
                    record_reference: refresh_reference,
                },
            }),
        };
        let admission = self
            .begin_host_instance_refresh_in_transaction(&tx, prepared)
            .await?;
        // A lost COMMIT acknowledgement never permits transport. A subsequent
        // invocation sees the same journal identity and can only reconcile it.
        tx.commit().await.map_err(db_err)?;
        let (action, dispatch) = match admission {
            CredentialRefreshAdmission::Dispatch(action) => (action, true),
            CredentialRefreshAdmission::Original(action) => (action, false),
        };
        Ok(Some(AdmittedCodexRefresh {
            current,
            action,
            dispatch,
        }))
    }

    pub(super) async fn execute_codex_refresh(
        &self,
        transport: &crate::oauth_provider_client::HostOAuthProviderClient,
        admitted: &AdmittedCodexRefresh,
        selected: &CodexAccessSelection<'_>,
    ) -> Result<(), AuthProductError> {
        let AdmittedCodexRefresh {
            current,
            action,
            dispatch,
        } = admitted;
        if *dispatch {
            let result = async {
                let mut client = self.pool.get().await.map_err(pool_err)?;
                let tx = client.transaction().await.map_err(db_err)?;
                self.verify_codex_refresh(&tx, action, selected).await?;
                let source = action.source.as_ref().ok_or_else(unavailable)?;
                let lease = self
                    .secret_store
                    .lease_once_if_record_reference_in_transaction(
                        &tx,
                        &source.scope,
                        &source.handle,
                        &source.record_reference,
                    )
                    .await
                    .map_err(|_| unavailable())?
                    .ok_or(AuthProductError::BackendConflict)?;
                let token = self
                    .secret_store
                    .consume_in_transaction(&tx, &source.scope, lease.id)
                    .await
                    .map_err(|_| unavailable())?;
                tx.commit().await.map_err(db_err)?;
                let body = transport.native_refresh_body(
                    &current.registration.binding.client.client_id,
                    token.expose_secret(),
                )?;
                let request = transport
                    .prepare_oauth_http_request(
                        action.original.scope.resource.clone(),
                        &current.registration.binding.client.refresh_endpoint,
                        "application/x-www-form-urlencoded",
                        body,
                    )
                    .await?;
                let egress = transport.native_original_egress()?;
                let custodian = RefreshCustodian {
                    auth: self,
                    action,
                    selected,
                    expected: request.clone(),
                    transport: None,
                };
                // Public response may be sanitized or fail AFTER private custody.
                let _returned = egress
                    .execute_with_original_custody(request, Box::new(custodian))
                    .await;
                Ok::<(), AuthProductError>(())
            }
            .await;
            // Only this winning, returned execution can prove no request was
            // permitted. Recovered callers never classify another live driver's
            // absent-yet preflight as no-send.
            if self.record_codex_no_send_if_absent(action).await? {
                return Err(AuthProductError::BackendUnavailable);
            }
            // An installed request with no retained response remains unresolved.
            result?;
        }
        self.reconcile_codex_refresh(action, selected).await
    }

    async fn record_codex_no_send_if_absent(
        &self,
        action: &CredentialRefreshAction,
    ) -> Result<bool, AuthProductError> {
        let mut client = self.pool.get().await.map_err(pool_err)?;
        let tx = client.transaction().await.map_err(db_err)?;
        // Same account -> action order as admission and publication. No model
        // or provider transport is active after the winning future returned.
        super::callback::locked_account(&tx, &action.original.scope, action.original.account_id)
            .await?
            .ok_or(AuthProductError::CredentialMissing)?;
        let row=tx.query_opt("SELECT data FROM brassclaw_product_auth_refreshes WHERE tenant_id=$1 AND scope_ref=$2 AND id=$3 FOR UPDATE",
            &[&action.original.scope.resource.tenant_id.as_str(),&action.original.scope_ref()?,&action.original.id.to_string()]).await.map_err(db_err)?.ok_or(AuthProductError::BackendConflict)?;
        let stored: CredentialRefreshAction =
            serde_json::from_value(row.get(0)).map_err(|_| unavailable())?;
        if !stored.same_intent(action) {
            return Err(AuthProductError::BackendConflict);
        }
        if stored.outcome != CredentialRefreshOutcome::Submitted
            || stored.publication != brassclaw_auth::CredentialRefreshPublication::Pending
        {
            tx.rollback().await.map_err(db_err)?;
            return Ok(false);
        }
        if self
            .secret_store
            .custody_in_transaction(
                &tx,
                &action.original.scope.resource,
                &handle(action.original.id, "request")?,
            )
            .await
            .map_err(|_| unavailable())?
            .is_some()
        {
            tx.rollback().await.map_err(db_err)?;
            return Ok(false);
        }
        self.record_refresh_result_in_transaction(
            &tx,
            CredentialRefreshCompletion {
                dispatched: action.clone(),
                outcome: CredentialRefreshOutcome::NotSubmitted(
                    brassclaw_auth::CredentialRefreshStopReason::HostPreparationFailed,
                ),
            },
        )
        .await?;
        self.reconcile_refresh_action_in_transaction(&tx, &action.original)
            .await?;
        tx.commit().await.map_err(db_err)?;
        Ok(true)
    }

    async fn verify_codex_refresh(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
        action: &CredentialRefreshAction,
        selected: &CodexAccessSelection<'_>,
    ) -> Result<(), AuthProductError> {
        let CodexAccessSelection {
            namespace,
            provider_id,
            head,
            api_base_url,
            ..
        } = *selected;
        let current = provider_codex_credentials::current_access(
            tx,
            &self.secret_store,
            namespace,
            provider_id,
            head,
            api_base_url,
            AccountRead::ForUpdate,
        )
        .await
        .map_err(|_| AuthProductError::BackendConflict)?;
        if current.account != action.expected_account {
            return Err(AuthProductError::BackendConflict);
        }
        // Account -> journal -> head -> source. No locks cross HTTP transport.
        let row = tx.query_opt("SELECT data FROM brassclaw_product_auth_refreshes WHERE tenant_id=$1 AND scope_ref=$2 AND id=$3 FOR UPDATE",
            &[&namespace,&action.original.scope_ref()?,&action.original.id.to_string()]).await.map_err(db_err)?.ok_or(AuthProductError::BackendConflict)?;
        let stored: CredentialRefreshAction =
            serde_json::from_value(row.get(0)).map_err(|_| unavailable())?;
        if stored != *action {
            return Err(AuthProductError::BackendConflict);
        }
        if !self
            .secret_store
            .lock_matches_if_record_reference_in_transaction(
                tx,
                &ResourceScope::system(),
                &provider_codex_credentials::oauth_handle(provider_id)
                    .map_err(|_| unavailable())?,
                head,
                &current
                    .registration
                    .binding
                    .material()
                    .map_err(|_| unavailable())?,
            )
            .await
            .map_err(|_| unavailable())?
        {
            return Err(AuthProductError::BackendConflict);
        }
        let source = action.source.as_ref().ok_or_else(unavailable)?;
        if generation_id(&current, &source.record_reference)? != action.original.id {
            return Err(AuthProductError::BackendConflict);
        }
        if self
            .secret_store
            .inspect_record_reference_in_transaction(tx, &source.scope, &source.handle)
            .await
            .map_err(|_| unavailable())?
            .as_deref()
            != Some(source.record_reference.as_str())
        {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(())
    }

    async fn reconcile_codex_refresh(
        &self,
        action: &CredentialRefreshAction,
        selected: &CodexAccessSelection<'_>,
    ) -> Result<(), AuthProductError> {
        let head = selected.head;
        let stored = self
            .read_refresh_action(&action.original.request())
            .await?
            .ok_or(AuthProductError::BackendConflict)?;
        if !stored.same_intent(action) {
            return Err(AuthProductError::BackendConflict);
        }
        let action = &stored;
        let mut client = self.pool.get().await.map_err(pool_err)?;
        let tx = client.transaction().await.map_err(db_err)?;
        // Readback of an already applied result never consumes its retained raw
        // bytes again and never rewrites the account or credential head.
        if action.receipt()?.confirmed_refresh() {
            return Ok(());
        }
        self.verify_codex_refresh(&tx, action, selected).await?;
        let scope = &action.original.scope.resource;
        let request = self
            .secret_store
            .custody_in_transaction(&tx, scope, &handle(action.original.id, "request")?)
            .await
            .map_err(|_| unavailable())?
            .ok_or(AuthProductError::RefreshFailed)?;
        let SecretCustodyState::Original {
            context: request_context,
        } = &request.state
        else {
            return Err(AuthProductError::BackendConflict);
        };
        let started: DateTime<Utc> = serde_json::from_value(
            request_context
                .get("started_at")
                .cloned()
                .ok_or_else(unavailable)?,
        )
        .map_err(|_| unavailable())?;
        if *request_context
            != serde_json::json!({"refresh":context(action,head),"started_at":started})
        {
            return Err(AuthProductError::BackendConflict);
        }
        let raw = self
            .secret_store
            .custody_in_transaction(&tx, scope, &handle(action.original.id, "response")?)
            .await
            .map_err(|_| unavailable())?
            .ok_or(AuthProductError::RefreshFailed)?;
        if raw.state
            != (SecretCustodyState::Original {
                context: serde_json::json!({"refresh":context(action,head),"request":request.reference}),
            })
        {
            return Err(AuthProductError::BackendConflict);
        }
        let source = SecretCustodySource {
            scope: raw.scope.clone(),
            handle: raw.handle.clone(),
            reference: raw.reference.clone(),
        };
        let retained = self
            .secret_store
            .custody_in_transaction(&tx, scope, &handle(action.original.id, "returned")?)
            .await
            .map_err(|_| unavailable())?
            .ok_or_else(unavailable)?;
        if retained.state != (SecretCustodyState::Retained { source }) {
            return Err(AuthProductError::BackendConflict);
        }
        let lease = self
            .secret_store
            .lease_once_if_record_reference_in_transaction(
                &tx,
                scope,
                &retained.handle,
                &retained.reference,
            )
            .await
            .map_err(|_| unavailable())?
            .ok_or_else(unavailable)?;
        let material = self
            .secret_store
            .consume_in_transaction(&tx, scope, lease.id)
            .await
            .map_err(|_| unavailable())?;
        let response: ResponseOriginal =
            serde_json::from_str(material.expose_secret()).map_err(|_| unavailable())?;
        let parsed = if (200..300).contains(&response.status) {
            brassclaw_llm::parse_codex_device_tokens(&response.body, started).ok()
        } else {
            None
        };
        let outcome = if let Some(tokens) = parsed {
            let destinations = action.destinations.as_ref().ok_or_else(unavailable)?;
            let has_refresh = !tokens.refresh_token.expose_secret().is_empty();
            if self
                .secret_store
                .put_with_expiry_if_record_reference_in_transaction(
                    &tx,
                    scope.clone(),
                    destinations.access_secret.clone(),
                    tokens.access_token,
                    Some(tokens.expires_at),
                    None,
                )
                .await
                .map_err(|_| unavailable())?
                .is_none()
            {
                return Err(AuthProductError::BackendConflict);
            }
            if has_refresh
                && self
                    .secret_store
                    .put_if_record_reference_in_transaction(
                        &tx,
                        scope.clone(),
                        destinations.refresh_secret.clone(),
                        tokens.refresh_token,
                        None,
                    )
                    .await
                    .map_err(|_| unavailable())?
                    .is_none()
            {
                return Err(AuthProductError::BackendConflict);
            }
            // RFC 6749 §6: an omitted refresh_token preserves the original.
            CredentialRefreshOutcome::Returned(OAuthProviderRefresh {
                provider: action.original.provider.clone(),
                access_secret: destinations.access_secret.clone(),
                access_expires_at: Some(tokens.expires_at),
                refresh_secret: has_refresh.then(|| destinations.refresh_secret.clone()),
                scopes: action.expected_account.scopes.clone(),
            })
        } else {
            CredentialRefreshOutcome::Unknown(brassclaw_auth::AuthErrorCode::RefreshFailed)
        };
        self.record_refresh_result_in_transaction(
            &tx,
            CredentialRefreshCompletion {
                dispatched: action.clone(),
                outcome,
            },
        )
        .await?;
        let (published, _) = self
            .reconcile_refresh_action_in_transaction(&tx, &action.original)
            .await?;
        let confirmed = published
            .ok_or_else(unavailable)?
            .receipt()?
            .confirmed_refresh();
        tx.commit().await.map_err(db_err)?;
        if confirmed {
            Ok(())
        } else {
            Err(AuthProductError::RefreshFailed)
        }
    }
}

struct RefreshCustodian<'a> {
    auth: &'a PgAuthProductServices,
    action: &'a CredentialRefreshAction,
    selected: &'a CodexAccessSelection<'a>,
    expected: RuntimeHttpEgressRequest,
    transport: Option<SecretCustodyMetadata>,
}
#[async_trait::async_trait]
impl HostHttpOriginalCustody for RefreshCustodian<'_> {
    async fn authenticate_request(
        &mut self,
        actual: &RuntimeHttpEgressRequest,
    ) -> Result<(), HostHttpOriginalCustodyError> {
        let mut expected = self.expected.clone();
        expected.network_policy = actual.network_policy.clone();
        if self.transport.is_some()
            || expected != *actual
            || !actual.credential_injections.is_empty()
            || actual.save_body_to.is_some()
        {
            return Err(HostHttpOriginalCustodyError);
        }
        let result=async {
            let mut client=self.auth.pool.get().await.map_err(pool_err)?;
            let tx=client.transaction().await.map_err(db_err)?;
            self.auth.verify_codex_refresh(&tx,self.action,self.selected).await?;
            let scope=&self.action.original.scope.resource;
            let destination=handle(self.action.original.id,"request")?;
            // Unlike idempotent storage readback, a second preflight can never
            // grant another transport, even if its bytes are identical.
            if self.auth.secret_store.custody_in_transaction(&tx,scope,&destination).await.map_err(|_|unavailable())?.is_some() {return Err(AuthProductError::BackendConflict)}
            let started_at=Utc::now();
            let request=RequestOriginal {started_at,request:actual.clone()};
            let material=SecretString::from(serde_json::to_string(&request).map_err(|_|unavailable())?);
            let stored=self.auth.secret_store.put_original_in_transaction(&tx,scope.clone(),destination,material,
                serde_json::json!({"refresh":context(self.action,self.selected.head),"started_at":started_at})).await.map_err(|_|unavailable())?.ok_or(AuthProductError::BackendConflict)?;
            tx.commit().await.map_err(db_err)?;
            Ok::<_,AuthProductError>(stored)
        }.await.map_err(|_|HostHttpOriginalCustodyError)?;
        self.transport = Some(result);
        Ok(())
    }
    async fn retain(
        self: Box<Self>,
        original: SecretString,
    ) -> Result<(), HostHttpOriginalCustodyError> {
        let result=async {
            let request=self.transport.as_ref().ok_or_else(unavailable)?;
            let scope=&self.action.original.scope.resource;
            let mut client=self.auth.pool.get().await.map_err(pool_err)?;
            let tx=client.transaction().await.map_err(db_err)?;
            // Retain the actual returned bytes even if a newer login won. This
            // operation publishes no account, role or model-ready success.
            let actual=self.auth.secret_store.custody_in_transaction(&tx,scope,&request.handle).await.map_err(|_|unavailable())?.ok_or_else(unavailable)?;
            if actual != *request {return Err(AuthProductError::BackendConflict)}
            self.auth.secret_store.put_original_and_retained_in_transaction(&tx,
                (scope.clone(),handle(self.action.original.id,"response")?),original,
                serde_json::json!({"refresh":context(self.action,self.selected.head),"request":request.reference}),
                (scope.clone(),handle(self.action.original.id,"returned")?)).await.map_err(|_|unavailable())?.ok_or(AuthProductError::BackendConflict)?;
            tx.commit().await.map_err(db_err)?; Ok::<_,AuthProductError>(())
        }.await;
        result.map_err(|_| HostHttpOriginalCustodyError)
    }
}
