//! Postgres-backed [`ExtensionInstallationStore`] implementation.
//!
//! Stores manifests in `brassclaw_extension_manifests` (V010) as raw TOML
//! (since `ExtensionManifestRecord` is not directly serializable) and
//! installations in `brassclaw_extensions` as JSONB.
//!
//! Activation state: `installed`, `disabled`, `enabled`.
//! `delete_installation` soft-deletes (sets `removed_at = now()`).

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_host_api::HostPortCatalog;
use brassclaw_pg::PgPool;
use serde_json::Value;

use crate::installations::{
    ExtensionActivationState, ExtensionHealthSnapshot, ExtensionInstallation,
    ExtensionInstallationError, ExtensionInstallationId, ExtensionInstallationStore,
    ExtensionManifestRecord,
};
use crate::{ExtensionId, ManifestSource};

fn map_pool(e: deadpool_postgres::PoolError) -> ExtensionInstallationError {
    ExtensionInstallationError::InvalidInstallation {
        reason: e.to_string(),
    }
}

fn map_pg(e: tokio_postgres::Error) -> ExtensionInstallationError {
    ExtensionInstallationError::InvalidInstallation {
        reason: e.to_string(),
    }
}

fn map_json(e: serde_json::Error) -> ExtensionInstallationError {
    ExtensionInstallationError::InvalidInstallation {
        reason: e.to_string(),
    }
}

fn activation_str(state: ExtensionActivationState) -> &'static str {
    match state {
        ExtensionActivationState::Installed => "installed",
        ExtensionActivationState::Disabled => "disabled",
        ExtensionActivationState::Enabled => "enabled",
    }
}

fn manifest_from_raw_toml(
    raw: String,
) -> Result<ExtensionManifestRecord, ExtensionInstallationError> {
    let catalog = HostPortCatalog::empty();
    ExtensionManifestRecord::from_toml(raw, ManifestSource::InstalledLocal, &catalog, None)
}

fn installation_from_value(
    payload: Value,
) -> Result<ExtensionInstallation, ExtensionInstallationError> {
    serde_json::from_value(payload).map_err(map_json)
}

/// Postgres-backed [`ExtensionInstallationStore`].
pub struct PgExtensionInstallationStore {
    pool: Arc<PgPool>,
    tenant_id: String,
}

impl PgExtensionInstallationStore {
    pub fn new(pool: Arc<PgPool>, tenant_id: impl Into<String>) -> Self {
        Self {
            pool,
            tenant_id: tenant_id.into(),
        }
    }
}

