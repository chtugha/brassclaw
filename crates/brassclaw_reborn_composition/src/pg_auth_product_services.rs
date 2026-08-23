//! PostgreSQL-backed product-auth durable services.
//!
//! Stores auth records (flows, accounts, interactions) in the
//! `brassclaw_product_auth_*` tables using JSONB columns, mirroring the same
//! domain logic and trait surface as `FilesystemAuthProductServices`. Records
//! are serialised/deserialised as JSON; the SQL layer provides CAS via
//! `revision` columns.
//!
//! Migration: the schema is created via `run_migrations()` which executes
//! `CREATE TABLE IF NOT EXISTS` DDL inline (no refinery migration number
//! assigned; these tables are composition-internal to the auth layer).

use std::sync::{Arc, Mutex, Weak};

use async_trait::async_trait;
use brassclaw_auth::domain::{
    account_is_authorized_for_requester, recovery_projection_for_single_account,
    recovery_projection_for_unconfigured_accounts, update_account_from_request,
    validate_account_update_target, validate_callback_claim, validate_credential_status_transition,
    validate_flow_update_binding, validate_manual_token_flow, validate_manual_token_update_binding,
    validate_new_credential_account,
};
use brassclaw_auth::{
    AuthChallenge, AuthFlowId, AuthFlowKind, AuthFlowManager, AuthFlowOwnerScope, AuthFlowRecord,
    AuthFlowRecordSource, AuthFlowStatus, AuthInteractionId, AuthInteractionService,
    AuthProductError, AuthProductScope, AuthProviderId, AuthSurface, CredentialAccount,
    CredentialAccountChoiceRequest, CredentialAccountId, CredentialAccountLabel,
    CredentialAccountListPage, CredentialAccountListRequest, CredentialAccountLookupRequest,
    CredentialAccountMutation, CredentialAccountOwnerScope, CredentialAccountProjection,
    CredentialAccountRecordSource, CredentialAccountSelectionRequest, CredentialAccountService,
    CredentialAccountStatus, CredentialAccountUpdateBinding, CredentialOwnership,
    CredentialRecoveryProjection, CredentialRecoveryReason, CredentialRecoveryRequest,
    CredentialRefreshReport, CredentialRefreshRequest, CredentialSetupService,
    ManualTokenSetupRequest, NewAuthFlow, NewCredentialAccount, OAuthCallbackClaimRequest,
    OAuthCallbackFailureInput, OAuthCallbackInput, ProviderCallbackOutcome, SecretCleanupAction,
    SecretCleanupReport, SecretCleanupRequest, SecretCleanupService, SecretSubmitRequest,
    SecretSubmitResult, TurnGateAuthFlowQuery, credential_status_for_completed_flow,
    flow_matches_turn_gate_query, is_terminal_status, scope_matches,
};
use brassclaw_host_api::SecretHandle;
use brassclaw_secrets::SecretStore;
use chrono::Utc;
use secrecy::ExposeSecret as _;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio_postgres::types::ToSql;

use crate::manual_token_flow::RebornManualTokenFlowService;

type Pool = deadpool_postgres::Pool;

// ---------------------------------------------------------------------------
// DDL
// ---------------------------------------------------------------------------

pub(crate) const MIGRATIONS: &str = r#"
CREATE TABLE IF NOT EXISTS brassclaw_product_auth_accounts (
    id          TEXT NOT NULL,
    tenant_id   TEXT NOT NULL,
    user_id     TEXT NOT NULL,
    revision    BIGINT NOT NULL DEFAULT 0,
    data        JSONB NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, id)
);
CREATE INDEX IF NOT EXISTS brassclaw_product_auth_accounts_tenant_user_idx
    ON brassclaw_product_auth_accounts (tenant_id, user_id);

CREATE TABLE IF NOT EXISTS brassclaw_product_auth_flows (
    id          TEXT NOT NULL,
    tenant_id   TEXT NOT NULL,
    user_id     TEXT NOT NULL,
    revision    BIGINT NOT NULL DEFAULT 0,
    data        JSONB NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, id)
);
CREATE INDEX IF NOT EXISTS brassclaw_product_auth_flows_tenant_user_idx
    ON brassclaw_product_auth_flows (tenant_id, user_id);

CREATE TABLE IF NOT EXISTS brassclaw_product_auth_interactions (
    id          TEXT NOT NULL,
    tenant_id   TEXT NOT NULL,
    user_id     TEXT NOT NULL,
    revision    BIGINT NOT NULL DEFAULT 0,
    data        JSONB NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, id)
);
CREATE INDEX IF NOT EXISTS brassclaw_product_auth_interactions_tenant_user_idx
    ON brassclaw_product_auth_interactions (tenant_id, user_id);
"#;

// ---------------------------------------------------------------------------
// Struct
// ---------------------------------------------------------------------------

/// PostgreSQL-backed implementation of the product-auth durable ports.
pub(crate) struct PgAuthProductServices {
    pool: Arc<Pool>,
    secret_store: Arc<dyn SecretStore>,
    locks: Mutex<std::collections::HashMap<String, Weak<tokio::sync::Mutex<()>>>>,
}

impl PgAuthProductServices {
    pub(crate) fn new(pool: Arc<Pool>, secret_store: Arc<dyn SecretStore>) -> Self {
        Self {
            pool,
            secret_store,
            locks: Mutex::new(std::collections::HashMap::new()),
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

    // ---- generic helpers ----

    /// Tables that may be passed to the generic SQL helpers.
    const ALLOWED_TABLES: &'static [&'static str] = &[
        "brassclaw_product_auth_accounts",
        "brassclaw_product_auth_flows",
        "brassclaw_product_auth_interactions",
    ];

    /// Column names that may be used as filter columns in `list_records`.
    const ALLOWED_FILTER_COLS: &'static [&'static str] = &["user_id"];

    async fn get_record<T: DeserializeOwned>(
        &self,
        table: &str,
        tenant_id: &str,
        id: &str,
    ) -> Result<Option<(T, i64)>, AuthProductError> {
        // Defence-in-depth: reject unknown tables in all builds (debug_assert
        // alone is stripped in --release).
        if !Self::ALLOWED_TABLES.contains(&table) {
            tracing::debug!(
                table,
                "pg_auth_product_services: get_record called with unknown table"
            );
            return Err(AuthProductError::BackendUnavailable);
        }
        let client = self.pool.get().await.map_err(pool_err)?;
        let query = format!("SELECT data, revision FROM {table} WHERE tenant_id = $1 AND id = $2");
        match client
            .query_opt(&query, &[&tenant_id, &id])
            .await
            .map_err(db_err)?
        {
            None => Ok(None),
            Some(row) => {
                let json: serde_json::Value = row.get(0);
                let revision: i64 = row.get(1);
                let record: T = serde_json::from_value(json).map_err(|e| {
                    tracing::debug!(table, error = %e, "pg_auth_product_services: deserialisation failed");
                    AuthProductError::BackendUnavailable
                })?;
                Ok(Some((record, revision)))
            }
        }
    }

