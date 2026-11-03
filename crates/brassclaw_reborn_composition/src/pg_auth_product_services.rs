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
    account_is_authorized_for_requester, prepare_callback_flow,
    recovery_projection_for_single_account, recovery_projection_for_unconfigured_accounts,
    update_account_from_request, validate_account_update_target,
    validate_credential_status_transition, validate_flow_update_binding,
    validate_manual_token_flow, validate_manual_token_update_binding,
    validate_new_credential_account, validate_selection_flow,
};
use brassclaw_auth::{
    AuthChallenge, AuthFlowId, AuthFlowManager, AuthFlowOwnerScope, AuthFlowRecord,
    AuthFlowRecordSource, AuthFlowStatus, AuthInteractionId, AuthInteractionService,
    AuthProductError, AuthProductScope, AuthProviderId, CredentialAccount,
    CredentialAccountChoiceRequest, CredentialAccountId, CredentialAccountLabel,
    CredentialAccountListPage, CredentialAccountListRequest, CredentialAccountLookupRequest,
    CredentialAccountMutation, CredentialAccountOwnerScope, CredentialAccountProjection,
    CredentialAccountRecordSource, CredentialAccountSelectionRequest, CredentialAccountService,
    CredentialAccountStatus, CredentialAccountUpdateBinding, CredentialOwnership,
    CredentialRecoveryProjection, CredentialRecoveryReason, CredentialRecoveryRequest,
    CredentialRefreshReport, CredentialRefreshRequest, CredentialSetupService,
    ManualTokenSetupIdentity, ManualTokenSetupRequest, NewAuthFlow, NewCredentialAccount,
    OAuthCallbackClaimRequest, OAuthCallbackFailureInput, OAuthCallbackInput,
    ProviderCallbackOutcome, SecretCleanupReport, SecretCleanupRequest, SecretCleanupService,
    SecretSubmitRequest, SecretSubmitResult, TurnGateAuthFlowQuery,
    credential_status_for_completed_flow, flow_matches_turn_gate_query, is_terminal_status,
    scope_matches,
};
use brassclaw_host_api::SecretHandle;
use brassclaw_secrets::{PgSecretStore, SecretStore};
use chrono::Utc;
use secrecy::ExposeSecret as _;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio_postgres::types::ToSql;

use crate::manual_token_flow::RebornManualTokenFlowService;

type Pool = deadpool_postgres::Pool;

#[path = "pg_auth_callback.rs"]
mod callback;
#[path = "pg_auth_cleanup.rs"]
mod cleanup;
#[cfg(all(feature = "root-llm-provider", feature = "postgres"))]
#[path = "pg_auth_codex_access.rs"]
pub(crate) mod codex_access;
#[cfg(all(feature = "root-llm-provider", feature = "postgres"))]
#[path = "pg_auth_codex_publication.rs"]
pub(crate) mod codex_publication;
#[cfg(feature = "root-llm-provider")]
#[path = "pg_auth_codex_refresh.rs"]
pub(crate) mod codex_refresh;
#[path = "pg_auth_dcr.rs"]
mod dcr;
#[cfg(feature = "root-llm-provider")]
#[path = "pg_auth_device.rs"]
mod device;
#[path = "pg_auth_filesystem_migration.rs"]
mod filesystem_migration;
#[path = "pg_auth_manual.rs"]
mod manual;
#[path = "pg_auth_manual_setup.rs"]
mod manual_setup;
#[path = "pg_auth_oauth_setup.rs"]
mod oauth_setup;
#[path = "pg_auth_refresh.rs"]
mod refresh;
#[path = "pg_auth_requirement.rs"]
mod requirement;
#[path = "pg_auth_selection.rs"]
mod selection;

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

CREATE TABLE IF NOT EXISTS brassclaw_product_auth_refreshes (
    tenant_id TEXT NOT NULL,
    scope_ref TEXT NOT NULL,
    id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    refresh_handle TEXT,
    blocks_refresh BOOLEAN NOT NULL,
    revision BIGINT NOT NULL DEFAULT 0,
    data JSONB NOT NULL,
    PRIMARY KEY (tenant_id,scope_ref,id)
);
CREATE INDEX IF NOT EXISTS brassclaw_product_auth_refreshes_unconfirmed_idx
    ON brassclaw_product_auth_refreshes (tenant_id,account_id,refresh_handle)
    WHERE blocks_refresh;

