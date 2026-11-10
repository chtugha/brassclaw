//! PRIVATE036: native login publication in the existing provider/Auth owners.
//! Mounted by the LLM-config service with its composed native Auth facade.
//! Issuer protocol/custody remains in PgAuthDevice.
use super::*;
use brassclaw_auth::{
    AuthFlowId, AuthProductScope, AuthSurface, CredentialAccountId, CredentialAccountLabel,
    NativeDeviceLoginDefinition, NativeDeviceLoginOriginal, NativeDeviceOAuthClient,
};
use brassclaw_host_api::{InvocationId, ResourceScope, TenantId};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::provider_codex_credentials::{CodexCredentialBinding, LOGIN_KIND};

/// First exact setup data, encrypted in the existing preparation Original.
/// Neither its decoded shape nor its request fields authorize an issuer call.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CodexLoginContext {
    format: String,
    namespace: String,
    actor: String,
    request: Value,
    definition: NativeDeviceLoginDefinition,
    provider_revision: u64,
    selection_ref: String,
    target_model: String,
    expected_head_reference: Option<String>,
}

fn original_request(request: &SetActiveLlmRequest) -> Value {
    json!({
        "mutation_id":request.mutation_id,
        "provider_id":request.provider_id,
        "expected_revision":request.expected_revision,
        "expected_selection_ref":request.expected_selection_ref,
        "model":request.model,
        "role":request.role,
    })
}

fn login_scope(
    namespace: &str,
    caller: &WebUiAuthenticatedCaller,
    id: uuid::Uuid,
) -> Result<AuthProductScope, LlmConfigServiceError> {
    if id.is_nil() {
        return Err(LlmConfigServiceError::InvalidRequest {
            field: Some("mutation_id".into()),
            reason: "Use a nonempty original login ID.".into(),
        });
    }
    Ok(AuthProductScope::new(
        ResourceScope {
            tenant_id: TenantId::new(namespace).map_err(|_| LlmConfigServiceError::Unavailable)?,
            user_id: caller.user_id.clone(),
            agent_id: None,
            project_id: None,
            thread_id: None,
            invocation_id: InvocationId::from_uuid(id),
        },
        AuthSurface::SetupAdmin,
    ))
}

fn role_key(role: ProviderRole) -> Result<&'static str, LlmConfigServiceError> {
    match role {
        ProviderRole::Kohai => Ok("kohai"),
        ProviderRole::Sempai => Ok("sempai"),
        ProviderRole::Embedding => Err(LlmConfigServiceError::InvalidRequest {
            field: Some("role".into()),
            reason: "Codex device login supports text-model roles.".into(),
        }),
    }
}

impl CodexLoginContext {
    fn verify(
        &self,
        namespace: &str,
        caller: &WebUiAuthenticatedCaller,
        request: &SetActiveLlmRequest,
        original: &NativeDeviceLoginOriginal,
    ) -> Result<(), LlmConfigServiceError> {
        self.definition
            .validate()
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        if self.format != "codex-login-context/1"
            || self.namespace != namespace
            || self.actor != caller.user_id.as_str()
            || self.request != original_request(request)
            || self.definition != original.definition
            || self.definition.flow_id.as_uuid() != request.mutation_id
            || self.definition.provider.as_str() != request.provider_id
            || self.definition.scope != login_scope(namespace, caller, request.mutation_id)?
            || Some(self.provider_revision) != request.expected_revision
            || Some(self.selection_ref.as_str()) != request.expected_selection_ref.as_deref()
            || self.target_model.is_empty()
        {
            return Err(LlmConfigServiceError::Conflict {
                reason: "Login ID belongs to a different original setup request.".into(),
            });
        }
        role_key(request.role.unwrap_or(ProviderRole::Kohai))?;
        Ok(())
    }

    fn mutation(
        &self,
    ) -> Result<crate::provider_mutation::ProviderMutation, LlmConfigServiceError> {
        let binding = CodexCredentialBinding::from_definition(
            &self.namespace,
            self.definition.provider.as_str(),
            &self.definition,
        )?;
        crate::provider_mutation::ProviderMutation::new(
            self.definition.flow_id.as_uuid(),
            "upsert",
            json!({"kind":LOGIN_KIND,"context":self,"credential_binding":binding}),
            None,
        )
    }

    fn material(&self) -> Result<SecretString, LlmConfigServiceError> {
        serde_json::to_string(self)
            .map(SecretString::from)
            .map_err(|_| LlmConfigServiceError::Unavailable)
    }
}

