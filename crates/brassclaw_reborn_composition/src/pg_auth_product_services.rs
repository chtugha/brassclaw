/// PostgreSQL-backed product-auth durable services.
///
/// Stores auth records (flows, accounts, interactions) in the
/// `brassclaw_product_auth_*` tables using JSONB columns, mirroring the same
/// domain logic and trait surface as `FilesystemAuthProductServices`. Records
/// are serialised/deserialised as JSON; the SQL layer provides CAS via
/// `revision` columns.
///
/// Migration: the schema is created via `run_migrations()` which executes
/// `CREATE TABLE IF NOT EXISTS` DDL inline (no refinery migration number
/// assigned; these tables are composition-internal to the auth layer).

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_auth::{
    AuthChallenge, AuthFlowId, AuthFlowKind, AuthFlowManager, AuthFlowOwnerScope, AuthFlowRecord,
    AuthFlowRecordSource, AuthFlowStatus, AuthInteractionId, AuthProductError, AuthProductScope,
    AuthSessionId, AuthSurface, CredentialAccount, CredentialAccountChoiceRequest,
    CredentialAccountId, CredentialAccountListPage, CredentialAccountListRequest,
    CredentialAccountLookupRequest, CredentialAccountMutation, CredentialAccountOwnerScope,
    CredentialAccountProjection, CredentialAccountRecordSource, CredentialAccountSelectionRequest,
    CredentialAccountService, CredentialAccountStatus, CredentialRecoveryProjection,
    CredentialRecoveryReason, CredentialRecoveryRequest, CredentialRefreshReport,
    CredentialRefreshRequest, CredentialSetupService, NewAuthFlow, NewCredentialAccount,
    SecretCleanupService, SecretCleanupTarget,
};
use brassclaw_auth::{AuthInteractionService, SecretSubmitRequest, SecretSubmitResult};
use brassclaw_host_api::ResourceScope;
use brassclaw_secrets::SecretStore;
use chrono::Utc;
use serde::{Serialize, de::DeserializeOwned};
use tokio_postgres::types::ToSql;

use crate::manual_token_flow::{
    ManualTokenSetupRequest, RebornManualTokenFlowService, abandon_manual_token_flow_with,
    request_manual_token_flow_with, submit_manual_token_flow_with,
};
use crate::product_auth_durable::domain::{
    account_is_authorized_for_requester, prepare_callback_flow,
    recovery_projection_for_single_account, recovery_projection_for_unconfigured_accounts,
    update_account_from_exchange, update_account_from_request,
    validate_credential_status_transition, validate_new_credential_account,
    validate_account_update_target, validate_bound_update_authority, validate_callback_claim,
    validate_flow_update_binding, validate_manual_token_flow, validate_manual_token_update_binding,
    validate_selection_flow, PreparedCallbackFlow,
};

type Pool = deadpool_postgres::Pool;

// ---------------------------------------------------------------------------
// DDL
// ---------------------------------------------------------------------------

const MIGRATIONS: &str = r#"
CREATE TABLE IF NOT EXISTS brassclaw_product_auth_accounts (
    id          TEXT NOT NULL,
    tenant_id   TEXT NOT NULL,
    revision    BIGINT NOT NULL DEFAULT 0,
    data        JSONB NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, id)
);

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

CREATE TABLE IF NOT EXISTS brassclaw_product_auth_sessions (
    id          TEXT NOT NULL,
    tenant_id   TEXT NOT NULL,
    user_id     TEXT NOT NULL,
    surface     TEXT NOT NULL,
    revision    BIGINT NOT NULL DEFAULT 0,
    data        JSONB NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, id)
);
"#;

// ---------------------------------------------------------------------------
// Struct
// ---------------------------------------------------------------------------

/// PostgreSQL-backed implementation of the product-auth durable ports.
pub(crate) struct PgAuthProductServices {
    pool: Arc<Pool>,
    secret_store: Arc<dyn SecretStore>,
}

impl PgAuthProductServices {
    pub(crate) fn new(pool: Arc<Pool>, secret_store: Arc<dyn SecretStore>) -> Self {
        Self { pool, secret_store }
    }