CREATE TABLE IF NOT EXISTS brassclaw_product_auth_requirement_selections (
    id TEXT NOT NULL,
    tenant_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    revision BIGINT NOT NULL DEFAULT 0,
    data JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
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

-- Composition-internal custody, owned by the same schema attachment as the
-- five Auth tables above. This records a writer fence, never credentials.
CREATE TABLE IF NOT EXISTS brassclaw_product_auth_filesystem_recoveries (
    tenant_id TEXT PRIMARY KEY
);
CREATE OR REPLACE FUNCTION brassclaw_fence_native_auth_during_filesystem_recovery()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM brassclaw_product_auth_filesystem_recoveries
        WHERE tenant_id IN (
            CASE WHEN TG_OP <> 'INSERT' THEN OLD.tenant_id END,
            CASE WHEN TG_OP <> 'DELETE' THEN NEW.tenant_id END
        )
    ) THEN
        RAISE EXCEPTION 'product auth remains under original filesystem recovery custody'
            USING ERRCODE = '55000';
    END IF;
    IF TG_OP = 'DELETE' THEN RETURN OLD; ELSE RETURN NEW; END IF;
END;
$$;
DO $$
DECLARE target TEXT;
BEGIN
    FOREACH target IN ARRAY ARRAY[
        'brassclaw_product_auth_accounts',
        'brassclaw_product_auth_flows',
        'brassclaw_product_auth_interactions',
        'brassclaw_product_auth_requirement_selections',
        'brassclaw_product_auth_refreshes'
    ] LOOP
        IF NOT EXISTS (SELECT 1 FROM pg_trigger
                       WHERE tgrelid=target::regclass
                       AND tgname='brassclaw_native_auth_recovery_custody') THEN
            EXECUTE format('CREATE TRIGGER brassclaw_native_auth_recovery_custody BEFORE INSERT OR UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION brassclaw_fence_native_auth_during_filesystem_recovery()', target);
        END IF;
    END LOOP;
END;
$$;
"#;

// ---------------------------------------------------------------------------
// Struct
// ---------------------------------------------------------------------------

/// PostgreSQL-backed implementation of the product-auth durable ports.
pub(crate) struct PgAuthProductServices {
    pool: Arc<Pool>,
    secret_store: Arc<PgSecretStore>,
    locks: Mutex<std::collections::HashMap<String, Weak<tokio::sync::Mutex<()>>>>,
}