impl RebornLlmConfigService {
    async fn validate_native_codex_target(
        &self,
        role: ProviderRole,
    ) -> Result<(), LlmConfigServiceError> {
        role_key(role)?;
        if role == ProviderRole::Kohai && self.reload.is_none()
            || role == ProviderRole::Sempai && self.sempai_swappable.is_none()
        {
            return Err(LlmConfigServiceError::Unavailable);
        }
        #[cfg(feature = "skills-db")]
        {
            self.bootstrap_runtime()?
                .ok_or(LlmConfigServiceError::Unavailable)?
                .prepare_provider_bootstrap()
                .await?;
            Ok(())
        }
        #[cfg(not(feature = "skills-db"))]
        {
            Err(LlmConfigServiceError::Unavailable)
        }
    }

    fn native_codex_auth(
        &self,
    ) -> Result<Arc<crate::RebornProductAuthServices>, LlmConfigServiceError> {
        self.native_auth
            .upgrade()
            .ok_or(LlmConfigServiceError::Unavailable)
    }

    pub(super) async fn start_native_codex_login(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: SetActiveLlmRequest,
    ) -> Result<CodexLoginStart, LlmConfigServiceError> {
        use brassclaw_auth::{AuthFlowStatus, NativeDeviceOutcome};
        let auth = self.native_codex_auth()?;
        let owner = auth
            .native_codex_owner()
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let scope = login_scope(&self.db_tenant_id, &caller, request.mutation_id)?;
        let flow_id = AuthFlowId::from_uuid(request.mutation_id);
        let existing = owner
            .read_native_device_state(&scope, flow_id)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let (original, context) = match existing {
            Some(state) => {
                let original = state.action.original;
                let context = self
                    .recover_native_codex_context(&caller, &request, &original)
                    .await?;
                (original, context)
            }
            None => {
                // A committed mutation without its original native flow is not
                // an invitation to issue a different device login under its ID.
                match self
                    .provider_mutation(caller.clone(), request.mutation_id)
                    .await
                {
                    Ok(_) => {
                        return Err(LlmConfigServiceError::Conflict {
                            reason: "Login ID already belongs to a committed mutation.".into(),
                        });
                    }
                    Err(LlmConfigServiceError::NotFound) => {}
                    Err(error) => return Err(error),
                }
                let context = self.capture_native_codex_login(&caller, &request).await?;
                match owner
                    .prepare_native_device(context.definition.clone(), context.material()?)
                    .await
                {
                    Ok(action) => (action.original, context),
                    Err(_) => {
                        // Handles both a racing first submission and uncertain
                        // preparation ACK. Errors/absence never authorize I/O.
                        let state = owner
                            .read_native_device_state(&scope, flow_id)
                            .await
                            .map_err(|_| LlmConfigServiceError::Unavailable)?
                            .ok_or(LlmConfigServiceError::Unavailable)?;
                        let original = state.action.original;
                        let context = self
                            .recover_native_codex_context(&caller, &request, &original)
                            .await?;
                        (original, context)
                    }
                }
            }
        };
        context.verify(&self.db_tenant_id, &caller, &request, &original)?;
        let transport = auth
            .native_codex_transport(&original.definition.client)
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let lock = owner
            .native_device_driver_lock(&original)
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let driver = lock.try_lock_owned().ok();
        let state = owner
            .read_native_device_state(&scope, flow_id)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?
            .ok_or(LlmConfigServiceError::Unavailable)?;
        if driver.is_some() && !brassclaw_auth::is_terminal_status(state.flow.status) {
            self.validate_native_codex_target(request.role.unwrap_or(ProviderRole::Kohai))
                .await?;
            match state.action.requests.last().map(|request| &request.outcome) {
                None => {
                    owner
                        .run_native_device_once(&transport, &original, uuid::Uuid::new_v4())
                        .await
                        .map_err(|_| LlmConfigServiceError::Unavailable)?;
                }
                Some(NativeDeviceOutcome::Submitted | NativeDeviceOutcome::Unknown { .. }) => {
                    // With no local driver, a submitted/unknown effect remains
                    // unresolved. A resume does not invent permission to replay.
                    return Err(LlmConfigServiceError::Unavailable);
                }
                _ => {}
            }
        }
        let challenge = owner
            .native_device_challenge(&original)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let limits = auth
            .native_codex_limits()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let state = owner
            .read_native_device_state(&scope, flow_id)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?
            .ok_or(LlmConfigServiceError::Unavailable)?;
        let expires_at = state.action.expiry();
        let (user_code, verification_uri) = match challenge {
            Some(challenge)
                if !brassclaw_auth::is_terminal_status(state.flow.status)
                    && challenge.expires_at == expires_at
                    && expires_at > Utc::now() =>
            {
                (
                    Some(challenge.user_code.expose_secret().to_owned()),
                    Some(challenge.verification_uri),
                )
            }
            _ => (None, None),
        };
        if let Some(driver) = driver
            && !matches!(
                state.flow.status,
                AuthFlowStatus::Completed
                    | AuthFlowStatus::Failed
                    | AuthFlowStatus::Expired
                    | AuthFlowStatus::Canceled
            )
        {
            let worker: Self = self.clone();
            tokio::spawn(async move {
                if worker
                    .drive_native_codex_login(caller, request, original, context, driver)
                    .await
                    .is_err()
                {
                    tracing::warn!(
                        "native Codex original requires status inspection or reconciliation"
                    );
                }
            });
        }
        Ok(CodexLoginStart {
            mutation_id: flow_id.as_uuid(),
            expires_at,
            poll_interval_millis: limits.status_poll_interval_millis,
            user_code,
            verification_uri,
        })
    }