    async fn put_record<T: Serialize>(
        &self,
        table: &str,
        tenant_id: &str,
        id: &str,
        extra_cols: &[(&str, &(dyn ToSql + Sync))],
        record: &T,
        expected_revision: Option<i64>,
    ) -> Result<(), AuthProductError> {
        if !Self::ALLOWED_TABLES.contains(&table) {
            tracing::debug!(
                table,
                "pg_auth_product_services: put_record called with unknown table"
            );
            return Err(AuthProductError::BackendUnavailable);
        }
        let json = serde_json::to_value(record).map_err(|e| {
            tracing::debug!(table, error = %e, "pg_auth_product_services: serialisation failed");
            AuthProductError::BackendUnavailable
        })?;
        let client = self.pool.get().await.map_err(pool_err)?;

        let extra_names: Vec<&str> = extra_cols.iter().map(|(n, _)| *n).collect();
        let extra_vals: Vec<&(dyn ToSql + Sync)> = extra_cols.iter().map(|(_, v)| *v).collect();

        match expected_revision {
            None => {
                let col_list: String = extra_names
                    .iter()
                    .enumerate()
                    .map(|(i, name)| format!(", {name} = ${}", i + 4))
                    .collect();
                let mut params: Vec<&(dyn ToSql + Sync)> = vec![&tenant_id, &id, &json];
                params.extend(extra_vals.iter());
                let query = format!(
                    "INSERT INTO {table} (tenant_id, id, data{extra_insert}) \
                     VALUES ($1, $2, $3{extra_placeholders}) \
                     ON CONFLICT (tenant_id, id) DO UPDATE \
                     SET data = EXCLUDED.data, revision = {table}.revision + 1, \
                     updated_at = NOW(){col_list}",
                    extra_insert = extra_names
                        .iter()
                        .map(|n| format!(", {n}"))
                        .collect::<String>(),
                    extra_placeholders = (4..=3 + extra_names.len())
                        .map(|i| format!(", ${i}"))
                        .collect::<String>(),
                );
                client.execute(&query, &params).await.map_err(db_err)?;
            }
            Some(rev) => {
                let col_list: String = extra_names
                    .iter()
                    .enumerate()
                    .map(|(i, name)| format!(", {name} = ${}", i + 4))
                    .collect();
                let mut params: Vec<&(dyn ToSql + Sync)> = vec![&tenant_id, &id, &json];
                params.extend(extra_vals.iter());
                let rev_param_idx = 4 + extra_names.len();
                let query = format!(
                    "UPDATE {table} SET data = $3, revision = revision + 1, \
                     updated_at = NOW(){col_list} \
                     WHERE tenant_id = $1 AND id = $2 AND revision = ${rev_param_idx}"
                );
                params.push(&rev);
                let rows = client.execute(&query, &params).await.map_err(db_err)?;
                if rows == 0 {
                    return Err(AuthProductError::BackendConflict);
                }
            }
        }
        Ok(())
    }

    async fn delete_record(
        &self,
        table: &str,
        tenant_id: &str,
        id: &str,
    ) -> Result<bool, AuthProductError> {
        if !Self::ALLOWED_TABLES.contains(&table) {
            tracing::debug!(
                table,
                "pg_auth_product_services: delete_record called with unknown table"
            );
            return Err(AuthProductError::BackendUnavailable);
        }
        let client = self.pool.get().await.map_err(pool_err)?;
        let query = format!("DELETE FROM {table} WHERE tenant_id = $1 AND id = $2");
        let n = client
            .execute(&query, &[&tenant_id, &id])
            .await
            .map_err(db_err)?;
        Ok(n > 0)
    }

    async fn list_records<T: DeserializeOwned>(
        &self,
        table: &str,
        tenant_id: &str,
        filter_col: &str,
        filter_val: &str,
    ) -> Result<Vec<T>, AuthProductError> {
        if !Self::ALLOWED_TABLES.contains(&table) {
            tracing::debug!(
                table,
                "pg_auth_product_services: list_records called with unknown table"
            );
            return Err(AuthProductError::BackendUnavailable);
        }
        if !Self::ALLOWED_FILTER_COLS.contains(&filter_col) {
            tracing::debug!(
                filter_col,
                "pg_auth_product_services: list_records called with unknown filter column"
            );
            return Err(AuthProductError::BackendUnavailable);
        }
        let client = self.pool.get().await.map_err(pool_err)?;
        let query = format!(
            "SELECT data FROM {table} WHERE tenant_id = $1 AND {filter_col} = $2 \
             ORDER BY created_at ASC"
        );
        let rows = client
            .query(&query, &[&tenant_id, &filter_val])
            .await
            .map_err(db_err)?;
        rows.iter()
            .map(|row| {
                let json: serde_json::Value = row.get(0);
                serde_json::from_value(json).map_err(|e| {
                    tracing::debug!(table, filter_col, error = %e, "pg_auth_product_services: deserialisation failed");
                    AuthProductError::BackendUnavailable
                })
            })
            .collect()
    }

    // ---- account helpers ----

    async fn read_account(
        &self,
        scope: &AuthProductScope,
        account_id: CredentialAccountId,
    ) -> Result<Option<(CredentialAccount, i64)>, AuthProductError> {
        let tenant_id = scope.resource.tenant_id.as_str().to_string();
        let id = account_id.to_string();
        self.get_record("brassclaw_product_auth_accounts", &tenant_id, &id)
            .await
    }

    async fn write_account(
        &self,
        account: &CredentialAccount,
        expected_revision: Option<i64>,
    ) -> Result<(), AuthProductError> {
        let tenant_id = account.scope.resource.tenant_id.as_str().to_string();
        let user_id = account.scope.resource.user_id.as_str().to_string();
        let id = account.id.to_string();
        let user_id_ref: &str = &user_id;
        self.put_record(
            "brassclaw_product_auth_accounts",
            &tenant_id,
            &id,
            &[("user_id", &user_id_ref as &(dyn ToSql + Sync))],
            account,
            expected_revision,
        )
        .await
    }