impl PgAuthProductServices {
    // Keep the actual native owner so account/flow/secret writes can use one
    // PostgreSQL transaction. Composition supplies its same pool/store; there
    // is no downcast, separate store or active unsupported generic port.
    pub(crate) fn new(pool: Arc<Pool>, secret_store: Arc<PgSecretStore>) -> Self {
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
        "brassclaw_product_auth_requirement_selections",
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
                let mut params: Vec<&(dyn ToSql + Sync)> = vec![&tenant_id, &id, &json];
                params.extend(extra_vals.iter());
                let query = format!(
                    "INSERT INTO {table} (tenant_id, id, data{extra_insert}) \
                     VALUES ($1, $2, $3{extra_placeholders}) \
                     ON CONFLICT (tenant_id, id) DO NOTHING",
                    extra_insert = extra_names
                        .iter()
                        .map(|n| format!(", {n}"))
                        .collect::<String>(),
                    extra_placeholders = (4..=3 + extra_names.len())
                        .map(|i| format!(", ${i}"))
                        .collect::<String>(),
                );
                if client.execute(&query, &params).await.map_err(db_err)? != 1 {
                    return Err(AuthProductError::BackendConflict);
                }
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
                let data_expr = if table == "brassclaw_product_auth_accounts" {
                    "jsonb_set($3::jsonb,'{auth_lifecycle_cleanup}',COALESCE(data->'auth_lifecycle_cleanup','[]'::jsonb),true)"
                } else {
                    "$3"
                };
                let query = format!(
                    "UPDATE {table} SET data = {data_expr}, revision = revision + 1, \
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
        let record: Option<(CredentialAccount, i64)> = self
            .get_record("brassclaw_product_auth_accounts", &tenant_id, &id)
            .await?;
        if let Some((record, _)) = &record
            && !scope_matches(scope, &record.scope)
        {
            return Err(AuthProductError::CrossScopeDenied);
        }
        Ok(record)
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
        let records: Vec<CredentialAccount> = self
            .list_records(
                "brassclaw_product_auth_accounts",
                &tenant_id,
                "user_id",
                &user_id,
            )
            .await?;
        Ok(records
            .into_iter()
            .filter(|record| scope_matches(scope, &record.scope))
            .collect())
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

    async fn read_flow_document(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
    ) -> Result<Option<(crate::auth_flow_custody::StoredAuthFlow, i64)>, AuthProductError> {
        let tenant = scope.resource.tenant_id.as_str().to_owned();
        let id = flow_id.to_string();
        let record: Option<(crate::auth_flow_custody::StoredAuthFlow, i64)> = self
            .get_record("brassclaw_product_auth_flows", &tenant, &id)
            .await?;
        if let Some((document, _)) = &record
            && (!scope_matches(scope, &document.flow.scope) || document.flow.id != flow_id)
        {
            return Err(AuthProductError::CrossScopeDenied);
        }
        Ok(record)
    }
    async fn read_flow(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
    ) -> Result<Option<(AuthFlowRecord, i64)>, AuthProductError> {
        Ok(self
            .read_flow_document(scope, flow_id)
            .await?
            .map(|(doc, revision)| (doc.flow, revision)))
    }
    async fn write_flow_document(
        &self,
        document: &crate::auth_flow_custody::StoredAuthFlow,
        expected_revision: Option<i64>,
    ) -> Result<(), AuthProductError> {
        let flow = &document.flow;
        let tenant = flow.scope.resource.tenant_id.as_str().to_owned();
        let user = flow.scope.resource.user_id.as_str().to_owned();
        let id = flow.id.to_string();
        let user_ref: &str = &user;
        self.put_record(
            "brassclaw_product_auth_flows",
            &tenant,
            &id,
            &[("user_id", &user_ref as &(dyn ToSql + Sync))],
            document,
            expected_revision,
        )
        .await
    }
    async fn write_new_flow(
        &self,
        flow: &AuthFlowRecord,
        snapshot: Option<CredentialAccount>,
        setup: Option<Box<brassclaw_auth::CredentialRequirementSetup>>,
    ) -> Result<(), AuthProductError> {
        let document = crate::auth_flow_custody::StoredAuthFlow::new(flow.clone(), snapshot)?
            .with_requirement_setup(setup)?;
        self.write_flow_document(&document, None).await
    }
    async fn write_flow(
        &self,
        flow: &AuthFlowRecord,
        expected_revision: Option<i64>,
    ) -> Result<(), AuthProductError> {
        let expected = expected_revision.ok_or(AuthProductError::BackendConflict)?;
        let (mut document, revision) = self
            .read_flow_document(&flow.scope, flow.id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        if revision != expected {
            return Err(AuthProductError::BackendConflict);
        }
        document.flow = flow.clone();
        self.write_flow_document(&document, Some(revision)).await
    }

    async fn flows_for_scope(
        &self,
        scope: &AuthProductScope,
    ) -> Result<Vec<AuthFlowRecord>, AuthProductError> {
        let tenant_id = scope.resource.tenant_id.as_str().to_string();
        let user_id = scope.resource.user_id.as_str().to_string();
        let records: Vec<AuthFlowRecord> = self
            .list_records(
                "brassclaw_product_auth_flows",
                &tenant_id,
                "user_id",
                &user_id,
            )
            .await?;
        Ok(records
            .into_iter()
            .filter(|record| scope_matches(scope, &record.scope))
            .collect())
    }

    // ---- interaction helpers ----

    async fn read_interaction(
        &self,
        scope: &AuthProductScope,
        interaction_id: AuthInteractionId,
    ) -> Result<Option<(StoredManualTokenInteraction, i64)>, AuthProductError> {
        let tenant_id = scope.resource.tenant_id.as_str().to_string();
        let id = interaction_id.to_string();
        let record: Option<(StoredManualTokenInteraction, i64)> = self
            .get_record("brassclaw_product_auth_interactions", &tenant_id, &id)
            .await?;
        if let Some((record, _)) = &record
            && !scope_matches(scope, &record.scope)
        {
            return Err(AuthProductError::CrossScopeDenied);
        }
        Ok(record)
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

    async fn abandon_manual_interaction_owned(
        &self,
        scope: &AuthProductScope,
        interaction_id: AuthInteractionId,
    ) -> Result<bool, AuthProductError> {
        let Some((mut record, revision)) = self.read_interaction(scope, interaction_id).await?
        else {
            return Ok(false);
        };
        if record.consumed_at.is_some() || record.canceled_at.is_some() {
            return Ok(false);
        }
        let before =
            serde_json::to_value(&record).map_err(|_| AuthProductError::BackendUnavailable)?;
        record.canceled_at = Some(Utc::now());
        let after =
            serde_json::to_value(&record).map_err(|_| AuthProductError::BackendUnavailable)?;
        let write: Result<(), AuthProductError> = async {
            let mut client = self.pool.get().await.map_err(pool_err)?;
            let transaction = client.transaction().await.map_err(db_err)?;
            let changed = transaction.execute(
                "UPDATE brassclaw_product_auth_interactions SET data=$3,revision=revision+1,updated_at=NOW()
                 WHERE tenant_id=$1 AND id=$2 AND revision=$4 AND data=$5",
                &[&scope.resource.tenant_id.as_str(), &interaction_id.to_string(), &after, &revision, &before],
            ).await.map_err(db_err)?;
            if changed != 1 {return Err(AuthProductError::BackendConflict);}
            transaction.commit().await.map_err(db_err)
        }.await;
        let error = match write {
            Ok(()) => return Ok(true),
            Err(error) => error,
        };
        // A missing acknowledgement does not authorize deleting this original.
        // Read the actual disposition, including a concurrent completed writer.
        let Some((mut current, _)) = self.read_interaction(scope, interaction_id).await? else {
            return Err(error);
        };
        if current.consumed_at.is_some() {
            return Ok(false);
        }
        let canceled = current.canceled_at.is_some();
        current.canceled_at = record.canceled_at;
        if canceled && current == record {
            return Ok(true);
        }
        Err(error)
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct StoredManualTokenInteraction {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    requirement_setup: Option<Box<brassclaw_auth::CredentialRequirementSetup>>,
    id: AuthInteractionId,
    scope: AuthProductScope,
    provider: AuthProviderId,
    label: CredentialAccountLabel,
    continuation: brassclaw_auth::AuthContinuationRef,
    update_binding: Option<CredentialAccountUpdateBinding>,
    expires_at: brassclaw_auth::Timestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    update_account_snapshot: Option<CredentialAccount>,
    consumed_at: Option<brassclaw_auth::Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    canceled_at: Option<brassclaw_auth::Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    setup_flow_id: Option<AuthFlowId>,
}

// ---------------------------------------------------------------------------
// AuthFlowManager
// ---------------------------------------------------------------------------

#[async_trait]
impl AuthFlowManager for PgAuthProductServices {
    async fn requirement_setup(
        &self,
        scope: &AuthProductScope,
        flow: AuthFlowId,
    ) -> Result<Option<brassclaw_auth::CredentialRequirementSetup>, AuthProductError> {
        let (original, _) = self
            .read_flow_document(scope, flow)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        let setup = original.auth_custody.requirement_setup.map(|setup| *setup);
        if let Some(setup) = &setup {
            setup
                .verify_flow(&original.flow)
                .map_err(|_| AuthProductError::BackendUnavailable)?;
        }
        Ok(setup)
    }

    async fn claim_dcr_cleanup(
        &self,
        prepared: brassclaw_auth::DcrCleanupPrepared,
    ) -> Result<brassclaw_auth::DcrCleanupAdmission, AuthProductError> {
        self.claim_dcr_cleanup_owned(prepared).await
    }
    async fn record_dcr_cleanup(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        completion: brassclaw_auth::DcrCleanupCompletion,
    ) -> Result<brassclaw_auth::DcrRegistrationState, AuthProductError> {
        self.record_dcr_cleanup_owned(scope, completion).await
    }
    async fn update_dcr_retirement(
        &self,
        original: &brassclaw_auth::DcrRegistrationOriginal,
        update: brassclaw_auth::DcrRetirementUpdate,
    ) -> Result<brassclaw_auth::DcrRegistrationState, brassclaw_auth::AuthProductError> {
        self.update_dcr_retirement_owned(original, update).await
    }

    async fn retain_legacy_dcr(
        &self,
        selection: brassclaw_auth::DcrLegacySelection,
    ) -> Result<brassclaw_auth::DcrLegacyState, brassclaw_auth::AuthProductError> {
        self.retain_legacy_dcr_owned(selection).await
    }

    async fn mark_dcr_cleanup_unavailable(
        &self,
        original: &brassclaw_auth::DcrRegistrationOriginal,
        issue: brassclaw_auth::DcrCleanupUnavailable,
    ) -> Result<brassclaw_auth::DcrRegistrationState, AuthProductError> {
        self.mark_dcr_cleanup_unavailable_owned(original, issue)
            .await
    }

    async fn prepare_oauth_setup(
        &self,
        original: brassclaw_auth::OAuthSetupOriginal,
        retirement: Option<brassclaw_auth::OAuthSetupRetirement>,
    ) -> Result<brassclaw_auth::OAuthSetupState, brassclaw_auth::AuthProductError> {
        self.prepare_oauth_setup_owned(original, retirement).await
    }

    async fn read_oauth_setup(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        flow_id: brassclaw_auth::AuthFlowId,
    ) -> Result<Option<brassclaw_auth::OAuthSetupState>, brassclaw_auth::AuthProductError> {
        self.read_oauth_setup_owned(scope, flow_id).await
    }

    async fn prepare_dcr_registration(
        &self,
        original: brassclaw_auth::DcrRegistrationOriginal,
    ) -> Result<brassclaw_auth::DcrRegistrationState, AuthProductError> {
        self.prepare_dcr_original(original).await
    }

    async fn retire_dcr_preparation(
        &self,
        original: brassclaw_auth::DcrRegistrationOriginal,
        reason: brassclaw_auth::DcrCleanupReason,
    ) -> Result<brassclaw_auth::DcrRegistrationState, brassclaw_auth::AuthProductError> {
        self.retire_dcr_preparation_owned(original, reason).await
    }
    async fn read_dcr_registration(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        id: AuthFlowId,
    ) -> Result<Option<brassclaw_auth::DcrRegistrationState>, AuthProductError> {
        self.read_dcr_original(scope, id).await
    }
    async fn claim_dcr_registration(
        &self,
        original: &brassclaw_auth::DcrRegistrationOriginal,
    ) -> Result<brassclaw_auth::DcrRegistrationAdmission, AuthProductError> {
        self.claim_dcr_original(original).await
    }
    async fn record_dcr_registration(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        completion: brassclaw_auth::DcrRegistrationCompletion,
    ) -> Result<brassclaw_auth::DcrRegistrationState, AuthProductError> {
        self.record_dcr_original(scope, completion).await
    }
    async fn publish_dcr_challenge(
        &self,
        original: &brassclaw_auth::DcrRegistrationOriginal,
        challenge: brassclaw_auth::DcrRegistrationChallenge,
    ) -> Result<brassclaw_auth::DcrRegistrationState, AuthProductError> {
        self.publish_dcr_original(original, challenge).await
    }

    async fn create_flow(&self, request: NewAuthFlow) -> Result<AuthFlowRecord, AuthProductError> {
        let snapshot = if let Some(binding) = &request.update_binding {
            let scope = request.scope.clone();
            let account = self
                .read_account(&scope, binding.account_id)
                .await?
                .map(|(account, _)| account)
                .ok_or(AuthProductError::CredentialMissing)?;
            validate_flow_update_binding(&account, &request)?;
            Some(account)
        } else {
            None
        };
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
        self.write_new_flow(&record, snapshot, request.requirement_setup)
            .await?;
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
    ) -> Result<brassclaw_auth::OAuthCallbackAdmission, AuthProductError> {
        self.admit_callback(scope, request).await
    }
    async fn read_oauth_callback_original(
        &self,
        scope: &AuthProductScope,
        reference: &brassclaw_auth::OAuthCallbackReference,
    ) -> Result<Option<brassclaw_auth::OAuthCallbackState>, AuthProductError> {
        self.read_callback_original(scope, reference).await
    }
    async fn reconcile_oauth_callback(
        &self,
        scope: &AuthProductScope,
        reference: &brassclaw_auth::OAuthCallbackReference,
    ) -> Result<brassclaw_auth::OAuthCallbackState, AuthProductError> {
        self.reconcile_callback_original(scope, reference).await
    }
    async fn complete_oauth_callback(
        &self,
        scope: &AuthProductScope,
        input: OAuthCallbackInput,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        self.complete_callback_owned(scope, input).await
    }

    async fn complete_credential_selection(
        &self,
        scope: &AuthProductScope,
        input: brassclaw_auth::CredentialSelectionInput,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        self.complete_selection_owned(scope, input).await
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
            || account.effective_status() != CredentialAccountStatus::Configured
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
        let Some(flow_id) = self.flows_for_scope(scope).await?.into_iter().find_map(|flow| {
            matches!(&flow.challenge, Some(AuthChallenge::ManualTokenRequired {interaction_id:id,..}) if id == &interaction_id).then_some(flow.id)
        }) else { return Ok(None); };
        let lock = self.lock_for(format!("flow:{flow_id}"));
        let _guard = lock.lock().await;
        let (mut flow, revision) = self
            .read_flow(scope, flow_id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        if !matches!(&flow.challenge, Some(AuthChallenge::ManualTokenRequired {interaction_id:id,..}) if id == &interaction_id)
        {
            return Err(AuthProductError::BackendConflict);
        }
        if !is_terminal_status(flow.status) {
            flow.status = AuthFlowStatus::Canceled;
            flow.error = Some(brassclaw_auth::AuthErrorCode::Canceled);
            flow.updated_at = Utc::now();
            self.write_flow(&flow, Some(revision)).await?;
        }
        Ok(Some(flow))
    }

    async fn fail_oauth_callback(
        &self,
        scope: &AuthProductScope,
        input: OAuthCallbackFailureInput,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        self.fail_callback_owned(scope, input).await
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
        if flow.status != AuthFlowStatus::Completed {
            return Err(AuthProductError::FlowAlreadyTerminal);
        }
        if flow.continuation_emitted_at.is_some() {
            return Ok(flow);
        }
        flow.continuation_emitted_at = Some(emitted_at);
        flow.updated_at = emitted_at;
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
        let (mut document, revision) = self
            .read_flow_document(scope, flow_id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        if is_terminal_status(document.flow.status) {
            return Err(if document.flow.status == AuthFlowStatus::Canceled {
                AuthProductError::Canceled
            } else {
                AuthProductError::FlowAlreadyTerminal
            });
        }
        let now = Utc::now();
        document.flow.status = AuthFlowStatus::Canceled;
        document.flow.error = Some(brassclaw_auth::AuthErrorCode::Canceled);
        document.flow.updated_at = now;
        document
            .auth_custody
            .require_dcr_cleanup(&mut document.flow, now)?;
        // Cancellation and cleanup responsibility are the same real CAS.
        self.write_flow_document(&document, Some(revision)).await?;
        Ok(document.flow)
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
        Ok(self
            .flows_for_owner(query.owner.clone())
            .await?
            .into_iter()
            .find(|flow| flow_matches_turn_gate_query(flow, &query)))
    }

    async fn flows_for_owner(
        &self,
        owner: AuthFlowOwnerScope,
    ) -> Result<Vec<AuthFlowRecord>, AuthProductError> {
        let mut results: Vec<AuthFlowRecord> = self
            .list_records(
                "brassclaw_product_auth_flows",
                owner.tenant_id.as_str(),
                "user_id",
                owner.user_id.as_str(),
            )
            .await?;
        results.retain(|flow| owner.matches(flow));
        results.sort_by_key(|flow| flow.id);
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
        if let Some(setup) = &request.requirement_setup {
            setup.verify_manual(&request.scope, &request.provider)?;
        }
        let update_account_snapshot = if let Some(binding) = &request.update_binding {
            let scope = request.scope.clone();
            let account = self
                .read_account(&scope, binding.account_id)
                .await?
                .map(|(account, _)| account)
                .ok_or(AuthProductError::CredentialMissing)?;
            validate_manual_token_update_binding(&account, &request, binding)?;
            Some(account)
        } else {
            None
        };
        let interaction = StoredManualTokenInteraction {
            requirement_setup: request.requirement_setup,
            id: AuthInteractionId::new(),
            scope: request.scope.clone(),
            provider: request.provider.clone(),
            label: request.label.clone(),
            continuation: request.continuation,
            update_binding: request.update_binding,
            expires_at: request.expires_at,
            update_account_snapshot,
            consumed_at: None,
            canceled_at: None,
            setup_flow_id: None,
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
        self.submit_manual_native(scope, request, false, None)
            .await
            .map(|(result, _)| result)
    }

    async fn abandon_manual_token(
        &self,
        scope: &AuthProductScope,
        interaction_id: AuthInteractionId,
    ) -> Result<bool, AuthProductError> {
        self.abandon_manual_interaction_owned(scope, interaction_id)
            .await
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
            access_expires_at: request.access_expires_at,
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
                    && account.effective_status() == CredentialAccountStatus::Configured
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
            .filter(|account| account.effective_status() == CredentialAccountStatus::Configured)
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
        if account.effective_status() != CredentialAccountStatus::Configured {
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
    async fn account_record(
        &self,
        scope: &AuthProductScope,
        account: CredentialAccountId,
    ) -> Result<Option<CredentialAccount>, AuthProductError> {
        self.read_account(scope, account)
            .await
            .map(|record| record.map(|(account, _)| account))
    }

    async fn requirement_selection(
        &self,
        scope: &AuthProductScope,
        requirement: &brassclaw_host_api::RuntimeCredentialAccountRequirement,
    ) -> Result<Option<brassclaw_auth::CredentialRequirementSelection>, AuthProductError> {
        let key = brassclaw_auth::CredentialRequirementOwner::from_scope(&scope.resource)
            .key(requirement)?;
        let selected: Option<(brassclaw_auth::CredentialRequirementSelection, i64)> = self
            .get_record(
                "brassclaw_product_auth_requirement_selections",
                scope.resource.tenant_id.as_str(),
                key.as_str(),
            )
            .await?;
        let selected = selected.map(|(selected, _)| selected);
        if let Some(selected) = &selected {
            selected
                .verify_requested(scope, requirement)
                .map_err(|_| AuthProductError::BackendUnavailable)?;
            let (origin, _) = self
                .read_flow_document(selected.original_scope(), selected.original_flow())
                .await?
                .ok_or(AuthProductError::BackendUnavailable)?;
            let setup = origin
                .auth_custody
                .requirement_setup
                .as_deref()
                .ok_or(AuthProductError::BackendUnavailable)?;
            if origin.auth_custody.requirement_selection.as_deref() != Some(selected) {
                return Err(AuthProductError::BackendUnavailable);
            }
            selected
                .verify_receipt(&origin.flow, setup)
                .map_err(|_| AuthProductError::BackendUnavailable)?;
            if let Some(source) = &origin.auth_custody.requirement_source {
                source
                    .verify_selection(selected)
                    .map_err(|_| AuthProductError::BackendUnavailable)?;
            }
        }
        Ok(selected)
    }

    async fn requirement_selection_for_flow(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        flow_id: brassclaw_auth::AuthFlowId,
        requirement: &brassclaw_host_api::RuntimeCredentialAccountRequirement,
    ) -> Result<Option<brassclaw_auth::CredentialRequirementSelection>, AuthProductError> {
        let Some((document, _)) = self.read_flow_document(scope, flow_id).await? else {
            return Ok(None);
        };
        document.auth_custody.original_requirement_selection(
            scope,
            flow_id,
            &document.flow,
            requirement,
        )
    }

    async fn requirement_source_for_flow(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        flow_id: brassclaw_auth::AuthFlowId,
        requirement: &brassclaw_host_api::RuntimeCredentialAccountRequirement,
    ) -> Result<Option<brassclaw_auth::CredentialRequirementSource>, AuthProductError> {
        let Some((document, _)) = self.read_flow_document(scope, flow_id).await? else {
            return Ok(None);
        };
        document.auth_custody.original_requirement_source(
            scope,
            flow_id,
            &document.flow,
            requirement,
        )
    }

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
    async fn refresh_action(
        &self,
        request: &CredentialRefreshRequest,
    ) -> Result<Option<brassclaw_auth::CredentialRefreshAction>, AuthProductError> {
        self.read_refresh_action(request).await
    }
    async fn begin_refresh(
        &self,
        prepared: brassclaw_auth::CredentialRefreshPrepared,
    ) -> Result<brassclaw_auth::CredentialRefreshAdmission, AuthProductError> {
        self.begin_refresh_action(prepared).await
    }
    async fn reconcile_refresh(
        &self,
        request: &CredentialRefreshRequest,
    ) -> Result<Option<brassclaw_auth::CredentialRefreshAction>, AuthProductError> {
        self.reconcile_refresh_action(request).await
    }

    async fn complete_refresh(
        &self,
        completion: brassclaw_auth::CredentialRefreshCompletion,
    ) -> Result<brassclaw_auth::CredentialRefreshAction, AuthProductError> {
        self.complete_refresh_action(completion).await
    }

    async fn compare_and_update_account(
        &self,
        expected: &CredentialAccount,
        request: NewCredentialAccount,
    ) -> Result<CredentialAccount, AuthProductError> {
        let lock = self.lock_for(format!("account:{}", expected.id));
        let _guard = lock.lock().await;
        let (mut current, revision) = self
            .read_account(&expected.scope, expected.id)
            .await?
            .ok_or(AuthProductError::CredentialMissing)?;
        if &current != expected {
            return Err(AuthProductError::BackendConflict);
        }
        update_account_from_request(&mut current, request, Utc::now())?;
        // The revision comes from the same read as the exact snapshot check.
        // An independent writer between read and write is rejected by SQL CAS.
        self.write_account(&current, Some(revision)).await?;
        Ok(current)
    }

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

// ---------------------------------------------------------------------------
// RebornManualTokenFlowService (inline — private helpers are not pub)
// ---------------------------------------------------------------------------

#[async_trait]
impl RebornManualTokenFlowService for PgAuthProductServices {
    async fn request_manual_token_flow(
        &self,
        request: ManualTokenSetupRequest,
    ) -> Result<AuthChallenge, AuthProductError> {
        crate::manual_token_flow::request_manual_token_flow_with(self, self, request).await
    }

    async fn submit_original_manual_token_flow(
        &self,
        scope: &AuthProductScope,
        identity: ManualTokenSetupIdentity,
        secret: secrecy::SecretString,
    ) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError> {
        let (result, flow) = self
            .submit_manual_native(
                scope,
                SecretSubmitRequest {
                    interaction_id: identity.interaction_id,
                    secret,
                },
                true,
                Some(identity.flow_id),
            )
            .await?;
        Ok((result, flow.ok_or(AuthProductError::BackendConflict)?))
    }

    async fn submit_manual_token_flow(
        &self,
        scope: &AuthProductScope,
        request: SecretSubmitRequest,
    ) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError> {
        let (result, flow) = self
            .submit_manual_native(scope, request, true, None)
            .await?;
        let flow = flow.ok_or(AuthProductError::BackendConflict)?;
        Ok((result, flow))
    }

    async fn recover_original_manual_token_flow(
        &self,
        scope: &AuthProductScope,
        identity: ManualTokenSetupIdentity,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
        self.recover_native_manual_setup_original(scope, identity)
            .await
    }

    async fn request_original_manual_token_flow(
        &self,
        identity: ManualTokenSetupIdentity,
        request: ManualTokenSetupRequest,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        self.request_native_manual_setup_original(identity, request)
            .await
    }

    async fn continue_original_manual_token_flow(
        &self,
        scope: &AuthProductScope,
        identity: ManualTokenSetupIdentity,
    ) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError> {
        self.continue_native_manual_original(scope, identity).await
    }

    async fn abandon_manual_token_flow(
        &self,
        scope: &AuthProductScope,
        interaction_id: AuthInteractionId,
    ) -> Result<bool, AuthProductError> {
        let canceled = self.cancel_manual_token(scope, interaction_id).await?;
        if canceled.as_ref().is_some_and(|flow| {
            is_terminal_status(flow.status) && flow.status != AuthFlowStatus::Canceled
        }) {
            return Ok(false);
        }
        let abandoned = self.abandon_manual_token(scope, interaction_id).await?;
        Ok(abandoned || canceled.is_some_and(|flow| flow.status == AuthFlowStatus::Canceled))
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

#[cfg(all(test, feature = "skills-db"))]
#[path = "pg_auth_refresh_tests.rs"]
mod refresh_tests;

#[cfg(all(test, feature = "skills-db"))]
#[path = "pg_auth_requirement_tests.rs"]
mod requirement_tests;