    async fn drive_native_codex_login(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: SetActiveLlmRequest,
        original: NativeDeviceLoginOriginal,
        context: CodexLoginContext,
        _driver: tokio::sync::OwnedMutexGuard<()>,
    ) -> Result<(), LlmConfigServiceError> {
        use brassclaw_auth::NativeDeviceOutcome;
        let auth = self.native_codex_auth()?;
        let owner = auth
            .native_codex_owner()
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let transport = auth
            .native_codex_transport(&original.definition.client)
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        loop {
            let state = owner
                .read_native_device_state(&original.definition.scope, original.definition.flow_id)
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable)?
                .ok_or(LlmConfigServiceError::Unavailable)?;
            if state.action.original != original {
                return Err(LlmConfigServiceError::Unavailable);
            }
            if brassclaw_auth::is_terminal_status(state.flow.status) {
                return Ok(());
            }
            match state.action.requests.last().map(|request| &request.outcome) {
                Some(NativeDeviceOutcome::Tokens { .. }) => {
                    let receipt = self
                        .commit_native_codex_login(&caller, &request, &original, &context)
                        .await?;
                    self.finish_provider_mutation(receipt).await?;
                    return Ok(());
                }
                Some(NativeDeviceOutcome::Submitted | NativeDeviceOutcome::Unknown { .. }) => {
                    return Err(LlmConfigServiceError::Unavailable);
                }
                Some(
                    NativeDeviceOutcome::Challenge { next_poll_at, .. }
                    | NativeDeviceOutcome::Pending { next_poll_at, .. },
                ) => {
                    let now = Utc::now();
                    let next = (*next_poll_at).min(state.action.expiry());
                    if next > now {
                        let delay = next
                            .signed_duration_since(now)
                            .to_std()
                            .map_err(|_| LlmConfigServiceError::Unavailable)?;
                        tokio::time::sleep(delay).await;
                    }
                }
                _ => {}
            }
            owner
                .run_native_device_once(&transport, &original, uuid::Uuid::new_v4())
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable)?;
        }
    }

    pub(super) async fn read_native_codex_login_status(
        &self,
        caller: WebUiAuthenticatedCaller,
        id: uuid::Uuid,
    ) -> Result<brassclaw_product_workflow::CodexLoginStatus, LlmConfigServiceError> {
        use brassclaw_auth::AuthFlowStatus;
        let auth = self.native_codex_auth()?;
        let owner = auth
            .native_codex_owner()
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let state = owner
            .read_native_device_state(
                &login_scope(&self.db_tenant_id, &caller, id)?,
                AuthFlowId::from_uuid(id),
            )
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?
            .ok_or(LlmConfigServiceError::NotFound)?;
        // A completed flow and its receipt commit together. Pending observations
        // never adopt a newer role or infer success from a provider name.
        let mutation_receipt = if state.flow.status == AuthFlowStatus::Completed {
            let receipt = self.provider_mutation(caller, id).await?;
            if receipt.mutation_id != id
                || receipt.operation != "upsert"
                || receipt.provider_id.as_deref() != Some(state.flow.provider.as_str())
            {
                return Err(LlmConfigServiceError::Unavailable);
            }
            Some(receipt)
        } else {
            None
        };
        let limits = auth
            .native_codex_limits()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        Ok(brassclaw_product_workflow::CodexLoginStatus {
            mutation_id: id,
            status: state.flow.status,
            expires_at: state.action.expiry(),
            error: state.flow.error,
            mutation_receipt,
            poll_interval_millis: limits.status_poll_interval_millis,
        })
    }

    /// Only after a true absent original. Capture actual DB selection and the
    /// acknowledged first deadline before retaining preparation or issuer I/O.
    async fn capture_native_codex_login(
        &self,
        caller: &WebUiAuthenticatedCaller,
        request: &SetActiveLlmRequest,
    ) -> Result<CodexLoginContext, LlmConfigServiceError> {
        let id = validate_provider_id(&request.provider_id)?;
        let role = request.role.unwrap_or(ProviderRole::Kohai);
        let key = role_key(role)?;
        let expected_selection = required_selection_ref(request.expected_selection_ref.as_deref())?;
        let expected_revision =
            request
                .expected_revision
                .ok_or_else(|| LlmConfigServiceError::InvalidRequest {
                    field: Some("expected_revision".into()),
                    reason: "Select an exact provider revision before starting login.".into(),
                })?;
        let auth = self.native_codex_auth()?;
        let limits = auth
            .native_codex_limits()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let created_at = Utc::now();
        let expires_at = created_at
            .checked_add_signed(chrono::Duration::seconds(i64::from(
                limits.oauth_setup_default_ttl_secs,
            )))
            .ok_or(LlmConfigServiceError::Unavailable)?;
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
            .get_revision_from_client(&transaction, &id, false)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?
            .ok_or(LlmConfigServiceError::NotFound)?;
        if selected.revision != expected_revision
            || selected.definition.protocol != ProviderProtocol::OpenAiCodex
        {
            return Err(LlmConfigServiceError::Conflict {
                reason: "Provider changed; reload its settings before login.".into(),
            });
        }
        let roles = crate::db_config::provider_role_selections(&transaction, &self.db_tenant_id)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let refs = crate::db_config::model_selection_refs_from_values(&roles, &self.db_tenant_id)
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        if refs.get(key).map(String::as_str) != Some(expected_selection) {
            return Err(LlmConfigServiceError::Conflict {
                reason: "Role selection changed; reload its settings before login.".into(),
            });
        }
        let config = resolve_against_registry_with_credentials(
            &LlmSlotSelection {
                provider_id: Some(id.clone()),
                model: request.model.clone(),
                api_key_env: None,
                base_url: None,
            },
            &ProviderRegistry::new(vec![selected.definition.clone()]),
            None,
        )
        .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let codex = config
            .openai_codex
            .as_ref()
            .ok_or(LlmConfigServiceError::Unavailable)?;
        let native_client = NativeDeviceOAuthClient {
            client_id: codex.client_id.clone(),
            auth_endpoint: codex.auth_endpoint.clone(),
            refresh_endpoint: format!("{}/oauth/token", codex.auth_endpoint.trim_end_matches('/')),
            api_base_url: codex.api_base_url.clone(),
        };
        native_client
            .validate()
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let expected_head_reference = self
            .keys
            .inspect_usage_credential_reference_in_transaction(&transaction, &selected.definition)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        transaction
            .commit()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        drop(client);
        let scope = login_scope(&self.db_tenant_id, caller, request.mutation_id)?;
        let flow_id = AuthFlowId::from_uuid(request.mutation_id);
        let destinations = auth
            .native_codex_transport(&native_client)
            .map_err(|_| LlmConfigServiceError::Unavailable)?
            .native_device_destinations(flow_id, scope.resource.invocation_id)
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let definition = NativeDeviceLoginDefinition {
            scope,
            flow_id,
            account_id: CredentialAccountId::new(),
            provider: brassclaw_auth::AuthProviderId::new(id.clone())
                .map_err(|_| LlmConfigServiceError::Unavailable)?,
            account_label: CredentialAccountLabel::new(id)
                .map_err(|_| LlmConfigServiceError::Unavailable)?,
            client: native_client,
            destinations,
            created_at,
            expires_at,
        };
        definition
            .validate()
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        Ok(CodexLoginContext {
            format: "codex-login-context/1".into(),
            namespace: self.db_tenant_id.clone(),
            actor: caller.user_id.as_str().into(),
            request: original_request(request),
            definition,
            provider_revision: selected.revision,
            selection_ref: expected_selection.into(),
            target_model: config.active_model_name(),
            expected_head_reference,
        })
    }

    async fn recover_native_codex_context(
        &self,
        caller: &WebUiAuthenticatedCaller,
        request: &SetActiveLlmRequest,
        original: &NativeDeviceLoginOriginal,
    ) -> Result<CodexLoginContext, LlmConfigServiceError> {
        let owner = self
            .native_codex_auth()?
            .native_codex_owner()
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let mut client = self
            .pg_pool
            .get()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let transaction = client
            .transaction()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let material = owner
            .native_device_preparation_in_transaction(&transaction, original)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let context: CodexLoginContext = serde_json::from_str(material.expose_secret())
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        context.verify(&self.db_tenant_id, caller, request, original)?;
        transaction
            .commit()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        Ok(context)
    }

    /// Publish the already captured result only. Original receipt lookup comes
    /// before current policy/preparation and before consuming any token return.
    /// This transaction never spans issuer/model I/O.
    async fn commit_native_codex_login(
        &self,
        caller: &WebUiAuthenticatedCaller,
        request: &SetActiveLlmRequest,
        original: &NativeDeviceLoginOriginal,
        context: &CodexLoginContext,
    ) -> Result<brassclaw_product_workflow::ProviderMutationReceipt, LlmConfigServiceError> {
        context.verify(&self.db_tenant_id, caller, request, original)?;
        let mutation = context.mutation()?;
        if let Some(receipt) = self.lookup_provider_mutation(&mutation).await? {
            return Ok(receipt);
        }
        let auth = self.native_codex_auth()?;
        let owner = auth
            .native_codex_owner()
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        #[cfg(feature = "skills-db")]
        let runtime = self
            .bootstrap_runtime()?
            .ok_or(LlmConfigServiceError::Unavailable)?;
        #[cfg(feature = "skills-db")]
        let bootstrap = runtime.prepare_provider_bootstrap().await?;

        let _publication = self.publication.lock().await;
        let mut client = self
            .pg_pool
            .get()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        // Existing model-selection writer requires ReadCommitted. Independent
        // component qualification above has its own read-only RR transaction.
        let transaction = client
            .transaction()
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        mutation.lock(&transaction, &self.db_tenant_id).await?;
        if let Some(receipt) = mutation
            .original(&transaction, &self.db_tenant_id, &self.keys)
            .await?
        {
            transaction
                .commit()
                .await
                .map_err(|_| LlmConfigServiceError::Unavailable)?;
            return Ok(receipt);
        }
        let selected = self
            .pg_provider_repo
            .lock_active_revision(&transaction, original.definition.provider.as_str())
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?
            .ok_or(LlmConfigServiceError::NotFound)?;
        if selected.revision != context.provider_revision
            || selected.definition.protocol != ProviderProtocol::OpenAiCodex
        {
            return Err(LlmConfigServiceError::Conflict {
                reason: "Provider changed during login; inspect the original setup.".into(),
            });
        }
        let role = request.role.unwrap_or(ProviderRole::Kohai);
        crate::db_config::save_model_selection_in_transaction(
            &transaction,
            &self.db_tenant_id,
            role_key(role)?,
            &context.selection_ref,
            Some(original.definition.provider.as_str()),
            Some(&context.target_model),
        )
        .await
        .map_err(map_selection_edit)?;
        let ready = owner
            .native_device_ready_in_transaction(&transaction, original)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        let published = owner
            .publish_codex_account_and_head_in_transaction(
                &transaction,
                &ready,
                &self.db_tenant_id,
                original.definition.provider.as_str(),
                context.expected_head_reference.as_deref(),
            )
            .await?;
        // The actual account-backed descriptor must equal the original metadata
        // used by the mutation proof, not an independently chosen latest login.
        if published.binding
            != CodexCredentialBinding::from_definition(
                &self.db_tenant_id,
                original.definition.provider.as_str(),
                &original.definition,
            )?
        {
            return Err(LlmConfigServiceError::Unavailable);
        }
        #[cfg(feature = "skills-db")]
        let bootstrap_job_id = Some(
            runtime
                .reserve_provider_bootstrap(
                    &transaction,
                    &bootstrap,
                    request.mutation_id,
                    caller,
                    &selected,
                    &context.target_model,
                )
                .await?,
        );
        #[cfg(not(feature = "skills-db"))]
        let bootstrap_job_id = None;
        let receipt = mutation
            .record_with_links(
                &transaction,
                &self.db_tenant_id,
                caller.user_id.as_str(),
                Some((original.definition.provider.as_str(), selected.revision)),
                &self.keys,
                crate::provider_mutation::ProviderMutationLinks {
                    bootstrap_job_id,
                    credential_head_ref: Some(published.head_reference.clone()),
                },
            )
            .await?;
        owner
            .finish_native_device_in_transaction(&transaction, &ready, &published.account)
            .await
            .map_err(|_| LlmConfigServiceError::Unavailable)?;
        match transaction.commit().await {
            Ok(()) => Ok(receipt),
            Err(_) => {
                // Publication ACK uncertainty is resolved from its original
                // immutable receipt; never consume a return or call issuer twice.
                drop(client);
                self.lookup_provider_mutation(&mutation)
                    .await?
                    .ok_or(LlmConfigServiceError::Unavailable)
            }
        }
    }
}