    async fn accounts_for_scope(
        &self,
        scope: &AuthProductScope,
    ) -> Result<Vec<CredentialAccount>, AuthProductError> {
        let tenant_id = scope.resource.tenant_id.as_str().to_string();
        let user_id = scope.resource.user_id.as_str().to_string();
        self.list_records(
            "brassclaw_product_auth_accounts",
            &tenant_id,
            "user_id",
            &user_id,
        )
        .await
    }

    async fn accounts_for_owner(
        &self,
        owner: &CredentialAccountOwnerScope,
    ) -> Result<Vec<CredentialAccount>, AuthProductError> {
        let tenant_id = owner.tenant_id.as_str().to_string();
        let user_id = owner.user_id.as_str().to_string();
        let mut accounts: Vec<CredentialAccount> = self
            .list_records(
                "brassclaw_product_auth_accounts",
                &tenant_id,
                "user_id",
                &user_id,
            )
            .await?;
        accounts.retain(|account| owner.matches(account));
        accounts.sort_by_key(|account| account.id);
        Ok(accounts)
    }

    // ---- flow helpers ----

    async fn read_flow(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
    ) -> Result<Option<(AuthFlowRecord, i64)>, AuthProductError> {
        let tenant_id = scope.resource.tenant_id.as_str().to_string();
        let id = flow_id.to_string();
        self.get_record("brassclaw_product_auth_flows", &tenant_id, &id)
            .await
    }

    async fn write_flow(
        &self,
        flow: &AuthFlowRecord,
        expected_revision: Option<i64>,
    ) -> Result<(), AuthProductError> {
        let tenant_id = flow.scope.resource.tenant_id.as_str().to_string();
        let user_id = flow.scope.resource.user_id.as_str().to_string();
        let id = flow.id.to_string();
        let user_id_ref: &str = &user_id;
        self.put_record(
            "brassclaw_product_auth_flows",
            &tenant_id,
            &id,
            &[("user_id", &user_id_ref as &(dyn ToSql + Sync))],
            flow,
            expected_revision,
        )
        .await
    }

    async fn flows_for_scope(
        &self,
        scope: &AuthProductScope,
    ) -> Result<Vec<AuthFlowRecord>, AuthProductError> {
        let tenant_id = scope.resource.tenant_id.as_str().to_string();
        let user_id = scope.resource.user_id.as_str().to_string();
        self.list_records(
            "brassclaw_product_auth_flows",
            &tenant_id,
            "user_id",
            &user_id,
        )
        .await
    }

    // ---- interaction helpers ----

    async fn read_interaction(
        &self,
        scope: &AuthProductScope,
        interaction_id: AuthInteractionId,
    ) -> Result<Option<(StoredManualTokenInteraction, i64)>, AuthProductError> {
        let tenant_id = scope.resource.tenant_id.as_str().to_string();
        let id = interaction_id.to_string();
        self.get_record("brassclaw_product_auth_interactions", &tenant_id, &id)
            .await
    }

    async fn write_interaction(
        &self,
        interaction: &StoredManualTokenInteraction,
        expected_revision: Option<i64>,
    ) -> Result<(), AuthProductError> {
        let tenant_id = interaction.scope.resource.tenant_id.as_str().to_string();
        let user_id = interaction.scope.resource.user_id.as_str().to_string();
        let id = interaction.id.to_string();
        let user_id_ref: &str = &user_id;
        self.put_record(
            "brassclaw_product_auth_interactions",
            &tenant_id,
            &id,
            &[("user_id", &user_id_ref as &(dyn ToSql + Sync))],
            interaction,
            expected_revision,
        )
        .await
    }

    async fn delete_interaction(
        &self,
        scope: &AuthProductScope,
        interaction_id: AuthInteractionId,
    ) -> Result<bool, AuthProductError> {
        let tenant_id = scope.resource.tenant_id.as_str().to_string();
        let id = interaction_id.to_string();
        self.delete_record("brassclaw_product_auth_interactions", &tenant_id, &id)
            .await
    }

    async fn cleanup_secret(
        &self,
        scope: &brassclaw_host_api::ResourceScope,
        handle: &Option<SecretHandle>,
    ) {
        if let Some(h) = handle
            && let Err(e) = self.secret_store.delete(scope, h).await
        {
            tracing::debug!(error = %e, "pg_auth_product_services: best-effort secret delete failed; orphaned handle is unreachable via account record");
        }
    }
}

// ---------------------------------------------------------------------------
// Error helpers
// ---------------------------------------------------------------------------

fn pool_err(_e: deadpool_postgres::PoolError) -> AuthProductError {
    AuthProductError::BackendUnavailable
}

fn db_err(_e: tokio_postgres::Error) -> AuthProductError {
    AuthProductError::BackendUnavailable
}

// ---------------------------------------------------------------------------
// Stored interaction type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredManualTokenInteraction {
    id: AuthInteractionId,
    scope: AuthProductScope,
    provider: AuthProviderId,
    label: CredentialAccountLabel,
    continuation: brassclaw_auth::AuthContinuationRef,
    update_binding: Option<CredentialAccountUpdateBinding>,
    expires_at: brassclaw_auth::Timestamp,
    consumed_at: Option<brassclaw_auth::Timestamp>,
}

// ---------------------------------------------------------------------------
// AuthFlowManager
// ---------------------------------------------------------------------------

#[async_trait]
impl AuthFlowManager for PgAuthProductServices {
    async fn create_flow(&self, request: NewAuthFlow) -> Result<AuthFlowRecord, AuthProductError> {
        if let Some(binding) = &request.update_binding {
            let scope =
                AuthProductScope::new(request.scope.resource.clone(), request.scope.surface);
            let account = self
                .read_account(&scope, binding.account_id)
                .await?
                .map(|(account, _)| account)
                .ok_or(AuthProductError::CredentialMissing)?;
            validate_flow_update_binding(&account, &request)?;
        }
        let now = Utc::now();
        let record = AuthFlowRecord {
            id: request.id.unwrap_or_default(),
            scope: request.scope,
            kind: request.kind,
            status: AuthFlowStatus::AwaitingUser,
            provider: request.provider,
            challenge: Some(request.challenge),
            continuation: request.continuation,
            credential_account_id: None,
            update_binding: request.update_binding,
            opaque_state_hash: request.opaque_state_hash,
            pkce_verifier_hash: request.pkce_verifier_hash,
            authorization_code_hash: None,
            error: None,
            continuation_emitted_at: None,
            created_at: now,
            updated_at: now,
            expires_at: request.expires_at,
        };
        self.write_flow(&record, None).await?;
        Ok(record)
    }