    pub(crate) async fn run_migrations(&self) -> Result<(), AuthProductError> {
        let client = self.pool.get().await.map_err(pool_err)?;
        client.batch_execute(MIGRATIONS).await.map_err(db_err)
    }

    // ---- generic helpers ----

    async fn get_record<T: DeserializeOwned>(
        &self,
        table: &str,
        tenant_id: &str,
        id: &str,
    ) -> Result<Option<(T, i64)>, AuthProductError> {
        let client = self.pool.get().await.map_err(pool_err)?;
        let query = format!(
            "SELECT data, revision FROM {table} WHERE tenant_id = $1 AND id = $2"
        );
        match client.query_opt(&query, &[&tenant_id, &id]).await.map_err(db_err)? {
            None => Ok(None),
            Some(row) => {
                let json: serde_json::Value = row.get(0);
                let revision: i64 = row.get(1);
                let record: T =
                    serde_json::from_value(json).map_err(|e| AuthProductError::Internal {
                        reason: format!("deserialize error: {e}"),
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
        let json = serde_json::to_value(record).map_err(|e| AuthProductError::Internal {
            reason: format!("serialize error: {e}"),
        })?;
        let client = self.pool.get().await.map_err(pool_err)?;

        let extra_names: Vec<&str> = extra_cols.iter().map(|(n, _)| *n).collect();
        let extra_vals: Vec<&(dyn ToSql + Sync)> = extra_cols.iter().map(|(_, v)| *v).collect();

        match expected_revision {
            None => {
                // Insert-or-update on absent expected revision
                let col_list: String = extra_names
                    .iter()
                    .enumerate()
                    .map(|(i, name)| format!(", {name} = ${}", i + 4))
                    .collect();
                let mut params: Vec<&(dyn ToSql + Sync)> =
                    vec![&tenant_id, &id, &json];
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
                // CAS update
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
                    return Err(AuthProductError::Conflict);
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
        let client = self.pool.get().await.map_err(pool_err)?;
        let query = format!("DELETE FROM {table} WHERE tenant_id = $1 AND id = $2");
        let n = client.execute(&query, &[&tenant_id, &id]).await.map_err(db_err)?;
        Ok(n > 0)
    }

    async fn list_records<T: DeserializeOwned>(
        &self,
        table: &str,
        tenant_id: &str,
        filter_col: &str,
        filter_val: &str,
    ) -> Result<Vec<T>, AuthProductError> {
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
                serde_json::from_value(json).map_err(|e| AuthProductError::Internal {
                    reason: format!("deserialize error: {e}"),
                })
            })
            .collect()
    }

    // ---- account helpers ----

    async fn read_account(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        account_id: CredentialAccountId,
    ) -> Result<Option<CredentialAccount>, AuthProductError> {
        let tenant_id = scope.resource.tenant_id().as_str().to_string();
        let id = account_id.to_string();
        Ok(self
            .get_record::<CredentialAccount>("brassclaw_product_auth_accounts", &tenant_id, &id)
            .await?
            .map(|(rec, _)| rec))
    }

    async fn write_account(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        account: &CredentialAccount,
        expected_revision: Option<i64>,
    ) -> Result<(), AuthProductError> {
        let tenant_id = scope.resource.tenant_id().as_str().to_string();
        let user_id = scope.resource.user_id().as_str().to_string();
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
        scope: &brassclaw_auth::AuthProductScope,
    ) -> Result<Vec<CredentialAccount>, AuthProductError> {
        let tenant_id = scope.resource.tenant_id().as_str().to_string();
        let user_id = scope.resource.user_id().as_str().to_string();
        self.list_records(
            "brassclaw_product_auth_accounts",
            &tenant_id,
            "user_id",
            &user_id,
        )
        .await
    }

    async fn create_account_with_id(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        id: CredentialAccountId,
        request: NewCredentialAccount,
    ) -> Result<CredentialAccount, AuthProductError> {
        validate_new_credential_account(&request)?;
        let account = CredentialAccount {
            id,
            scope: request.scope,
            provider: request.provider,
            label: request.label,
            owner_extension: request.owner_extension,
            grants: request.grants,
            status: CredentialAccountStatus::Active,
            access_secret: None,
            refresh_secret: None,
            provider_scopes: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        self.write_account(scope, &account, None).await?;
        Ok(account)
    }

    // ---- flow helpers ----

    async fn read_flow(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        flow_id: AuthFlowId,
    ) -> Result<Option<(AuthFlowRecord, i64)>, AuthProductError> {
        let tenant_id = scope.resource.tenant_id().as_str().to_string();
        let id = flow_id.to_string();
        self.get_record("brassclaw_product_auth_flows", &tenant_id, &id)
            .await
    }

    async fn write_flow(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        flow: &AuthFlowRecord,
        expected_revision: Option<i64>,
    ) -> Result<(), AuthProductError> {
        let tenant_id = scope.resource.tenant_id().as_str().to_string();
        let user_id = scope.resource.user_id().as_str().to_string();
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
        scope: &brassclaw_auth::AuthProductScope,
    ) -> Result<Vec<AuthFlowRecord>, AuthProductError> {
        let tenant_id = scope.resource.tenant_id().as_str().to_string();
        let user_id = scope.resource.user_id().as_str().to_string();
        self.list_records("brassclaw_product_auth_flows", &tenant_id, "user_id", &user_id)
            .await
    }

    // ---- interaction helpers ----

    async fn read_interaction(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        interaction_id: AuthInteractionId,
    ) -> Result<Option<serde_json::Value>, AuthProductError> {
        let tenant_id = scope.resource.tenant_id().as_str().to_string();
        let id = interaction_id.to_string();
        let client = self.pool.get().await.map_err(pool_err)?;
        let row = client
            .query_opt(
                "SELECT data FROM brassclaw_product_auth_interactions \
                 WHERE tenant_id = $1 AND id = $2",
                &[&tenant_id, &id],
            )
            .await
            .map_err(db_err)?;
        Ok(row.map(|r| r.get(0)))
    }

    async fn write_interaction(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        interaction_id: AuthInteractionId,
        data: serde_json::Value,
    ) -> Result<(), AuthProductError> {
        let tenant_id = scope.resource.tenant_id().as_str().to_string();
        let user_id = scope.resource.user_id().as_str().to_string();
        let id = interaction_id.to_string();
        let user_id_ref: &str = &user_id;
        self.put_record(
            "brassclaw_product_auth_interactions",
            &tenant_id,
            &id,
            &[("user_id", &user_id_ref as &(dyn ToSql + Sync))],
            &data,
            None,
        )
        .await
    }

    async fn delete_interaction(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        interaction_id: AuthInteractionId,
    ) -> Result<bool, AuthProductError> {
        let tenant_id = scope.resource.tenant_id().as_str().to_string();
        let id = interaction_id.to_string();
        self.delete_record("brassclaw_product_auth_interactions", &tenant_id, &id)
            .await
    }
}

// ---------------------------------------------------------------------------
// Error helpers
// ---------------------------------------------------------------------------

fn pool_err(e: deadpool_postgres::PoolError) -> AuthProductError {
    AuthProductError::Internal { reason: e.to_string() }
}

fn db_err(e: tokio_postgres::Error) -> AuthProductError {
    AuthProductError::Internal { reason: e.to_string() }
}

// ---------------------------------------------------------------------------
// AuthFlowManager
// ---------------------------------------------------------------------------

#[async_trait]
impl AuthFlowManager for PgAuthProductServices {
    async fn create_flow(
        &self,
        request: NewAuthFlow,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let flow = AuthFlowRecord {
            id: request.id.unwrap_or_else(AuthFlowId::new),
            scope: request.scope.clone(),
            kind: request.kind,
            provider: request.provider,
            status: AuthFlowStatus::Pending,
            challenge: request.challenge,
            continuation: request.continuation,
            update_binding: request.update_binding,
            opaque_state_hash: request.opaque_state_hash,
            pkce_verifier_hash: request.pkce_verifier_hash,
            expires_at: request.expires_at,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        self.write_flow(&request.scope, &flow, None).await?;
        Ok(flow)
    }

    async fn get_flow(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
        Ok(self.read_flow(scope, flow_id).await?.map(|(f, _)| f))
    }

    async fn update_flow_status(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
        status: AuthFlowStatus,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let (mut flow, rev) = self
            .read_flow(scope, flow_id)
            .await?
            .ok_or(AuthProductError::NotFound)?;
        validate_flow_update_binding(&flow, scope)?;
        flow.status = status;
        flow.updated_at = Utc::now();
        self.write_flow(scope, &flow, Some(rev)).await?;
        Ok(flow)
    }

    async fn bind_flow(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
        account_id: CredentialAccountId,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let (mut flow, rev) = self
            .read_flow(scope, flow_id)
            .await?
            .ok_or(AuthProductError::NotFound)?;
        validate_flow_update_binding(&flow, scope)?;
        flow.update_binding = Some(brassclaw_auth::CredentialAccountUpdateBinding {
            account_id,
            mutation: CredentialAccountMutation::Replace,
        });
        flow.updated_at = Utc::now();
        self.write_flow(scope, &flow, Some(rev)).await?;
        Ok(flow)
    }

    async fn delete_flow(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
    ) -> Result<bool, AuthProductError> {
        let tenant_id = scope.resource.tenant_id().as_str().to_string();
        let id = flow_id.to_string();
        self.delete_record("brassclaw_product_auth_flows", &tenant_id, &id).await
    }

    async fn list_flows_for_scope(
        &self,
        owner_scope: &AuthFlowOwnerScope,
    ) -> Result<Vec<AuthFlowRecord>, AuthProductError> {
        let scope = brassclaw_auth::AuthProductScope {
            resource: owner_scope.resource.clone(),
            owner_extension: None,
        };
        self.flows_for_scope(&scope).await
    }
}

// ---------------------------------------------------------------------------
// AuthFlowRecordSource
// ---------------------------------------------------------------------------

#[async_trait]
impl AuthFlowRecordSource for PgAuthProductServices {
    async fn get_flow_record(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
        self.get_flow(scope, flow_id).await
    }
}

// ---------------------------------------------------------------------------
// AuthInteractionService
// ---------------------------------------------------------------------------

#[async_trait]
impl AuthInteractionService for PgAuthProductServices {
    async fn request_secret_input(
        &self,
        request: brassclaw_auth::SecretInputRequest,
    ) -> Result<AuthChallenge, AuthProductError> {
        use brassclaw_auth::SecretInputRequest;
        let interaction_id = AuthInteractionId::new();
        let challenge = match request {
            SecretInputRequest::ManualToken { scope: _, provider, label, expires_at } => {
                AuthChallenge::ManualTokenRequired {
                    interaction_id,
                    provider,
                    label,
                    expires_at,
                }
            }
        };
        // Store a minimal interaction record (challenge JSON).
        let scope_placeholder = brassclaw_auth::AuthProductScope {
            resource: ResourceScope::from_parts(
                brassclaw_host_api::TenantId::new("_").expect("placeholder"),
                brassclaw_host_api::UserId::new("_").expect("placeholder"),
                None,
                None,
            ),
            owner_extension: None,
        };
        let data = serde_json::to_value(&challenge).map_err(|e| AuthProductError::Internal {
            reason: format!("serialize challenge: {e}"),
        })?;
        // Use the challenge's interaction_id to store it keyed by that id.
        self.write_interaction(&scope_placeholder, interaction_id, data).await?;
        Ok(challenge)
    }

    async fn resolve_secret_interaction(
        &self,
        scope: &AuthProductScope,
        interaction_id: AuthInteractionId,
    ) -> Result<Option<AuthChallenge>, AuthProductError> {
        let data = self.read_interaction(scope, interaction_id).await?;
        match data {
            None => Ok(None),
            Some(v) => {
                let challenge: AuthChallenge = serde_json::from_value(v)
                    .map_err(|e| AuthProductError::Internal { reason: e.to_string() })?;
                Ok(Some(challenge))
            }
        }
    }

    async fn submit_secret(
        &self,
        scope: &AuthProductScope,
        interaction_id: AuthInteractionId,
        secret: secrecy::SecretString,
    ) -> Result<brassclaw_secrets::SecretHandle, AuthProductError> {
        // Verify the interaction exists.
        let _ = self
            .read_interaction(scope, interaction_id)
            .await?
            .ok_or(AuthProductError::NotFound)?;
        // Store the secret and return a handle.
        let handle = brassclaw_secrets::SecretHandle::new();
        self.secret_store
            .put(&scope.resource, handle, secret)
            .await
            .map_err(|e| AuthProductError::Internal { reason: e.to_string() })?;
        // Clean up the interaction after submission.
        let _ = self.delete_interaction(scope, interaction_id).await;
        Ok(handle)
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
        let scope = brassclaw_auth::AuthProductScope {
            resource: request.scope.resource.clone(),
            owner_extension: request.owner_extension.clone(),
        };
        self.create_account_with_id(&scope, CredentialAccountId::new(), request).await
    }

    async fn get_account(
        &self,
        request: CredentialAccountLookupRequest,
    ) -> Result<Option<CredentialAccount>, AuthProductError> {
        let account = self.read_account(&request.scope, request.account_id).await?;
        let Some(account) = account else {
            return Ok(None);
        };
        if account.scope.resource.tenant_id() != request.scope.resource.tenant_id() {
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
            .filter(|a| {
                a.provider == request.provider
                    && request.cursor.is_none_or(|c| a.id > c)
                    && account_is_authorized_for_requester(a, request.requester_extension.as_ref())
            })
            .map(|a| a.projection())
            .collect::<Vec<_>>();
        accounts.sort_by_key(|a| a.id);
        let next_cursor = if accounts.len() > request.limit {
            accounts.truncate(request.limit);
            accounts.last().map(|a| a.id)
        } else {
            None
        };
        Ok(CredentialAccountListPage { accounts, next_cursor })
    }

    async fn update_account(
        &self,
        request: brassclaw_auth::CredentialAccountUpdateRequest,
    ) -> Result<CredentialAccount, AuthProductError> {
        validate_account_update_target(&request)?;
        let (mut account, rev) = self
            .read_account(&request.scope, request.account_id)
            .await?
            .map(|a| {
                let scope = brassclaw_auth::AuthProductScope {
                    resource: request.scope.resource.clone(),
                    owner_extension: request.scope.owner_extension.clone(),
                };
                (a, 0i64) // revision not needed here since we re-read
            })
            .ok_or(AuthProductError::NotFound)?;
        let _ = rev;
        let (mut account2, rev2) = self
            .get_record::<CredentialAccount>(
                "brassclaw_product_auth_accounts",
                account.scope.resource.tenant_id().as_str(),
                &account.id.to_string(),
            )
            .await?
            .ok_or(AuthProductError::NotFound)?;
        validate_bound_update_authority(&account2, &request)?;
        update_account_from_request(&mut account2, request)?;
        account2.updated_at = Utc::now();
        self.write_account(&brassclaw_auth::AuthProductScope {
            resource: account2.scope.resource.clone(),
            owner_extension: None,
        }, &account2, Some(rev2)).await?;
        Ok(account2)
    }

    async fn select_account(
        &self,
        request: CredentialAccountSelectionRequest,
    ) -> Result<Option<CredentialAccount>, AuthProductError> {
        validate_selection_flow(&request.flow, &request.scope)?;
        let accounts = self.accounts_for_scope(&request.scope).await?;
        let matching = accounts
            .into_iter()
            .find(|a| a.provider == request.provider && a.status == CredentialAccountStatus::Active);
        Ok(matching)
    }

    async fn choose_account(
        &self,
        request: CredentialAccountChoiceRequest,
    ) -> Result<CredentialAccount, AuthProductError> {
        let (account, _) = self
            .read_account(&request.scope, request.account_id)
            .await?
            .ok_or(AuthProductError::NotFound)?;
        // Re-read with revision for CAS
        let (mut account2, rev) = self
            .get_record::<CredentialAccount>(
                "brassclaw_product_auth_accounts",
                account.scope.resource.tenant_id().as_str(),
                &account.id.to_string(),
            )
            .await?
            .ok_or(AuthProductError::NotFound)?;
        validate_credential_status_transition(account2.status, CredentialAccountStatus::Active)?;
        account2.status = CredentialAccountStatus::Active;
        account2.updated_at = Utc::now();
        self.write_account(
            &brassclaw_auth::AuthProductScope {
                resource: account2.scope.resource.clone(),
                owner_extension: None,
            },
            &account2,
            Some(rev),
        ).await?;
        Ok(account2)
    }

    async fn refresh_account(
        &self,
        request: CredentialRefreshRequest,
    ) -> Result<CredentialRefreshReport, AuthProductError> {
        let (mut account, rev) = self
            .get_record::<CredentialAccount>(
                "brassclaw_product_auth_accounts",
                request.scope.resource.tenant_id().as_str(),
                &request.account_id.to_string(),
            )
            .await?
            .ok_or(AuthProductError::NotFound)?;
        account.updated_at = Utc::now();
        self.write_account(&request.scope, &account, Some(rev)).await?;
        Ok(CredentialRefreshReport { account_id: account.id, refreshed: true })
    }

    async fn delete_account(
        &self,
        scope: &AuthProductScope,
        account_id: CredentialAccountId,
    ) -> Result<bool, AuthProductError> {
        let tenant_id = scope.resource.tenant_id().as_str().to_string();
        let id = account_id.to_string();
        self.delete_record("brassclaw_product_auth_accounts", &tenant_id, &id).await
    }

    async fn recovery_projection(
        &self,
        request: CredentialRecoveryRequest,
    ) -> Result<CredentialRecoveryProjection, AuthProductError> {
        let accounts = self.accounts_for_scope(&request.scope).await?;
        let scope = &request.scope;
        Ok(match &request.reason {
            CredentialRecoveryReason::AccountNotFound { provider } => {
                recovery_projection_for_unconfigured_accounts(scope, provider, &accounts)
            }
            CredentialRecoveryReason::AccountFound { account_id } => {
                let account = accounts.into_iter().find(|a| a.id == *account_id);
                recovery_projection_for_single_account(scope, account.as_ref())
            }
        })
    }
}

// ---------------------------------------------------------------------------
// CredentialAccountRecordSource
// ---------------------------------------------------------------------------

#[async_trait]
impl CredentialAccountRecordSource for PgAuthProductServices {
    async fn get_account_record(
        &self,
        scope: &CredentialAccountOwnerScope,
        account_id: CredentialAccountId,
    ) -> Result<Option<CredentialAccountProjection>, AuthProductError> {
        let auth_scope = brassclaw_auth::AuthProductScope {
            resource: scope.resource.clone(),
            owner_extension: scope.owner_extension.clone(),
        };
        let account = self.read_account(&auth_scope, account_id).await?;
        Ok(account.map(|a| a.projection()))
    }
}

// ---------------------------------------------------------------------------
// CredentialSetupService
// ---------------------------------------------------------------------------

#[async_trait]
impl CredentialSetupService for PgAuthProductServices {
    async fn initiate_oauth_setup(
        &self,
        request: brassclaw_auth::OAuthSetupRequest,
    ) -> Result<brassclaw_auth::OAuthSetupChallenge, AuthProductError> {
        use brassclaw_auth::OAuthSetupChallenge;
        // Create an auth flow to track the OAuth setup.
        let flow = self
            .create_flow(NewAuthFlow {
                id: None,
                scope: request.scope.clone(),
                kind: AuthFlowKind::IntegrationCredential,
                provider: request.provider.clone(),
                challenge: AuthChallenge::OAuthRedirectRequired {
                    redirect_url: request.redirect_url.clone(),
                },
                continuation: request.continuation,
                update_binding: request.update_binding,
                opaque_state_hash: request.state_hash,
                pkce_verifier_hash: request.pkce_verifier_hash,
                expires_at: request.expires_at,
            })
            .await?;
        Ok(OAuthSetupChallenge {
            flow_id: flow.id,
            redirect_url: request.redirect_url,
        })
    }

    async fn complete_oauth_setup(
        &self,
        request: brassclaw_auth::OAuthCallbackRequest,
    ) -> Result<CredentialAccount, AuthProductError> {
        let (flow, rev) = self
            .read_flow(&request.scope, request.flow_id)
            .await?
            .ok_or(AuthProductError::NotFound)?;
        validate_callback_claim(&flow, &request.scope)?;
        let prepared = prepare_callback_flow(&flow, &request)?;
        let account = self.apply_callback_prepared(prepared, &request.scope).await?;
        // Mark flow as completed.
        let mut completed_flow = flow;
        completed_flow.status = AuthFlowStatus::Completed;
        completed_flow.updated_at = Utc::now();
        self.write_flow(&request.scope, &completed_flow, Some(rev)).await?;
        Ok(account)
    }
}

impl PgAuthProductServices {
    async fn apply_callback_prepared(
        &self,
        prepared: PreparedCallbackFlow,
        scope: &AuthProductScope,
    ) -> Result<CredentialAccount, AuthProductError> {
        let account = match &prepared.update_binding {
            Some(binding) => {
                // Update existing account.
                let (mut existing, rev) = self
                    .get_record::<CredentialAccount>(
                        "brassclaw_product_auth_accounts",
                        scope.resource.tenant_id().as_str(),
                        &binding.account_id.to_string(),
                    )
                    .await?
                    .ok_or(AuthProductError::NotFound)?;
                update_account_from_exchange(&mut existing, &prepared)?;
                existing.updated_at = Utc::now();
                self.write_account(scope, &existing, Some(rev)).await?;
                existing
            }
            None => {
                // Create new account.
                let mut account = CredentialAccount {
                    id: CredentialAccountId::new(),
                    scope: scope.clone().into(),
                    provider: prepared.provider,
                    label: prepared.label.unwrap_or_default(),
                    owner_extension: scope.owner_extension.clone(),
                    grants: Vec::new(),
                    status: CredentialAccountStatus::Active,
                    access_secret: None,
                    refresh_secret: None,
                    provider_scopes: Vec::new(),
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                update_account_from_exchange(&mut account, &prepared)?;
                self.write_account(scope, &account, None).await?;
                account
            }
        };
        Ok(account)
    }
}

// ---------------------------------------------------------------------------
// SecretCleanupService
// ---------------------------------------------------------------------------

#[async_trait]
impl SecretCleanupService for PgAuthProductServices {
    async fn cleanup_secrets(
        &self,
        request: SecretCleanupTarget,
    ) -> Result<(), AuthProductError> {
        // Best-effort: clean up all secret handles associated with accounts.
        let scope = brassclaw_auth::AuthProductScope {
            resource: request.resource.clone(),
            owner_extension: None,
        };
        let accounts = self.accounts_for_scope(&scope).await?;
        for account in accounts {
            if let Some(h) = account.access_secret {
                let _ = self.secret_store.delete(&request.resource, h).await;
            }
            if let Some(h) = account.refresh_secret {
                let _ = self.secret_store.delete(&request.resource, h).await;
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// RebornManualTokenFlowService (delegates to the shared helpers)
// ---------------------------------------------------------------------------

#[async_trait]
impl RebornManualTokenFlowService for PgAuthProductServices {
    async fn request_manual_token_flow(
        &self,
        request: ManualTokenSetupRequest,
    ) -> Result<AuthChallenge, AuthProductError> {
        request_manual_token_flow_with(self, self, request).await
    }

    async fn submit_manual_token_flow(
        &self,
        scope: &AuthProductScope,
        request: SecretSubmitRequest,
    ) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError> {
        submit_manual_token_flow_with(self, self, self, scope, request).await
    }

    async fn abandon_manual_token_flow(
        &self,
        scope: &AuthProductScope,
        interaction_id: AuthInteractionId,
    ) -> Result<bool, AuthProductError> {
        abandon_manual_token_flow_with(self, self, scope, interaction_id).await
    }
}