#[async_trait]
impl ExtensionInstallationStore for PgExtensionInstallationStore {
    async fn list_manifests(
        &self,
    ) -> Result<Vec<ExtensionManifestRecord>, ExtensionInstallationError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let rows = client
            .query(
                "SELECT manifest FROM brassclaw_extension_manifests \
                 WHERE tenant_id = $1 ORDER BY name, version",
                &[&self.tenant_id],
            )
            .await
            .map_err(map_pg)?;
        let mut records = Vec::new();
        for row in rows {
            let raw_toml: String = row.get(0);
            if let Ok(r) = manifest_from_raw_toml(raw_toml) {
                records.push(r);
            }
        }
        Ok(records)
    }

    async fn get_manifest(
        &self,
        extension_id: &ExtensionId,
    ) -> Result<Option<ExtensionManifestRecord>, ExtensionInstallationError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let row = client
            .query_opt(
                "SELECT manifest FROM brassclaw_extension_manifests \
                 WHERE tenant_id = $1 AND name = $2 \
                 ORDER BY version DESC LIMIT 1",
                &[&self.tenant_id, &extension_id.as_str()],
            )
            .await
            .map_err(map_pg)?;
        match row {
            None => Ok(None),
            Some(r) => {
                let raw_toml: String = r.get(0);
                Ok(Some(manifest_from_raw_toml(raw_toml)?))
            }
        }
    }

    async fn upsert_manifest(
        &self,
        manifest: ExtensionManifestRecord,
    ) -> Result<(), ExtensionInstallationError> {
        let extension_id = manifest.extension_id().as_str().to_string();
        let version = manifest.manifest().version.clone();
        let raw_toml = manifest.raw_toml().to_string();
        let client = self.pool.get().await.map_err(map_pool)?;
        client
            .execute(
                "INSERT INTO brassclaw_extension_manifests \
                 (tenant_id, name, version, manifest) \
                 VALUES ($1, $2, $3, $4) \
                 ON CONFLICT (tenant_id, name, version) DO UPDATE \
                 SET manifest = excluded.manifest, updated_at = now()",
                &[&self.tenant_id, &extension_id, &version, &raw_toml],
            )
            .await
            .map_err(map_pg)?;
        Ok(())
    }

    async fn upsert_manifest_and_installation(
        &self,
        manifest: ExtensionManifestRecord,
        installation: ExtensionInstallation,
    ) -> Result<(), ExtensionInstallationError> {
        self.upsert_manifest(manifest).await?;
        self.upsert_installation(installation).await
    }

    async fn list_installations(
        &self,
    ) -> Result<Vec<ExtensionInstallation>, ExtensionInstallationError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let rows = client
            .query(
                "SELECT config FROM brassclaw_extensions \
                 WHERE tenant_id = $1 AND removed_at IS NULL \
                 ORDER BY updated_at DESC, id",
                &[&self.tenant_id],
            )
            .await
            .map_err(map_pg)?;
        rows.into_iter()
            .map(|r| installation_from_value(r.get(0)))
            .collect()
    }

    async fn list_enabled_installations(
        &self,
    ) -> Result<Vec<ExtensionInstallation>, ExtensionInstallationError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let rows = client
            .query(
                "SELECT config FROM brassclaw_extensions \
                 WHERE tenant_id = $1 AND activation_state = 'enabled' \
                   AND removed_at IS NULL \
                 ORDER BY updated_at DESC, id",
                &[&self.tenant_id],
            )
            .await
            .map_err(map_pg)?;
        rows.into_iter()
            .map(|r| installation_from_value(r.get(0)))
            .collect()
    }

    async fn get_installation(
        &self,
        installation_id: &ExtensionInstallationId,
    ) -> Result<Option<ExtensionInstallation>, ExtensionInstallationError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let row = client
            .query_opt(
                "SELECT config FROM brassclaw_extensions \
                 WHERE id = $1 AND tenant_id = $2 AND removed_at IS NULL",
                &[&installation_id.as_str(), &self.tenant_id],
            )
            .await
            .map_err(map_pg)?;
        match row {
            None => Ok(None),
            Some(r) => Ok(Some(installation_from_value(r.get(0))?)),
        }
    }

    async fn upsert_installation(
        &self,
        installation: ExtensionInstallation,
    ) -> Result<(), ExtensionInstallationError> {
        let payload = serde_json::to_value(&installation).map_err(map_json)?;
        let activation = activation_str(installation.activation_state());
        let extension_id = installation.extension_id().as_str().to_string();
        let version = installation
            .manifest_ref()
            .manifest_hash()
            .map(|h| h.as_str().to_string())
            .unwrap_or_default();
        let client = self.pool.get().await.map_err(map_pool)?;
        client
            .execute(
                "INSERT INTO brassclaw_extensions \
                 (id, tenant_id, user_id, name, version, activation_state, config) \
                 VALUES ($1, $2, $2, $3, $4, $5, $6) \
                 ON CONFLICT (id) DO UPDATE \
                 SET activation_state = excluded.activation_state, \
                     config = excluded.config, \
                     updated_at = now() \
                 WHERE brassclaw_extensions.removed_at IS NULL",
                &[
                    &installation.installation_id().as_str(),
                    &self.tenant_id,
                    &extension_id,
                    &version,
                    &activation,
                    &payload,
                ],
            )
            .await
            .map_err(map_pg)?;
        Ok(())
    }

    async fn set_activation_state(
        &self,
        installation_id: &ExtensionInstallationId,
        state: ExtensionActivationState,
    ) -> Result<(), ExtensionInstallationError> {
        let state_str = activation_str(state);
        let client = self.pool.get().await.map_err(map_pool)?;
        client
            .execute(
                "UPDATE brassclaw_extensions \
                 SET activation_state = $1, updated_at = now() \
                 WHERE id = $2 AND tenant_id = $3 AND removed_at IS NULL",
                &[&state_str, &installation_id.as_str(), &self.tenant_id],
            )
            .await
            .map_err(map_pg)?;
        Ok(())
    }

    async fn delete_installation(
        &self,
        installation_id: &ExtensionInstallationId,
    ) -> Result<(), ExtensionInstallationError> {
        // Soft-delete per §4.11.
        let client = self.pool.get().await.map_err(map_pool)?;
        client
            .execute(
                "UPDATE brassclaw_extensions \
                 SET removed_at = now(), updated_at = now() \
                 WHERE id = $1 AND tenant_id = $2 AND removed_at IS NULL",
                &[&installation_id.as_str(), &self.tenant_id],
            )
            .await
            .map_err(map_pg)?;
        Ok(())
    }

    async fn delete_manifest(
        &self,
        extension_id: &ExtensionId,
    ) -> Result<(), ExtensionInstallationError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        client
            .execute(
                "DELETE FROM brassclaw_extension_manifests \
                 WHERE tenant_id = $1 AND name = $2",
                &[&self.tenant_id, &extension_id.as_str()],
            )
            .await
            .map_err(map_pg)?;
        Ok(())
    }

    async fn update_health(
        &self,
        installation_id: &ExtensionInstallationId,
        health: ExtensionHealthSnapshot,
    ) -> Result<(), ExtensionInstallationError> {
        let health_val = serde_json::to_value(&health).map_err(map_json)?;
        let client = self.pool.get().await.map_err(map_pool)?;
        client
            .execute(
                "UPDATE brassclaw_extensions \
                 SET config = jsonb_set(config, '{health}', $1), updated_at = now() \
                 WHERE id = $2 AND tenant_id = $3 AND removed_at IS NULL",
                &[&health_val, &installation_id.as_str(), &self.tenant_id],
            )
            .await
            .map_err(map_pg)?;
        Ok(())
    }
}