    async fn get_flow(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
        Ok(self.read_flow(scope, flow_id).await?.map(|(f, _)| f))
    }

    async fn claim_oauth_callback(
        &self,
        scope: &AuthProductScope,
        request: OAuthCallbackClaimRequest,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let lock = self.lock_for(format!("flow:{}", request.flow_id));
        let _guard = lock.lock().await;
        let now = Utc::now();
        let (mut record, rev) = self
            .read_flow(scope, request.flow_id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        match validate_callback_claim(&mut record, scope, &request, now) {
            Ok(()) => {}
            Err(AuthProductError::UnknownOrExpiredFlow) => {
                self.write_flow(&record, Some(rev)).await?;
                return Err(AuthProductError::UnknownOrExpiredFlow);
            }
            Err(error) => return Err(error),
        }
        if record.status == AuthFlowStatus::Completed {
            return Ok(record);
        }
        record.status = AuthFlowStatus::CallbackReceived;
        record.updated_at = now;
        self.write_flow(&record, Some(rev)).await?;
        Ok(record)
    }

    async fn complete_oauth_callback(
        &self,
        scope: &AuthProductScope,
        input: OAuthCallbackInput,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let lock = self.lock_for(format!("flow:{}", input.flow_id));
        let _guard = lock.lock().await;
        let now = Utc::now();
        let (mut flow, flow_rev) = self
            .read_flow(scope, input.flow_id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        if is_terminal_status(flow.status) || flow.status != AuthFlowStatus::CallbackReceived {
            return Err(AuthProductError::UnknownOrExpiredFlow);
        }
        let account = match &input.outcome {
            ProviderCallbackOutcome::Authorized { exchange } => {
                let account_id = flow
                    .update_binding
                    .as_ref()
                    .map(|b| b.account_id)
                    .or(exchange.account_id)
                    .unwrap_or_else(CredentialAccountId::new);
                let ownership = flow
                    .update_binding
                    .as_ref()
                    .map(|b| b.ownership)
                    .unwrap_or(CredentialOwnership::UserReusable);
                let owner_extension = flow
                    .update_binding
                    .as_ref()
                    .and_then(|b| b.owner_extension.clone());
                let granted_extensions = flow
                    .update_binding
                    .as_ref()
                    .map(|b| b.granted_extensions.clone())
                    .unwrap_or_default();
                let new_account = NewCredentialAccount {
                    scope: flow.scope.clone(),
                    provider: flow.provider.clone(),
                    label: exchange.account_label.clone(),
                    status: credential_status_for_completed_flow(),
                    ownership,
                    owner_extension,
                    granted_extensions,
                    access_secret: Some(exchange.access_secret.clone()),
                    refresh_secret: exchange.refresh_secret.clone(),
                    scopes: exchange.scopes.clone(),
                };
                validate_new_credential_account(&new_account)?;
                let account = CredentialAccount {
                    id: account_id,
                    scope: new_account.scope,
                    provider: new_account.provider,
                    label: new_account.label,
                    status: new_account.status,
                    ownership: new_account.ownership,
                    owner_extension: new_account.owner_extension,
                    granted_extensions: new_account.granted_extensions,
                    access_secret: new_account.access_secret,
                    refresh_secret: new_account.refresh_secret,
                    scopes: new_account.scopes,
                    created_at: now,
                    updated_at: now,
                };
                let existing_rev = self
                    .read_account(scope, account.id)
                    .await?
                    .map(|(_, rev)| rev);
                self.write_account(&account, existing_rev).await?;
                flow.credential_account_id = Some(account.id);
                account
            }
            ProviderCallbackOutcome::Denied => {
                flow.status = AuthFlowStatus::Failed;
                flow.updated_at = now;
                self.write_flow(&flow, Some(flow_rev)).await?;
                return Ok(flow);
            }
        };
        flow.status = AuthFlowStatus::Completed;
        flow.credential_account_id = Some(account.id);
        flow.updated_at = now;
        self.write_flow(&flow, Some(flow_rev)).await?;
        Ok(flow)
    }

    async fn complete_credential_selection(
        &self,
        scope: &AuthProductScope,
        input: brassclaw_auth::CredentialSelectionInput,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let lock = self.lock_for(format!("flow:{}", input.flow_id));
        let _guard = lock.lock().await;
        let now = Utc::now();
        let (mut flow, flow_rev) = self
            .read_flow(scope, input.flow_id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        if is_terminal_status(flow.status) {
            return Err(AuthProductError::UnknownOrExpiredFlow);
        }
        if flow.kind != AuthFlowKind::IntegrationCredential {
            return Err(AuthProductError::InvalidRequest {
                reason: "flow is not a credential selection flow".to_string(),
            });
        }
        flow.status = AuthFlowStatus::Completed;
        flow.credential_account_id = Some(input.credential_account_id);
        flow.updated_at = now;
        self.write_flow(&flow, Some(flow_rev)).await?;
        Ok(flow)
    }

    async fn complete_manual_token(
        &self,
        scope: &AuthProductScope,
        input: brassclaw_auth::ManualTokenCompletionInput,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        // Scan flows to find the one whose challenge references this interaction.
        let flow_id = self
            .flows_for_scope(scope)
            .await?
            .into_iter()
            .find_map(|flow| {
                let matches = matches!(
                    &flow.challenge,
                    Some(AuthChallenge::ManualTokenRequired { interaction_id, .. })
                        if interaction_id == &input.interaction_id
                );
                matches.then_some(flow.id)
            })
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        let lock = self.lock_for(format!("flow:{flow_id}"));
        let _guard = lock.lock().await;
        let now = Utc::now();
        let (mut record, rev) = self
            .read_flow(scope, flow_id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        match validate_manual_token_flow(&mut record, scope, &input, now) {
            Ok(()) => {}
            Err(AuthProductError::UnknownOrExpiredFlow) => {
                self.write_flow(&record, Some(rev)).await?;
                return Err(AuthProductError::UnknownOrExpiredFlow);
            }
            Err(error) => return Err(error),
        }
        if record.status == AuthFlowStatus::Completed {
            return Ok(record);
        }
        let account = self
            .read_account(scope, input.credential_account_id)
            .await?
            .map(|(a, _)| a)
            .ok_or(AuthProductError::CredentialMissing)?;
        if !scope_matches(&record.scope, &account.scope)
            || account.provider != record.provider
            || account.status != CredentialAccountStatus::Configured
        {
            return Err(AuthProductError::CrossScopeDenied);
        }
        record.status = AuthFlowStatus::Completed;
        record.error = None;
        record.credential_account_id = Some(input.credential_account_id);
        record.updated_at = now;
        self.write_flow(&record, Some(rev)).await?;
        Ok(record)
    }

    async fn cancel_manual_token(
        &self,
        scope: &AuthProductScope,
        interaction_id: AuthInteractionId,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
        // Best-effort: find and cancel flows referencing this interaction.
        let flows = self.flows_for_scope(scope).await?;
        for mut flow in flows {
            if is_terminal_status(flow.status) {
                continue;
            }
            if flow.kind != AuthFlowKind::IntegrationCredential {
                continue;
            }
            // There is no direct interaction_id field on AuthFlowRecord;
            // the interaction is linked through the challenge. Cancel any
            // active manual token flow for this scope.
            let (_, rev) = match self.read_flow(scope, flow.id).await? {
                Some(pair) => pair,
                None => continue,
            };
            flow.status = AuthFlowStatus::Canceled;
            flow.updated_at = Utc::now();
            if let Err(e) = self.write_flow(&flow, Some(rev)).await {
                tracing::debug!(error = %e, "pg_auth_product_services: best-effort canceled-flow write failed");
            }
            return Ok(Some(flow));
        }
        // Also clean up the interaction record.
        if let Err(e) = self.delete_interaction(scope, interaction_id).await {
            tracing::debug!(error = %e, "pg_auth_product_services: best-effort interaction delete failed");
        }
        Ok(None)
    }

    async fn fail_oauth_callback(
        &self,
        scope: &AuthProductScope,
        input: OAuthCallbackFailureInput,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let lock = self.lock_for(format!("flow:{}", input.flow_id));
        let _guard = lock.lock().await;
        let now = Utc::now();
        let (mut flow, rev) = self
            .read_flow(scope, input.flow_id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        if is_terminal_status(flow.status) {
            return Err(AuthProductError::UnknownOrExpiredFlow);
        }
        flow.status = AuthFlowStatus::Failed;
        flow.error = Some(input.error);
        flow.updated_at = now;
        self.write_flow(&flow, Some(rev)).await?;
        Ok(flow)
    }

    async fn mark_continuation_dispatched(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
        emitted_at: brassclaw_auth::Timestamp,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let lock = self.lock_for(format!("flow:{flow_id}"));
        let _guard = lock.lock().await;
        let (mut flow, rev) = self
            .read_flow(scope, flow_id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        flow.continuation_emitted_at = Some(emitted_at);
        flow.updated_at = Utc::now();
        self.write_flow(&flow, Some(rev)).await?;
        Ok(flow)
    }

    async fn cancel_flow(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let lock = self.lock_for(format!("flow:{flow_id}"));
        let _guard = lock.lock().await;
        let (mut flow, rev) = self
            .read_flow(scope, flow_id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        if is_terminal_status(flow.status) {
            return Ok(flow);
        }
        flow.status = AuthFlowStatus::Canceled;
        flow.updated_at = Utc::now();
        self.write_flow(&flow, Some(rev)).await?;
        Ok(flow)
    }
}

// ---------------------------------------------------------------------------
// AuthFlowRecordSource
// ---------------------------------------------------------------------------

#[async_trait]
impl AuthFlowRecordSource for PgAuthProductServices {
    async fn flow_for_turn_gate(
        &self,
        query: TurnGateAuthFlowQuery,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
        // Build a minimal scope from the owner to query flows.
        let resource = brassclaw_host_api::ResourceScope {
            tenant_id: query.owner.tenant_id.clone(),
            user_id: query.owner.user_id.clone(),
            agent_id: query.owner.agent_id.clone(),
            project_id: query.owner.project_id.clone(),
            thread_id: Some(query.owner.thread_id.clone()),
            invocation_id: brassclaw_host_api::InvocationId::new(),
        };
        for surface in AuthSurface::ALL {
            let scope = AuthProductScope::new(resource.clone(), surface);
            let flows = self.flows_for_scope(&scope).await?;
            for flow in flows {
                if flow_matches_turn_gate_query(&flow, &query) {
                    return Ok(Some(flow));
                }
            }
        }
        Ok(None)
    }

    async fn flows_for_owner(
        &self,
        owner: AuthFlowOwnerScope,
    ) -> Result<Vec<AuthFlowRecord>, AuthProductError> {
        let resource = brassclaw_host_api::ResourceScope {
            tenant_id: owner.tenant_id.clone(),
            user_id: owner.user_id.clone(),
            agent_id: owner.agent_id.clone(),
            project_id: owner.project_id.clone(),
            thread_id: Some(owner.thread_id.clone()),
            invocation_id: brassclaw_host_api::InvocationId::new(),
        };
        let mut results = Vec::new();
        for surface in AuthSurface::ALL {
            let scope = AuthProductScope::new(resource.clone(), surface);
            let flows = self.flows_for_scope(&scope).await?;
            results.extend(flows.into_iter().filter(|flow| owner.matches(flow)));
        }
        results.sort_by_key(|flow| flow.id);
        results.dedup_by_key(|flow| flow.id);
        Ok(results)
    }
}

// ---------------------------------------------------------------------------
// AuthInteractionService
// ---------------------------------------------------------------------------

#[async_trait]
impl brassclaw_auth::AuthInteractionService for PgAuthProductServices {
    async fn request_secret_input(
        &self,
        request: ManualTokenSetupRequest,
    ) -> Result<AuthChallenge, AuthProductError> {
        if let Some(binding) = &request.update_binding {
            let scope =
                AuthProductScope::new(request.scope.resource.clone(), request.scope.surface);
            let account = self
                .read_account(&scope, binding.account_id)
                .await?
                .map(|(account, _)| account)
                .ok_or(AuthProductError::CredentialMissing)?;
            validate_manual_token_update_binding(&account, &request, binding)?;
        }
        let interaction = StoredManualTokenInteraction {
            id: AuthInteractionId::new(),
            scope: request.scope.clone(),
            provider: request.provider.clone(),
            label: request.label.clone(),
            continuation: request.continuation,
            update_binding: request.update_binding,
            expires_at: request.expires_at,
            consumed_at: None,
        };
        self.write_interaction(&interaction, None).await?;
        Ok(AuthChallenge::ManualTokenRequired {
            interaction_id: interaction.id,
            provider: request.provider,
            label: request.label,
            expires_at: request.expires_at,
        })
    }

    async fn submit_manual_token(
        &self,
        scope: &AuthProductScope,
        request: SecretSubmitRequest,
    ) -> Result<SecretSubmitResult, AuthProductError> {
        validate_secret(&request)?;
        let lock = self.lock_for(format!("interaction:{}", request.interaction_id));
        let _guard = lock.lock().await;
        let (mut pending, rev) = self
            .read_interaction(scope, request.interaction_id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        if !brassclaw_auth::scope_matches(scope, &pending.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let now = Utc::now();
        if pending.consumed_at.is_some() || now > pending.expires_at {
            return Err(AuthProductError::UnknownOrExpiredFlow);
        }
        let continuation = pending.continuation.clone();
        // Build or update credential account.
        let account_id = pending
            .update_binding
            .as_ref()
            .map(|b| b.account_id)
            .unwrap_or_else(|| CredentialAccountId::from_uuid(pending.id.as_uuid()));
        let access_handle = SecretHandle::new(format!(
            "product-auth-manual-{account_id}-{pending_id}",
            pending_id = pending.id
        ))
        .map_err(|_| AuthProductError::BackendUnavailable)?;
        let ownership = pending
            .update_binding
            .as_ref()
            .map(|b| b.ownership)
            .unwrap_or(CredentialOwnership::UserReusable);
        let owner_extension = pending
            .update_binding
            .as_ref()
            .and_then(|b| b.owner_extension.clone());
        let granted_extensions = pending
            .update_binding
            .as_ref()
            .map(|b| b.granted_extensions.clone())
            .unwrap_or_default();
        let account_scope = pending.scope.clone();
        let new_account = NewCredentialAccount {
            scope: account_scope,
            provider: pending.provider.clone(),
            label: pending.label.clone(),
            status: credential_status_for_completed_flow(),
            ownership,
            owner_extension,
            granted_extensions,
            access_secret: Some(access_handle.clone()),
            refresh_secret: None,
            scopes: Vec::new(),
        };
        validate_new_credential_account(&new_account)?;
        self.secret_store
            .put(
                pending.scope.resource.clone(),
                access_handle.clone(),
                request.secret,
            )
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?;
        let (existing_rev, existing) = match self.read_account(scope, account_id).await? {
            Some((existing, rev)) => (Some(rev), Some(existing)),
            None => (None, None),
        };
        let account = if let (Some(mut existing), Some(existing_rev)) = (existing, existing_rev) {
            validate_account_update_target(&existing, &new_account)?;
            let previous_access = existing.access_secret.clone();
            update_account_from_request(&mut existing, new_account, now)?;
            if let Err(error) = self.write_account(&existing, Some(existing_rev)).await {
                self.cleanup_secret(&pending.scope.resource, &Some(access_handle))
                    .await;
                return Err(error);
            }
            if previous_access.as_ref() != existing.access_secret.as_ref() {
                self.cleanup_secret(&pending.scope.resource, &previous_access)
                    .await;
            }
            existing
        } else {
            let account = CredentialAccount {
                id: account_id,
                scope: new_account.scope,
                provider: new_account.provider,
                label: new_account.label,
                status: new_account.status,
                ownership: new_account.ownership,
                owner_extension: new_account.owner_extension,
                granted_extensions: new_account.granted_extensions,
                access_secret: new_account.access_secret,
                refresh_secret: new_account.refresh_secret,
                scopes: new_account.scopes,
                created_at: now,
                updated_at: now,
            };
            if let Err(error) = self.write_account(&account, None).await {
                self.cleanup_secret(&pending.scope.resource, &account.access_secret)
                    .await;
                return Err(error);
            }
            account
        };
        // Mark interaction consumed.
        pending.consumed_at = Some(now);
        self.write_interaction(&pending, Some(rev)).await?;
        Ok(SecretSubmitResult {
            account_id: account.id,
            status: account.status,
            continuation,
        })
    }

    async fn abandon_manual_token(
        &self,
        scope: &AuthProductScope,
        interaction_id: AuthInteractionId,
    ) -> Result<bool, AuthProductError> {
        self.delete_interaction(scope, interaction_id).await
    }
}

// ---------------------------------------------------------------------------
// CredentialAccountService
// ---------------------------------------------------------------------------

#[async_trait]
impl CredentialAccountService for PgAuthProductServices {
    async fn create_account(
        &self,
        request: NewCredentialAccount,
    ) -> Result<CredentialAccount, AuthProductError> {
        validate_new_credential_account(&request)?;
        let now = Utc::now();
        let account = CredentialAccount {
            id: CredentialAccountId::new(),
            scope: request.scope,
            provider: request.provider,
            label: request.label,
            status: request.status,
            ownership: request.ownership,
            owner_extension: request.owner_extension,
            granted_extensions: request.granted_extensions,
            access_secret: request.access_secret,
            refresh_secret: request.refresh_secret,
            scopes: request.scopes,
            created_at: now,
            updated_at: now,
        };
        self.write_account(&account, None).await?;
        Ok(account)
    }

    async fn get_account(
        &self,
        request: CredentialAccountLookupRequest,
    ) -> Result<Option<CredentialAccount>, AuthProductError> {
        let Some((account, _)) = self
            .read_account(&request.scope, request.account_id)
            .await?
        else {
            return Ok(None);
        };
        if !brassclaw_auth::scope_matches(&request.scope, &account.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if !account_is_authorized_for_requester(&account, request.requester_extension.as_ref()) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        Ok(Some(account))
    }

    async fn list_accounts(
        &self,
        request: CredentialAccountListRequest,
    ) -> Result<CredentialAccountListPage, AuthProductError> {
        request.validate()?;
        let mut accounts = self
            .accounts_for_scope(&request.scope)
            .await?
            .into_iter()
            .filter(|account| {
                account.provider == request.provider
                    && request.cursor.is_none_or(|cursor| account.id > cursor)
                    && account_is_authorized_for_requester(
                        account,
                        request.requester_extension.as_ref(),
                    )
            })
            .map(|account| account.projection())
            .collect::<Vec<_>>();
        accounts.sort_by_key(|account| account.id);
        let next_cursor = if accounts.len() > request.limit {
            accounts.truncate(request.limit);
            accounts.last().map(|account| account.id)
        } else {
            None
        };
        Ok(CredentialAccountListPage {
            accounts,
            next_cursor,
        })
    }

    async fn update_status(
        &self,
        scope: &AuthProductScope,
        account_id: CredentialAccountId,
        status: CredentialAccountStatus,
    ) -> Result<CredentialAccount, AuthProductError> {
        let lock = self.lock_for(format!("account:{account_id}"));
        let _guard = lock.lock().await;
        let (mut account, rev) = self
            .read_account(scope, account_id)
            .await?
            .ok_or(AuthProductError::CredentialMissing)?;
        if !brassclaw_auth::scope_matches(scope, &account.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        validate_credential_status_transition(account.status, status)?;
        account.status = status;
        account.updated_at = Utc::now();
        self.write_account(&account, Some(rev)).await?;
        Ok(account)
    }

    async fn select_unique_configured_account(
        &self,
        request: CredentialAccountSelectionRequest,
    ) -> Result<CredentialAccountProjection, AuthProductError> {
        let configured = self
            .accounts_for_scope(&request.scope)
            .await?
            .into_iter()
            .filter(|account| {
                account.provider == request.provider
                    && account.status == CredentialAccountStatus::Configured
            })
            .collect::<Vec<_>>();
        if configured.is_empty() {
            return Err(AuthProductError::CredentialMissing);
        }
        let selectable = configured
            .iter()
            .filter(|account| {
                account_is_authorized_for_requester(account, request.requester_extension.as_ref())
            })
            .collect::<Vec<_>>();
        match selectable.as_slice() {
            [] => Err(AuthProductError::CrossScopeDenied),
            [account] => Ok(account.projection()),
            _ => Err(AuthProductError::AccountSelectionRequired),
        }
    }

    async fn project_credential_recovery(
        &self,
        request: CredentialRecoveryRequest,
    ) -> Result<CredentialRecoveryProjection, AuthProductError> {
        let mut accounts = self
            .accounts_for_scope(&request.scope)
            .await?
            .into_iter()
            .filter(|account| account.provider == request.provider)
            .collect::<Vec<_>>();
        accounts.sort_by_key(|account| account.id);
        if accounts.is_empty() {
            return Ok(CredentialRecoveryProjection::setup_required(
                request.provider,
                CredentialRecoveryReason::NoAccount,
                Vec::new(),
            ));
        }
        let authorized = accounts
            .iter()
            .filter(|account| {
                account_is_authorized_for_requester(account, request.requester_extension.as_ref())
            })
            .collect::<Vec<_>>();
        if authorized.is_empty() {
            return Ok(CredentialRecoveryProjection::setup_required(
                request.provider,
                CredentialRecoveryReason::NoAccount,
                Vec::new(),
            ));
        }
        let configured = authorized
            .iter()
            .copied()
            .filter(|account| account.status == CredentialAccountStatus::Configured)
            .collect::<Vec<_>>();
        match configured.as_slice() {
            [account] => {
                return Ok(CredentialRecoveryProjection::configured(
                    request.provider,
                    account.projection(),
                ));
            }
            [_, ..] => {
                return Ok(CredentialRecoveryProjection::account_selection_required(
                    request.provider,
                    configured.iter().map(|a| a.projection()).collect(),
                ));
            }
            [] => {}
        }
        if let [account] = authorized.as_slice() {
            return Ok(recovery_projection_for_single_account(
                request.provider,
                account,
            ));
        }
        Ok(recovery_projection_for_unconfigured_accounts(
            request.provider,
            &authorized,
        ))
    }

    async fn select_configured_account(
        &self,
        request: CredentialAccountChoiceRequest,
    ) -> Result<CredentialAccountProjection, AuthProductError> {
        let account = self
            .read_account(&request.scope, request.account_id)
            .await?
            .map(|(a, _)| a)
            .ok_or(AuthProductError::CredentialMissing)?;
        if !brassclaw_auth::scope_matches(&request.scope, &account.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if account.provider != request.provider {
            return Err(AuthProductError::CredentialMissing);
        }
        if account.status != CredentialAccountStatus::Configured {
            return Err(AuthProductError::CredentialMissing);
        }
        if !account_is_authorized_for_requester(&account, request.requester_extension.as_ref()) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        Ok(account.projection())
    }

    async fn refresh_account(
        &self,
        _request: CredentialRefreshRequest,
    ) -> Result<CredentialRefreshReport, AuthProductError> {
        // Refresh requires a provider client; PgAuthProductServices does not
        // hold one. The composition root wires refresh through a
        // ProviderBackedCredentialAccountService decorating this service.
        Err(AuthProductError::BackendUnavailable)
    }
}

// ---------------------------------------------------------------------------
// CredentialAccountRecordSource
// ---------------------------------------------------------------------------

#[async_trait]
impl CredentialAccountRecordSource for PgAuthProductServices {
    async fn accounts_for_owner(
        &self,
        scope: &AuthProductScope,
    ) -> Result<Vec<CredentialAccount>, AuthProductError> {
        let owner = CredentialAccountOwnerScope::from_scope(scope);
        self.accounts_for_owner(&owner).await
    }
}

// ---------------------------------------------------------------------------
// CredentialSetupService
// ---------------------------------------------------------------------------

#[async_trait]
impl CredentialSetupService for PgAuthProductServices {
    async fn create_or_update_account(
        &self,
        request: CredentialAccountMutation,
    ) -> Result<CredentialAccount, AuthProductError> {
        match request {
            CredentialAccountMutation::Create(new_account) => {
                self.create_account(new_account).await
            }
            CredentialAccountMutation::Update(update) => {
                let lock = self.lock_for(format!("account:{}", update.account_id));
                let _guard = lock.lock().await;
                let scope = &update.account.scope;
                let (mut account, rev) = self
                    .read_account(scope, update.account_id)
                    .await?
                    .ok_or(AuthProductError::CredentialMissing)?;
                validate_account_update_target(&account, &update.account)?;
                let now = Utc::now();
                update_account_from_request(&mut account, update.account, now)?;
                self.write_account(&account, Some(rev)).await?;
                Ok(account)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// SecretCleanupService
// ---------------------------------------------------------------------------

#[async_trait]
impl SecretCleanupService for PgAuthProductServices {
    async fn cleanup_for_lifecycle(
        &self,
        request: SecretCleanupRequest,
    ) -> Result<SecretCleanupReport, AuthProductError> {
        let mut report = SecretCleanupReport::default();
        for account in self.accounts_for_scope(&request.scope).await? {
            let owns_extension_account = account.owner_extension.as_ref()
                == Some(&request.extension_id)
                && account.ownership == CredentialOwnership::ExtensionOwned;
            let had_grant = account
                .granted_extensions
                .iter()
                .any(|ext| ext == &request.extension_id);
            if !(owns_extension_account || had_grant) {
                continue;
            }
            let lock = self.lock_for(format!("account:{}", account.id));
            let _guard = lock.lock().await;
            let (mut current, rev) = self
                .read_account(&request.scope, account.id)
                .await?
                .ok_or(AuthProductError::CredentialMissing)?;
            current
                .granted_extensions
                .retain(|ext| ext != &request.extension_id);
            if had_grant {
                report.removed_grants.push(current.id);
            }
            let (purge_access, purge_refresh) = if owns_extension_account {
                match request.action {
                    SecretCleanupAction::Deactivate => {
                        current.status = CredentialAccountStatus::Inactive;
                        report.retained_accounts.push(current.id);
                        (None, None)
                    }
                    SecretCleanupAction::Uninstall => {
                        let access = current.access_secret.take();
                        let refresh = current.refresh_secret.take();
                        if current.status != CredentialAccountStatus::Revoked {
                            current.status = CredentialAccountStatus::Revoked;
                            report.revoked_accounts.push(current.id);
                        }
                        (access, refresh)
                    }
                }
            } else {
                if had_grant {
                    report.retained_accounts.push(current.id);
                }
                (None, None)
            };
            current.updated_at = Utc::now();
            self.write_account(&current, Some(rev)).await?;
            if let Some(h) = &purge_access
                && let Err(e) = self.secret_store.delete(&request.scope.resource, h).await
            {
                tracing::debug!(error = %e, "pg_auth_product_services: best-effort access-secret purge failed");
            }
            if let Some(h) = &purge_refresh
                && let Err(e) = self.secret_store.delete(&request.scope.resource, h).await
            {
                tracing::debug!(error = %e, "pg_auth_product_services: best-effort refresh-secret purge failed");
            }
        }
        Ok(report)
    }
}

// ---------------------------------------------------------------------------
// RebornManualTokenFlowService (inline — private helpers are not pub)
// ---------------------------------------------------------------------------

#[async_trait]
impl RebornManualTokenFlowService for PgAuthProductServices {
    async fn request_manual_token_flow(
        &self,
        request: ManualTokenSetupRequest,
    ) -> Result<AuthChallenge, AuthProductError> {
        let flow_scope = request.scope.clone();
        let flow_provider = request.provider.clone();
        let flow_continuation = request.continuation.clone();
        let flow_update_binding = request.update_binding.clone();
        let flow_expires_at = request.expires_at;
        let challenge = self.request_secret_input(request).await?;
        let brassclaw_auth::AuthChallenge::ManualTokenRequired {
            interaction_id,
            provider,
            label,
            expires_at,
        } = &challenge
        else {
            return Err(AuthProductError::InvalidRequest {
                reason: "manual token setup returned an unexpected challenge".to_string(),
            });
        };
        if let Err(error) = self
            .create_flow(NewAuthFlow {
                id: None,
                scope: flow_scope.clone(),
                kind: AuthFlowKind::IntegrationCredential,
                provider: flow_provider,
                challenge: brassclaw_auth::AuthChallenge::ManualTokenRequired {
                    interaction_id: *interaction_id,
                    provider: provider.clone(),
                    label: label.clone(),
                    expires_at: *expires_at,
                },
                continuation: flow_continuation,
                update_binding: flow_update_binding,
                opaque_state_hash: None,
                pkce_verifier_hash: None,
                expires_at: flow_expires_at,
            })
            .await
        {
            let _ = self
                .abandon_manual_token(&flow_scope, *interaction_id)
                .await;
            return Err(error);
        }
        Ok(challenge)
    }

    async fn submit_manual_token_flow(
        &self,
        scope: &AuthProductScope,
        request: SecretSubmitRequest,
    ) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError> {
        let interaction_id = request.interaction_id;
        let result = self.submit_manual_token(scope, request).await?;
        let completed = match self
            .complete_manual_token(
                scope,
                brassclaw_auth::ManualTokenCompletionInput {
                    interaction_id,
                    credential_account_id: result.account_id,
                },
            )
            .await
        {
            Ok(completed) => completed,
            Err(error) => {
                if let Err(e) = self.cancel_manual_token(scope, interaction_id).await {
                    tracing::debug!(error = %e, "pg_auth_product_services: best-effort cancel_manual_token failed on error path");
                }
                if let Err(e) = self
                    .update_status(scope, result.account_id, CredentialAccountStatus::Revoked)
                    .await
                {
                    tracing::debug!(error = %e, "pg_auth_product_services: best-effort revoke-status update failed on error path");
                }
                return Err(error);
            }
        };
        Ok((result, completed))
    }

    async fn abandon_manual_token_flow(
        &self,
        scope: &AuthProductScope,
        interaction_id: AuthInteractionId,
    ) -> Result<bool, AuthProductError> {
        let canceled = self.cancel_manual_token(scope, interaction_id).await.ok();
        let deleted = self.abandon_manual_token(scope, interaction_id).await?;
        Ok(deleted || canceled.flatten().is_some())
    }
}

// ---------------------------------------------------------------------------
// Secret validation (mirrors brassclaw_auth SecretSubmitRequest::validate_secret
// which is pub(crate) within brassclaw_auth)
// ---------------------------------------------------------------------------

fn validate_secret(request: &SecretSubmitRequest) -> Result<(), AuthProductError> {
    let exposed = request.secret.expose_secret();
    if exposed.trim().is_empty() {
        return Err(AuthProductError::InvalidRequest {
            reason: "secret value must not be empty".to_string(),
        });
    }
    if exposed.chars().any(|c| c == '\0' || c.is_control()) {
        return Err(AuthProductError::InvalidRequest {
            reason: "secret value must not contain NUL/control characters".to_string(),
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Migration entry point (pool-only, no secret store required)
// ---------------------------------------------------------------------------

/// Run the product-auth DDL migrations against the given pool.
///
/// Called at startup from `build_postgres_production` before any
/// `PgAuthProductServices` instance is wired. Idempotent: all statements
/// use `CREATE TABLE IF NOT EXISTS`.
pub(crate) async fn run_auth_migrations(
    pool: &deadpool_postgres::Pool,
) -> Result<(), AuthProductError> {
    let client = pool.get().await.map_err(pool_err)?;
    client.batch_execute(MIGRATIONS).await.map_err(db_err)
}
